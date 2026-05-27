//! Integration-Tests für die Block-3b-Lock-Pipeline.
//!
//! Verifiziert die End-to-End-Sequenz `create_draft → run_lock_pipeline`
//! gegen eine echte SQLite-DB (mit allen GoBD-Triggern) und gegen einen
//! gemockten Sidecar (`KLEIN_BUCH_SIDECAR_MOCK=1`). Tests ohne Java/KoSIT
//! lauffähig, daher CI-tauglich.
//!
//! Coverage:
//! - Lock-Pipeline läuft grün durch, schreibt PDF + XML ins Archive,
//!   setzt `locked_at`, `status='issued'`, `validation_status='passed'`.
//! - Doppel-Lock wird abgelehnt.
//! - record_payment: partiell → `partially_paid`, voll → `paid`.
//! - GoBD-Trigger blockt Update auf Kernfelder nach Lock.
//! - Archive-Tamper-Detection.

use chrono::NaiveDate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

use klein_buch_lib::archive;
use klein_buch_lib::commands::invoices as inv_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, invoices, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{self, InvoiceDirection, InvoiceInput, InvoiceItemInput};

const TEMPLATE_TEST: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
// Minimal-Template für Integration-Tests. Echte Production-Template
// liegt unter inputs/pdf-templates/default.typ.
#set page(paper: "a4")
#let data = json.decode(sys.inputs.at("data-json"))
= Rechnung #data.invoice.number
Empfänger: #data.buyer.name \
#data.invoice.gross_amount
#data.kleinunternehmer.hinweis_text
"#;

struct Env {
    pool: SqlitePool,
    paths: Paths,
    _tmp: tempfile::TempDir,
}

async fn setup_env(is_kleinunternehmer: bool) -> Env {
    // Mock-Sidecar — validator + mustang liefern Passed / pass-through.
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let archive_dir = data_dir.join("archive");
    let backups_dir = data_dir.join("backups");
    let inputs_dir = tmp.path().join("inputs");
    let templates_dir = inputs_dir.join("pdf-templates");
    let db_file = data_dir.join("test.sqlite");

    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::create_dir_all(&backups_dir).unwrap();
    std::fs::create_dir_all(&templates_dir).unwrap();
    std::fs::write(templates_dir.join("default.typ"), TEMPLATE_TEST).unwrap();

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
    MIGRATOR.run(&pool).await.unwrap();

    // Seller-Profile anlegen
    seller_profile::upsert(
        &pool,
        &seller_profile::SellerProfileInput {
            name: "Wildbach Computerhilfe".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "schmidm@wildbach-computerhilfe.de".into(),
            phone: None,
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
            confirm_waive_paragraph_19: Some(true),
        },
    )
    .await
    .unwrap();

    let paths = Paths {
        data_dir,
        db_file,
        archive_dir,
        backups_dir,
        inputs_dir,
        sidecar_dir: PathBuf::from("/non/existent/sidecar"), // Mock-Mode ignoriert das
    };

    Env {
        pool,
        paths,
        _tmp: tmp,
    }
}

async fn create_test_contact(pool: &SqlitePool) -> String {
    let c = contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: Some("DE111111111".into()),
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: Some("info@kunde.de".into()),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        },
    )
    .await
    .unwrap();
    c.id
}

fn good_input() -> InvoiceInput {
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
        delivery_date: Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap()),
        due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()),
        currency_code: "EUR".into(),
        items: vec![
            InvoiceItemInput {
                position: 1,
                description: "Beratung 4 Std".into(),
                quantity: 4.0,
                unit_code: "HUR".into(),
                unit_price_cents: 12_500,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            },
            InvoiceItemInput {
                position: 2,
                description: "Anfahrt".into(),
                quantity: 1.0,
                unit_code: "C62".into(),
                unit_price_cents: 5_000,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            },
        ],
        notes: Some("Vielen Dank".into()),
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

async fn create_draft(env: &Env, contact_id: &str, input: InvoiceInput) -> String {
    let seller = seller_profile::get(&env.pool).await.unwrap().unwrap();
    let totals = invoice::compute_totals(&input.items);
    let number = klein_buch_lib::db::numbering::next_number(
        &env.pool,
        klein_buch_lib::domain::numbering::DocType::Invoice,
        2026,
    )
    .await
    .unwrap();
    let snapshot = invoices::SellerSnapshot {
        name: &seller.name,
        street: &seller.street,
        postal_code: &seller.postal_code,
        city: &seller.city,
        tax_number: seller.tax_number.as_deref(),
        vat_id: seller.vat_id.as_deref(),
    };
    let buyer_row = contacts::get(&env.pool, contact_id).await.unwrap().unwrap();
    let buyer_snapshot = invoices::BuyerSnapshot {
        name: &buyer_row.name,
        street: buyer_row.street.as_deref(),
        postal_code: buyer_row.postal_code.as_deref(),
        city: buyer_row.city.as_deref(),
        country_code: &buyer_row.country_code,
        vat_id: buyer_row.vat_id.as_deref(),
        email: buyer_row.email.as_deref(),
    };
    let payload = invoices::DraftCreatePayload {
        contact_id: contact_id.to_string(),
        fiscal_year: 2026,
        is_kleinunternehmer: seller.is_kleinunternehmer == 1,
        input,
        derived_from_quote_id: None,
    };
    let row = invoices::create_draft(
        &env.pool,
        &payload,
        &number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await
    .unwrap();
    row.id
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn lock_pipeline_runs_end_to_end_with_mock_sidecar() {
    let env = setup_env(true).await;
    let contact_id = create_test_contact(&env.pool).await;
    let id = create_draft(&env, &contact_id, good_input()).await;

    let resp = inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .expect("lock_pipeline mit Mock-Sidecar muss grün laufen");

    assert!(resp.invoice.locked_at.is_some());
    assert_eq!(resp.invoice.status, "issued");
    assert_eq!(resp.invoice.validation_status.as_deref(), Some("passed"));
    assert!(resp.invoice.pdf_archive_id.is_some());
    assert!(resp.invoice.xml_archive_id.is_some());

    // Beide Archive-Files müssen physisch existieren und SHA-256-Verify durchlaufen
    let pdf = archive::read_and_verify(&env.pool, &resp.pdf_archive_id)
        .await
        .expect("PDF re-hash");
    let xml = archive::read_and_verify(&env.pool, &resp.xml_archive_id)
        .await
        .expect("XML re-hash");
    assert!(!pdf.is_empty(), "PDF darf nicht leer sein");
    assert!(
        std::str::from_utf8(&xml).unwrap().contains("xrechnung_3.0"),
        "XML muss XRechnung-3.0-Customization haben"
    );
    // §19-Hardline: Klausel-Text muss wortgleich im XML stehen
    assert!(
        std::str::from_utf8(&xml)
            .unwrap()
            .contains("Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."),
        "§19-Klausel muss in BT-22 stehen"
    );

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}

#[tokio::test]
async fn double_lock_is_rejected() {
    let env = setup_env(true).await;
    let contact_id = create_test_contact(&env.pool).await;
    let id = create_draft(&env, &contact_id, good_input()).await;

    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap();
    let err = inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("bereits gelockt"));

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}

#[tokio::test]
async fn record_payment_transitions_status_partial_then_paid() {
    let env = setup_env(true).await;
    let contact_id = create_test_contact(&env.pool).await;
    let id = create_draft(&env, &contact_id, good_input()).await;
    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap();

    // Gross = 4*125 + 1*50 = 550 € = 55_000 Cent
    let part = invoices::record_payment(&env.pool, &id, 30_000, "2026-06-01", None)
        .await
        .unwrap();
    assert_eq!(part.status, "partially_paid");
    assert_eq!(part.paid_amount_cents, 30_000);

    let full = invoices::record_payment(&env.pool, &id, 25_000, "2026-06-10", None)
        .await
        .unwrap();
    assert_eq!(full.status, "paid");
    assert_eq!(full.paid_amount_cents, 55_000);

    // Überzahlung wird abgelehnt
    let err = invoices::record_payment(&env.pool, &id, 100, "2026-06-11", None)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("Überzahlung"));

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}

#[tokio::test]
async fn gobd_trigger_blocks_core_field_update_after_lock() {
    let env = setup_env(true).await;
    let contact_id = create_test_contact(&env.pool).await;
    let id = create_draft(&env, &contact_id, good_input()).await;
    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap();

    // Versuch, das net_amount_cents nach Lock zu ändern → muss am Trigger scheitern
    let r = sqlx::query("UPDATE invoices SET net_amount_cents = 0 WHERE id = ?")
        .bind(&id)
        .execute(&env.pool)
        .await;
    assert!(r.is_err(), "Trigger trg_invoices_immutable muss feuern");
    let msg = format!("{:?}", r.unwrap_err());
    assert!(
        msg.contains("locked") || msg.contains("immutable"),
        "Trigger-Meldung erwartet, got {msg}"
    );

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}

#[tokio::test]
async fn archive_tamper_after_lock_is_detected() {
    let env = setup_env(true).await;
    let contact_id = create_test_contact(&env.pool).await;
    let id = create_draft(&env, &contact_id, good_input()).await;
    let resp = inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap();

    // PDF tampern: readonly-bit weg, neue Bytes, dann re-verify
    let row = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    let pdf_archive_id = row.pdf_archive_id.unwrap();
    use sqlx::Row;
    let path_row = sqlx::query("SELECT file_path FROM archive_entries WHERE id = ?")
        .bind(&pdf_archive_id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    let path: String = path_row.try_get("file_path").unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms).unwrap();
    std::fs::write(&path, b"tampered").unwrap();

    let err = archive::read_and_verify(&env.pool, &resp.pdf_archive_id)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("tamper"));

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}

#[tokio::test]
async fn paragraph_19_violation_blocks_lock() {
    let env = setup_env(true).await; // is_kleinunternehmer = true
    let contact_id = create_test_contact(&env.pool).await;
    // Erstelle Items mit USt-Ausweis → §14c-Verstoß. Domain validate_for_issue
    // im create_draft erlaubt das (Draft kann §14/§19-Fehler haben), aber
    // lock_pipeline bricht ab.
    let mut input = good_input();
    input.items[0].tax_rate_percent = 19.0;
    input.items[0].tax_category_code = "S".into();
    let id = create_draft(&env, &contact_id, input).await;

    let err = inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Paragraph19VatViolation") || msg.contains("Validation failed"),
        "erwarte §19-Block, got: {msg}"
    );

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}

#[tokio::test]
async fn storno_creates_new_invoice_and_marks_original() {
    let env = setup_env(true).await;
    let contact_id = create_test_contact(&env.pool).await;
    let id = create_draft(&env, &contact_id, good_input()).await;
    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &id, "N/A")
        .await
        .unwrap();

    // Storno manuell (analog zum Command, ohne AppHandle):
    use klein_buch_lib::domain::storno::{
        build_storno_input, OriginalInvoiceView, OriginalItemView,
    };
    let detail = invoices::get_detail(&env.pool, &id).await.unwrap().unwrap();
    let original_view = OriginalInvoiceView {
        invoice_number: &detail.invoice.invoice_number,
        currency_code: &detail.invoice.currency_code,
        pdf_template: &detail.invoice.pdf_template,
        items: detail
            .items
            .iter()
            .map(|it| OriginalItemView {
                position: it.position as u32,
                description: &it.description,
                quantity: it.quantity,
                unit_code: &it.unit_code,
                unit_price_cents: it.unit_price_cents,
                tax_rate_percent: it.tax_rate_percent,
                tax_category_code: &it.tax_category_code,
            })
            .collect(),
    };
    // Storno-Datum: heute (Europe/Berlin) — sonst löst validate_for_issue
    // einen `InvoiceDateInFuture`-Fehler aus, wenn der Test vor dem
    // hardgecodeten 2026-05-25 ausgeführt wird.
    let today = chrono::Local::now().date_naive();
    let storno_input = build_storno_input(
        &original_view,
        detail.invoice.id.clone(),
        today,
        Some("Test-Storno".into()),
    );

    let seller = seller_profile::get(&env.pool).await.unwrap().unwrap();
    let totals = invoice::compute_totals(&storno_input.items);
    let storno_number = klein_buch_lib::db::numbering::next_number(
        &env.pool,
        klein_buch_lib::domain::numbering::DocType::StornoInvoice,
        2026,
    )
    .await
    .unwrap();
    assert!(storno_number.starts_with("ST-2026-"));

    let snapshot = invoices::SellerSnapshot {
        name: &seller.name,
        street: &seller.street,
        postal_code: &seller.postal_code,
        city: &seller.city,
        tax_number: seller.tax_number.as_deref(),
        vat_id: seller.vat_id.as_deref(),
    };
    let buyer_row = contacts::get(&env.pool, &contact_id)
        .await
        .unwrap()
        .unwrap();
    let buyer_snapshot = invoices::BuyerSnapshot {
        name: &buyer_row.name,
        street: buyer_row.street.as_deref(),
        postal_code: buyer_row.postal_code.as_deref(),
        city: buyer_row.city.as_deref(),
        country_code: &buyer_row.country_code,
        vat_id: buyer_row.vat_id.as_deref(),
        email: buyer_row.email.as_deref(),
    };
    let payload = invoices::DraftCreatePayload {
        contact_id: contact_id.clone(),
        fiscal_year: 2026,
        is_kleinunternehmer: true,
        input: storno_input,
        derived_from_quote_id: None,
    };
    let storno_draft = invoices::create_draft(
        &env.pool,
        &payload,
        &storno_number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await
    .unwrap();
    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &storno_draft.id, "N/A")
        .await
        .unwrap();
    invoices::mark_canceled(&env.pool, &id, &storno_draft.id, Some("Test-Storno"))
        .await
        .unwrap();

    // Verifikation
    let original = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    assert_eq!(original.status, "canceled");
    assert_eq!(
        original.canceled_by_storno_id.as_deref(),
        Some(storno_draft.id.as_str())
    );

    let storno = invoices::get(&env.pool, &storno_draft.id)
        .await
        .unwrap()
        .unwrap();
    assert!(storno.locked_at.is_some());
    assert_eq!(storno.is_storno_for.as_deref(), Some(id.as_str()));
    // Netto-Storno ist negativ
    assert!(storno.net_amount_cents < 0);

    // ENV-Var bleibt absichtlich gesetzt — parallele Tests teilen sich
    // "MOCK on" sicher; ein remove_var hier wäre eine Race gegen
    // gleichzeitig laufende Tests im selben Crate.
}
