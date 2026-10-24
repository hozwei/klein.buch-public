//! Plan W11: echte Sidecar-Smoke-Tests (KoSIT-Validator und Mustang) gegen
//! `src-tauri/sidecar` — ohne Mock. Die Java-Laufzeit kommt lokal aus
//! `sidecar/jre` (per `jlink` gebaut) bzw. `KLEIN_BUCH_JAVA`, im Image aus
//! `JAVA_HOME`. Aufruf (ohne `KLEIN_BUCH_SIDECAR_MOCK`):
//! `cargo test --test sidecar_real_test -- --ignored sidecar_real`
//! Andere Sidecar-Ordner über `KLEIN_BUCH_SIDECAR_DIR`.

use std::path::PathBuf;

use chrono::NaiveDate;

use klein_buch_lib::domain::invoice::{
    BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use klein_buch_lib::einvoice::types::ValidationStatus;
use klein_buch_lib::einvoice::{generator, mustang_bridge, validator};
use klein_buch_lib::pdf::typst_render;

fn sidecar_dir() -> PathBuf {
    assert!(
        std::env::var("KLEIN_BUCH_SIDECAR_MOCK").is_err(),
        "echte Sidecar-Tests ohne KLEIN_BUCH_SIDECAR_MOCK starten"
    );
    std::env::var_os("KLEIN_BUCH_SIDECAR_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sidecar"))
}

fn inputs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../inputs")
}

fn seller() -> SellerView<'static> {
    SellerView {
        name: "Beispiel IT-Service",
        street: "Beispielweg 1",
        postal_code: "12345",
        city: "Beispielstadt",
        country_code: "DE",
        tax_number: Some("12/345/67890"),
        vat_id: None,
        email: "rechnung@example.org",
        phone: Some("+49 30 1234567"),
        contact_name: None,
        iban: None,
        bic: None,
        is_kleinunternehmer: true,
        waived_since: None,
    }
}

fn buyer() -> BuyerView<'static> {
    BuyerView {
        name: "Beispiel Kunde GmbH",
        street: Some("Kundenweg 2"),
        postal_code: Some("54321"),
        city: Some("Musterort"),
        country_code: "DE",
        vat_id: None,
        email: Some("kunde@example.org"),
    }
}

fn invoice() -> InvoiceInput {
    let d = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: d,
        delivery_date: Some(d),
        due_date: Some(NaiveDate::from_ymd_opt(2026, 11, 7).unwrap()),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Einrichtung Notebook".into(),
            quantity: 2.0,
            unit_code: "HUR".into(),
            unit_price_cents: 4_500,
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
    }
}

fn xml() -> String {
    generator::to_xrechnung("RE-2026-0001", &invoice(), &seller(), &buyer(), "N/A", &[])
        .expect("Generator")
}

#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_kosit_accepts_generated_xrechnung() {
    let report = validator::validate(&xml(), &sidecar_dir())
        .await
        .expect("KoSIT läuft");
    assert!(
        matches!(
            report.status,
            ValidationStatus::Passed | ValidationStatus::Warning
        ),
        "{report:?}"
    );
}

#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_kosit_rejects_broken_xrechnung() {
    let broken = xml().replace(
        "<ram:TypeCode>380</ram:TypeCode>",
        "<ram:TypeCode>999</ram:TypeCode>",
    );
    let report = validator::validate(&broken, &sidecar_dir())
        .await
        .expect("KoSIT läuft");
    assert!(
        matches!(report.status, ValidationStatus::Failed),
        "{report:?}"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id.as_deref().is_some_and(|r| r.starts_with("BR-"))),
        "Ablehnung durch eine Geschäftsregel (nicht durch das Schema): {:?}",
        report.findings
    );
}

#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_mustang_combine_and_extract_pdfa3() {
    let pdf = typst_render::render_invoice_default_template(
        &inputs_dir(),
        "default",
        "RE-2026-0001",
        &invoice(),
        &seller(),
        &buyer(),
        None,
        None,
    )
    .expect("PDF");
    let xml = xml();
    let zugferd = mustang_bridge::create_zugferd(&pdf, &xml, &sidecar_dir())
        .await
        .expect("Mustang combine");
    assert!(zugferd.starts_with(b"%PDF"));
    let text = String::from_utf8_lossy(&zugferd);
    assert!(
        text.contains("pdfaid:part>3<") || text.contains("pdfaid:part=\"3\""),
        "PDF/A-3 in den XMP-Metadaten"
    );
    let extracted = mustang_bridge::extract_xml(&zugferd, &sidecar_dir())
        .await
        .expect("Mustang extract");
    assert!(extracted.contains("RE-2026-0001"), "{extracted}");
    assert!(extracted.contains("Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."));
}
