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

/// Verzicht auf §19 ab `since` (1. Januar; Lauf F8d: Kosten tragen ihren Status
/// bei Leistungsbezug aus der Statushistorie, ADR 0042; ohne Eintrag gilt §19).
async fn waive_paragraph_19_since(pool: &SqlitePool, since: &str) {
    let input = klein_buch_lib::db::repo::seller_profile::SellerProfileInput {
        name: "Beispiel IT-Service".into(),
        legal_form: None,
        street: "Beispielweg 1".into(),
        postal_code: "12345".into(),
        city: "Musterstadt".into(),
        country_code: "DE".into(),
        tax_number: Some("123/456/78901".into()),
        vat_id: None,
        email: "kontakt@example.org".into(),
        phone: None,
        iban: None,
        bic: None,
        logo_filename: None,
        is_kleinunternehmer: None,
        default_pdf_template: Some("default".into()),
        default_currency: Some("EUR".into()),
    };
    klein_buch_lib::db::repo::seller_profile::upsert(pool, &input)
        .await
        .unwrap();
    use klein_buch_lib::db::repo::tax_status;
    use klein_buch_lib::domain::tax_status::{verzicht_binding_until, ChangeReason, TaxStatus};
    let since = NaiveDate::parse_from_str(since, "%Y-%m-%d").unwrap();
    tax_status::insert(
        pool,
        &tax_status::NewEntry {
            effective_from: since,
            status: TaxStatus::Regelbesteuerung,
            reason: ChangeReason::Verzicht,
            binding_until: Some(verzicht_binding_until(since)),
            trigger_document: None,
            note: None,
        },
        klein_buch_lib::clock::today_berlin(),
    )
    .await
    .unwrap();
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
) -> String {
    let id = mk_locked_invoice(
        pool,
        contact_id,
        number,
        date,
        vec![item(cents)],
        true,
        None,
    )
    .await;
    invoices::record_payment(pool, &id, cents, &date.to_string(), None)
        .await
        .unwrap();
    id
}

/// Legt eine ausgehende Rechnung über das Repo an und schreibt sie per
/// Direkt-UPDATE fest (umgeht die Sidecar-Pipeline).
async fn mk_locked_invoice(
    pool: &SqlitePool,
    contact_id: &str,
    number: &str,
    date: NaiveDate,
    items: Vec<InvoiceItemInput>,
    is_kleinunternehmer: bool,
    is_storno_for: Option<String>,
) -> String {
    let input = InvoiceInput {
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        direction: InvoiceDirection::Issued,
        invoice_date: date,
        delivery_date: None,
        due_date: None,
        currency_code: "EUR".into(),
        items,
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for,
        cancel_reason: None,
    };
    let totals = compute_totals(&input.items);
    let payload = DraftCreatePayload {
        contact_id: contact_id.into(),
        fiscal_year: date.year() as i64,
        is_kleinunternehmer,
        input,
        derived_from_quote_id: None,
    };
    let seller = SellerSnapshot {
        name: "Beispiel IT-Service",
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
    row.id
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
    let form = build_form(&pool, &report, &split, true).await;

    // Vordruck 2026: Zeile und Kennzahl.
    assert_eq!(entry(&form, 111), Some((12, 100_000))); // §19-Einnahmen
    assert_eq!(entry(&form, 100), Some((29, 20_000))); // Waren
    assert_eq!(entry(&form, 229), Some((52, 5_000))); // Arbeitsmittel (Büro)
    assert_eq!(entry(&form, 130), Some((34, 30_000))); // AfA beweglich
    assert_eq!(entry(&form, 132), Some((37, 80_000))); // GWG
    assert_eq!(form.income_total_cents, 100_000);
    assert_eq!(form.expense_total_cents, 135_000);
    assert_eq!(form.surplus_cents, -35_000);
    assert_eq!(form.surplus_cents, report.surplus_cents);

    // CSV-Sanity: BOM + Kopfzeile + ein bekannter Betrag in deutscher Notation.
    let csv = elster_csv::to_csv(&form);
    assert!(csv.starts_with('\u{FEFF}'));
    assert!(csv.contains("Zeile;Kennzahl;Position;Betrag;Art"));
    assert!(csv.contains("12;111;"));
    assert!(csv.contains("1000,00")); // Einnahme 100.000 ct
}

/// Baut die Ausfüllhilfe wie `commands::euer` aus Report und Einzelposten.
async fn build_form(
    pool: &SqlitePool,
    report: &aggregate::EuerReport,
    split: &elster_csv::AfaSplit,
    klein: bool,
) -> elster_csv::ElsterForm {
    let y = report.fiscal_year;
    let income = euer_repo::income_detail(pool, y).await.unwrap();
    let storno = euer_repo::storno_detail(pool, y).await.unwrap();
    let expenses = euer_repo::expense_detail(pool, y).await.unwrap();
    let capitalized = euer_repo::capitalized_expense_detail(pool, y)
        .await
        .unwrap();
    elster_csv::build_form(&elster_csv::ElsterInputs {
        report,
        afa: split,
        income: &income,
        storno: &storno,
        expenses: &expenses,
        capitalized: &capitalized,
        is_kleinunternehmer: klein,
        status_change: None,
    })
}

/// Eingetragene Position nach Kennzahl → (Zeile, Betrag).
fn entry(form: &elster_csv::ElsterForm, kz: u16) -> Option<(u16, i64)> {
    form.lines
        .iter()
        .find(|l| l.kennzahl == Some(kz) && l.is_entry)
        .map(|l| (l.zeile, l.amount_cents))
}

/// DATEV-Stapel eines Jahres wie `commands::euer` (alle Einzelposten).
async fn datev_bookings(pool: &SqlitePool, year: i32) -> Vec<datev_csv::DatevBooking> {
    let income = euer_repo::income_detail(pool, year).await.unwrap();
    let storno = euer_repo::storno_detail(pool, year).await.unwrap();
    let expenses = euer_repo::expense_detail(pool, year).await.unwrap();
    let capitalized = euer_repo::capitalized_expense_detail(pool, year)
        .await
        .unwrap();
    let disposals = euer_repo::disposal_detail(pool, year).await.unwrap();
    let assets = euer_repo::aveeur_items(pool, year).await.unwrap();
    let private_movements = euer_repo::private_movement_detail(pool, year)
        .await
        .unwrap();
    datev_csv::build_bookings(
        datev_csv::Skr::Skr03,
        year,
        &datev_csv::DatevInputs {
            income: &income,
            storno: &storno,
            expenses: &expenses,
            capitalized: &capitalized,
            disposals: &disposals,
            assets: &assets,
            private_movements: &private_movements,
        },
    )
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

    let bookings = datev_bookings(&pool, 2026).await;
    // 1 Einnahme + 1 Ausgabe + 1 AfA = 3
    assert_eq!(bookings.len(), 3);

    let header = datev_csv::DatevHeader {
        fiscal_year: 2026,
        skr: datev_csv::Skr::Skr03,
        generated_at: "20260521120000000".into(),
        consultant_number: 1001,
        client_number: 1,
    };
    let bytes = datev_csv::to_datev(&header, &bookings);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("\"EXTF\";700;21;"));
    assert!(text.contains(";1001;1;20260101;4;")); // ADR 0042 Nr. 15
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

// ---- Lauf F5b: Konsistenz, Storno, Regelbesteuerung, Zahlungskonten -------

fn item_taxed(cents: i64, rate: f64) -> InvoiceItemInput {
    InvoiceItemInput {
        tax_rate_percent: rate,
        tax_category_code: "S".into(),
        ..item(cents)
    }
}

fn expense_taxed(
    category: &str,
    net: i64,
    tax: i64,
    paid: NaiveDate,
    account: Option<&str>,
) -> ExpenseInput {
    ExpenseInput {
        net_amount_cents: net,
        tax_amount_cents: tax,
        gross_amount_cents: net + tax,
        paid_from_account_id: account.map(str::to_string),
        ..expense_input(category, net + tax, paid)
    }
}

async fn add_account(pool: &SqlitePool, id: &str, kind: &str) {
    sqlx::query("INSERT INTO payment_accounts (id, label, type) VALUES (?, ?, ?)")
        .bind(id)
        .bind(format!("Konto {kind}"))
        .bind(kind)
        .execute(pool)
        .await
        .unwrap();
}

/// Storno-Beleg zum Original anlegen, festschreiben und das Original als
/// storniert markieren (wie `lock_with_pair_cancel`).
async fn mk_storno(
    pool: &SqlitePool,
    cust: &str,
    original: &str,
    number: &str,
    date: NaiveDate,
    cents: i64,
) {
    let storno = mk_locked_invoice(
        pool,
        cust,
        number,
        date,
        vec![item(-cents)],
        true,
        Some(original.to_string()),
    )
    .await;
    sqlx::query(
        "UPDATE invoices SET status = 'canceled', canceled_at = datetime('now','utc'),
                canceled_by_storno_id = ?, cancel_reason = 'Rückabwicklung'
          WHERE id = ?",
    )
    .bind(&storno)
    .bind(original)
    .execute(pool)
    .await
    .unwrap();
}

/// KB-0022 / KB-0025a: Die AfA des Jahres ist in Report, Zeilen-Aufteilung,
/// Anlageverzeichnis und DATEV-Stapel dieselbe — auch mit einer im Jahr
/// veräußerten Anlage (anteilige AfA im Abgangsjahr).
#[tokio::test]
async fn afa_totals_agree_across_report_split_aveeur_and_datev() {
    let (pool, _dir) = setup_pool().await;
    add_asset(&pool, "a-lin", "AV-2026-0001", "linear", 2026).await;
    add_depreciation(&pool, "d-lin", "a-lin", 2026, 30_000).await;
    add_asset(&pool, "a-gwg", "AV-2026-0002", "gwg_sofort", 2026).await;
    add_depreciation(&pool, "d-gwg", "a-gwg", 2026, 80_000).await;
    add_asset(&pool, "a-sold", "AV-2025-0001", "linear", 2025).await;
    add_depreciation(&pool, "d-sold-25", "a-sold", 2025, 20_000).await;
    add_depreciation(&pool, "d-sold-26", "a-sold", 2026, 5_000).await;
    dispose_asset(&pool, "a-sold", "2026-03-31", 40_000, 75_000).await;

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    let split = euer_repo::depreciation_split_for_year(&pool, 2026)
        .await
        .unwrap();
    let assets = euer_repo::aveeur_items(&pool, 2026).await.unwrap();
    let aveeur_sum: i64 = assets.iter().map(|a| a.afa_year_cents).sum();
    let bookings = datev_bookings(&pool, 2026).await;
    let datev_afa: i64 = bookings
        .iter()
        .filter(|b| b.konto == 4830 || b.konto == 4855)
        .map(|b| b.amount_cents)
        .sum();

    assert_eq!(report.depreciation_total_cents, 115_000);
    assert_eq!(split.total_cents(), report.depreciation_total_cents);
    assert_eq!(aveeur_sum, report.depreciation_total_cents);
    assert_eq!(datev_afa, report.depreciation_total_cents);
    // Abgang: Restbuchwert (Buchverlust) an Anlagekonto, Erlös ohne USt.
    assert!(bookings
        .iter()
        .any(|b| b.konto == 2310 && b.gegenkonto == 420 && b.amount_cents == 75_000));
    assert!(bookings
        .iter()
        .any(|b| b.konto == 1200 && b.gegenkonto == 8800 && b.amount_cents == 40_000));

    let form = build_form(&pool, &report, &split, true).await;
    assert_eq!(form.surplus_cents, report.surplus_cents);
}

/// KB-0025b: DATEV-Storno im Folgejahr — Geldkonto im Haben an Erlöskonto, Betrag aus der
/// Erstattung, Belegfeld 2 = Original. Die Ausfüllhilfe weist den Überhang
/// als Betriebsausgabe aus (keine negative Zeile 12).
#[tokio::test]
async fn storno_in_following_year_reaches_datev_and_elster() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;
    let orig = mk_paid_invoice(&pool, &cust, "RE-2025-0001", d(2025, 11, 3), 100_000).await;
    mk_storno(&pool, &cust, &orig, "ST-2026-0001", d(2026, 2, 10), 100_000).await;

    let b = datev_bookings(&pool, 2026).await;
    assert_eq!(b.len(), 1);
    assert_eq!(
        (b[0].konto, b[0].gegenkonto, b[0].soll_haben),
        (1200, 8195, 'H')
    );
    assert_eq!(b[0].amount_cents, 100_000);
    assert_eq!(b[0].belegfeld1, "ST-2026-0001");
    assert_eq!(b[0].belegfeld2, "RE-2025-0001");

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    let form = build_form(&pool, &report, &elster_csv::AfaSplit::default(), true).await;
    assert_eq!(entry(&form, 111), None);
    assert_eq!(entry(&form, 183), Some((61, 100_000)));
    assert_eq!(form.surplus_cents, report.surplus_cents);
    assert!(form.lines.iter().all(|l| l.zeile != 18));
}

/// KB-0021 / KB-0025c: Rechnung mit Regelbesteuerung — Zeile 15 netto,
/// Zeile 17 Umsatzsteuer, Kosten netto mit Vorsteuer; DATEV auf 8400 und
/// BU-Schlüssel 9; Hinweis zur Grenze der Ausfüllhilfe.
#[tokio::test]
async fn regelbesteuerung_is_split_in_elster_and_datev() {
    let (pool, _dir) = setup_pool().await;
    waive_paragraph_19_since(&pool, "2026-01-01").await;
    let cust = mk_customer(&pool).await;
    let id = mk_locked_invoice(
        &pool,
        &cust,
        "RE-2026-0001",
        d(2026, 3, 1),
        vec![item_taxed(100_000, 19.0)],
        false,
        None,
    )
    .await;
    invoices::record_payment(&pool, &id, 119_000, "2026-03-05", None)
        .await
        .unwrap();
    expenses::create(
        &pool,
        &expense_taxed("office", 10_000, 1_900, d(2026, 4, 1), None),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    let form = build_form(&pool, &report, &elster_csv::AfaSplit::default(), false).await;
    assert_eq!(entry(&form, 111), None);
    assert_eq!(entry(&form, 112), Some((15, 100_000)));
    assert_eq!(entry(&form, 140), Some((17, 19_000)));
    assert_eq!(entry(&form, 229), Some((52, 10_000)));
    assert_eq!(entry(&form, 185), Some((58, 1_900)));
    assert_eq!(form.surplus_cents, report.surplus_cents);
    assert!(form.notes.iter().any(|n| n.starts_with("Regelbesteuerung")));
    let csv = elster_csv::to_csv(&form);
    assert!(csv.contains("# Regelbesteuerung"));

    let b = datev_bookings(&pool, 2026).await;
    assert_eq!(
        (b[0].konto, b[0].gegenkonto, b[0].amount_cents),
        (1200, 8400, 119_000)
    );
    // BU-Schlüssel am Gegenkonto (Aufwand), Konto = Bank im Haben (F5-fix).
    assert_eq!(
        (
            b[1].konto,
            b[1].gegenkonto,
            b[1].bu_schluessel.as_str(),
            b[1].soll_haben
        ),
        (1200, 4930, "9", 'H')
    );
}

/// R2-011: Zahlungskonten bestimmen das DATEV-Geldkonto (Eingang über die
/// Zahlungshistorie, Kosten, Privatbewegung, Storno-Erstattung).
#[tokio::test]
async fn datev_money_accounts_follow_payment_accounts() {
    let (pool, _dir) = setup_pool().await;
    add_account(&pool, "acc-cash", "cash").await;
    add_account(&pool, "acc-paypal", "paypal").await;
    let cust = mk_customer(&pool).await;
    let id = mk_locked_invoice(
        &pool,
        &cust,
        "RE-2026-0001",
        d(2026, 3, 1),
        vec![item(50_000)],
        true,
        None,
    )
    .await;
    invoices::record_payment_to_account(&pool, &id, 50_000, "2026-03-02", None, Some("acc-paypal"))
        .await
        .unwrap();
    mk_storno(&pool, &cust, &id, "ST-2026-0001", d(2026, 3, 20), 50_000).await;
    expenses::create(
        &pool,
        &expense_taxed("office", 2_000, 0, d(2026, 4, 1), Some("acc-cash")),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO private_movements (id, movement_number, fiscal_year, movement_date,
            movement_type, amount_cents, account_id, description)
         VALUES ('pm1', 'PV-2026-0001', 2026, '2026-05-01', 'entnahme', 10000, 'acc-cash', 'Bar')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let b = datev_bookings(&pool, 2026).await;
    let find = |nr: &str| b.iter().find(|x| x.belegfeld1 == nr).unwrap();
    assert_eq!(
        (find("RE-2026-0001").konto, find("RE-2026-0001").gegenkonto),
        (1360, 8195)
    );
    assert_eq!(
        (find("ST-2026-0001").konto, find("ST-2026-0001").gegenkonto),
        (1360, 8195)
    );
    assert_eq!(find("ST-2026-0001").soll_haben, 'H');
    assert_eq!(
        (find("KO-2026-0001").konto, find("KO-2026-0001").gegenkonto),
        (1000, 4930)
    );
    assert_eq!(
        (find("PV-2026-0001").konto, find("PV-2026-0001").gegenkonto),
        (1000, 1800)
    );
    assert_eq!(find("PV-2026-0001").soll_haben, 'H');

    // Ein unbekanntes Konto wird beim Erfassen abgelehnt.
    let other = mk_locked_invoice(
        &pool,
        &cust,
        "RE-2026-0002",
        d(2026, 3, 1),
        vec![item(100)],
        true,
        None,
    )
    .await;
    assert!(invoices::record_payment_to_account(
        &pool,
        &other,
        100,
        "2026-03-02",
        None,
        Some("nope")
    )
    .await
    .is_err());
}

/// Lauf F5b: aktivierte Kosten sind keine Betriebsausgabe (die AfA der Anlage
/// wirkt), im DATEV-Stapel stehen sie als Zugang auf dem Anlagekonto.
#[tokio::test]
async fn capitalized_expense_is_not_deducted_twice() {
    let (pool, _dir) = setup_pool().await;
    let row = expenses::create(
        &pool,
        &expense_input("hardware", 100_000, d(2026, 1, 10)),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();
    add_asset(&pool, "a-cap", "AV-2026-0001", "linear", 2026).await;
    add_depreciation(&pool, "d-cap", "a-cap", 2026, 33_333).await;
    expenses::set_capitalized_asset_id(&pool, &row.id, "a-cap")
        .await
        .unwrap();

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    assert_eq!(
        report.expenses_total_cents, 0,
        "Kosten nicht zusätzlich zur AfA"
    );
    assert_eq!(report.total_expenses_cents, 33_333);
    assert!(euer_repo::expense_detail(&pool, 2026)
        .await
        .unwrap()
        .is_empty());

    let b = datev_bookings(&pool, 2026).await;
    assert!(b
        .iter()
        .any(|x| x.konto == 1200 && x.gegenkonto == 420 && x.amount_cents == 100_000));
    assert!(b.iter().all(|x| x.gegenkonto != 4980));
}
