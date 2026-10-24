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
    let r = call(
        &srv.app,
        &cookie,
        "backup_restore_apply",
        serde_json::json!({ "backupId": "fremd", "passphrase": PASS }),
    )
    .await;
    assert_eq!(r.json()["state"], "failed", "{}", r.text());
    assert!(r.text().contains("Nur lokale Backups"), "{}", r.text());
    // beide oder keine Quelle
    let r = call(
        &srv.app,
        &cookie,
        "backup_restore_apply",
        serde_json::json!({ "passphrase": PASS }),
    )
    .await;
    assert_eq!(r.json()["state"], "failed", "{}", r.text());

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
    let m = klein_buch_lib::backup::manifest::read_manifest_only(&std::fs::read(name).unwrap())
        .unwrap();
    assert_eq!(m.schema_version, old);
}

/// Plan W7/W8: `.kbk` mit Schema 30 → Restore → beim Entsperren
/// Pflicht-Backup (Schema 30) → Vorwärtsmigration (0031).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schema_30_backup_is_restored_and_migrated_forward() {
    let old = EXPECTED_SCHEMA_VERSION - 1;
    assert!(old >= MIN_RESTORE_SCHEMA_VERSION);
    // Instanz A auf den Stand vor der letzten Migration zurücksetzen
    let a = test_server();
    let cookie_a = setup(&a).await;
    {
        let pool = a.state.ctx.pool().unwrap();
        let last = klein_buch_lib::db::MIGRATOR.migrations.last().unwrap();
        assert!(
            last.description.contains("secrets"),
            "Test setzt 0031 zurück: {}",
            last.description
        );
        sqlx::query("DROP TABLE secrets")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(last.version)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'schema_version'")
            .bind(old.to_string())
            .execute(&pool)
            .await
            .unwrap();
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
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'secrets'"
        )
        .await,
        1,
        "0031 angewendet"
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
    assert_eq!(
        scalar_i64(
            &b,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'pre_restore'"
        )
        .await,
        1,
        "Pre-Restore-Backup entstand (Prüfung erst nach dem Entpacken)"
    );
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

    // Stand A: eine Rechnung. Danach zwei weitere (Nummern 2 und 3).
    let _ = locked_invoice(&srv, &cookie).await;
    let a = backup_now(&srv, &cookie).await;
    let a_id = a["historyId"].as_str().unwrap().to_string();
    let _ = locked_invoice(&srv, &cookie).await;
    let _ = locked_invoice(&srv, &cookie).await;
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
             AND body LIKE '%von 1 auf 3%'"
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
