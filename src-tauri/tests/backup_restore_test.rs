//! Integration-Tests für Block 4 — Backup, Restore-Decrypt, Migrations-Export.
//!
//! Diese Tests fahren das **echte** Schema über den eingebetteten MIGRATOR auf
//! eine Datei-DB und prüfen die End-to-End-Pfade:
//! - Backup erstellen → entschlüsseln → entpacken → DB wieder lesbar.
//! - Falsche Passphrase / manipuliertes Backup schlagen fehl (GCM-Auth).
//! - Migrations-Export-ZIP enthält alle Tabellen + Archive + Reader.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use klein_buch_lib::backup::{self, restore};
use klein_buch_lib::config::Paths;
use klein_buch_lib::migration_export::export;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tempfile::TempDir;

fn paths_for(dir: &Path) -> Paths {
    Paths {
        data_dir: dir.to_path_buf(),
        db_file: dir.join("klein-buch.sqlite"),
        archive_dir: dir.join("archive"),
        backups_dir: dir.join("backups"),
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
        resource_dir: std::path::PathBuf::from("/non/existent/resources"),
        web_dir: std::path::PathBuf::from("/non/existent/web"),
        backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
    }
}

async fn migrated_pool(db_file: &Path) -> SqlitePool {
    std::fs::create_dir_all(db_file.parent().unwrap()).unwrap();
    let url = format!("sqlite://{}", db_file.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    klein_buch_lib::db::MIGRATOR.run(&pool).await.unwrap();
    pool
}

async fn insert_demo_contact(pool: &SqlitePool, id: &str, name: &str) {
    sqlx::query("INSERT INTO contacts (id, contact_type, name) VALUES (?, 'customer', ?)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn backup_roundtrip_decrypt_and_extract() {
    let dir = TempDir::new().unwrap();
    let paths = paths_for(dir.path());
    let pool = migrated_pool(&paths.db_file).await;
    insert_demo_contact(&pool, "c-1", "Wildbach Computerhilfe").await;

    // Eine Archive-Datei simulieren.
    std::fs::create_dir_all(paths.archive_dir.join("2026/invoices/pdf")).unwrap();
    std::fs::write(
        paths.archive_dir.join("2026/invoices/pdf/RE-2026-0001.pdf"),
        b"%PDF-1.7 demo",
    )
    .unwrap();

    let outcome = backup::create_now(&pool, &paths, "passphrase-1234", "manual")
        .await
        .unwrap();
    assert!(Path::new(&outcome.file_path).is_file());
    assert!(!outcome.retention_tag.is_empty());

    // backup_history hat genau eine Zeile.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backup_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);

    // Entschlüsseln + entpacken.
    let bytes = std::fs::read(&outcome.file_path).unwrap();
    let (manifest, content) = restore::decrypt_backup(&bytes, "passphrase-1234").unwrap();
    assert_eq!(
        manifest.schema_version,
        klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION
    );

    let staging: PathBuf = dir.path().join("verify-staging");
    extract_zip_to(&content, &staging);
    let restored_db = staging.join("klein-buch.sqlite");
    assert!(restored_db.is_file());
    assert!(staging
        .join("archive/2026/invoices/pdf/RE-2026-0001.pdf")
        .is_file());

    // Wiederhergestellte DB öffnen → Kontakt muss vorhanden sein.
    let pool2 = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}", restored_db.to_string_lossy()))
        .await
        .unwrap();
    let name: String = sqlx::query_scalar("SELECT name FROM contacts WHERE id = 'c-1'")
        .fetch_one(&pool2)
        .await
        .unwrap();
    assert_eq!(name, "Wildbach Computerhilfe");

    // Falsche Passphrase schlägt fehl.
    assert!(restore::decrypt_backup(&bytes, "falsch").is_err());
}

#[tokio::test]
async fn tampered_backup_fails_auth() {
    let dir = TempDir::new().unwrap();
    let paths = paths_for(dir.path());
    let pool = migrated_pool(&paths.db_file).await;
    insert_demo_contact(&pool, "c-2", "ACME").await;

    let outcome = backup::create_now(&pool, &paths, "pw-abcdef", "manual")
        .await
        .unwrap();
    let mut bytes = std::fs::read(&outcome.file_path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01; // ein Byte im Ciphertext kippen
    assert!(restore::decrypt_backup(&bytes, "pw-abcdef").is_err());
}

#[tokio::test]
async fn migration_export_contains_all_data() {
    let dir = TempDir::new().unwrap();
    let paths = paths_for(dir.path());
    let pool = migrated_pool(&paths.db_file).await;
    insert_demo_contact(&pool, "c-3", "Müller GmbH").await;

    std::fs::create_dir_all(paths.archive_dir.join("2026")).unwrap();
    std::fs::write(paths.archive_dir.join("2026/beleg.xml"), b"<xml/>").unwrap();

    let zip_path = dir.path().join("export.zip");
    let report = export::export_all(&pool, &paths, &zip_path).await.unwrap();
    assert!(report.table_count >= 10); // Phase-1-Schema hat ≥ 10 Tabellen
    assert!(report.archive_file_count >= 1);

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(names.contains(&"tables/contacts.json".to_string()));
    assert!(names.contains(&"archive/2026/beleg.xml".to_string()));
    assert!(names.contains(&"manifest.json".to_string()));
    assert!(names.contains(&"read_export.py".to_string()));
    assert!(names
        .iter()
        .any(|n| n.starts_with("schema/") && n.ends_with(".sql")));

    use std::io::Read;
    let mut f = archive.by_name("tables/contacts.json").unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    assert!(s.contains("Müller GmbH"));
}

fn extract_zip_to(zip_bytes: &[u8], dest: &Path) {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let out = dest.join(file.name());
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out).unwrap();
            continue;
        }
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        std::fs::write(&out, &buf).unwrap();
    }
}

// ---- Rotation mit DB und Dateien (KB-0035, W14-011; Lauf F3a) -------------

async fn history(
    pool: &SqlitePool,
    id: &str,
    created: &str,
    tag: &str,
    trigger: &str,
    path: &Path,
) {
    sqlx::query(
        "INSERT INTO backup_history (id, created_at, target_path, file_hash_sha256,
            file_size_bytes, retention_tag, trigger_reason, db_schema_version, app_version)
         VALUES (?, ?, ?, 'x', 1, ?, ?, 32, 'x')",
    )
    .bind(id)
    .bind(created)
    .bind(path.to_string_lossy().to_string())
    .bind(tag)
    .bind(trigger)
    .execute(pool)
    .await
    .unwrap();
}

fn kbk(dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, b"kbk").unwrap();
    p
}

async fn rows(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM backup_history ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// KB-0035: `rotation::run` gegen echte DB und Dateien — Floor kurz (ein
/// Off-Site-Ziel war zuletzt erfolgreich), Off-Site lang; manuelle und
/// Pre-Restore-Backups bleiben, das jüngste bleibt, gelöschte Dateien sind
/// wirklich weg. W14-011: Einträge, die (aufgelöst) aus dem Backup-Ordner
/// hinauszeigen, keine `.kbk` sind oder als Symlink nach draußen zeigen,
/// verlieren nur ihren Eintrag — die Dateien bleiben.
#[tokio::test]
async fn rotation_run_deletes_files_only_under_the_roots() {
    let dir = TempDir::new().unwrap();
    let paths = paths_for(dir.path());
    let pool = migrated_pool(&paths.db_file).await;
    let floor = paths.backups_dir.clone();
    let targets_root = dir.path().join("backup-targets");
    let offsite_dir = targets_root.join("nas/kb");
    std::fs::create_dir_all(&offsite_dir).unwrap();
    let outside = dir.path().join("fremd");

    // Ziel „nas/kb“, zuletzt erfolgreich → Floor-Policy kurz (7/3/1).
    let target = backup::target::TargetEntry {
        id: "nas".into(),
        label: "NAS".into(),
        target: backup::target::BackupTarget::Directory {
            path: "nas/kb".into(),
        },
    };
    backup::target::save_targets(&pool, std::slice::from_ref(&target))
        .await
        .unwrap();
    backup::target::store_results(
        &pool,
        std::slice::from_ref(&target),
        &[(
            "nas".into(),
            backup::target::TargetResult {
                ok: true,
                at: "2026-05-10T02:30:00+02:00".into(),
                error: None,
            },
        )],
    )
    .await
    .unwrap();

    // Floor: zehn Tage (01.–10.05.), dazu manuell und Pre-Restore (alt).
    for day in 1..=10 {
        let name = format!("klein-buch-202605{day:02}.kbk");
        let p = kbk(&floor, &name);
        history(
            &pool,
            &format!("f{day:02}"),
            &format!("2026-05-{day:02} 00:30:00"),
            "daily",
            "auto_daily",
            &p,
        )
        .await;
    }
    let manual = kbk(&floor, "klein-buch-2020-manuell.kbk");
    history(
        &pool,
        "m-manual",
        "2020-01-02 10:00:00",
        "manual",
        "manual",
        &manual,
    )
    .await;
    let pre = kbk(&floor, "klein-buch-2019-pre.kbk");
    history(
        &pool,
        "m-pre",
        "2019-06-01 10:00:00",
        "manual",
        "pre_restore",
        &pre,
    )
    .await;

    // Manipulierte Floor-Einträge (alt genug für die Rotation).
    let escaped_target = kbk(&outside, "klein-buch-fremd.kbk");
    let escaped = floor.join("..").join("fremd").join("klein-buch-fremd.kbk");
    history(
        &pool,
        "x-dotdot",
        "2026-05-02 00:10:00",
        "daily",
        "auto_critical",
        &escaped,
    )
    .await;
    let txt = floor.join("notiz.txt");
    std::fs::write(&txt, b"keine Sicherung").unwrap();
    history(
        &pool,
        "x-txt",
        "2026-05-02 00:11:00",
        "daily",
        "auto_critical",
        &txt,
    )
    .await;
    let link_target = kbk(&outside, "ziel.kbk");
    let link = floor.join("klein-buch-link.kbk");
    std::os::unix::fs::symlink(&link_target, &link).unwrap();
    history(
        &pool,
        "x-link",
        "2026-05-02 00:12:00",
        "daily",
        "auto_critical",
        &link,
    )
    .await;

    // Off-Site: 35 Tage (06.04.–10.05.).
    let start = chrono::NaiveDate::from_ymd_opt(2026, 4, 6).unwrap();
    for i in 0..35 {
        let day = start + chrono::Duration::days(i);
        let p = kbk(
            &offsite_dir,
            &format!("klein-buch-{}.kbk", day.format("%Y%m%d")),
        );
        history(
            &pool,
            &format!("o{i:02}"),
            &format!("{day} 00:30:00"),
            "daily",
            "auto_daily",
            &p,
        )
        .await;
    }

    let now = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 5, 10, 12, 0, 0).unwrap();
    let deleted = backup::rotation::run(&pool, &floor, &targets_root, now)
        .await
        .unwrap();

    let left = rows(&pool).await;
    // Floor: 04.–10.05. bleiben (7 Tage; Monat und Jahr decken sich mit dem
    // 10.), 01.–03. gehen samt Datei.
    for day in 1..=10 {
        let id = format!("f{day:02}");
        let file = floor.join(format!("klein-buch-202605{day:02}.kbk"));
        if day >= 4 {
            assert!(left.contains(&id) && file.exists(), "{id} bleibt");
        } else {
            assert!(!left.contains(&id) && !file.exists(), "{id} gelöscht");
        }
    }
    assert!(left.contains(&"m-manual".to_string()) && manual.exists());
    assert!(left.contains(&"m-pre".to_string()) && pre.exists());
    // Off-Site: 30 Tage (11.04.–10.05.), 06.–10.04. gehen samt Datei.
    for i in 0..35 {
        let day = start + chrono::Duration::days(i);
        let id = format!("o{i:02}");
        let file = offsite_dir.join(format!("klein-buch-{}.kbk", day.format("%Y%m%d")));
        assert_eq!(left.contains(&id), i >= 5, "{id}");
        assert_eq!(file.exists(), i >= 5, "{id}");
    }
    // W14-011: Einträge weg, Dateien unangetastet.
    for id in ["x-dotdot", "x-txt", "x-link"] {
        assert!(!left.contains(&id.to_string()), "{id}");
    }
    assert!(escaped_target.exists(), "Datei außerhalb bleibt");
    assert!(txt.exists(), "keine .kbk — bleibt");
    assert!(
        link_target.exists() && link.symlink_metadata().is_ok(),
        "Symlink bleibt"
    );
    assert_eq!(deleted, 3 + 3 + 5);
    let refused: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.rotation.prune'
           AND json_extract(details_json, '$.file') = 'refused'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(refused, 3);
    let removed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.rotation.prune'
           AND json_extract(details_json, '$.file') = 'removed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(removed, 8);
}
