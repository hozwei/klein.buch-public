//! Kosten-Domain (Functional Core) — Phase 2B, Block 9.
//!
//! ## Inhalte
//!
//! - [`ExpenseInput`] — User-Eingabe für eine erfasste Kosten-Position.
//! - [`VALID_CATEGORIES`] — die EÜR-Kategorien (BMF-orientiert), 1:1 zum
//!   CHECK-Constraint in `0007_expenses.sql`.
//! - [`compute_gross`] — pure Konsistenz-Helfer (net + tax).
//! - [`validate_expense`] — Struktur- und Betrags-Checks.
//!
//! Alle Funktionen pure, keine I/O. Belegnummern-Allokation und Persistenz
//! liegen in [`crate::db::numbering`] und [`crate::db::repo::expenses`].
//!
//! ## §19 / §13b — wichtige Abgrenzung
//!
//! Kosten sind die **Eingangs-Seite**. Die §19-Hardline (kein USt-Ausweis)
//! gilt ausschließlich für **ausgehende** Belege (Rechnungen/Angebote). Eine
//! Eingangsrechnung eines Lieferanten DARF Umsatzsteuer enthalten — der
//! Kleinunternehmer zahlt sie, kann aber keine Vorsteuer ziehen. Für die EÜR
//! (Cash-Basis, Block 13) ist deshalb der **Brutto**-Betrag die
//! Betriebsausgabe. Diese Domain erzwingt daher **keine** USt-Freiheit (Ausnahme
//! §13b, unten).
//!
//! §13b (Reverse-Charge): Der Lieferant stellt **netto** in Rechnung, die
//! Steuer schuldet der Leistungsempfänger — auch ein Kleinunternehmer
//! (§13b Abs. 5 Satz 1 und Abs. 8 UStG). Klein.Buch
//! berechnet und meldet sie nicht (PRD G16: "Berechnung extern"); das Flag
//! markiert die Kosten. Erzwungen wird nur: bei gesetztem Flag ist
//! `tax_amount_cents == 0` (eine Lieferantenrechnung mit §13b weist keine USt
//! aus; R1-S2-13B, Lauf F7). Die selbst geschuldete USt ist bei Zahlung an das
//! Finanzamt eine eigene Betriebsausgabe (Abflussprinzip, §11 Abs. 2 EStG).
//!
//! ## Geld-Konvention
//!
//! Alle Beträge in **Integer-Cents** (i64). `gross == net + tax` wird hart
//! geprüft (kein implizites Nachrechnen) — der Caller liefert konsistente Werte.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Erlaubte EÜR-Kategorien — synchron zum CHECK-Constraint in
/// `0007_expenses.sql`. **Niemals** divergieren lassen.
pub const VALID_CATEGORIES: &[&str] = &[
    "office",
    "software",
    "hardware",
    "travel",
    "services",
    "goods",
    "communications",
    "vehicle",
    "rent",
    "insurance",
    "training",
    "fees",
    "marketing",
    "other",
];

// ---- Input-Typ -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseInput {
    /// Beleg-Datum (Pflicht). Bestimmt das Geschäftsjahr der Belegnummer.
    pub expense_date: NaiveDate,
    /// Zahlungsausgang (cash-basis-relevant für die EÜR). `None`, solange
    /// die Kosten erfasst aber noch nicht bezahlt sind.
    pub paid_date: Option<NaiveDate>,
    /// Konto, von dem gezahlt wurde (optional).
    pub paid_from_account_id: Option<String>,
    /// Verknüpfter Lieferanten-Kontakt (optional).
    pub vendor_contact_id: Option<String>,
    /// Lieferanten-Name als Snapshot (Pflicht, auch ohne Kontakt).
    pub vendor_name: String,
    /// Rechnungsnummer des Lieferanten (optional).
    pub vendor_invoice_number: Option<String>,
    /// EÜR-Kategorie — muss in [`VALID_CATEGORIES`] sein.
    pub category: String,
    pub description: String,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    /// §13b Reverse-Charge: mit Flag muss `tax_amount_cents` 0 sein (ADR 0039 Nr. 143).
    pub reverse_charge_13b: bool,
    pub notes: Option<String>,
}

// ---- Helpers ---------------------------------------------------------------

/// Pure: Brutto aus Netto + Steuer. Für UI-Vorbefüllung und Tests.
pub fn compute_gross(net_cents: i64, tax_cents: i64) -> i64 {
    net_cents + tax_cents
}

// ---- Validation ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ExpenseValidationError {
    VendorNameMissing,
    DescriptionMissing,
    CurrencyEmpty,
    /// Währung gesetzt, aber nicht in der Whitelist (v0.1: nur `EUR`). KoSIT
    /// blockt Non-EUR später eh; der Domain-Layer lehnt es vorher ab. EÜR-
    /// Cash-Basis (Brutto-Cent) rechnet ohne FX-Umrechnung — Non-EUR-Beträge
    /// wären semantisch falsch. R1-015 (v2026.5-Re-Review).
    CurrencyUnsupported(String),
    CategoryInvalid {
        category: String,
    },
    NetNegative {
        net: i64,
    },
    TaxNegative {
        tax: i64,
    },
    /// `gross_amount_cents != net_amount_cents + tax_amount_cents`.
    GrossMismatch {
        net: i64,
        tax: i64,
        gross: i64,
    },
    /// §13b gesetzt, aber ein USt-Betrag erfasst: bei Reverse-Charge weist der
    /// Lieferant keine Umsatzsteuer aus (R1-S2-13B).
    ReverseChargeWithTax {
        tax: i64,
    },
    /// Kosten ohne jeden Betrag (alles 0) sind vermutlich ein Eingabefehler.
    AmountZero,
    /// Ein Betrag liegt über der Domänengrenze
    /// ([`crate::domain::money::MAX_AMOUNT_CENTS`]); F5-fix.
    AmountOutOfRange,
    /// Beleg-Datum liegt in der Zukunft (ein erfasster Beleg kann nicht aus der
    /// Zukunft stammen).
    ExpenseDateInFuture {
        expense_date: NaiveDate,
        today: NaiveDate,
    },
    /// Zahldatum liegt in der Zukunft — verstößt gegen das Abfluss-Prinzip
    /// (§11 EStG, Cash-Basis-EÜR): die Zahlung hat noch nicht stattgefunden.
    PaidDateInFuture {
        paid_date: NaiveDate,
        today: NaiveDate,
    },
}

/// Validiert eine [`ExpenseInput`]. Aggregiert alle Fehler (kein Fail-Fast).
/// Leerer Vec → die Kosten dürfen erfasst werden. `today` (Europe/Berlin) wird
/// injiziert, damit der Functional Core pur testbar bleibt.
///
/// Bewusst KEIN §19-/USt-Verbot — siehe Modul-Doku (Eingangs-Seite).
pub fn validate_expense(
    input: &ExpenseInput,
    today: NaiveDate,
) -> Result<(), Vec<ExpenseValidationError>> {
    use ExpenseValidationError as E;
    let mut errs = Vec::new();

    if input.expense_date > today {
        errs.push(E::ExpenseDateInFuture {
            expense_date: input.expense_date,
            today,
        });
    }
    if let Some(pd) = input.paid_date {
        if pd > today {
            errs.push(E::PaidDateInFuture {
                paid_date: pd,
                today,
            });
        }
    }

    if input.vendor_name.trim().is_empty() {
        errs.push(E::VendorNameMissing);
    }
    if input.description.trim().is_empty() {
        errs.push(E::DescriptionMissing);
    }
    let currency = input.currency_code.trim();
    if currency.is_empty() {
        errs.push(E::CurrencyEmpty);
    } else if !crate::domain::invoice::is_supported_currency(currency) {
        // R1-015 (v2026.5-Re-Review): EUR-Whitelist auch auf Eingangs-Seite.
        errs.push(E::CurrencyUnsupported(currency.to_string()));
    }
    if !VALID_CATEGORIES.contains(&input.category.as_str()) {
        errs.push(E::CategoryInvalid {
            category: input.category.clone(),
        });
    }
    if input.net_amount_cents < 0 {
        errs.push(E::NetNegative {
            net: input.net_amount_cents,
        });
    }
    if input.tax_amount_cents < 0 {
        errs.push(E::TaxNegative {
            tax: input.tax_amount_cents,
        });
    }
    let amounts = [
        input.net_amount_cents,
        input.tax_amount_cents,
        input.gross_amount_cents,
    ];
    if !amounts
        .iter()
        .all(|a| crate::domain::money::amount_in_range(*a))
    {
        errs.push(E::AmountOutOfRange);
    }
    if input.net_amount_cents.checked_add(input.tax_amount_cents) != Some(input.gross_amount_cents)
    {
        errs.push(E::GrossMismatch {
            net: input.net_amount_cents,
            tax: input.tax_amount_cents,
            gross: input.gross_amount_cents,
        });
    }
    if input.reverse_charge_13b && input.tax_amount_cents != 0 {
        errs.push(E::ReverseChargeWithTax {
            tax: input.tax_amount_cents,
        });
    }
    if input.net_amount_cents == 0 && input.tax_amount_cents == 0 && input.gross_amount_cents == 0 {
        errs.push(E::AmountZero);
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI). `variant_name`
/// bleibt der maschinenlesbare Code.
pub fn message(e: &ExpenseValidationError) -> String {
    use crate::domain::money::eur;
    use ExpenseValidationError as E;
    match e {
        E::VendorNameMissing => "Lieferant (Name) ist erforderlich.".into(),
        E::DescriptionMissing => "Beschreibung ist erforderlich.".into(),
        E::CurrencyEmpty => "Währung fehlt.".into(),
        E::CurrencyUnsupported(code) => {
            format!("Währung '{code}' wird nicht unterstützt (nur 'EUR').")
        }
        E::CategoryInvalid { category } => format!("Ungültige Kategorie: {category}."),
        E::NetNegative { net } => format!("Netto-Betrag darf nicht negativ sein ({}).", eur(*net)),
        E::TaxNegative { tax } => format!("USt-Betrag darf nicht negativ sein ({}).", eur(*tax)),
        E::GrossMismatch { net, tax, gross } => format!(
            "Brutto ({}) stimmt nicht mit Netto ({}) + USt ({}) überein.",
            eur(*gross),
            eur(*net),
            eur(*tax)
        ),
        E::ReverseChargeWithTax { tax } => format!(
            "§13b (Reverse-Charge): Der Lieferant weist keine Umsatzsteuer aus, der USt-Betrag muss 0 sein (erfasst {}). Die Steuer meldest du selbst in der Umsatzsteuer-Voranmeldung an.",
            eur(*tax)
        ),
        E::AmountZero => "Der Betrag muss größer als 0 sein.".into(),
        E::AmountOutOfRange => crate::domain::money::amount_out_of_range_message(),
        E::ExpenseDateInFuture { expense_date, today } => format!(
            "Beleg-Datum ({expense_date}) liegt in der Zukunft (heute {today})."
        ),
        E::PaidDateInFuture { paid_date, today } => format!(
            "Zahldatum ({paid_date}) liegt in der Zukunft (heute {today}) — die Zahlung hat noch nicht stattgefunden."
        ),
    }
}

/// Maschinenlesbarer Variantenname für DTOs/Logs.
pub fn variant_name(e: &ExpenseValidationError) -> &'static str {
    use ExpenseValidationError as E;
    match e {
        E::VendorNameMissing => "VendorNameMissing",
        E::DescriptionMissing => "DescriptionMissing",
        E::CurrencyEmpty => "CurrencyEmpty",
        E::CurrencyUnsupported(_) => "CurrencyUnsupported",
        E::CategoryInvalid { .. } => "CategoryInvalid",
        E::NetNegative { .. } => "NetNegative",
        E::TaxNegative { .. } => "TaxNegative",
        E::GrossMismatch { .. } => "GrossMismatch",
        E::ReverseChargeWithTax { .. } => "ReverseChargeWithTax",
        E::AmountZero => "AmountZero",
        E::AmountOutOfRange => "AmountOutOfRange",
        E::ExpenseDateInFuture { .. } => "ExpenseDateInFuture",
        E::PaidDateInFuture { .. } => "PaidDateInFuture",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // KB-0006/KB-0007: Fehlertexte rein über i64, Vorzeichen und Cent exakt.
    #[test]
    fn message_formats_amounts_without_float() {
        let m = message(&ExpenseValidationError::NetNegative { net: -1 });
        assert!(m.contains("(-0,01 €)"), "{m}");
        let m = message(&ExpenseValidationError::GrossMismatch {
            net: 10_000,
            tax: 1_900,
            gross: 11_901,
        });
        assert!(
            m.contains("Brutto (119,01 €)") && m.contains("USt (19,00 €)"),
            "{m}"
        );
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()
    }

    fn good() -> ExpenseInput {
        ExpenseInput {
            expense_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            paid_date: Some(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()),
            paid_from_account_id: None,
            vendor_contact_id: None,
            vendor_name: "Microsoft Ireland".into(),
            vendor_invoice_number: Some("MS-99".into()),
            category: "software".into(),
            description: "Microsoft 365 Business".into(),
            net_amount_cents: 10_000,
            tax_amount_cents: 1_900,
            gross_amount_cents: 11_900,
            currency_code: "EUR".into(),
            reverse_charge_13b: false,
            notes: None,
        }
    }

    #[test]
    fn valid_expense_passes() {
        assert!(validate_expense(&good(), today()).is_ok());
    }

    #[test]
    fn vendor_with_vat_is_allowed_on_input_side() {
        // §19 betrifft nur Ausgangsbelege — eine Eingangsrechnung mit USt ist ok.
        let mut e = good();
        e.net_amount_cents = 10_000;
        e.tax_amount_cents = 1_900;
        e.gross_amount_cents = 11_900;
        assert!(validate_expense(&e, today()).is_ok());
    }

    #[test]
    fn gross_mismatch_is_flagged() {
        let mut e = good();
        e.gross_amount_cents = 12_000; // != 10_000 + 1_900
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::GrossMismatch { .. })));
    }

    #[test]
    fn zero_amount_is_flagged() {
        let mut e = good();
        e.net_amount_cents = 0;
        e.tax_amount_cents = 0;
        e.gross_amount_cents = 0;
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err.contains(&ExpenseValidationError::AmountZero));
    }

    #[test]
    fn invalid_category_is_flagged() {
        let mut e = good();
        e.category = "bananas".into();
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::CategoryInvalid { .. })));
    }

    #[test]
    fn missing_vendor_and_description_flagged() {
        let mut e = good();
        e.vendor_name = "   ".into();
        e.description = "".into();
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err.contains(&ExpenseValidationError::VendorNameMissing));
        assert!(err.contains(&ExpenseValidationError::DescriptionMissing));
    }

    #[test]
    fn negative_net_flagged() {
        let mut e = good();
        e.net_amount_cents = -100;
        e.tax_amount_cents = 0;
        e.gross_amount_cents = -100;
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::NetNegative { .. })));
    }

    #[test]
    fn compute_gross_adds_net_and_tax() {
        assert_eq!(compute_gross(10_000, 1_900), 11_900);
        assert_eq!(compute_gross(5_000, 0), 5_000);
    }

    #[test]
    fn future_paid_date_flagged() {
        let mut e = good();
        e.paid_date = Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::PaidDateInFuture { .. })));
    }

    #[test]
    fn future_expense_date_flagged() {
        let mut e = good();
        e.expense_date = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        // paid_date sonst ebenfalls in der Zukunft → mit-anpassen, damit der Test
        // gezielt das Beleg-Datum prüft.
        e.paid_date = None;
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::ExpenseDateInFuture { .. })));
    }

    #[test]
    fn today_dates_are_allowed() {
        // == today ist erlaubt (nur > today blockt).
        let mut e = good();
        e.expense_date = today();
        e.paid_date = Some(today());
        assert!(validate_expense(&e, today()).is_ok());
    }

    #[test]
    fn reverse_charge_13b_net_invoice_is_valid() {
        // §13b setzt keine USt-Berechnung in Gang — der Lieferant stellt netto
        // in Rechnung, Brutto = Netto.
        let mut e = good();
        e.reverse_charge_13b = true;
        e.net_amount_cents = 10_000;
        e.tax_amount_cents = 0;
        e.gross_amount_cents = 10_000;
        assert!(validate_expense(&e, today()).is_ok());
    }

    /// R1-S2-13B (Lauf F7): §13b mit ausgewiesener USt ist ein Erfassungsfehler.
    #[test]
    fn reverse_charge_13b_rejects_tax_amount() {
        let mut e = good();
        e.reverse_charge_13b = true;
        e.net_amount_cents = 10_000;
        e.tax_amount_cents = 1_900;
        e.gross_amount_cents = 11_900;
        let err = validate_expense(&e, today()).unwrap_err();
        assert_eq!(
            err,
            vec![ExpenseValidationError::ReverseChargeWithTax { tax: 1_900 }]
        );
        assert_eq!(variant_name(&err[0]), "ReverseChargeWithTax");
        assert!(message(&err[0]).contains("19,00 €"), "{}", message(&err[0]));
        // Ohne Flag ist dieselbe Rechnung (Vorsteuer nicht abziehbar) gültig.
        e.reverse_charge_13b = false;
        assert!(validate_expense(&e, today()).is_ok());
    }

    // ---- R1-015 (v2026.5-Re-Review) ---------------------------------------

    #[test]
    fn unsupported_currency_flagged() {
        // EÜR rechnet ohne FX-Umrechnung; Non-EUR-Beträge wären semantisch falsch.
        let mut e = good();
        e.currency_code = "USD".into();
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(
            err.iter()
                .any(|x| matches!(x, ExpenseValidationError::CurrencyUnsupported(c) if c == "USD")),
            "expected CurrencyUnsupported(USD), got {err:?}"
        );
    }

    #[test]
    fn empty_currency_takes_precedence_over_unsupported() {
        let mut e = good();
        e.currency_code = "".into();
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err.contains(&ExpenseValidationError::CurrencyEmpty));
        assert!(!err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::CurrencyUnsupported(_))));
    }

    #[test]
    fn eur_is_accepted_case_sensitive() {
        let mut e = good();
        e.currency_code = "EUR".into();
        assert!(validate_expense(&e, today()).is_ok());
        e.currency_code = "eur".into(); // case-sensitive Whitelist
        assert!(validate_expense(&e, today()).is_err());
    }

    /// F5-fix: Beträge über 1 Mrd. € sind ein Validierungsfehler; `net + tax`
    /// läuft auch an der i64-Grenze nicht über (keine Panik).
    #[test]
    fn amounts_above_limit_are_rejected_without_overflow() {
        use crate::domain::money::MAX_AMOUNT_CENTS;
        let mut e = good();
        e.net_amount_cents = MAX_AMOUNT_CENTS;
        e.tax_amount_cents = 0;
        e.gross_amount_cents = MAX_AMOUNT_CENTS;
        assert!(validate_expense(&e, today()).is_ok());
        e.net_amount_cents = MAX_AMOUNT_CENTS + 1;
        e.gross_amount_cents = MAX_AMOUNT_CENTS + 1;
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err.contains(&ExpenseValidationError::AmountOutOfRange));
        e.net_amount_cents = i64::MAX;
        e.tax_amount_cents = 1;
        e.gross_amount_cents = i64::MIN;
        let err = validate_expense(&e, today()).unwrap_err();
        assert!(err.contains(&ExpenseValidationError::AmountOutOfRange));
        assert!(err
            .iter()
            .any(|x| matches!(x, ExpenseValidationError::GrossMismatch { .. })));
        assert!(message(&ExpenseValidationError::AmountOutOfRange).contains("1000000000 €"));
    }
}
