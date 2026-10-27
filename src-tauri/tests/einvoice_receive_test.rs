//! Integration-Tests für den E-Rechnung-Empfang (Block 11).
//!
//! Deckt die Empfangs-Pipeline auf Repo-/Core-Ebene ab (ohne Server):
//! Parsen → Mapping → Archiv (`ReceivedEinvoice`, write-once) → `expenses::create`
//! (sofort gelockt) → KoSIT-Befund persistieren. Der Command
//! `expenses_create_from_einvoice` orchestriert genau diese Schritte.
//!
//! KB-0016 (Lauf F5c): Die Pipeline wird zusätzlich mit einer UBL-Rechnung und
//! mit defekten Eingaben (abgeschnitten, Nicht-UTF-8, Gutschrift 381, fehlendes
//! Pflichtfeld, unbekannte Syntax, leer) über
//! `commands::expenses::parse_einvoice_with_paths` getrieben — derselbe Kern wie
//! Upload und Command. Alle Fixtures sind selbst erzeugt (Dokumentations-Adressen).

use chrono::NaiveDate;
use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::commands::expenses::parse_einvoice_with_paths;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::expenses;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::expense::{validate_expense, ExpenseInput};
use klein_buch_lib::domain::invoice::{
    BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use klein_buch_lib::einvoice::generator::to_xrechnung;
use klein_buch_lib::einvoice::parser::{self, Syntax};
use klein_buch_lib::einvoice::types::{
    ValidationFinding, ValidationReport, ValidationStatus, ValidationSummary,
};
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

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
}

/// Erzeugt eine CII-XRechnung (wie sie ein anderes Klein.Buch o. ä. versenden
/// würde) als realistische Empfangs-Eingabe.
fn sample_cii_xml() -> String {
    let seller = SellerView {
        name: "Lieferant Systeme GmbH",
        street: "Industriestr. 5",
        postal_code: "10115",
        city: "Berlin",
        country_code: "DE",
        tax_number: Some("11/222/33333"),
        vat_id: Some("DE123456789"),
        email: "rechnung@lieferant.example.org",
        phone: Some("+49 000 000000"),
        contact_name: None,
        iban: None,
        bic: None,
        is_kleinunternehmer: false,
        waived_since: None,
    };
    let buyer = BuyerView {
        name: "Beispiel IT-Service",
        street: Some("Beispielweg 1"),
        postal_code: Some("84028"),
        city: Some("Landshut"),
        country_code: "DE",
        vat_id: None,
        email: Some("inhaber@example.org"),
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
    to_xrechnung("LF-2026-0042", &input, &seller, &buyer, "N/A", &[]).unwrap()
}

#[tokio::test]
async fn parse_maps_to_valid_expense_input() {
    let xml = sample_cii_xml();
    let parsed = parser::parse(&xml).unwrap();
    assert_eq!(parsed.syntax, Some(Syntax::Cii));
    assert_eq!(parsed.invoice_number.as_deref(), Some("LF-2026-0042"));
    assert_eq!(
        parsed.seller_name.as_deref(),
        Some("Lieferant Systeme GmbH")
    );
    assert_eq!(parsed.seller_vat_id.as_deref(), Some("DE123456789"));
    // 100,00 € netto + 19 % = 119,00 € brutto.
    assert_eq!(parsed.net_amount_cents, Some(10_000));
    assert_eq!(parsed.tax_amount_cents, Some(1_900));
    assert_eq!(parsed.gross_amount_cents, Some(11_900));

    let input = parser::build_expense_input(&parsed, today());
    assert!(validate_expense(&input, today()).is_ok());
    assert_eq!(input.gross_amount_cents, 11_900);
    assert_eq!(input.tax_amount_cents, 1_900);
    assert_eq!(input.net_amount_cents, 10_000);
    assert!(
        input.paid_date.is_none(),
        "Empfang ist erst mit Zahlung EÜR-relevant"
    );
}

#[tokio::test]
async fn receive_pipeline_archives_original_and_persists_validation() {
    let (pool, dir) = setup_pool().await;
    let archive_root = dir.path().join("archive");
    let xml = sample_cii_xml();

    // 1) Parsen + Mapping.
    let parsed = parser::parse(&xml).unwrap();
    let input: ExpenseInput = parser::build_expense_input(&parsed, today());

    // 2) Original write-once als ReceivedEinvoice archivieren.
    let stored = store_bytes(
        &pool,
        &archive_root,
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0001-LF-2026-0042.xml",
        "application/xml",
        xml.as_bytes(),
    )
    .await
    .unwrap();
    assert!(
        stored.file_path.contains("received-einvoices"),
        "Original landet im ReceivedEinvoice-Zweig: {}",
        stored.file_path
    );

    // 3) Kosten anlegen (sofort gelockt), Beleg = das Archiv-Original.
    let row = expenses::create(
        &pool,
        &input,
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();
    assert!(row.locked_at.is_some());
    assert_eq!(
        row.receipt_archive_id.as_deref(),
        Some(stored.archive_id.as_str())
    );

    // 4) Beratenden KoSIT-Befund persistieren (post-lock erlaubt).
    let report = ValidationReport {
        status: ValidationStatus::Warning,
        error_count: 0,
        warning_count: 1,
        raw_xml: "<svrl/>".into(),
        findings: vec![ValidationFinding {
            severity: "warning".into(),
            rule_id: Some("BR-DE-15".into()),
            message: "Käufer-Referenz fehlt".into(),
            location: None,
        }],
    };
    let summary = ValidationSummary::from_report(&report);
    let json = serde_json::to_string(&summary).unwrap();
    expenses::set_einvoice_validation(&pool, &row.id, Some(summary.status_str()), Some(&json))
        .await
        .unwrap();

    // 5) Persistenz prüfen.
    let reloaded = expenses::get(&pool, &row.id).await.unwrap().unwrap();
    assert_eq!(
        reloaded.einvoice_validation_status.as_deref(),
        Some("warning")
    );
    let stored_report = reloaded.einvoice_validation_report.unwrap();
    let back: ValidationSummary = serde_json::from_str(&stored_report).unwrap();
    assert_eq!(back.status, ValidationStatus::Warning);
    assert_eq!(back.warning_count, 1);
    assert_eq!(back.findings[0].rule_id.as_deref(), Some("BR-DE-15"));
}

#[tokio::test]
async fn failed_validation_is_advisory_not_blocking() {
    // Selbst bei 'failed' bleibt das Anlegen möglich — eine eingegangene
    // Rechnung muss fürs EÜR unabhängig vom Formal-Befund erfasst werden.
    let (pool, dir) = setup_pool().await;
    let archive_root = dir.path().join("archive");
    let xml = sample_cii_xml();
    let parsed = parser::parse(&xml).unwrap();
    let input = parser::build_expense_input(&parsed, today());

    let stored = store_bytes(
        &pool,
        &archive_root,
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0001-orig.xml",
        "application/xml",
        xml.as_bytes(),
    )
    .await
    .unwrap();
    let row = expenses::create(
        &pool,
        &input,
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();

    let report = ValidationReport {
        status: ValidationStatus::Failed,
        error_count: 3,
        warning_count: 0,
        raw_xml: "<svrl/>".into(),
        findings: vec![],
    };
    let summary = ValidationSummary::from_report(&report);
    expenses::set_einvoice_validation(
        &pool,
        &row.id,
        Some(summary.status_str()),
        Some(&serde_json::to_string(&summary).unwrap()),
    )
    .await
    .unwrap();

    let reloaded = expenses::get(&pool, &row.id).await.unwrap().unwrap();
    assert_eq!(reloaded.status, "recorded");
    assert_eq!(
        reloaded.einvoice_validation_status.as_deref(),
        Some("failed")
    );
}

// ---- KB-0016: UBL und defekte Eingaben durch die Empfangs-Pipeline ---------

/// Pfade ohne Sidecar: die beratende KoSIT-Prüfung entfällt (validation = None),
/// der Import läuft trotzdem (ADR 0024).
fn paths_without_sidecar(root: &std::path::Path) -> Paths {
    Paths::from_dirs(
        root.join("data"),
        root.join("inputs"),
        root.join("keine-ressourcen"),
        root.join("web"),
    )
}

/// UBL-Rechnung (XRechnung-Syntax 2) eines Lieferanten mit 19 % USt.
const UBL_INVOICE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ubl:Invoice xmlns:ubl="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
  xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
  xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
  <cbc:CustomizationID>urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0</cbc:CustomizationID>
  <cbc:ID>UBL-2026-0815</cbc:ID>
  <cbc:IssueDate>2026-04-20</cbc:IssueDate>
  <cbc:DueDate>2026-05-20</cbc:DueDate>
  <cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>
  <cbc:DocumentCurrencyCode>EUR</cbc:DocumentCurrencyCode>
  <cbc:BuyerReference>N/A</cbc:BuyerReference>
  <cac:AccountingSupplierParty>
    <cac:Party>
      <cbc:EndpointID schemeID="EM">rechnung@lieferant.example.org</cbc:EndpointID>
      <cac:PartyTaxScheme>
        <cbc:CompanyID>DE123456789</cbc:CompanyID>
        <cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme>
      </cac:PartyTaxScheme>
      <cac:PartyLegalEntity>
        <cbc:RegistrationName>Lieferant Hosting &amp; Co. KG</cbc:RegistrationName>
      </cac:PartyLegalEntity>
    </cac:Party>
  </cac:AccountingSupplierParty>
  <cac:AccountingCustomerParty>
    <cac:Party>
      <cac:PartyLegalEntity>
        <cbc:RegistrationName>Beispiel IT-Service</cbc:RegistrationName>
      </cac:PartyLegalEntity>
    </cac:Party>
  </cac:AccountingCustomerParty>
  <cac:TaxTotal>
    <cbc:TaxAmount currencyID="EUR">3.80</cbc:TaxAmount>
    <cac:TaxSubtotal>
      <cbc:TaxableAmount currencyID="EUR">20.00</cbc:TaxableAmount>
      <cbc:TaxAmount currencyID="EUR">3.80</cbc:TaxAmount>
      <cac:TaxCategory><cbc:ID>S</cbc:ID><cbc:Percent>19</cbc:Percent></cac:TaxCategory>
    </cac:TaxSubtotal>
  </cac:TaxTotal>
  <cac:LegalMonetaryTotal>
    <cbc:LineExtensionAmount currencyID="EUR">20.00</cbc:LineExtensionAmount>
    <cbc:TaxExclusiveAmount currencyID="EUR">20.00</cbc:TaxExclusiveAmount>
    <cbc:TaxInclusiveAmount currencyID="EUR">23.80</cbc:TaxInclusiveAmount>
    <cbc:PayableAmount currencyID="EUR">23.80</cbc:PayableAmount>
  </cac:LegalMonetaryTotal>
  <cac:InvoiceLine>
    <cbc:ID>1</cbc:ID>
    <cbc:InvoicedQuantity unitCode="MON">1</cbc:InvoicedQuantity>
    <cbc:LineExtensionAmount currencyID="EUR">20.00</cbc:LineExtensionAmount>
    <cac:Item><cbc:Name>Webhosting April</cbc:Name></cac:Item>
  </cac:InvoiceLine>
</ubl:Invoice>
"#;

#[tokio::test]
async fn ubl_invoice_runs_through_receive_pipeline() {
    let (pool, dir) = setup_pool().await;
    let paths = paths_without_sidecar(dir.path());
    let result = parse_einvoice_with_paths(
        &paths,
        UBL_INVOICE.as_bytes().to_vec(),
        "eingang.xml".into(),
    )
    .await
    .expect("UBL wird eingelesen");
    assert!(!result.is_pdf);
    assert_eq!(result.source_format, "xrechnung-ubl");
    assert_eq!(result.parsed.syntax, Some(Syntax::Ubl));
    assert_eq!(
        result.parsed.invoice_number.as_deref(),
        Some("UBL-2026-0815")
    );
    assert_eq!(
        result.parsed.seller_name.as_deref(),
        Some("Lieferant Hosting & Co. KG")
    );
    let input = result.input;
    assert_eq!(input.vendor_name, "Lieferant Hosting & Co. KG");
    assert_eq!(
        input.vendor_invoice_number.as_deref(),
        Some("UBL-2026-0815")
    );
    assert_eq!(input.net_amount_cents, 2_000);
    assert_eq!(input.tax_amount_cents, 380);
    assert_eq!(input.gross_amount_cents, 2_380);
    assert_eq!(input.description, "Webhosting April");
    assert!(!input.reverse_charge_13b);
    assert!(validate_expense(&input, today()).is_ok());

    // Original write-once archivieren und als gelockte Kosten anlegen.
    let stored = store_bytes(
        &pool,
        &dir.path().join("archive"),
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0001-eingang.xml",
        "application/xml",
        UBL_INVOICE.as_bytes(),
    )
    .await
    .unwrap();
    let row = expenses::create(
        &pool,
        &input,
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();
    assert!(row.locked_at.is_some());
    assert_eq!(row.gross_amount_cents, 2_380);
}

#[tokio::test]
async fn defective_inputs_are_rejected_by_receive_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_without_sidecar(dir.path());
    let reject = |bytes: Vec<u8>| {
        let paths = paths.clone();
        async move {
            parse_einvoice_with_paths(&paths, bytes, "eingang.xml".into())
                .await
                .expect_err("defekte Eingabe muss abgelehnt werden")
                .to_string()
        }
    };

    // Abgeschnitten (CII aus dem eigenen Generator, Übertragung abgebrochen).
    let cii = sample_cii_xml();
    let cut = cii[..cii
        .find("<ram:SpecifiedTradeSettlementHeaderMonetarySummation>")
        .unwrap()]
        .to_string();
    let err = reject(cut.into_bytes()).await;
    assert!(err.contains("unvollständig"), "{err}");
    // UBL abgeschnitten.
    let ubl_cut = &UBL_INVOICE[..UBL_INVOICE.find("<cac:InvoiceLine>").unwrap()];
    let err = reject(ubl_cut.as_bytes().to_vec()).await;
    assert!(err.contains("unvollständig"), "{err}");

    // Nicht UTF-8 (ISO-8859-1 „ü“ = 0xFC).
    let mut latin1 = UBL_INVOICE
        .replace("Hosting &amp; Co.", "M\u{fc}ller")
        .into_bytes();
    let pos = latin1.windows(2).position(|w| w == "ü".as_bytes()).unwrap();
    latin1.splice(pos..pos + 2, [0xFC]);
    let err = reject(latin1).await;
    assert!(err.contains("nicht UTF-8"), "{err}");

    // Gutschrift (381) wird nicht automatisch importiert.
    let credit = UBL_INVOICE.replace(
        "<cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>",
        "<cbc:InvoiceTypeCode>381</cbc:InvoiceTypeCode>",
    );
    let err = reject(credit.into_bytes()).await;
    assert!(err.contains("381"), "{err}");

    // Pflichtfeld BT-1 fehlt.
    let no_id = UBL_INVOICE.replace("<cbc:ID>UBL-2026-0815</cbc:ID>", "");
    let err = reject(no_id.into_bytes()).await;
    assert!(err.contains("BT-1"), "{err}");

    // Fremde Währung.
    let usd = UBL_INVOICE.replace(
        "<cbc:DocumentCurrencyCode>EUR</cbc:DocumentCurrencyCode>",
        "<cbc:DocumentCurrencyCode>USD</cbc:DocumentCurrencyCode>",
    );
    let err = reject(usd.into_bytes()).await;
    assert!(err.contains("USD"), "{err}");

    // Unbekannte Syntax und leere Datei.
    let err = reject(b"<?xml version=\"1.0\"?><Bestellung/>".to_vec()).await;
    assert!(err.contains("Unbekanntes"), "{err}");
    let err = reject(Vec::new()).await;
    assert!(err.contains("leer"), "{err}");
}
