//! Plan W7 — Restore und Pflicht-Backup vor Migration (W7c):
//! - Restore per Upload und aus der Historie (nur lokaler Backup-Ordner)
//! - Disaster Recovery: frische Instanz, anderes Backup-Passwort, Neustart,
//!   Anmeldung mit der Backup-Passphrase und dem aktuellen TOTP
//! - abgelehnt: Schema > EXPECTED / < 30, `../`, absoluter Pfad, fremder Name,
//!   Zip-Bombe, falsche Passphrase (Upload bleibt) — Staging danach leer, kein
//!   Pre-Restore-Backup vor der Passphrase-Prüfung
//! - Schreib-Commands während Phase A → 503
//! - fehlende eigene Vorlage nach dem Restore → Inbox-Warnung
//! - Pflicht-Backup beim Entsperren einer älteren DB, scheiterndes Backup →
//!   keine Migration

mod server_support;

use std::borrow::Cow;
use std::io::Write;

use axum::http::StatusCode;
use klein_buch_lib::backup::restore::{RESTORE_MARKER, RESTORE_STAGING};
use klein_buch_lib::db::schema_version::{EXPECTED_SCHEMA_VERSION, MIN_RESTORE_SCHEMA_VERSION};
use klein_buch_lib::domain::recurring::RecurringInput;
use klein_buch_lib::domain::recurring_invoice::RecurringInvoiceInput;
use server_support::*;

const OTHER: &str = "anderes-pferd-batterie-klammer";

async fn scalar_i64(srv: &TestServer, sql: &str) -> i64 {
    let pool = srv.state.ctx.pool().unwrap();
    sqlx::query_scalar(sql).fetch_one(&pool).await.unwrap()
}

/// Backup über den Command; liefert die Bytes der `.kbk`.
async fn backup_bytes(srv: &TestServer, cookie: &str) -> Vec<u8> {
    // Argon2id; nicht parallel zu den Anmeldungen anderer Tests (KDF-Platz 1)
    let _serial = kdf_serial().await;
    let r = call(&srv.app, cookie, "backup_create_now", serde_json::json!({})).await;
    let body = r.json();
    let out = if body.get("state").is_some() {
        assert_eq!(body["state"], "done", "{}", r.text());
        body["result"].clone()
    } else {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        body
    };
    let name = out["fileName"].as_str().unwrap();
    std::fs::read(srv.state.ctx.paths.backups_dir.join(name)).unwrap()
}

fn staging_is_clean(srv: &TestServer) -> bool {
    let staging = srv.state.ctx.paths.data_dir.join(RESTORE_STAGING);
    !staging.exists() || std::fs::read_dir(&staging).unwrap().next().is_none()
}

fn marker_exists(srv: &TestServer) -> bool {
    srv.state.ctx.paths.data_dir.join(RESTORE_MARKER).exists()
}

async fn restore_upload(srv: &TestServer, cookie: &str, kbk: &[u8], pass: &str) -> Reply {
    // Entschlüsseln + Umschlüsseln über die KDF; serialisiert wie login()
    let _serial = kdf_serial().await;
    let id = upload_id(srv, cookie, "restore", "sicherung.kbk", kbk).await;
    call(
        &srv.app,
        cookie,
        "backup_restore_apply",
        serde_json::json!({ "uploadId": id, "passphrase": pass }),
    )
    .await
}

/// Ersetzt die Schema-Version im (unverschlüsselten) Manifest-Kopf.
fn with_header_schema(kbk: &[u8], schema: i32) -> Vec<u8> {
    let nl1 = kbk.iter().position(|b| *b == b'\n').unwrap();
    let rest = &kbk[nl1 + 1..];
    let nl2 = rest.iter().position(|b| *b == b'\n').unwrap();
    let mut m: serde_json::Value = serde_json::from_slice(&rest[..nl2]).unwrap();
    m["schemaVersion"] = serde_json::json!(schema);
    let mut out = kbk[..=nl1].to_vec();
    out.extend_from_slice(serde_json::to_string(&m).unwrap().as_bytes());
    out.extend_from_slice(&rest[nl2..]);
    out
}

/// Schreibt ein Backup als Format 2 um (Hülle ohne AAD, wie bis Lauf F3a).
fn as_format_2(kbk: &[u8], pass: &str) -> Vec<u8> {
    use klein_buch_lib::backup::{encrypt, manifest};
    let (mut m, content) = klein_buch_lib::backup::restore::decrypt_backup(kbk, pass).unwrap();
    let salt = encrypt::random_bytes(encrypt::SALT_LEN);
    let nonce = encrypt::random_bytes(encrypt::NONCE_LEN);
    let key = encrypt::derive_key(pass.as_bytes(), &salt).unwrap();
    let ct = encrypt::encrypt(&key, &encrypt::nonce_array(&nonce).unwrap(), &content).unwrap();
    m.format_version = 2;
    m.salt_hex = manifest::to_hex(&salt);
    m.nonce_hex = manifest::to_hex(&nonce);
    manifest::frame(&m, &ct).unwrap()
}

/// Verschlüsseltes Backup aus beliebigen ZIP-Einträgen (Angriffsfälle).
fn sealed(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::<u8>::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        zw.start_file(*name, opts).unwrap();
        zw.write_all(bytes).unwrap();
    }
    let zip_bytes = zw.finish().unwrap().into_inner();
    let sha = klein_buch_lib::backup::snapshot::sha256_hex(&zip_bytes);
    klein_buch_lib::backup::seal_backup(&zip_bytes, &sha, PASS, EXPECTED_SCHEMA_VERSION)
        .unwrap()
        .1
}

/// Festgeschriebene Rechnung (Archiveinträge); liefert die PDF-Archiv-ID.
async fn locked_invoice(srv: &TestServer, cookie: &str) -> String {
    let id = draft_invoice(srv).await;
    let r = call(
        &srv.app,
        cookie,
        "invoices_lock_and_issue",
        serde_json::json!({ "id": id }),
    )
    .await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    r.json()["result"]["pdfArchiveId"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Instanz zerstören (Datenverzeichnis gelöscht) und auf demselben Pfad frisch
/// aufsetzen — wie ein neuer Container mit demselben `/data`-Mount.
async fn destroy_and_recreate(srv: TestServer) -> TestServer {
    srv.state.finish_shutdown().await;
    let TestServer { state, app, dir } = srv;
    drop(app);
    let data = state.ctx.paths.data_dir.clone();
    drop(state);
    std::fs::remove_dir_all(&data).unwrap();
    let paths = prepare_dirs(dir.path());
    let state = klein_buch_lib::server::prepare(ctx_for(paths), &local_config()).unwrap();
    let app = klein_buch_lib::server::router(state.clone());
    TestServer { state, app, dir }
}

async fn scheduler_paused(srv: &TestServer) -> bool {
    srv.state
        .ctx
        .scheduler_paused
        .load(std::sync::atomic::Ordering::SeqCst)
}

async fn assert_rejected(srv: &TestServer, cookie: &str, kbk: &[u8], needle: &str) {
    let r = restore_upload(srv, cookie, kbk, PASS).await;
    let text = r.text();
    assert_eq!(r.json()["state"], "failed", "{needle}: {text}");
    assert!(text.contains(needle), "erwartet „{needle}“: {text}");
    // W5-ERR-PATHS (Lauf F4c): keine Serverpfade in der Meldung an den Client.
    let data_dir = srv.state.ctx.paths.data_dir.to_string_lossy().into_owned();
    assert!(
        !text.contains(&data_dir),
        "{needle}: Serverpfad in der Meldung: {text}"
    );
    assert!(staging_is_clean(srv), "{needle}: Staging nicht leer");
    assert!(!marker_exists(srv), "{needle}: Marker");
    assert!(!srv.state.writes_blocked(), "{needle}: Sperre aufgehoben");
    assert!(
        !scheduler_paused(srv).await,
        "{needle}: Scheduler fortgesetzt"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disaster_recovery_with_other_passphrase() {
    // Instanz A mit eigener Passphrase, festgeschriebener Rechnung (Archiv)
    // und eigener (fehlender) Vorlage
    let a = test_server();
    let cookie_a = setup_with(&a, OTHER).await;
    let pdf_id = locked_invoice(&a, &cookie_a).await;
    {
        let pool = a.state.ctx.pool().unwrap();
        sqlx::query("UPDATE seller_profile SET default_pdf_template = 'eigene-vorlage'")
            .execute(&pool)
            .await
            .unwrap();
    }
    let kbk = backup_bytes(&a, &cookie_a).await;

    // A geht verloren; neuer Container mit demselben /data-Mount, aber leer
    let b = destroy_and_recreate(a).await;
    // Frische Instanz B (andere Passphrase), Restore des Backups von A
    let cookie_b = setup(&b).await;
    let cookie_b = reauth_cookie(&b, &cookie_b).await;
    let id = upload_id(&b, &cookie_b, "restore", "von-a.kbk", &kbk).await;
    let r = send(
        &b.app,
        post(
            "/api/cmd/backup_restore_apply",
            &serde_json::json!({ "uploadId": id, "passphrase": OTHER }).to_string(),
            Some(&cookie_b),
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.text());
    // Phase A: Änderungen werden abgelehnt
    let w = send(
        &b.app,
        post(
            "/api/cmd/contacts_create",
            "{}",
            Some(&cookie_b),
            Some("w1"),
        ),
    )
    .await;
    assert_eq!(w.status, StatusCode::SERVICE_UNAVAILABLE, "{}", w.text());
    // … und kein Scheduler-Tick (Plan W7)
    let mut paused = false;
    for _ in 0..100 {
        if scheduler_paused(&b).await {
            paused = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(paused, "Scheduler während Phase A angehalten");
    let job = r.json()["jobId"].as_str().unwrap().to_string();
    let done = wait_job(&b.app, &cookie_b, &job).await;
    assert_eq!(done.json()["state"], "done", "{}", done.text());
    let report = &done.json()["result"];
    assert_eq!(report["source"], "Upload von-a.kbk");
    assert!(!report
        .to_string()
        .contains(&*b.state.ctx.paths.data_dir.to_string_lossy()));
    let pre = report["preRestoreBackup"].as_str().unwrap().to_string();
    assert!(
        b.state.ctx.paths.backups_dir.join(&pre).is_file(),
        "Pre-Restore-Backup"
    );
    assert!(marker_exists(&b));
    assert!(b.state.ctx.uploads.is_empty(), "Upload verbraucht");

    // Neustart → Restore angewendet → Anmeldung mit A-Passphrase + B-TOTP
    let b = restart(b).await;
    assert!(!marker_exists(&b) && staging_is_clean(&b));
    assert_eq!(login(&b, PASS).await.status, StatusCode::UNAUTHORIZED);
    let r = login(&b, OTHER).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        scalar_i64(&b, "SELECT COUNT(*) FROM invoices").await,
        1,
        "Daten von A"
    );
    // Archiv von A lesbar (hash-geprüft), Pre-Restore-Backup in der Historie
    let cookie = session_cookie(&r);
    let got = send(
        &b.app,
        get(&format!("/files/archive/{pdf_id}"), Some(&cookie)),
    )
    .await;
    assert_eq!(got.status, StatusCode::OK, "{}", got.text());
    assert!(got.body.starts_with(b"%PDF"));
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'pre_restore'"
        )
        .await,
        1,
        "Pre-Restore-Backup nach dem Tausch in der Historie"
    );
    assert!(
        !b.state
            .ctx
            .paths
            .data_dir
            .join(klein_buch_lib::backup::restore::RESTORE_APPLIED)
            .exists(),
        "Audit geschrieben, Datei entfernt"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.applied'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = 'schema_version'"
        )
        .await,
        EXPECTED_SCHEMA_VERSION as i64
    );
    if MIN_RESTORE_SCHEMA_VERSION < EXPECTED_SCHEMA_VERSION {
        // ab W8: ältere Backups werden beim Entsperren mit Pflicht-Backup migriert
        assert!(
            scalar_i64(
                &b,
                "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.pre_migration'"
            )
            .await
                <= 1
        );
    }
    let warned = scalar_i64(
        &b,
        "SELECT COUNT(*) FROM notifications WHERE title = 'PDF-Vorlagen nach dem Restore fehlen'
         AND body LIKE '%eigene-vorlage%'",
    )
    .await;
    assert_eq!(warned, 1, "Inbox-Warnung zur fehlenden Vorlage");
    // Lauf F: Pause nach dem Restore (W14-023) und Hinweis zu den
    // Nummernkreisen ohne Stände einer ersetzten DB (D-24, Disaster Recovery).
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM app_settings WHERE key = 'scheduler_hold' AND value = 'restore'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_numbering:%'
             AND body LIKE '%Nummernkreise unter Einstellungen%'"
        )
        .await,
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_backups_are_rejected_and_leave_nothing() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let good = backup_bytes(&srv, &cookie).await;

    assert_rejected(
        &srv,
        &cookie,
        &with_header_schema(&good, EXPECTED_SCHEMA_VERSION + 1),
        "höchstens",
    )
    .await;
    assert_rejected(
        &srv,
        &cookie,
        &with_header_schema(&good, MIN_RESTORE_SCHEMA_VERSION - 1),
        "ab Version",
    )
    .await;
    // falsche Passphrase: kein Pre-Restore-Backup, Upload bleibt für den neuen Versuch
    let r = restore_upload(&srv, &cookie, &good, OTHER).await;
    assert_eq!(r.json()["state"], "failed", "{}", r.text());
    assert!(r.text().contains("falsche Passphrase"), "{}", r.text());
    assert_eq!(srv.state.ctx.uploads.len(), 1, "Upload bleibt");
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'pre_restore'"
        )
        .await,
        0,
        "kein Pre-Restore-Backup vor der Passphrase-Prüfung"
    );
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("../evil.txt", b"x")]),
        "unsicherer Pfad",
    )
    .await;
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("/etc/evil", b"x")]),
        "unsicherer Pfad",
    )
    .await;
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("evil.sh", b"x")]),
        "unerwarteter Eintrag",
    )
    .await;
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("archive/x/../../evil", b"x")]),
        "unsicherer Pfad",
    )
    .await;
    let zeros = vec![0u8; 64 * 1024 * 1024];
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("archive/bombe.bin", &zeros)]),
        "Verhältnis",
    )
    .await;
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("archive/2026/a.pdf", b"%PDF")]),
        "keine klein-buch.sqlite",
    )
    .await;
    assert_rejected(
        &srv,
        &cookie,
        &sealed(&[("klein-buch.sqlite", b"SQLite format 3\0klartext")]),
        "unverschlüsselt",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_from_history_only_local() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let _ = backup_bytes(&srv, &cookie).await;
    let pool = srv.state.ctx.pool().unwrap();
    // Eintrag, der aus dem Backup-Ordner hinauszeigt
    let outside = srv.dir.path().join("fremd.kbk");
    std::fs::write(&outside, b"x").unwrap();
    sqlx::query(
        "INSERT INTO backup_history (id, target_path, file_hash_sha256, file_size_bytes,
         retention_tag, trigger_reason, db_schema_version, app_version)
         VALUES ('fremd', ?, 'x', 1, 'manual', 'manual', 30, 'x')",
    )
    .bind(outside.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();
    // W14-039 (Lauf F3a): ungültige Aufrufe scheitern schon vor der Annahme
    // mit 400 — ohne Sperre der Änderungen und ohne Pre-Restore-Backup.
    let r = call(
        &srv.app,
        &cookie,
        "backup_restore_apply",
        serde_json::json!({ "backupId": "fremd", "passphrase": PASS }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.text().contains("Nur lokale Backups"), "{}", r.text());
    for args in [
        serde_json::json!({}),
        serde_json::json!({ "passphrase": PASS }),
        serde_json::json!({ "backupId": "fremd", "uploadId": "x", "passphrase": PASS }),
        serde_json::json!({ "backupId": "gibt-es-nicht", "passphrase": PASS }),
        serde_json::json!({ "uploadId": uuid::Uuid::new_v4().to_string(), "passphrase": PASS }),
        serde_json::json!({ "backupId": "fremd", "passphrase": "" }),
    ] {
        let r = call(&srv.app, &cookie, "backup_restore_apply", args.clone()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{args}: {}", r.text());
        let w = call(
            &srv.app,
            &cookie,
            "package_category_create",
            serde_json::json!({ "name": format!("nach {args}") }),
        )
        .await;
        assert_eq!(
            w.status,
            StatusCode::OK,
            "keine Sperre nach {args}: {}",
            w.text()
        );
    }
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'pre_restore'"
        )
        .await,
        0
    );
    // Wiederholung mit demselben Idempotenz-Schlüssel: dieselbe Antwort.
    let req = || {
        post(
            "/api/cmd/backup_restore_apply",
            &serde_json::json!({ "passphrase": PASS }).to_string(),
            Some(&cookie),
            Some("restore-ungueltig"),
        )
    };
    let first = send(&srv.app, req()).await;
    let again = send(&srv.app, req()).await;
    assert_eq!(first.status, StatusCode::BAD_REQUEST);
    assert_eq!(again.status, StatusCode::BAD_REQUEST, "{}", again.text());

    let id: String = sqlx::query_scalar(
        "SELECT id FROM backup_history WHERE trigger_reason = 'manual' AND id != 'fremd' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let r = {
        let _serial = kdf_serial().await;
        call(
            &srv.app,
            &cookie,
            "backup_restore_apply",
            serde_json::json!({ "backupId": id, "passphrase": PASS }),
        )
        .await
    };
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    assert!(r.json()["result"]["source"]
        .as_str()
        .unwrap()
        .starts_with("Backup vom "));
    assert!(marker_exists(&srv));
}

fn expense_args(description: &str) -> serde_json::Value {
    let year = klein_buch_lib::clock::today_berlin().to_string();
    serde_json::json!({ "args": {
        "input": {
            "expenseDate": year, "paidDate": null, "paidFromAccountId": null,
            "vendorContactId": null, "vendorName": "Bürobedarf Beispiel",
            "vendorInvoiceNumber": null, "category": "office",
            "description": description, "netAmountCents": 1000,
            "taxAmountCents": 0, "grossAmountCents": 1000, "currencyCode": "EUR",
            "reverseCharge13b": false, "notes": null
        },
        "fiscalYear": null,
        "receiptUploadId": null
    }})
}

/// W5-AUTOBACKUP-SYNC (Lauf F3a): Ein festschreibender Command antwortet,
/// ohne auf das Backup zu warten — auch wenn der Scheduler gerade blockiert
/// ist. Der Vermerk steht sofort in der DB; danach legt der Scheduler genau
/// ein Backup für beide Ereignisse an (Akteur `scheduler`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lock_event_backup_runs_after_the_answer() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let _ = draft_invoice(&srv).await; // Verkäuferprofil
    let auto = "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_critical'";
    let pending = "SELECT COUNT(*) FROM app_settings WHERE key = 'backup_pending_since'";
    {
        // Scheduler blockieren: ein synchrones Backup im Command hinge hier.
        let _gate = srv.state.ctx.scheduler_gate.lock().await;
        for d in ["Papier", "Toner"] {
            let r = tokio::time::timeout(
                std::time::Duration::from_secs(20),
                call(&srv.app, &cookie, "expenses_create", expense_args(d)),
            )
            .await
            .expect("Antwort ohne Warten auf das Backup");
            assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        }
        assert_eq!(scalar_i64(&srv, auto).await, 0);
        assert_eq!(scalar_i64(&srv, pending).await, 1, "Vermerk sofort");
        assert_eq!(
            scalar_i64(
                &srv,
                "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.requested'
                   AND json_extract(details_json, '$.event') = 'expense.lock'"
            )
            .await,
            2
        );
    }
    let mut done = false;
    for _ in 0..1200 {
        if scalar_i64(&srv, pending).await == 0 {
            done = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(done, "Scheduler legt das Backup an");
    assert_eq!(
        scalar_i64(&srv, auto).await,
        1,
        "beide Ereignisse, ein Backup"
    );
    assert_eq!(
        scalar_text(
            &srv,
            "SELECT actor FROM audit_log WHERE action = 'backup.created'
              AND json_extract(details_json, '$.trigger') = 'auto_critical'"
        )
        .await
        .as_deref(),
        Some("scheduler")
    );
}

/// Lauf F3c (S2, ADR 0040 Nr. 57): Ein Lock-Backup, das beim Sperren bzw.
/// Herunterfahren noch aussteht, entsteht vor dem Schließen des Pools. Der
/// geweckte Lauf stellt sich hinter einem (simulierten) Tick an und verwirft
/// sein Backup wegen der Abbruch-Anforderung; ohne Nachholen fehlte es bis zum
/// nächsten Entsperren.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lock_and_shutdown_write_pending_lock_backup() {
    use std::time::Duration;
    for shutdown in [false, true] {
        let srv = test_server();
        let cookie = setup(&srv).await;
        let _ = draft_invoice(&srv).await; // Verkäuferprofil
        let tick = srv.state.ctx.scheduler_gate.lock().await;
        let r = call(&srv.app, &cookie, "expenses_create", expense_args("Papier")).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        // Der geweckte Lauf wartet BACKUP_COALESCE und stellt sich dann an.
        tokio::time::sleep(
            klein_buch_lib::scheduler::tick::BACKUP_COALESCE + Duration::from_millis(700),
        )
        .await;
        let _serial = kdf_serial().await; // Argon2id des nachgeholten Backups
        let state = srv.state.clone();
        let stop = tokio::spawn(async move {
            if shutdown {
                state.finish_shutdown().await;
                Ok(())
            } else {
                klein_buch_lib::server::auth::lock_database(&state).await
            }
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        drop(tick);
        assert!(stop.await.unwrap().is_ok(), "shutdown={shutdown}");
        assert!(!srv.state.ctx.is_unlocked(), "Pool geschlossen");
        let pool = klein_buch_lib::db::open_pool(&srv.state.ctx.paths.db_file, Some(PASS))
            .await
            .unwrap();
        let backups: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_critical'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_settings WHERE key = 'backup_pending_since'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        pool.close().await;
        assert_eq!(backups, 1, "Lock-Backup geschrieben (shutdown={shutdown})");
        assert_eq!(pending, 0, "nichts steht mehr aus (shutdown={shutdown})");
    }
}

/// W13-BACKUP-DOWNLOAD (Lauf F3a): Download der verschlüsselten `.kbk` nur
/// mit Re-Auth, einmalig, unverändert; nur aus dem Backup-Ordner bzw. unter
/// `/backup-targets` (W14-011), nicht von SFTP. Das Backup selbst bleibt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backup_download_needs_reauth_and_serves_only_local_kbk() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let pool = srv.state.ctx.pool().unwrap();
    let (id, path): (String, String) = sqlx::query_as(
        "SELECT id, target_path FROM backup_history WHERE trigger_reason = 'manual' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let r = call(
        &srv.app,
        &cookie,
        "backup_download",
        serde_json::json!({ "backupId": id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    assert_eq!(r.json()["code"], "reauth_required");

    let cookie = reauth_cookie(&srv, &cookie).await;
    let r = call(
        &srv.app,
        &cookie,
        "backup_download",
        serde_json::json!({ "backupId": id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let ticket = r.json();
    let original = std::fs::read(&path).unwrap();
    assert_eq!(ticket["sizeBytes"].as_u64(), Some(original.len() as u64));
    assert!(ticket["fileName"].as_str().unwrap().ends_with(".kbk"));
    let url = format!("/files/download/{}", ticket["downloadId"].as_str().unwrap());
    let got = send(&srv.app, get(&url, Some(&cookie))).await;
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(
        got.body.as_ref(),
        original.as_slice(),
        "unverändert (verschlüsselt)"
    );
    assert!(got.headers[axum::http::header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    let again = send(&srv.app, get(&url, Some(&cookie))).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND, "einmalig");
    assert_eq!(std::fs::read(&path).unwrap(), original, "Backup bleibt");
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.downloaded'"
        )
        .await,
        1
    );

    // Manipulierte Einträge: außerhalb der Wurzeln, keine .kbk, SFTP.
    let outside = srv.dir.path().join("fremd.kbk");
    std::fs::write(&outside, b"x").unwrap();
    let not_kbk = srv.state.ctx.paths.backups_dir.join("notiz.txt");
    std::fs::write(&not_kbk, b"x").unwrap();
    for (row, target) in [
        ("fremd", outside.to_string_lossy().to_string()),
        ("txt", not_kbk.to_string_lossy().to_string()),
        (
            "sftp",
            "sftp://kb@backup.example.org:22/kb/klein-buch-x.kbk".to_string(),
        ),
    ] {
        sqlx::query(
            "INSERT INTO backup_history (id, target_path, file_hash_sha256, file_size_bytes,
             retention_tag, trigger_reason, db_schema_version, app_version)
             VALUES (?, ?, 'x', 1, 'manual', 'manual', 32, 'x')",
        )
        .bind(row)
        .bind(&target)
        .execute(&pool)
        .await
        .unwrap();
        let r = call(
            &srv.app,
            &cookie,
            "backup_download",
            serde_json::json!({ "backupId": row }),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{row}: {}",
            r.text()
        );
    }
    assert_eq!(srv.state.ctx.downloads.len(), 0);
}

/// Lauf F3c (S3): Ein Backup in einem Verzeichnis-Ziel auf einem anderen
/// Dateisystem (hier `/dev/shm`) wird ohne Kopie nach `<data>/tmp` ausgeliefert
/// (vorher: volle Kopie ohne Platzprüfung, bis zu einer Stunde liegend) und
/// bleibt nach dem Abruf liegen.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backup_download_from_other_filesystem_needs_no_copy() {
    use std::os::unix::fs::MetadataExt;
    let Ok(shm) = tempfile::Builder::new()
        .prefix("kb-ziele-")
        .tempdir_in("/dev/shm")
    else {
        eprintln!("/dev/shm fehlt — Test übersprungen");
        return;
    };
    let root = shm.path().to_path_buf();
    let srv = test_server_with_paths(|p| p.backup_targets_dir = root.clone(), |_| {});
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let pool = srv.state.ctx.pool().unwrap();
    let floor: String = sqlx::query_scalar(
        "SELECT target_path FROM backup_history WHERE trigger_reason = 'manual' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let dir = root.join("nas").join("kb");
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join(std::path::Path::new(&floor).file_name().unwrap());
    std::fs::copy(&floor, &copy).unwrap();
    let original = std::fs::read(&copy).unwrap();
    sqlx::query(
        "INSERT INTO backup_history (id, target_path, file_hash_sha256, file_size_bytes,
         retention_tag, trigger_reason, db_schema_version, app_version)
         VALUES ('ziel', ?, 'x', ?, 'manual', 'manual', 32, 'x')",
    )
    .bind(copy.to_string_lossy().to_string())
    .bind(original.len() as i64)
    .execute(&pool)
    .await
    .unwrap();
    let other_fs = std::fs::metadata(&copy).unwrap().dev()
        != std::fs::metadata(&srv.state.ctx.paths.data_dir)
            .unwrap()
            .dev();

    let r = call(
        &srv.app,
        &cookie,
        "backup_download",
        serde_json::json!({ "backupId": "ziel" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let ticket = r.json();
    assert_eq!(ticket["sizeBytes"].as_u64(), Some(original.len() as u64));
    let downloads = srv.state.ctx.paths.data_dir.join("tmp").join("downloads");
    let copies = || {
        std::fs::read_dir(&downloads)
            .map(|d| d.count())
            .unwrap_or(0)
    };
    if other_fs {
        assert_eq!(copies(), 0, "keine Kopie unter <data>/tmp");
    }
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT json_extract(details_json, '$.linked') FROM audit_log
              WHERE action = 'backup.downloaded'"
        )
        .await,
        i64::from(!other_fs)
    );
    let url = format!("/files/download/{}", ticket["downloadId"].as_str().unwrap());
    let got = send(&srv.app, get(&url, Some(&cookie))).await;
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(got.body.as_ref(), original.as_slice());
    let again = send(&srv.app, get(&url, Some(&cookie))).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND, "einmalig");
    assert_eq!(std::fs::read(&copy).unwrap(), original, "Backup bleibt");
    assert_eq!(copies(), 0);
}

/// Migrator mit allen Migrationen außer der letzten (DB eine Version älter).
fn migrator_without_last() -> sqlx::migrate::Migrator {
    let all = &klein_buch_lib::db::MIGRATOR.migrations;
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(all[..all.len() - 1].to_vec()),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_migration_backup_on_unlock() {
    use std::os::unix::fs::PermissionsExt;
    let srv = test_server();
    let _cookie = setup(&srv).await;
    srv.state.ctx.take_pool().unwrap().close().await;
    srv.state.ctx.session.clear();
    // DB durch eine eine Version ältere ersetzen (verschlüsselt, gleiche Passphrase)
    let db = srv.state.ctx.paths.db_file.clone();
    for suffix in ["", "-wal", "-shm"] {
        let mut p = db.as_os_str().to_os_string();
        p.push(suffix);
        let _ = std::fs::remove_file(p);
    }
    let old = EXPECTED_SCHEMA_VERSION - 1;
    {
        let pool = klein_buch_lib::db::open_pool(&db, Some(PASS))
            .await
            .unwrap();
        migrator_without_last().run(&pool).await.unwrap();
        let v: String =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(v, old.to_string());
        pool.close().await;
    }

    // Backup scheitert (Backup-Ordner schreibgeschützt) → keine Migration
    let backups = srv.state.ctx.paths.backups_dir.clone();
    std::fs::create_dir_all(&backups).unwrap();
    std::fs::set_permissions(&backups, std::fs::Permissions::from_mode(0o500)).unwrap();
    let r = login(&srv, PASS).await;
    std::fs::set_permissions(&backups, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(r.json()["code"], "pre_migration_backup_failed");
    assert!(!srv.state.ctx.is_unlocked(), "bleibt gesperrt");
    {
        let pool = klein_buch_lib::db::open_pool(&db, Some(PASS))
            .await
            .unwrap();
        let v: String =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(v, old.to_string(), "nicht migriert");
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE title = 'Datenbank-Migration abgebrochen'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "Inbox-Hinweis");
        pool.close().await;
    }

    // Mit Platz: Pflicht-Backup mit alter Version, dann Migration + Audit
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let row: (String, i64) = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_as(
            "SELECT trigger_reason, db_schema_version FROM backup_history
             WHERE trigger_reason = 'manual' ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(row, ("manual".to_string(), old as i64));
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = 'schema_version'"
        )
        .await,
        EXPECTED_SCHEMA_VERSION as i64
    );
    let details: String = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_scalar(
            "SELECT details_json FROM audit_log WHERE action = 'backup.pre_migration'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert!(
        details.contains(&format!("\"from\":{old}"))
            && details.contains(&format!("\"to\":{EXPECTED_SCHEMA_VERSION}")),
        "{details}"
    );
    // Das Pflicht-Backup selbst trägt die alte Version im Manifest
    let name: String = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_scalar(
            "SELECT target_path FROM backup_history WHERE trigger_reason = 'manual'
             ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let bytes = std::fs::read(&name).unwrap();
    let m = klein_buch_lib::backup::manifest::read_manifest_only(&bytes).unwrap();
    assert_eq!(m.schema_version, old);
    // Lauf F3c (S1): im Format der Vorversion — v2026.10.0 (Schema 31) liest
    // Format 1–2 ohne AAD. Mit genau deren Leselogik entschlüsseln und die
    // Datenbank im Inhalt mit der Passphrase öffnen: Schema `old`.
    assert_eq!(
        m.format_version,
        klein_buch_lib::backup::manifest::readable_format_for_schema(old)
    );
    if old <= 31 {
        assert_eq!(m.format_version, 2, "Rückfall auf v2026.10.0");
    }
    let content = decrypt_like_v2026_10(&bytes, PASS);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(content)).unwrap();
    let mut db_bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut zip.by_name("klein-buch.sqlite").unwrap(),
        &mut db_bytes,
    )
    .unwrap();
    let probe = srv
        .state
        .ctx
        .paths
        .data_dir
        .join("tmp")
        .join("probe-v2026-10.sqlite");
    std::fs::create_dir_all(probe.parent().unwrap()).unwrap();
    std::fs::write(&probe, &db_bytes).unwrap();
    let pool = klein_buch_lib::db::open_pool(&probe, Some(PASS))
        .await
        .unwrap();
    let v: String =
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    pool.close().await;
    assert_eq!(v, old.to_string());
    // Die laufende Version liest es weiterhin (Format 1–3).
    klein_buch_lib::backup::verify_backup_file(std::path::Path::new(&name), PASS)
        .await
        .unwrap();
}

/// Leselogik von v2026.10.0 (`backup::restore::decrypt_backup` dort): Format
/// 1–2, AES-256-GCM ohne AAD über alles nach dem Kopf, danach Content-Hash
/// (Lauf F3c, Rückfall).
fn decrypt_like_v2026_10(bytes: &[u8], pass: &str) -> Vec<u8> {
    use klein_buch_lib::backup::{encrypt, manifest, snapshot};
    let (m, ct) = manifest::unframe(bytes).unwrap();
    assert!(
        (1..=2).contains(&m.format_version),
        "v2026.10.0 liest Format 1–2, nicht {}",
        m.format_version
    );
    let salt = manifest::from_hex(&m.salt_hex).unwrap();
    let nonce = encrypt::nonce_array(&manifest::from_hex(&m.nonce_hex).unwrap()).unwrap();
    let key = encrypt::derive_key(pass.as_bytes(), &salt).unwrap();
    let content = encrypt::decrypt(&key, &nonce, &ct).unwrap();
    assert_eq!(snapshot::sha256_hex(&content), m.content_sha256);
    content
}

/// Lauf F2c (S1): Ein Altfehler (verwaister Verweis) in einer älteren
/// Datenbank lehnt das Entsperren **vor** dem Pflicht-Backup und vor jeder
/// Migration ab — die Datenbank bleibt auf dem alten Schema, das vorige Image
/// öffnet sie weiter, und die Meldung „nichts verändert“ stimmt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_key_violation_before_migration_refuses_unlock_without_changes() {
    let srv = test_server();
    let _cookie = setup(&srv).await;
    srv.state.ctx.take_pool().unwrap().close().await;
    srv.state.ctx.session.clear();
    let db = srv.state.ctx.paths.db_file.clone();
    for suffix in ["", "-wal", "-shm"] {
        let mut p = db.as_os_str().to_os_string();
        p.push(suffix);
        let _ = std::fs::remove_file(p);
    }
    let old = EXPECTED_SCHEMA_VERSION - 1;
    {
        let _serial = kdf_serial().await;
        let pool = klein_buch_lib::db::open_pool(&db, Some(PASS))
            .await
            .unwrap();
        migrator_without_last().run(&pool).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (id, rule_id, title, body, severity)
             VALUES ('n-alt', 'rule_gibt_es_nicht', 'T', 'B', 'info')",
        )
        .execute(&mut *conn)
        .await
        .unwrap();
        drop(conn);
        pool.close().await;
    }
    let kbk_count = || {
        std::fs::read_dir(&srv.state.ctx.paths.backups_dir)
            .map(|it| {
                it.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|x| x == "kbk"))
                    .count()
            })
            .unwrap_or(0)
    };
    let backups_before = kbk_count();
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(r.json()["code"], "db_integrity_failed", "{}", r.text());
    assert!(
        r.text().contains("nicht verändert"),
        "Datenbank ist unverändert: {}",
        r.text()
    );
    assert!(!srv.state.ctx.is_unlocked(), "bleibt gesperrt");
    assert_eq!(
        kbk_count(),
        backups_before,
        "kein Pflicht-Backup vor der Ablehnung"
    );
    {
        let _serial = kdf_serial().await;
        let pool = klein_buch_lib::db::open_pool(&db, Some(PASS))
            .await
            .unwrap();
        let v: String =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(v, old.to_string(), "nicht migriert");
        pool.close().await;
    }
}

/// Macht Migration 0032 auf einer Datenbank rückgängig (Testhilfe für ein
/// Backup mit Schema 31): alle in 0032 angelegten Trigger und Indizes — die
/// Namen kommen aus der Migrationsdatei, damit ein neuer nicht vergessen wird
/// (Lauf F2c) —, die Spalten und der Seed.
async fn roll_back_0032(pool: &sqlx::SqlitePool) {
    const SQL: &str = include_str!("../migrations/0032_gobd_hardening.sql");
    let mut stmts: Vec<String> = Vec::new();
    for line in SQL.lines() {
        for (prefix, kind) in [
            ("CREATE TRIGGER ", "TRIGGER"),
            ("CREATE UNIQUE INDEX ", "INDEX"),
            ("CREATE INDEX ", "INDEX"),
        ] {
            if let Some(rest) = line.trim_start().strip_prefix(prefix) {
                let name = rest.split_whitespace().next().unwrap();
                stmts.push(format!("DROP {kind} {name}"));
            }
        }
    }
    assert!(
        stmts
            .iter()
            .any(|s| s == "DROP TRIGGER trg_invoices_voided_final")
            && stmts
                .iter()
                .any(|s| s == "DROP INDEX uq_payment_accounts_default"),
        "{stmts:?}"
    );
    stmts.extend(
        [
            "ALTER TABLE invoices DROP COLUMN voided_at",
            "ALTER TABLE invoices DROP COLUMN void_reason",
            "ALTER TABLE archive_integrity_checks DROP COLUMN files_missing",
            "ALTER TABLE archive_integrity_checks DROP COLUMN files_unreadable",
            "ALTER TABLE archive_integrity_checks DROP COLUMN missing_archive_ids_json",
            "ALTER TABLE archive_integrity_checks DROP COLUMN unreadable_archive_ids_json",
            "DELETE FROM notification_rules WHERE id = 'rule_recurring_due'",
        ]
        .map(str::to_string),
    );
    for sql in &stmts {
        sqlx::query(sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
    }
}

/// Setzt die Datenbank einer laufenden Test-Instanz auf Schema `target`
/// zurück (31: ohne 0032; 30: zusätzlich ohne 0031 = Tabelle `secrets`).
async fn roll_back_to(pool: &sqlx::SqlitePool, target: i32) {
    let migrations = &klein_buch_lib::db::MIGRATOR.migrations;
    let last = migrations.last().unwrap();
    assert!(
        last.description.contains("gobd hardening"),
        "Test setzt 0032 zurück: {}",
        last.description
    );
    roll_back_0032(pool).await;
    if target <= 30 {
        let prev = &migrations[migrations.len() - 2];
        assert!(prev.description.contains("secrets"), "{}", prev.description);
        sqlx::query("DROP TABLE secrets")
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version > ?")
        .bind(match target {
            31 => migrations[migrations.len() - 2].version,
            _ => migrations[migrations.len() - 3].version,
        })
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'schema_version'")
        .bind(target.to_string())
        .execute(pool)
        .await
        .unwrap();
}

/// Lauf F2c: `.kbk` mit Schema 30 (Mindeststand, v2026.5 — der Umstiegsweg
/// der Bestandsnutzer) → Restore → beim Entsperren Vorab-Prüfung,
/// Pflicht-Backup (Schema 30), zwei Migrationen in Folge (0031, 0032) und
/// Fremdschlüssel-Prüfung. Bestandsdaten gehen durch: zwei Standardkonten
/// (0032 bereinigt mit Audit) und eine Rechnung mit festgeschriebenem Storno.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schema_30_backup_is_restored_and_migrated_forward() {
    assert_eq!(MIN_RESTORE_SCHEMA_VERSION, 30);
    let a = test_server();
    let cookie_a = setup(&a).await;
    let defaults_before: i64;
    {
        let pool = a.state.ctx.pool().unwrap();
        roll_back_to(&pool, 30).await;
        // Bestandsdaten wie aus der Desktop-Fassung
        for (id, label) in [
            ("01928300-0000-7000-8000-0000000000a1", "Bank Altbestand 1"),
            ("01928300-0000-7000-8000-0000000000a2", "Bank Altbestand 2"),
        ] {
            sqlx::query(
                "INSERT INTO payment_accounts (id, label, \"type\", is_default, active)
                 VALUES (?, ?, 'bank', 1, 1)",
            )
            .bind(id)
            .bind(label)
            .execute(&pool)
            .await
            .unwrap();
        }
        defaults_before = sqlx::query_scalar(
            "SELECT COUNT(*) FROM payment_accounts WHERE is_default = 1 AND active = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(defaults_before >= 2);
        sqlx::query(
            "INSERT INTO contacts (id, contact_type, name, email, country_code)
             VALUES ('c-alt', 'customer', 'Kunde Altbestand', 'kunde@example.org', 'DE')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for (id, number, storno_for) in [
            ("inv-alt", "RE-2026-0901", None),
            ("st-alt", "ST-2026-0901", Some("inv-alt")),
        ] {
            sqlx::query(
                "INSERT INTO invoices (
                    id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
                    seller_name, seller_street, seller_postal_code, seller_city,
                    net_amount_cents, gross_amount_cents, status, locked_at, is_storno_for,
                    buyer_name, buyer_country_code
                 ) VALUES (?, ?, 2026, 'issued', '2026-05-20', 'c-alt',
                    'Verkäufer', 'Beispielweg 1', '84028', 'Landshut',
                    10000, 10000, 'issued', '2026-05-20 10:00:00', ?, 'Kunde Altbestand', 'DE')",
            )
            .bind(id)
            .bind(number)
            .bind(storno_for)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "UPDATE invoices SET status = 'canceled', canceled_at = '2026-05-20 10:00:00',
                    canceled_by_storno_id = 'st-alt' WHERE id = 'inv-alt'",
        )
        .execute(&pool)
        .await
        .unwrap();
    }
    // Lauf F3a: als Format 2 (ohne AAD) — so sehen alle bis dahin
    // geschriebenen Backups aus.
    let kbk = as_format_2(&backup_bytes(&a, &cookie_a).await, PASS);
    let m = klein_buch_lib::backup::manifest::read_manifest_only(&kbk).unwrap();
    assert_eq!(m.schema_version, 30, "Backup trägt Schema 30");
    assert_eq!(m.format_version, 2);

    // Instanz B stellt es wieder her
    let b = test_server();
    let cookie_b = setup(&b).await;
    let cookie_b = reauth_cookie(&b, &cookie_b).await;
    let r = restore_upload(&b, &cookie_b, &kbk, PASS).await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    assert_eq!(r.json()["result"]["schemaVersion"], 30, "{}", r.text());
    let b = restart(b).await;
    let r = login(&b, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    assert_eq!(
        scalar_i64(
            &b,
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = 'schema_version'"
        )
        .await,
        EXPECTED_SCHEMA_VERSION as i64
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'secrets'"
        )
        .await,
        1,
        "0031 angewendet"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM sqlite_master
              WHERE name IN ('ux_invoices_one_locked_storno', 'trg_invoices_voided_final',
                             'trg_quotes_converted_final', 'uq_payment_accounts_default')"
        )
        .await,
        4,
        "0032 angewendet"
    );
    // Bestand: genau ein Standardkonto, je bereinigtes Konto ein Audit
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM payment_accounts WHERE is_default = 1"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'payment_account.default_cleared'
               AND json_extract(details_json, '$.migration') = 32"
        )
        .await,
        defaults_before - 1
    );
    // Storno-Paar unverändert und durch den Index geschützt
    assert_eq!(
        scalar_text(
            &b,
            "SELECT status || '|' || canceled_by_storno_id FROM invoices WHERE id = 'inv-alt'"
        )
        .await
        .as_deref(),
        Some("canceled|st-alt")
    );
    // Pflicht-Backup mit Schema 30 und Audit 30 → 32
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT db_schema_version FROM backup_history WHERE trigger_reason = 'manual'
             ORDER BY created_at DESC, rowid DESC LIMIT 1"
        )
        .await,
        30
    );
    let details: String = {
        let pool = b.state.ctx.pool().unwrap();
        sqlx::query_scalar(
            "SELECT details_json FROM audit_log WHERE action = 'backup.pre_migration'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert!(
        details.contains("\"from\":30")
            && details.contains(&format!("\"to\":{EXPECTED_SCHEMA_VERSION}")),
        "{details}"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.applied'"
        )
        .await,
        1
    );
}

/// Plan W7/W8 + Lauf F2a: `.kbk` mit Schema 31 → Restore → beim Entsperren
/// Pflicht-Backup (Schema 31) → Vorwärtsmigration (0032).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schema_31_backup_is_restored_and_migrated_forward() {
    let old = EXPECTED_SCHEMA_VERSION - 1;
    assert!(old >= MIN_RESTORE_SCHEMA_VERSION);
    // Instanz A auf den Stand vor der letzten Migration zurücksetzen
    let a = test_server();
    let cookie_a = setup(&a).await;
    {
        let pool = a.state.ctx.pool().unwrap();
        roll_back_to(&pool, old).await;
    }
    let kbk = backup_bytes(&a, &cookie_a).await;
    let m = klein_buch_lib::backup::manifest::read_manifest_only(&kbk).unwrap();
    assert_eq!(m.schema_version, old, "Backup trägt Schema {old}");

    // Instanz B stellt es wieder her
    let b = test_server();
    let cookie_b = setup(&b).await;
    let cookie_b = reauth_cookie(&b, &cookie_b).await;
    let r = restore_upload(&b, &cookie_b, &kbk, PASS).await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    assert_eq!(r.json()["result"]["schemaVersion"], old, "{}", r.text());
    let b = restart(b).await;
    let r = login(&b, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    assert_eq!(
        scalar_i64(
            &b,
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = 'schema_version'"
        )
        .await,
        EXPECTED_SCHEMA_VERSION as i64
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM pragma_table_info('invoices') WHERE name = 'voided_at'"
        )
        .await,
        1,
        "0032 angewendet (Spalte)"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM sqlite_master
              WHERE name IN ('ux_invoices_one_locked_storno', 'trg_invoice_items_frozen_update',
                             'trg_archive_no_delete', 'uq_payment_accounts_default')"
        )
        .await,
        4,
        "0032 angewendet (Index, Trigger)"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM notification_rules WHERE id = 'rule_recurring_due'"
        )
        .await,
        1,
        "0032 angewendet (Regel)"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT db_schema_version FROM backup_history WHERE trigger_reason = 'manual'
             ORDER BY created_at DESC, rowid DESC LIMIT 1"
        )
        .await,
        old as i64,
        "Pflicht-Backup mit Schema {old}"
    );
    let details: String = {
        let pool = b.state.ctx.pool().unwrap();
        sqlx::query_scalar(
            "SELECT details_json FROM audit_log WHERE action = 'backup.pre_migration'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert!(
        details.contains(&format!("\"from\":{old}"))
            && details.contains(&format!("\"to\":{EXPECTED_SCHEMA_VERSION}")),
        "{details}"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.applied'"
        )
        .await,
        1
    );
}

/// Gegenprüfung W7: Backup einer Instanz mit anderem Datenpfad (Archivpfade
/// passen nicht) wird in Phase A abgelehnt, statt die Instanz nach dem Tausch
/// dauerhaft zu sperren.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_archive_paths_are_rejected() {
    let a = test_server();
    let cookie_a = setup(&a).await;
    let _ = locked_invoice(&a, &cookie_a).await;
    let kbk = backup_bytes(&a, &cookie_a).await;
    let b = test_server();
    let cookie_b = setup(&b).await;
    let cookie_b = reauth_cookie(&b, &cookie_b).await;
    assert_rejected(&b, &cookie_b, &kbk, "Archivpfade").await;
    // Lauf F3c: Die Prüfung der gestagten DB läuft vor dem Pre-Restore-Backup
    // (der entschlüsselte Inhalt ist dann schon freigegeben) — abgelehnt
    // heißt: kein Pre-Restore-Backup, aber der Audit des Versuchs.
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'pre_restore'"
        )
        .await,
        0,
        "kein Pre-Restore-Backup (Prüfung vor ihm)"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.requested'"
        )
        .await,
        1,
        "Versuch auditiert"
    );
}

/// Lauf F2a: Ein Backup, das das Entsperren nach dem Tausch verweigern würde
/// (verletzter Fremdschlüssel, R1-012; zwei festgeschriebene Stornos zu einem
/// Original, Vorab-Prüfung 0032), lehnt schon Phase A ab — die Instanz bleibt
/// nutzbar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backups_the_unlock_would_refuse_are_rejected_in_phase_a() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    {
        let pool = srv.state.ctx.pool().unwrap();
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (id, rule_id, title, body, severity)
             VALUES ('n-fk', 'rule_gibt_es_nicht', 'T', 'B', 'info')",
        )
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    let kbk = backup_bytes(&srv, &cookie).await;
    assert_rejected(&srv, &cookie, &kbk, "verwaiste Verweise").await;
    assert!(srv.state.ctx.is_unlocked(), "Instanz bleibt entsperrt");
}

/// Gegenprüfung W7: Der Audit des angewendeten Restores übersteht einen
/// Neustart vor dem ersten Entsperren (RESTORE_APPLIED.json statt RAM).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn applied_audit_survives_restart_before_unlock() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let kbk = backup_bytes(&srv, &cookie).await;
    let r = restore_upload(&srv, &cookie, &kbk, PASS).await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    let srv = restart(srv).await; // Tausch
    let applied = srv
        .state
        .ctx
        .paths
        .data_dir
        .join(klein_buch_lib::backup::restore::RESTORE_APPLIED);
    assert!(applied.exists());
    let srv = restart(srv).await; // noch ein Neustart, ohne Entsperren
    assert!(applied.exists(), "bleibt bis zum Audit");
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.applied'"
        )
        .await,
        1
    );
    assert!(!applied.exists());
}

/// Manuelles Backup über den Command; liefert das Ergebnis (`BackupOutcome`).
async fn backup_now(srv: &TestServer, cookie: &str) -> serde_json::Value {
    let _serial = kdf_serial().await;
    let r = call(&srv.app, cookie, "backup_create_now", serde_json::json!({})).await;
    let body = r.json();
    if body.get("state").is_some() {
        assert_eq!(body["state"], "done", "{}", r.text());
        body["result"].clone()
    } else {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        body
    }
}

async fn scalar_text(srv: &TestServer, sql: &str) -> Option<String> {
    let pool = srv.state.ctx.pool().unwrap();
    sqlx::query_scalar(sql).fetch_optional(&pool).await.unwrap()
}

/// Lauf F (W14-023, D-22, D-23, D-24): Restore eines **älteren** Backups auf
/// derselben Instanz.
/// - bereits vergebene Rechnungsnummern werden nicht erneut vergeben (Zähler
///   auf den Stand vor dem Restore angehoben, Audit, Hinweis),
/// - Abo-Läufe und Versand pausieren bis zur Freigabe (Re-Auth),
/// - Backups gehen bis dahin nur in den lokalen Ordner (keine Spiegelung),
/// - die nach dem Backup entstandenen Backups stehen wieder in der Historie.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn older_backup_restore_keeps_numbers_pauses_and_mirrors_only_after_release() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    std::fs::create_dir_all(srv.state.ctx.paths.backup_targets_dir.join("nas")).unwrap();
    let r = call(
        &srv.app,
        &cookie,
        "backup_target_add_directory",
        serde_json::json!({ "label": "NAS", "directory": "nas/kb" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // Stand A: eine Rechnung. Danach zwei weitere (Nummern 2 und 3). Die
    // Lock-Backups entstehen im Hintergrund; der Test wartet sie jeweils ab
    // (Lauf F4d), sonst hängt die Zahl der Backup-Dateien unten am Timing und
    // ein im Stand A vermerktes Backup würde nach dem Restore nachgeholt.
    let _ = locked_invoice(&srv, &cookie).await;
    settle_lock_backups(&srv).await;
    let a = backup_now(&srv, &cookie).await;
    let a_id = a["historyId"].as_str().unwrap().to_string();
    let _ = locked_invoice(&srv, &cookie).await;
    settle_lock_backups(&srv).await;
    let _ = locked_invoice(&srv, &cookie).await;
    settle_lock_backups(&srv).await;
    let year = scalar_i64(&srv, "SELECT MAX(fiscal_year) FROM invoices").await;
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT last_value FROM doc_number_counters WHERE doc_type = 'invoice'"
        )
        .await,
        3
    );

    let r = {
        let _serial = kdf_serial().await;
        call(
            &srv.app,
            &cookie,
            "backup_restore_apply",
            serde_json::json!({ "backupId": a_id, "passphrase": PASS }),
        )
        .await
    };
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    let srv = restart(srv).await;
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let cookie = session_cookie(&r);

    // D-24: Stand A, Zähler aber auf 3 — die nächste Rechnung ist Nr. 4.
    assert_eq!(scalar_i64(&srv, "SELECT COUNT(*) FROM invoices").await, 1);
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT last_value FROM doc_number_counters WHERE doc_type = 'invoice'"
        )
        .await,
        3
    );
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'numbering.raised_after_restore'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_numbering:%'
             AND body LIKE '%von 1 auf 3%' AND body NOT LIKE '%nicht aus dieser Instanz%'"
        )
        .await,
        1
    );
    let draft = draft_invoice(&srv).await;
    let number = scalar_text(
        &srv,
        &format!("SELECT invoice_number FROM invoices WHERE id = '{draft}'"),
    )
    .await
    .unwrap();
    assert_eq!(number, format!("RE-{year}-0004"), "keine Doppelvergabe");

    // W14-023: Pause mit Grund „restore“, Abo-Läufe ausgesetzt.
    assert_eq!(
        scalar_text(
            &srv,
            "SELECT value FROM app_settings WHERE key = 'scheduler_hold'"
        )
        .await
        .as_deref(),
        Some("restore")
    );
    {
        let pool = srv.state.ctx.pool().unwrap();
        let clock = srv.state.ctx.clock();
        let job =
            klein_buch_lib::scheduler::JobCtx::from_app(&srv.state.ctx, &pool, clock.as_ref());
        let report = klein_buch_lib::scheduler::recurring_invoice::process_due(&job)
            .await
            .unwrap();
        assert_eq!(report.skipped_hold.as_deref(), Some("restore"));
        let report = klein_buch_lib::scheduler::recurring::process_due(&job)
            .await
            .unwrap();
        assert_eq!(report.skipped_hold.as_deref(), Some("restore"));
    }

    // D-22: jede Backup-Datei im lokalen Ordner steht in der Historie.
    let floor = srv.state.ctx.paths.backups_dir.clone();
    let files: Vec<String> = std::fs::read_dir(&floor)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("klein-buch-") && n.ends_with(".kbk"))
        .collect();
    assert!(files.len() >= 4, "{files:?}");
    for name in &files {
        let path = floor.join(name).to_string_lossy().to_string();
        assert_eq!(
            scalar_i64(
                &srv,
                &format!("SELECT COUNT(*) FROM backup_history WHERE target_path = '{path}'")
            )
            .await,
            1,
            "{name} fehlt in der Historie"
        );
    }
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.history_rescanned'"
        )
        .await,
        1
    );

    // D-23: bis zur Freigabe nur lokal.
    let mirrored =
        "SELECT COUNT(*) FROM backup_log WHERE target_kind = 'directory' AND status = 'ok'";
    let before = scalar_i64(&srv, mirrored).await;
    let out = backup_now(&srv, &cookie).await;
    assert_eq!(
        out["offsite"][0]["error"].as_str(),
        Some(klein_buch_lib::backup::MIRROR_SUSPENDED),
        "{out}"
    );
    assert_eq!(scalar_i64(&srv, mirrored).await, before, "keine Spiegelung");
    assert!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.mirror.skipped'"
        )
        .await
            >= 1
    );

    // Freigabe (Re-Auth) → Spiegelung und Abo-Läufe wieder aktiv.
    let cookie = reauth_cookie(&srv, &cookie).await;
    let r = call(
        &srv.app,
        &cookie,
        "scheduler_release_hold",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["held"], false, "{}", r.text());
    let out = backup_now(&srv, &cookie).await;
    assert!(out["offsite"][0]["error"].is_null(), "{out}");
    assert_eq!(scalar_i64(&srv, mirrored).await, before + 1);
}

/// Vorlage (monatlich, heute fällig) für die F1d-Tests.
fn tmpl_today(contact: &str, label: &str, mode: &str) -> RecurringInvoiceInput {
    use chrono::Datelike;
    let today = klein_buch_lib::clock::today_berlin();
    RecurringInvoiceInput {
        label: label.into(),
        contact_id: contact.into(),
        frequency: "monthly".into(),
        day_of_period: today.day() as i64,
        next_due_date: today,
        start_date: None,
        end_date: None,
        auto_mode: mode.into(),
        payment_terms_days: 14,
        pdf_template: "default".into(),
        service_period_note: true,
        notes: None,
        items: vec![klein_buch_lib::domain::invoice::InvoiceItemInput {
            position: 1,
            description: "Server-Wartung (Pauschale)".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 9_900,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
    }
}

/// Kosten-Abo (monatlich, heute fällig, automatisch) für die F1d-Tests.
fn sub_today(label: &str) -> RecurringInput {
    use chrono::Datelike;
    let today = klein_buch_lib::clock::today_berlin();
    RecurringInput {
        label: label.into(),
        vendor_contact_id: None,
        frequency: "monthly".into(),
        day_of_period: today.day() as i64,
        next_due_date: today,
        expected_amount_cents: 1_190,
        category: "software".into(),
        description_template: format!("{label} — Abo"),
        auto_create_expense: true,
        reverse_charge_13b_default: false,
    }
}

/// Hält den Scheduler-Tick der Instanz an (bis zum nächsten Entsperren), damit
/// er unter Last nicht vor dem Backup fällige Termine abarbeitet; die
/// Abbruch-Anforderung wird zurückgenommen, damit die Läufe des Tests arbeiten.
async fn pause_instance_scheduler(srv: &TestServer) {
    klein_buch_lib::scheduler::tick::stop(&srv.state.ctx).await;
    klein_buch_lib::scheduler::tick::withdraw_stop(&srv.state.ctx);
}

/// Abo-Läufe (Rechnungen, Kosten) mit der Uhr der Instanz; KDF-lastig
/// (Lock-Backups), daher serialisiert.
async fn run_recurring(srv: &TestServer) -> (usize, usize, usize) {
    let _serial = kdf_serial().await;
    let pool = srv.state.ctx.pool().unwrap();
    let clock = srv.state.ctx.clock();
    let job = klein_buch_lib::scheduler::JobCtx::from_app(&srv.state.ctx, &pool, clock.as_ref());
    let inv = klein_buch_lib::scheduler::recurring_invoice::process_due(&job)
        .await
        .unwrap();
    let sub = klein_buch_lib::scheduler::recurring::process_due(&job)
        .await
        .unwrap();
    (
        inv.created_invoices,
        sub.created_expenses,
        inv.awaiting_decision + sub.awaiting_decision,
    )
}

async fn restore_from_history(srv: TestServer, cookie: &str, backup_id: &str) -> TestServer {
    let r = {
        let _serial = kdf_serial().await;
        call(
            &srv.app,
            cookie,
            "backup_restore_apply",
            serde_json::json!({ "backupId": backup_id, "passphrase": PASS }),
        )
        .await
    };
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    restart(srv).await
}

/// Lauf F8 (ADR 0040 Nr. 62): Ein älteres Backup öffnet ein inzwischen
/// abgeschlossenes Geschäftsjahr nicht wieder. Der Abschluss der ersetzten DB
/// wird beim ersten Entsperren übernommen (Snapshot, Vermerk, Audit, Hinweis);
/// Buchungen mit Datum in diesem Jahr scheitern danach.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_keeps_fiscal_year_closed_after_backup() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let closed = i64::from(chrono::Datelike::year(
        &klein_buch_lib::clock::today_berlin(),
    )) - 1;
    let a = backup_now(&srv, &cookie).await;
    let a_id = a["historyId"].as_str().unwrap().to_string();
    // Nach dem Backup: Abschluss des Vorjahres (Protokoll direkt, der
    // Abschluss-Weg selbst ist anderswo getestet).
    {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query(
            "INSERT INTO fiscal_year_locks
                (fiscal_year, closed_at, income_total_cents, expense_total_cents,
                 afa_total_cents, surplus_cents, app_version, schema_version)
             VALUES (?, '2026-02-01 10:00:00', 500000, 100000, 0, 400000, '2026.11.0', 32)",
        )
        .bind(closed)
        .execute(&pool)
        .await
        .unwrap();
    }

    let srv = restore_from_history(srv, &cookie, &a_id).await;
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let cookie = session_cookie(&r);

    let (income, notes): (i64, Option<String>) = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_as(
            "SELECT income_total_cents, notes FROM fiscal_year_locks WHERE fiscal_year = ?",
        )
        .bind(closed)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(income, 500000, "Snapshot der ersetzten DB");
    assert!(
        notes
            .as_deref()
            .is_some_and(|n| n.starts_with("nach Restore übernommen")),
        "{notes:?}"
    );
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'fiscal_year.lock_restored_after_restore'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_fiscal_year_locks:%'"
        )
        .await,
        1
    );
    // Buchung mit Datum im abgeschlossenen Jahr: abgelehnt.
    let mut args = expense_args("Papier im Vorjahr");
    args["args"]["input"]["expenseDate"] = serde_json::Value::from(format!("{closed}-12-01"));
    let r = call(&srv.app, &cookie, "expenses_create", args).await;
    assert_ne!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        r.text()
            .contains(&format!("Geschäftsjahr {closed} ist abgeschlossen")),
        "{}",
        r.text()
    );
}

/// Lauf F1d (S1, S2, Nummernkreis-Hinweis): Restore eines **älteren** Backups
/// in dieselbe Instanz.
/// - Ein nach dem Backup abgerechneter Abo-Termin und ein gebuchtes Kosten-Abo
///   werden nach der Freigabe nicht erneut abgerechnet (Termine auf den Stand
///   vor dem Restore vorgerückt, Audit, Hinweis).
/// - Ein Entwurf, dessen Nummer nach dem Backup festgeschrieben wurde, lässt
///   sich erst nach der Bestätigung (Re-Auth, Begründung) festschreiben.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn older_backup_restore_bills_no_period_twice_and_guards_used_draft_numbers() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let today = klein_buch_lib::clock::today_berlin().to_string();
    pause_instance_scheduler(&srv).await;
    // Entwurf D (Nummer 1), Vorlage und Kosten-Abo, beide heute fällig.
    let d_id = draft_invoice(&srv).await;
    let (tmpl_id, sub_id) = {
        let pool = srv.state.ctx.pool().unwrap();
        let contact: String = sqlx::query_scalar("SELECT contact_id FROM invoices WHERE id = ?")
            .bind(&d_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let t = klein_buch_lib::db::repo::recurring_invoice::create(
            &pool,
            &tmpl_today(&contact, "Wartung", "draft"),
        )
        .await
        .unwrap();
        let s = klein_buch_lib::db::repo::recurring::create(&pool, &sub_today("Hosting"))
            .await
            .unwrap();
        (t.template.id, s.id)
    };
    let a = backup_now(&srv, &cookie).await;
    let a_id = a["historyId"].as_str().unwrap().to_string();

    // Nach dem Backup: Termin abgerechnet bzw. gebucht (vom Lauf hier oder vom
    // Scheduler-Tick der Instanz — geprüft wird der Stand), D festgeschrieben.
    run_recurring(&srv).await;
    assert_eq!(scalar_i64(&srv, "SELECT COUNT(*) FROM invoices").await, 2);
    assert_eq!(scalar_i64(&srv, "SELECT COUNT(*) FROM expenses").await, 1);
    let r = call(
        &srv.app,
        &cookie,
        "invoices_lock_and_issue",
        serde_json::json!({ "id": d_id }),
    )
    .await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    let d_number = scalar_text(
        &srv,
        &format!("SELECT invoice_number FROM invoices WHERE id = '{d_id}'"),
    )
    .await
    .unwrap();
    let tmpl_next =
        |id: &str| format!("SELECT next_due_date FROM recurring_invoices WHERE id = '{id}'");
    let sub_next =
        |id: &str| format!("SELECT next_due_date FROM recurring_subscriptions WHERE id = '{id}'");
    let next_tmpl = scalar_text(&srv, &tmpl_next(&tmpl_id)).await.unwrap();
    let next_sub = scalar_text(&srv, &sub_next(&sub_id)).await.unwrap();
    assert!(next_tmpl > today && next_sub > today);

    let srv = restore_from_history(srv, &cookie, &a_id).await;
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let cookie = session_cookie(&r);

    // S1: Termine auf dem Stand vor dem Restore, Audit und Hinweis.
    assert_eq!(
        scalar_text(&srv, &tmpl_next(&tmpl_id)).await.unwrap(),
        next_tmpl
    );
    assert_eq!(
        scalar_text(&srv, &sub_next(&sub_id)).await.unwrap(),
        next_sub
    );
    for action in [
        "recurring_invoice.advanced_after_restore",
        "recurring.advanced_after_restore",
    ] {
        assert_eq!(
            scalar_i64(
                &srv,
                &format!("SELECT COUNT(*) FROM audit_log WHERE action = '{action}'")
            )
            .await,
            1,
            "{action}"
        );
    }
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_recurring:%'
             AND body LIKE '%vorgerückt%'"
        )
        .await,
        1
    );
    // F3b: Das vorgerückte Kosten-Abo steht getrennt — seine gebuchten Kosten
    // sind mit dem ersetzten Stand verworfen und von Hand nachzutragen.
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_recurring:%'
             AND body LIKE '%Kosten-Abos: %„Hosting“%'
             AND body LIKE '%fehlen jetzt in Kosten und EÜR%von Hand erfassen%'
             AND body LIKE '%Abo-Rechnungen: %„Wartung“%'"
        )
        .await,
        1
    );
    // S2: D ist vorgemerkt, der Hinweis nennt die Nummer.
    assert_eq!(
        scalar_i64(
            &srv,
            &format!(
                "SELECT COUNT(*) FROM app_settings WHERE key = 'restored_number_conflict:invoice:{d_id}'"
            )
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &srv,
            &format!(
                "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_numbering:%'
                 AND severity = 'urgent' AND body LIKE '%{d_number}%'"
            )
        )
        .await,
        1
    );

    // Freigabe → kein zweiter Termin, keine zweite Kosten-Position.
    let cookie = reauth_cookie(&srv, &cookie).await;
    let r = call(
        &srv.app,
        &cookie,
        "scheduler_release_hold",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(run_recurring(&srv).await, (0, 0, 0));
    assert_eq!(scalar_i64(&srv, "SELECT COUNT(*) FROM invoices").await, 1);
    assert_eq!(scalar_i64(&srv, "SELECT COUNT(*) FROM expenses").await, 0);

    // D lässt sich erst nach der Bestätigung festschreiben.
    let r = call(
        &srv.app,
        &cookie,
        "invoices_lock_and_issue",
        serde_json::json!({ "id": d_id }),
    )
    .await;
    assert_eq!(r.json()["state"], "failed", "{}", r.text());
    assert!(r.text().contains("bereits festgeschrieben"), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "numbering_restored_drafts_list",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()[0]["number"], d_number.as_str(), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "numbering_restored_draft_confirm",
        serde_json::json!({ "kind": "invoice", "id": d_id, "reason": "kurz" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "numbering_restored_draft_confirm",
        serde_json::json!({
            "kind": "invoice",
            "id": d_id,
            "reason": "an das versandte Original laut Mail-Ausgang angepasst",
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json().as_array().map(Vec::len), Some(0), "{}", r.text());
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'numbering.restored_draft_confirmed'
             AND details_json LIKE '%Mail-Ausgang%'"
        )
        .await,
        1
    );
    let r = call(
        &srv.app,
        &cookie,
        "invoices_lock_and_issue",
        serde_json::json!({ "id": d_id }),
    )
    .await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
}

/// Lauf F1d (S1, ohne bekannten Stand): Kennt Klein.Buch den Stand einer Reihe
/// vor dem Restore nicht — hier simuliert, indem die Reihen nach dem Backup aus
/// der laufenden Datenbank verschwinden, wie bei einer Wiederherstellung in
/// eine leere Instanz —, verlangt ein offener Termin bis zum Restore-Tag die
/// Entscheidung, auch unter der Nachhol-Grenze. „Ab heute fortsetzen“ lässt
/// den heutigen Termin aus, „nachholen“ erstellt nur Entwürfe; Marker aus dem
/// Backup gelten nicht mehr. Ein Restore ohne höhere Zähler meldet „keine
/// Lücke“ statt des Textes für eine leere Instanz.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_without_series_state_requires_decision_and_never_auto_issues() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let today = klein_buch_lib::clock::today_berlin().to_string();
    pause_instance_scheduler(&srv).await;
    let d_id = draft_invoice(&srv).await;
    let (skip_id, catch_id, sub_id) = {
        let pool = srv.state.ctx.pool().unwrap();
        let contact: String = sqlx::query_scalar("SELECT contact_id FROM invoices WHERE id = ?")
            .bind(&d_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let t1 = klein_buch_lib::db::repo::recurring_invoice::create(
            &pool,
            &tmpl_today(&contact, "Wartung A", "issue_send"),
        )
        .await
        .unwrap();
        let t2 = klein_buch_lib::db::repo::recurring_invoice::create(
            &pool,
            &tmpl_today(&contact, "Wartung B", "issue"),
        )
        .await
        .unwrap();
        let s = klein_buch_lib::db::repo::recurring::create(&pool, &sub_today("Hosting"))
            .await
            .unwrap();
        // F3b: Reihen, für die kein Lauf etwas erzeugt, bekommen keinen
        // Prüf-Marker — eine abgelaufene Vorlage und ein Abo ohne Auto-Buchung.
        let expired = klein_buch_lib::db::repo::recurring_invoice::create(
            &pool,
            &tmpl_today(&contact, "Abgelaufen", "issue"),
        )
        .await
        .unwrap();
        let yesterday =
            (klein_buch_lib::clock::today_berlin() - chrono::Duration::days(1)).to_string();
        sqlx::query("UPDATE recurring_invoices SET end_date = ? WHERE id = ?")
            .bind(&yesterday)
            .bind(&expired.template.id)
            .execute(&pool)
            .await
            .unwrap();
        let mut manual_input = sub_today("Manuell");
        manual_input.auto_create_expense = false;
        klein_buch_lib::db::repo::recurring::create(&pool, &manual_input)
            .await
            .unwrap();
        // Marker, die nicht aus dem Backup weiterwirken dürfen.
        klein_buch_lib::db::repo::app_settings::set(
            &pool,
            &format!("recurring_catch_up_ok:inv:{}", t1.template.id),
            &today,
        )
        .await
        .unwrap();
        (t1.template.id, t2.template.id, s.id)
    };
    let a = backup_now(&srv, &cookie).await;
    let a_id = a["historyId"].as_str().unwrap().to_string();
    {
        let pool = srv.state.ctx.pool().unwrap();
        for sql in [
            "DELETE FROM recurring_invoice_items",
            "DELETE FROM recurring_invoices",
            "DELETE FROM recurring_subscriptions",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
    }

    let srv = restore_from_history(srv, &cookie, &a_id).await;
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let cookie = session_cookie(&r);

    // Prüfung je Reihe, Marker aus dem Backup verworfen, Hinweise.
    for key in [
        format!("recurring_restore_review:inv:{skip_id}"),
        format!("recurring_restore_review:inv:{catch_id}"),
        format!("recurring_restore_review:sub:{sub_id}"),
    ] {
        assert_eq!(
            scalar_text(
                &srv,
                &format!("SELECT value FROM app_settings WHERE key = '{key}'")
            )
            .await
            .as_deref(),
            Some(today.as_str()),
            "{key}"
        );
    }
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'recurring|_catch|_up|_ok:%' ESCAPE '|'"
        )
        .await,
        0
    );
    // F3b: nur die drei Reihen mit Handlungsweg; die abgelaufene Vorlage und
    // das Abo ohne Auto-Buchung stehen weder als Marker noch im Hinweis.
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'recurring|_restore|_review:%' ESCAPE '|'"
        )
        .await,
        3
    );
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_recurring:%'
             AND (body LIKE '%Abgelaufen%' OR body LIKE '%Manuell%')"
        )
        .await,
        0
    );
    let r = call(
        &srv.app,
        &cookie,
        "recurring_restore_review_list",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["templates"].as_array().map(Vec::len), Some(2));
    assert_eq!(r.json()["subscriptions"][0], sub_id.as_str());
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_recurring:%'
             AND body LIKE '%kennt Klein.Buch den Stand%'"
        )
        .await,
        1
    );
    // Gleiche Zählerstände: kurzer Hinweis „keine Lücke“ (kein DR-Text).
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_numbering:%'
             AND severity = 'info' AND body LIKE '%keine wird doppelt vergeben%'"
        )
        .await,
        1
    );

    // Nach der Freigabe: nichts automatisch, nur „Entscheidung nötig“.
    let cookie = reauth_cookie(&srv, &cookie).await;
    let r = call(
        &srv.app,
        &cookie,
        "scheduler_release_hold",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(run_recurring(&srv).await, (0, 0, 3));

    // „Ab heute fortsetzen“ lässt auch den heutigen Termin aus.
    for (cmd, id) in [
        ("recurring_invoices_resolve_backlog", skip_id.as_str()),
        ("recurring_resolve_backlog", sub_id.as_str()),
    ] {
        let r = call(
            &srv.app,
            &cookie,
            cmd,
            serde_json::json!({ "id": id, "mode": "skip" }),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{cmd}: {}", r.text());
        assert!(
            r.json()["nextDueDate"].as_str().unwrap() > today.as_str(),
            "{cmd}: {}",
            r.text()
        );
    }
    // „Nachholen“: nur als Entwurf, auch im Modus „Auto-Rechnung“.
    let r = call(
        &srv.app,
        &cookie,
        "recurring_invoices_resolve_backlog",
        serde_json::json!({ "id": catch_id, "mode": "catchUp" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    // Lauf hier oder Scheduler-Tick der Instanz — geprüft wird der Stand.
    run_recurring(&srv).await;
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM invoices WHERE locked_at IS NOT NULL"
        )
        .await,
        0,
        "nachgeholt nur als Entwurf"
    );
    assert_eq!(scalar_i64(&srv, "SELECT COUNT(*) FROM invoices").await, 2);
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'recurring|_restore|_review:%' ESCAPE '|'"
        )
        .await,
        0,
        "alle Prüfungen erledigt"
    );
    assert_eq!(run_recurring(&srv).await, (0, 0, 0));
}

/// D-24 (Disaster Recovery): Nummernkreis von Hand anheben — nur aufwärts,
/// mit Begründung, Re-Auth, Audit; die nächste Nummer folgt dem neuen Stand.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn numbering_raise_only_upwards_with_reason() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let first = draft_invoice(&srv).await;
    let year = scalar_i64(
        &srv,
        &format!("SELECT fiscal_year FROM invoices WHERE id = '{first}'"),
    )
    .await;
    let raise = |value: i64, reason: &str| {
        serde_json::json!({
            "docType": "invoice",
            "fiscalYear": year,
            "lastValue": value,
            "reason": reason,
        })
    };
    // ohne Re-Auth → 403
    let r = call(
        &srv.app,
        &cookie,
        "numbering_raise",
        raise(41, "laut Mail-Ausgang RE-41"),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    let cookie = reauth_cookie(&srv, &cookie).await;
    // zu kurze Begründung, Absenken, unbekannter Kreis → abgelehnt
    let r = call(&srv.app, &cookie, "numbering_raise", raise(41, "kurz")).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "numbering_raise",
        raise(1, "Absenken versucht"),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.text());
    assert!(r.text().contains("absenken"), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "numbering_raise",
        serde_json::json!({ "docType": "x", "fiscalYear": year, "lastValue": 5, "reason": "unbekannter Kreis" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    // gültig
    let r = call(
        &srv.app,
        &cookie,
        "numbering_raise",
        raise(41, "laut Mail-Ausgang zuletzt RE-41"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let list = r.json();
    let invoice = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["docType"] == "invoice")
        .unwrap();
    assert_eq!(invoice["lastValue"], 41);
    assert_eq!(invoice["lastNumber"], format!("RE-{year}-0041"));
    let details: String = scalar_text(
        &srv,
        "SELECT details_json FROM audit_log WHERE action = 'numbering.raised'",
    )
    .await
    .unwrap();
    assert!(
        details.contains("\"from\":1")
            && details.contains("\"to\":41")
            && details.contains("Mail-Ausgang"),
        "{details}"
    );
    let next = draft_invoice(&srv).await;
    assert_eq!(
        scalar_text(
            &srv,
            &format!("SELECT invoice_number FROM invoices WHERE id = '{next}'"),
        )
        .await
        .unwrap(),
        format!("RE-{year}-0042")
    );
}

/// Lauf F3b (Nummernkreis-Hinweis bei fremdem Stand): Wiederherstellung nach
/// einem Totalausfall in eine neue Instanz, in der vorher ein Test-Entwurf
/// angelegt wurde (Zähler 1). Das Backup des ausgefallenen Servers hat einen
/// höheren Zähler (2) — kein Kreis wird angehoben, der Hinweis ist trotzdem
/// die dringende Anleitung zum Anheben und nicht „keine Lücke“: Der alte
/// Server kann nach dem Backup weiter nummeriert haben.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_over_other_instance_with_lower_counters_asks_to_raise() {
    let a = test_server();
    let cookie_a = setup(&a).await;
    let _ = draft_invoice(&a).await;
    let _ = draft_invoice(&a).await;
    let kbk = backup_bytes(&a, &cookie_a).await;

    let b = destroy_and_recreate(a).await;
    let cookie_b = setup(&b).await;
    let _ = draft_invoice(&b).await;
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT last_value FROM doc_number_counters WHERE doc_type = 'invoice'"
        )
        .await,
        1
    );
    let cookie_b = reauth_cookie(&b, &cookie_b).await;
    let r = restore_upload(&b, &cookie_b, &kbk, PASS).await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    let b = restart(b).await;
    let r = login(&b, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    assert_eq!(
        scalar_i64(
            &b,
            "SELECT last_value FROM doc_number_counters WHERE doc_type = 'invoice'"
        )
        .await,
        2,
        "Stand des Backups, nichts angehoben"
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_numbering:%'
             AND severity = 'urgent'
             AND body LIKE '%höher als in der ersetzten Datenbank (Rechnungen (RE) %: 2)%'
             AND body LIKE '%Nummernkreise unter Einstellungen%'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'restore_numbering:%'
             AND (severity = 'info' OR body LIKE '%keine wird doppelt vergeben%')"
        )
        .await,
        0
    );
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.applied'
             AND details_json LIKE '%\"counters_ahead\":1%'"
        )
        .await,
        1
    );
}

/// W14-037 (Lauf F4a): Entsperren mit Migration (Pflicht-Backup + Umstellung)
/// über die Wartezeit der Anmelde-Route hinaus antwortet 202 mit
/// Ticket-Cookie; die Abholung liefert die Sitzung, die Datenbank ist
/// migriert. Die Wartezeit ist hier auf 0 verkürzt (statt einer künstlich
/// verzögerten Migration: jede echte Migration dauert länger).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_unlock_with_migration_answers_202_and_poll_delivers_session() {
    use axum::http::header;
    let srv = test_server();
    let _cookie = setup(&srv).await;
    srv.state.ctx.take_pool().unwrap().close().await;
    srv.state.ctx.session.clear();
    let db = srv.state.ctx.paths.db_file.clone();
    for suffix in ["", "-wal", "-shm"] {
        let mut p = db.as_os_str().to_os_string();
        p.push(suffix);
        let _ = std::fs::remove_file(p);
    }
    let old = EXPECTED_SCHEMA_VERSION - 1;
    {
        let _serial = kdf_serial().await;
        let pool = klein_buch_lib::db::open_pool(&db, Some(PASS))
            .await
            .unwrap();
        migrator_without_last().run(&pool).await.unwrap();
        pool.close().await;
    }
    srv.state
        .set_login_sync_wait_for_tests(std::time::Duration::ZERO);
    let _serial = kdf_serial().await;
    let code = next_totp(&srv);
    let r = send(
        &srv.app,
        post(
            "/api/auth/login",
            &serde_json::json!({ "passphrase": PASS, "totpCode": code }).to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.text());
    let ticket = r
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|raw| {
            raw.split(';')
                .next()
                .and_then(|c| c.strip_prefix("__Host-kb_login="))
                .map(str::to_string)
        })
        .expect("Ticket-Cookie");
    let mut done = None;
    for _ in 0..1200 {
        let mut req = post("/api/auth/login/poll", "{}", None, None);
        req.headers_mut().insert(
            header::COOKIE,
            format!("__Host-kb_login={ticket}").parse().unwrap(),
        );
        let r = send(&srv.app, req).await;
        if r.status != StatusCode::ACCEPTED {
            done = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let done = done.expect("Entsperren nach höchstens 60 s fertig");
    assert_eq!(done.status, StatusCode::OK, "{}", done.text());
    let cookie = session_cookie(&done);
    let s = send(&srv.app, get("/api/auth/status", Some(&cookie))).await;
    assert_eq!(s.json()["state"], "ok");
    assert_eq!(
        scalar_i64(
            &srv,
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = 'schema_version'"
        )
        .await,
        EXPECTED_SCHEMA_VERSION as i64,
        "migriert"
    );
    assert_eq!(
        scalar_i64(
            &srv,
            &format!(
                "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'manual' \
                 AND db_schema_version = {old}"
            )
        )
        .await,
        1,
        "Pflicht-Backup vor der Umstellung"
    );
}
