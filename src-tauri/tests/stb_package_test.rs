//! Lauf F5b (review-v2026:R2-S2-STB) — Integrationstest des
//! Steuerberater-Pakets über `euer_export_stb_zip`: Belege im write-once-Archiv,
//! Export, ZIP öffnen und prüfen (Wurzelordner `EUER-{Jahr}/`, LIESMICH mit
//! §19-Hinweis, Manifest-Hashes, Original-Belege, DATEV-Kopf, genau ein Audit).
//!
//! Tempfile-DB mit den echten Migrationen; nur Dokumentations-Adressen.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::archive::store::{store_bytes, ArchiveKind};
use klein_buch_lib::commands::euer as euer_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::expenses as expenses_repo;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::kleinunternehmer::HINWEIS_TEXT;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

fn paths_for(dir: &Path) -> Paths {
    let data = dir.join("data");
    for d in ["archive", "backups", "tmp"] {
        std::fs::create_dir_all(data.join(d)).unwrap();
    }
    std::fs::create_dir_all(dir.join("inputs")).unwrap();
    Paths {
        data_dir: data.clone(),
        db_file: data.join("klein-buch.sqlite"),
        archive_dir: data.join("archive"),
        backups_dir: data.join("backups"),
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
        resource_dir: std::path::PathBuf::from("/non/existent/resources"),
        web_dir: std::path::PathBuf::from("/non/existent/web"),
        backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
    }
}

async fn setup() -> (SqlitePool, Arc<AppCtx>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    let url = format!("sqlite://{}", paths.db_file.to_string_lossy());
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
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    (pool, ctx, dir)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::test]
async fn stb_zip_contains_receipts_readme_and_matching_manifest() {
    let (pool, ctx, _dir) = setup().await;
    let year = 2025;

    // Rechnung (festgeschrieben, bezahlt) mit PDF im Archiv.
    let invoice_pdf = b"%PDF-1.7 Rechnung RE-2025-0001".to_vec();
    let stored = store_bytes(
        &pool,
        &ctx.paths.archive_dir,
        year,
        ArchiveKind::InvoicePdf,
        "RE-2025-0001.pdf",
        "application/pdf",
        &invoice_pdf,
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO contacts (id, contact_type, name, email, country_code)
         VALUES ('c-1', 'customer', 'Kunde Beispiel', 'kunde@example.org', 'DE')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status,
            pdf_archive_id, locked_at)
         VALUES ('i-1', 'RE-2025-0001', 2025, 'issued', '2025-03-01', 'c-1', 'V', 'W',
            '12345', 'O', 50000, 50000, 'issued', ?,
            '2025-03-01 10:00:00')",
    )
    .bind(&stored.archive_id)
    .execute(&pool)
    .await
    .unwrap();
    // Zahlung über das Zahlungsbuch (Migration 0034).
    klein_buch_lib::db::repo::invoices::record_payment(&pool, "i-1", 50_000, "2025-03-10", None)
        .await
        .unwrap();

    // Kostenbeleg mit Original im Archiv.
    let receipt = b"%PDF-1.7 Beleg KO-2025-0001".to_vec();
    let stored_receipt = store_bytes(
        &pool,
        &ctx.paths.archive_dir,
        year,
        ArchiveKind::ExpenseOriginal,
        "KO-2025-0001-beleg.pdf",
        "application/pdf",
        &receipt,
    )
    .await
    .unwrap();
    let expense = expenses_repo::create(
        &pool,
        &ExpenseInput {
            expense_date: chrono::NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            paid_date: chrono::NaiveDate::from_ymd_opt(2025, 4, 2),
            paid_from_account_id: None,
            vendor_contact_id: None,
            vendor_name: "Bürobedarf Beispiel".into(),
            vendor_invoice_number: None,
            category: "office".into(),
            description: "Papier".into(),
            net_amount_cents: 2_000,
            tax_amount_cents: 0,
            gross_amount_cents: 2_000,
            currency_code: "EUR".into(),
            reverse_charge_13b: false,
            notes: None,
        },
        "KO-2025-0001",
        year as i64,
        Some(&stored_receipt.archive_id),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE expenses SET locked_at = datetime('now','utc') WHERE id = ? AND locked_at IS NULL",
    )
    .bind(&expense.id)
    .execute(&pool)
    .await
    .unwrap();

    let result = euer_cmd::euer_export_stb_zip(
        &ctx,
        serde_json::from_value(serde_json::json!({
            "fiscalYear": year,
            "skr": "SKR04",
            "consultantNumber": 29098,
            "clientNumber": 55003,
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    let entry = ctx
        .downloads
        .take(&result.download.download_id)
        .expect("Download");
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&entry.path).unwrap()).unwrap();
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        files.insert(f.name().to_string(), buf);
    }
    let root = format!("EUER-{year}/");
    assert!(
        files.keys().all(|n| n.starts_with(&root)),
        "{:?}",
        files.keys()
    );
    assert_eq!(files.len(), result.file_count);

    // Original-Belege bitgleich aus dem Archiv.
    assert_eq!(
        files[&format!("{root}belege/rechnungen/RE-2025-0001.pdf")],
        invoice_pdf
    );
    assert_eq!(
        files[&format!("{root}belege/kosten/KO-2025-0001.bin")],
        receipt
    );

    // LIESMICH mit §19-Hinweis und DATEV-Angaben.
    let readme = String::from_utf8(files[&format!("{root}LIESMICH.txt")].clone()).unwrap();
    assert!(readme.contains(HINWEIS_TEXT), "{readme}");
    assert!(readme.contains("Berater- und Mandantennummer stehen im Kopf"));

    // DATEV-Kopf mit Berater, Mandant und Sachkontenrahmen.
    let datev =
        String::from_utf8_lossy(&files[&format!("{root}datev-buchungsstapel.csv")]).to_string();
    let head: Vec<&str> = datev.lines().next().unwrap().split(';').collect();
    assert_eq!((head[10], head[11], head[26]), ("29098", "55003", "\"04\""));
    assert!(datev.contains(";4185;")); // §19-Erlöse SKR04

    // Manifest: jede andere Datei genau einmal, Hash stimmt.
    let manifest: serde_json::Value =
        serde_json::from_slice(&files[&format!("{root}manifest.json")]).unwrap();
    let listed = manifest["files"].as_array().unwrap();
    assert_eq!(listed.len(), files.len() - 1);
    for f in listed {
        let name = f["name"].as_str().unwrap();
        assert_eq!(
            f["sha256"].as_str().unwrap(),
            sha256_hex(&files[name]),
            "Hash von {name}"
        );
        assert_eq!(f["bytes"].as_u64().unwrap() as usize, files[name].len());
    }

    // Genau ein Audit-Eintrag, ohne Inhalte.
    let audits: Vec<String> =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'euer.stb_export'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(audits.len(), 1);
    assert!(!audits[0].contains("Papier"));

    // Ungültige Beraternummer wird abgelehnt.
    let err = euer_cmd::euer_export_datev(
        &ctx,
        serde_json::from_value(serde_json::json!({
            "fiscalYear": year,
            "skr": "SKR03",
            "consultantNumber": 12,
        }))
        .unwrap(),
    )
    .await;
    assert!(err.is_err());
}
