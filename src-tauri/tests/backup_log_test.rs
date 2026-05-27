//! Integration-Tests für das Backup-Protokoll `backup_log` (G1-LOG, ADR 0034).
//!
//! Deckt ab: Schreiben (insert), Lesen (list neueste-zuerst + limit), Suche/
//! Filter (Volltext, Status, Auslöser, Ziel-Typ) und die Append-only-Hard-Line
//! (DB-Trigger lehnen UPDATE/DELETE ab). Geheimnis-Check: das Protokoll trägt
//! keine Passphrase (nur Fehlertext in `detail`).

use klein_buch_lib::db::repo::backup_log::{self, BackupLogEntry, BackupLogFilter};
use klein_buch_lib::db::MIGRATOR;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

fn floor_ok(trigger: &str, file_name: &str) -> BackupLogEntry {
    BackupLogEntry {
        trigger: trigger.into(),
        target_kind: "local".into(),
        target_label: None,
        file_name: file_name.into(),
        full_path: format!("C:\\Users\\m\\AppData\\…\\backups\\{file_name}"),
        size_bytes: 12_345,
        status: "ok".into(),
        detail: None,
    }
}

fn offsite_failed(trigger: &str, file_name: &str) -> BackupLogEntry {
    BackupLogEntry {
        trigger: trigger.into(),
        target_kind: "sftp".into(),
        target_label: Some("backup.example.de".into()),
        file_name: file_name.into(),
        full_path: format!("sftp://manuel@backup.example.de:22/klein-buch/{file_name}"),
        size_bytes: 12_345,
        status: "failed".into(),
        detail: Some("Off-Site-Spiegelung fehlgeschlagen: connection refused".into()),
    }
}

#[tokio::test]
async fn insert_and_list_newest_first() {
    let (pool, _dir) = setup_pool().await;

    backup_log::insert(&pool, &floor_ok("manual", "klein-buch-20260525-100000.kbk"))
        .await
        .unwrap();
    backup_log::insert(
        &pool,
        &offsite_failed("auto_critical", "klein-buch-20260525-100500.kbk"),
    )
    .await
    .unwrap();

    let all = backup_log::list(&pool, 100).await.unwrap();
    assert_eq!(all.len(), 2);
    // created_at-DESC, id-DESC: der zuletzt eingefügte (UUIDv7, größer) zuerst.
    assert_eq!(all[0].trigger, "auto_critical");
    assert_eq!(all[0].target_kind, "sftp");
    assert_eq!(all[0].status, "failed");
    assert_eq!(all[0].target_label.as_deref(), Some("backup.example.de"));
    assert_eq!(
        all[0].detail.as_deref(),
        Some("Off-Site-Spiegelung fehlgeschlagen: connection refused")
    );
    assert_eq!(all[1].trigger, "manual");
    assert_eq!(all[1].target_kind, "local");
    assert_eq!(all[1].status, "ok");
    assert!(all[1].detail.is_none());

    // limit greift.
    let one = backup_log::list(&pool, 1).await.unwrap();
    assert_eq!(one.len(), 1);
}

#[tokio::test]
async fn search_filters_by_status_trigger_target_and_text() {
    let (pool, _dir) = setup_pool().await;

    backup_log::insert(&pool, &floor_ok("manual", "klein-buch-A.kbk"))
        .await
        .unwrap();
    backup_log::insert(&pool, &floor_ok("auto_daily", "klein-buch-B.kbk"))
        .await
        .unwrap();
    backup_log::insert(&pool, &offsite_failed("auto_critical", "klein-buch-C.kbk"))
        .await
        .unwrap();

    // Status-Filter.
    let failed = backup_log::search(
        &pool,
        &BackupLogFilter {
            status: Some("failed".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].file_name, "klein-buch-C.kbk");

    // Auslöser-Filter.
    let daily = backup_log::search(
        &pool,
        &BackupLogFilter {
            trigger: Some("auto_daily".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0].file_name, "klein-buch-B.kbk");

    // Ziel-Typ-Filter.
    let local = backup_log::search(
        &pool,
        &BackupLogFilter {
            target_kind: Some("local".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(local.len(), 2);
    assert!(local.iter().all(|e| e.target_kind == "local"));

    // Volltext über Dateiname/Pfad/Label/Detail.
    let by_host = backup_log::search(
        &pool,
        &BackupLogFilter {
            search: Some("example.de".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(by_host.len(), 1);
    assert_eq!(by_host[0].file_name, "klein-buch-C.kbk");
}

#[tokio::test]
async fn append_only_rejects_update_and_delete() {
    let (pool, _dir) = setup_pool().await;
    let id = backup_log::insert(&pool, &floor_ok("manual", "klein-buch-9.kbk"))
        .await
        .unwrap();

    // UPDATE muss vom Trigger abgelehnt werden.
    let upd = sqlx::query("UPDATE backup_log SET status = 'failed' WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(
        upd.is_err(),
        "UPDATE auf backup_log darf nicht erlaubt sein"
    );

    // DELETE ebenso.
    let del = sqlx::query("DELETE FROM backup_log WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(
        del.is_err(),
        "DELETE auf backup_log darf nicht erlaubt sein"
    );

    // Eintrag ist unverändert vorhanden.
    let all = backup_log::list(&pool, 10).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, "ok");
}

#[tokio::test]
async fn check_constraints_reject_invalid_trigger_and_target() {
    let (pool, _dir) = setup_pool().await;

    // Ungültiger Auslöser (Entwurfs-Vokabular 'weekly' wird nie erzeugt) → CHECK.
    let bad_trigger = backup_log::insert(
        &pool,
        &BackupLogEntry {
            trigger: "weekly".into(),
            ..floor_ok("manual", "x.kbk")
        },
    )
    .await;
    assert!(
        bad_trigger.is_err(),
        "ungültiger trigger muss am CHECK scheitern"
    );

    // Ungültiger Ziel-Typ ('cloud_folder' aus dem Entwurf) → CHECK.
    let bad_target = backup_log::insert(
        &pool,
        &BackupLogEntry {
            target_kind: "cloud_folder".into(),
            ..floor_ok("manual", "y.kbk")
        },
    )
    .await;
    assert!(
        bad_target.is_err(),
        "ungültiger target_kind muss am CHECK scheitern"
    );
}
