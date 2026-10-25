//! R1-S2-13B (Lauf F7): Kosten mit §13b (Reverse-Charge) tragen keinen
//! USt-Betrag. Der Lieferant stellt netto in Rechnung, die Steuer schuldet der
//! Leistungsempfänger (§13b Abs. 5 und 8 UStG) und meldet sie selbst an;
//! Klein.Buch berechnet sie nicht. Geprüft am Create-Command gegen eine echte
//! Tempfile-DB: mit USt-Betrag scheitert das Erfassen ohne Schreibwirkung
//! (keine Belegnummer verbraucht), netto wird erfasst.

use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::commands::expenses;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::MIGRATOR;

async fn setup() -> (SqlitePool, Paths, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let archive_dir = data_dir.join("archive");
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::create_dir_all(&backups_dir).unwrap();
    let db_file = data_dir.join("test.sqlite");
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_file.display()))
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    let paths = Paths {
        data_dir: data_dir.clone(),
        db_file,
        archive_dir,
        backups_dir,
        inputs_dir: tmp.path().join("inputs"),
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
        resource_dir: PathBuf::from("/non/existent/resources"),
        web_dir: PathBuf::from("/non/existent/web"),
        backup_targets_dir: PathBuf::from("/backup-targets"),
    };
    (pool, paths, tmp)
}

fn args(tax: i64) -> expenses::ExpenseCreateArgs {
    serde_json::from_value(json!({
        "input": {
            "expenseDate": "2026-01-15",
            "paidDate": null,
            "paidFromAccountId": null,
            "vendorContactId": null,
            "vendorName": "Werbeplattform Beispiel Ltd.",
            "vendorInvoiceNumber": "EU-2026-17",
            "category": "marketing",
            "description": "Anzeigen Januar",
            "netAmountCents": 10_000,
            "taxAmountCents": tax,
            "grossAmountCents": 10_000 + tax,
            "currencyCode": "EUR",
            "reverseCharge13b": true,
            "notes": null
        },
        "receiptUploadId": null
    }))
    .unwrap()
}

async fn expense_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM expenses")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn reverse_charge_with_tax_amount_is_rejected_without_side_effects() {
    let (pool, paths, _tmp) = setup().await;
    let session = BackupSession::default();

    let err = expenses::create_core(
        &pool,
        &paths,
        &session,
        args(1_900),
        klein_buch_lib::clock::now_utc(),
    )
    .await
    .expect_err("§13b mit USt-Betrag muss scheitern");
    let msg = err.to_string();
    assert!(msg.contains("§13b"), "{msg}");
    assert!(msg.contains("19,00 €"), "{msg}");
    assert_eq!(expense_count(&pool).await, 0);

    // Netto erfasst: gültig, und die erste Belegnummer ist noch frei.
    let detail = expenses::create_core(
        &pool,
        &paths,
        &session,
        args(0),
        klein_buch_lib::clock::now_utc(),
    )
    .await
    .expect("§13b netto wird erfasst");
    assert_eq!(detail.expense.expense_number, "KO-2026-0001");
    assert_eq!(detail.expense.reverse_charge_13b, 1);
    assert_eq!(detail.expense.tax_amount_cents, 0);
    assert_eq!(detail.expense.gross_amount_cents, 10_000);
    assert_eq!(expense_count(&pool).await, 1);
}
