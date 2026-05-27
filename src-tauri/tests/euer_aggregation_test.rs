//! Integration-Tests für die EÜR-Aggregation (Block 13).
//!
//! Prüft die echten Repo-Queries ([`db::repo::euer`]) gegen das reale Schema +
//! den Functional Core ([`euer::aggregate`]). Cash-Basis (§4 Abs. 3, §11 EStG):
//! - Einnahmen am tatsächlichen Zahlungseingang (Teilzahlungen pro Jahr),
//! - Storno-Erstattung im Jahr des Storno-Belegs (kein rückwirkender Eingriff),
//! - Kosten am Zahlungsausgang (unbezahlte/stornierte raus),
//! - AfA als Jahres-Größe, Anlagen-Veräußerung mit Erlös + Restbuchwert-Abgang,
//! - Privatbewegungen tauchen NICHT auf.
//!
//! Rechnungen werden über das Repo angelegt und per Direkt-UPDATE gelockt
//! (statt über die volle Lock-Pipeline mit KoSIT/Mustang-Sidecar), damit der
//! Test ohne Sidecar läuft. Zahlungen laufen über `record_payment` (echte
//! `payment_history_json`).

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
use klein_buch_lib::euer::aggregate;
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

/// Legt eine ausgehende Rechnung an (Repo) und lockt sie per Direkt-UPDATE
/// (umgeht die Sidecar-Pipeline). Liefert die Invoice-ID zurück.
async fn mk_issued_invoice(
    pool: &SqlitePool,
    contact_id: &str,
    number: &str,
    invoice_date: NaiveDate,
    unit_price_cents: i64,
    is_storno_for: Option<String>,
) -> String {
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date,
        delivery_date: None,
        due_date: None,
        currency_code: "EUR".into(),
        items: vec![item(unit_price_cents)],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for,
        cancel_reason: None,
    };
    let totals = compute_totals(&input.items);
    let payload = DraftCreatePayload {
        contact_id: contact_id.into(),
        fiscal_year: invoice_date.year() as i64,
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
    // Lock per Direkt-UPDATE: OLD.locked_at IS NULL ⇒ trg_invoices_immutable
    // feuert nicht. Danach erlaubt record_payment Zahlungen.
    sqlx::query(
        "UPDATE invoices SET locked_at = datetime('now','utc'), status = 'issued' WHERE id = ?",
    )
    .bind(&row.id)
    .execute(pool)
    .await
    .unwrap();
    row.id
}

fn expense_input(category: &str, gross: i64, paid_date: Option<NaiveDate>) -> ExpenseInput {
    ExpenseInput {
        expense_date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        paid_date,
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

async fn add_asset(pool: &SqlitePool, id: &str, number: &str, fy: i64) {
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, business_share_percent, book_value_cents)
         VALUES (?, ?, ?, ?, ?, ?, 'linear', 100.0, ?)",
    )
    .bind(id)
    .bind(number)
    .bind("Wirtschaftsgut")
    .bind("2026-01-01")
    .bind(100_000i64)
    .bind(fy)
    .bind(100_000i64)
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

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ============================================================================

#[tokio::test]
async fn full_year_report_sums_all_buckets() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;

    // Einnahmen: zwei voll bezahlte Rechnungen 2026.
    let a = mk_issued_invoice(&pool, &cust, "RE-2026-0001", d(2026, 1, 15), 100_000, None).await;
    invoices::record_payment(&pool, &a, 100_000, "2026-01-15", None)
        .await
        .unwrap();
    let b = mk_issued_invoice(&pool, &cust, "RE-2026-0002", d(2026, 6, 1), 50_000, None).await;
    invoices::record_payment(&pool, &b, 50_000, "2026-06-01", None)
        .await
        .unwrap();

    // Ausgaben: zwei bezahlt, eine unbezahlt (raus).
    expenses::create(
        &pool,
        &expense_input("software", 30_000, Some(d(2026, 3, 1))),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();
    expenses::create(
        &pool,
        &expense_input("office", 20_000, Some(d(2026, 4, 1))),
        "KO-2026-0002",
        2026,
        None,
    )
    .await
    .unwrap();
    expenses::create(
        &pool,
        &expense_input("travel", 99_999, None),
        "KO-2026-0003",
        2026,
        None,
    )
    .await
    .unwrap();

    // AfA 2026.
    add_asset(&pool, "asset-afa", "AV-2026-0001", 2026).await;
    add_depreciation(&pool, "dep-1", "asset-afa", 2026, 25_000).await;

    // Veräußerung 2026: Erlös 10.000, Restbuchwert 4.000 → Gewinn 6.000.
    add_asset(&pool, "asset-sold", "AV-2026-0002", 2026).await;
    dispose_asset(&pool, "asset-sold", "2026-09-01", 10_000, 4_000).await;

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let r = aggregate::aggregate(2026, &inputs);

    assert_eq!(r.invoice_income_cents, 150_000);
    assert_eq!(r.storno_refunds_cents, 0);
    assert_eq!(r.disposal_proceeds_cents, 10_000);
    assert_eq!(r.total_income_cents, 160_000);

    assert_eq!(r.expenses_total_cents, 50_000);
    assert_eq!(r.depreciation_total_cents, 25_000);
    assert_eq!(r.disposal_book_value_cents, 4_000);
    assert_eq!(r.total_expenses_cents, 79_000);

    assert_eq!(r.disposal_gain_loss_cents, 6_000);
    assert_eq!(r.surplus_cents, 81_000);

    // Größte Ausgaben-Kategorie zuerst, unbezahlte travel-Kosten fehlen.
    assert_eq!(r.expenses_by_category[0].category, "software");
    assert_eq!(r.expenses_by_category[0].amount_cents, 30_000);
    assert!(r
        .expenses_by_category
        .iter()
        .all(|c| c.category != "travel"));
}

#[tokio::test]
async fn partial_payments_land_in_their_payment_year() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;

    // Eine Rechnung über 1000 €, zwei Teilzahlungen über den Jahreswechsel.
    let inv = mk_issued_invoice(&pool, &cust, "RE-2025-0001", d(2025, 12, 1), 100_000, None).await;
    invoices::record_payment(&pool, &inv, 50_000, "2025-12-20", None)
        .await
        .unwrap();
    invoices::record_payment(&pool, &inv, 50_000, "2026-01-10", None)
        .await
        .unwrap();

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    assert_eq!(
        aggregate::aggregate(2025, &inputs).invoice_income_cents,
        50_000
    );
    assert_eq!(
        aggregate::aggregate(2026, &inputs).invoice_income_cents,
        50_000
    );
}

#[tokio::test]
async fn storno_reverses_in_storno_year_without_touching_prior_year() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;

    // Rechnung bezahlt 2026.
    let orig = mk_issued_invoice(&pool, &cust, "RE-2026-0001", d(2026, 3, 1), 100_000, None).await;
    invoices::record_payment(&pool, &orig, 100_000, "2026-03-01", None)
        .await
        .unwrap();

    // Storno-Beleg datiert 2027, Original als storniert markiert.
    let storno = mk_issued_invoice(
        &pool,
        &cust,
        "ST-2027-0001",
        d(2027, 2, 1),
        -100_000,
        Some(orig.clone()),
    )
    .await;
    invoices::mark_canceled(&pool, &orig, &storno, Some("Rückabwicklung"))
        .await
        .unwrap();

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();

    // 2026 bleibt stabil bei +1000 € (GoBD: keine Rückwirkung).
    let r26 = aggregate::aggregate(2026, &inputs);
    assert_eq!(r26.invoice_income_cents, 100_000);
    assert_eq!(r26.storno_refunds_cents, 0);
    assert_eq!(r26.total_income_cents, 100_000);

    // 2027 verbucht die Erstattung als negativen Zufluss.
    let r27 = aggregate::aggregate(2027, &inputs);
    assert_eq!(r27.invoice_income_cents, 0);
    assert_eq!(r27.storno_refunds_cents, 100_000);
    assert_eq!(r27.total_income_cents, -100_000);
    assert_eq!(r27.surplus_cents, -100_000);
}

#[tokio::test]
async fn canceled_and_unpaid_expenses_are_excluded() {
    let (pool, _dir) = setup_pool().await;

    // Bezahlt → zählt.
    expenses::create(
        &pool,
        &expense_input("software", 30_000, Some(d(2026, 2, 1))),
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();
    // Unbezahlt → raus.
    expenses::create(
        &pool,
        &expense_input("office", 5_000, None),
        "KO-2026-0002",
        2026,
        None,
    )
    .await
    .unwrap();
    // Bezahlt, dann storniert → raus.
    let canceled = expenses::create(
        &pool,
        &expense_input("goods", 7_000, Some(d(2026, 2, 5))),
        "KO-2026-0003",
        2026,
        None,
    )
    .await
    .unwrap()
    .id;
    expenses::cancel(&pool, &canceled, "Fehlbuchung")
        .await
        .unwrap();

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let r = aggregate::aggregate(2026, &inputs);
    assert_eq!(r.expenses_total_cents, 30_000);
    assert_eq!(r.expenses_by_category.len(), 1);
    assert_eq!(r.expenses_by_category[0].category, "software");
}

#[tokio::test]
async fn private_movements_never_enter_euer() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;
    let inv = mk_issued_invoice(&pool, &cust, "RE-2026-0001", d(2026, 5, 1), 40_000, None).await;
    invoices::record_payment(&pool, &inv, 40_000, "2026-05-01", None)
        .await
        .unwrap();

    // Privatentnahme direkt einfügen (EÜR-neutral).
    sqlx::query(
        "INSERT INTO private_movements (id, movement_number, fiscal_year, movement_date,
            movement_type, amount_cents, description, locked_at)
         VALUES ('pv-1', 'PV-2026-0001', 2026, '2026-05-02', 'entnahme', 999999, 'Privat', datetime('now','utc'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let inputs = euer_repo::load_inputs(&pool).await.unwrap();
    let r = aggregate::aggregate(2026, &inputs);
    // Nur die Rechnungs-Einnahme zählt; die Privatentnahme bleibt außen vor.
    assert_eq!(r.total_income_cents, 40_000);
    assert_eq!(r.total_expenses_cents, 0);
    assert_eq!(r.surplus_cents, 40_000);
}

#[tokio::test]
async fn available_years_reports_data_years_descending() {
    let (pool, _dir) = setup_pool().await;
    let cust = mk_customer(&pool).await;

    let i25 = mk_issued_invoice(&pool, &cust, "RE-2025-0001", d(2025, 4, 1), 10_000, None).await;
    invoices::record_payment(&pool, &i25, 10_000, "2025-04-01", None)
        .await
        .unwrap();
    let i26 = mk_issued_invoice(&pool, &cust, "RE-2026-0001", d(2026, 4, 1), 10_000, None).await;
    invoices::record_payment(&pool, &i26, 10_000, "2026-04-01", None)
        .await
        .unwrap();

    let years = euer_repo::available_years(&pool).await.unwrap();
    assert!(years.contains(&2025));
    assert!(years.contains(&2026));
    // absteigend sortiert
    assert!(years.windows(2).all(|w| w[0] >= w[1]));
}
