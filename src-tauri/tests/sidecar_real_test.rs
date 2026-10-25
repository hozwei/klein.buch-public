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

/// Lauf E: Storno-XML mit BT-25 (Nummer) und BT-26 (Rechnungsdatum) des
/// Originals besteht den echten Validator.
#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_kosit_accepts_storno_with_preceding_invoice() {
    let mut storno = invoice();
    storno.is_storno_for = Some("019a0000-0000-7000-8000-000000000001".into());
    for it in &mut storno.items {
        it.unit_price_cents = -it.unit_price_cents;
    }
    storno.due_date = None;
    let preceding = klein_buch_lib::domain::storno::PrecedingInvoice {
        number: "RE-2025-0042".into(),
        issue_date: NaiveDate::from_ymd_opt(2025, 12, 10).unwrap(),
    };
    let xml = generator::to_xrechnung_with_preceding(
        "ST-2026-0001",
        &storno,
        &seller(),
        &buyer(),
        "N/A",
        &[],
        Some(&preceding),
    )
    .expect("Generator");
    assert!(xml.contains("<qdt:DateTimeString format=\"102\">20251210</qdt:DateTimeString>"));
    let report = validator::validate(&xml, &sidecar_dir())
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

/// Lauf F5a (R1-S2-TESTS): Regelbesteuerung mit vielen Kleinpositionen und
/// zwei Sätzen. Die USt je Gruppe entsteht aus der Nettosumme (BR-CO-17), die
/// Summe gerundeter Positionssteuern wiche um Cents ab. Dazu der Storno mit
/// exakt negierten Beträgen.
#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_kosit_accepts_regelbesteuerung_with_small_items() {
    let regel = SellerView {
        is_kleinunternehmer: false,
        waived_since: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
        vat_id: Some("DE123456788"),
        ..seller()
    };
    let mut inv = invoice();
    inv.items = (1..=40)
        .map(|p| InvoiceItemInput {
            position: p,
            description: format!("Kleinteil {p}"),
            quantity: if p % 3 == 0 { 0.333 } else { 1.0 },
            unit_code: "C62".into(),
            unit_price_cents: 333 + i64::from(p),
            tax_rate_percent: if p % 2 == 0 { 19.0 } else { 7.0 },
            tax_category_code: "S".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        })
        .collect();
    let totals = klein_buch_lib::domain::invoice::compute_totals(&inv.items);
    let per_line: i64 = totals.items.iter().map(|t| t.tax_amount_cents).sum();
    assert_ne!(
        per_line, totals.tax_amount_cents,
        "Testdaten sollen den Unterschied der Rundungswege zeigen"
    );
    for (number, input) in [
        ("RE-2026-0002", inv.clone()),
        ("ST-2026-0002", {
            let mut s = inv.clone();
            s.is_storno_for = Some("019a0000-0000-7000-8000-000000000002".into());
            s.due_date = None;
            for it in &mut s.items {
                it.unit_price_cents = -it.unit_price_cents;
            }
            s
        }),
    ] {
        let preceding = klein_buch_lib::domain::storno::PrecedingInvoice {
            number: "RE-2026-0002".into(),
            issue_date: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
        };
        let xml = generator::to_xrechnung_with_preceding(
            number,
            &input,
            &regel,
            &buyer(),
            "N/A",
            &[],
            input.is_storno_for.as_ref().map(|_| &preceding),
        )
        .expect("Generator");
        let report = validator::validate(&xml, &sidecar_dir())
            .await
            .expect("KoSIT läuft");
        assert!(
            matches!(
                report.status,
                ValidationStatus::Passed | ValidationStatus::Warning
            ),
            "{number}: {report:?}"
        );
    }
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
    // R3-017 (Lauf F5c): Container passt zur XRechnung-Guideline des XML.
    assert!(
        text.contains("ConformanceLevel>XRECHNUNG<")
            || text.contains("ConformanceLevel=\"XRECHNUNG\""),
        "ZUGFeRD-Profil XRECHNUNG in den XMP-Metadaten"
    );
    assert!(text.contains("xrechnung.xml"), "Anhang xrechnung.xml");
    let extracted = mustang_bridge::extract_xml(&zugferd, &sidecar_dir())
        .await
        .expect("Mustang extract");
    assert!(extracted.contains("RE-2026-0001"), "{extracted}");
    assert!(extracted.contains("Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."));
}

// ---------------------------------------------------------------------------
// W14: Angriffe über fremde E-Rechnungen (XXE, Dekompressionsbombe)
// ---------------------------------------------------------------------------

/// Lokaler „Kanarienvogel“ auf 127.0.0.1: zählt jede eingehende Verbindung.
struct Canary {
    port: u16,
    hits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

fn canary() -> Canary {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Port");
    let port = listener.local_addr().unwrap().port();
    let hits = std::sync::Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            if conn.is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }
    });
    Canary { port, hits }
}

impl Canary {
    fn hits(&self) -> usize {
        self.hits.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// CII mit DOCTYPE: eine Entität auf `/etc/passwd`, eine auf den Kanarienvogel.
fn xxe_xml(port: u16) -> String {
    let doctype = format!(
        "<!DOCTYPE rsm:CrossIndustryInvoice [\n  <!ENTITY file SYSTEM \"file:///etc/passwd\">\n  \
         <!ENTITY net SYSTEM \"http://127.0.0.1:{port}/xxe\">\n]>\n"
    );
    xml()
        .replacen(
            "<rsm:CrossIndustryInvoice",
            &format!("{doctype}<rsm:CrossIndustryInvoice"),
            1,
        )
        .replacen(
            "<ram:Name>Beispiel IT-Service</ram:Name>",
            "<ram:Name>&file;&net;</ram:Name>",
            1,
        )
}

#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_kosit_does_not_resolve_external_entities() {
    let c = canary();
    let xml = xxe_xml(c.port);
    assert!(xml.contains("&net;"), "Testaufbau: Entität eingesetzt");
    let result = validator::validate(&xml, &sidecar_dir()).await;
    let text = format!("{result:?}");
    assert!(
        !text.contains("root:x:0:0"),
        "Dateiinhalt im Ergebnis: {text}"
    );
    if let Ok(report) = &result {
        assert!(
            !matches!(report.status, ValidationStatus::Passed),
            "XRechnung mit DOCTYPE darf nicht als gültig gelten: {report:?}"
        );
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(c.hits(), 0, "KoSIT hat eine externe Entität abgerufen");
}

/// Ersetzt den Inhalt der eingebetteten Datei einer ZUGFeRD-PDF.
fn replace_embedded_xml(pdf: &[u8], content: Vec<u8>, compress: bool) -> Vec<u8> {
    use lopdf::{Document, Object};
    let mut doc = Document::load_mem(pdf).expect("PDF lesbar");
    let mut content = Some(content);
    let mut replaced = 0;
    for object in doc.objects.values_mut() {
        if let Object::Stream(stream) = object {
            let is_embedded = stream
                .dict
                .get(b"Type")
                .and_then(|t| t.as_name())
                .map(|n| n == b"EmbeddedFile")
                .unwrap_or(false);
            if is_embedded {
                stream.dict.remove(b"Params");
                stream.set_plain_content(content.take().expect("genau eine eingebettete Datei"));
                if compress {
                    stream.compress().expect("komprimieren");
                }
                replaced += 1;
            }
        }
    }
    assert_eq!(replaced, 1, "genau eine eingebettete Datei erwartet");
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("PDF schreiben");
    out
}

async fn zugferd_pdf() -> Vec<u8> {
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
    mustang_bridge::create_zugferd(&pdf, &xml(), &sidecar_dir())
        .await
        .expect("Mustang combine")
}

#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_mustang_extract_does_not_resolve_external_entities() {
    let c = canary();
    let pdf = replace_embedded_xml(&zugferd_pdf().await, xxe_xml(c.port).into_bytes(), false);
    let result = mustang_bridge::extract_xml(&pdf, &sidecar_dir()).await;
    let text = format!("{result:?}");
    assert!(
        !text.contains("root:x:0:0"),
        "Dateiinhalt im Ergebnis: {text}"
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(c.hits(), 0, "Mustang hat eine externe Entität abgerufen");
    // Kommt XML zurück, ist es der Rohtext (DOCTYPE unaufgelöst); der Parser von
    // Klein.Buch lehnt die Entitäten danach ab.
    if let Ok(xml) = result {
        assert!(klein_buch_lib::einvoice::parser::parse(&xml).is_err());
    }
}

#[tokio::test]
#[ignore = "braucht Java-Laufzeit (sidecar_real)"]
async fn sidecar_real_mustang_extract_survives_decompression_bomb() {
    // 512 MiB Leerzeichen, komprimiert wenige hundert KiB: Mustang darf weder das
    // Volume füllen noch den Server-Speicher (ulimit -f, Größenprüfung, -Xmx).
    let mut bomb = xml().into_bytes();
    bomb.resize(512 * 1024 * 1024, b' ');
    let pdf = replace_embedded_xml(&zugferd_pdf().await, bomb, true);
    assert!(
        pdf.len() < 8 * 1024 * 1024,
        "Bombe ist klein: {} Byte",
        pdf.len()
    );
    let started = std::time::Instant::now();
    let result = mustang_bridge::extract_xml(&pdf, &sidecar_dir()).await;
    assert!(result.is_err(), "Bombe muss abgelehnt werden");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(150),
        "Abbruch innerhalb des Zeitlimits: {:?}",
        started.elapsed()
    );
}
