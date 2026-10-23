//! Plan W5.9 — keine Geheimnisse im Log.
//!
//! Globaler Subscriber auf TRACE (ohne die Server-Direktive `sqlx::query=off`,
//! damit der Test auch `db::open_pool` prüft) in einen Puffer: Pool mit Key,
//! ein Pragma, das hinter einer gehaltenen Schreibsperre über 1 s wartet
//! („slow statement“), Einrichtung, Anmeldung und Commands über den Router.
//! Keine Zeile darf Passphrase, `PRAGMA key`, Sitzungs-ID, Setup-Token oder
//! TOTP-Code enthalten (Plan W5.9/W6). Der Setup-Token steht nur als eigene
//! Zeile auf stderr, nie im `tracing`-Log.

mod server_support;

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use server_support::*;

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
    type Writer = Buffer;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_passphrase_or_session_id_in_logs() {
    let buffer = Buffer::default();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(buffer.clone())
        .try_init()
        .expect("einziger Subscriber in diesem Test-Binary");

    // 1. Pool mit Key + langsames Pragma hinter gehaltener Schreibsperre.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("probe.sqlite");
    let pool = klein_buch_lib::db::open_pool(&db, Some(PASS))
        .await
        .unwrap();
    sqlx::query("CREATE TABLE t (x INTEGER)")
        .execute(&pool)
        .await
        .unwrap();
    let mut holder = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *holder)
        .await
        .unwrap();
    let waiter = {
        let pool = pool.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            // neue Verbindung (PRAGMA key beim Aufbau), wartet auf die Sperre
            sqlx::query("PRAGMA user_version = 7")
                .execute(&pool)
                .await
                .unwrap();
            started.elapsed()
        })
    };
    tokio::time::sleep(Duration::from_millis(1500)).await;
    sqlx::query("COMMIT").execute(&mut *holder).await.unwrap();
    drop(holder);
    let waited = waiter.await.unwrap();
    assert!(
        waited >= Duration::from_secs(1),
        "Pragma hat nur {waited:?} gewartet"
    );
    pool.close().await;

    // 2. Einrichtung, Anmeldung, Commands über den Router.
    let srv = test_server();
    let setup_token = srv.state.setup_token_for_tests().unwrap();
    let setup = setup_reply(&srv).await;
    let cookie = session_cookie(&setup);
    let recovery: Vec<String> =
        serde_json::from_value(setup.json()["recoveryCodes"].clone()).unwrap();
    let totp_code = next_totp(&srv);
    let reply = send(
        &srv.app,
        post(
            "/api/auth/login",
            &serde_json::json!({ "passphrase": PASS, "totpCode": totp_code }).to_string(),
            None,
            None,
        ),
    )
    .await;
    let cookie2 = session_cookie(&reply);
    let reauth_reply = reauth(&srv, &cookie2).await;
    assert!(reauth_reply.status.is_success());
    let r = call(&srv.app, &cookie2, "contacts_list", serde_json::json!({})).await;
    assert!(r.status.is_success());
    let _ = send(
        &srv.app,
        get("/api/auth/status?probe=geheim-im-query", None),
    )
    .await;
    let wrong = login(&srv, "falsche-passphrase-0000000").await;
    assert_eq!(wrong.status.as_u16(), 401);

    let text = String::from_utf8_lossy(&buffer.0.lock().unwrap()).into_owned();
    assert!(text.contains("auth.login.ok"), "Log wird erfasst");
    assert!(text.contains("auth.login.failed"), "Fehlversuch geloggt");
    assert!(text.contains("/api/auth/status"), "Zugriffsprotokoll");
    for secret in [
        PASS,
        "falsche-passphrase-0000000",
        cookie.as_str(),
        cookie2.as_str(),
        "geheim-im-query",
        setup_token.as_str(),
        // 6 Ziffern kämen zufällig in Zeitstempeln vor → im JSON-Kontext suchen
        format!("\"{totp_code}\"").as_str(),
        "totpCode",
        recovery[0].as_str(),
        recovery[9].as_str(),
    ] {
        assert!(!text.contains(secret), "Geheimnis im Log: {secret}");
    }
    assert!(
        !text.to_ascii_lowercase().contains("pragma key"),
        "PRAGMA key im Log"
    );
}
