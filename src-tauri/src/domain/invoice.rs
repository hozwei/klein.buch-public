//! Invoice-Domain (Functional Core).
//!
//! ## Inhalte
//!
//! - [`InvoiceInput`] + [`InvoiceItemInput`] — User-Eingabe (vor Lock).
//! - [`SellerView`] + [`BuyerView`] — Snapshots zur Issue-Zeit, kein DB-Row.
//! - [`Totals`] + [`ItemTotals`] — pure Betrags-Aggregation in Cent.
//! - [`compute_totals`] — pro Item Netto/USt/Brutto + Aggregat.
//! - [`validate_for_issue`] — §14 UStG + §19 (über
//!   [`kleinunternehmer::assert_no_vat`]) +
//!   Kleinbetragsrechnung-Modus (§33 UStDV).
//!
//! Alle Funktionen pure, keine I/O. Counter-Allokation und DB-Persistenz
//! liegen in [`crate::db::numbering`] und [`crate::commands::invoices`]
//! (Block 3b).
//!
//! ## Geld-Konvention
//!
//! Alle Beträge in **Integer-Cents**. Multiplikation `quantity * unit_price`
//! rundet `f64`-Ergebnis kaufmännisch (`.round()`) auf den nächsten Cent.
//! Für die Aggregat-Summe wird auf Item-Cent-Werten addiert — kein
//! Round-Half-Even-Drift.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::kleinbetragsrechnung;
use crate::domain::kleinunternehmer::{self, ItemVatCheck, KleinunternehmerStatus, NoVatViolation};
use crate::domain::money;

/// v0.1 ist single-currency EUR (Kleinunternehmer-Fokus, DE-Domäne). Multi-
/// Currency würde FX-Umrechnung + Snapshot-Kurs + zusätzliche §14-Pflichten
/// (Brutto in EUR auf der Rechnung) nach sich ziehen. Bis dahin: harte
/// Whitelist — R1-016 (v2026.5-Re-Review).
pub const SUPPORTED_CURRENCIES: &[&str] = &["EUR"];

/// Prüft, ob der `currency_code` (case-sensitive, trimmed) unterstützt ist.
/// Externe Inputs (E-Rechnung-Empfang) werden in der Shell bereits normalisiert.
pub fn is_supported_currency(code: &str) -> bool {
    SUPPORTED_CURRENCIES.contains(&code)
}

// ---- Input-Typen -----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceDirection {
    /// Ausgangsrechnung (wir schreiben sie). Block 3.
    Issued,
    /// Eingangsrechnung (Lieferant). Block 11.
    Received,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceItemInput {
    pub position: u32,
    pub description: String,
    /// Stück / Stunden / kg — als f64 für Teil-Mengen.
    pub quantity: f64,
    /// EN-16931 / UN/ECE Rec 20 Unit-Code. Default 'C62' (one).
    pub unit_code: String,
    /// Netto-Preis pro Einheit in Cent.
    pub unit_price_cents: i64,
    /// 0.0 bei §19. Sonst 7.0 / 19.0 / etc.
    pub tax_rate_percent: f64,
    /// EN-16931-Tax-Category-Code. 'E' bei §19.
    pub tax_category_code: String,
    /// Positions-Titel bei Paket-/Rich-Positionen — steht in der PDF-Zelle
    /// (Beschreibungs-Spalte), NICHT im XRechnung-XML. `None` = einfache Position.
    /// Block P3 (Migration 0021).
    #[serde(default)]
    pub description_title: Option<String>,
    /// Optionales Markup (Markdown-Subset), das NUR den PDF-Block treibt (volle
    /// Breite, via [`crate::domain::package::to_typst`]). `None` = schmale
    /// Beschreibungs-Zelle wie bisher. Die XRechnung nutzt weiter `description`
    /// (Klartext). Block P3, Migration 0020.
    #[serde(default)]
    pub description_markup: Option<String>,
    /// Soft-Zeiger auf das Quell-Paket (P3-Provenienz). `None` = Custom-Position
    /// oder „Paket angepasst" (entkoppelt). Kein FK.
    #[serde(default)]
    pub source_package_id: Option<String>,
    /// Snapshot der Paket-Revisionsnummer zur Einfüge-Zeit.
    #[serde(default)]
    pub source_package_revision: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceInput {
    pub direction: InvoiceDirection,
    pub invoice_date: NaiveDate,
    pub delivery_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub currency_code: String,
    pub items: Vec<InvoiceItemInput>,
    pub notes: Option<String>,
    /// Optionaler Bezahlt-/Zahlungshinweis — reiner PDF-Text (z. B. „Betrag
    /// dankend bar erhalten am 23.05.2026"). Keine EÜR- und keine
    /// XRechnung-Wirkung; informativer Hinweis auf dem Beleg.
    pub payment_note: Option<String>,
    pub pdf_template: String,
    /// Bei Storno: ID der Original-Rechnung. Sonst `None`.
    pub is_storno_for: Option<String>,
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct SellerView<'a> {
    pub name: &'a str,
    pub street: &'a str,
    pub postal_code: &'a str,
    pub city: &'a str,
    pub country_code: &'a str,
    pub tax_number: Option<&'a str>,
    pub vat_id: Option<&'a str>,
    pub email: &'a str,
    /// Telefon (XRechnung BT-42, Pflicht nach BR-DE-6; Plan W1.11).
    pub phone: Option<&'a str>,
    /// Ansprechpartner (BT-41). `None` → der Verkäufer-Name.
    pub contact_name: Option<&'a str>,
    pub iban: Option<&'a str>,
    pub bic: Option<&'a str>,
    pub is_kleinunternehmer: bool,
    pub waived_since: Option<NaiveDate>,
}

impl<'a> SellerView<'a> {
    pub fn status(&self) -> KleinunternehmerStatus {
        KleinunternehmerStatus {
            is_kleinunternehmer: self.is_kleinunternehmer,
            waived_since: self.waived_since,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuyerView<'a> {
    pub name: &'a str,
    pub street: Option<&'a str>,
    pub postal_code: Option<&'a str>,
    pub city: Option<&'a str>,
    pub country_code: &'a str,
    pub vat_id: Option<&'a str>,
    pub email: Option<&'a str>,
}

// ---- Totals -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemTotals {
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totals {
    pub items: Vec<ItemTotals>,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
}

/// Pure Item- und Aggregats-Berechnung. Keine Validierung — Caller prüft.
///
/// Pro Item: `net = round(quantity * unit_price)` (kaufmännisch). Die USt der
/// Rechnung entsteht **je (Kategorie, Satz)-Gruppe** aus der Summe der
/// Nettobeträge: `round(Σ net × rate / 100)` (EN 16931 BR-CO-17: BT-117 =
/// BT-116 × BT-119 / 100, auf zwei Stellen gerundet; R1-S2-TESTS). So weicht die
/// Steuer einer Rechnung mit vielen Kleinpositionen nicht von der
/// Steueraufschlüsselung im XML ab. `ItemTotals::tax_amount_cents` bleibt die
/// gerundete Steuer der einzelnen Position (nur informativ, nicht summiert).
/// Summen sättigen statt überzulaufen; Eingaben außerhalb der Domänengrenzen
/// ([`money`]) lehnt [`validate_for_issue`] ab (`ItemAmountOutOfRange`,
/// `TotalOutOfRange`), bevor ein Betrag gespeichert wird (ASVS V5.4.3).
pub fn compute_totals(items: &[InvoiceItemInput]) -> Totals {
    totals_from_lines(
        items
            .iter()
            .map(|it| {
                (
                    it.tax_category_code.as_str(),
                    it.quantity,
                    it.unit_price_cents,
                    it.tax_rate_percent,
                )
            })
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

/// Gemeinsamer Kern von [`compute_totals`] (Rechnung) und
/// [`crate::domain::quote::compute_totals`] (Angebot): Zeilen als
/// `(Kategorie, Menge, Einzelpreis, Satz)`.
pub(crate) fn totals_from_lines(lines: &[(&str, f64, i64, f64)]) -> Totals {
    let mut out = Vec::with_capacity(lines.len());
    let mut net_total: i64 = 0;
    for &(_, quantity, unit_price_cents, rate) in lines {
        let net = money::line_net_cents(quantity, unit_price_cents);
        let tax = money::line_tax_cents(net, rate);
        out.push(ItemTotals {
            net_amount_cents: net,
            tax_amount_cents: tax,
            gross_amount_cents: net.saturating_add(tax),
        });
        net_total = net_total.saturating_add(net);
    }
    let tax_total = tax_groups(
        lines
            .iter()
            .zip(out.iter())
            .map(|(&(code, _, _, rate), t)| (code, rate, t.net_amount_cents)),
    )
    .iter()
    .fold(0i64, |acc, g| acc.saturating_add(g.tax_amount_cents));
    Totals {
        items: out,
        net_amount_cents: net_total,
        tax_amount_cents: tax_total,
        gross_amount_cents: net_total.saturating_add(tax_total),
    }
}

/// Eine Zeile der Steueraufschlüsselung (BG-23): Kategorie, Satz, Summe der
/// Nettobeträge (BT-116) und daraus gerundete Steuer (BT-117).
#[derive(Debug, Clone, PartialEq)]
pub struct TaxGroupTotal {
    pub category_code: String,
    pub rate: f64,
    pub taxable_amount_cents: i64,
    pub tax_amount_cents: i64,
}

/// Gruppiert `(Kategorie, Satz, Netto)` je (Kategorie, Satz) und rechnet die
/// Steuer je Gruppe aus der Gruppensumme (BR-CO-17). Reihenfolge stabil nach
/// Kategorie und Satz.
pub fn tax_groups<'a, I>(lines: I) -> Vec<TaxGroupTotal>
where
    I: IntoIterator<Item = (&'a str, f64, i64)>,
{
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, TaxGroupTotal> = BTreeMap::new();
    for (code, rate, net) in lines {
        let key = format!("{code}|{rate:.4}");
        groups
            .entry(key)
            .and_modify(|g| g.taxable_amount_cents = g.taxable_amount_cents.saturating_add(net))
            .or_insert(TaxGroupTotal {
                category_code: code.to_string(),
                rate,
                taxable_amount_cents: net,
                tax_amount_cents: 0,
            });
    }
    groups
        .into_values()
        .map(|mut g| {
            g.tax_amount_cents = money::line_tax_cents(g.taxable_amount_cents, g.rate);
            g
        })
        .collect()
}

// ---- Validation -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum InvoiceValidationError {
    // --- Datum / Struktur ---
    InvoiceDateInFuture {
        invoice_date: NaiveDate,
        today: NaiveDate,
    },
    DueDateBeforeInvoiceDate {
        invoice_date: NaiveDate,
        due_date: NaiveDate,
    },
    DeliveryDateAfterInvoiceDate {
        invoice_date: NaiveDate,
        delivery_date: NaiveDate,
    },
    NoItems,
    CurrencyEmpty,
    /// Währung ist gesetzt, aber nicht in der unterstützten Liste (v0.1: nur `EUR`).
    /// R1-016 (v2026.5-Re-Review): KoSIT würde Non-EUR später blocken; der
    /// Domain-Layer lehnt es vorher ab.
    CurrencyUnsupported(String),
    /// Storno-Beleg wurde mit leerem `is_storno_for`-String konstruiert.
    /// R1-014 (v2026.5-Re-Review): `Option<String>::Some("")` würde als Storno
    /// gelten und alle Preise als negativ erzwingen — silent Verhaltens-Drift.
    StornoIdEmpty,
    /// Gesamtbetrag (brutto) ist nicht positiv. Eine Rechnung über 0 € ist fast
    /// immer ein Eingabefehler. Gilt NICHT für Storno-Belege (die sind negativ).
    TotalNotPositive,

    // --- §14 Pflicht-Angaben ---
    SellerNameMissing,
    SellerAddressIncomplete,
    /// §14 Abs 4 Nr 2 — Steuernummer ODER USt-IdNr. Pflicht (nicht bei
    /// Kleinbetragsrechnung).
    SellerMissingTaxIdAndVatId,

    // --- XRechnung (Plan W1.11) — jede ausgestellte Rechnung bekommt ein
    // XRechnung-XML, auch Kleinbetragsrechnungen. ---
    /// BR-DE-6: Telefon des Verkäufers (BT-42) ist Pflicht.
    XRechnungSellerPhoneMissing,
    /// BR-CO-25: offener Betrag ohne Fälligkeitsdatum (BT-9) und ohne
    /// Zahlungsbedingungen (BT-20) — Zahlungsbedingungen werden nicht erfunden.
    XRechnungDueDateMissing,
    /// BR-CO-26: ohne USt-IdNr. braucht es eine Kennung (BT-29 = Steuernummer).
    /// Nur bei Kleinbetragsrechnungen eigener Fehler, sonst greift
    /// [`Self::SellerMissingTaxIdAndVatId`].
    XRechnungSellerIdMissing,
    BuyerNameMissing,
    /// Empfänger-Adresse Pflicht (nicht bei Kleinbetragsrechnung).
    BuyerAddressIncomplete,

    // --- Item-Ebene ---
    ItemDescriptionMissing(u32),
    ItemQuantityNotPositive(u32),
    ItemUnitPriceNegative(u32),
    ItemDuplicatePosition(u32),
    /// EN-16931 erlaubt nur Codes S, Z, E, AE, K, G, O, L, M.
    ItemInvalidTaxCategoryCode {
        position: u32,
        code: String,
    },
    /// Negative tax_rate.
    ItemTaxRateNegative {
        position: u32,
        rate: f64,
    },
    /// Menge, Einzelpreis oder Steuersatz außerhalb der Domänengrenzen
    /// ([`money`]: Menge endlich, ≤ 1 000 000, höchstens 4 Nachkommastellen;
    /// |Einzelpreis| ≤ 1 Mrd. €; Steuersatz ≤ 100 %). ASVS V5.4.3.
    ItemAmountOutOfRange(u32),
    /// Die Summe der Positionen verließe den darstellbaren Bereich.
    TotalOutOfRange,
    /// Kategorie 'E' (steuerbefreit) ohne §19: Klein.Buch kennt keine
    /// Steuerbefreiungen nach §4 UStG; der Befreiungsgrund (BT-120, BR-E-10)
    /// wäre sonst der §19-Text (KB-0014).
    ItemExemptWithoutKleinunternehmer(u32),

    // --- §19 — siehe kleinunternehmer::assert_no_vat ---
    Paragraph19VatViolation(Vec<NoVatViolation>),
}

/// Datum in Meldungen nach DIN 5008 (TT.MM.JJJJ, U4).
pub(crate) fn de_date(d: &NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// Text zu `Paragraph19VatViolation` (Rechnung und Angebot).
pub const PARAGRAPH19_VAT_MESSAGE: &str = "Als Kleinunternehmer (§19 UStG) darf keine \
     Umsatzsteuer ausgewiesen werden — bearbeite den Entwurf, damit alle Positionen 0 % haben.";

/// Firmendaten (Verkäuferprofil) fehlen ganz — Belege, Prüfung und Mails brauchen sie.
pub const SELLER_PROFILE_MISSING: &str =
    "Die Firmendaten fehlen noch — trage sie unter Einstellungen → Meine Firmendaten ein.";

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI). Der maschinenlesbare
/// Code lebt in `commands::invoices::variant_name`.
///
/// U4 (V1-03, V1-18): Begriffe der Oberfläche („Firmendaten“ statt „Stammdaten“,
/// Seite „Meine Firmendaten“), Daten als TT.MM.JJJJ, keine Regel-IDs der
/// XRechnung im Text (sie stehen im Prüfbericht); gesetzliche Fundstellen bleiben.
pub fn message(e: &InvoiceValidationError) -> String {
    use InvoiceValidationError as E;
    match e {
        E::InvoiceDateInFuture {
            invoice_date,
            today,
        } => format!(
            "Das Rechnungsdatum ({}) liegt in der Zukunft (heute ist der {}).",
            de_date(invoice_date),
            de_date(today)
        ),
        E::DueDateBeforeInvoiceDate {
            invoice_date,
            due_date,
        } => format!(
            "Das Fälligkeitsdatum ({}) liegt vor dem Rechnungsdatum ({}).",
            de_date(due_date),
            de_date(invoice_date)
        ),
        E::DeliveryDateAfterInvoiceDate {
            invoice_date,
            delivery_date,
        } => format!(
            "Das Leistungsdatum ({}) liegt nach dem Rechnungsdatum ({}).",
            de_date(delivery_date),
            de_date(invoice_date)
        ),
        E::NoItems => "Mindestens eine Position ist erforderlich.".into(),
        E::CurrencyEmpty => "Die Währung fehlt.".into(),
        E::CurrencyUnsupported(code) => {
            format!("Die Währung „{code}“ wird nicht unterstützt; Klein.Buch rechnet nur in Euro (EUR).")
        }
        E::StornoIdEmpty => {
            "Der Storno-Beleg verweist auf keine Rechnung (interner Fehler, bitte melden).".into()
        }
        E::TotalNotPositive => "Der Gesamtbetrag muss größer als 0 € sein.".into(),
        E::SellerNameMissing => "Der Firmenname fehlt in den Firmendaten.".into(),
        E::SellerAddressIncomplete => {
            "Die Adresse in den Firmendaten ist unvollständig (Straße, PLZ, Ort).".into()
        }
        E::SellerMissingTaxIdAndVatId => {
            "Steuernummer oder USt-IdNr. fehlt in den Firmendaten (Pflichtangabe nach §14 Abs. 4 UStG).".into()
        }
        E::XRechnungSellerPhoneMissing => {
            "Die Telefonnummer fehlt in den Firmendaten — für die E-Rechnung ist sie Pflicht.".into()
        }
        E::XRechnungDueDateMissing => {
            "Das Fälligkeitsdatum fehlt — für die E-Rechnung ist es Pflicht, solange ein Betrag offen ist.".into()
        }
        E::XRechnungSellerIdMissing => {
            "Steuernummer oder USt-IdNr. fehlt in den Firmendaten — für die E-Rechnung ist eine davon Pflicht, auch bei Kleinbetragsrechnungen.".into()
        }
        E::BuyerNameMissing => "Der Name des Empfängers fehlt im Kontakt.".into(),
        E::BuyerAddressIncomplete => {
            "Die Adresse des Empfängers ist unvollständig (Straße, PLZ, Ort) — ergänze sie im Kontakt.".into()
        }
        E::ItemDescriptionMissing(p) => format!("Position {p}: Die Beschreibung fehlt."),
        E::ItemQuantityNotPositive(p) => format!("Position {p}: Die Menge muss größer als 0 sein."),
        E::ItemUnitPriceNegative(p) => {
            format!("Position {p}: Der Einzelpreis darf nicht negativ sein.")
        }
        E::ItemDuplicatePosition(p) => format!("Position {p} ist doppelt vergeben."),
        E::ItemInvalidTaxCategoryCode { position, code } => {
            format!("Position {position}: Die Steuerart „{code}“ ist ungültig.")
        }
        E::ItemTaxRateNegative { position, rate } => {
            format!("Position {position}: Der Steuersatz darf nicht negativ sein ({rate} %).")
        }
        E::ItemAmountOutOfRange(p) => item_amount_out_of_range_message(*p),
        E::TotalOutOfRange => TOTAL_OUT_OF_RANGE_MESSAGE.into(),
        E::ItemExemptWithoutKleinunternehmer(p) => exempt_without_klein_message(*p),
        E::Paragraph19VatViolation(_) => PARAGRAPH19_VAT_MESSAGE.into(),
    }
}

/// Text zu `ItemAmountOutOfRange` (Rechnung und Angebot).
pub fn item_amount_out_of_range_message(position: u32) -> String {
    format!(
        "Position {position}: Menge (höchstens 1.000.000, bis zu 4 Nachkommastellen), \
         Einzelpreis (höchstens 1 Mrd. €) oder Steuersatz (höchstens 100 %) liegt außerhalb \
         des zulässigen Bereichs."
    )
}

/// Text zu `TotalOutOfRange` (Rechnung und Angebot).
pub const TOTAL_OUT_OF_RANGE_MESSAGE: &str =
    "Die Summe der Positionen ist zu groß und lässt sich nicht verbuchen.";

/// Text zu `ItemExemptWithoutKleinunternehmer` (Rechnung und Angebot).
pub fn exempt_without_klein_message(position: u32) -> String {
    format!(
        "Position {position}: Steuerart E (steuerbefreit) gibt es nur als Kleinunternehmer (§19). \
         Bei Regelbesteuerung bitte Steuerart und Steuersatz der Position setzen (z. B. S, 19 %)."
    )
}

/// Domänengrenzen einer Position, ohne die Fälle, die schon eigene Fehler
/// haben (Menge ≤ 0/NaN, negativer Steuersatz).
pub(crate) fn item_out_of_range(quantity: f64, unit_price_cents: i64, rate: f64) -> bool {
    let quantity_bad = quantity > 0.0 && !money::quantity_in_range(quantity);
    let rate_bad = (rate.is_nan() || rate >= 0.0) && !money::tax_rate_in_range(rate);
    quantity_bad || rate_bad || !money::unit_price_in_range(unit_price_cents)
}

/// Validiert eine [`InvoiceInput`] gegen §14 UStG + §19 + §33 UStDV.
///
/// `today` ist injizierbar — Tests pinnen das Datum, Production reicht
/// `crate::clock::today_berlin()` (Europe/Berlin) rein.
///
/// **Aggregiert** alle Fehler; gibt nicht beim ersten Fail auf. Wenn Vec
/// leer → Rechnung darf in `lock_and_issue` weiter.
pub fn validate_for_issue(
    input: &InvoiceInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    today: NaiveDate,
) -> Result<(), Vec<InvoiceValidationError>> {
    use InvoiceValidationError as E;
    let mut errs = Vec::new();

    // --- Struktur ---
    if input.items.is_empty() {
        errs.push(E::NoItems);
    }
    let currency = input.currency_code.trim();
    if currency.is_empty() {
        errs.push(E::CurrencyEmpty);
    } else if !is_supported_currency(currency) {
        // R1-016: Whitelist statt is_empty()-Check (v2026.5-Re-Review).
        errs.push(E::CurrencyUnsupported(currency.to_string()));
    }
    // R1-014: Storno-ID darf nicht der leere String sein. Some("") würde silent
    // als Storno gewertet (alle Preise müssen negativ sein) und das Verhalten
    // verbiegen.
    if let Some(sid) = input.is_storno_for.as_deref() {
        if sid.trim().is_empty() {
            errs.push(E::StornoIdEmpty);
        }
    }
    if input.invoice_date > today {
        errs.push(E::InvoiceDateInFuture {
            invoice_date: input.invoice_date,
            today,
        });
    }
    if let Some(due) = input.due_date {
        if due < input.invoice_date {
            errs.push(E::DueDateBeforeInvoiceDate {
                invoice_date: input.invoice_date,
                due_date: due,
            });
        }
    }
    if let Some(deliv) = input.delivery_date {
        if deliv > input.invoice_date {
            errs.push(E::DeliveryDateAfterInvoiceDate {
                invoice_date: input.invoice_date,
                delivery_date: deliv,
            });
        }
    }

    // --- Items ---
    let mut seen_positions = std::collections::HashSet::new();
    const VALID_TAX_CODES: &[&str] = &["S", "Z", "E", "AE", "K", "G", "O", "L", "M"];
    let is_storno = input.is_storno_for.is_some();
    for it in &input.items {
        if it.description.trim().is_empty() {
            errs.push(E::ItemDescriptionMissing(it.position));
        }
        if it.quantity.is_nan() || it.quantity <= 0.0 {
            // NaN/0/negativ → alle nicht-positiv
            errs.push(E::ItemQuantityNotPositive(it.position));
        }
        // Storno-Belege haben qua Konstruktion negative Preise — domain::storno
        // negiert das Original. Sanity-Check: bei Nicht-Storno muss positiv,
        // bei Storno muss strikt negativ sein.
        if !is_storno && it.unit_price_cents < 0 {
            errs.push(E::ItemUnitPriceNegative(it.position));
        }
        if is_storno && it.unit_price_cents > 0 {
            errs.push(E::ItemUnitPriceNegative(it.position));
        }
        if it.tax_rate_percent < 0.0 {
            errs.push(E::ItemTaxRateNegative {
                position: it.position,
                rate: it.tax_rate_percent,
            });
        }
        if !VALID_TAX_CODES.contains(&it.tax_category_code.as_str()) {
            errs.push(E::ItemInvalidTaxCategoryCode {
                position: it.position,
                code: it.tax_category_code.clone(),
            });
        }
        if !seen_positions.insert(it.position) {
            errs.push(E::ItemDuplicatePosition(it.position));
        }
        if item_out_of_range(it.quantity, it.unit_price_cents, it.tax_rate_percent) {
            errs.push(E::ItemAmountOutOfRange(it.position));
        }
        // KB-0014: 'E' nur mit §19 (sonst trüge BT-120 den §19-Text).
        if !seller.is_kleinunternehmer && it.tax_category_code == kleinunternehmer::EXEMPT_CATEGORY
        {
            errs.push(E::ItemExemptWithoutKleinunternehmer(it.position));
        }
    }
    // Summen ohne Überlauf (ASVS V5.4.3) — nur sinnvoll, wenn jede Position in
    // den Grenzen liegt (sonst steht schon ItemAmountOutOfRange da).
    let items_in_range = !errs.iter().any(|e| {
        matches!(
            e,
            E::ItemAmountOutOfRange(_)
                | E::ItemQuantityNotPositive(_)
                | E::ItemTaxRateNegative { .. }
        )
    });
    if items_in_range
        && money::checked_document_totals(
            input
                .items
                .iter()
                .map(|it| (it.quantity, it.unit_price_cents, it.tax_rate_percent)),
        )
        .is_none()
    {
        errs.push(E::TotalOutOfRange);
    }

    // --- §19 (§14c-Schutz) ---
    let checks: Vec<ItemVatCheck> = input
        .items
        .iter()
        .map(|it| ItemVatCheck {
            position: it.position,
            tax_category_code: it.tax_category_code.as_str(),
            tax_amount_cents: compute_totals(std::slice::from_ref(it)).tax_amount_cents,
            tax_rate_percent: it.tax_rate_percent,
        })
        .collect();
    if let Err(viol) = kleinunternehmer::assert_no_vat(&seller.status(), &checks) {
        errs.push(E::Paragraph19VatViolation(viol));
    }

    // --- §14 Pflichtangaben ---
    if seller.name.trim().is_empty() {
        errs.push(E::SellerNameMissing);
    }
    if seller.street.trim().is_empty()
        || seller.postal_code.trim().is_empty()
        || seller.city.trim().is_empty()
    {
        errs.push(E::SellerAddressIncomplete);
    }
    if buyer.name.trim().is_empty() {
        errs.push(E::BuyerNameMissing);
    }
    // XRechnung BR-DE-6 (BG-6 Seller Contact): Telefon ist Pflicht und wird
    // nicht erfunden (Plan W1.11).
    if seller.phone.map(|s| s.trim().is_empty()).unwrap_or(true) {
        errs.push(E::XRechnungSellerPhoneMissing);
    }
    let has_seller_id = seller
        .tax_number
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
        || seller.vat_id.map(|s| !s.trim().is_empty()).unwrap_or(false);

    // Kleinbetrag-Modus: nutzt brutto-Gesamtsumme aus compute_totals.
    let totals = compute_totals(&input.items);

    // XRechnung BR-CO-25 (W11-Fund mit dem echten KoSIT): ist ein Betrag offen,
    // braucht es ein Fälligkeitsdatum (Zahlungsbedingungen erfinden wir nicht).
    if !is_storno && input.due_date.is_none() && totals.gross_amount_cents > 0 {
        errs.push(E::XRechnungDueDateMissing);
    }

    // Gesamtbetrag muss positiv sein — außer bei Storno-Belegen (negativ per
    // Konstruktion). `NoItems` deckt den leeren Fall bereits ab.
    if !is_storno && !input.items.is_empty() && totals.gross_amount_cents <= 0 {
        errs.push(E::TotalNotPositive);
    }

    let kleinbetrag = kleinbetragsrechnung::is_applicable(totals.gross_amount_cents);
    if kleinbetrag && !has_seller_id {
        errs.push(E::XRechnungSellerIdMissing);
    }

    if !kleinbetrag {
        // §14 Abs 4 Nr 2: Steuernummer ODER USt-IdNr.
        let has_tax = seller
            .tax_number
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let has_vat = seller.vat_id.map(|s| !s.trim().is_empty()).unwrap_or(false);
        if !has_tax && !has_vat {
            errs.push(E::SellerMissingTaxIdAndVatId);
        }
        // Empfänger-Adresse Pflicht.
        let addr_ok = buyer.street.map(|s| !s.trim().is_empty()).unwrap_or(false)
            && buyer
                .postal_code
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            && buyer.city.map(|s| !s.trim().is_empty()).unwrap_or(false);
        if !addr_ok {
            errs.push(E::BuyerAddressIncomplete);
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Bequemer Check, ob die Rechnung im milderen §33-Modus ist.
pub fn is_kleinbetragsrechnung(input: &InvoiceInput) -> bool {
    let totals = compute_totals(&input.items);
    kleinbetragsrechnung::is_applicable(totals.gross_amount_cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seller_klein() -> SellerView<'static> {
        SellerView {
            name: "Beispiel IT-Service",
            street: "Beispielweg 1",
            postal_code: "84028",
            city: "Landshut",
            country_code: "DE",
            tax_number: Some("123/456/78901"),
            vat_id: None,
            email: "inhaber@example.org",
            phone: Some("+49 000 000000"),
            contact_name: None,
            iban: None,
            bic: None,
            is_kleinunternehmer: true,
            waived_since: None,
        }
    }

    fn buyer_full() -> BuyerView<'static> {
        BuyerView {
            name: "Kunde GmbH",
            street: Some("Hauptstr. 7"),
            postal_code: Some("80331"),
            city: Some("München"),
            country_code: "DE",
            vat_id: Some("DE111111111"),
            email: Some("info@kunde.example.org"),
        }
    }

    fn item(position: u32, qty: f64, price: i64) -> InvoiceItemInput {
        InvoiceItemInput {
            position,
            description: format!("Position {position}"),
            quantity: qty,
            unit_code: "C62".into(),
            unit_price_cents: price,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }
    }

    fn good_invoice() -> InvoiceInput {
        InvoiceInput {
            direction: InvoiceDirection::Issued,
            invoice_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
            delivery_date: Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap()),
            due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()),
            currency_code: "EUR".into(),
            items: vec![item(1, 1.0, 50_000)], // 500,00 € — keine Kleinbetrag
            notes: None,
            payment_note: None,
            pdf_template: "default".into(),
            is_storno_for: None,
            cancel_reason: None,
        }
    }

    #[test]
    fn compute_totals_simple_klein() {
        let items = vec![item(1, 2.0, 10_000), item(2, 1.0, 5_000)];
        let t = compute_totals(&items);
        assert_eq!(t.items.len(), 2);
        assert_eq!(t.items[0].net_amount_cents, 20_000);
        assert_eq!(t.items[0].tax_amount_cents, 0);
        assert_eq!(t.items[0].gross_amount_cents, 20_000);
        assert_eq!(t.net_amount_cents, 25_000);
        assert_eq!(t.tax_amount_cents, 0);
        assert_eq!(t.gross_amount_cents, 25_000);
    }

    #[test]
    fn compute_totals_with_vat_19() {
        let mut it = item(1, 1.0, 10_000); // 100,00 € netto
        it.tax_rate_percent = 19.0;
        it.tax_category_code = "S".into();
        let t = compute_totals(&[it]);
        assert_eq!(t.net_amount_cents, 10_000);
        assert_eq!(t.tax_amount_cents, 1_900); // 19,00 €
        assert_eq!(t.gross_amount_cents, 11_900);
    }

    #[test]
    fn validate_passes_for_good_invoice() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let r = validate_for_issue(&good_invoice(), &seller_klein(), &buyer_full(), today);
        assert!(r.is_ok(), "expected ok, got {r:?}");
    }

    #[test]
    fn validate_flags_no_items() {
        let mut inv = good_invoice();
        inv.items.clear();
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err.contains(&InvoiceValidationError::NoItems));
    }

    #[test]
    fn validate_flags_zero_total() {
        // Eine Position mit Preis 0 → Gesamt 0 € → muss blocken (keine Nullrechnung).
        let mut inv = good_invoice();
        inv.items = vec![item(1, 1.0, 0)];
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err.contains(&InvoiceValidationError::TotalNotPositive));
    }

    #[test]
    fn validate_flags_invoice_date_in_future() {
        let mut inv = good_invoice();
        inv.invoice_date = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, InvoiceValidationError::InvoiceDateInFuture { .. })));
    }

    #[test]
    fn validate_flags_due_before_invoice() {
        let mut inv = good_invoice();
        inv.due_date = Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, InvoiceValidationError::DueDateBeforeInvoiceDate { .. })));
    }

    #[test]
    fn validate_requires_due_date_for_open_amount() {
        let mut inv = good_invoice();
        inv.due_date = None;
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let err = validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).unwrap_err();
        assert!(
            err.contains(&InvoiceValidationError::XRechnungDueDateMissing),
            "{err:?}"
        );
    }

    #[test]
    fn validate_flags_paragraph_19_vat_violation() {
        // §19 aktiv (default) aber Items mit Steuersatz 19% / Code S
        let mut inv = good_invoice();
        inv.items[0].tax_rate_percent = 19.0;
        inv.items[0].tax_category_code = "S".into();
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, InvoiceValidationError::Paragraph19VatViolation(_))));
    }

    #[test]
    fn validate_kleinbetrag_drops_tax_id_and_buyer_address_requirement() {
        // Brutto 200 € → Kleinbetrag: §33 UStDV verlangt weder Steuernummer noch
        // Empfänger-Adresse. Die XRechnung (BR-CO-26) braucht aber eine
        // Verkäufer-Kennung — das ist seit Plan W1.11 der einzige Fehler.
        let mut inv = good_invoice();
        inv.items = vec![item(1, 1.0, 20_000)]; // 200,00 €
        let mut s = seller_klein();
        s.tax_number = None;
        s.vat_id = None;
        let mut b = buyer_full();
        b.street = None;
        b.postal_code = None;
        b.city = None;
        let err = validate_for_issue(&inv, &s, &b, NaiveDate::from_ymd_opt(2026, 5, 20).unwrap())
            .unwrap_err();
        assert_eq!(err, vec![InvoiceValidationError::XRechnungSellerIdMissing]);

        // Mit Steuernummer ist die Kleinbetragsrechnung ohne Empfänger-Adresse ok.
        s.tax_number = Some("123/456/78901");
        let r = validate_for_issue(&inv, &s, &b, NaiveDate::from_ymd_opt(2026, 5, 20).unwrap());
        assert!(r.is_ok(), "kleinbetrag soll milder sein, got {r:?}");
    }

    /// Plan W1.11: ohne Telefon kein Festschreiben (XRechnung BR-DE-6), ohne
    /// erfundene Ersatzangabe.
    #[test]
    fn validate_requires_seller_phone_for_xrechnung() {
        let mut s = seller_klein();
        s.phone = None;
        let err = validate_for_issue(
            &good_invoice(),
            &s,
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            vec![InvoiceValidationError::XRechnungSellerPhoneMissing]
        );
        s.phone = Some("   ");
        let err = validate_for_issue(
            &good_invoice(),
            &s,
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            vec![InvoiceValidationError::XRechnungSellerPhoneMissing]
        );
    }

    #[test]
    fn validate_non_kleinbetrag_requires_tax_id_and_buyer_address() {
        let inv = good_invoice(); // 500 € → kein Kleinbetrag
        let mut s = seller_klein();
        s.tax_number = None;
        s.vat_id = None;
        let mut b = buyer_full();
        b.street = None;
        let err = validate_for_issue(&inv, &s, &b, NaiveDate::from_ymd_opt(2026, 5, 20).unwrap())
            .unwrap_err();
        assert!(err.contains(&InvoiceValidationError::SellerMissingTaxIdAndVatId));
        assert!(err.contains(&InvoiceValidationError::BuyerAddressIncomplete));
    }

    #[test]
    fn validate_flags_duplicate_position() {
        let mut inv = good_invoice();
        inv.items.push(item(1, 1.0, 10_000));
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err.contains(&InvoiceValidationError::ItemDuplicatePosition(1)));
    }

    #[test]
    fn validate_flags_invalid_tax_category() {
        let mut inv = good_invoice();
        inv.items[0].tax_category_code = "XX".into();
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, InvoiceValidationError::ItemInvalidTaxCategoryCode { .. })));
    }

    #[test]
    fn is_kleinbetragsrechnung_detects_threshold() {
        let mut inv = good_invoice();
        inv.items = vec![item(1, 1.0, 25_000)]; // 250 €
        assert!(is_kleinbetragsrechnung(&inv));
        inv.items = vec![item(1, 1.0, 25_001)];
        assert!(!is_kleinbetragsrechnung(&inv));
    }

    // ---- R1-014 / R1-016 (v2026.5-Re-Review) ------------------------------

    #[test]
    fn is_supported_currency_accepts_eur_only() {
        assert!(is_supported_currency("EUR"));
        assert!(!is_supported_currency("USD"));
        assert!(!is_supported_currency("eur")); // case-sensitive
        assert!(!is_supported_currency(""));
        assert!(!is_supported_currency("EU"));
    }

    #[test]
    fn validate_flags_currency_unsupported() {
        // R1-016: Non-EUR muss als Domain-Fehler blocken (nicht erst KoSIT).
        let mut inv = good_invoice();
        inv.currency_code = "USD".into();
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(
            err.iter()
                .any(|e| matches!(e, InvoiceValidationError::CurrencyUnsupported(c) if c == "USD")),
            "expected CurrencyUnsupported(USD), got {err:?}"
        );
    }

    #[test]
    fn validate_currency_empty_takes_precedence_over_unsupported() {
        let mut inv = good_invoice();
        inv.currency_code = "  ".into();
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err.contains(&InvoiceValidationError::CurrencyEmpty));
        assert!(!err
            .iter()
            .any(|e| matches!(e, InvoiceValidationError::CurrencyUnsupported(_))));
    }

    #[test]
    fn validate_flags_empty_storno_id_string() {
        // R1-014: Some("") als is_storno_for würde silent Storno-Modus aktivieren.
        let mut inv = good_invoice();
        inv.is_storno_for = Some("".into());
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(
            err.contains(&InvoiceValidationError::StornoIdEmpty),
            "expected StornoIdEmpty in errors, got {err:?}"
        );
    }

    #[test]
    fn validate_flags_whitespace_only_storno_id() {
        let mut inv = good_invoice();
        inv.is_storno_for = Some("   ".into());
        let err = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap_err();
        assert!(err.contains(&InvoiceValidationError::StornoIdEmpty));
    }

    #[test]
    fn validate_accepts_real_storno_id_with_negative_prices() {
        // Sanity: Some("…echte uuid…") schaltet legitim in den Storno-Modus,
        // in dem unit_price_cents < 0 sein müssen.
        let mut inv = good_invoice();
        inv.is_storno_for = Some("019283ab-1234-7abc-9000-abcdef012345".into());
        inv.items[0].unit_price_cents = -inv.items[0].unit_price_cents;
        // Storno hat negativen Gesamtbetrag — TotalNotPositive feuert NICHT für Storno.
        let r = validate_for_issue(
            &inv,
            &seller_klein(),
            &buyer_full(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        );
        assert!(r.is_ok(), "echter Storno-ID darf nicht blocken, got {r:?}");
    }

    fn vat_item(position: u32, qty: f64, price: i64, rate: f64) -> InvoiceItemInput {
        let mut it = item(position, qty, price);
        it.tax_rate_percent = rate;
        it.tax_category_code = if rate > 0.0 { "S".into() } else { "Z".into() };
        it
    }

    fn seller_regel() -> SellerView<'static> {
        SellerView {
            is_kleinunternehmer: false,
            waived_since: NaiveDate::from_ymd_opt(2026, 1, 1),
            ..seller_klein()
        }
    }

    // KB-0008: Bruchteil-Mengen und USt-Rundung (kaufmännisch, je Position bzw.
    // je Steuergruppe).
    #[test]
    fn compute_totals_fractional_quantity_rounds_half_away_from_zero() {
        // 0,333 × 3,33 € = 1,10889 € → 1,11 €; USt 19 % auf 1,11 € = 0,2109 → 0,21 €.
        let t = compute_totals(&[vat_item(1, 0.333, 333, 19.0)]);
        assert_eq!(t.net_amount_cents, 111);
        assert_eq!(t.tax_amount_cents, 21);
        assert_eq!(t.gross_amount_cents, 132);
        // 0,5 × 1 ct = 0,5 ct → 1 ct (half away from zero), USt 19 % → 0 ct.
        let t = compute_totals(&[vat_item(1, 0.5, 1, 19.0)]);
        assert_eq!(t.net_amount_cents, 1);
        // 3 × 3,33 € = 9,99 €; USt 1,8981 € → 1,90 €.
        let t = compute_totals(&[vat_item(1, 3.0, 333, 19.0)]);
        assert_eq!((t.net_amount_cents, t.tax_amount_cents), (999, 190));
        // 7 % auf 0,50 € = 0,035 € → 0,04 € (Halbcent rundet auf).
        let t = compute_totals(&[vat_item(1, 1.0, 50, 7.0)]);
        assert_eq!(t.tax_amount_cents, 4);
    }

    // R1-S2-TESTS: 100 Kleinpositionen — die USt entsteht je Steuergruppe aus
    // der Nettosumme (BR-CO-17), nicht als Summe gerundeter Positionssteuern.
    #[test]
    fn compute_totals_hundred_small_items_do_not_drift() {
        let items: Vec<_> = (1..=100).map(|p| vat_item(p, 1.0, 1, 19.0)).collect();
        let t = compute_totals(&items);
        assert_eq!(t.net_amount_cents, 100);
        // 1,00 € × 19 % = 0,19 € (je Position wären es 100 × 0 ct).
        assert_eq!(t.tax_amount_cents, 19);
        assert_eq!(t.gross_amount_cents, 119);
        assert!(t.items.iter().all(|i| i.tax_amount_cents == 0));
        // Gemischte Sätze: je Gruppe gerundet, dann addiert.
        let mut mixed: Vec<_> = (1..=3).map(|p| vat_item(p, 1.0, 333, 19.0)).collect();
        mixed.extend((4..=6).map(|p| vat_item(p, 1.0, 333, 7.0)));
        let t = compute_totals(&mixed);
        assert_eq!(t.net_amount_cents, 1_998);
        // 9,99 € × 19 % = 1,8981 → 1,90; 9,99 € × 7 % = 0,6993 → 0,70.
        assert_eq!(t.tax_amount_cents, 260);
        let groups = tax_groups(mixed.iter().zip(t.items.iter()).map(|(it, l)| {
            (
                it.tax_category_code.as_str(),
                it.tax_rate_percent,
                l.net_amount_cents,
            )
        }));
        assert_eq!(groups.len(), 2);
        assert_eq!(
            groups.iter().map(|g| g.tax_amount_cents).sum::<i64>(),
            t.tax_amount_cents
        );
    }

    // ASVS V5.4.3 / 14.1.2: Mengen ohne Obergrenze und Summen nahe i64::MAX
    // werden abgelehnt statt still überzulaufen.
    #[test]
    fn validate_rejects_amounts_out_of_range() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let mut inv = good_invoice();
        inv.items = vec![item(1, 1e300, 50_000)];
        let err = validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).unwrap_err();
        assert!(
            err.contains(&InvoiceValidationError::ItemAmountOutOfRange(1)),
            "{err:?}"
        );
        // Zu viele Nachkommastellen in der Menge (CII überträgt 4).
        inv.items = vec![item(1, 1.0 / 3.0, 50_000)];
        let err = validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).unwrap_err();
        assert!(err.contains(&InvoiceValidationError::ItemAmountOutOfRange(1)));
        // Einzelpreis über 1 Mrd. €.
        inv.items = vec![item(1, 1.0, money::MAX_UNIT_PRICE_CENTS + 1)];
        let err = validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).unwrap_err();
        assert!(err.contains(&InvoiceValidationError::ItemAmountOutOfRange(1)));
        // Unendliche Menge.
        inv.items = vec![item(1, f64::INFINITY, 100)];
        let err = validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).unwrap_err();
        assert!(err.contains(&InvoiceValidationError::ItemAmountOutOfRange(1)));
        // Jede Position in den Grenzen, die Summe nicht.
        inv.items = (1..=100)
            .map(|p| item(p, money::MAX_QUANTITY, money::MAX_UNIT_PRICE_CENTS))
            .collect();
        let err = validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).unwrap_err();
        assert!(
            err.contains(&InvoiceValidationError::TotalOutOfRange),
            "{err:?}"
        );
        // Die Summe läuft nicht über (sättigt), wird also nie negativ.
        assert!(compute_totals(&inv.items).gross_amount_cents > 0);
    }

    // KB-0014: Kategorie E nur mit §19 — sonst stünde der §19-Text als
    // Befreiungsgrund (BT-120) im XML.
    #[test]
    fn validate_rejects_exempt_category_without_paragraph_19() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let inv = good_invoice(); // Positionen mit 'E'
        let err = validate_for_issue(&inv, &seller_regel(), &buyer_full(), today).unwrap_err();
        assert!(
            err.contains(&InvoiceValidationError::ItemExemptWithoutKleinunternehmer(
                1
            ))
        );
        // Mit §19 ist 'E' Pflicht und zulässig.
        assert!(validate_for_issue(&inv, &seller_klein(), &buyer_full(), today).is_ok());
        // Regelbesteuerung mit 'S' 19 % ist zulässig.
        let mut regel = good_invoice();
        regel.items = vec![vat_item(1, 1.0, 50_000, 19.0)];
        let r = validate_for_issue(&regel, &seller_regel(), &buyer_full(), today);
        assert!(r.is_ok(), "{r:?}");
    }
}
