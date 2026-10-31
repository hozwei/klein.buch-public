//! Lauf F8 (ADR 0040 Nr. 63): „Datenbank sperren“ und „Passphrase ändern“ mit
//! festem Ausgang. Eigenes Test-Binary: Der Test zum Passphrase-Wechsel hält die
//! prozessweite Backup-Sperre rund 20 s, der Sperr-Test wartet über
//! `DB_GATE_WAIT` — beides soll die zeitkritischen Anmelde-Tests in
//! `server_auth_test` nicht ausbremsen.

mod server_support;

use axum::http::StatusCode;
use server_support::*;

/// Lauf F8 (ADR 0040 Nr. 63): Hält ein Backup (hier vom Test gehalten) die
/// Backup-Sperre, wartet „Passphrase ändern“ höchstens `REKEY_WAIT_BUDGET` und
/// antwortet dann vor dem Antwort-Timeout mit 503 `busy` — nie mit `timeout`,
/// nach dem der Wechsel im Hintergrund doch noch wirksam würde. Nichts ist
/// geändert, der Scheduler läuft weiter; ohne laufendes Backup gelingt der
/// Wechsel.
#[tokio::test]
async fn passphrase_change_waits_for_running_backup() {
    use klein_buch_lib::server::limits::REQUEST_TIMEOUT;
    use klein_buch_lib::server::state::REKEY_WAIT_BUDGET;
    use std::sync::atomic::Ordering;
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let body =
        serde_json::json!({ "newPassphrase": "ganz neue lange Passphrase 2026" }).to_string();
    let r = {
        // Eigenes Test-Binary: kein anderer Test wartet auf diese Sperre.
        let backup =
            klein_buch_lib::backup::exclusive_against_backups(std::time::Duration::from_secs(60))
                .await
                .expect("Backup-Sperre frei");
        let started = std::time::Instant::now();
        let r = send(
            &srv.app,
            post("/api/auth/passphrase", &body, Some(&cookie), None),
        )
        .await;
        let took = started.elapsed();
        drop(backup);
        assert!(
            took >= REKEY_WAIT_BUDGET - std::time::Duration::from_secs(1) && took < REQUEST_TIMEOUT,
            "nach {took:?}"
        );
        r
    };
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{}", r.text());
    assert_eq!(r.json()["code"], "busy");
    assert!(srv.state.ctx.is_unlocked());
    assert!(!srv.state.ctx.scheduler_stop.load(Ordering::SeqCst));
    assert!(!srv.state.ctx.scheduler_paused.load(Ordering::SeqCst));
    let pool = srv.state.ctx.pool().unwrap();
    assert!(klein_buch_lib::backup::verify_passphrase(&pool, PASS)
        .await
        .unwrap());
    // ADR 0042 Nr. 16 (U7-A06): Ein von der gehaltenen Sperre aufgestautes
    // Backup erst zu Ende laufen lassen.
    drop(
        klein_buch_lib::backup::exclusive_against_backups(std::time::Duration::from_secs(120))
            .await
            .expect("Backup nach 120 s nicht fertig"),
    );
    let r = {
        let _serial = kdf_serial().await;
        send(
            &srv.app,
            post("/api/auth/passphrase", &body, Some(&cookie), None),
        )
        .await
    };
    if r.status == StatusCode::SERVICE_UNAVAILABLE && r.json()["code"] == "timeout" {
        // Unter Last (Debug-Build, volle Suite) braucht das Neuverschlüsseln
        // länger als der Antwort-Timeout; der Wechsel läuft dann im Hintergrund
        // zu Ende (er hält `transition` bis zum Schluss).
        //
        // ADR 0042 Nr. 16 (V6-R02): Weg der Oberfläche. `/api/auth/passphrase`
        // läuft nicht über die Idempotenz der Commands; `authChangePassphrase`
        // (transport.ts) wiederholt deshalb nicht, sondern meldet bei genau
        // diesem Code „Ergebnis unklar“ (`PassphraseChangeUnclear`) und rät,
        // zuerst die neue Passphrase zu versuchen. Geprüft wird, dass dieser
        // Rat stimmt: Der Wechsel wird wirksam, die Sitzung bleibt gültig, die
        // bisherige Passphrase gilt nicht mehr.
        let _done = tokio::time::timeout(
            std::time::Duration::from_secs(180),
            srv.state.transition.lock(),
        )
        .await
        .expect("Passphrase-Wechsel nach 180 s nicht fertig");
    } else {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    // Die Sitzung des Wechsels bleibt gültig (W14: nur andere Sitzungen enden).
    let s = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(s.status, StatusCode::OK, "{}", s.text());
    let pool = srv
        .state
        .ctx
        .pool()
        .expect("entsperrt mit der neuen Passphrase");
    assert!(
        klein_buch_lib::backup::verify_passphrase(&pool, "ganz neue lange Passphrase 2026")
            .await
            .unwrap()
    );
    assert!(
        !klein_buch_lib::backup::verify_passphrase(&pool, PASS)
            .await
            .unwrap(),
        "bisherige Passphrase gilt nicht mehr"
    );
}

/// Lauf F8 (ADR 0040 Nr. 63): „Datenbank sperren“ bricht nach dem Beenden der
/// Sitzungen nicht mehr ab, auch wenn eine laufende Aktion den `db_gate`
/// länger als `DB_GATE_WAIT` hält — es sperrt danach zu Ende (früher: still
/// abgebrochen, Pool und Passphrase blieben, der Scheduler lief weiter).
#[tokio::test]
async fn lock_completes_after_long_running_action() {
    use klein_buch_lib::server::state::DB_GATE_WAIT;
    let srv = test_server();
    let cookie = setup(&srv).await;
    let action = srv.state.db_gate.read().await; // simulierte lange Aktion
    let lock = {
        let state = srv.state.clone();
        tokio::spawn(async move { klein_buch_lib::server::auth::lock_database(&state).await })
    };
    tokio::time::sleep(DB_GATE_WAIT + std::time::Duration::from_secs(2)).await;
    assert!(!lock.is_finished(), "wartet weiter statt abzubrechen");
    let r = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "Sitzungen beendet");
    drop(action);
    assert!(lock.await.unwrap().is_ok(), "zu Ende gesperrt");
    assert!(!srv.state.ctx.is_unlocked(), "Pool zu");
    assert!(
        srv.state.ctx.session.get().is_none(),
        "Passphrase aus dem RAM"
    );
}
