//! Referenz-PDFs für Typst-Updates (Lauf F6b, LE-TYPST).
//!
//! Die mit `#[ignore]` markierten Tests `pdf_snapshot_*` schreiben alle
//! Dokumentarten mit festen Fixture-Daten nach `$KB_PDF_SNAPSHOT_DIR`
//! (vier Rechnungsvorlagen, Storno, Regelbesteuerung, Angebot, EÜR-PDF,
//! Deckblatt, DSGVO-Auskunft, Paket-Vorschau und -Broschüre). Vor und nach
//! einem Update der Typst-Crates erzeugt, lassen sich die Fassungen per
//! `pdftotext` (Text) und `pdftoppm` (Pixel) vergleichen:
//!
//! `KB_PDF_SNAPSHOT_DIR=/pfad TZ=UTC cargo test --lib -- --ignored pdf_snapshot_`
//!
//! Die Tests nutzen nur die öffentliche Render-API (keine Typst-Typen), damit
//! sie unverändert über Typst-Versionen hinweg laufen. Ohne die Variable tun
//! sie nichts.

use std::path::PathBuf;

use chrono::NaiveDate;
use serde_json::json;

use crate::domain::invoice::{
    BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use crate::domain::quote::QuoteItemInput;
use crate::domain::storno::PrecedingInvoice;
use crate::pdf::templates;
use crate::pdf::typst_render::{self, CatalogEntry, QuoteRenderInput};

/// Zielordner aus `KB_PDF_SNAPSHOT_DIR` (angelegt), sonst `None`.
pub(crate) fn out_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("KB_PDF_SNAPSHOT_DIR")?);
    std::fs::create_dir_all(&dir).expect("Snapshot-Ordner anlegen");
    Some(dir)
}

/// Schreibt `name` (ohne Endung) als PDF in den Snapshot-Ordner.
pub(crate) fn write(dir: &std::path::Path, name: &str, pdf: &[u8]) {
    assert!(pdf.starts_with(b"%PDF-"), "{name}: kein PDF");
    std::fs::write(dir.join(format!("{name}.pdf")), pdf).expect("Snapshot schreiben");
}

fn logo() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/assets/app-icon/logo-1024.png"
    ))
    .expect("Logo docs/assets/app-icon/logo-1024.png")
}

fn default_typ() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../inputs/pdf-templates/default.typ"
    ))
    .expect("default.typ")
}

fn seller(klein: bool) -> SellerView<'static> {
    SellerView {
        name: "Beispiel IT-Service",
        street: "Beispielweg 1",
        postal_code: "12345",
        city: "Beispielstadt",
        country_code: "DE",
        tax_number: Some("12/345/67890"),
        vat_id: if klein { None } else { Some("DE123456789") },
        email: "rechnung@example.org",
        phone: Some("+49 30 1234567"),
        contact_name: None,
        iban: Some("DE02120300000000202051"),
        bic: Some("BYLADEM1001"),
        is_kleinunternehmer: klein,
    }
}

fn buyer() -> BuyerView<'static> {
    BuyerView {
        name: "Beispiel Kunde GmbH",
        street: Some("Kundenweg 2"),
        postal_code: Some("54321"),
        city: Some("Musterort"),
        country_code: "DE",
        vat_id: Some("DE111111111"),
        email: Some("kunde@example.org"),
    }
}

fn accounts() -> serde_json::Value {
    json!([
        {"type":"bank","label":"Geschäftskonto","holder":"Erika Beispiel","iban":"DE02120300000000202051","bic":"BYLADEM1001","details":null},
        {"type":"paypal","label":"PayPal","holder":"Erika Beispiel","iban":null,"bic":null,"details":"paypal.me/beispiel"}
    ])
}

const PACKAGE_MARKUP: &str = "Kompaktes **Paket** für kleine Büros:\n\n- Vorbesprechung vor Ort\n- Einrichtung (2 h)\n- *Einweisung* und Übergabe\n\n| Leistung | Umfang |\n|---|---|\n| Notebook | 2 Geräte |\n| Drucker | 1 Gerät |";

fn items(klein: bool) -> Vec<InvoiceItemInput> {
    let (rate, cat) = if klein { (0.0, "E") } else { (19.0, "S") };
    let item = |position: u32, description: &str, quantity: f64, unit: &str, price: i64| {
        InvoiceItemInput {
            position,
            description: description.into(),
            quantity,
            unit_code: unit.into(),
            unit_price_cents: price,
            tax_rate_percent: rate,
            tax_category_code: cat.into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }
    };
    let mut package = item(3, "Büro-Paket klein", 1.0, "C62", 49_000);
    package.description_title = Some("Büro-Paket klein".into());
    package.description_markup = Some(PACKAGE_MARKUP.into());
    vec![
        item(
            1,
            "Einrichtung Notebook inkl. Datenübernahme",
            2.5,
            "HUR",
            4_500,
        ),
        item(2, "Anfahrt 34 km × 0,30 €", 34.0, "KMT", 30),
        package,
        item(
            4,
            "USB-Stick 64 GB (Ersatz für defektes Gerät)",
            3.0,
            "C62",
            1_299,
        ),
    ]
}

fn invoice(klein: bool) -> InvoiceInput {
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
        delivery_date: NaiveDate::from_ymd_opt(2026, 10, 20),
        due_date: NaiveDate::from_ymd_opt(2026, 11, 7),
        currency_code: "EUR".into(),
        items: items(klein),
        notes: Some("Vielen Dank für die gute Zusammenarbeit.".into()),
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

fn quote_items() -> Vec<QuoteItemInput> {
    items(true)
        .into_iter()
        .map(|i| QuoteItemInput {
            position: i.position,
            description: i.description,
            quantity: i.quantity,
            unit_code: i.unit_code,
            unit_price_cents: i.unit_price_cents,
            tax_rate_percent: i.tax_rate_percent,
            tax_category_code: i.tax_category_code,
            description_title: i.description_title,
            description_markup: i.description_markup,
            source_package_id: None,
            source_package_revision: None,
        })
        .collect()
}

fn invoice_templates() -> Vec<(&'static str, String)> {
    vec![
        ("default", default_typ()),
        ("modern", templates::TEMPLATE_MODERN.to_string()),
        ("klassisch", templates::TEMPLATE_KLASSISCH.to_string()),
        ("minimal", templates::TEMPLATE_MINIMAL.to_string()),
    ]
}

/// Alle Render-Aufträge mit festen Daten: Rechnung (§19, ohne Logo,
/// Regelbesteuerung) und Storno je Rechnungsvorlage, Angebot je Vorlage,
/// Deckblatt, Paket-Vorschau und Broschüre (§19 und Regelbesteuerung).
fn jobs() -> Vec<(String, typst_render::PdfJob)> {
    let logo = logo();
    let logo_ref = Some(("/branding/logo.png", logo.as_slice()));
    let owner = Some("Erika Beispiel");
    let mut out = Vec::new();
    for (name, tpl) in invoice_templates() {
        crate::pdf::klausel_check::verify_for_kleinunternehmer(&tpl)
            .unwrap_or_else(|e| panic!("{name}: §19-Marker {e:?}"));
        let job = typst_render::invoice_job(
            &tpl,
            "RE-2026-0042",
            &invoice(true),
            &seller(true),
            &buyer(),
            None,
            logo_ref,
            owner,
            &accounts(),
            true,
        );
        out.push((format!("rechnung-{name}"), job));

        let mut no_delivery = invoice(true);
        no_delivery.delivery_date = None;
        no_delivery.payment_note = Some("Betrag dankend bar erhalten am 24.10.2026".into());
        let job = typst_render::invoice_job(
            &tpl,
            "RE-2026-0043",
            &no_delivery,
            &seller(true),
            &buyer(),
            None,
            None,
            None,
            &json!([]),
            true,
        );
        out.push((format!("rechnung-{name}-ohne-logo"), job));

        let job = typst_render::invoice_job(
            &tpl,
            "RE-2026-0044",
            &invoice(false),
            &seller(false),
            &buyer(),
            None,
            logo_ref,
            owner,
            &accounts(),
            true,
        );
        out.push((format!("rechnung-{name}-regel"), job));

        let mut storno = invoice(true);
        storno.is_storno_for = Some("019a0000-0000-7000-8000-000000000001".into());
        storno.cancel_reason = Some("Falscher Rechnungsempfänger".into());
        for it in &mut storno.items {
            it.unit_price_cents = -it.unit_price_cents;
        }
        let preceding = PrecedingInvoice {
            number: "RE-2026-0042".into(),
            issue_date: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
        };
        let job = typst_render::invoice_job_with_storno(
            &tpl,
            "ST-2026-0001",
            &storno,
            &seller(true),
            &buyer(),
            None,
            logo_ref,
            owner,
            &accounts(),
            true,
            Some(&preceding),
        );
        out.push((format!("storno-{name}"), job));
    }

    let quote = QuoteRenderInput {
        quote_date: "2026-10-24".into(),
        valid_until: "2026-11-23".into(),
        currency_code: "EUR".into(),
        items: quote_items(),
    };
    for (name, tpl) in [
        ("default", templates::DEFAULT_QUOTE_TEMPLATE),
        ("modern", templates::TEMPLATE_MODERN),
        ("klassisch", templates::TEMPLATE_KLASSISCH),
        ("minimal", templates::TEMPLATE_MINIMAL),
    ] {
        let job = typst_render::quote_job(
            tpl,
            "AN-2026-0007",
            &quote,
            &seller(true),
            &buyer(),
            None,
            logo_ref,
            Some(("/branding/signature.png", logo.as_slice())),
            true,
            owner,
            &accounts(),
        );
        out.push((format!("angebot-{name}"), job));
    }

    let cover = json!({
        "year": 2026,
        "generatedAt": "24.10.2026",
        "skr": "SKR03",
        "isKleinunternehmer": true,
        "kleinunternehmerHinweis": crate::domain::kleinunternehmer::HINWEIS_TEXT,
        "disclaimer": "Klein.Buch ist ein Werkzeug, kein Steuerberater. Diese Auswertung ist eine technische Aufbereitung der erfassten Belege; die steuerliche Würdigung obliegt dem Steuerberater.",
        "incomeTotal": "12.345,67 €",
        "expenseTotal": "2.345,00 €",
        "surplus": "10.000,67 €",
        "seller": {
            "name": "Beispiel IT-Service", "street": "Beispielweg 1",
            "postalCode": "12345", "city": "Beispielstadt",
            "taxNumber": "12/345/67890", "vatId": null,
            "email": "rechnung@example.org",
        },
        "contents": [
            "00-deckblatt.pdf — dieses Deckblatt",
            "LIESMICH.txt — §19-Klausel, Disclaimer, Inhaltsverzeichnis",
            "anlage-euer-2026.pdf — Anlage EÜR, Anlageverzeichnis, Einzelaufstellung",
            "manifest.json — SHA-256-Hash über jede Datei (Tamper-Detection beim Steuerberater)",
        ],
    });
    out.push((
        "deckblatt".into(),
        Ok(typst_render::plain_job(
            templates::DEFAULT_COVER_TEMPLATE,
            &cover.to_string(),
            None,
        )),
    ));

    let preview = json!({
        "title": "Büro-Paket klein",
        "body_typst": crate::domain::package::to_typst(&crate::domain::package::parse_markup(PACKAGE_MARKUP)),
        "price": "490,00 €",
        "is_klein": true,
        "klausel": crate::domain::kleinunternehmer::HINWEIS_TEXT,
    });
    out.push((
        "paket-vorschau".into(),
        Ok(typst_render::plain_job(
            templates::PACKAGE_PREVIEW_TEMPLATE,
            &preview.to_string(),
            None,
        )),
    ));

    let entries = [
        CatalogEntry {
            title: "Büro-Paket klein",
            body_markup: PACKAGE_MARKUP,
            price_cents: 49_000,
        },
        CatalogEntry {
            title: "Rundum-sorglos",
            body_markup:
                "# Wartung\n\nMonatliche Wartung mit *allem*, inkl. Updates und Backup-Kontrolle.",
            price_cents: 9_900,
        },
    ];
    for (suffix, klein) in [("", true), ("-regel", false)] {
        let job = typst_render::package_catalog_job(
            templates::PACKAGE_CATALOG_TEMPLATE,
            &entries,
            &seller(klein),
            Some("Beispiel Kunde GmbH"),
            owner,
            "24.10.2026",
            None,
            logo_ref,
        );
        out.push((format!("paket-broschuere{suffix}"), job));
    }
    out.into_iter()
        .map(|(name, job)| {
            let job = job.unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, job)
        })
        .collect()
}

/// Schreibt die Referenz-PDFs aller Aufträge aus [`jobs`].
#[test]
#[ignore = "erzeugt Referenz-PDFs nach KB_PDF_SNAPSHOT_DIR (Typst-Update)"]
fn pdf_snapshot_documents() {
    let Some(dir) = out_dir() else { return };
    for (name, job) in jobs() {
        let pdf = job.compile().unwrap_or_else(|e| panic!("{name}: {e}"));
        write(&dir, &name, &pdf);
    }
}

/// Lauf F6b: Die eingebauten Vorlagen nutzen nichts, was Typst als veraltet
/// meldet — veraltete Funktionen fallen mit dem nächsten Update weg (so
/// verschwand `json.decode` mit Typst 0.15). Erlaubt bleibt nur der Hinweis
/// auf die fehlende Schrift „Liberation Sans“ (Fallback, bewusst so; ADR 0008).
#[test]
fn builtin_templates_compile_without_typst_warnings() {
    for (name, job) in jobs() {
        let warnings: Vec<String> = job
            .compile_warnings()
            .into_iter()
            .filter(|w| !w.starts_with("unknown font family: liberation sans"))
            .collect();
        assert!(warnings.is_empty(), "{name}: Typst-Warnungen {warnings:?}");
    }
}
