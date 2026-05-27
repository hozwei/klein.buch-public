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
