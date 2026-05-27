//! Integrationstests fuer den Drop-Folder-Sync (Block PV1-DROP).
//!
//! Deckt die Schalen-Logik in `scheduler::drop_folder` ab:
//! - Settings-Pre-Check (off / leerer Pfad / nicht existierendes Verzeichnis)
//! - Klassifikation (xml/pdf/hidden/other) ueber die volle Pipeline
//! - File-Routing: `processed/YYYY-MM/` vs. `failed/`
//! - Notification-Inbox-Eintraege bei OK und Fehler
//!
//! ZUGFeRD-PDF-Pfad braucht den Mustang-Java-Sidecar und ist daher nicht
//! Bestandteil dieser Suite — der UI-Smoke (PRD R7.4) und der bestehende
//! `einvoice_receive_test` (ZUGFeRD-Generator + KoSIT) decken den PDF-Pfad
//! manuell ab. Hier laufen ausschliesslich XML-basierte Faelle, die ohne
//! Sidecar deterministisch durchgehen.

use std::path::Path;
use std::str::FromStr;

use chrono::NaiveDate;
use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::app_settings;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::drop_folder::{classify_file, processed_subdir, DropClassification};
use klein_buch_lib::domain::invoice::{
    BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use klein_buch_lib::einvoice::generator::to_xrechnung;
use klein_buch_lib::scheduler::drop_folder::{run_sync, DropSyncReport};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

// ---------------------------------------------------------------------------
// Pure-FC: Re-Test der domain-Klassifikation (auch wenn schon inline getestet).
// Bleibt im Integrationstest, damit eine versehentliche Refaktorisierung des
// Public-API in `domain::drop_folder` sofort hier auffaellt.
// ---------------------------------------------------------------------------

#[test]
fn classifies_xml_pdf_hidden_other_via_public_api() {
    assert_eq!(classify_file("Rechnung.xml"), DropClassification::Xml);
    assert_eq!(classify_file("Beleg.PDF"), DropClassification::Pdf);
    assert_eq!(classify_file(".DS_Store"), DropClassification::IgnoreHidden);
    assert_eq!(classify_file("Thumbs.db"), DropClassification::IgnoreHidden);
    assert_eq!(
        classify_file("Rechnung.docx"),
        DropClassification::IgnoreOther
    );
}

#[test]
fn processed_subdir_uses_iso_month() {
    let d = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
    assert_eq!(processed_subdir(d), "processed/2026-05");
}

// ---------------------------------------------------------------------------
// Test-Setup: TempDir mit DB + Paths + Drop-Folder, alle FS-Joins
// komponenten-weise (Memory feedback_windows_path_separator_in_tests).
// ---------------------------------------------------------------------------

struct Ctx {
    pool: SqlitePool,
    paths: Paths,
    session: BackupSession,
    drop_root: std::path::PathBuf,
    _temp: tempfile::TempDir,
}

async fn setup() -> Ctx {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path();
    let backups = dir.join("backups");
    let archive = dir.join("archive");
    let drop_root = dir.join("drop");
    std::fs::create_dir_all(&backups).unwrap();
    std::fs::create_dir_all(&archive).unwrap();
    std::fs::create_dir_all(&drop_root).unwrap();
    let paths = Paths {
        data_dir: dir.to_path_buf(),
        db_file: dir.join("klein-buch.sqlite"),
        archive_dir: archive,
        backups_dir: backups,
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
    };

    let url = format!("sqlite://{}", paths.db_file.to_string_lossy());
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

    Ctx {
        pool,
        paths,
        session: BackupSession::default(),
        drop_root,
        _temp: temp,
    }
}

async fn enable_drop_folder(pool: &SqlitePool, drop_root: &Path) {
    app_settings::set_bool(pool, "drop_folder_enabled", true)
        .await
        .unwrap();
    app_settings::set(pool, "drop_folder_path", &drop_root.to_string_lossy())
        .await
        .unwrap();
}

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 5, 27).unwrap()
}

/// CII-XRechnung als realistische Empfangs-Eingabe (Lieferant 19 % USt).
/// Identische Vorlage wie in `einvoice_receive_test`, hier nur lokal.
fn sample_cii_xml(invoice_number: &str) -> String {
    let seller = SellerView {
        name: "Lieferant Systeme GmbH",
        street: "Industriestr. 5",
        postal_code: "10115",
        city: "Berlin",
        country_code: "DE",
        tax_number: Some("11/222/33333"),
        vat_id: Some("DE123456789"),
        email: "rechnung@lieferant.de",
        iban: None,
        bic: None,
        is_kleinunternehmer: false,
        waived_since: None,
    };
    let buyer = BuyerView {
        name: "Wildbach Computerhilfe",
        street: Some("Beispielweg 1"),
        postal_code: Some("84028"),
        city: Some("Landshut"),
        country_code: "DE",
        vat_id: None,
        email: Some("schmidm@wildbach-computerhilfe.de"),
    };
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        delivery_date: None,
        due_date: Some(NaiveDate::from_ymd_opt(2026, 5, 10).unwrap()),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Netzwerk-Switch 24-Port".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 10_000,
            tax_rate_percent: 19.0,
            tax_category_code: "S".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    to_xrechnung(invoice_number, &input, &seller, &buyer, "N/A", &[]).unwrap()
}

fn write_drop_file(drop_root: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = drop_root.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

async fn count_expenses(pool: &SqlitePool) -> i64 {
    use sqlx::Row;
    sqlx::query("SELECT COUNT(*) AS n FROM expenses")
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get::<i64, _>("n")
        .unwrap()
}

async fn count_notifications_by_rule(pool: &SqlitePool, rule_id: &str) -> i64 {
    use sqlx::Row;
    sqlx::query("SELECT COUNT(*) AS n FROM notifications WHERE rule_id = ?")
        .bind(rule_id)
        .fetch_one(pool)
        .await
        .unwrap()
        .try_get::<i64, _>("n")
        .unwrap()
}

fn assert_skipped(report: &DropSyncReport) {
    assert!(
        report.skipped_disabled,
        "report.skipped_disabled muss true sein"
    );
    assert_eq!(report.imported, 0);
    assert_eq!(report.failed, 0);
    assert_eq!(report.ignored_hidden, 0);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_sync_skipped_when_disabled() {
    let ctx = setup().await;
    // Setting steht per Migration 0029 default auf '0'/'' — kein Eingriff.
    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert_skipped(&report);
    // Nichts geschrieben.
    assert_eq!(count_expenses(&ctx.pool).await, 0);
}

#[tokio::test]
async fn run_sync_skipped_when_enabled_but_path_missing() {
    let ctx = setup().await;
    // Toggle an, aber Pfad nicht existierendes Verzeichnis.
    app_settings::set_bool(&ctx.pool, "drop_folder_enabled", true)
        .await
        .unwrap();
    app_settings::set(
        &ctx.pool,
        "drop_folder_path",
        &ctx.drop_root.join("nirgendwo").to_string_lossy(),
    )
    .await
    .unwrap();
    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert_skipped(&report);
}

#[tokio::test]
async fn run_sync_imports_valid_xrechnung_and_moves_to_processed() {
    let ctx = setup().await;
    enable_drop_folder(&ctx.pool, &ctx.drop_root).await;
    let xml = sample_cii_xml("LF-2026-0042");
    let original = write_drop_file(&ctx.drop_root, "Lieferant-2026-0042.xml", xml.as_bytes());

    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();

    assert!(!report.skipped_disabled);
    assert_eq!(report.imported, 1, "ein Beleg importiert");
    assert_eq!(report.failed, 0);
    assert_eq!(count_expenses(&ctx.pool).await, 1, "Expense in DB angelegt");

    // Original ist weg aus dem Top-Level; in processed/YYYY-MM/ wieder da.
    // Pfad komponenten-weise (Windows-Separator-Falle, Memory
    // feedback_windows_path_separator_in_tests).
    assert!(!original.exists(), "Original aus Top-Level entfernt");
    let processed = ctx
        .drop_root
        .join("processed")
        .join("2026-05")
        .join("Lieferant-2026-0042.xml");
    assert!(
        processed.exists(),
        "verarbeitete Datei in processed/2026-05/: {:?}",
        processed
    );

    // Erfolgs-Notification in Inbox geschrieben.
    let oks = count_notifications_by_rule(&ctx.pool, "rule_drop_folder_import_ok").await;
    assert_eq!(oks, 1, "eine import_ok-Notification");

    // Zweiter Lauf darf nichts mehr finden (idempotenter Sync).
    let report2 = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert_eq!(report2.imported, 0);
    assert_eq!(report2.failed, 0);
}

#[tokio::test]
async fn run_sync_routes_invalid_xml_to_failed() {
    let ctx = setup().await;
    enable_drop_folder(&ctx.pool, &ctx.drop_root).await;
    let bad = b"<not-an-xrechnung>kaputt</not-an-xrechnung>";
    let original = write_drop_file(&ctx.drop_root, "kaputt.xml", bad);

    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert!(!report.skipped_disabled);
    assert_eq!(report.imported, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(
        count_expenses(&ctx.pool).await,
        0,
        "kein Expense bei Parse-Fehler"
    );

    // Original ist weg, liegt jetzt in failed/.
    assert!(!original.exists());
    let failed_file = ctx.drop_root.join("failed").join("kaputt.xml");
    assert!(failed_file.exists(), "kaputtes XML in failed/");
    // Side-File mit Fehler-Text daneben.
    let err_file = ctx.drop_root.join("failed").join("kaputt.xml.error.txt");
    assert!(err_file.exists(), "error.txt neben kaputtem File");
    let err_text = std::fs::read_to_string(&err_file).unwrap();
    assert!(!err_text.is_empty(), "error.txt darf nicht leer sein");

    // Fehler-Notification in Inbox.
    let fails = count_notifications_by_rule(&ctx.pool, "rule_drop_folder_import_failed").await;
    assert_eq!(fails, 1);
}

#[tokio::test]
async fn run_sync_handles_hidden_files() {
    let ctx = setup().await;
    enable_drop_folder(&ctx.pool, &ctx.drop_root).await;
    write_drop_file(&ctx.drop_root, ".DS_Store", b"hidden-noise");
    write_drop_file(&ctx.drop_root, "Thumbs.db", b"hidden-noise");
    write_drop_file(&ctx.drop_root, "Rechnung.xml.tmp", b"hidden-noise");

    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert_eq!(report.imported, 0);
    assert_eq!(report.failed, 0);
    assert_eq!(report.ignored_hidden, 3, "alle drei System-Files ignoriert");

    // Versteckte Dateien bleiben am Originalort liegen.
    assert!(ctx.drop_root.join(".DS_Store").exists());
    assert!(ctx.drop_root.join("Thumbs.db").exists());
    assert!(ctx.drop_root.join("Rechnung.xml.tmp").exists());

    // Keine Notification.
    assert_eq!(
        count_notifications_by_rule(&ctx.pool, "rule_drop_folder_import_ok").await,
        0
    );
    assert_eq!(
        count_notifications_by_rule(&ctx.pool, "rule_drop_folder_import_failed").await,
        0
    );
}

#[tokio::test]
async fn run_sync_routes_unsupported_extension_to_failed() {
    let ctx = setup().await;
    enable_drop_folder(&ctx.pool, &ctx.drop_root).await;
    let original = write_drop_file(&ctx.drop_root, "anhang.docx", b"PK\x03\x04dummy");
    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert_eq!(report.imported, 0);
    assert_eq!(report.failed, 1);
    assert!(!original.exists(), "Original aus Top-Level verschoben");
    assert!(
        ctx.drop_root.join("failed").join("anhang.docx").exists(),
        ".docx in failed/"
    );
    assert_eq!(
        count_notifications_by_rule(&ctx.pool, "rule_drop_folder_import_failed").await,
        1
    );
}

#[tokio::test]
async fn run_sync_mixed_batch_counts_correctly() {
    let ctx = setup().await;
    enable_drop_folder(&ctx.pool, &ctx.drop_root).await;
    let xml = sample_cii_xml("LF-2026-0050");
    write_drop_file(&ctx.drop_root, "ok.xml", xml.as_bytes());
    write_drop_file(&ctx.drop_root, "kaputt.xml", b"<bad/>");
    write_drop_file(&ctx.drop_root, ".DS_Store", b"hidden");
    write_drop_file(&ctx.drop_root, "anhang.zip", b"dummy");

    let report = run_sync(&ctx.pool, &ctx.paths, &ctx.session, today())
        .await
        .unwrap();
    assert_eq!(report.imported, 1, "ein Erfolg");
    assert_eq!(report.failed, 2, "ein Parse-Fail + ein Unsupported");
    assert_eq!(report.ignored_hidden, 1);
    assert_eq!(count_expenses(&ctx.pool).await, 1);
}
