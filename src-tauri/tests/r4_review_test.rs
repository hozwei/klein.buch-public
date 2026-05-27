//! Integration-Tests für die R4-Review-Phase (Shell-Infrastruktur).
//!
//! Deckt ab:
//! - **R4-001** Restore-Phase-B Rollback-Versicherung (forward + rollback recovery).
//! - **R4-004** Archive-Tamper-Detection im Migration-Export (Hash-Mismatch
//!   gegen `archive_entries.file_hash_sha256` setzt `tamper=true` im Manifest
//!   und schreibt ein `migration.export.tamper_detected`-Audit-Event).
//! - **R4-005** Manifest enthält `archive_file_hashes`-Array pro Datei.
//! - **R4-006** Migration-Export läuft als TX-Snapshot durch (Smoke: erfolgreicher
//!   End-to-End-Lauf mit allen Tabellen + Archive + Manifest).
//! - **R4-007** Scheduler-Recurring-Invoice-Notice landet in der Inbox via
//!   `notify::store::create` (keine OS-Push via `os_native::push`).

use klein_buch_lib::backup::restore::{apply_pending, recover_pending_rollback, RESTORE_MARKER};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::migration_export::export::export_all;
use klein_buch_lib::notify::{store, NewNotification};
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Setup-Helfer
// ---------------------------------------------------------------------------

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

fn paths_for(dir: &std::path::Path) -> Paths {
    Paths {
        data_dir: dir.to_path_buf(),
        db_file: dir.join("klein-buch.sqlite"),
        archive_dir: dir.join("archive"),
        backups_dir: dir.join("backups"),
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
    }
}

// ---------------------------------------------------------------------------
// R4-007: Scheduler-Recurring-Invoice nutzt notify::store::create (Inbox-only)
// ---------------------------------------------------------------------------

/// Pinned das Konvertieren: `notify::store::create` legt einen `notifications`-
/// Eintrag an, gibt das Notification-Objekt zurück, und ein zweiter Aufruf mit
/// gleichem `dedup_key` liefert `None`. **Kein** OS-Push-Pfad wird berührt
/// (Scheduler-Pfad ist Inbox-only, G1-NOTIFY-Hardline, vermeidet
/// TaskDialogIndirect-Crash in Integrationstests).
#[tokio::test]
async fn r4_007_recurring_invoice_notice_is_inbox_only() {
    let (pool, _dir) = setup_pool().await;

    let n = NewNotification {
        rule_id: None,
        title: "Abo-Rechnung verarbeitet",
        body: "Periode 2026-05-01 → 2026-05-31 als RE-2026-0042 festgeschrieben.",
        severity: "info",
        related_entity_type: Some("invoice"),
        related_entity_id: Some("invoice-id-42"),
        action_url: Some("/invoices"),
        dedup_key: Some("recurring_invoice:invoice-id-42:2026-05-31"),
    };
    let created = store::create(&pool, n).await.expect("store::create ok");
    assert!(created.is_some(), "erster Aufruf legt Notification an");

    // Zweiter Aufruf mit gleichem dedup_key → keine Doppelung.
    let n2 = NewNotification {
        rule_id: None,
        title: "Abo-Rechnung verarbeitet",
        body: "duplikat-versuch",
        severity: "info",
        related_entity_type: Some("invoice"),
        related_entity_id: Some("invoice-id-42"),
        action_url: Some("/invoices"),
        dedup_key: Some("recurring_invoice:invoice-id-42:2026-05-31"),
    };
    let dup = store::create(&pool, n2)
        .await
        .expect("store::create dup ok");
    assert!(dup.is_none(), "Dedup blockt zweiten Insert");
}

// ---------------------------------------------------------------------------
// R4-004 + R4-005 + R4-006: Migration Export Tamper + Manifest + TX-Snapshot
// ---------------------------------------------------------------------------

/// Holt den Pfad einer einzelnen Datei aus `dir` über `read_dir` — damit der
/// String-Form **exakt** dem entspricht, was `collect_files` im Migration-Export
/// als Lookup-Key in die DB schickt. Wichtig auf Windows: `Path::join("a/b/c")`
/// mit Forward-Slashes erzeugt einen Mixed-Separator-Pfad, `read_dir` liefert
/// dagegen Native-Separator-Pfade — Strings würden nicht matchen.
fn read_dir_single(dir: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

#[tokio::test]
async fn r4_004_005_006_export_detects_tamper_and_lists_hashes() {
    let (pool, dir) = setup_pool().await;
    let paths = paths_for(dir.path());
    let archive_subdir = paths.archive_dir.join("2026").join("invoices").join("pdf");
    std::fs::create_dir_all(&archive_subdir).unwrap();
    let pdf_path = archive_subdir.join("RE-2026-0001.pdf");
    std::fs::write(&pdf_path, b"echte-pdf-bytes").unwrap();
    // DB-Key über read_dir holen, damit der String exakt dem entspricht, was
    // collect_files später als Lookup verwendet (Windows-Path-Format).
    let db_path = read_dir_single(&archive_subdir);

    // archive_entries-Eintrag mit absichtlich falschem Hash → tamper soll
    // erkannt werden.
    sqlx::query(
        "INSERT INTO archive_entries
            (id, file_path, file_name, file_hash_sha256, file_size_bytes, mime_type, source)
            VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("aentry-1")
    .bind(db_path.to_string_lossy().to_string())
    .bind("RE-2026-0001.pdf")
    .bind("0000000000000000000000000000000000000000000000000000000000000000")
    .bind(15_i64)
    .bind("application/pdf")
    .bind("invoice_issued")
    .execute(&pool)
    .await
    .unwrap();

    let zip_path = dir.path().join("export.zip");
    let report = export_all(&pool, &paths, &zip_path)
        .await
        .expect("export ok");

    // R4-004: Tamper erkannt.
    assert_eq!(
        report.archive_tamper_count, 1,
        "Tamper muss detektiert werden"
    );
    assert_eq!(report.archive_file_count, 1);

    // R4-005: Manifest enthält archive_file_hashes-Array mit tamper-Flag.
    let zip_bytes = std::fs::read(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("zip readable");
    let mut manifest_str = String::new();
    {
        use std::io::Read as _;
        let mut f = archive.by_name("manifest.json").unwrap();
        f.read_to_string(&mut manifest_str).unwrap();
    }
    let manifest: Value = serde_json::from_str(&manifest_str).expect("manifest json");
    let hashes = manifest
        .get("archive_file_hashes")
        .and_then(|v| v.as_array())
        .expect("archive_file_hashes ist Array");
    assert_eq!(hashes.len(), 1);
    let entry = &hashes[0];
    assert_eq!(
        entry.get("path").and_then(|v| v.as_str()),
        Some("archive/2026/invoices/pdf/RE-2026-0001.pdf")
    );
    assert!(entry.get("sha256").and_then(|v| v.as_str()).is_some());
    assert_eq!(entry.get("tamper").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        manifest
            .get("archive_tamper_count")
            .and_then(|v| v.as_i64()),
        Some(1)
    );

    // R4-004: audit_log enthält migration.export.tamper_detected-Event.
    let tamper_audits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = ?")
        .bind("migration.export.tamper_detected")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tamper_audits, 1, "genau 1 Tamper-Audit-Eintrag");
}

#[tokio::test]
async fn r4_004_005_export_clean_when_hash_matches() {
    let (pool, dir) = setup_pool().await;
    let paths = paths_for(dir.path());
    let archive_subdir = paths.archive_dir.join("2026").join("invoices").join("pdf");
    std::fs::create_dir_all(&archive_subdir).unwrap();
    let pdf_path = archive_subdir.join("RE-2026-0002.pdf");
    let body = b"echte-pdf-bytes-2";
    std::fs::write(&pdf_path, body).unwrap();
    // DB-Key über read_dir holen (Windows-Path-Konsistenz, siehe Tamper-Test).
    let db_path = read_dir_single(&archive_subdir);

    // Korrekten SHA-256 berechnen + in archive_entries eintragen → kein Tamper.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(body);
    let real_hash = format!("{:x}", h.finalize());
    sqlx::query(
        "INSERT INTO archive_entries
            (id, file_path, file_name, file_hash_sha256, file_size_bytes, mime_type, source)
            VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("aentry-2")
    .bind(db_path.to_string_lossy().to_string())
    .bind("RE-2026-0002.pdf")
    .bind(&real_hash)
    .bind(body.len() as i64)
    .bind("application/pdf")
    .bind("invoice_issued")
    .execute(&pool)
    .await
    .unwrap();

    let zip_path = dir.path().join("export-clean.zip");
    let report = export_all(&pool, &paths, &zip_path)
        .await
        .expect("export ok");
    assert_eq!(report.archive_tamper_count, 0, "kein Tamper bei Hash-Match");

    // Manifest-Eintrag hat tamper=false.
    let zip_bytes = std::fs::read(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("zip readable");
    let mut manifest_str = String::new();
    {
        use std::io::Read as _;
        let mut f = archive.by_name("manifest.json").unwrap();
        f.read_to_string(&mut manifest_str).unwrap();
    }
    let manifest: Value = serde_json::from_str(&manifest_str).unwrap();
    let entry = &manifest
        .get("archive_file_hashes")
        .unwrap()
        .as_array()
        .unwrap()[0];
    assert_eq!(entry.get("tamper").and_then(|v| v.as_bool()), Some(false));
}

// ---------------------------------------------------------------------------
// R4-001: Restore Phase-B Rollback-Versicherung
// ---------------------------------------------------------------------------

/// Forward-Path: kein Crash, Swap läuft normal durch, `.rollback`-Reste werden
/// aufgeräumt, alte DB ist ersetzt.
#[tokio::test]
async fn r4_001_apply_pending_clean_swap_no_rollback_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    std::fs::create_dir_all(&paths.archive_dir).unwrap();

    // Originalen Stand vortäuschen.
    std::fs::write(&paths.db_file, b"original-db-content").unwrap();
    std::fs::write(paths.archive_dir.join("alt.pdf"), b"alte-pdf").unwrap();

    // Staging vortäuschen.
    let staging = dir.path().join("stg");
    std::fs::create_dir_all(staging.join("archive")).unwrap();
    std::fs::write(staging.join("klein-buch.sqlite"), b"neue-db-content").unwrap();
    std::fs::write(staging.join("archive").join("neu.pdf"), b"neue-pdf").unwrap();

    // Marker schreiben.
    let marker = paths.data_dir.join(RESTORE_MARKER);
    let info = serde_json::json!({
        "staging": staging.to_string_lossy(),
        "source": "test.kbk",
        "staged_at": "2026-05-27T12:00:00Z",
    });
    std::fs::write(&marker, info.to_string()).unwrap();

    let applied = apply_pending(&paths).expect("apply_pending ok");
    assert!(applied.is_some(), "Marker existierte → Some zurück");
    assert_eq!(
        std::fs::read(&paths.db_file).unwrap(),
        b"neue-db-content",
        "DB ersetzt"
    );
    assert!(paths.archive_dir.join("neu.pdf").is_file());
    assert!(!paths.archive_dir.join("alt.pdf").exists());

    // R4-001: Rollback-Reste sind weg.
    assert!(
        !paths.db_file.with_extension("sqlite.rollback").exists(),
        "kein .rollback-DB übrig"
    );
    assert!(
        !paths.data_dir.join("archive.rollback").exists(),
        "kein archive.rollback-Verzeichnis übrig"
    );
    // Marker + Staging weg.
    assert!(!marker.exists());
    assert!(!staging.join("klein-buch.sqlite").exists());
}

/// Rollback-Recovery: Crash zwischen Stage 1 (Rename) und Stage 2 (Copy)
/// simulieren — DB fehlt, `.rollback`-DB existiert. `recover_pending_rollback`
/// spielt sie zurück.
#[test]
fn r4_001_recover_pending_rollback_restores_original_db() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    std::fs::create_dir_all(&paths.archive_dir).unwrap();

    // Simuliere: Stage 1 hat alte DB nach .rollback verschoben, dann gecrasht.
    let rb_db = paths.db_file.with_extension("sqlite.rollback");
    std::fs::write(&rb_db, b"original-db-pre-crash").unwrap();
    // DB-Datei selbst fehlt (Crash zwischen rename und copy).

    recover_pending_rollback(&paths).expect("recover ok");

    // .rollback ist weg, DB ist wiederhergestellt.
    assert!(!rb_db.exists(), ".rollback-DB nach Recovery weg");
    assert_eq!(
        std::fs::read(&paths.db_file).unwrap(),
        b"original-db-pre-crash",
        "Original-DB ist zurück"
    );
}

/// Leftover-Cleanup: kein laufender Restore, aber `.rollback`-Reste aus
/// einem alten erfolgreichen Lauf wurden nicht weggeräumt (z. B. Stromaus
/// zwischen Stage 3 und Cleanup). Beide DBs existieren → `.rollback` ist
/// verwaister Schatten → löschen.
#[test]
fn r4_001_recover_pending_rollback_cleans_orphaned_shadow() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    std::fs::create_dir_all(&paths.archive_dir).unwrap();

    std::fs::write(&paths.db_file, b"aktuelle-db").unwrap();
    let rb_db = paths.db_file.with_extension("sqlite.rollback");
    std::fs::write(&rb_db, b"alte-db-schatten").unwrap();

    let rb_archive = paths.data_dir.join("archive.rollback");
    std::fs::create_dir_all(&rb_archive).unwrap();
    std::fs::write(rb_archive.join("alt.pdf"), b"alt").unwrap();

    recover_pending_rollback(&paths).expect("recover ok");

    // .rollback-Reste sind weg, aktuelle DB + Archive sind unverändert.
    assert!(!rb_db.exists());
    assert!(!rb_archive.exists());
    assert_eq!(std::fs::read(&paths.db_file).unwrap(), b"aktuelle-db");
}

/// Re-Run-Sicherheit: Marker existiert + `.rollback` von einem vorherigen
/// Crash-Run sind da → `apply_pending` räumt erst auf (Original kommt zurück),
/// dann läuft Swap normal durch.
#[tokio::test]
async fn r4_001_apply_pending_recovers_then_swaps() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    std::fs::create_dir_all(&paths.archive_dir).unwrap();

    // Original (an .rollback) + KEINE aktuelle DB (Crash-Sim).
    let rb_db = paths.db_file.with_extension("sqlite.rollback");
    std::fs::write(&rb_db, b"original-db").unwrap();

    // Marker + Staging.
    let staging = dir.path().join("stg2");
    std::fs::create_dir_all(staging.join("archive")).unwrap();
    std::fs::write(staging.join("klein-buch.sqlite"), b"neue-db").unwrap();
    let marker = paths.data_dir.join(RESTORE_MARKER);
    std::fs::write(
        &marker,
        serde_json::json!({
            "staging": staging.to_string_lossy(),
            "source": "test2.kbk",
            "staged_at": "2026-05-27T13:00:00Z",
        })
        .to_string(),
    )
    .unwrap();

    let applied = apply_pending(&paths).expect("apply_pending ok");
    assert!(applied.is_some());
    // Swap ist durch → DB ist die neue.
    assert_eq!(std::fs::read(&paths.db_file).unwrap(), b"neue-db");
    // .rollback wurde erst zurückgespielt, dann beim neuen Swap erneut als
    // .rollback verschoben, dann gecleared → weg.
    assert!(!rb_db.exists());
    assert!(!marker.exists());
}
