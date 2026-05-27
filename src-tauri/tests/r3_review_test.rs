//! Integration-Tests für die R3-Re-Review-Fixes (v2026.5-Säule).
//!
//! Pinned-Verhalten für:
//! - **R3-001** XRechnung BT-22 `<ram:SubjectCode>REG</ram:SubjectCode>`.
//! - **R3-002** Currency-Code BT-5 case-sensitivity + Whitelist im Generator.
//! - **R3-003** Dedup des §19-Hinweises gegen Custom-Notes-Drift.
//! - **R3-005** Mustang-`combine`-Output-Validierung (leer / fehlende Magic).
//! - **R3-007** E-Rechnung-Empfang lehnt Storno-Belege (TypeCode 381) ab.
//! - **R3-008** Parser meldet fehlende EN-16931-Pflichtfelder (BT-1/-2/-5).
//! - **R3-009** Parser-Currency-Whitelist (single-currency EUR).
//!
//! R3-004 (klausel_check in Vorschau) verlangt Tauri-`AppHandle` und ist nur
//! per Host-Smoke verifizierbar — siehe Done-Report. R3-006 (UTF-8-Härtung im
//! Receive-Pfad) lebt im Tauri-Command und ist hier nicht integrationstestbar
//! ohne `AppHandle`; das pinned Verhalten wird per Host-Smoke verifiziert.

use chrono::NaiveDate;
use klein_buch_lib::domain::invoice::{
    BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use klein_buch_lib::domain::kleinunternehmer::HINWEIS_TEXT;
use klein_buch_lib::einvoice::generator::{to_xrechnung, GenerationError};
use klein_buch_lib::einvoice::mustang_bridge::validate_combine_output;
use klein_buch_lib::einvoice::parser::{self, ParseError};

fn seller_kleinunternehmer() -> SellerView<'static> {
    SellerView {
        name: "Wildbach Computerhilfe",
        street: "Wildbachstraße 2",
        postal_code: "84036",
        city: "Landshut",
        country_code: "DE",
        tax_number: Some("133/456/7890"),
        vat_id: None,
        email: "schmidm@wildbach-computerhilfe.de",
        iban: Some("DE02120300000000202051"),
        bic: Some("BYLADEM1001"),
        is_kleinunternehmer: true,
        waived_since: None,
    }
}

fn buyer_simple() -> BuyerView<'static> {
    BuyerView {
        name: "Beispielkunde GmbH",
        street: Some("Beispielweg 12"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: None,
        email: Some("einkauf@beispielkunde.example"),
    }
}

fn invoice_minimal(currency: &str, notes: Option<String>) -> InvoiceInput {
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        delivery_date: NaiveDate::from_ymd_opt(2026, 5, 1),
        due_date: NaiveDate::from_ymd_opt(2026, 5, 31),
        currency_code: currency.into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 1.0,
            unit_code: "Std.".into(),
            unit_price_cents: 10_000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

// =============================================================================
// R3-001 — BT-22 SubjectCode (EN-16931 / KoSIT-Schematron)
// =============================================================================

#[test]
fn r3_001_bt22_carries_subject_code_reg_for_kleinunternehmer() {
    let xml = to_xrechnung(
        "RE-2026-0001",
        &invoice_minimal("EUR", None),
        &seller_kleinunternehmer(),
        &buyer_simple(),
        "N/A",
        &[],
    )
    .unwrap();
    // Note mit SubjectCode REG muss exakt einmal vorkommen.
    let note_block = "<ram:IncludedNote>\n      <ram:SubjectCode>REG</ram:SubjectCode>";
    assert!(
        xml.contains(note_block),
        "BT-22 muss mit SubjectCode REG emittiert werden, gefundenes XML:\n{xml}"
    );
    // Klausel-Text muss im Content stehen.
    assert!(xml.contains(HINWEIS_TEXT));
}

// =============================================================================
// R3-002 — Currency BT-5 case-sensitive + Whitelist
// =============================================================================

#[test]
fn r3_002_currency_lowercase_is_uppercased_in_xml() {
    let xml = to_xrechnung(
        "RE-2026-0002",
        &invoice_minimal("eur", None),
        &seller_kleinunternehmer(),
        &buyer_simple(),
        "N/A",
        &[],
    )
    .unwrap();
    assert!(
        xml.contains("<ram:InvoiceCurrencyCode>EUR</ram:InvoiceCurrencyCode>"),
        "Currency muss in der XML großgeschrieben sein, XML:\n{xml}"
    );
    // Keine Lowercase-Variante im XML.
    assert!(!xml.contains("<ram:InvoiceCurrencyCode>eur</ram:InvoiceCurrencyCode>"));
}

#[test]
fn r3_002_currency_unsupported_blocks_generator() {
    let err = to_xrechnung(
        "RE-2026-0003",
        &invoice_minimal("USD", None),
        &seller_kleinunternehmer(),
        &buyer_simple(),
        "N/A",
        &[],
    )
    .unwrap_err();
    match err {
        GenerationError::CurrencyUnsupported(code) => assert_eq!(code, "USD"),
        other => panic!("Erwartet CurrencyUnsupported, bekommen: {other:?}"),
    }
}

// =============================================================================
// R3-003 — §19-Hinweis-Dedup gegen Notes-Drift
// =============================================================================

#[test]
fn r3_003_kleinunternehmer_note_dedup_when_user_repeats_clause() {
    // User trägt versehentlich denselben §19-Klausel-Text in `notes` ein.
    let xml = to_xrechnung(
        "RE-2026-0004",
        &invoice_minimal("EUR", Some(HINWEIS_TEXT.to_string())),
        &seller_kleinunternehmer(),
        &buyer_simple(),
        "N/A",
        &[],
    )
    .unwrap();
    let occurrences = xml.matches("<ram:IncludedNote>").count();
    assert_eq!(
        occurrences, 1,
        "Dedup: §19-Klausel darf nur EIN IncludedNote-Block sein, XML:\n{xml}"
    );
}

#[test]
fn r3_003_kleinunternehmer_note_keeps_distinct_custom_note() {
    let xml = to_xrechnung(
        "RE-2026-0005",
        &invoice_minimal("EUR", Some("Vielen Dank für Ihr Vertrauen.".into())),
        &seller_kleinunternehmer(),
        &buyer_simple(),
        "N/A",
        &[],
    )
    .unwrap();
    let occurrences = xml.matches("<ram:IncludedNote>").count();
    assert_eq!(
        occurrences, 2,
        "Distinkte Custom-Note muss zusätzlich neben §19-Klausel erscheinen"
    );
    assert!(xml.contains("Vielen Dank"));
}

// =============================================================================
// R3-005 — Mustang-Output-Validierung
// =============================================================================

#[test]
fn r3_005_validate_combine_output_rejects_empty_bytes() {
    let err = validate_combine_output(&[], "irrelevant log").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("leere") || msg.contains("0 Bytes"),
        "Erwartete Hinweis auf leere Datei, bekommen: {msg}"
    );
}

#[test]
fn r3_005_validate_combine_output_rejects_non_pdf_magic() {
    let err = validate_combine_output(b"<html>not a pdf</html>", "irrelevant log").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Magic-Bytes") || msg.contains("keine valide PDF"),
        "Erwartete Hinweis auf fehlende PDF-Magic, bekommen: {msg}"
    );
}

#[test]
fn r3_005_validate_combine_output_accepts_valid_pdf_prefix() {
    assert!(validate_combine_output(b"%PDF-1.7\n%binary\n...", "log ok").is_ok());
    assert!(validate_combine_output(b"%PDF-1.4 minimal", "log ok").is_ok());
}

// =============================================================================
// R3-007 — Empfangs-Parser lehnt Gutschrift-Beleg (DocumentTypeCode 381) ab
// =============================================================================

#[test]
fn r3_007_credit_note_381_is_rejected_with_clear_error() {
    // Minimales gültiges CII-XML mit TypeCode 381.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
                          xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocument>
    <ram:ID>GS-2026-7</ram:ID>
    <ram:TypeCode>381</ram:TypeCode>
    <ram:IssueDateTime>
      <udt:DateTimeString format="102">20260301</udt:DateTimeString>
    </ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeSettlement>
      <ram:InvoiceCurrencyCode>EUR</ram:InvoiceCurrencyCode>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;
    let err = parser::parse(xml).unwrap_err();
    assert_eq!(err, ParseError::CreditNoteNotSupported);
}

// =============================================================================
// R3-008 — Parser meldet fehlende EN-16931-Pflichtfelder
// =============================================================================

#[test]
fn r3_008_missing_invoice_number_is_rejected() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
                          xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocument>
    <ram:TypeCode>380</ram:TypeCode>
    <ram:IssueDateTime>
      <udt:DateTimeString format="102">20260301</udt:DateTimeString>
    </ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeSettlement>
      <ram:InvoiceCurrencyCode>EUR</ram:InvoiceCurrencyCode>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;
    let err = parser::parse(xml).unwrap_err();
    assert_eq!(err, ParseError::MissingMandatory("BT-1 (Rechnungsnummer)"));
}

#[test]
fn r3_008_missing_invoice_date_is_rejected() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100">
  <rsm:ExchangedDocument>
    <ram:ID>RE-2026-1</ram:ID>
    <ram:TypeCode>380</ram:TypeCode>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeSettlement>
      <ram:InvoiceCurrencyCode>EUR</ram:InvoiceCurrencyCode>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;
    let err = parser::parse(xml).unwrap_err();
    assert_eq!(err, ParseError::MissingMandatory("BT-2 (Rechnungsdatum)"));
}

#[test]
fn r3_008_missing_currency_is_rejected() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
                          xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocument>
    <ram:ID>RE-2026-2</ram:ID>
    <ram:TypeCode>380</ram:TypeCode>
    <ram:IssueDateTime>
      <udt:DateTimeString format="102">20260301</udt:DateTimeString>
    </ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeSettlement>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;
    let err = parser::parse(xml).unwrap_err();
    assert_eq!(err, ParseError::MissingMandatory("BT-5 (Währung)"));
}

// =============================================================================
// R3-009 — Currency-Whitelist (single-currency EUR)
// =============================================================================

#[test]
fn r3_009_non_eur_currency_in_xml_is_rejected() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
                          xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocument>
    <ram:ID>RE-2026-3</ram:ID>
    <ram:TypeCode>380</ram:TypeCode>
    <ram:IssueDateTime>
      <udt:DateTimeString format="102">20260301</udt:DateTimeString>
    </ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeSettlement>
      <ram:InvoiceCurrencyCode>USD</ram:InvoiceCurrencyCode>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;
    let err = parser::parse(xml).unwrap_err();
    match err {
        ParseError::CurrencyUnsupported(code) => assert_eq!(code, "USD"),
        other => panic!("Erwartet CurrencyUnsupported(USD), bekommen: {other:?}"),
    }
}

#[test]
fn r3_009_lowercase_eur_normalises_to_uppercase() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
                          xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
  <rsm:ExchangedDocument>
    <ram:ID>RE-2026-4</ram:ID>
    <ram:TypeCode>380</ram:TypeCode>
    <ram:IssueDateTime>
      <udt:DateTimeString format="102">20260301</udt:DateTimeString>
    </ram:IssueDateTime>
  </rsm:ExchangedDocument>
  <rsm:SupplyChainTradeTransaction>
    <ram:ApplicableHeaderTradeSettlement>
      <ram:InvoiceCurrencyCode>eur</ram:InvoiceCurrencyCode>
    </ram:ApplicableHeaderTradeSettlement>
  </rsm:SupplyChainTradeTransaction>
</rsm:CrossIndustryInvoice>"#;
    let parsed = parser::parse(xml).unwrap();
    assert_eq!(parsed.currency_code.as_deref(), Some("EUR"));
}
