//! Integration-Tests für die ELSTER-Ausfüllhilfe (Block 14a).
//!
//! Prüft die neue Shell-Query [`db::repo::euer::depreciation_split_for_year`]
//! (AfA-Aufteilung Zeile 33 vs. 36 über die `depreciation_method` der Anlage)
//! gegen das reale Schema und baut die Ausfüllhilfe end-to-end aus echten Daten
//! (`load_inputs` → `aggregate` → `build_form` → `to_csv`).
//!
//! Rechnungen werden über das Repo angelegt und per Direkt-UPDATE gelockt
//! (umgeht die Sidecar-Pipeline), Zahlungen über `record_payment`.

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::db::repo::euer as euer_repo;
use klein_buch_lib::db::repo::invoices::{self, BuyerSnapshot, DraftCreatePayload, SellerSnapshot};
use klein_buch_lib::db::repo::{contacts, expenses};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::invoice::{
    compute_totals, InvoiceDirection, InvoiceInput, InvoiceItemInput,
};
use klein_buch_lib::euer::{aggregate, datev_csv, elster_csv};
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

async fn mk_customer(pool: &SqlitePool) -> String {
    let input = ContactInput {
        contact_type: ContactType::Customer,
        name: "Kundin GmbH".into(),
        legal_form: None,
        vat_id: None,
        tax_number: None,
        street: "Hauptstr. 1".into(),
        postal_code: "80331".into(),
        city: "München".into(),
        country_code: "DE".into(),
        email: None,
        phone: None,
        iban: None,
        bic: None,
        accepts_einvoice: false,
        notes: None,
    };
    contacts::create(pool, &input).await.unwrap().id
}

fn item(cents: i64) -> InvoiceItemInput {
    InvoiceItemInput {
        position: 1,
        description: "Leistung".into(),
        quantity: 1.0,
        unit_code: "C62".into(),
        unit_price_cents: cents,
        tax_rate_percent: 0.0,
        tax_category_code: "E".into(),
        description_title: None,
        description_markup: None,
        source_package_id: None,
        source_package_revision: None,
    }
}

async fn mk_paid_invoice(
    pool: &SqlitePool,
    contact_id: &str,
    number: &str,
    date: NaiveDate,
    cents: i64,
) {
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: date,
        delivery_date: None,
        due_date: None,
        currency_code: "EUR".into(),
        items: vec![item(cents)],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    let totals = compute_totals(&input.items);
    let payload = DraftCreatePayload {
        contact_id: contact_id.into(),
        fiscal_year: date.year() as i64,
        is_kleinunternehmer: true,
        input,
        derived_from_quote_id: None,
    };
    let seller = SellerSnapshot {
        name: "Wildbach Computerhilfe",
        street: "Weg 1",
        postal_code: "84028",
        city: "Landshut",
        tax_number: Some("123/456/7890"),
        vat_id: None,
    };
    let buyer = BuyerSnapshot {
        name: "Kundin GmbH",
        street: Some("Hauptstr. 1"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: None,
        email: None,
    };
    let row = invoices::create_draft(pool, &payload, number, &seller, &buyer, &totals)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE invoices SET locked_at = datetime('now','utc'), status = 'issued' WHERE id = ?",
    )
    .bind(&row.id)
    .execute(pool)
    .await
    .unwrap();
    invoices::record_payment(pool, &row.id, cents, &date.to_string(), None)
        .await
        .unwrap();
}

fn expense_input(category: &str, gross: i64, paid: NaiveDate) -> ExpenseInput {
    ExpenseInput {
        expense_date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        paid_date: Some(paid),
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "Lieferant".into(),
        vendor_invoice_number: None,
        category: category.into(),
        description: "Kostenposition".into(),
        net_amount_cents: gross,
        tax_amount_cents: 0,
        gross_amount_cents: gross,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    }
}

async fn add_asset(pool: &SqlitePool, id: &str, number: &str, method: &str, fy: i64) {
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, business_share_percent, book_value_cents)
         VALUES (?, ?, ?, '2026-01-01', 100000, ?, ?, 100.0, 100000)",
    )
    .bind(id)
    .bind(number)
    .bind("Wirtschaftsgut")
    .bind(fy)
    .bind(method)
    .execute(pool)
    .await
    .unwrap();
}

async fn add_depreciation(pool: &SqlitePool, id: &str, asset_id: &str, fy: i64, amount: i64) {
    sqlx::query(
        "INSERT INTO depreciation_entries (id, asset_id, fiscal_year, depreciation_amount_cents,
            months_in_year, book_value_before_cents, book_value_after_cents, is_full_writeoff)
         VALUES (?, ?, ?, ?, 12, ?, 0, 0)",
    )
    .bind(id)
    .bind(asset_id)
    .bind(fy)
    .bind(amount)
    .bind(amount)
    .execute(pool)
    .await
    .unwrap();
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ============================================================================

#[tokio::test]
async fn depreciation_split_separates_gwg_from_beweglich() {
    let (pool, _dir) = setup_pool().await;

    add_asset(&pool, "a-lin", "AV-2026-0001", "linear", 2026).await;
    add_depreciation(&pool, "d-lin", "a-lin", 2026, 30_000).await;

    add_asset(
        &pool,
        "a-comp",
        "AV-2026-0002",
        "computer_special_2021",
        2026,
    )
    .await;
    add_depreciation(&pool, "d-comp", "a-comp", 2026, 60_000).await;

    add_asset(&pool, "a-gwg", "AV-2026-0003", "gwg_sofort", 2026).await;
    add_depreciation(&pool, "d-gwg", "a-gwg", 2026, 80_000).await;

    // andere Periode → wird ignoriert
    add_asset(&pool, "a-old", "AV-2025-0001", "linear", 2025).await;
    add_depreciation(&pool, "d-old", "a-old", 2025, 99_999).await;

    let split = euer_repo::depreciation_split_for_year(&pool, 2026)
        .await
        .unwrap();
    // linear + computer_special → bewegliche Wirtschaftsgüter (Zeile 33)
    assert_eq!(split.beweglich_cents, 90_000);
    // gwg_sofort → GWG (Zeile 36)
    assert_eq!(split.gwg_cents, 80_000);
    assert_eq!(split.total_cents(), 170_000);
}

#[tokio::test]
async fn split_is_zero_without_entries() {
    let (pool, _dir) = setup_pool().await;
    let split = euer_repo::depreciation_split_for_year(&pool, 2026)
        .await
        .unwrap();
    assert_eq!(split.beweglich_cents, 0);
    assert_eq!(split.gwg_cents, 0);
}

#[tokio::test]
async fn end_to_end_form_maps_income_expenses_and_afa() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;

    // Einnahme: bezahlte Rechnung über 1.000 €.
    mk_paid_invoice(&pool, &cust, "RE-2026-0001", d(2026, 3, 1), 100_000).await;

    // Kosten: Wareneinkauf 200 € (→ Zeile 27), Büro 50 € (→ Zeile 60).
    expenses::create(
        &pool,
        &expense_input("goods", 20_000, d(2026, 4, 1)),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();
    expenses::create(
        &pool,
        &expense_input("office", 5_000, d(2026, 5, 1)),
        "KO-2026-0002",
        2026,
        None,
    )
    .await
    .unwrap();

    // AfA: linear 300 € (Zeile 33) + GWG 800 € (Zeile 36).
    add_asset(&pool, "a-lin", "AV-2026-0001", "linear", 2026).await;
    add_depreciation(&pool, "d-lin", "a-lin", 2026, 30_000).await;
    add_asset(&pool, "a-gwg", "AV-2026-0002", "gwg_sofort", 2026).await;
    add_depreciation(&pool, "d-gwg", "a-gwg", 2026, 80_000).await;

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    let split = euer_repo::depreciation_split_for_year(&pool, 2026)
        .await
        .unwrap();
    let form = elster_csv::build_form(&report, &split, true);

    let entry = |zeile: u16| {
        form.lines
            .iter()
            .find(|l| l.zeile == zeile && l.is_entry)
            .map(|l| l.amount_cents)
    };

    assert_eq!(entry(12), Some(100_000)); // §19-Einnahmen
    assert_eq!(entry(27), Some(20_000)); // Waren
    assert_eq!(entry(60), Some(5_000)); // übrige BA (Büro)
    assert_eq!(entry(33), Some(30_000)); // AfA beweglich
    assert_eq!(entry(36), Some(80_000)); // GWG
    assert_eq!(form.income_total_cents, 100_000);
    assert_eq!(form.expense_total_cents, 135_000);
    assert_eq!(form.surplus_cents, -35_000);

    // CSV-Sanity: BOM + Kopfzeile + ein bekannter Betrag in deutscher Notation.
    let csv = elster_csv::to_csv(&form);
    assert!(csv.starts_with('\u{FEFF}'));
    assert!(csv.contains("Zeile;Position;Betrag;Art"));
    assert!(csv.contains("1000,00")); // Einnahme 100.000 ct
}

// ---- Einzelaufstellung + AVEÜR (Detail-Loader) -----------------------------

async fn dispose_asset(pool: &SqlitePool, id: &str, date: &str, proceeds: i64, residual: i64) {
    sqlx::query(
        "UPDATE assets SET disposed = 1, disposal_date = ?, disposal_type = 'sale',
            disposal_proceeds_cents = ?, disposal_residual_book_value_cents = ? WHERE id = ?",
    )
    .bind(date)
    .bind(proceeds)
    .bind(residual)
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn income_detail_lists_payments_with_invoice_and_customer() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;
    mk_paid_invoice(&pool, &cust, "RE-2026-0001", d(2026, 3, 1), 100_000).await;

    let items = euer_repo::income_detail(&pool, 2026).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].invoice_number, "RE-2026-0001");
    assert_eq!(items[0].customer, "Kundin GmbH");
    assert_eq!(items[0].description, "Leistung"); // aus der Rechnungsposition
    assert_eq!(items[0].amount_cents, 100_000);
    assert_eq!(items[0].paid_date, "2026-03-01");

    // Anderes Jahr → leer.
    assert!(euer_repo::income_detail(&pool, 2025)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn expense_detail_filters_by_payment_year() {
    let (pool, _dir) = setup_pool().await;
    expenses::create(
        &pool,
        &expense_input("software", 30_000, d(2026, 4, 1)),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();

    let items = euer_repo::expense_detail(&pool, 2026).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].expense_number, "KO-2026-0001");
    assert_eq!(items[0].vendor, "Lieferant");
    assert_eq!(items[0].category, "software");
    assert_eq!(items[0].description, "Kostenposition");
    assert_eq!(items[0].gross_cents, 30_000);

    assert!(euer_repo::expense_detail(&pool, 2025)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn aveeur_items_year_afa_and_left_join_fallback() {
    let (pool, _dir) = setup_pool().await;
    add_asset(&pool, "a1", "AV-2026-0001", "linear", 2026).await;
    add_depreciation(&pool, "d1", "a1", 2026, 40_000).await; // before=40.000, after=0

    // Anschaffungsjahr: AfA gebucht → Werte aus der Buchung.
    let y26 = euer_repo::aveeur_items(&pool, 2026).await.unwrap();
    assert_eq!(y26.len(), 1);
    assert_eq!(y26[0].asset_number, "AV-2026-0001");
    assert_eq!(y26[0].afa_year_cents, 40_000);
    assert_eq!(y26[0].book_value_start_cents, 40_000);
    assert_eq!(y26[0].book_value_end_cents, 0);
    assert!(!y26[0].disposed_in_year);

    // Folgejahr: keine Buchung → AfA 0, Restwert fällt auf assets.book_value_cents.
    let y27 = euer_repo::aveeur_items(&pool, 2027).await.unwrap();
    assert_eq!(y27.len(), 1);
    assert_eq!(y27[0].afa_year_cents, 0);
}

#[tokio::test]
async fn datev_export_end_to_end() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;
    mk_paid_invoice(&pool, &cust, "RE-2026-0001", d(2026, 3, 1), 100_000).await;
    expenses::create(
        &pool,
        &expense_input("goods", 30_000, d(2026, 4, 1)),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();
    add_asset(&pool, "a1", "AV-2026-0001", "linear", 2026).await;
    add_depreciation(&pool, "d1", "a1", 2026, 40_000).await;

    let income = euer_repo::income_detail(&pool, 2026).await.unwrap();
    let storno = euer_repo::storno_detail(&pool, 2026).await.unwrap();
    let expenses_d = euer_repo::expense_detail(&pool, 2026).await.unwrap();
    let disposals = euer_repo::disposal_detail(&pool, 2026).await.unwrap();
    let assets = euer_repo::aveeur_items(&pool, 2026).await.unwrap();
    let private_movements = euer_repo::private_movement_detail(&pool, 2026)
        .await
        .unwrap();

    let bookings = datev_csv::build_bookings(
        datev_csv::Skr::Skr03,
        2026,
        &income,
        &storno,
        &expenses_d,
        &disposals,
        &assets,
        &private_movements,
    );
    // 1 Einnahme + 1 Ausgabe + 1 AfA = 3
    assert_eq!(bookings.len(), 3);

    let header = datev_csv::DatevHeader {
        fiscal_year: 2026,
        skr: datev_csv::Skr::Skr03,
        generated_at: "20260521120000000".into(),
    };
    let bytes = datev_csv::to_datev(&header, &bookings);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("\"EXTF\";700;21;"));
    assert!(text.contains("8195")); // §19-Erlöskonto
    assert!(text.contains("3200")); // Wareneingang
    assert!(text.contains("4830")); // AfA-Aufwand
    assert!(text.contains("1000,00")); // Einnahme 100.000 ct
}

#[tokio::test]
async fn disposal_detail_lists_year_disposals_with_gain() {
    let (pool, _dir) = setup_pool().await;
    add_asset(&pool, "a-sold", "AV-2026-0002", "linear", 2026).await;
    dispose_asset(&pool, "a-sold", "2026-09-01", 30_000, 10_000).await;

    let items = euer_repo::disposal_detail(&pool, 2026).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].asset_number, "AV-2026-0002");
    assert_eq!(items[0].proceeds_cents, 30_000);
    assert_eq!(items[0].residual_book_value_cents, 10_000);
    assert_eq!(items[0].gain_loss_cents, 20_000);

    // Anderes Jahr → leer.
    assert!(euer_repo::disposal_detail(&pool, 2025)
        .await
        .unwrap()
        .is_empty());
}
