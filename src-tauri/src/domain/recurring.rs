//! Recurring-Abos (Functional Core) — Phase 2B, Block 10.
//!
//! ## Inhalte
//!
//! - [`Frequency`] — Abo-Frequenz (1:1 zum CHECK in `0009_recurring.sql`).
//! - [`RecurringInput`] — User-Eingabe für ein Abo (create/update).
//! - [`validate_recurring`] — Struktur-/Wert-Checks (pure).
//! - [`compute_next_due_date`] — schiebt einen Stichtag um eine Periode vor,
//!   geklemmt auf die Monatslänge (`day_of_period` 31 → Februar = 28/29).
//! - [`is_due`] — Stichtag erreicht/überschritten?
//!
//! Alle Funktionen pure, keine I/O. Persistenz liegt in
//! [`crate::db::repo::recurring`]; die Auto-Anlage von Kosten am Stichtag in
//! [`crate::scheduler::recurring`].
//!
//! ## Abgrenzung
//!
//! Ein Abo ist ein **Template/Stammdatum**, kein GoBD-Beleg — daher editierbar
//! und pausierbar. Die daraus erzeugte Kosten-Position ist dagegen sofort
//! gelockt (Eingangsseite, §19 betrifft nur Ausgangsbelege; siehe
//! [`crate::domain::expense`]). Der erwartete Betrag ist **Brutto**.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

/// Erlaubte Frequenzen — synchron zum CHECK-Constraint in `0009_recurring.sql`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Monthly,
    Quarterly,
    SemiAnnually,
    Annually,
}

impl Frequency {
    /// Anzahl Monate, um die ein Stichtag pro Periode vorrückt.
    pub fn months(self) -> u32 {
        match self {
            Frequency::Monthly => 1,
            Frequency::Quarterly => 3,
            Frequency::SemiAnnually => 6,
            Frequency::Annually => 12,
        }
    }

    /// DB-Wert (== CHECK-Constraint).
    pub fn as_db(self) -> &'static str {
        match self {
            Frequency::Monthly => "monthly",
            Frequency::Quarterly => "quarterly",
            Frequency::SemiAnnually => "semiannually",
            Frequency::Annually => "annually",
        }
    }

    /// Parst den DB-Wert. `None` bei unbekanntem String.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "monthly" => Some(Frequency::Monthly),
            "quarterly" => Some(Frequency::Quarterly),
            "semiannually" => Some(Frequency::SemiAnnually),
            "annually" => Some(Frequency::Annually),
            _ => None,
        }
    }
}

// ---- Input-Typ -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInput {
    /// Anzeige-Bezeichnung des Abos ("Microsoft 365 Business").
    pub label: String,
    /// Verknüpfter Lieferanten-Kontakt (optional).
    pub vendor_contact_id: Option<String>,
    /// 'monthly' | 'quarterly' | 'semiannually' | 'annually'.
    pub frequency: String,
    /// Stichtag im Periodenraster (1..=31).
    pub day_of_period: i64,
    /// Nächster fälliger Stichtag.
    pub next_due_date: NaiveDate,
    /// Erwarteter BRUTTO-Betrag in Cent.
    pub expected_amount_cents: i64,
    /// EÜR-Kategorie — muss in [`crate::domain::expense::VALID_CATEGORIES`] sein.
    pub category: String,
    /// Vorlage für die Beschreibung der erzeugten Kosten-Position.
    pub description_template: String,
    /// Am Stichtag automatisch eine Kosten-Position anlegen (Catch-up), sonst
    /// nur als „fällig" in der Liste melden (Reminder).
    pub auto_create_expense: bool,
    /// §13b-Vorgabe für die erzeugten Kosten (reines Hinweis-Flag).
    pub reverse_charge_13b_default: bool,
}

// ---- Validation ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RecurringValidationError {
    LabelMissing,
    DescriptionMissing,
    FrequencyInvalid { value: String },
    DayOfPeriodOutOfRange { day: i64 },
    CategoryInvalid { category: String },
    AmountNotPositive { amount: i64 },
}

/// Validiert eine [`RecurringInput`]. Aggregiert alle Fehler (kein Fail-Fast).
pub fn validate_recurring(input: &RecurringInput) -> Result<(), Vec<RecurringValidationError>> {
    use RecurringValidationError as E;
    let mut errs = Vec::new();

    if input.label.trim().is_empty() {
        errs.push(E::LabelMissing);
    }
    if input.description_template.trim().is_empty() {
        errs.push(E::DescriptionMissing);
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
    if !crate::domain::expense::VALID_CATEGORIES.contains(&input.category.as_str()) {
        errs.push(E::CategoryInvalid {
            category: input.category.clone(),
        });
    }
    if input.expected_amount_cents <= 0 {
        errs.push(E::AmountNotPositive {
            amount: input.expected_amount_cents,
        });
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

// ---- Stichtags-Rechnung ----------------------------------------------------

/// Anzahl Tage im Monat (year, month) — `month` 1..=12.
fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_this = NaiveDate::from_ymd_opt(year, month, 1).expect("valid first-of-month");
    let first_next =
        NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("valid first-of-next-month");
    (first_next - first_this).num_days() as u32
}

/// Schiebt `from` um `add_months` Monate vor und setzt den Tag auf
/// `day_of_period`, geklemmt auf die Monatslänge des Zielmonats.
///
/// Wichtig: Das Ziel-Datum basiert auf `day_of_period` (nicht auf dem Tag von
/// `from`), damit ein im Februar geklemmter Stichtag (z. B. 28.) im März wieder
/// auf den eigentlichen Stichtag (31. → 31.) zurückfindet.
pub fn add_months_clamped(from: NaiveDate, add_months: u32, day_of_period: u32) -> NaiveDate {
    // Monats-Arithmetik über einen 0-basierten Monatsindex.
    let total = from.year() * 12 + (from.month0() as i32) + add_months as i32;
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1; // zurück auf 1..=12
    let day = day_of_period.min(days_in_month(year, month)).max(1);
    NaiveDate::from_ymd_opt(year, month, day).expect("clamped date is always valid")
}

/// Nächster Stichtag nach `from`, eine Periode (`frequency`) später.
pub fn compute_next_due_date(
    frequency: Frequency,
    day_of_period: u32,
    from: NaiveDate,
) -> NaiveDate {
    add_months_clamped(from, frequency.months(), day_of_period)
}

/// Ist der Stichtag erreicht oder überschritten? (`<= today`).
pub fn is_due(next_due_date: NaiveDate, today: NaiveDate) -> bool {
    next_due_date <= today
}

// ---- Fehler-Texte ----------------------------------------------------------

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI).
pub fn message(e: &RecurringValidationError) -> String {
    use RecurringValidationError as E;
    fn eur(c: i64) -> String {
        format!("{:.2} €", c as f64 / 100.0).replace('.', ",")
    }
    match e {
        E::LabelMissing => "Bezeichnung ist erforderlich.".into(),
        E::DescriptionMissing => "Beschreibungs-Vorlage ist erforderlich.".into(),
        E::FrequencyInvalid { value } => format!("Ungültige Frequenz: {value}."),
        E::DayOfPeriodOutOfRange { day } => {
            format!("Stichtag muss zwischen 1 und 31 liegen (war {day}).")
        }
        E::CategoryInvalid { category } => format!("Ungültige Kategorie: {category}."),
        E::AmountNotPositive { amount } => {
            format!(
                "Erwarteter Betrag muss größer als 0 sein ({}).",
                eur(*amount)
            )
        }
    }
}

/// Maschinenlesbarer Variantenname für DTOs/Logs.
pub fn variant_name(e: &RecurringValidationError) -> &'static str {
    use RecurringValidationError as E;
    match e {
        E::LabelMissing => "LabelMissing",
        E::DescriptionMissing => "DescriptionMissing",
        E::FrequencyInvalid { .. } => "FrequencyInvalid",
        E::DayOfPeriodOutOfRange { .. } => "DayOfPeriodOutOfRange",
        E::CategoryInvalid { .. } => "CategoryInvalid",
        E::AmountNotPositive { .. } => "AmountNotPositive",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn good() -> RecurringInput {
        RecurringInput {
            label: "Microsoft 365 Business".into(),
            vendor_contact_id: None,
            frequency: "monthly".into(),
            day_of_period: 1,
            next_due_date: d(2026, 6, 1),
            expected_amount_cents: 1_190,
            category: "software".into(),
            description_template: "Microsoft 365 Business — Monatsabo".into(),
            auto_create_expense: true,
            reverse_charge_13b_default: false,
        }
    }

    #[test]
    fn valid_recurring_passes() {
        assert!(validate_recurring(&good()).is_ok());
    }

    #[test]
    fn invalid_frequency_flagged() {
        let mut r = good();
        r.frequency = "weekly".into();
        let err = validate_recurring(&r).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, RecurringValidationError::FrequencyInvalid { .. })));
    }

    #[test]
    fn day_of_period_out_of_range_flagged() {
        let mut r = good();
        r.day_of_period = 0;
        assert!(validate_recurring(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringValidationError::DayOfPeriodOutOfRange { .. })));
        r.day_of_period = 32;
        assert!(validate_recurring(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringValidationError::DayOfPeriodOutOfRange { .. })));
    }

    #[test]
    fn invalid_category_flagged() {
        let mut r = good();
        r.category = "bananas".into();
        assert!(validate_recurring(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringValidationError::CategoryInvalid { .. })));
    }

    #[test]
    fn non_positive_amount_flagged() {
        let mut r = good();
        r.expected_amount_cents = 0;
        assert!(validate_recurring(&r)
            .unwrap_err()
            .iter()
            .any(|x| matches!(x, RecurringValidationError::AmountNotPositive { .. })));
    }

    #[test]
    fn missing_label_and_description_flagged() {
        let mut r = good();
        r.label = "  ".into();
        r.description_template = "".into();
        let err = validate_recurring(&r).unwrap_err();
        assert!(err.contains(&RecurringValidationError::LabelMissing));
        assert!(err.contains(&RecurringValidationError::DescriptionMissing));
    }

    #[test]
    fn frequency_months_mapping() {
        assert_eq!(Frequency::Monthly.months(), 1);
        assert_eq!(Frequency::Quarterly.months(), 3);
        assert_eq!(Frequency::SemiAnnually.months(), 6);
        assert_eq!(Frequency::Annually.months(), 12);
    }

    #[test]
    fn frequency_db_roundtrip() {
        for f in [
            Frequency::Monthly,
            Frequency::Quarterly,
            Frequency::SemiAnnually,
            Frequency::Annually,
        ] {
            assert_eq!(Frequency::from_db(f.as_db()), Some(f));
        }
        assert_eq!(Frequency::from_db("nope"), None);
    }

    #[test]
    fn monthly_advances_one_month() {
        let next = compute_next_due_date(Frequency::Monthly, 15, d(2026, 1, 15));
        assert_eq!(next, d(2026, 2, 15));
    }

    #[test]
    fn quarterly_advances_three_months_over_year_boundary() {
        let next = compute_next_due_date(Frequency::Quarterly, 1, d(2025, 11, 1));
        assert_eq!(next, d(2026, 2, 1));
    }

    #[test]
    fn annually_advances_one_year_keeping_leap_clamp() {
        // 29.02. existiert nur in Schaltjahren → bei jährlich auf 28. geklemmt.
        let next = compute_next_due_date(Frequency::Annually, 29, d(2024, 2, 29));
        assert_eq!(next, d(2025, 2, 28));
    }

    #[test]
    fn day_31_clamps_to_february_then_recovers_in_march() {
        // monatlich, Stichtag 31: Jan→Feb klemmt auf 28, Feb→Mär findet 31 zurück.
        let from = d(2026, 1, 31);
        let feb = compute_next_due_date(Frequency::Monthly, 31, from);
        assert_eq!(feb, d(2026, 2, 28));
        let mar = compute_next_due_date(Frequency::Monthly, 31, feb);
        assert_eq!(
            mar,
            d(2026, 3, 31),
            "Stichtag basiert auf day_of_period, nicht auf geklemmtem Tag"
        );
    }

    #[test]
    fn day_31_in_leap_february() {
        let feb = compute_next_due_date(Frequency::Monthly, 31, d(2028, 1, 31));
        assert_eq!(feb, d(2028, 2, 29), "Schaltjahr → 29.02.");
    }

    #[test]
    fn is_due_inclusive_of_today() {
        let today = d(2026, 5, 20);
        assert!(is_due(d(2026, 5, 20), today), "== today ist fällig");
        assert!(is_due(d(2026, 5, 19), today), "Vergangenheit ist fällig");
        assert!(!is_due(d(2026, 5, 21), today), "Zukunft ist nicht fällig");
    }
}
