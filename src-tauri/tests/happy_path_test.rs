//! Konsolidierter Happy-Path-E2E-Test (Block 17b).
//!
//! Fährt die echte Wertschöpfungskette eines §19-Kleinunternehmers durch die
//! **echten Commands/Repos** — bewusst über die volle Lock-Pipeline
//! ([`commands::invoices::run_lock_pipeline`]) statt per Direkt-UPDATE, damit
//! auch PDF-Render (Block 17a Template-Resolver), Archivierung und §19-Klausel-
//! Check mitlaufen:
//!
//!   Stammdaten (§19) → Kontakt → Rechnung anlegen → festschreiben (Lock) →
//!   bezahlen → Kosten erfassen → EÜR berechnen.
//!
//! Die Cross-Modul-Assertion ist der Kern: eine über die echte Pipeline
//! ausgestellte + bezahlte Rechnung muss als Betriebseinnahme in der EÜR
//! auftauchen, die bezahlte Kostenposition als Betriebsausgabe, und der
//! Überschuss = Einnahmen − Ausgaben (Cash-Basis, §4 Abs. 3 / §11 EStG).
//!
//! ## Lauf-Voraussetzung
//!
//! Keine. Der Sidecar (KoSIT/Mustang) ist über `KLEIN_BUCH_SIDECAR_MOCK=1`
//! gemockt; der Keychain läuft im plattform-unabhängigen Mock-Store. Läuft
//! also ohne Java/MailHog/Secret-Service in jedem `cargo test`.

use std::path::PathBuf;
use std::str::FromStr;

use chrono::NaiveDate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use klein_buch_lib::commands::invoices as inv_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::euer as euer_repo;
use klein_buch_lib::db::repo::{contacts, expenses, invoices, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::invoice::{
    compute_totals, InvoiceDirection, InvoiceInput, InvoiceItemInput,
};
use klein_buch_lib::euer::aggregate;

/// Minimales §19-Rechnungs-Template: trägt den Klausel-Marker + rendert den
/// Hinweistext, besteht also den `pdf::klausel_check` in der Lock-Pipeline.
const TEMPLATE_TYP: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
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

async fn setup_env() -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

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
    std::fs::write(templates_dir.join("default.typ"), TEMPLATE_TYP).unwrap();

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
            is_kleinunternehmer: true,
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
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
    };

    Env {
        pool,
        paths,
        _tmp: tmp,
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

#[tokio::test]
async fn happy_path_invoice_lock_payment_expense_flows_into_euer() {
    let env = setup_env().await;

    // 1. Kunde anlegen.
    let contact_id = contacts::create(
        &env.pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: None,
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: Some("empfaenger@kunde-test.de".into()),
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

    // 2. Rechnungs-Entwurf (§19: 1 Position, 1000 € netto = brutto, USt 0).
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: d(2026, 5, 20),
        delivery_date: Some(d(2026, 5, 20)),
        due_date: Some(d(2026, 6, 19)),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 100_000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
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
    let totals = compute_totals(&input.items);
    assert_eq!(totals.gross_amount_cents, 100_000, "§19: brutto == netto");
    assert_eq!(totals.tax_amount_cents, 0, "§19: keine USt");

    let number = klein_buch_lib::db::numbering::next_number(
        &env.pool,
        klein_buch_lib::domain::numbering::DocType::Invoice,
        2026,
    )
    .await
    .unwrap();

    let seller = seller_profile::get(&env.pool).await.unwrap().unwrap();
    let snapshot = invoices::SellerSnapshot {
        name: &seller.name,
        street: &seller.street,
        postal_code: &seller.postal_code,
        city: &seller.city,
        tax_number: seller.tax_number.as_deref(),
        vat_id: seller.vat_id.as_deref(),
    };
    let buyer = contacts::get(&env.pool, &contact_id)
        .await
        .unwrap()
        .unwrap();
    let buyer_snapshot = invoices::BuyerSnapshot {
        name: &buyer.name,
        street: buyer.street.as_deref(),
        postal_code: buyer.postal_code.as_deref(),
        city: buyer.city.as_deref(),
        country_code: &buyer.country_code,
        vat_id: buyer.vat_id.as_deref(),
        email: buyer.email.as_deref(),
    };
    let payload = invoices::DraftCreatePayload {
        contact_id: contact_id.clone(),
        fiscal_year: 2026,
        is_kleinunternehmer: true,
        input,
        derived_from_quote_id: None,
    };
    let draft = invoices::create_draft(
        &env.pool,
        &payload,
        &number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await
    .unwrap();

    // 3. Festschreiben über die ECHTE Pipeline (Mock-Sidecar): Validierung →
    //    XRechnung → §19-Klausel-Check → Typst-Render (Block-17a-Resolver) →
    //    ZUGFeRD (mock) → Archiv. Muss grün durchlaufen.
    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &draft.id, "N/A")
        .await
        .expect("Lock-Pipeline (Mock-Sidecar) muss grün laufen");

    let issued = invoices::get(&env.pool, &draft.id).await.unwrap().unwrap();
    assert_eq!(issued.status, "issued", "nach Lock: Status issued");
    assert!(issued.locked_at.is_some(), "nach Lock: locked_at gesetzt");
    assert!(
        issued.pdf_archive_id.is_some(),
        "nach Lock: PDF muss archiviert sein"
    );

    // 4. Vollzahlung erfassen (Zahlungseingang im GJ 2026).
    invoices::record_payment(&env.pool, &draft.id, 100_000, "2026-05-25", None)
        .await
        .unwrap();
    let paid = invoices::get(&env.pool, &draft.id).await.unwrap().unwrap();
    assert_eq!(paid.status, "paid", "nach Vollzahlung: Status paid");

    // 5. Bezahlte Kostenposition im selben GJ.
    expenses::create(
        &env.pool,
        &ExpenseInput {
            expense_date: d(2026, 4, 10),
            paid_date: Some(d(2026, 4, 10)),
            paid_from_account_id: None,
            vendor_contact_id: None,
            vendor_name: "Lieferant".into(),
            vendor_invoice_number: None,
            category: "software".into(),
            description: "Lizenz".into(),
            net_amount_cents: 30_000,
            tax_amount_cents: 0,
            gross_amount_cents: 30_000,
            currency_code: "EUR".into(),
            reverse_charge_13b: false,
            notes: None,
        },
        "KO-2026-0001",
        2026,
        None,
    )
    .await
    .unwrap();

    // 6. EÜR 2026 berechnen — Cross-Modul-Assertion: Rechnung + Kosten fließen ein.
    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    let r = aggregate::aggregate(2026, &inputs);

    assert_eq!(
        r.invoice_income_cents, 100_000,
        "bezahlte Rechnung muss als Betriebseinnahme erscheinen"
    );
    assert_eq!(r.total_income_cents, 100_000);
    assert_eq!(
        r.expenses_total_cents, 30_000,
        "bezahlte Kosten müssen als Betriebsausgabe erscheinen"
    );
    assert_eq!(r.total_expenses_cents, 30_000);
    assert_eq!(
        r.surplus_cents, 70_000,
        "Überschuss = Einnahmen − Ausgaben (Cash-Basis)"
    );
}
