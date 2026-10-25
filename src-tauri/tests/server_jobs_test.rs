//! Plan W5.6 — Jobs und Abbruchsicherheit: Ein abgebrochener
//! Festschreiben-Request hinterlässt nichts halb, der Job läuft zu Ende, und
//! die Wiederholung mit demselben Idempotenz-Schlüssel liefert dasselbe
//! Ergebnis (ohne „existiert mit anderem Hash“). Dazu: Trigger-Fehler → 409.

mod server_support;

use axum::http::StatusCode;
use klein_buch_lib::error::Error;
use klein_buch_lib::server::errors::ApiError;
use server_support::*;
use tower::ServiceExt as _;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborted_lock_request_finishes_and_replays() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let id = draft_invoice(&srv).await;
    let body = serde_json::json!({ "id": id }).to_string();

    // Request starten und abbrechen, sobald der Job läuft.
    let app = srv.app.clone();
    let req = post(
        "/api/cmd/invoices_lock_and_issue",
        &body,
        Some(&cookie),
        Some("lock-1"),
    );
    let request = tokio::spawn(async move { app.oneshot(req).await });
    for _ in 0..200 {
        if srv.state.jobs.active() > 0 || request.is_finished() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    request.abort();

    // Wiederholung mit demselben Schlüssel → derselbe Job.
    let again = send(
        &srv.app,
        post(
            "/api/cmd/invoices_lock_and_issue",
            &body,
            Some(&cookie),
            Some("lock-1"),
        ),
    )
    .await;
    assert_eq!(again.status, StatusCode::ACCEPTED, "{}", again.text());
    let job_id = again.json()["jobId"].as_str().unwrap().to_string();
    let done = wait_job(&srv.app, &cookie, &job_id).await;
    assert_eq!(done.json()["state"], "done", "{}", done.text());
    let number = done.json()["result"]["invoice"]["invoiceNumber"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| panic!("Rechnungsnummer fehlt: {}", done.text()));

    // Noch einmal nach dem Ende: gleiche Job-ID, gleiches Ergebnis.
    let third = send(
        &srv.app,
        post(
            "/api/cmd/invoices_lock_and_issue",
            &body,
            Some(&cookie),
            Some("lock-1"),
        ),
    )
    .await;
    assert_eq!(third.json()["jobId"], job_id);
    let view = wait_job(&srv.app, &cookie, &job_id).await;
    assert_eq!(view.json()["result"]["invoice"]["invoiceNumber"], number);

    // Genau ein Festschreiben: Rechnung gesperrt, Archiv ohne Dubletten.
    let pool = srv.state.ctx.pool().unwrap();
    let locked: Option<String> = sqlx::query_scalar("SELECT locked_at FROM invoices WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(locked.is_some());
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.lock' AND entity_id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "genau ein Festschreiben-Audit");
    let archived: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM archive_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(archived, 2, "PDF + XML genau einmal archiviert");

    // Ein neuer Schlüssel für dieselbe Rechnung: fachlicher Fehler, kein Hash-Konflikt.
    let other = call(
        &srv.app,
        &cookie,
        "invoices_lock_and_issue",
        serde_json::json!({"id": id}),
    )
    .await;
    assert_eq!(other.json()["state"], "failed", "{}", other.text());
    let msg = other.json()["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(!msg.contains("anderem Hash"), "{msg}");

    // Trigger-Fehler (Update einer festgeschriebenen Rechnung) → 409 mit
    // deutscher Meldung und `ref`; der englische Trigger-Text steht nur im Log
    // (R1-S3-TRIGGERTEXT).
    let err =
        sqlx::query("UPDATE invoices SET net_amount_cents = net_amount_cents + 1 WHERE id = ?")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap_err();
    let api = ApiError::from_error(&Error::Db(err));
    assert_eq!(api.status, StatusCode::CONFLICT);
    assert_eq!(api.code, "constraint");
    assert!(
        api.message.starts_with("Die Rechnung ist festgeschrieben"),
        "{}",
        api.message
    );
    assert!(
        !api.message.contains("invoice is locked"),
        "{}",
        api.message
    );
    assert!(api.reference.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn job_status_requires_session_and_unknown_is_404() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let r = call(
        &srv.app,
        &cookie,
        "backup_create_now",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    let r = send(
        &srv.app,
        get(
            &format!("/api/jobs/{}", uuid::Uuid::new_v4()),
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(&srv.app, get("/api/jobs/kein-uuid", Some(&cookie))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

/// Plan W5.5: Liegt ein Archiv-Eintrag nicht unter dem aktuellen `archive_dir`,
/// wird das Entsperren verweigert — ohne dass Pool, Scheduler oder Audit
/// aktiv werden.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unlock_is_refused_when_archive_paths_do_not_match() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let pool = srv.state.ctx.pool().unwrap();
    sqlx::query(
        "INSERT INTO archive_entries
         (id, file_path, file_name, file_hash_sha256, file_size_bytes, mime_type, source)
         VALUES ('a1', '/anderes/volume/archive/2026/RE-2026-0001.pdf', 'RE-2026-0001.pdf',
                 'abc', 4, 'application/pdf', 'invoice')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let audits_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'backup.unlock.ok'")
            .fetch_one(&pool)
            .await
            .unwrap();
    srv.state.ctx.take_pool().unwrap().close().await;
    srv.state.ctx.session.clear();

    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(r.json()["code"], "unlock_refused");
    assert!(!srv.state.ctx.is_unlocked(), "bleibt gesperrt");
    assert!(
        srv.state.ctx.session.get().is_none(),
        "keine Passphrase im RAM"
    );
    let r = send(&srv.app, get("/api/auth/status", Some(&cookie))).await;
    assert_eq!(r.json()["state"], "login_required");

    let pool = klein_buch_lib::db::open_pool(&srv.state.ctx.paths.db_file, Some(PASS))
        .await
        .unwrap();
    let audits_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'backup.unlock.ok'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audits_after, audits_before, "kein Entsperr-Audit");
    pool.close().await;
}

/// Nach einem erfolgreichen Restore: Änderungen → 503, Lesen weiter möglich,
/// `/healthz` 503, nach der Schonfrist geordnetes Herunterfahren.
#[tokio::test(start_paused = true)]
async fn restart_after_restore_blocks_writes_and_shuts_down() {
    let srv = test_server();
    // Sitzungen gelten nur mit eingerichtetem zweiten Faktor (W6).
    srv.state
        .totp
        .install(
            &klein_buch_lib::server::totp::generate_secret(),
            1_800_000_000,
        )
        .unwrap();
    let token = srv.state.sessions.create();
    srv.state.request_restart();
    assert!(srv.state.is_stopping());

    let r = send(
        &srv.app,
        post("/api/cmd/contacts_create", "{}", Some(&token), Some("k")),
    )
    .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(r.json()["code"], "restarting");
    // Lesen geht weiter bis zur DB (hier gesperrt → 423, nicht 503)
    let r = send(
        &srv.app,
        post("/api/cmd/contacts_list", "{}", Some(&token), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::LOCKED);
    let r = send(&srv.app, get("/healthz", None)).await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);

    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        srv.state.shutdown.cancelled(),
    )
    .await
    .expect("Herunterfahren nach der Schonfrist");
    srv.state.finish_shutdown().await;
}
