//! Typst-Render (Shell).
//!
//! Eigene [`typst::World`]-Implementation:
//! - **Templates** kommen aus `inputs/pdf-templates/{name}.typ` (gelesen
//!   in [`crate::pdf::templates`]).
//! - **Fonts** aus `typst-assets` (Bundle: LibertinusSerif, NewCM,
//!   DejaVuSansMono) plus optional aus `inputs/branding/*.{ttf,otf}`,
//!   damit Manuel z. B. Liberation Sans nachreichen kann.
//! - **Daten-Injection** via `sys.inputs`: `data-json` = serialisierte
//!   Invoice + Items + Seller + Buyer + Kleinunternehmer-Hinweis.
//!
//! Public API: [`render_invoice`]. Caller hat das Template schon geladen
//! und gegen [`crate::pdf::klausel_check::verify_for_kleinunternehmer`]
//! gepingt.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::Datelike;
use serde_json::json;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Str, Value};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

use crate::domain::invoice::{compute_totals, BuyerView, InvoiceInput, SellerView};
use crate::domain::kleinunternehmer::HINWEIS_TEXT;
use crate::domain::package;
use crate::domain::quote::{compute_totals as quote_compute_totals, QuoteItemInput};
use crate::error::{Error, Result};

#[derive(Debug, thiserror::Error)]
pub enum PdfRenderError {
    #[error("Template-Datei nicht gefunden: {0}")]
    TemplateNotFound(PathBuf),
    #[error("Typst-Kompilation fehlgeschlagen: {0}")]
    Compile(String),
    #[error("Typst-PDF-Export fehlgeschlagen: {0}")]
    Export(String),
    #[error("Daten-Serialisierung fehlgeschlagen: {0}")]
    DataSerialization(#[from] serde_json::Error),
    #[error("Font-Datei beschädigt: {0}")]
    Font(String),
}

impl From<PdfRenderError> for Error {
    fn from(e: PdfRenderError) -> Self {
        Error::Config(format!("pdf-render: {e}"))
    }
}

/// Fallback-Text fürs Leistungsdatum, wenn kein Datum erfasst ist und der
/// globale Schalter `service_date_fallback_to_invoice_date` aktiv ist.
/// §14 Abs. 4 Nr. 6 UStG verlangt den Leistungszeitpunkt; der Fallback
/// vermeidet eine fehlende Pflichtangabe (außer Kleinbetrag ≤ 250 €).
pub const SERVICE_DATE_FALLBACK_NOTE: &str = "entspricht dem Rechnungsdatum";

/// Renderiert eine Rechnung als Standard-PDF (nicht PDF/A-3, das macht
/// die Mustang-Bridge danach). Daten werden als JSON in `sys.inputs`
/// injiziert; das Template liest sie via `json(sys.inputs.at("data-json"))`.
///
/// `service_date_fallback`: ist es `true` und das Leistungsdatum leer, emittiert
/// die Daten-JSON das Feld `delivery_date_fallback` (= [`SERVICE_DATE_FALLBACK_NOTE`]),
/// das die Vorlagen statt eines konkreten Datums anzeigen.
#[allow(clippy::too_many_arguments)]
pub fn render_invoice(
    template_source: &str,
    invoice_number: &str,
    input: &InvoiceInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    branding_dir: Option<&Path>,
    logo: Option<(&str, &[u8])>,
    owner_name: Option<&str>,
    accounts: &serde_json::Value,
    service_date_fallback: bool,
) -> Result<Vec<u8>> {
    let data = build_data_json(
        invoice_number,
        input,
        seller,
        buyer,
        logo.map(|(p, _)| p),
        owner_name,
        accounts,
        service_date_fallback,
    )?;
    // PDF/A-3b erzwingen: Mustang's ZUGFeRDExporterFromPDFA liest die
    // PDF/A-Kennung aus den XMP-Metadaten. Ein Plain-PDF (keine Kennung)
    // wird mit "PDF-A version not supported" abgelehnt — auch mit
    // ignorePDFAErrors, weil der Check VOR der Fehlerbehandlung läuft.
    // PDF/A-3b ist ohnehin der für ZUGFeRD geforderte Standard
    // (PDF 1.7 + erlaubte eingebettete Dateien).
    let embedded: Vec<(&str, &[u8])> = logo.into_iter().collect();
    compile_pdf(template_source, &data, branding_dir, true, &embedded)
}

/// Renderiert ein **Angebot** als Standard-PDF (Block 8). Anders als die
/// Rechnung läuft hier keine Mustang-/ZUGFeRD-Stufe (Angebote sind keine
/// E-Rechnungen) — daher ein einfaches PDF (kein PDF/A-3b erzwungen). Das
/// Template liest die Daten via `json.decode(sys.inputs.at("data-json"))` aus
/// dem `data.quote.*`-Schema (siehe Embedded-Template in
/// [`crate::pdf::templates`]).
#[allow(clippy::too_many_arguments)]
pub fn render_quote(
    template_source: &str,
    quote_number: &str,
    input: &QuoteRenderInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    branding_dir: Option<&Path>,
    logo: Option<(&str, &[u8])>,
    signature: Option<(&str, &[u8])>,
    signature_enabled: bool,
    owner_name: Option<&str>,
    accounts: &serde_json::Value,
) -> Result<Vec<u8>> {
    let data = build_quote_data_json(
        quote_number,
        input,
        seller,
        buyer,
        logo.map(|(p, _)| p),
        signature.map(|(p, _)| p),
        signature_enabled,
        owner_name,
        accounts,
    )?;
    let mut embedded: Vec<(&str, &[u8])> = Vec::new();
    if let Some(l) = logo {
        embedded.push(l);
    }
    if let Some(s) = signature {
        embedded.push(s);
    }
    compile_pdf(template_source, &data, branding_dir, false, &embedded)
}

/// Renderiert die **Anlage-EÜR-Übersicht** als Standard-PDF (Block 14a, Schritt 2).
/// Reines Anzeige-/Archivdokument (kein PDF/A-3, keine E-Rechnung). Die Daten
/// werden vom Caller fertig aufbereitet (Beträge als formatierte Strings) und via
/// `json.decode(sys.inputs.at("data-json"))` ins EÜR-Template injiziert.
pub fn render_euer(
    template_source: &str,
    data_json: &str,
    branding_dir: Option<&Path>,
) -> Result<Vec<u8>> {
    compile_pdf(template_source, data_json, branding_dir, false, &[])
}

/// Generisches Plain-PDF-Rendering aus einem Typst-Template + Daten-JSON
/// (`json.decode(sys.inputs.at("data-json"))`). Für einfache Dokumente wie das
/// Steuerberater-Paket-Deckblatt (Block 14c).
pub fn render_pdf(
    template_source: &str,
    data_json: &str,
    branding_dir: Option<&Path>,
) -> Result<Vec<u8>> {
    compile_pdf(template_source, data_json, branding_dir, false, &[])
}

/// Gemeinsame Typst-Kompilation + PDF-Export. `pdf_a3b` erzwingt PDF/A-3b
/// (Rechnung, für Mustang), sonst Plain-PDF (Angebot).
fn compile_pdf(
    template_source: &str,
    data_json: &str,
    branding_dir: Option<&Path>,
    pdf_a3b: bool,
    embedded: &[(&str, &[u8])],
) -> Result<Vec<u8>> {
    let world = TemplateWorld::new(template_source, data_json, branding_dir, embedded)?;
    let warned = typst::compile::<PagedDocument>(&world);
    let document = warned.output.map_err(|errs| {
        let msg = errs
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        PdfRenderError::Compile(msg)
    })?;
    let options = if pdf_a3b {
        let standards = typst_pdf::PdfStandards::new(&[typst_pdf::PdfStandard::A_3b])
            .map_err(|e| PdfRenderError::Export(format!("PDF/A-3b-Standards-Setup: {e}")))?;
        typst_pdf::PdfOptions {
            standards,
            ..Default::default()
        }
    } else {
        typst_pdf::PdfOptions::default()
    };
    let pdf_bytes = typst_pdf::pdf(&document, &options).map_err(|errs| {
        let msg = errs
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        PdfRenderError::Export(msg)
    })?;
    Ok(pdf_bytes)
}

/// Bequemer Wrapper, der das Default-Template aus dem inputs-Ordner lädt.
#[allow(clippy::too_many_arguments)]
pub fn render_invoice_default_template(
    inputs_dir: &Path,
    template_name: &str,
    invoice_number: &str,
    input: &InvoiceInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    branding_dir: Option<&Path>,
    logo: Option<(&str, &[u8])>,
) -> Result<Vec<u8>> {
    let template_path = inputs_dir
        .join("pdf-templates")
        .join(format!("{template_name}.typ"));
    let source = std::fs::read_to_string(&template_path)
        .map_err(|_| PdfRenderError::TemplateNotFound(template_path.clone()))?;
    render_invoice(
        &source,
        invoice_number,
        input,
        seller,
        buyer,
        branding_dir,
        logo,
        None,
        &serde_json::json!([]),
        true,
    )
}

/// Baut die Daten-JSON, die das Template via `sys.inputs.at("data-json")`
/// einliest. Schema-konform zum Default-Template (siehe Kommentar dort).
#[allow(clippy::too_many_arguments)]
pub fn build_data_json(
    invoice_number: &str,
    input: &InvoiceInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    logo_path: Option<&str>,
    owner_name: Option<&str>,
    accounts: &serde_json::Value,
    service_date_fallback: bool,
) -> std::result::Result<String, serde_json::Error> {
    let totals = compute_totals(&input.items);
    // §14 Abs. 4 Nr. 6 UStG: fehlt das Leistungsdatum und ist der Fallback aktiv,
    // zeigt die Vorlage „entspricht dem Rechnungsdatum" statt eines Datums.
    let delivery_date_fallback: Option<&str> =
        if input.delivery_date.is_none() && service_date_fallback {
            Some(SERVICE_DATE_FALLBACK_NOTE)
        } else {
            None
        };
    let items_json: Vec<_> = input
        .items
        .iter()
        .zip(totals.items.iter())
        .map(|(it, t)| {
            json!({
                "position": it.position,
                "description": it.description,
                "quantity": it.quantity,
                "unit": it.unit_code,
                "unit_price": it.unit_price_cents,
                "net_amount": t.net_amount_cents,
                "tax_rate": it.tax_rate_percent,
                "tax_category": it.tax_category_code,
                // P3: Titel für die Positionszeile (nur PDF) + formatierter
                // Volle-Breite-Block aus dem Body-Markup. null = schmale Zelle.
                "description_title": it.description_title,
                "description_typst": it.description_markup
                    .as_ref()
                    .filter(|m| !m.trim().is_empty())
                    .map(|m| package::to_typst(&package::parse_markup(m))),
            })
        })
        .collect();
    let data = json!({
        "invoice": {
            "number": invoice_number,
            "date": input.invoice_date.to_string(),
            "delivery_date": input.delivery_date.map(|d| d.to_string()),
            "delivery_date_fallback": delivery_date_fallback,
            "due_date": input.due_date.map(|d| d.to_string()),
            "payment_note": input.payment_note,
            "currency": input.currency_code,
            "is_kleinunternehmer": seller.is_kleinunternehmer,
            "net_amount": totals.net_amount_cents,
            "tax_amount": totals.tax_amount_cents,
            "gross_amount": totals.gross_amount_cents,
            "items": items_json,
        },
        "seller": {
            "name": seller.name,
            "owner_name": owner_name,
            "street": seller.street,
            "postal_code": seller.postal_code,
            "city": seller.city,
            "country_code": seller.country_code,
            "tax_number": seller.tax_number,
            "vat_id": seller.vat_id,
            "email": seller.email,
            "iban": seller.iban,
            "bic": seller.bic,
            "logo_path": logo_path,
        },
        "buyer": {
            "name": buyer.name,
            "street": buyer.street,
            "postal_code": buyer.postal_code,
            "city": buyer.city,
            "country_code": buyer.country_code,
            "vat_id": buyer.vat_id,
            "email": buyer.email,
        },
        "kleinunternehmer": {
            "hinweis_text": HINWEIS_TEXT,
        },
        "payment_accounts": accounts.clone(),
    });
    serde_json::to_string(&data)
}

/// Eingabe für [`render_quote`] — analog [`InvoiceInput`], aber mit
/// Angebots-Semantik: `valid_until` statt Fälligkeit, keine direction/storno.
#[derive(Debug, Clone)]
pub struct QuoteRenderInput {
    pub quote_date: String,
    pub valid_until: String,
    pub currency_code: String,
    pub items: Vec<QuoteItemInput>,
}

/// Baut die Daten-JSON fürs Angebots-Template (`data.quote.*`). Schema-konform
/// zum Embedded-Quote-Template in [`crate::pdf::templates`].
#[allow(clippy::too_many_arguments)]
pub fn build_quote_data_json(
    quote_number: &str,
    input: &QuoteRenderInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    logo_path: Option<&str>,
    signature_path: Option<&str>,
    signature_enabled: bool,
    owner_name: Option<&str>,
    accounts: &serde_json::Value,
) -> std::result::Result<String, serde_json::Error> {
    let totals = quote_compute_totals(&input.items);
    let items_json: Vec<_> = input
        .items
        .iter()
        .zip(totals.items.iter())
        .map(|(it, t)| {
            json!({
                "position": it.position,
                "description": it.description,
                "quantity": it.quantity,
                "unit": it.unit_code,
                "unit_price": it.unit_price_cents,
                "net_amount": t.net_amount_cents,
                "tax_rate": it.tax_rate_percent,
                "tax_category": it.tax_category_code,
                // P3: Titel für die Positionszeile (nur PDF) + formatierter
                // Volle-Breite-Block aus dem Body-Markup. null = schmale Zelle.
                "description_title": it.description_title,
                "description_typst": it.description_markup
                    .as_ref()
                    .filter(|m| !m.trim().is_empty())
                    .map(|m| package::to_typst(&package::parse_markup(m))),
            })
        })
        .collect();
    let data = json!({
        "quote": {
            "number": quote_number,
            "date": input.quote_date,
            "valid_until": input.valid_until,
            "currency": input.currency_code,
            "is_kleinunternehmer": seller.is_kleinunternehmer,
            "net_amount": totals.net_amount_cents,
            "tax_amount": totals.tax_amount_cents,
            "gross_amount": totals.gross_amount_cents,
            "items": items_json,
            "signature_enabled": signature_enabled,
            "signature_path": signature_path,
        },
        "seller": {
            "name": seller.name,
            "owner_name": owner_name,
            "street": seller.street,
            "postal_code": seller.postal_code,
            "city": seller.city,
            "country_code": seller.country_code,
            "tax_number": seller.tax_number,
            "vat_id": seller.vat_id,
            "email": seller.email,
            "iban": seller.iban,
            "bic": seller.bic,
            "logo_path": logo_path,
        },
        "buyer": {
            "name": buyer.name,
            "street": buyer.street,
            "postal_code": buyer.postal_code,
            "city": buyer.city,
            "country_code": buyer.country_code,
            "vat_id": buyer.vat_id,
            "email": buyer.email,
        },
        "kleinunternehmer": {
            "hinweis_text": HINWEIS_TEXT,
        },
        "payment_accounts": accounts.clone(),
    });
    serde_json::to_string(&data)
}

// -----------------------------------------------------------------------
// Paket-Katalog-Broschüre (Block P4) — KEIN §14-Beleg
// -----------------------------------------------------------------------

/// Ein Paket in der Broschüre: die aktive Revision, aufbereitet für das
/// „paketübersicht"-Template. Reine geliehene Sicht (kein DB-Coupling), analog
/// [`QuoteRenderInput`]; der Caller mappt `PackageRevisionRow` → `CatalogEntry`.
#[derive(Debug, Clone)]
pub struct CatalogEntry<'a> {
    /// Kundenseitiger Titel (Revisions-`title`).
    pub title: &'a str,
    /// Body-Markdown-Subset → formatierter PDF-Block (`to_typst`).
    pub body_markup: &'a str,
    /// Netto-Katalogpreis in Cent.
    pub price_cents: i64,
}

/// Baut die Daten-JSON für [`render_package_catalog`] (Schema-konform zur
/// eingebauten Vorlage `PACKAGE_CATALOG_TEMPLATE`). Jeder Paket-Block kommt aus
/// dem AST-only/escaped `domain::package::to_typst` und wird im Template via
/// `eval(mode: "markup")` eingebettet (gleiche Mechanik wie die Paket-Vorschau).
pub fn build_package_catalog_data_json(
    entries: &[CatalogEntry<'_>],
    seller: &SellerView<'_>,
    contact_name: Option<&str>,
    owner_name: Option<&str>,
    logo_path: Option<&str>,
    generated_at: &str,
) -> std::result::Result<String, serde_json::Error> {
    let packages_json: Vec<_> = entries
        .iter()
        .map(|e| {
            json!({
                "title": e.title,
                "price_cents": e.price_cents,
                "body_typst": package::to_typst(&package::parse_markup(e.body_markup)),
            })
        })
        .collect();
    let data = json!({
        "title": "Unsere Leistungspakete",
        "generated_at": generated_at,
        "contact_name": contact_name,
        "is_klein": seller.is_kleinunternehmer,
        "klausel": HINWEIS_TEXT,
        "packages": packages_json,
        "seller": {
            "name": seller.name,
            "owner_name": owner_name,
            "street": seller.street,
            "postal_code": seller.postal_code,
            "city": seller.city,
            "country_code": seller.country_code,
            "tax_number": seller.tax_number,
            "vat_id": seller.vat_id,
            "email": seller.email,
            "logo_path": logo_path,
        },
    });
    serde_json::to_string(&data)
}

/// Rendert die Paket-Katalog-Broschüre als Plain-PDF (kein PDF/A-3, keine
/// E-Rechnung — es ist **kein** §14-Beleg). Leere Auswahl wird abgelehnt.
/// `contact_name` setzt eine optionale persönliche Anrede (nur beim Versand).
#[allow(clippy::too_many_arguments)]
pub fn render_package_catalog(
    template_source: &str,
    entries: &[CatalogEntry<'_>],
    seller: &SellerView<'_>,
    contact_name: Option<&str>,
    owner_name: Option<&str>,
    generated_at: &str,
    branding_dir: Option<&Path>,
    logo: Option<(&str, &[u8])>,
) -> Result<Vec<u8>> {
    if entries.is_empty() {
        return Err(Error::Domain(
            "Bitte mindestens ein Paket für die Broschüre auswählen.".into(),
        ));
    }
    let data = build_package_catalog_data_json(
        entries,
        seller,
        contact_name,
        owner_name,
        logo.map(|(p, _)| p),
        generated_at,
    )?;
    let embedded: Vec<(&str, &[u8])> = logo.into_iter().collect();
    compile_pdf(template_source, &data, branding_dir, false, &embedded)
}

// -----------------------------------------------------------------------
// World-Implementation
// -----------------------------------------------------------------------

const MAIN_VPATH: &str = "/main.typ";

/// Statisch gecachte Fonts aus `typst-assets`. Wird beim ersten Render
/// aufgebaut, danach geteilt.
fn bundled_fonts() -> &'static [Font] {
    static FONTS: OnceLock<Vec<Font>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut fonts = Vec::new();
        for data in typst_assets::fonts() {
            // `data` ist `&'static [u8]` — erfüllt `AsRef<[u8]> + Send + Sync + 'static`.
            for font in Font::iter(Bytes::new(data)) {
                fonts.push(font);
            }
        }
        fonts
    })
}

fn load_branding_fonts(dir: &Path) -> Vec<Font> {
    let mut fonts = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return fonts;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());
        if !matches!(ext.as_deref(), Some("ttf") | Some("otf")) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        for font in Font::iter(Bytes::new(bytes)) {
            fonts.push(font);
        }
    }
    fonts
}

struct TemplateWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: FileId,
    source: Source,
    today: Datetime,
    files: Vec<(FileId, Bytes)>,
}

impl TemplateWorld {
    fn new(
        template_source: &str,
        data_json: &str,
        branding_dir: Option<&Path>,
        embedded: &[(&str, &[u8])],
    ) -> Result<Self> {
        let mut fonts: Vec<Font> = bundled_fonts().to_vec();
        if let Some(d) = branding_dir {
            fonts.extend(load_branding_fonts(d));
        }
        let book = FontBook::from_fonts(fonts.iter());

        let mut inputs = Dict::new();
        inputs.insert(Str::from("data-json"), Value::Str(Str::from(data_json)));
        let library = Library::builder().with_inputs(inputs).build();

        let main = FileId::new(None, VirtualPath::new(MAIN_VPATH));
        let source = Source::new(main, template_source.to_string());

        let today = today_de_local();

        let files = embedded
            .iter()
            .map(|&(vpath, bytes)| {
                (
                    FileId::new(None, VirtualPath::new(vpath)),
                    Bytes::new(bytes.to_vec()),
                )
            })
            .collect();

        Ok(Self {
            library: LazyHash::new(library),
            book: LazyHash::new(book),
            fonts,
            main,
            source,
            today,
            files,
        })
    }
}

fn today_de_local() -> Datetime {
    let now = chrono::Local::now().date_naive();
    Datetime::from_ymd_hms(now.year(), now.month() as u8, now.day() as u8, 0, 0, 0)
        .unwrap_or_else(|| Datetime::from_ymd_hms(2026, 1, 1, 0, 0, 0).expect("static"))
}

impl World for TemplateWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        for (fid, bytes) in &self.files {
            if id == *fid {
                return Ok(bytes.clone());
            }
        }
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Some(self.today)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::invoice::{InvoiceDirection, InvoiceItemInput};
    use chrono::NaiveDate;

    fn seller() -> SellerView<'static> {
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
            due_date: None,
            currency_code: "EUR".into(),
            items: vec![InvoiceItemInput {
                position: 1,
                description: "Beratung".into(),
                quantity: 1.0,
                unit_code: "HUR".into(),
                unit_price_cents: 10_000,
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

    #[test]
    fn build_data_json_emits_required_keys() {
        let s = build_data_json(
            "RE-2026-0001",
            &invoice(),
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        // Schema-Kontrakt zum default.typ-Template:
        assert!(s.contains("\"number\":\"RE-2026-0001\""));
        assert!(s.contains("\"is_kleinunternehmer\":true"));
        assert!(s.contains("\"hinweis_text\":\"Gemäß §19 UStG"));
        assert!(s.contains("\"items\":["));
        assert!(s.contains("\"unit\":\"HUR\""));
    }

    #[test]
    fn build_data_json_includes_seller_iban_when_present() {
        let mut s = seller();
        s.iban = Some("DE02120300000000202051");
        s.bic = Some("BYLADEM1001");
        let out = build_data_json(
            "RE-2026-0001",
            &invoice(),
            &s,
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        assert!(out.contains("\"iban\":\"DE02120300000000202051\""));
        assert!(out.contains("\"bic\":\"BYLADEM1001\""));
    }

    #[test]
    fn build_data_json_includes_payment_accounts() {
        let accounts = serde_json::json!([
            {
                "type": "bank",
                "label": "Geschäftskonto",
                "holder": "Max Mustermann",
                "iban": "DE02120300000000202051",
                "bic": "BYLADEM1001",
                "details": null,
            },
            {
                "type": "paypal",
                "label": "PayPal",
                "holder": "Max Mustermann",
                "iban": null,
                "bic": null,
                "details": "paypal.me/mustermann",
            }
        ]);
        let out = build_data_json(
            "RE-2026-0001",
            &invoice(),
            &seller(),
            &buyer(),
            None,
            None,
            &accounts,
            true,
        )
        .unwrap();
        assert!(out.contains("\"payment_accounts\":["));
        assert!(out.contains("\"holder\":\"Max Mustermann\""));
        assert!(out.contains("\"details\":\"paypal.me/mustermann\""));
    }

    #[test]
    fn build_data_json_emits_delivery_fallback_when_empty_and_enabled() {
        let inv = InvoiceInput {
            delivery_date: None,
            ..invoice()
        };
        // Fallback an + kein Leistungsdatum → Hinweis-Text gesetzt.
        let on = build_data_json(
            "RE-2026-0001",
            &inv,
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        assert!(on.contains("\"delivery_date_fallback\":\"entspricht dem Rechnungsdatum\""));
        // Fallback aus → kein Hinweis (null).
        let off = build_data_json(
            "RE-2026-0001",
            &inv,
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            false,
        )
        .unwrap();
        assert!(off.contains("\"delivery_date_fallback\":null"));
        // Leistungsdatum vorhanden → kein Fallback, auch wenn aktiv.
        let with_date = build_data_json(
            "RE-2026-0001",
            &invoice(),
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        assert!(with_date.contains("\"delivery_date_fallback\":null"));
    }

    #[test]
    fn build_data_json_emits_payment_note_when_set() {
        let inv = InvoiceInput {
            payment_note: Some("Betrag dankend bar erhalten am 23.05.2026".into()),
            ..invoice()
        };
        let on = build_data_json(
            "RE-2026-0001",
            &inv,
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        assert!(on.contains("\"payment_note\":\"Betrag dankend bar erhalten am 23.05.2026\""));
        // Ohne Hinweis (Fixture payment_note=None) → null.
        let off = build_data_json(
            "RE-2026-0001",
            &invoice(),
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        assert!(off.contains("\"payment_note\":null"));
    }

    #[test]
    fn bundled_fonts_load_at_least_libertinus() {
        let fonts = bundled_fonts();
        assert!(
            !fonts.is_empty(),
            "typst-assets/fonts feature must be enabled"
        );
    }

    fn quote_input() -> QuoteRenderInput {
        QuoteRenderInput {
            quote_date: "2026-05-19".into(),
            valid_until: "2026-06-18".into(),
            currency_code: "EUR".into(),
            items: vec![QuoteItemInput {
                position: 1,
                description: "Beratung".into(),
                quantity: 1.0,
                unit_code: "HUR".into(),
                unit_price_cents: 10_000,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            }],
        }
    }

    #[test]
    fn build_quote_data_json_emits_required_keys() {
        let s = build_quote_data_json(
            "AN-2026-0001",
            &quote_input(),
            &seller(),
            &buyer(),
            None,
            None,
            false,
            None,
            &serde_json::json!([]),
        )
        .unwrap();
        assert!(s.contains("\"number\":\"AN-2026-0001\""));
        assert!(s.contains("\"valid_until\":\"2026-06-18\""));
        assert!(s.contains("\"is_kleinunternehmer\":true"));
        assert!(s.contains("\"hinweis_text\":\"Gemäß §19 UStG"));
    }

    #[test]
    fn build_quote_data_json_includes_signature_when_enabled() {
        let s = build_quote_data_json(
            "AN-2026-0001",
            &quote_input(),
            &seller(),
            &buyer(),
            None,
            Some("/branding/signature.png"),
            true,
            None,
            &serde_json::json!([]),
        )
        .unwrap();
        assert!(s.contains("\"signature_enabled\":true"));
        assert!(s.contains("\"signature_path\":\"/branding/signature.png\""));
    }

    #[test]
    fn render_quote_with_embedded_template_produces_pdf() {
        let tpl = crate::pdf::templates::DEFAULT_QUOTE_TEMPLATE;
        let pdf = render_quote(
            tpl,
            "AN-2026-0001",
            &quote_input(),
            &seller(),
            &buyer(),
            None,
            None,
            None,
            false,
            None,
            &serde_json::json!([]),
        )
        .expect("render quote");
        assert!(
            pdf.starts_with(b"%PDF-"),
            "expected %PDF- header, got {:?}",
            &pdf[..10.min(pdf.len())]
        );
        assert!(pdf.len() > 500, "pdf seems too small: {} bytes", pdf.len());
    }

    #[test]
    fn render_euer_with_embedded_template_produces_pdf() {
        let data = json!({
            "year": 2026,
            "generatedAt": "2026-05-21",
            "isKleinunternehmer": true,
            "kleinunternehmerHinweis": "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.",
            "seller": {
                "name": "Wildbach Computerhilfe", "street": "Weg 1",
                "postalCode": "84028", "city": "Landshut",
                "taxNumber": "123/456/7890", "vatId": serde_json::Value::Null,
                "email": "schmidm@wildbach-computerhilfe.de",
            },
            "form": {
                "lines": [
                    {"zeile": "12", "bezeichnung": "Betriebseinnahmen §19", "betrag": "1.000,00 €", "isEntry": true},
                    {"zeile": "—", "bezeichnung": "Summe der Betriebseinnahmen", "betrag": "1.000,00 €", "isEntry": false},
                ],
                "incomeTotal": "1.000,00 €", "expenseTotal": "20,00 €", "surplus": "980,00 €",
            },
            "assets": [
                {"assetNumber": "AV-2026-0001", "label": "Notebook", "acquisitionDate": "2026-01-10",
                 "acquisitionCost": "1.200,00 €", "method": "linear", "afaYear": "400,00 €",
                 "bookValueEnd": "800,00 €", "disposalNote": ""},
            ],
            "income": [
                {"paidDate": "2026-03-01", "invoiceNumber": "RE-2026-0001", "customer": "ACME",
                 "description": "Beratung", "amount": "1.000,00 €"},
            ],
            "storno": [],
            "incomeSum": "1.000,00 €",
            "expenses": [
                {"paidDate": "2026-04-01", "expenseNumber": "KO-2026-0001", "vendor": "Shop",
                 "category": "Hardware", "description": "Maus", "amount": "20,00 €"},
            ],
            "expenseSum": "20,00 €",
            "disposals": [],
        });
        let s = serde_json::to_string(&data).unwrap();
        let pdf = render_euer(crate::pdf::templates::DEFAULT_EUER_TEMPLATE, &s, None)
            .expect("render euer");
        assert!(
            pdf.starts_with(b"%PDF-"),
            "expected %PDF- header, got {:?}",
            &pdf[..10.min(pdf.len())]
        );
        assert!(pdf.len() > 500, "pdf seems too small: {} bytes", pdf.len());
    }

    #[test]
    fn render_package_preview_eval_markup_produces_pdf() {
        // Verifiziert die zentrale Render-Mechanik: `eval(mode: "markup")` über den
        // AST-only/escaped `to_typst`-Output (Überschrift, Liste, fett, Tabelle).
        use crate::domain::package;
        let body = package::to_typst(&package::parse_markup(
            "# Hochzeit klein\n\nKurze **Beschreibung** mit Liste:\n\n- Vorbesprechung\n- Shooting (2h)\n- Bildauswahl",
        ));
        let data = json!({
            "title": "Hochzeit klein",
            "body_typst": body,
            "price": "900,00 €",
            "is_klein": true,
            "klausel": "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.",
        });
        let s = serde_json::to_string(&data).unwrap();
        let pdf = render_pdf(crate::pdf::templates::PACKAGE_PREVIEW_TEMPLATE, &s, None)
            .expect("render package preview");
        assert!(
            pdf.starts_with(b"%PDF-"),
            "expected %PDF- header, got {:?}",
            &pdf[..10.min(pdf.len())]
        );
        assert!(pdf.len() > 500, "pdf seems too small: {} bytes", pdf.len());
    }

    // ---- Block P4: Paket-Katalog-Broschüre ---------------------------------

    fn catalog_entries() -> Vec<CatalogEntry<'static>> {
        vec![
            CatalogEntry {
                title: "Hochzeit klein",
                body_markup:
                    "Kompaktes Paket:\n\n- Vorbesprechung\n- **Shooting** (2h)\n- Bildauswahl",
                price_cents: 90_000,
            },
            CatalogEntry {
                title: "Hochzeit groß",
                body_markup: "# Rundum-sorglos\n\nGanztägige Begleitung mit *allem*.",
                price_cents: 180_000,
            },
        ]
    }

    #[test]
    fn build_package_catalog_data_json_emits_packages_and_seller() {
        let entries = catalog_entries();
        let s = build_package_catalog_data_json(
            &entries,
            &seller(),
            Some("Kunde GmbH"),
            Some("Manuel Schmid"),
            None,
            "2026-05-24",
        )
        .unwrap();
        assert!(s.contains("\"title\":\"Unsere Leistungspakete\""));
        assert!(s.contains("\"contact_name\":\"Kunde GmbH\""));
        assert!(s.contains("\"is_klein\":true"));
        assert!(s.contains("Gemäß §19 UStG"));
        // Markup → eingebetteter Typst-Block (AST-only, escaped).
        assert!(s.contains("#list("));
        assert!(s.contains("#strong[Shooting]"));
        assert!(s.contains("\"price_cents\":90000"));
        assert!(s.contains("\"generated_at\":\"2026-05-24\""));
    }

    #[test]
    fn render_package_catalog_one_and_many_produces_pdf() {
        let tpl = crate::pdf::templates::PACKAGE_CATALOG_TEMPLATE;
        // Eine Auswahl.
        let one = vec![catalog_entries().remove(0)];
        let pdf = render_package_catalog(
            tpl,
            &one,
            &seller(),
            None,
            Some("Manuel Schmid"),
            "2026-05-24",
            None,
            None,
        )
        .expect("render catalog (1 Paket)");
        assert!(pdf.starts_with(b"%PDF-"), "kein PDF-Header (1 Paket)");
        assert!(
            pdf.len() > 500,
            "PDF zu klein (1 Paket): {} Bytes",
            pdf.len()
        );

        // Mehrere Pakete + persönliche Anrede.
        let many = catalog_entries();
        let pdf = render_package_catalog(
            tpl,
            &many,
            &seller(),
            Some("Kunde GmbH"),
            Some("Manuel Schmid"),
            "2026-05-24",
            None,
            None,
        )
        .expect("render catalog (N Pakete)");
        assert!(pdf.starts_with(b"%PDF-"), "kein PDF-Header (N Pakete)");

        // Regelbesteuerung (USt-Hinweis-Zweig) rendert ebenfalls.
        let regel = SellerView {
            is_kleinunternehmer: false,
            ..seller()
        };
        let pdf = render_package_catalog(tpl, &many, &regel, None, None, "2026-05-24", None, None)
            .expect("render catalog (Regelbesteuerung)");
        assert!(pdf.starts_with(b"%PDF-"), "kein PDF-Header (Regel)");
    }

    #[test]
    fn render_package_catalog_rejects_empty_selection() {
        let tpl = crate::pdf::templates::PACKAGE_CATALOG_TEMPLATE;
        let err = render_package_catalog(tpl, &[], &seller(), None, None, "2026-05-24", None, None)
            .unwrap_err();
        assert!(
            err.to_string().contains("mindestens ein Paket"),
            "unerwartete Fehlermeldung: {err}"
        );
    }

    #[test]
    fn build_data_json_emits_description_typst_for_markup_item() {
        let mut inv = invoice();
        inv.items[0].description_markup =
            Some("**Hochzeit klein**\n\n- Vorbesprechung\n- Shooting".into());
        let s = build_data_json(
            "RE-2026-0001",
            &inv,
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        // Markup → eingebetteter Typst-Block (AST-only). Ohne Markup wäre es null.
        assert!(s.contains("\"description_typst\":\"#strong[Hochzeit klein]"));
        assert!(s.contains("#list("));
        let plain = build_data_json(
            "RE-2026-0001",
            &invoice(),
            &seller(),
            &buyer(),
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .unwrap();
        assert!(plain.contains("\"description_typst\":null"));
    }

    #[test]
    fn render_with_markup_item_full_width_block() {
        // P3: Position mit description_markup → Volle-Breite-Block via
        // eval(mode:"markup"). Übt den neuen colspan-Pfad in allen Built-ins
        // (Rechnung) + dem eingebauten Angebots-Default. Fängt Typst-Syntax-
        // Fehler in den geänderten Vorlagen-Strings ab.
        use crate::pdf::templates::{
            DEFAULT_QUOTE_TEMPLATE, TEMPLATE_KLASSISCH, TEMPLATE_MINIMAL, TEMPLATE_MODERN,
        };
        let markup =
            "**Hochzeit klein**\n\nEnthält:\n\n- Vorbesprechung\n- Shooting (2h)\n- Bildauswahl";
        let mut inv = invoice();
        inv.items[0].description_markup = Some(markup.into());
        inv.items[0].source_package_id = Some("pkg-1".into());
        inv.items[0].source_package_revision = Some(2);

        for (name, tpl) in [
            ("modern", TEMPLATE_MODERN),
            ("klassisch", TEMPLATE_KLASSISCH),
            ("minimal", TEMPLATE_MINIMAL),
        ] {
            let pdf = render_invoice(
                tpl,
                "RE-2026-0009",
                &inv,
                &seller(),
                &buyer(),
                None,
                None,
                None,
                &serde_json::json!([]),
                true,
            )
            .unwrap_or_else(|e| panic!("{name} markup-Rechnung: {e}"));
            assert!(pdf.starts_with(b"%PDF-"), "{name}: kein PDF-Header");
        }

        let mut q = quote_input();
        q.items[0].description_markup = Some(markup.into());
        q.items[0].source_package_id = Some("pkg-1".into());
        q.items[0].source_package_revision = Some(2);
        let pdf = render_quote(
            DEFAULT_QUOTE_TEMPLATE,
            "AN-2026-0009",
            &q,
            &seller(),
            &buyer(),
            None,
            None,
            None,
            false,
            None,
            &serde_json::json!([]),
        )
        .expect("markup-Angebot");
        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn render_cover_with_embedded_template_produces_pdf() {
        let data = json!({
            "year": 2026,
            "generatedAt": "2026-05-21",
            "skr": "SKR03",
            "incomeTotal": "10.000,00 €",
            "expenseTotal": "4.000,00 €",
            "surplus": "6.000,00 €",
            "seller": {
                "name": "Wildbach Computerhilfe", "street": "Weg 1",
                "postalCode": "84028", "city": "Landshut",
                "taxNumber": "123/456/7890", "vatId": serde_json::Value::Null,
                "email": "a@b.de",
            },
            "contents": [
                "00-deckblatt.pdf — dieses Deckblatt",
                "anlage-euer-2026.pdf — Anlage EÜR",
                "datev-buchungsstapel.csv — DATEV (SKR03)",
            ],
        });
        let s = serde_json::to_string(&data).unwrap();
        let pdf = render_pdf(crate::pdf::templates::DEFAULT_COVER_TEMPLATE, &s, None)
            .expect("render cover");
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 500);
    }

    #[test]
    fn render_minimal_typst_produces_pdf_bytes() {
        // Minimales Template, das NICHT klausel_check bestehen muss (eigener Test).
        // `json.decode()` für In-Memory-Strings; `json()` lädt vom Filesystem
        // und würde den JSON-Inhalt als Pfad interpretieren.
        let tpl = r#"
            #let data = json.decode(sys.inputs.at("data-json"))
            #set page(paper: "a4")
            = Test-Rechnung #data.invoice.number
            #data.invoice.is_kleinunternehmer
        "#;
        let pdf = render_invoice(
            tpl,
            "RE-2026-0001",
            &invoice(),
            &seller(),
            &buyer(),
            None,
            None,
            None,
            &serde_json::json!([]),
            true,
        )
        .expect("render");
        // PDF-Header
        assert!(
            pdf.starts_with(b"%PDF-"),
            "expected %PDF- header, got {:?}",
            &pdf[..10.min(pdf.len())]
        );
        assert!(pdf.len() > 500, "pdf seems too small: {} bytes", pdf.len());
    }

    /// Rendert ALLE eingebetteten Built-in-Vorlagen in beiden Modi (Rechnung +
    /// Angebot) und beiden Steuer-Lagen (§19 + Regelbesteuerung). Fängt Typst-
    /// Syntaxfehler in den Vorlagen-Strings ab, die `cargo build` nicht sieht
    /// (sie sind nur `&str`-Konstanten — Fehler tauchen erst beim Render auf).
    #[test]
    fn all_builtin_templates_render_both_modes() {
        use crate::pdf::templates::{
            DEFAULT_QUOTE_TEMPLATE, TEMPLATE_KLASSISCH, TEMPLATE_MINIMAL, TEMPLATE_MODERN,
        };
        // Bank-Konto (IBAN) + PayPal → übt Fuß-Iteration, §19-Beträge, Zahlungshinweis.
        let accounts = serde_json::json!([
            {"type":"bank","label":"Geschäftskonto","holder":"Manuel Schmid","iban":"DE02120300000000202051","bic":"BYLADEM1001","details":null},
            {"type":"paypal","label":"PayPal","holder":"Manuel Schmid","iban":null,"bic":null,"details":"paypal.me/ich"}
        ]);
        let regel = SellerView {
            is_kleinunternehmer: false,
            ..seller()
        };
        let ok = |pdf: &[u8], ctx: &str| {
            assert!(pdf.starts_with(b"%PDF-"), "{ctx}: kein PDF-Header");
            assert!(pdf.len() > 500, "{ctx}: PDF zu klein ({} Bytes)", pdf.len());
        };

        for (name, tpl) in [
            ("modern", TEMPLATE_MODERN),
            ("klassisch", TEMPLATE_KLASSISCH),
            ("minimal", TEMPLATE_MINIMAL),
        ] {
            // Rechnung §19 (Kleinunternehmer): nur Rechnungsbetrag, Leistungsdatum, Konten-Fuß.
            let pdf = render_invoice(
                tpl,
                "RE-2026-0001",
                &invoice(),
                &seller(),
                &buyer(),
                None,
                None,
                Some("Manuel Schmid"),
                &accounts,
                true,
            )
            .unwrap_or_else(|e| panic!("{name} Rechnung §19: {e}"));
            ok(&pdf, &format!("{name} Rechnung §19"));

            // Rechnung §19 OHNE Leistungsdatum + Fallback aktiv → else-if-Zweig
            // „Leistungsdatum: entspricht dem Rechnungsdatum" wird gerendert.
            let inv_no_delivery = InvoiceInput {
                delivery_date: None,
                ..invoice()
            };
            let pdf = render_invoice(
                tpl,
                "RE-2026-0003",
                &inv_no_delivery,
                &seller(),
                &buyer(),
                None,
                None,
                Some("Manuel Schmid"),
                &accounts,
                true,
            )
            .unwrap_or_else(|e| panic!("{name} Rechnung §19 Fallback: {e}"));
            ok(&pdf, &format!("{name} Rechnung §19 Fallback"));

            // Rechnung Regelbesteuerung: Netto/USt/Brutto-Zweig.
            let pdf = render_invoice(
                tpl,
                "RE-2026-0002",
                &invoice(),
                &regel,
                &buyer(),
                None,
                None,
                Some("Manuel Schmid"),
                &accounts,
                true,
            )
            .unwrap_or_else(|e| panic!("{name} Rechnung Regel: {e}"));
            ok(&pdf, &format!("{name} Rechnung Regel"));

            // Angebot §19 mit aktivierten Unterschriftsfeldern.
            let pdf = render_quote(
                tpl,
                "AN-2026-0001",
                &quote_input(),
                &seller(),
                &buyer(),
                None,
                None,
                None,
                true,
                Some("Manuel Schmid"),
                &accounts,
            )
            .unwrap_or_else(|e| panic!("{name} Angebot §19: {e}"));
            ok(&pdf, &format!("{name} Angebot §19"));
        }

        // Eingebettetes Default-Angebot in beiden Steuer-Lagen.
        let pdf = render_quote(
            DEFAULT_QUOTE_TEMPLATE,
            "AN-2026-0002",
            &quote_input(),
            &seller(),
            &buyer(),
            None,
            None,
            None,
            false,
            None,
            &accounts,
        )
        .expect("Angebot-Default §19");
        ok(&pdf, "angebot-default §19");
        let pdf = render_quote(
            DEFAULT_QUOTE_TEMPLATE,
            "AN-2026-0003",
            &quote_input(),
            &regel,
            &buyer(),
            None,
            None,
            None,
            false,
            None,
            &accounts,
        )
        .expect("Angebot-Default Regel");
        ok(&pdf, "angebot-default Regel");
    }
}
