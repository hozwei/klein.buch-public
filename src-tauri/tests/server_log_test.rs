//! Plan W5.9 — keine Geheimnisse im Log.
//!
//! Globaler Subscriber auf TRACE (ohne die Server-Direktive `sqlx::query=off`,
//! damit der Test auch `db::open_pool` prüft) in einen Puffer: Pool mit Key,
//! ein Pragma, das hinter einer gehaltenen Schreibsperre über 1 s wartet
//! („slow statement“), Einrichtung, Anmeldung und Commands über den Router.
//! Keine Zeile darf Passphrase, `PRAGMA key`, Sitzungs-ID, Setup-Token oder
//! TOTP-Code enthalten (Plan W5.9/W6). Der Setup-Token steht nur als eigene
//! Zeile auf stderr, nie im `tracing`-Log.
//!
//! Lauf F4c: auch Backup-Snapshot und Passphrase-Wechsel unter TRACE
//! (W5-LOG-TESTS), Ablehnung eines SMTP-Hosts mit Zeilenumbruch (W14-017) und
//! das Zugriffsprotokoll mit `ip=`, `req=` (= `ref` interner Fehler) und
//! `sess=` (W14-018).

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
    let cookie3 = session_cookie(&reauth_reply);
    let r = call(&srv.app, &cookie3, "contacts_list", serde_json::json!({})).await;
    assert!(r.status.is_success());
    // W14: SMTP-Passwort beim Anlegen eines Kontos (SecretStore) und das
    // TOTP-Geheimnis dürfen ebenfalls nie im Log stehen.
    let smtp_password = "smtp-log-geheim-4711";
    let r = call(
        &srv.app,
        &cookie3,
        "mail_account_create",
        serde_json::json!({
            "input": {
                "label": "Log-Test", "authType": "smtp_password", "smtpHost": "smtp.example.org",
                "smtpPort": 587, "smtpUser": "kb@example.org", "smtpUseTls": true,
                "fromEmail": "rechnung@example.org", "fromName": "Beispiel IT-Service",
                "isDefault": true, "oauthTenantId": null, "oauthClientId": null
            },
            "password": smtp_password
        }),
    )
    .await;
    assert!(r.status.is_success(), "{}", r.text());
    let totp_b32: String = {
        let path = srv.state.ctx.paths.data_dir.join("auth").join("totp.json");
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        file["secret_b32"].as_str().unwrap().to_string()
    };
    let _ = send(
        &srv.app,
        get("/api/auth/status?probe=geheim-im-query", None),
    )
    .await;
    let wrong = login(&srv, "falsche-passphrase-0000000").await;
    assert_eq!(wrong.status.as_u16(), 401);
    // ASVS V2.8.5 (Lauf F4a): der eben verbrauchte Code noch einmal — gleiche
    // Antwort, im Log aber `reason=totp_replay`.
    let used = klein_buch_lib::server::totp::code_for_step(
        &totp_secret(&srv),
        klein_buch_lib::server::totp::step_at(srv.state.totp_now()),
    );
    let replay = send(
        &srv.app,
        post(
            "/api/auth/login",
            &serde_json::json!({ "passphrase": PASS, "totpCode": used }).to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(replay.status.as_u16(), 401);

    // W5-LOG-TESTS (Lauf F4c): Backup-Snapshot (verschlüsselte Kopie der DB)
    // und Passphrase-Wechsel (Umschlüsseln) unter TRACE — weder die alte noch
    // die neue Passphrase im Log; danach ein Backup mit der neuen.
    let backup = call(
        &srv.app,
        &cookie3,
        "backup_create_now",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(backup.json()["state"], "done", "{}", backup.text());
    let new_pass = "neue-log-passphrase-zum-testen";
    let changed = {
        let _serial = kdf_serial().await;
        send(
            &srv.app,
            post(
                "/api/auth/passphrase",
                &serde_json::json!({ "newPassphrase": new_pass }).to_string(),
                Some(&cookie3),
                None,
            ),
        )
        .await
    };
    assert_eq!(changed.status.as_u16(), 200, "{}", changed.text());
    let backup = call(
        &srv.app,
        &cookie3,
        "backup_create_now",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(backup.json()["state"], "done", "{}", backup.text());

    // W14-017 (Lauf F4c): Ein SMTP-Host mit Zeilenumbruch wird abgelehnt —
    // beim Anlegen und beim Verbindungstest; die Meldung wiederholt ihn nicht.
    let forged = "smtp.example.org\n2026-10-25T10:00:00.000000Z  WARN klein_buch_lib::auth: \
                  auth.login.failed ip=192.0.2.66 reason=totp";
    let r = call(
        &srv.app,
        &cookie3,
        "mail_account_create",
        serde_json::json!({
            "input": {
                "label": "Fälschung", "authType": "smtp_password", "smtpHost": forged,
                "smtpPort": 587, "smtpUser": null, "smtpUseTls": true,
                "fromEmail": "rechnung@example.org", "fromName": "Beispiel IT-Service",
                "isDefault": false, "oauthTenantId": null, "oauthClientId": null
            },
            "password": null
        }),
    )
    .await;
    assert_eq!(r.status.as_u16(), 422, "{}", r.text());
    assert!(!r.text().contains("192.0.2.66"), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie3,
        "mail_account_test_connection",
        serde_json::json!({ "args": {
            "smtpHost": forged, "smtpPort": 587, "smtpUseTls": true,
            "smtpUser": null, "password": null
        }}),
    )
    .await;
    assert_eq!(r.status.as_u16(), 422, "{}", r.text());

    // W14-018 (Lauf F4c): Zugriffsprotokoll mit ip=, req= und sess=; die `ref`
    // eines internen Fehlers ist die Kennung der Anfrage.
    let routes = axum::Router::<std::sync::Arc<klein_buch_lib::server::state::ServerState>>::new()
        .route(
            "/api/panik",
            axum::routing::get(|| async {
                if std::hint::black_box(true) {
                    panic!("panik-im-log-test");
                }
                "nie"
            }),
        );
    let panic_app = klein_buch_lib::server::layered(routes, srv.state.clone());
    let boom = send(
        &panic_app,
        from_peer(get("/api/panik", Some(&cookie3)), "203.0.113.9:40000"),
    )
    .await;
    assert_eq!(boom.status.as_u16(), 500);
    let reference = boom.json()["ref"].as_str().unwrap().to_string();
    let sess = {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_str(&format!("__Host-kb_session={cookie3}")).unwrap(),
        );
        klein_buch_lib::server::session::short_id_from_headers(&headers).unwrap()
    };

    let text = String::from_utf8_lossy(&buffer.0.lock().unwrap()).into_owned();
    assert!(text.contains("auth.login.ok"), "Log wird erfasst");
    assert!(text.contains("auth.login.failed"), "Fehlversuch geloggt");
    assert!(text.contains("reason=totp_replay"), "Replay eigens geloggt");
    assert!(
        text.contains("auth.reauth.ok ip="),
        "Re-Auth mit IP geloggt"
    );
    assert!(text.contains("/api/auth/status"), "Zugriffsprotokoll");
    let access = text
        .lines()
        .find(|l| l.contains("GET /api/panik 500"))
        .unwrap_or_else(|| panic!("Zugriffszeile fehlt:\n{text}"));
    assert!(access.contains("ip=203.0.113.9 "), "{access}");
    assert!(access.contains(&format!("req={reference} ")), "{access}");
    assert!(access.ends_with(&format!("sess={sess}")), "{access}");
    assert!(
        text.lines()
            .any(|l| l.contains("interner Fehler") && l.contains(&reference)),
        "ERROR-Zeile mit derselben ref:\n{text}"
    );
    assert!(
        text.lines()
            .filter(|l| l.contains("klein_buch_lib::http:") && l.contains(" ms "))
            .all(|l| l.contains(" ip=") && l.contains(" req=") && l.contains(" sess=")),
        "jede Zugriffszeile mit ip=, req=, sess="
    );
    // Der Panik-Text geht nur über den Panic-Hook nach stderr, nie ins Log.
    assert!(!text.contains("panik-im-log-test"), "Panik-Text im Log");
    assert!(!text.contains("192.0.2.66"), "gefälschte Zeile im Log");
    for secret in [
        PASS,
        "falsche-passphrase-0000000",
        cookie.as_str(),
        cookie2.as_str(),
        cookie3.as_str(),
        "geheim-im-query",
        setup_token.as_str(),
        // 6 Ziffern kämen zufällig in Zeitstempeln vor → im JSON-Kontext suchen
        format!("\"{totp_code}\"").as_str(),
        "totpCode",
        recovery[0].as_str(),
        recovery[9].as_str(),
        smtp_password,
        totp_b32.as_str(),
        new_pass,
    ] {
        assert!(!text.contains(secret), "Geheimnis im Log: {secret}");
    }
    assert!(
        !text.to_ascii_lowercase().contains("pragma key"),
        "PRAGMA key im Log"
    );
}
