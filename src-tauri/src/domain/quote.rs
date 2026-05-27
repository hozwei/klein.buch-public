//! Angebots-Domain (Functional Core) — Phase 2A, Block 6.
//!
//! ## Inhalte
//!
//! - [`QuoteItemInput`] — User-Eingabe für eine Angebots-Position.
//! - [`QuoteInput`] — Kopf + Positionen (vor Lock).
//! - [`compute_totals`] — pure Betrags-Aggregation in Cent (identische
//!   kaufmännische Rundung wie [`crate::domain::invoice::compute_totals`]).
//! - [`validate_quote`] — Struktur-Checks + §19 (über
//!   [`kleinunternehmer::assert_no_vat`]).
//!
//! Alle Funktionen pure, keine I/O. Counter-Allokation und DB-Persistenz
//! liegen in [`crate::db::numbering`] und [`crate::db::repo::quotes`].
//!
//! ## Abgrenzung zur Rechnung
//!
//! Ein Angebot ist **keine** §14-Rechnung: es löst keine Steuerpflicht aus,
//! braucht daher die §14-Abs.-4-Pflichtangaben (Steuernummer, vollständige
//! Empfänger-Adresse) nicht zwingend. Die §19-Hardline gilt trotzdem — ein
//! Kleinunternehmer darf auch im Angebot keine USt ausweisen, sonst droht
//! §14c-Risiko bei späterer Konvertierung.
//!
//! ## Geld-Konvention
//!
//! Wie bei Rechnungen: alle Beträge in **Integer-Cents**, Multiplikation
//! `quantity * unit_price` kaufmännisch (`.round()`) gerundet, Aggregat auf
//! Item-Cent-Werten addiert.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::invoice::{
    InvoiceDirection, InvoiceInput, InvoiceItemInput, ItemTotals, SellerView, Totals,
};
use crate::domain::kleinunternehmer::{self, ItemVatCheck, NoVatViolation};

// ---- Input-Typen -----------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteItemInput {
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
    /// Beschreibungs-Zelle wie bisher. Block P3, Migration 0020.
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
pub struct QuoteInput {
    pub quote_date: NaiveDate,
    /// Gültig-bis-Datum (Bindefrist des Angebots).
    pub valid_until: NaiveDate,
    pub currency_code: String,
    pub items: Vec<QuoteItemInput>,
    pub notes: Option<String>,
    pub pdf_template: String,
}

/// Empfänger-Sicht für die Angebots-Validierung. Schlanker als die
/// Rechnungs-Variante: ein Angebot braucht nur den Namen zwingend.
#[derive(Debug, Clone, Copy)]
pub struct QuoteBuyerView<'a> {
    pub name: &'a str,
}

// ---- Totals ----------------------------------------------------------------

/// Pure Item- und Aggregats-Berechnung. Keine Validierung — Caller prüft.
///
/// Spiegelt [`crate::domain::invoice::compute_totals`] exakt; eigene Funktion,
/// damit die Angebots-Schicht nicht an Rechnungs-Eingabetypen koppelt.
pub fn compute_totals(items: &[QuoteItemInput]) -> Totals {
    let mut out = Vec::with_capacity(items.len());
    let mut net_total: i64 = 0;
    let mut tax_total: i64 = 0;
    for it in items {
        let net = (it.quantity * it.unit_price_cents as f64).round() as i64;
        let tax = (net as f64 * it.tax_rate_percent / 100.0).round() as i64;
        let gross = net + tax;
        out.push(ItemTotals {
            net_amount_cents: net,
            tax_amount_cents: tax,
            gross_amount_cents: gross,
        });
        net_total += net;
        tax_total += tax;
    }
    Totals {
        items: out,
        net_amount_cents: net_total,
        tax_amount_cents: tax_total,
        gross_amount_cents: net_total + tax_total,
    }
}

// ---- Validation ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum QuoteValidationError {
    // --- Struktur ---
    NoItems,
    CurrencyEmpty,
    /// Gesamtbetrag ist nicht positiv. Ein Angebot über 0 € ist ein Eingabefehler.
    TotalNotPositive,
    /// Gültig-bis liegt vor dem Angebotsdatum.
    ValidUntilBeforeQuoteDate {
        quote_date: NaiveDate,
        valid_until: NaiveDate,
    },

    // --- §14-light (Angebot ist keine Rechnung) ---
    SellerNameMissing,
    SellerAddressIncomplete,
    BuyerNameMissing,

    // --- Item-Ebene ---
    ItemDescriptionMissing(u32),
    ItemQuantityNotPositive(u32),
    ItemUnitPriceNegative(u32),
    ItemDuplicatePosition(u32),
    ItemInvalidTaxCategoryCode {
        position: u32,
        code: String,
    },
    ItemTaxRateNegative {
        position: u32,
        rate: f64,
    },

    // --- §19 — siehe kleinunternehmer::assert_no_vat ---
    Paragraph19VatViolation(Vec<NoVatViolation>),
}

/// Validiert ein [`QuoteInput`]. Aggregiert alle Fehler (gibt nicht beim
/// ersten Fail auf). Leerer Vec → das Angebot darf festgeschrieben werden.
///
/// `today` wird injiziert (Tests pinnen das Datum); aktuell nur für
/// potenzielle künftige Datums-Regeln durchgereicht — Angebote dürfen
/// bewusst vor- und rückdatiert werden, daher kein Future-Date-Block.
pub fn validate_quote(
    input: &QuoteInput,
    seller: &SellerView<'_>,
    buyer: &QuoteBuyerView<'_>,
    _today: NaiveDate,
) -> Result<(), Vec<QuoteValidationError>> {
    use QuoteValidationError as E;
    let mut errs = Vec::new();

    // --- Struktur ---
    if input.items.is_empty() {
        errs.push(E::NoItems);
    }
    if input.currency_code.trim().is_empty() {
        errs.push(E::CurrencyEmpty);
    }
    // Gesamtbetrag muss positiv sein (leerer Fall ist über NoItems abgedeckt).
    if !input.items.is_empty() && compute_totals(&input.items).gross_amount_cents <= 0 {
        errs.push(E::TotalNotPositive);
    }
    if input.valid_until < input.quote_date {
        errs.push(E::ValidUntilBeforeQuoteDate {
            quote_date: input.quote_date,
            valid_until: input.valid_until,
        });
    }

    // --- Items ---
    let mut seen_positions = std::collections::HashSet::new();
    const VALID_TAX_CODES: &[&str] = &["S", "Z", "E", "AE", "K", "G", "O", "L", "M"];
    for it in &input.items {
        if it.description.trim().is_empty() {
            errs.push(E::ItemDescriptionMissing(it.position));
        }
        if it.quantity.is_nan() || it.quantity <= 0.0 {
            errs.push(E::ItemQuantityNotPositive(it.position));
        }
        if it.unit_price_cents < 0 {
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
    }

    // --- §19 (§14c-Schutz, auch im Angebot) ---
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

    // --- Seller / Buyer (light) ---
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

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI).
pub fn message(e: &QuoteValidationError) -> String {
    use QuoteValidationError as E;
    match e {
        E::NoItems => "Mindestens eine Position ist erforderlich.".into(),
        E::CurrencyEmpty => "Währung fehlt.".into(),
        E::TotalNotPositive => "Der Gesamtbetrag muss größer als 0 € sein.".into(),
        E::ValidUntilBeforeQuoteDate {
            quote_date,
            valid_until,
        } => format!("Gültig-bis ({valid_until}) liegt vor dem Angebotsdatum ({quote_date})."),
        E::SellerNameMissing => "Verkäufer-Name fehlt (Stammdaten).".into(),
        E::SellerAddressIncomplete => {
            "Verkäufer-Adresse ist unvollständig (Straße, PLZ, Ort).".into()
        }
        E::BuyerNameMissing => "Empfänger-Name fehlt.".into(),
        E::ItemDescriptionMissing(p) => format!("Position {p}: Beschreibung fehlt."),
        E::ItemQuantityNotPositive(p) => format!("Position {p}: Menge muss größer als 0 sein."),
        E::ItemUnitPriceNegative(p) => {
            format!("Position {p}: Einzelpreis darf nicht negativ sein.")
        }
        E::ItemDuplicatePosition(p) => format!("Position {p} ist doppelt vergeben."),
        E::ItemInvalidTaxCategoryCode { position, code } => {
            format!("Position {position}: ungültiger Steuer-Kategorie-Code {code}.")
        }
        E::ItemTaxRateNegative { position, rate } => {
            format!("Position {position}: Steuersatz darf nicht negativ sein ({rate} %).")
        }
        E::Paragraph19VatViolation(_) => {
            "§19-Verstoß: Bei Kleinunternehmer darf keine Umsatzsteuer ausgewiesen werden.".into()
        }
    }
}

/// Maschinenlesbarer Variantenname für DTOs/Logs.
pub fn variant_name(e: &QuoteValidationError) -> &'static str {
    use QuoteValidationError as E;
    match e {
        E::NoItems => "NoItems",
        E::CurrencyEmpty => "CurrencyEmpty",
        E::TotalNotPositive => "TotalNotPositive",
        E::ValidUntilBeforeQuoteDate { .. } => "ValidUntilBeforeQuoteDate",
        E::SellerNameMissing => "SellerNameMissing",
        E::SellerAddressIncomplete => "SellerAddressIncomplete",
        E::BuyerNameMissing => "BuyerNameMissing",
        E::ItemDescriptionMissing(_) => "ItemDescriptionMissing",
        E::ItemQuantityNotPositive(_) => "ItemQuantityNotPositive",
        E::ItemUnitPriceNegative(_) => "ItemUnitPriceNegative",
        E::ItemDuplicatePosition(_) => "ItemDuplicatePosition",
        E::ItemInvalidTaxCategoryCode { .. } => "ItemInvalidTaxCategoryCode",
        E::ItemTaxRateNegative { .. } => "ItemTaxRateNegative",
        E::Paragraph19VatViolation(_) => "Paragraph19VatViolation",
    }
}

// ---- Konvertierung Angebot → Rechnung (Block 7) ----------------------------

/// Rechnungs-spezifische Felder, die ein Angebot nicht trägt und beim
/// Umwandeln gesetzt werden. Datum/Fälligkeit kommen vom Nutzer (das Angebots-
/// datum ist für die Rechnung irrelevant), `currency`/`notes`/`pdf_template`
/// werden i. d. R. aus dem Angebot übernommen — der Caller entscheidet.
#[derive(Debug, Clone)]
pub struct ConvertToInvoiceOptions {
    pub invoice_date: NaiveDate,
    pub delivery_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub currency_code: String,
    pub notes: Option<String>,
    /// Bezahlt-/Zahlungshinweis für die entstehende Rechnung (reiner PDF-Text).
    pub payment_note: Option<String>,
    pub pdf_template: String,
}

/// Pure: baut aus Angebots-Positionen eine [`InvoiceInput`] für eine
/// **Ausgangsrechnung** (`direction = Issued`). Reines Mapping ohne I/O.
///
/// Die Positionen werden 1:1 übernommen (Beschreibung, Menge, Einheit,
/// Einzelpreis und — wichtig für die §19-Hardline — `tax_rate_percent` +
/// `tax_category_code`). Damit bleibt ein §19-Angebot auch nach der
/// Konvertierung USt-frei (Category `E`, Rate `0`); ein versehentlicher
/// USt-Ausweis kann nicht über die Konvertierung „hereinrutschen".
///
/// Der **Status-Guard** (Konvertierung nur aus `accepted`) ist bewusst NICHT
/// hier, sondern in der Imperative Shell ([`crate::db::repo::quotes::mark_converted`]
/// / [`crate::commands::quotes`]) — er hängt am DB-Zustand, nicht an den Items.
pub fn convert_to_invoice(
    items: &[QuoteItemInput],
    opts: &ConvertToInvoiceOptions,
) -> InvoiceInput {
    let inv_items = items
        .iter()
        .map(|qi| InvoiceItemInput {
            position: qi.position,
            description: qi.description.clone(),
            quantity: qi.quantity,
            unit_code: qi.unit_code.clone(),
            unit_price_cents: qi.unit_price_cents,
            tax_rate_percent: qi.tax_rate_percent,
            tax_category_code: qi.tax_category_code.clone(),
            // Paket-Provenienz + PDF-Markup wandern 1:1 in die Rechnung (P3).
            description_title: qi.description_title.clone(),
            description_markup: qi.description_markup.clone(),
            source_package_id: qi.source_package_id.clone(),
            source_package_revision: qi.source_package_revision,
        })
        .collect();
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: opts.invoice_date,
        delivery_date: opts.delivery_date,
        due_date: opts.due_date,
        currency_code: opts.currency_code.clone(),
        items: inv_items,
        notes: opts.notes.clone(),
        payment_note: opts.payment_note.clone(),
        pdf_template: opts.pdf_template.clone(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn buyer() -> QuoteBuyerView<'static> {
        QuoteBuyerView { name: "Kunde GmbH" }
    }

    fn item(position: u32, qty: f64, price: i64) -> QuoteItemInput {
        QuoteItemInput {
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

    fn good_quote() -> QuoteInput {
        QuoteInput {
            quote_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
            valid_until: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            currency_code: "EUR".into(),
            items: vec![item(1, 2.0, 25_000)],
            notes: None,
            pdf_template: "default".into(),
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()
    }

    #[test]
    fn compute_totals_klein_no_vat() {
        let items = vec![item(1, 2.0, 10_000), item(2, 1.0, 5_000)];
        let t = compute_totals(&items);
        assert_eq!(t.net_amount_cents, 25_000);
        assert_eq!(t.tax_amount_cents, 0);
        assert_eq!(t.gross_amount_cents, 25_000);
        assert_eq!(t.items.len(), 2);
        assert_eq!(t.items[0].net_amount_cents, 20_000);
    }

    #[test]
    fn compute_totals_with_vat() {
        let mut it = item(1, 1.0, 10_000);
        it.tax_rate_percent = 19.0;
        it.tax_category_code = "S".into();
        let t = compute_totals(&[it]);
        assert_eq!(t.net_amount_cents, 10_000);
        assert_eq!(t.tax_amount_cents, 1_900);
        assert_eq!(t.gross_amount_cents, 11_900);
    }

    #[test]
    fn validate_passes_for_good_quote() {
        let r = validate_quote(&good_quote(), &seller_klein(), &buyer(), today());
        assert!(r.is_ok(), "expected ok, got {r:?}");
    }

    #[test]
    fn validate_flags_no_items() {
        let mut q = good_quote();
        q.items.clear();
        let err = validate_quote(&q, &seller_klein(), &buyer(), today()).unwrap_err();
        assert!(err.contains(&QuoteValidationError::NoItems));
    }

    #[test]
    fn validate_flags_zero_total() {
        // Eine Position mit Preis 0 → Gesamt 0 € → muss blocken.
        let mut q = good_quote();
        q.items = vec![item(1, 1.0, 0)];
        let err = validate_quote(&q, &seller_klein(), &buyer(), today()).unwrap_err();
        assert!(err.contains(&QuoteValidationError::TotalNotPositive));
    }

    #[test]
    fn validate_flags_valid_until_before_quote_date() {
        let mut q = good_quote();
        q.valid_until = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let err = validate_quote(&q, &seller_klein(), &buyer(), today()).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, QuoteValidationError::ValidUntilBeforeQuoteDate { .. })));
    }

    #[test]
    fn validate_flags_paragraph_19_violation() {
        let mut q = good_quote();
        q.items[0].tax_rate_percent = 19.0;
        q.items[0].tax_category_code = "S".into();
        let err = validate_quote(&q, &seller_klein(), &buyer(), today()).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, QuoteValidationError::Paragraph19VatViolation(_))));
    }

    #[test]
    fn validate_flags_duplicate_position() {
        let mut q = good_quote();
        q.items.push(item(1, 1.0, 1_000));
        let err = validate_quote(&q, &seller_klein(), &buyer(), today()).unwrap_err();
        assert!(err.contains(&QuoteValidationError::ItemDuplicatePosition(1)));
    }

    #[test]
    fn validate_flags_invalid_tax_category() {
        let mut q = good_quote();
        q.items[0].tax_category_code = "XX".into();
        let err = validate_quote(&q, &seller_klein(), &buyer(), today()).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, QuoteValidationError::ItemInvalidTaxCategoryCode { .. })));
    }

    #[test]
    fn validate_flags_missing_buyer_name() {
        let b = QuoteBuyerView { name: "  " };
        let err = validate_quote(&good_quote(), &seller_klein(), &b, today()).unwrap_err();
        assert!(err.contains(&QuoteValidationError::BuyerNameMissing));
    }

    // ---- convert_to_invoice (Block 7) ----

    fn convert_opts() -> ConvertToInvoiceOptions {
        ConvertToInvoiceOptions {
            invoice_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            delivery_date: Some(NaiveDate::from_ymd_opt(2026, 5, 18).unwrap()),
            due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 3).unwrap()),
            currency_code: "EUR".into(),
            notes: Some("aus Angebot AN-2026-0001".into()),
            payment_note: None,
            pdf_template: "default".into(),
        }
    }

    #[test]
    fn convert_maps_items_and_sets_invoice_fields() {
        let q = good_quote();
        let opts = convert_opts();
        let inv = convert_to_invoice(&q.items, &opts);

        assert!(matches!(inv.direction, InvoiceDirection::Issued));
        assert_eq!(inv.invoice_date, opts.invoice_date);
        assert_eq!(inv.delivery_date, opts.delivery_date);
        assert_eq!(inv.due_date, opts.due_date);
        assert_eq!(inv.currency_code, "EUR");
        assert_eq!(inv.notes.as_deref(), Some("aus Angebot AN-2026-0001"));
        assert_eq!(inv.pdf_template, "default");
        assert!(inv.is_storno_for.is_none());
        assert!(inv.cancel_reason.is_none());

        assert_eq!(inv.items.len(), q.items.len());
        let (qi, ii) = (&q.items[0], &inv.items[0]);
        assert_eq!(ii.position, qi.position);
        assert_eq!(ii.description, qi.description);
        assert_eq!(ii.quantity, qi.quantity);
        assert_eq!(ii.unit_code, qi.unit_code);
        assert_eq!(ii.unit_price_cents, qi.unit_price_cents);
        assert_eq!(ii.tax_rate_percent, qi.tax_rate_percent);
        assert_eq!(ii.tax_category_code, qi.tax_category_code);
    }

    #[test]
    fn convert_preserves_paragraph_19_zero_vat() {
        // §19-Angebot (Category E, Rate 0) → Rechnung bleibt USt-frei.
        let q = good_quote();
        let inv = convert_to_invoice(&q.items, &convert_opts());
        for it in &inv.items {
            assert_eq!(it.tax_category_code, "E");
            assert_eq!(it.tax_rate_percent, 0.0);
        }
        // Totals der konvertierten Rechnung enthalten keine USt.
        let totals = crate::domain::invoice::compute_totals(&inv.items);
        assert_eq!(totals.tax_amount_cents, 0);
        assert_eq!(totals.net_amount_cents, totals.gross_amount_cents);
    }

    #[test]
    fn convert_keeps_regular_vat_when_present() {
        // Defensiv: falls (Regelbesteuerung) USt-Items vorliegen, werden sie
        // unverändert übernommen — die §19-Sperre liegt beim Seller-Status,
        // nicht in der Mapping-Funktion.
        let mut q = good_quote();
        q.items[0].tax_rate_percent = 19.0;
        q.items[0].tax_category_code = "S".into();
        let inv = convert_to_invoice(&q.items, &convert_opts());
        assert_eq!(inv.items[0].tax_rate_percent, 19.0);
        assert_eq!(inv.items[0].tax_category_code, "S");
    }
}
