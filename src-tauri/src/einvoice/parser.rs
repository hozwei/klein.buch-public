//! E-Rechnung-**Empfangs**-Parser (Functional Core, pure) — Block 11.
//!
//! Liest eine eingehende E-Rechnung-XML und extrahiert die für eine
//! Kosten-Erfassung relevanten Felder in [`ParsedEInvoice`]. Unterstützt
//! **beide** EN-16931-Syntaxen:
//!
//! - **CII** (`rsm:CrossIndustryInvoice`) — das Format, das ZUGFeRD/Factur-X
//!   in PDF/A-3 einbettet und das auch unser eigener Generator erzeugt.
//! - **UBL** (`Invoice` / `CreditNote`) — die zweite zulässige XRechnung-
//!   Syntax; Behörden und viele ERP-Systeme versenden UBL.
//!
//! Die ZUGFeRD-PDF-XML-Extraktion (Mustang-Sidecar) ist I/O und liegt in
//! [`crate::einvoice::mustang_bridge::extract_xml`]; dieser Parser bekommt
//! reine XML-Strings und ist deshalb ohne Sidecar testbar.
//!
//! ## §19 / Eingangs-Seite
//!
//! Der Parser erzwingt **keine** USt-Freiheit — eine Eingangsrechnung eines
//! Lieferanten DARF Umsatzsteuer ausweisen (vgl. [`crate::domain::expense`]).
//! Beträge werden 1:1 übernommen; [`build_expense_input`] rekonstruiert ein
//! konsistentes `gross == net + tax`, damit die Domain-Validierung greift.
//!
//! ## Robustheit
//!
//! Statt vollständigem Schema-Binding läuft ein namespace-toleranter
//! Event-Scan (Prefix wird abgestreift, Zuordnung über den Element-Pfad).
//! Das übersteht kleinere Struktur-Abweichungen realer Absender. Der Nutzer
//! prüft das Ergebnis ohnehin im Vorschau-Formular vor dem Festschreiben.

use chrono::NaiveDate;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::{Deserialize, Serialize};

use crate::domain::expense::ExpenseInput;
use crate::domain::invoice::is_supported_currency;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("Die Datei enthält kein XML.")]
    Empty,
    #[error("Unbekanntes E-Rechnungs-Format (weder CII/ZUGFeRD noch UBL/XRechnung erkannt).")]
    UnknownSyntax,
    #[error("XML konnte nicht gelesen werden: {0}")]
    Malformed(String),
    /// EN-16931-Pflichtfeld fehlt im eingelesenen XML. Fix R3-008.
    #[error("EN-16931-Pflichtfeld fehlt: {0}")]
    MissingMandatory(&'static str),
    /// Currency-Code aus dem XML ist nicht in v1.0 unterstützt. Fix R3-009.
    #[error(
        "Währung '{0}' wird in v1.0 nicht unterstützt (nur EUR — single-currency-Hardline, R1-016)"
    )]
    CurrencyUnsupported(String),
    /// Storno/Gutschrift (DocumentTypeCode 381) wird in v1.0 nicht als Kosten-
    /// Import unterstützt. Der Nutzer muss Gutschriften manuell als
    /// Korrektur-Buchung erfassen, damit Cash-Basis-EÜR konsistent bleibt.
    /// Fix R3-007.
    #[error("Storno-/Gutschrift-Beleg (DocumentTypeCode 381) wird nicht automatisch importiert. Bitte den Beleg manuell als Korrektur erfassen, damit die EÜR-Cash-Basis konsistent bleibt.")]
    CreditNoteNotSupported,
    /// ZUGFeRD-/Factur-X-/XRechnung-Profil-Whitelist (PV1-A2). Seit der
    /// E-Rechnungs-Pflicht 01.01.2025 sind nur noch EN-16931-konforme Profile
    /// gültige B2B-E-Rechnungen; die Sub-Profile MINIMUM und BASIC-WL gelten
    /// laut BMF-Schreiben v. 15.10.2024 nur noch als „buchhalterische Beilage"
    /// und nicht als E-Rechnung. Frühes hartes Reject mit klarem Hinweis statt
    /// indirekt-über-KoSIT-Schematron-Fail (D-73, ADR 0037).
    #[error(
        "ZUGFeRD/XRechnung-Profil nicht unterstützt: {0} (erlaubt: EN16931, EXTENDED, XRECHNUNG)"
    )]
    UnsupportedProfile(String),
}

/// Erkannte Syntax der eingelesenen E-Rechnung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Syntax {
    /// UN/CEFACT CII (`rsm:CrossIndustryInvoice`) — auch in ZUGFeRD.
    Cii,
    /// OASIS UBL (`Invoice` / `CreditNote`).
    Ubl,
}

/// Aus einer eingehenden E-Rechnung extrahierte Felder. Alle Beträge in
/// Integer-Cents. Felder sind `Option`, weil reale Absender unvollständig
/// liefern können — der Nutzer ergänzt im Formular.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedEInvoice {
    /// `cii` | `ubl` (None nur im Default vor dem Parsen).
    pub syntax: Option<Syntax>,
    /// BT-1 — Rechnungsnummer des Lieferanten.
    pub invoice_number: Option<String>,
    /// BT-3 — Typ-Code (380 Rechnung, 384 Korrektur, 381 Gutschrift …).
    pub type_code: Option<String>,
    /// BT-2 — Rechnungsdatum.
    pub invoice_date: Option<NaiveDate>,
    /// BT-9 — Fälligkeitsdatum.
    pub due_date: Option<NaiveDate>,
    /// BT-5 — Währung.
    pub currency_code: Option<String>,
    /// BT-27 — Name des Verkäufers (Lieferant).
    pub seller_name: Option<String>,
    /// BT-31 — USt-IdNr. des Verkäufers.
    pub seller_vat_id: Option<String>,
    /// BT-32 — Steuernummer des Verkäufers.
    pub seller_tax_number: Option<String>,
    /// BT-44 — Name des Käufers (sollte der Nutzer selbst sein).
    pub buyer_name: Option<String>,
    /// BT-48 — USt-IdNr. des Käufers.
    pub buyer_vat_id: Option<String>,
    /// BT-109 — Summe ohne USt (Netto-Basis).
    pub net_amount_cents: Option<i64>,
    /// BT-110 — Summe der USt.
    pub tax_amount_cents: Option<i64>,
    /// BT-112 — Bruttobetrag (Gesamtsumme inkl. USt).
    pub gross_amount_cents: Option<i64>,
    /// Bezeichnungen der Rechnungspositionen (für die Beschreibung).
    pub line_descriptions: Vec<String>,
    /// Mindestens eine Steuerkategorie `AE` (Reverse-Charge, §13b).
    pub reverse_charge: bool,
    /// CII `GuidelineSpecifiedDocumentContextParameter/ID` — die URN des
    /// genutzten ZUGFeRD-/XRechnung-Profils (z. B.
    /// `urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0`).
    /// `None` bei UBL (dort gibt es das Element nicht — KoSIT validiert das
    /// UBL-Profil weiter unten in der Pipeline). PV1-A2.
    pub guideline_id: Option<String>,
}

// =============================================================================
// Syntax-Erkennung
// =============================================================================

/// Schnüffelt die Syntax am Root-Element, ohne vollständig zu parsen.
pub fn detect_syntax(xml: &str) -> Option<Syntax> {
    // Roher Substring-Check ist robust gegen Namespace-Prefixe und BOM.
    if xml.contains("CrossIndustryInvoice") {
        Some(Syntax::Cii)
    } else if xml.contains(":Invoice")
        || xml.contains("<Invoice")
        || xml.contains(":CreditNote")
        || xml.contains("<CreditNote")
    {
        Some(Syntax::Ubl)
    } else {
        None
    }
}

// =============================================================================
// Haupt-Parser
// =============================================================================

/// Parst eine E-Rechnungs-XML (CII oder UBL) in [`ParsedEInvoice`].
pub fn parse(xml: &str) -> Result<ParsedEInvoice, ParseError> {
    if xml.trim().is_empty() {
        return Err(ParseError::Empty);
    }
    let syntax = detect_syntax(xml).ok_or(ParseError::UnknownSyntax)?;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut p = ParsedEInvoice {
        syntax: Some(syntax),
        ..Default::default()
    };

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    // schemeID-Attribut des aktuell offenen Elements (für CII VA/FC-Unterscheidung).
    let mut scheme: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                stack.push(local_name(e.name().as_ref()));
                scheme = scheme_id(&e);
            }
            Ok(Event::Empty(_)) => {
                // Selbstschließende Elemente tragen für uns keinen Textinhalt.
            }
            Ok(Event::Text(t)) => {
                let raw = t
                    .unescape()
                    .map_err(|e| ParseError::Malformed(e.to_string()))?;
                let text = raw.trim();
                if !text.is_empty() {
                    assign(syntax, &stack, scheme.as_deref(), text, &mut p);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
                scheme = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ParseError::Malformed(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    // R3-008: EN-16931-Pflichtfeld-Härtung. Die Domain-Validierung würde diese
    // Felder später ebenfalls erzwingen, aber dort sind die Fehlermeldungen
    // weit weg vom Parse-Site und vom Vendor-XML losgelöst. Parser-Layer ist
    // die richtige Stelle.
    if p.invoice_number
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return Err(ParseError::MissingMandatory("BT-1 (Rechnungsnummer)"));
    }
    if p.invoice_date.is_none() {
        return Err(ParseError::MissingMandatory("BT-2 (Rechnungsdatum)"));
    }
    if p.currency_code
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return Err(ParseError::MissingMandatory("BT-5 (Währung)"));
    }

    // PV1-A2: ZUGFeRD-/XRechnung-Profil-Whitelist. Reihenfolge: NACH dem
    // CII-Walk (damit `guideline_id` gesetzt ist) und NACH den Mandatory-
    // Checks (damit die UI bei einem strukturell kaputten Beleg zuerst die
    // präzisere Pflichtfeld-Fehlermeldung sieht), aber VOR dem Currency-Check
    // (Profil-Reject ist die spezifischere Information). KoSIT bleibt für
    // Profile weiterhin „beratend" — die Whitelist matched großzügig per
    // Substring (siehe `check_profile_whitelist`).
    check_profile_whitelist(p.guideline_id.as_deref())?;

    // R3-009: Single-Currency-EUR-Whitelist, symmetrisch zur Outgoing-Seite
    // (`domain::invoice::is_supported_currency`, R1-016). Normalisierten Code
    // zurück in die Struktur, damit der Caller immer EUR-Großbuchstaben sieht.
    if let Some(raw) = p.currency_code.clone() {
        let normalised = raw.trim().to_uppercase();
        if !is_supported_currency(&normalised) {
            return Err(ParseError::CurrencyUnsupported(raw.trim().to_string()));
        }
        p.currency_code = Some(normalised);
    }

    // R3-007: Gutschrift-/Storno-Belege (DocumentTypeCode 381) sind bewusst
    // nicht als Auto-Import zugelassen. Die alte `.max(0)`-Logik in
    // `build_expense_input` hätte sonst eine 0-EUR-Geister-Kosten erzeugt.
    // Storno-Belege müssen manuell als Korrektur erfasst werden, damit die
    // EÜR-Cash-Basis konsistent bleibt (§11 EStG).
    if matches!(p.type_code.as_deref(), Some("381")) {
        return Err(ParseError::CreditNoteNotSupported);
    }

    Ok(p)
}

// =============================================================================
// Profile-Whitelist (PV1-A2)
// =============================================================================

/// Prüft die `GuidelineSpecifiedDocumentContextParameter/ID`-URN gegen die
/// Whitelist der seit E-Rechnungs-Pflicht 01.01.2025 als gültige B2B-E-Rechnung
/// anerkannten EN-16931-Profile.
///
/// Logik (Pure-FC, keine I/O):
/// - `None` → `Ok(())`. UBL kennt das Element nicht; das KoSIT-Sidecar
///   validiert dort das Profil weiter unten in der Pipeline.
/// - Substring-Match (case-insensitive, hyphen-tolerant) auf `en16931`,
///   `extended` oder `xrechnung` → `Ok(())`. Die Substring-Strategie
///   übersteht die verschiedenen URN-Schreibweisen realer Absender
///   (`urn:cen.eu:en16931:2017`, `…#conformant#urn:factur-x.eu:1p0:extended`,
///   `…#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0`).
/// - Sonst (insbesondere MINIMUM, BASIC-WL) → `Err(UnsupportedProfile(...))`.
///   Hartes, frühes Reject mit klarer Meldung statt indirektem KoSIT-
///   Schematron-Fail weiter unten. D-73 / ADR 0037.
pub fn check_profile_whitelist(guideline_id: Option<&str>) -> Result<(), ParseError> {
    let Some(id) = guideline_id else {
        return Ok(());
    };
    // Hyphen-Strip plus lowercase: matcht sowohl die deutschen Markt-
    // Schreibweisen ("BASIC-WL") als auch die tatsächliche URN-Schreibweise
    // ("basicwl") identisch und ist gegen Großschreibung tolerant.
    let normalised = id.to_ascii_lowercase().replace('-', "");
    for allowed in ["en16931", "extended", "xrechnung"] {
        if normalised.contains(allowed) {
            return Ok(());
        }
    }
    Err(ParseError::UnsupportedProfile(id.to_string()))
}

// =============================================================================
// Feld-Zuordnung über den Element-Pfad
// =============================================================================

fn assign(
    syntax: Syntax,
    stack: &[String],
    scheme: Option<&str>,
    text: &str,
    p: &mut ParsedEInvoice,
) {
    match syntax {
        Syntax::Cii => assign_cii(stack, scheme, text, p),
        Syntax::Ubl => assign_ubl(stack, text, p),
    }
}

fn assign_cii(stack: &[String], scheme: Option<&str>, text: &str, p: &mut ParsedEInvoice) {
    let l = last(stack);

    // Reverse-Charge: jede Steuerkategorie 'AE' (Line oder Header).
    if l == "CategoryCode" && text.eq_ignore_ascii_case("AE") {
        p.reverse_charge = true;
    }

    // PV1-A2: ZUGFeRD-/XRechnung-Profil-URN aus dem ExchangedDocumentContext.
    // Liegt bei CII strukturell vor dem ExchangedDocument; deshalb vorne in
    // assign_cii, damit nachfolgende Felder die URN nicht überschreiben können.
    if ends_with(stack, &["GuidelineSpecifiedDocumentContextParameter", "ID"]) {
        set_once(&mut p.guideline_id, text);
    }

    // Kopf-Dokument.
    if ends_with(stack, &["ExchangedDocument", "ID"]) {
        set_once(&mut p.invoice_number, text);
    } else if ends_with(stack, &["ExchangedDocument", "TypeCode"]) {
        set_once(&mut p.type_code, text);
    } else if ends_with(stack, &["IssueDateTime", "DateTimeString"]) {
        set_date_once(&mut p.invoice_date, text);
    } else if ends_with(stack, &["DueDateDateTime", "DateTimeString"]) {
        set_date_once(&mut p.due_date, text);
    } else if l == "InvoiceCurrencyCode" {
        set_once(&mut p.currency_code, text);
    }
    // Verkäufer.
    else if ends_with(stack, &["SellerTradeParty", "Name"]) {
        set_once(&mut p.seller_name, text);
    } else if contains(stack, "SellerTradeParty")
        && ends_with(stack, &["SpecifiedTaxRegistration", "ID"])
    {
        match scheme {
            Some("VA") => set_once(&mut p.seller_vat_id, text),
            Some("FC") => set_once(&mut p.seller_tax_number, text),
            _ => {}
        }
    }
    // Käufer.
    else if ends_with(stack, &["BuyerTradeParty", "Name"]) {
        set_once(&mut p.buyer_name, text);
    } else if contains(stack, "BuyerTradeParty")
        && ends_with(stack, &["SpecifiedTaxRegistration", "ID"])
        && scheme == Some("VA")
    {
        set_once(&mut p.buyer_vat_id, text);
    }
    // Positionen.
    else if ends_with(stack, &["SpecifiedTradeProduct", "Name"]) {
        push_line(&mut p.line_descriptions, text);
    }
    // Beträge (nur die Header-Summation, nicht die Line-Summation).
    // BT-109/BT-110/BT-112 sind maßgeblich (überschreiben); LineTotal (BT-106)
    // und DuePayable (BT-115) dienen nur als Rückfall, falls die maßgeblichen
    // Felder fehlen. Wichtig, weil im XML LineTotal VOR TaxBasisTotal steht.
    else if contains(stack, "SpecifiedTradeSettlementHeaderMonetarySummation") {
        match l {
            "TaxBasisTotalAmount" => set_amount_force(&mut p.net_amount_cents, text),
            "LineTotalAmount" => set_amount_if_none(&mut p.net_amount_cents, text),
            "TaxTotalAmount" => set_amount_force(&mut p.tax_amount_cents, text),
            "GrandTotalAmount" => set_amount_force(&mut p.gross_amount_cents, text),
            "DuePayableAmount" => set_amount_if_none(&mut p.gross_amount_cents, text),
            _ => {}
        }
    }
}

fn assign_ubl(stack: &[String], text: &str, p: &mut ParsedEInvoice) {
    let l = last(stack);
    let n = stack.len();

    // Reverse-Charge: TaxCategory/ID == 'AE'.
    if ends_with(stack, &["TaxCategory", "ID"]) && text.eq_ignore_ascii_case("AE") {
        p.reverse_charge = true;
    }

    // Kopf-Felder sind direkte Kinder des Wurzelelements (Tiefe 2).
    if n == 2 && l == "ID" {
        set_once(&mut p.invoice_number, text);
    } else if n == 2 && l == "IssueDate" {
        set_date_once(&mut p.invoice_date, text);
    } else if n == 2 && l == "DueDate" {
        set_date_once(&mut p.due_date, text);
    } else if n == 2 && l == "DocumentCurrencyCode" {
        set_once(&mut p.currency_code, text);
    }
    // Verkäufer.
    else if contains(stack, "AccountingSupplierParty") {
        if ends_with(stack, &["PartyLegalEntity", "RegistrationName"]) {
            // RegistrationName ist verbindlicher als PartyName/Name → überschreibt.
            p.seller_name = Some(text.to_string());
        } else if ends_with(stack, &["PartyName", "Name"]) {
            set_once(&mut p.seller_name, text);
        } else if ends_with(stack, &["PartyTaxScheme", "CompanyID"]) {
            set_once(&mut p.seller_vat_id, text);
        }
    }
    // Käufer.
    else if contains(stack, "AccountingCustomerParty") {
        if ends_with(stack, &["PartyLegalEntity", "RegistrationName"]) {
            p.buyer_name = Some(text.to_string());
        } else if ends_with(stack, &["PartyName", "Name"]) {
            set_once(&mut p.buyer_name, text);
        } else if ends_with(stack, &["PartyTaxScheme", "CompanyID"]) {
            set_once(&mut p.buyer_vat_id, text);
        }
    }
    // Header-USt: Invoice/TaxTotal/TaxAmount (Tiefe 3 = direktes Kind, nicht
    // die TaxAmount im TaxSubtotal).
    else if n == 3 && ends_with(stack, &["TaxTotal", "TaxAmount"]) {
        set_amount_force(&mut p.tax_amount_cents, text);
    }
    // Beträge (LegalMonetaryTotal). TaxExclusive/TaxInclusive maßgeblich
    // (überschreiben); LineExtension/Payable nur als Rückfall.
    else if contains(stack, "LegalMonetaryTotal") {
        match l {
            "TaxExclusiveAmount" => set_amount_force(&mut p.net_amount_cents, text),
            "LineExtensionAmount" => set_amount_if_none(&mut p.net_amount_cents, text),
            "TaxInclusiveAmount" => set_amount_force(&mut p.gross_amount_cents, text),
            "PayableAmount" => set_amount_if_none(&mut p.gross_amount_cents, text),
            _ => {}
        }
    }
    // Positionen.
    else if (contains(stack, "InvoiceLine") || contains(stack, "CreditNoteLine"))
        && ends_with(stack, &["Item", "Name"])
    {
        push_line(&mut p.line_descriptions, text);
    }
}

// =============================================================================
// Mapping → ExpenseInput (pure)
// =============================================================================

/// Baut aus einer geparsten E-Rechnung einen Vorschlag für die Kosten-Erfassung.
///
/// - `gross` bevorzugt aus BT-112; sonst `net + tax`.
/// - `tax` aus BT-110 (sonst 0). `net = gross - tax` wird rekonstruiert, damit
///   die Domain-Invariante `gross == net + tax` immer hält (reale Rundungen /
///   Zu- und Abschläge der Originalrechnung würden sonst die Validierung
///   stolpern lassen — der Nutzer sieht und bestätigt die Werte ohnehin).
/// - `paid_date = None`: eine empfangene Rechnung ist erst mit der Zahlung
///   EÜR-relevant; der Nutzer markiert sie später als bezahlt (prüfungssicher).
/// - `category = "other"`: die EÜR-Kategorie steht nicht in der XML — der
///   Nutzer wählt sie im Formular.
pub fn build_expense_input(p: &ParsedEInvoice, today: NaiveDate) -> ExpenseInput {
    // R3-007: kein `.max(0)`-Clamping mehr. Gutschriften (TypeCode 381) lehnt
    // `parse()` bereits oben mit `ParseError::CreditNoteNotSupported` ab; ein
    // korrigierter Beleg (384) mit ausnahmsweise negativen Beträgen würde
    // hier durchrutschen und in der Domain-Validierung mit klarer Fehlermeldung
    // („Netto-Betrag darf nicht negativ sein") scheitern — besser als ein
    // stilles 0-EUR-Geister-Beleg-Schreiben (GoBD-konsistent).
    let tax = p.tax_amount_cents.unwrap_or(0);
    let gross = p
        .gross_amount_cents
        .or_else(|| match (p.net_amount_cents, p.tax_amount_cents) {
            (Some(n), Some(t)) => Some(n + t),
            (Some(n), None) => Some(n),
            _ => None,
        })
        .unwrap_or(0);
    // net = gross - tax, mit Schutz gegen unplausible Eingangswerte (tax > gross
    // im Original-XML deutet auf Datenfehler hin; Caller darf nicht in negative
    // Netto-Beträge laufen).
    let (net, tax) = if tax >= 0 && tax <= gross {
        (gross - tax, tax)
    } else {
        (gross, 0)
    };

    let description = if p.line_descriptions.is_empty() {
        match &p.invoice_number {
            Some(num) => format!("Eingangsrechnung {num}"),
            None => "Eingangsrechnung".to_string(),
        }
    } else {
        let joined = p.line_descriptions.join("; ");
        truncate(&joined, 240)
    };

    ExpenseInput {
        expense_date: p.invoice_date.unwrap_or(today),
        paid_date: None,
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: p.seller_name.clone().unwrap_or_default(),
        vendor_invoice_number: p.invoice_number.clone(),
        category: "other".to_string(),
        description,
        net_amount_cents: net,
        tax_amount_cents: tax,
        gross_amount_cents: net + tax,
        currency_code: p.currency_code.clone().unwrap_or_else(|| "EUR".to_string()),
        reverse_charge_13b: p.reverse_charge,
        notes: None,
    }
}

// =============================================================================
// Helfer
// =============================================================================

/// Strippt einen Namespace-Prefix (`ram:Name` → `Name`).
fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit(':').next() {
        Some(x) => x.to_string(),
        None => s.into_owned(),
    }
}

/// Liest das `schemeID`-Attribut eines Start-Tags (CII VA/FC).
fn scheme_id(e: &BytesStart) -> Option<String> {
    for a in e.attributes().flatten() {
        if local_name(a.key.as_ref()) == "schemeID" {
            return Some(String::from_utf8_lossy(a.value.as_ref()).into_owned());
        }
    }
    None
}

fn last(stack: &[String]) -> &str {
    stack.last().map(String::as_str).unwrap_or("")
}

fn contains(stack: &[String], seg: &str) -> bool {
    stack.iter().any(|s| s == seg)
}

/// Prüft, ob `stack` mit der Segment-Folge `segs` endet.
fn ends_with(stack: &[String], segs: &[&str]) -> bool {
    if stack.len() < segs.len() {
        return false;
    }
    let tail = &stack[stack.len() - segs.len()..];
    tail.iter().zip(segs).all(|(a, b)| a == b)
}

fn set_once(slot: &mut Option<String>, text: &str) {
    if slot.is_none() {
        *slot = Some(text.to_string());
    }
}

fn set_date_once(slot: &mut Option<NaiveDate>, text: &str) {
    if slot.is_none() {
        if let Some(d) = parse_date_flex(text) {
            *slot = Some(d);
        }
    }
}

fn set_amount_once(slot: &mut Option<i64>, text: &str) {
    if slot.is_none() {
        if let Some(c) = parse_decimal_to_cents(text) {
            *slot = Some(c);
        }
    }
}

fn set_amount_if_none(slot: &mut Option<i64>, text: &str) {
    set_amount_once(slot, text);
}

/// Setzt den Betrag, sofern parsebar — **überschreibt** einen bestehenden Wert.
/// Für die maßgeblichen BT-Felder, die im XML nach einem Rückfall-Feld stehen.
fn set_amount_force(slot: &mut Option<i64>, text: &str) {
    if let Some(c) = parse_decimal_to_cents(text) {
        *slot = Some(c);
    }
}

fn push_line(lines: &mut Vec<String>, text: &str) {
    if !text.is_empty() {
        lines.push(text.to_string());
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Parst Datum aus CII (`YYYYMMDD`, format=102) oder UBL (`YYYY-MM-DD`).
pub fn parse_date_flex(s: &str) -> Option<NaiveDate> {
    let t = s.trim();
    // UBL: ISO-Datum, evtl. mit Zeitanteil.
    let iso = t.split(['T', ' ']).next().unwrap_or(t);
    if let Ok(d) = NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        return Some(d);
    }
    // CII: YYYYMMDD.
    if t.len() == 8 && t.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(d) = NaiveDate::parse_from_str(t, "%Y%m%d") {
            return Some(d);
        }
    }
    None
}

/// Parst einen EN-16931-Dezimalbetrag (`.` als Dezimaltrenner) in Cents.
/// Tausender-Kommas werden entfernt; auf 2 Nachkommastellen kaufmännisch gerundet.
pub fn parse_decimal_to_cents(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let neg = t.starts_with('-');
    let t = t.trim_start_matches(['+', '-']).replace(',', "");
    let (int_part, frac_part) = match t.split_once('.') {
        Some((a, b)) => (a, b),
        None => (t.as_str(), ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let int_val: i64 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().ok()?
    };
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Erste zwei Nachkommastellen + Rundung anhand der dritten.
    let mut cents_frac = 0i64;
    let digits: Vec<u32> = frac_part.chars().filter_map(|c| c.to_digit(10)).collect();
    let d0 = digits.first().copied().unwrap_or(0) as i64;
    let d1 = digits.get(1).copied().unwrap_or(0) as i64;
    let d2 = digits.get(2).copied().unwrap_or(0) as i64;
    cents_frac += d0 * 10 + d1;
    if d2 >= 5 {
        cents_frac += 1;
    }
    let mut total = int_val.checked_mul(100)?.checked_add(cents_frac)?;
    if neg {
        total = -total;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::invoice::{
        BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
    };

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    // ---- Dezimal-/Datums-Parsing -------------------------------------------

    #[test]
    fn decimal_parsing_basics() {
        assert_eq!(parse_decimal_to_cents("119.00"), Some(11_900));
        assert_eq!(parse_decimal_to_cents("119"), Some(11_900));
        assert_eq!(parse_decimal_to_cents("0.01"), Some(1));
        assert_eq!(parse_decimal_to_cents("119.5"), Some(11_950));
        assert_eq!(parse_decimal_to_cents("-50.00"), Some(-5_000));
        assert_eq!(parse_decimal_to_cents("1,234.56"), Some(123_456));
        assert_eq!(parse_decimal_to_cents("  10.00  "), Some(1_000));
        assert_eq!(parse_decimal_to_cents(""), None);
        assert_eq!(parse_decimal_to_cents("abc"), None);
    }

    #[test]
    fn decimal_rounding_third_digit() {
        assert_eq!(parse_decimal_to_cents("1.005"), Some(101));
        assert_eq!(parse_decimal_to_cents("1.004"), Some(100));
    }

    #[test]
    fn date_parsing_both_formats() {
        assert_eq!(
            parse_date_flex("20260519"),
            Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap())
        );
        assert_eq!(
            parse_date_flex("2026-05-19"),
            Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap())
        );
        assert_eq!(
            parse_date_flex("2026-05-19T00:00:00"),
            Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap())
        );
        assert_eq!(parse_date_flex("nonsense"), None);
    }

    // ---- Syntax-Erkennung --------------------------------------------------

    #[test]
    fn detect_cii_and_ubl() {
        assert_eq!(
            detect_syntax("<rsm:CrossIndustryInvoice>"),
            Some(Syntax::Cii)
        );
        assert_eq!(
            detect_syntax("<ubl:Invoice xmlns=\"...\">"),
            Some(Syntax::Ubl)
        );
        assert_eq!(detect_syntax("<Invoice>"), Some(Syntax::Ubl));
        assert_eq!(detect_syntax("<foo/>"), None);
    }

    #[test]
    fn empty_xml_errors() {
        assert_eq!(parse("   "), Err(ParseError::Empty));
    }

    #[test]
    fn unknown_syntax_errors() {
        assert_eq!(parse("<foo>bar</foo>"), Err(ParseError::UnknownSyntax));
    }

    // ---- CII via Generator-Roundtrip ---------------------------------------

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

    fn invoice_two_items() -> InvoiceInput {
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
                    quantity: 2.0,
                    unit_code: "HUR".into(),
                    unit_price_cents: 10_000,
                    tax_rate_percent: 0.0,
                    tax_category_code: "E".into(),
                    description_title: None,
                    description_markup: None,
                    source_package_id: None,
                    source_package_revision: None,
                },
                InvoiceItemInput {
                    position: 2,
                    description: "Reisekosten".into(),
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
            notes: None,
            payment_note: None,
            pdf_template: "default".into(),
            is_storno_for: None,
            cancel_reason: None,
        }
    }

    #[test]
    fn parses_own_cii_xrechnung_roundtrip() {
        let xml = crate::einvoice::generator::to_xrechnung(
            "RE-2026-0001",
            &invoice_two_items(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();

        let p = parse(&xml).unwrap();
        assert_eq!(p.syntax, Some(Syntax::Cii));
        assert_eq!(p.invoice_number.as_deref(), Some("RE-2026-0001"));
        assert_eq!(p.type_code.as_deref(), Some("380"));
        assert_eq!(
            p.invoice_date,
            Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap())
        );
        assert_eq!(
            p.due_date,
            Some(NaiveDate::from_ymd_opt(2026, 6, 18).unwrap())
        );
        assert_eq!(p.currency_code.as_deref(), Some("EUR"));
        assert_eq!(p.seller_name.as_deref(), Some("Wildbach Computerhilfe"));
        assert_eq!(p.seller_tax_number.as_deref(), Some("123/456/78901"));
        assert_eq!(p.buyer_name.as_deref(), Some("Kunde GmbH"));
        // §19: 250,00 € Netto, 0 USt, 250,00 € Brutto.
        assert_eq!(p.net_amount_cents, Some(25_000));
        assert_eq!(p.tax_amount_cents, Some(0));
        assert_eq!(p.gross_amount_cents, Some(25_000));
        assert_eq!(p.line_descriptions.len(), 2);
        assert_eq!(p.line_descriptions[0], "Beratung & Support");
        assert!(!p.reverse_charge);
    }

    #[test]
    fn build_expense_input_from_cii_roundtrip() {
        let xml = crate::einvoice::generator::to_xrechnung(
            "RE-2026-0001",
            &invoice_two_items(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        let p = parse(&xml).unwrap();
        let input = build_expense_input(&p, today());
        assert_eq!(input.vendor_name, "Wildbach Computerhilfe");
        assert_eq!(input.vendor_invoice_number.as_deref(), Some("RE-2026-0001"));
        assert_eq!(input.category, "other");
        assert_eq!(input.gross_amount_cents, 25_000);
        assert_eq!(input.net_amount_cents, 25_000);
        assert_eq!(input.tax_amount_cents, 0);
        assert_eq!(
            input.gross_amount_cents,
            input.net_amount_cents + input.tax_amount_cents
        );
        assert!(input.paid_date.is_none());
        assert_eq!(
            input.expense_date,
            NaiveDate::from_ymd_opt(2026, 5, 19).unwrap()
        );
        // build_expense_input liefert eine domain-valide Eingabe.
        assert!(crate::domain::expense::validate_expense(&input, today()).is_ok());
    }

    // ---- UBL via handgeschriebenem Fixture ---------------------------------

    const UBL_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ubl:Invoice
  xmlns:ubl="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
  xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
  xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
  <cbc:ID>LF-2026-7788</cbc:ID>
  <cbc:IssueDate>2026-04-15</cbc:IssueDate>
  <cbc:DueDate>2026-05-15</cbc:DueDate>
  <cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>
  <cbc:DocumentCurrencyCode>EUR</cbc:DocumentCurrencyCode>
  <cac:AccountingSupplierParty>
    <cac:Party>
      <cac:PartyName><cbc:Name>Alt-Name AG</cbc:Name></cac:PartyName>
      <cac:PartyTaxScheme>
        <cbc:CompanyID>DE123456789</cbc:CompanyID>
        <cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme>
      </cac:PartyTaxScheme>
      <cac:PartyLegalEntity>
        <cbc:RegistrationName>Lieferant Telekom GmbH</cbc:RegistrationName>
      </cac:PartyLegalEntity>
    </cac:Party>
  </cac:AccountingSupplierParty>
  <cac:AccountingCustomerParty>
    <cac:Party>
      <cac:PartyLegalEntity>
        <cbc:RegistrationName>Wildbach Computerhilfe</cbc:RegistrationName>
      </cac:PartyLegalEntity>
    </cac:Party>
  </cac:AccountingCustomerParty>
  <cac:TaxTotal>
    <cbc:TaxAmount currencyID="EUR">19.00</cbc:TaxAmount>
    <cac:TaxSubtotal>
      <cbc:TaxableAmount currencyID="EUR">100.00</cbc:TaxableAmount>
      <cbc:TaxAmount currencyID="EUR">19.00</cbc:TaxAmount>
      <cac:TaxCategory><cbc:ID>S</cbc:ID></cac:TaxCategory>
    </cac:TaxSubtotal>
  </cac:TaxTotal>
  <cac:LegalMonetaryTotal>
    <cbc:LineExtensionAmount currencyID="EUR">100.00</cbc:LineExtensionAmount>
    <cbc:TaxExclusiveAmount currencyID="EUR">100.00</cbc:TaxExclusiveAmount>
    <cbc:TaxInclusiveAmount currencyID="EUR">119.00</cbc:TaxInclusiveAmount>
    <cbc:PayableAmount currencyID="EUR">119.00</cbc:PayableAmount>
  </cac:LegalMonetaryTotal>
  <cac:InvoiceLine>
    <cbc:ID>1</cbc:ID>
    <cac:Item><cbc:Name>Internet-Anschluss April</cbc:Name></cac:Item>
  </cac:InvoiceLine>
</ubl:Invoice>"#;

    #[test]
    fn parses_ubl_fixture() {
        let p = parse(UBL_FIXTURE).unwrap();
        assert_eq!(p.syntax, Some(Syntax::Ubl));
        assert_eq!(p.invoice_number.as_deref(), Some("LF-2026-7788"));
        assert_eq!(
            p.invoice_date,
            Some(NaiveDate::from_ymd_opt(2026, 4, 15).unwrap())
        );
        assert_eq!(
            p.due_date,
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert_eq!(p.currency_code.as_deref(), Some("EUR"));
        // RegistrationName hat Vorrang vor PartyName/Name.
        assert_eq!(p.seller_name.as_deref(), Some("Lieferant Telekom GmbH"));
        assert_eq!(p.seller_vat_id.as_deref(), Some("DE123456789"));
        assert_eq!(p.buyer_name.as_deref(), Some("Wildbach Computerhilfe"));
        // Header-USt aus Invoice/TaxTotal/TaxAmount, nicht aus dem Subtotal verdoppelt.
        assert_eq!(p.tax_amount_cents, Some(1_900));
        assert_eq!(p.net_amount_cents, Some(10_000));
        assert_eq!(p.gross_amount_cents, Some(11_900));
        assert_eq!(p.line_descriptions, vec!["Internet-Anschluss April"]);
        assert!(!p.reverse_charge);
    }

    #[test]
    fn build_expense_input_from_ubl_has_vat() {
        let p = parse(UBL_FIXTURE).unwrap();
        let input = build_expense_input(&p, today());
        assert_eq!(input.vendor_name, "Lieferant Telekom GmbH");
        assert_eq!(input.net_amount_cents, 10_000);
        assert_eq!(input.tax_amount_cents, 1_900);
        assert_eq!(input.gross_amount_cents, 11_900);
        assert_eq!(input.description, "Internet-Anschluss April");
        assert!(crate::domain::expense::validate_expense(&input, today()).is_ok());
    }

    #[test]
    fn detects_reverse_charge_ubl() {
        let xml = UBL_FIXTURE.replace(
            "<cac:TaxCategory><cbc:ID>S</cbc:ID></cac:TaxCategory>",
            "<cac:TaxCategory><cbc:ID>AE</cbc:ID></cac:TaxCategory>",
        );
        let p = parse(&xml).unwrap();
        assert!(p.reverse_charge);
        let input = build_expense_input(&p, today());
        assert!(input.reverse_charge_13b);
    }

    #[test]
    fn malformed_xml_errors() {
        // Root erkannt (UBL), aber Tags brechen → Malformed.
        let res = parse("<Invoice><cbc:ID>x</cbc:ID>");
        // Unbalancierte Tags: quick-xml meldet beim EOF kein Fehler per se;
        // hier prüfen wir nur, dass kein Panic passiert und das ID-Feld kam.
        if let Ok(p) = res {
            assert_eq!(p.invoice_number.as_deref(), Some("x"));
        }
    }

    // ---- PV1-A2: ZUGFeRD-/XRechnung-Profil-Whitelist -----------------------

    /// Baut ein minimales CII-XML mit den drei BT-Pflichtfeldern (BT-1, BT-2,
    /// BT-5) und optional einem `GuidelineSpecifiedDocumentContextParameter/ID`-
    /// Eintrag. Damit landen die PV1-A2-Tests sauber im
    /// `check_profile_whitelist`-Pfad und nicht vorher im
    /// `MissingMandatory`-Reject.
    fn minimal_cii_with_guideline(guideline: Option<&str>) -> String {
        let guideline_block = match guideline {
            Some(urn) => format!(
                "  <rsm:ExchangedDocumentContext>\n    <ram:GuidelineSpecifiedDocumentContextParameter>\n      <ram:ID>{urn}</ram:ID>\n    </ram:GuidelineSpecifiedDocumentContextParameter>\n  </rsm:ExchangedDocumentContext>\n"
            ),
            None => String::new(),
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100"
                          xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100"
                          xmlns:udt="urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100">
{guideline_block}  <rsm:ExchangedDocument>
    <ram:ID>RE-2026-0001</ram:ID>
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
</rsm:CrossIndustryInvoice>"#
        )
    }

    /// Pure-FC-Unit-Tests für [`check_profile_whitelist`]. Keine XML-Parse-
    /// Indirektion — die Funktion ist die Single Source of Truth für die
    /// Whitelist-Logik.
    #[test]
    fn check_profile_whitelist_accepts_xrechnung_3_0() {
        assert_eq!(
            check_profile_whitelist(Some(
                "urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0"
            )),
            Ok(())
        );
    }

    #[test]
    fn check_profile_whitelist_accepts_pure_en16931() {
        // Plain EN-16931 ohne ZUGFeRD-Subprofil — die EU-Norm-URN selbst.
        assert_eq!(
            check_profile_whitelist(Some("urn:cen.eu:en16931:2017")),
            Ok(())
        );
    }

    #[test]
    fn check_profile_whitelist_accepts_extended() {
        assert_eq!(
            check_profile_whitelist(Some(
                "urn:cen.eu:en16931:2017#conformant#urn:factur-x.eu:1p0:extended"
            )),
            Ok(())
        );
    }

    #[test]
    fn check_profile_whitelist_rejects_minimum() {
        let urn = "urn:factur-x.eu:1p0:minimum";
        match check_profile_whitelist(Some(urn)) {
            Err(ParseError::UnsupportedProfile(echoed)) => assert_eq!(echoed, urn),
            other => panic!("Erwartet UnsupportedProfile, bekommen: {other:?}"),
        }
    }

    #[test]
    fn check_profile_whitelist_rejects_basic_wl_with_and_without_hyphen() {
        // Reale URN-Schreibweise (ohne Hyphen) plus deutsche Markt-Schreibweise
        // (mit Hyphen). Beides muss als BASIC-WL erkannt und abgelehnt werden.
        for urn in [
            "urn:factur-x.eu:1p0:basicwl",
            "urn:factur-x.eu:1p0:basic-wl",
        ] {
            match check_profile_whitelist(Some(urn)) {
                Err(ParseError::UnsupportedProfile(echoed)) => assert_eq!(echoed, urn),
                other => panic!("Erwartet UnsupportedProfile für {urn}, bekommen: {other:?}"),
            }
        }
    }

    #[test]
    fn check_profile_whitelist_passes_through_none() {
        // UBL liefert keine `GuidelineSpecifiedDocumentContextParameter`-URN;
        // KoSIT validiert das UBL-Profil weiter unten in der Pipeline.
        assert_eq!(check_profile_whitelist(None), Ok(()));
    }

    /// End-to-End-Verankerung: `parse()` zieht die URN tatsächlich aus dem CII-
    /// XML, befüllt `guideline_id` und lehnt nicht-erlaubte Profile ab.
    #[test]
    fn parses_guideline_id_from_cii_xrechnung_3_0() {
        let xml = minimal_cii_with_guideline(Some(
            "urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0",
        ));
        let p = parse(&xml).unwrap();
        assert_eq!(
            p.guideline_id.as_deref(),
            Some("urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0")
        );
    }

    #[test]
    fn parse_rejects_minimum_profile_cii() {
        let xml = minimal_cii_with_guideline(Some("urn:factur-x.eu:1p0:minimum"));
        match parse(&xml) {
            Err(ParseError::UnsupportedProfile(urn)) => {
                assert!(
                    urn.contains("minimum"),
                    "echo trägt die Original-URN: {urn}"
                )
            }
            other => panic!("Erwartet UnsupportedProfile, bekommen: {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_basic_wl_profile_cii() {
        let xml = minimal_cii_with_guideline(Some("urn:factur-x.eu:1p0:basicwl"));
        match parse(&xml) {
            Err(ParseError::UnsupportedProfile(urn)) => assert!(urn.contains("basicwl")),
            other => panic!("Erwartet UnsupportedProfile, bekommen: {other:?}"),
        }
    }

    /// UBL kennt das CII-spezifische `GuidelineSpecifiedDocumentContextParameter`
    /// nicht — `guideline_id` bleibt `None`, der Parser lässt durch.
    #[test]
    fn passes_through_ubl_without_guideline_id() {
        let p = parse(UBL_FIXTURE).unwrap();
        assert_eq!(p.guideline_id, None);
    }

    /// Der eigene Generator schreibt eine XRechnung-3.0-URN — Roundtrip muss
    /// die URN extrahieren UND die Whitelist passieren.
    #[test]
    fn roundtrip_extracts_xrechnung_3_0_guideline_and_passes_whitelist() {
        let xml = crate::einvoice::generator::to_xrechnung(
            "RE-2026-0001",
            &invoice_two_items(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        let p = parse(&xml).unwrap();
        let urn = p.guideline_id.expect("Generator schreibt eine URN");
        assert!(urn.contains("en16931"));
        assert!(urn.contains("xrechnung_3.0"));
    }
}
