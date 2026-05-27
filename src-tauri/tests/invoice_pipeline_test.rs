//! Integration-Tests für die Block-3a-Pipeline.
//!
//! Verifiziert, dass die einzelnen Module korrekt ineinandergreifen:
//! - `domain::invoice::validate_for_issue` lässt eine valide §19-Rechnung durch.
//! - `einvoice::generator::to_xrechnung` erzeugt XML mit §19-Klausel an allen
//!   Pflicht-Stellen (BT-22 + BT-120) und Code 'E'.
//! - `pdf::klausel_check::verify_for_kleinunternehmer` akzeptiert das echte
//!   `inputs/pdf-templates/default.typ`-Template.
//! - `einvoice::validator::parse_report` mappt synthetische Reports.
//! - `einvoice::mustang_bridge::create_zugferd` läuft im Mock-Modus durch.
//! - `domain::storno::build_storno_input` produziert eine valide Storno-Rechnung.

use chrono::NaiveDate;

use klein_buch_lib::domain::invoice::{
    self, BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use klein_buch_lib::domain::storno::{build_storno_input, OriginalInvoiceView, OriginalItemView};
use klein_buch_lib::einvoice::generator;
use klein_buch_lib::einvoice::mustang_bridge;
use klein_buch_lib::einvoice::types::ValidationStatus;
use klein_buch_lib::einvoice::validator;
use klein_buch_lib::pdf::klausel_check;
use klein_buch_lib::pdf::templates;

fn seller_klein() -> SellerView<'static> {
    SellerView {
        name: "Wildbach Computerhilfe",
        street: "Beispielweg 1",
        postal_code: "84028",
        city: "Landshut",
        country_code: "DE",
        tax_number: Some("123/456/78901"),
        vat_id: None,
        email: "schmidm@wildbach-computerhilfe.de",
        iban: None,
        bic: None,
        is_kleinunternehmer: true,
        waived_since: None,
    }
}

fn buyer() -> BuyerView<'static> {
    BuyerView {
        name: "Kunde GmbH",
        street: Some("Hauptstr. 7"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: Some("DE111111111"),
        email: Some("info@kunde.de"),
    }
}

fn invoice() -> InvoiceInput {
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
        delivery_date: Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap()),
        due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()),
        currency_code: "EUR".into(),
        items: vec![
            InvoiceItemInput {
                position: 1,
                description: "Beratung & Support".into(),
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
                description: "Anfahrt Landshut→München".into(),
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
        notes: Some("Vielen Dank für Ihren Auftrag".into()),
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

#[test]
fn end_to_end_klein_invoice_passes_domain_validation_and_generator() {
    let inv = invoice();
    let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
    invoice::validate_for_issue(&inv, &seller_klein(), &buyer(), today)
        .expect("§14 + §19 sollten grün sein");

    let xml = generator::to_xrechnung("RE-2026-0001", &inv, &seller_klein(), &buyer(), "N/A", &[])
        .expect("Generator");

    // CII-Format (ZUGFeRD-embeddable), nicht UBL
    assert!(xml.contains("<rsm:CrossIndustryInvoice"));
    // §19-Hardline: BT-22 Note + BT-120 Reason mit wortgleichem Text (CII)
    assert!(xml.contains(
        "<ram:Content>Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.</ram:Content>"
    ));
    assert!(xml.contains("<ram:ExemptionReason>Gemäß §19 UStG"));
    // Aggregierte Summe netto: 4 × 125 + 1 × 50 = 550 €
    assert!(xml.contains("<ram:GrandTotalAmount>550.00</ram:GrandTotalAmount>"));
    // Customization: XRechnung 3.0
    assert!(xml.contains("xrechnung_3.0"));
    // Type-Code 380 (kein Storno)
    assert!(xml.contains("<ram:TypeCode>380</ram:TypeCode>"));
}

#[test]
fn real_default_template_passes_klausel_check() {
    // CWD bei `cargo test` (integration) = src-tauri/. Das Template
    // liegt eine Ebene höher: ../inputs/pdf-templates/default.typ.
    let inputs = std::path::Path::new("..").join("inputs");
    let src = templates::load_source(&inputs, "default")
        .expect("default.typ muss in inputs/pdf-templates/ existieren");
    klausel_check::verify_for_kleinunternehmer(&src).expect(
        "default.typ muss §19-KLAUSEL-BLOCK + hinweis_text-Datenfeld haben (Block-0/1-Initial)",
    );

    let list = templates::list_templates(&inputs).expect("list_templates");
    let default = list
        .iter()
        .find(|m| m.name == "default")
        .expect("default in list");
    assert!(default.klausel_status.is_klein_compatible);
}

#[test]
fn validator_parses_synthetic_passed_report_to_passed() {
    let r = validator::parse_report(r#"<rep:report outcome="valid"/>"#);
    assert_eq!(r.status, ValidationStatus::Passed);
}

#[test]
fn validator_parses_failed_assert_to_failed_with_findings() {
    let r = validator::parse_report(
        r#"<rep:report>
            <svrl:failed-assert role="error" id="BR-DE-15" location="/Invoice">
              <svrl:text>BuyerReference fehlt</svrl:text>
            </svrl:failed-assert>
          </rep:report>"#,
    );
    assert_eq!(r.status, ValidationStatus::Failed);
    assert_eq!(r.error_count, 1);
    assert_eq!(r.findings[0].rule_id.as_deref(), Some("BR-DE-15"));
}

#[tokio::test]
async fn mustang_bridge_mock_mode_returns_input_pdf_unchanged() {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let pdf_bytes = b"%PDF-1.4\nfake".to_vec();
    let xml = "<Invoice/>";
    let out = mustang_bridge::create_zugferd(&pdf_bytes, xml, std::path::Path::new("/n/a"))
        .await
        .expect("mock");
    assert_eq!(out, pdf_bytes);
    std::env::remove_var("KLEIN_BUCH_SIDECAR_MOCK");
}

#[test]
fn storno_pattern_yields_valid_negative_invoice() {
    let original = OriginalInvoiceView {
        invoice_number: "RE-2026-0001",
        currency_code: "EUR",
        pdf_template: "default",
        items: vec![OriginalItemView {
            position: 1,
            description: "Beratung",
            quantity: 4.0,
            unit_code: "HUR",
            unit_price_cents: 12_500,
            tax_rate_percent: 0.0,
            tax_category_code: "E",
        }],
    };
    let storno = build_storno_input(
        &original,
        "uuid-orig".into(),
        NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
        Some("Falscher Empfänger".into()),
    );
    // §14-Validation muss durchgehen — Storno ist auch eine §14-Rechnung.
    invoice::validate_for_issue(
        &storno,
        &seller_klein(),
        &buyer(),
        NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
    )
    .expect("Storno-Rechnung muss §14 bestehen");

    // Generator-XML: Type-Code 384 + InvoiceReferencedDocument (CII)
    let xml = generator::to_xrechnung(
        "ST-2026-0001",
        &storno,
        &seller_klein(),
        &buyer(),
        "N/A",
        &[],
    )
    .expect("Storno-Generator");
    assert!(xml.contains("<ram:TypeCode>384</ram:TypeCode>"));
    assert!(xml.contains("<ram:InvoiceReferencedDocument>"));
    assert!(xml.contains("<ram:IssuerAssignedID>uuid-orig</ram:IssuerAssignedID>"));
    // Negativer Total — 4 × -125 € = -500 €
    assert!(xml.contains("<ram:GrandTotalAmount>-500.00</ram:GrandTotalAmount>"));
}

#[test]
fn paragraph_19_violation_blocks_issue() {
    let mut inv = invoice();
    inv.items[0].tax_rate_percent = 19.0;
    inv.items[0].tax_category_code = "S".into();
    let err = invoice::validate_for_issue(
        &inv,
        &seller_klein(),
        &buyer(),
        NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
    )
    .unwrap_err();
    assert!(err.iter().any(|e| matches!(
        e,
        invoice::InvoiceValidationError::Paragraph19VatViolation(_)
    )));
}
