//! Plan W1.2 + W1.8: Geschäftsjahr kommt serverseitig aus dem Belegdatum, das
//! Belegdatum aus der Uhr in Europe/Berlin.
//!
//! - Feste Uhr 2026-12-31T23:30:00Z → heute in Berlin 2027-01-01 → GJ 2027,
//!   Belegnummer aus dem Kreis 2027.
//! - Je Command mit GJ vom Client (Rechnung, Angebot, Kosten, Kosten aus
//!   E-Rechnung, Privatbewegung): ein abweichendes GJ wird abgelehnt, ohne GJ
//!   kommt die Nummer aus dem Kreis des Belegdatums.
//!
//! Args werden wie vom Frontend als camelCase-JSON gebaut.

use chrono::{TimeZone, Utc};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::clock;
use klein_buch_lib::commands::{expenses, invoices, private_movements, quotes};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};

struct Env {
    pool: SqlitePool,
    paths: Paths,
    session: BackupSession,
    contact_id: String,
    _tmp: tempfile::TempDir,
}

async fn setup() -> Env {
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

    seller_profile::upsert(
        &pool,
        &seller_profile::SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "kontakt@example.org".into(),
            phone: Some("+49 000 000000".into()),
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: true,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
            confirm_waive_paragraph_19: None,
        },
    )
    .await
    .unwrap();
    let contact_id = contacts::create(
        &pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: None,
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            email: Some("kunde@example.org".into()),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        },
    )
    .await
    .unwrap()
    .id;

    let paths = Paths {
        data_dir: data_dir.clone(),
        db_file,
        archive_dir,
        backups_dir,
        inputs_dir: tmp.path().join("inputs"),
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
    };
    Env {
        pool,
        paths,
        // Gesperrt: das Auto-Backup nach dem Erfassen ist best-effort und entfällt.
        session: BackupSession::default(),
        contact_id,
        _tmp: tmp,
    }
}

fn silvester_late_evening_utc() -> clock::FixedClockGuard {
    clock::fix_now_for_tests(Utc.with_ymd_and_hms(2026, 12, 31, 23, 30, 0).unwrap())
}

fn invoice_args(
    contact_id: &str,
    date: &str,
    fiscal_year: Option<i64>,
) -> invoices::CreateDraftArgs {
    serde_json::from_value(json!({
        "contactId": contact_id,
        "fiscalYear": fiscal_year,
        "buyerReference": null,
        "input": {
            "direction": "issued",
            "invoiceDate": date,
            "deliveryDate": date,
            "dueDate": null,
            "currencyCode": "EUR",
            "items": [{
                "position": 1,
                "description": "Beratung",
                "quantity": 1.0,
                "unitCode": "HUR",
                "unitPriceCents": 10000,
                "taxRatePercent": 0.0,
                "taxCategoryCode": "E"
            }],
            "notes": null,
            "paymentNote": null,
            "pdfTemplate": "default",
            "isStornoFor": null,
            "cancelReason": null
        }
    }))
    .expect("CreateDraftArgs aus camelCase-JSON")
}

#[tokio::test]
async fn invoice_date_and_fiscal_year_follow_berlin_clock() {
    let env = setup().await;
    let _clock = silvester_late_evening_utc();
    let today = clock::today_berlin();
    assert_eq!(today.to_string(), "2027-01-01");

    let args = invoice_args(&env.contact_id, &today.to_string(), None);
    let detail = invoices::create_invoice_draft_from_input(
        &env.pool,
        &args.contact_id,
        args.fiscal_year,
        &args.input,
        None,
    )
    .await
    .unwrap();
    assert_eq!(detail.invoice.fiscal_year, 2027);
    assert_eq!(detail.invoice.invoice_number, "RE-2027-0001");
}

#[tokio::test]
async fn invoice_rejects_deviating_client_fiscal_year() {
    let env = setup().await;
    let args = invoice_args(&env.contact_id, "2027-01-01", Some(2026));
    let err = invoices::create_invoice_draft_from_input(
        &env.pool,
        &args.contact_id,
        args.fiscal_year,
        &args.input,
        None,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Geschäftsjahr 2026"), "{err}");

    let ok = invoice_args(&env.contact_id, "2027-01-01", Some(2027));
    let detail = invoices::create_invoice_draft_from_input(
        &env.pool,
        &ok.contact_id,
        ok.fiscal_year,
        &ok.input,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        detail.invoice.invoice_number, "RE-2027-0001",
        "abgelehnter Versuch verbraucht keine Nummer"
    );
}

#[tokio::test]
async fn quote_fiscal_year_from_quote_date() {
    let env = setup().await;
    let args = |fy: Option<i64>| -> quotes::CreateDraftArgs {
        serde_json::from_value(json!({
            "contactId": env.contact_id,
            "fiscalYear": fy,
            "input": {
                "quoteDate": "2027-01-01",
                "validUntil": "2027-01-31",
                "currencyCode": "EUR",
                "items": [{
                    "position": 1,
                    "description": "Einrichtung",
                    "quantity": 1.0,
                    "unitCode": "C62",
                    "unitPriceCents": 5000,
                    "taxRatePercent": 0.0,
                    "taxCategoryCode": "E"
                }],
                "notes": null,
                "pdfTemplate": "default"
            }
        }))
        .unwrap()
    };
    let err = quotes::create_draft_core(&env.pool, args(Some(2026)))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Geschäftsjahr 2026"), "{err}");
    let detail = quotes::create_draft_core(&env.pool, args(None))
        .await
        .unwrap();
    assert_eq!(detail.quote.fiscal_year, 2027);
    assert_eq!(detail.quote.quote_number, "AN-2027-0001");
}

fn expense_input(date: &str) -> serde_json::Value {
    json!({
        "expenseDate": date,
        "paidDate": null,
        "paidFromAccountId": null,
        "vendorContactId": null,
        "vendorName": "Bürobedarf Beispiel",
        "vendorInvoiceNumber": null,
        "category": "office",
        "description": "Druckerpapier",
        "netAmountCents": 1000,
        "taxAmountCents": 0,
        "grossAmountCents": 1000,
        "currencyCode": "EUR",
        "reverseCharge13b": false,
        "notes": null
    })
}

#[tokio::test]
async fn expense_fiscal_year_from_expense_date() {
    let env = setup().await;
    let _clock = silvester_late_evening_utc();
    let args = |fy: Option<i64>| -> expenses::ExpenseCreateArgs {
        serde_json::from_value(json!({
            "input": expense_input("2027-01-01"),
            "fiscalYear": fy,
            "receiptBytes": null,
            "receiptFilename": null
        }))
        .unwrap()
    };
    let err = expenses::create_core(&env.pool, &env.paths, &env.session, args(Some(2026)))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Geschäftsjahr 2026"), "{err}");
    let detail = expenses::create_core(&env.pool, &env.paths, &env.session, args(None))
        .await
        .unwrap();
    assert_eq!(detail.expense.fiscal_year, 2027);
    assert_eq!(detail.expense.expense_number, "KO-2027-0001");
}

#[tokio::test]
async fn expense_from_einvoice_fiscal_year_from_expense_date() {
    let env = setup().await;
    let _clock = silvester_late_evening_utc();
    let args = |fy: Option<i64>| -> expenses::EInvoiceCreateArgs {
        serde_json::from_value(json!({
            "input": expense_input("2027-01-01"),
            "fiscalYear": fy,
            "originalBytes": b"<Invoice/>".to_vec(),
            "originalFileName": "eingang.xml",
            "sourceFormat": "xrechnung-ubl",
            "validation": null
        }))
        .unwrap()
    };
    let err =
        expenses::create_from_einvoice_with(&env.pool, &env.paths, &env.session, args(Some(2026)))
            .await
            .unwrap_err();
    assert!(err.to_string().contains("Geschäftsjahr 2026"), "{err}");
    let detail =
        expenses::create_from_einvoice_with(&env.pool, &env.paths, &env.session, args(None))
            .await
            .unwrap();
    assert_eq!(detail.expense.fiscal_year, 2027);
    assert_eq!(detail.expense.expense_number, "KO-2027-0001");
}

#[tokio::test]
async fn private_movement_fiscal_year_from_movement_date() {
    let env = setup().await;
    let _clock = silvester_late_evening_utc();
    let args = |fy: Option<i64>| -> private_movements::PrivateMovementCreateArgs {
        serde_json::from_value(json!({
            "input": {
                "movementDate": "2027-01-01",
                "movementType": "einlage",
                "amountCents": 5000,
                "accountId": null,
                "description": "Bareinlage",
                "notes": null
            },
            "fiscalYear": fy,
            "receiptBytes": null,
            "receiptFilename": null
        }))
        .unwrap()
    };
    let err = private_movements::create_core(&env.pool, &env.paths, &env.session, args(Some(2026)))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Geschäftsjahr 2026"), "{err}");
    let row = private_movements::create_core(&env.pool, &env.paths, &env.session, args(None))
        .await
        .unwrap();
    assert_eq!(row.fiscal_year, 2027);
    assert_eq!(row.movement_number, "PV-2027-0001");
}
