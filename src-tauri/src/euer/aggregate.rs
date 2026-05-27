//! EÜR-Aggregation pro Geschäftsjahr (**Functional Core**, Cash-Basis).
//!
//! Reine Rechnung ohne I/O: nimmt vorgeladene, DB-agnostische Bewegungs-Views
//! (Zahlungseingänge, Storno-Erstattungen, bezahlte Kosten, AfA-Buchungen,
//! Anlagen-Veräußerungen) und ein Geschäftsjahr und liefert einen
//! [`EuerReport`]. Die Shell ([`crate::db::repo::euer`] +
//! [`crate::commands::euer`]) lädt die Views; hier lebt nur die Aggregations-
//! und Periodenzuordnungs-Logik.
//!
//! ## Rechtsgrundlage (Cash-Basis / Zufluss-Abfluss-Prinzip)
//!
//! - **§4 Abs. 3 EStG** — Gewinnermittlung als Einnahmen-Überschuss-Rechnung.
//! - **§11 EStG** — Einnahmen werden im Jahr des tatsächlichen *Zuflusses*
//!   erfasst, Ausgaben im Jahr des *Abflusses*. Daher:
//!   - Eine Rechnungs-Zahlung zählt im Jahr ihres Zahlungseingangs
//!     (`paid_date` der einzelnen Zahlung — Teilzahlungen werden pro Zahlung
//!     dem jeweiligen Jahr zugeordnet).
//!   - Eine **Erstattung nach Storno** ist ein negativer Zufluss (Abfluss) und
//!     wird im Jahr des **Storno-Belegs** gegengerechnet — NICHT rückwirkend im
//!     Ursprungsjahr. Das wahrt zugleich die GoBD-Periodenfestschreibung
//!     (§146 AO): ein abgeschlossenes Jahr ändert sich nie nachträglich.
//!     (Hintergrund: ein Storno-Beleg kann technisch keine eigene Zahlung
//!     tragen — `record_payment` verlangt `amount > 0` und verbietet
//!     Überzahlung; der erstattete Betrag = der auf dem Original tatsächlich
//!     gezahlte Betrag.)
//!   - Eine Kosten-Position zählt im Jahr ihres Zahlungsausgangs (`paid_date`);
//!     nicht bezahlte Kosten (`paid_date IS NULL`) bleiben außen vor.
//! - **AfA** ist eine Jahres-Größe (§7 EStG): sie zählt im `fiscal_year` der
//!   Buchung, unabhängig von Zahlungsdaten.
//! - **Privatentnahmen/-einlagen** sind EÜR-neutral und tauchen hier nicht auf.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::fiscal_year::fiscal_year_bounds;

// ============================================================================
// Eingabe-Views (DB-agnostisch; von der Shell befüllt)
// ============================================================================

/// Ein einzelner Zahlungseingang einer ausgehenden, NICHT-Storno-Rechnung.
/// Teilzahlungen liefern je einen Eintrag (aus der Zahlungs-Historie).
#[derive(Debug, Clone)]
pub struct PaymentView {
    pub paid_date: NaiveDate,
    pub amount_cents: i64,
}

/// Eine Storno-Erstattung: der auf dem stornierten Original tatsächlich
/// gezahlte Betrag, erfasst zum Datum des Storno-Belegs (Abfluss-Jahr).
#[derive(Debug, Clone)]
pub struct StornoReversalView {
    pub storno_date: NaiveDate,
    pub refunded_cents: i64,
}

/// Eine bezahlte (cash-wirksame) Kosten-Position.
#[derive(Debug, Clone)]
pub struct ExpenseView {
    pub paid_date: NaiveDate,
    pub category: String,
    pub gross_cents: i64,
}

/// Eine Jahres-AfA-Buchung (Periodenzuordnung über `fiscal_year`, nicht Datum).
#[derive(Debug, Clone)]
pub struct DepreciationView {
    pub fiscal_year: i32,
    pub amount_cents: i64,
}

/// Eine Anlagen-Veräußerung/-Verschrottung mit Erlös und Restbuchwert-Abgang.
#[derive(Debug, Clone)]
pub struct DisposalView {
    pub disposal_date: NaiveDate,
    pub proceeds_cents: i64,
    pub residual_book_value_cents: i64,
}

/// Gebündelte Eingabe für [`aggregate`].
#[derive(Debug, Clone, Default)]
pub struct EuerInputs {
    pub payments: Vec<PaymentView>,
    pub storno_reversals: Vec<StornoReversalView>,
    pub expenses: Vec<ExpenseView>,
    pub depreciation: Vec<DepreciationView>,
    pub disposals: Vec<DisposalView>,
}

// ============================================================================
// Report (an das Frontend serialisiert, camelCase)
// ============================================================================

/// Eine Ausgaben-Kategorie mit Summe (brutto, cash-wirksam im GJ).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryExpense {
    /// DB-Kategorie-Code (Frontend mappt auf deutsches Label).
    pub category: String,
    pub amount_cents: i64,
}

/// Vollständiger EÜR-Report eines Geschäftsjahres.
///
/// Beträge in Cent (i64). `surplus_cents` = `total_income_cents` −
/// `total_expenses_cents` ist der zu versteuernde Überschuss (kann negativ
/// sein = Verlust).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerReport {
    pub fiscal_year: i32,

    // ---- Betriebseinnahmen ----
    /// Zahlungseingänge aus ausgehenden Rechnungen (cash, ohne Storno-Belege).
    pub invoice_income_cents: i64,
    /// Erstattungen aus Stornos (positiver Betrag; mindert die Einnahmen).
    pub storno_refunds_cents: i64,
    /// Verkaufserlöse veräußerter Anlagen (Betriebseinnahme).
    pub disposal_proceeds_cents: i64,
    /// Summe Betriebseinnahmen = invoice_income − storno_refunds + disposal_proceeds.
    pub total_income_cents: i64,

    // ---- Betriebsausgaben ----
    /// Bezahlte Kosten nach Kategorie (absteigend nach Betrag sortiert).
    pub expenses_by_category: Vec<CategoryExpense>,
    /// Summe der bezahlten Kosten (alle Kategorien).
    pub expenses_total_cents: i64,
    /// Summe der AfA-Buchungen des Geschäftsjahres.
    pub depreciation_total_cents: i64,
    /// Restbuchwert-Abgänge veräußerter Anlagen (außerordentliche Ausgabe).
    pub disposal_book_value_cents: i64,
    /// Summe Betriebsausgaben = expenses_total + depreciation_total + disposal_book_value.
    pub total_expenses_cents: i64,

    // ---- Abgeleitete Info ----
    /// Veräußerungsgewinn/-verlust = disposal_proceeds − disposal_book_value
    /// (positiv = Gewinn, negativ = Verlust). Nur informativ; bereits im
    /// Überschuss enthalten.
    pub disposal_gain_loss_cents: i64,

    // ---- Ergebnis ----
    /// Überschuss (Gewinn/Verlust) = total_income − total_expenses.
    pub surplus_cents: i64,
}

// ============================================================================
// Aggregation
// ============================================================================

fn is_in_year(date: &NaiveDate, year: i32) -> bool {
    let (start, end) = fiscal_year_bounds(year);
    *date >= start && *date <= end
}

/// Aggregiert alle Bewegungen eines Geschäftsjahres zu einem [`EuerReport`].
///
/// Filtert jede Eingabe-Liste nach `fiscal_year` (Datum innerhalb der
/// GJ-Grenzen bzw. — bei AfA — `fiscal_year`-Feld) und summiert auf.
/// Kosten werden nach `category` gruppiert.
pub fn aggregate(fiscal_year: i32, inputs: &EuerInputs) -> EuerReport {
    // ---- Einnahmen: Zahlungseingänge (cash) ----
    let invoice_income_cents: i64 = inputs
        .payments
        .iter()
        .filter(|p| is_in_year(&p.paid_date, fiscal_year))
        .map(|p| p.amount_cents)
        .sum();

    // ---- Storno-Erstattungen (Abfluss-Jahr = Storno-Datum) ----
    let storno_refunds_cents: i64 = inputs
        .storno_reversals
        .iter()
        .filter(|s| is_in_year(&s.storno_date, fiscal_year))
        .map(|s| s.refunded_cents)
        .sum();

    // ---- Anlagen-Veräußerung ----
    let mut disposal_proceeds_cents: i64 = 0;
    let mut disposal_book_value_cents: i64 = 0;
    for d in inputs
        .disposals
        .iter()
        .filter(|d| is_in_year(&d.disposal_date, fiscal_year))
    {
        disposal_proceeds_cents += d.proceeds_cents;
        disposal_book_value_cents += d.residual_book_value_cents;
    }

    let total_income_cents = invoice_income_cents - storno_refunds_cents + disposal_proceeds_cents;

    // ---- Ausgaben: bezahlte Kosten, gruppiert nach Kategorie ----
    let mut by_category: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    for e in inputs
        .expenses
        .iter()
        .filter(|e| is_in_year(&e.paid_date, fiscal_year))
    {
        *by_category.entry(e.category.clone()).or_insert(0) += e.gross_cents;
    }
    let expenses_total_cents: i64 = by_category.values().sum();
    // Absteigend nach Betrag, bei Gleichstand alphabetisch nach Kategorie
    // (BTreeMap liefert alphabetisch; stabiler sort_by hält das als Tiebreak).
    let mut expenses_by_category: Vec<CategoryExpense> = by_category
        .into_iter()
        .map(|(category, amount_cents)| CategoryExpense {
            category,
            amount_cents,
        })
        .collect();
    expenses_by_category.sort_by_key(|c| std::cmp::Reverse(c.amount_cents));

    // ---- AfA (Jahres-Größe) ----
    let depreciation_total_cents: i64 = inputs
        .depreciation
        .iter()
        .filter(|d| d.fiscal_year == fiscal_year)
        .map(|d| d.amount_cents)
        .sum();

    let total_expenses_cents =
        expenses_total_cents + depreciation_total_cents + disposal_book_value_cents;

    let disposal_gain_loss_cents = disposal_proceeds_cents - disposal_book_value_cents;
    let surplus_cents = total_income_cents - total_expenses_cents;

    EuerReport {
        fiscal_year,
        invoice_income_cents,
        storno_refunds_cents,
        disposal_proceeds_cents,
        total_income_cents,
        expenses_by_category,
        expenses_total_cents,
        depreciation_total_cents,
        disposal_book_value_cents,
        total_expenses_cents,
        disposal_gain_loss_cents,
        surplus_cents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn pay(date: NaiveDate, cents: i64) -> PaymentView {
        PaymentView {
            paid_date: date,
            amount_cents: cents,
        }
    }

    fn exp(date: NaiveDate, cat: &str, cents: i64) -> ExpenseView {
        ExpenseView {
            paid_date: date,
            category: cat.to_string(),
            gross_cents: cents,
        }
    }

    #[test]
    fn empty_inputs_yield_zeroes() {
        let r = aggregate(2026, &EuerInputs::default());
        assert_eq!(r.fiscal_year, 2026);
        assert_eq!(r.invoice_income_cents, 0);
        assert_eq!(r.total_income_cents, 0);
        assert_eq!(r.total_expenses_cents, 0);
        assert_eq!(r.surplus_cents, 0);
        assert!(r.expenses_by_category.is_empty());
    }

    #[test]
    fn income_counts_only_payments_in_year() {
        let inputs = EuerInputs {
            payments: vec![
                pay(d(2026, 1, 15), 100_000),
                pay(d(2026, 12, 31), 50_000),
                pay(d(2025, 12, 31), 999_999), // Vorjahr
                pay(d(2027, 1, 1), 888_888),   // Folgejahr
            ],
            ..Default::default()
        };
        let r = aggregate(2026, &inputs);
        assert_eq!(r.invoice_income_cents, 150_000);
        assert_eq!(r.total_income_cents, 150_000);
        assert_eq!(r.surplus_cents, 150_000);
    }

    #[test]
    fn partial_payments_split_across_years() {
        // §11 EStG: 500 € Dez 2025 + 500 € Jan 2026 → je 500 € im jeweiligen Jahr.
        let payments = vec![pay(d(2025, 12, 20), 50_000), pay(d(2026, 1, 10), 50_000)];
        let r25 = aggregate(
            2025,
            &EuerInputs {
                payments: payments.clone(),
                ..Default::default()
            },
        );
        let r26 = aggregate(
            2026,
            &EuerInputs {
                payments,
                ..Default::default()
            },
        );
        assert_eq!(r25.invoice_income_cents, 50_000);
        assert_eq!(r26.invoice_income_cents, 50_000);
    }

    #[test]
    fn storno_reversal_subtracts_at_storno_year() {
        // Zahlung 2026 +1000 €, Storno-Erstattung 2027 −1000 €.
        let payments = vec![pay(d(2026, 3, 1), 100_000)];
        let stornos = vec![StornoReversalView {
            storno_date: d(2027, 2, 1),
            refunded_cents: 100_000,
        }];
        let r26 = aggregate(
            2026,
            &EuerInputs {
                payments: payments.clone(),
                storno_reversals: stornos.clone(),
                ..Default::default()
            },
        );
        let r27 = aggregate(
            2027,
            &EuerInputs {
                payments,
                storno_reversals: stornos,
                ..Default::default()
            },
        );
        // 2026 bleibt stabil bei +1000 € (keine Rückwirkung).
        assert_eq!(r26.invoice_income_cents, 100_000);
        assert_eq!(r26.storno_refunds_cents, 0);
        assert_eq!(r26.total_income_cents, 100_000);
        // 2027 verbucht die Erstattung als negativen Zufluss.
        assert_eq!(r27.invoice_income_cents, 0);
        assert_eq!(r27.storno_refunds_cents, 100_000);
        assert_eq!(r27.total_income_cents, -100_000);
        assert_eq!(r27.surplus_cents, -100_000);
    }

    #[test]
    fn same_year_payment_and_storno_net_to_zero() {
        let r = aggregate(
            2026,
            &EuerInputs {
                payments: vec![pay(d(2026, 3, 1), 100_000)],
                storno_reversals: vec![StornoReversalView {
                    storno_date: d(2026, 4, 1),
                    refunded_cents: 100_000,
                }],
                ..Default::default()
            },
        );
        assert_eq!(r.total_income_cents, 0);
        assert_eq!(r.surplus_cents, 0);
    }

    #[test]
    fn expenses_grouped_and_sorted_desc() {
        let inputs = EuerInputs {
            expenses: vec![
                exp(d(2026, 2, 1), "office", 5_000),
                exp(d(2026, 3, 1), "software", 30_000),
                exp(d(2026, 4, 1), "office", 2_500),
                exp(d(2025, 5, 1), "software", 99_999), // Vorjahr → ignoriert
            ],
            ..Default::default()
        };
        let r = aggregate(2026, &inputs);
        assert_eq!(r.expenses_total_cents, 37_500);
        // Größte Kategorie zuerst.
        assert_eq!(r.expenses_by_category[0].category, "software");
        assert_eq!(r.expenses_by_category[0].amount_cents, 30_000);
        assert_eq!(r.expenses_by_category[1].category, "office");
        assert_eq!(r.expenses_by_category[1].amount_cents, 7_500);
    }

    #[test]
    fn depreciation_matched_by_fiscal_year() {
        let inputs = EuerInputs {
            depreciation: vec![
                DepreciationView {
                    fiscal_year: 2026,
                    amount_cents: 20_000,
                },
                DepreciationView {
                    fiscal_year: 2026,
                    amount_cents: 5_000,
                },
                DepreciationView {
                    fiscal_year: 2025,
                    amount_cents: 9_999,
                },
            ],
            ..Default::default()
        };
        let r = aggregate(2026, &inputs);
        assert_eq!(r.depreciation_total_cents, 25_000);
        assert_eq!(r.total_expenses_cents, 25_000);
        assert_eq!(r.surplus_cents, -25_000);
    }

    #[test]
    fn disposal_gain_increases_surplus() {
        // Verkauf 30.000 ct, Restbuchwert 10.000 ct → Gewinn 20.000 ct.
        let r = aggregate(
            2026,
            &EuerInputs {
                disposals: vec![DisposalView {
                    disposal_date: d(2026, 6, 1),
                    proceeds_cents: 30_000,
                    residual_book_value_cents: 10_000,
                }],
                ..Default::default()
            },
        );
        assert_eq!(r.disposal_proceeds_cents, 30_000);
        assert_eq!(r.disposal_book_value_cents, 10_000);
        assert_eq!(r.disposal_gain_loss_cents, 20_000);
        assert_eq!(r.total_income_cents, 30_000);
        assert_eq!(r.total_expenses_cents, 10_000);
        assert_eq!(r.surplus_cents, 20_000);
    }

    #[test]
    fn disposal_loss_via_scrap() {
        // Verschrottung: Erlös 0, Restbuchwert 10.000 ct → Verlust −10.000 ct.
        let r = aggregate(
            2026,
            &EuerInputs {
                disposals: vec![DisposalView {
                    disposal_date: d(2026, 6, 1),
                    proceeds_cents: 0,
                    residual_book_value_cents: 10_000,
                }],
                ..Default::default()
            },
        );
        assert_eq!(r.disposal_gain_loss_cents, -10_000);
        assert_eq!(r.surplus_cents, -10_000);
    }

    #[test]
    fn full_surplus_math() {
        let inputs = EuerInputs {
            payments: vec![pay(d(2026, 1, 1), 500_000), pay(d(2026, 7, 1), 300_000)],
            storno_reversals: vec![StornoReversalView {
                storno_date: d(2026, 8, 1),
                refunded_cents: 100_000,
            }],
            expenses: vec![
                exp(d(2026, 2, 1), "software", 60_000),
                exp(d(2026, 3, 1), "office", 40_000),
            ],
            depreciation: vec![DepreciationView {
                fiscal_year: 2026,
                amount_cents: 50_000,
            }],
            disposals: vec![DisposalView {
                disposal_date: d(2026, 9, 1),
                proceeds_cents: 20_000,
                residual_book_value_cents: 5_000,
            }],
        };
        let r = aggregate(2026, &inputs);
        // Einnahmen: 800.000 − 100.000 + 20.000 = 720.000
        assert_eq!(r.total_income_cents, 720_000);
        // Ausgaben: 100.000 (Kosten) + 50.000 (AfA) + 5.000 (Restbuchwert) = 155.000
        assert_eq!(r.total_expenses_cents, 155_000);
        // Überschuss: 720.000 − 155.000 = 565.000
        assert_eq!(r.surplus_cents, 565_000);
    }

    #[test]
    fn it_compiles() {}
}
