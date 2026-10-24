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
    *srv.state.ctx.scheduler_gate.lock().await
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
