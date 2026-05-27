//! Wiederkehrende Ausgangsrechnungen (Functional Core) — Phase 4, Block RI-1.
//!
//! ## Inhalte
//!
//! - [`AutoMode`] — wie weit der Scheduler automatisiert (1:1 zum CHECK in
//!   `0024_recurring_invoices.sql`).
//! - [`RecurringInvoiceInput`] — User-Eingabe für eine Abo-Rechnungs-Vorlage
//!   (create/update). Positionen sind der kanonische [`InvoiceItemInput`].
//! - [`validate_recurring_invoice`] — Struktur-/Wert-Checks (pure, aggregiert).
//!
//! Die Stichtags-Mathematik (Frequenz, nächster Stichtag, Klemmung) wird aus
//! [`crate::domain::recurring`] wiederverwendet — nicht neu gebaut. Persistenz
//! liegt in [`crate::db::repo::recurring_invoice`]; die Materialisierung am
//! Stichtag (Block RI-2) in `crate::scheduler::recurring_invoice`.
//!
//! ## Abgrenzung
//!
//! Eine Vorlage ist ein **Template/Stammdatum**, kein GoBD-Beleg — editierbar
//! und pausierbar. Die daraus erzeugte Rechnung ist nach dem Festschreiben
//! unveränderlich (invoices-Trigger). Beträge sind **Netto**-Cent; bei §19
//! (`tax_category_code = 'E'`, Rate 0) ist Netto = Brutto. Die §19-Klausel und
//! die Pflichtangaben kommen aus der bestehenden draft-/issue-Pipeline; die
//! §14c-Durchsetzung (kein USt-Ausweis bei Kleinunternehmer) passiert in der
//! Command-/Issue-Schicht, nicht hier.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::invoice::{compute_totals, InvoiceItemInput};
use crate::domain::recurring::Frequency;

/// EN-16931-Tax-Category-Codes — synchron zum CHECK in `0024_recurring_invoices.sql`
/// (und zur invoice_items-Tabelle).
const VALID_TAX_CODES: &[&str] = &["S", "Z", "E", "AE", "K", "G", "O", "L", "M"];

// ---- Automatik-Stufe -------------------------------------------------------

/// Wie weit der Scheduler eine fällige Abo-Rechnung automatisch verarbeitet.
/// DB-Werte == CHECK-Constraint in `0024_recurring_invoices.sql`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoMode {
    /// Nur Rechnungs-Entwurf anlegen + benachrichtigen (prüfungssicher).
    Draft,
    /// Automatisch festschreiben (volle Pipeline), kein Versand.
    Issue,
    /// Festschreiben + automatisch per E-Mail senden.
    IssueSend,
}

impl AutoMode {
    /// DB-Wert (== CHECK-Constraint).
    pub fn as_db(self) -> &'static str {
        match self {
            AutoMode::Draft => "draft",
            AutoMode::Issue => "issue",
            AutoMode::IssueSend => "issue_send",
        }
    }

    /// Parst den DB-Wert. `None` bei unbekanntem String.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(AutoMode::Draft),
            "issue" => Some(AutoMode::Issue),
            "issue_send" => Some(AutoMode::IssueSend),
            _ => None,
        }
    }
}

// ---- Input-Typ -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoiceInput {
    /// Interne Bezeichnung der Vorlage ("Wartung Server – Müller GmbH").
    pub label: String,
    /// Rechnungsempfänger (Kunde).
    pub contact_id: String,
    /// 'monthly' | 'quarterly' | 'semiannually' | 'annually'.
    pub frequency: String,
    /// Stichtag im Periodenraster (1..=31).
    pub day_of_period: i64,
    /// Nächster fälliger Stichtag.
    pub next_due_date: NaiveDate,
    /// Optionales Startdatum (nur Dokumentation).
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
    /// Optionales Laufzeit-Ende — nach diesem Datum wird nichts mehr erzeugt.
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    /// 'draft' | 'issue' | 'issue_send' — Automatik-Stufe.
    pub auto_mode: String,
    /// Zahlungsziel in Tagen (Fälligkeit = Rechnungsdatum + diese Tage).
    pub payment_terms_days: i64,
    /// PDF-Template-Name (Default "default").
    pub pdf_template: String,
    /// Leistungszeitraum der Periode automatisch in Leistungsdatum/Beschreibung
    /// setzen.
    #[serde(default)]
    pub service_period_note: bool,
    /// Interne Notiz (optional).
    #[serde(default)]
    pub notes: Option<String>,
    /// Positionen — kanonischer Rechnungs-Positionstyp.
    pub items: Vec<InvoiceItemInput>,
}

// ---- Validation ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RecurringInvoiceValidationError {
    LabelMissing,
    ContactMissing,
    FrequencyInvalid { value: String },
    DayOfPeriodOutOfRange { day: i64 },
    AutoModeInvalid { value: String },
    PaymentTermsNegative { days: i64 },
    EndDateBeforeNextDue { end: NaiveDate, next_due: NaiveDate },
    NoItems,
    ItemDescriptionMissing { position: u32 },
    ItemQuantityNotPositive { position: u32, quantity: f64 },
    ItemUnitPriceNegative { position: u32, cents: i64 },
    ItemTaxCategoryInvalid { position: u32, code: String },
    TotalNotPositive { total: i64 },
}

/// Validiert eine [`RecurringInvoiceInput`]. Aggregiert alle Fehler (kein
/// Fail-Fast), damit das UI sie gesammelt anzeigen kann.
pub fn validate_recurring_invoice(
    input: &RecurringInvoiceInput,
) -> Result<(), Vec<RecurringInvoiceValidationError>> {
    use RecurringInvoiceValidationError as E;
    let mut errs = Vec::new();

    if input.label.trim().is_empty() {
        errs.push(E::LabelMissing);
    }
    if input.contact_id.trim().is_empty() {
        errs.push(E::ContactMissing);
    }
    if Frequency::from_db(&input.frequency).is_none() {
        errs.push(E::FrequencyInvalid {
            value: input.frequency.clone(),
        });
    }
    if !(1..=31).contains(&input.day_of_period) {
        errs.push(E::DayOfPeriodOutOfRange {
            day: input.day_of_period,
        });
    }
    if AutoMode::from_db(&input.auto_mode).is_none() {
        errs.push(E::AutoModeInvalid {
            value: input.auto_mode.clone(),
        });
    }
    if input.payment_terms_days < 0 {
        errs.push(E::PaymentTermsNegative {
            days: input.payment_terms_days,
        });
    }
    if let Some(end) = input.end_date {
        if end < input.next_due_date {
            errs.push(E::EndDateBeforeNextDue {
                end,
                next_due: input.next_due_date,
            });
        }
    }

    if input.items.is_empty() {
        errs.push(E::NoItems);
    }
    for it in &input.items {
        if it.description.trim().is_empty() {
            errs.push(E::ItemDescriptionMissing {
                position: it.position,
            });
        }
        if it.quantity <= 0.0 {
            errs.push(E::ItemQuantityNotPositive {
                position: it.position,
                quantity: it.quantity,
            });
        }
        if it.unit_price_cents < 0 {
            errs.push(E::ItemUnitPriceNegative {
                position: it.position,
                cents: it.unit_price_cents,
            });
        }
        if !VALID_TAX_CODES.contains(&it.tax_category_code.as_str()) {
            errs.push(E::ItemTaxCategoryInvalid {
                position: it.position,
                code: it.tax_category_code.clone(),
            });
        }
    }

    // Gesamt-Netto muss positiv sein (eine Vorlage über 0,00 € ergibt keinen
    // sinnvollen Beleg). Nur prüfen, wenn überhaupt Positionen da sind.
    if !input.items.is_empty() {
        let total = compute_totals(&input.items).net_amount_cents;
        if total <= 0 {
            errs.push(E::TotalNotPositive { total });
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

// ---- Fehler-Texte ----------------------------------------------------------

fn eur(c: i64) -> String {
    // i64-pur (Schema-Hardline: kein Float für Geld, vgl. KB-0055/KB-0006).
    // Vorzeichen erhalten — die Helfer formatieren auch negative Beträge
    // (negativer Einzelpreis / Gesamtbetrag in Fehlertexten).
    let sign = if c < 0 { "-" } else { "" };
    let abs = c.unsigned_abs();
    format!("{sign}{},{:02} €", abs / 100, abs % 100)
}

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI).
pub fn message(e: &RecurringInvoiceValidationError) -> String {
    use RecurringInvoiceValidationError as E;
    match e {
        E::LabelMissing => "Bezeichnung ist erforderlich.".into(),
        E::ContactMissing => "Ein Kunde muss ausgewählt sein.".into(),
        E::FrequencyInvalid { value } => format!("Ungültige Frequenz: {value}."),
        E::DayOfPeriodOutOfRange { day } => {
            format!("Stichtag muss zwischen 1 und 31 liegen (war {day}).")
        }
        E::AutoModeInvalid { value } => format!("Ungültige Automatik-Stufe: {value}."),
        E::PaymentTermsNegative { days } => {
            format!("Zahlungsziel darf nicht negativ sein (war {days}).")
        }
        E::EndDateBeforeNextDue { end, next_due } => {
            format!("Laufzeit-Ende ({end}) liegt vor dem nächsten Stichtag ({next_due}).")
        }
        E::NoItems => "Mindestens eine Position ist erforderlich.".into(),
        E::ItemDescriptionMissing { position } => {
            format!("Position {position}: Beschreibung ist erforderlich.")
        }
        E::ItemQuantityNotPositive { position, quantity } => {
            format!("Position {position}: Menge muss größer als 0 sein (war {quantity}).")
        }
        E::ItemUnitPriceNegative { position, cents } => {
            format!(
                "Position {position}: Einzelpreis darf nicht negativ sein ({}).",
                eur(*cents)
            )
        }
        E::ItemTaxCategoryInvalid { position, code } => {
            format!("Position {position}: ungültiger Steuer-Code '{code}'.")
        }
        E::TotalNotPositive { total } => {
            format!("Gesamtbetrag muss größer als 0 sein ({}).", eur(*total))
        }
    }
}

/// Maschinenlesbarer Variantenname für DTOs/Logs.
pub fn variant_name(e: &RecurringInvoiceValidationError) -> &'static str {
    use RecurringInvoiceValidationError as E;
    match e {
        E::LabelMissing => "LabelMissing",
        E::ContactMissing => "ContactMissing",
        E::FrequencyInvalid { .. } => "FrequencyInvalid",
        E::DayOfPeriodOutOfRange { .. } => "DayOfPeriodOutOfRange",
        E::AutoModeInvalid { .. } => "AutoModeInvalid",
        E::PaymentTermsNegative { .. } => "PaymentTermsNegative",
        E::EndDateBeforeNextDue { .. } => "EndDateBeforeNextDue",
        E::NoItems => "NoItems",
        E::ItemDescriptionMissing { .. } => "ItemDescriptionMissing",
        E::ItemQuantityNotPositive { .. } => "ItemQuantityNotPositive",
        E::ItemUnitPriceNegative { .. } => "ItemUnitPriceNegative",
        E::ItemTaxCategoryInvalid { .. } => "ItemTaxCategoryInvalid",
        E::TotalNotPositive { .. } => "TotalNotPositive",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn item() -> InvoiceItemInput {
        InvoiceItemInput {
            position: 1,
            description: "Server-Wartung (Pauschale)".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 9_900,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }
    }

    fn good() -> RecurringInvoiceInput {
        RecurringInvoiceInput {
            label: "Wartung Server – Müller GmbH".into(),
            contact_id: "contact-123".into(),
            frequency: "monthly".into(),
            day_of_period: 1,
            next_due_date: d(2026, 6, 1),
            start_date: None,
            end_date: None,
            auto_mode: "draft".into(),
            payment_terms_days: 14,
            pdf_template: "default".into(),
            service_period_note: true,
            notes: None,
            items: vec![item()],
        }
    }

    #[test]
    fn valid_template_passes() {
        assert!(validate_recurring_invoice(&good()).is_ok());
    }

    #[test]
    fn missing_label_and_contact_flagged() {
        let mut r = good();
        r.label = "  ".into();
        r.contact_id = "".into();
        let err = validate_recurring_invoice(&r).unwrap_err();
        assert!(err.contains(&RecurringInvoiceValidationError::LabelMissing));
        assert!(err.contains(&RecurringInvoiceValidationError::ContactMissing));
    }

    #[test]
    fn invalid_frequency_flagged() {
        let mut r = good();
        r.frequency = "weekly".into();
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringInvoiceValidationError::FrequencyInvalid { .. })));
    }

    #[test]
    fn day_of_period_out_of_range_flagged() {
        let mut r = good();
        r.day_of_period = 0;
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(
                x,
                RecurringInvoiceValidationError::DayOfPeriodOutOfRange { .. }
            )));
        r.day_of_period = 32;
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(
                x,
                RecurringInvoiceValidationError::DayOfPeriodOutOfRange { .. }
            )));
    }

    #[test]
    fn invalid_auto_mode_flagged() {
        let mut r = good();
        r.auto_mode = "yolo".into();
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringInvoiceValidationError::AutoModeInvalid { .. })));
    }

    #[test]
    fn negative_payment_terms_flagged() {
        let mut r = good();
        r.payment_terms_days = -1;
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(
                x,
                RecurringInvoiceValidationError::PaymentTermsNegative { .. }
            )));
    }

    #[test]
    fn end_date_before_next_due_flagged() {
        let mut r = good();
        r.end_date = Some(d(2026, 5, 1)); // vor next_due 2026-06-01
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(
                x,
                RecurringInvoiceValidationError::EndDateBeforeNextDue { .. }
            )));
    }

    #[test]
    fn end_date_equal_next_due_ok() {
        let mut r = good();
        r.end_date = Some(d(2026, 6, 1)); // == next_due, erlaubt (eine Periode)
        assert!(validate_recurring_invoice(&r).is_ok());
    }

    #[test]
    fn no_items_flagged() {
        let mut r = good();
        r.items.clear();
        let err = validate_recurring_invoice(&r).unwrap_err();
        assert!(err.contains(&RecurringInvoiceValidationError::NoItems));
        // TotalNotPositive darf NICHT zusätzlich feuern, wenn keine Items da sind.
        assert!(!err
            .iter()
            .any(|x| matches!(x, RecurringInvoiceValidationError::TotalNotPositive { .. })));
    }

    #[test]
    fn item_description_and_quantity_flagged() {
        let mut r = good();
        r.items[0].description = "   ".into();
        r.items[0].quantity = 0.0;
        let err = validate_recurring_invoice(&r).unwrap_err();
        assert!(err.iter().any(|x| matches!(
            x,
            RecurringInvoiceValidationError::ItemDescriptionMissing { .. }
        )));
        assert!(err.iter().any(|x| matches!(
            x,
            RecurringInvoiceValidationError::ItemQuantityNotPositive { .. }
        )));
    }

    #[test]
    fn invalid_tax_category_flagged() {
        let mut r = good();
        r.items[0].tax_category_code = "XX".into();
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(
                x,
                RecurringInvoiceValidationError::ItemTaxCategoryInvalid { .. }
            )));
    }

    #[test]
    fn zero_total_flagged() {
        let mut r = good();
        r.items[0].unit_price_cents = 0; // Menge 1 × 0 = 0 -> Gesamt 0
        assert!(validate_recurring_invoice(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringInvoiceValidationError::TotalNotPositive { .. })));
    }

    #[test]
    fn auto_mode_db_roundtrip() {
        for m in [AutoMode::Draft, AutoMode::Issue, AutoMode::IssueSend] {
            assert_eq!(AutoMode::from_db(m.as_db()), Some(m));
        }
        assert_eq!(AutoMode::from_db("nope"), None);
    }
}
