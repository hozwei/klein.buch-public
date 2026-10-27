//! §19-UStG-Logik (Functional Core). Pure, kein I/O.
//!
//! ## Anwendung
//!
//! - Ob §19 an einem Datum gilt, steht in der Statushistorie
//!   ([`crate::domain::tax_status`], ADR 0042 Nr. 1); der Schalter im
//!   Verkäuferprofil ist seit Migration 0033 nur noch ein Spiegel.
//! - `hinweis_text()` liefert den exakten Klausel-Text, der laut PRD-Hardline
//!   **wortgleich** auf XRechnung (BT-22 Note) UND PDF erscheinen muss.
//!
//! `assert_no_vat()` (§14c-Schutz auf `invoice_items`) prüft seit Block 3,
//! dass bei aktivem §19 weder ein USt-Betrag noch ein nicht-Exempt-Steuer-
//! Kategorie-Code auf einem Item steht. Das ist der Defense-in-Depth-Layer
//! unter der UI-Sperre.

/// EN-16931-Tax-Category-Code für "Exempt — Kleinunternehmer".
pub const EXEMPT_CATEGORY: &str = "E";

/// Wortgleicher Klausel-Text. **NICHT ändern.** Wird sowohl in der XRechnung-
/// XML (`BT-22` Note) als auch auf jeder PDF dargestellt.
///
/// Templates ohne diesen Text werden von `pdf::klausel_check` abgelehnt
/// (Block 3).
pub const HINWEIS_TEXT: &str = "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.";

pub fn hinweis_text() -> &'static str {
    HINWEIS_TEXT
}

/// Minimaler Item-View für den §14c-Check. Wird vom Caller (Invoice-Domain)
/// aus seinen Items aufgebaut — so bleibt `kleinunternehmer` frei von
/// Invoice-Strukturen und entkoppelt testbar.
#[derive(Debug, Clone, Copy)]
pub struct ItemVatCheck<'a> {
    pub position: u32,
    pub tax_category_code: &'a str,
    pub tax_amount_cents: i64,
    pub tax_rate_percent: f64,
}

/// Konkreter Verstoß gegen die §19-Hardline pro Item. Frontend zeigt das
/// item-genau (Position + Reason), damit der User direkt korrigiert.
#[derive(Debug, Clone, PartialEq)]
pub struct NoVatViolation {
    pub position: u32,
    pub reason: NoVatReason,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NoVatReason {
    /// Item-Code ist nicht 'E' (Exempt). Bei §19 Pflicht.
    NonExemptCategoryCode(String),
    /// `tax_amount_cents > 0` — direkter §14c-Risiko-Treffer.
    TaxAmountGreaterZero(i64),
    /// `tax_rate_percent > 0.0`. Auch bei `tax_amount_cents == 0` ein
    /// formaler Verstoß, weil Steuersatz auf Rechnung erscheinen würde.
    TaxRateGreaterZero(f64),
}

/// §14c-Schutz: Wenn §19 aktiv ist, MUSS jedes Item Exempt sein und einen
/// USt-Betrag von 0 zeigen. Aggregiert ALLE Verstöße, gibt nicht beim
/// ersten Treffer auf — die UI soll dem User komplettes Bild zeigen.
///
/// Wenn §19 inaktiv ist, ist diese Funktion ein No-Op (returns `Ok(())`).
/// Die normale USt-Validierung läuft in dem Fall woanders (Regelbesteuerung
/// — Phase 2D).
pub fn assert_no_vat(
    is_kleinunternehmer: bool,
    items: &[ItemVatCheck<'_>],
) -> Result<(), Vec<NoVatViolation>> {
    if !is_kleinunternehmer {
        return Ok(());
    }
    let mut violations = Vec::new();
    for it in items {
        if it.tax_category_code != EXEMPT_CATEGORY {
            violations.push(NoVatViolation {
                position: it.position,
                reason: NoVatReason::NonExemptCategoryCode(it.tax_category_code.to_string()),
            });
        }
        if it.tax_amount_cents != 0 {
            violations.push(NoVatViolation {
                position: it.position,
                reason: NoVatReason::TaxAmountGreaterZero(it.tax_amount_cents),
            });
        }
        if it.tax_rate_percent != 0.0 {
            violations.push(NoVatViolation {
                position: it.position,
                reason: NoVatReason::TaxRateGreaterZero(it.tax_rate_percent),
            });
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hinweis_text_is_wortgleich() {
        assert_eq!(
            hinweis_text(),
            "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."
        );
    }

    fn item(position: u32, code: &str, amount: i64, rate: f64) -> ItemVatCheck<'_> {
        ItemVatCheck {
            position,
            tax_category_code: code,
            tax_amount_cents: amount,
            tax_rate_percent: rate,
        }
    }

    #[test]
    fn assert_no_vat_passes_when_all_items_exempt_and_zero() {
        let items = [item(1, "E", 0, 0.0), item(2, "E", 0, 0.0)];
        assert!(assert_no_vat(true, &items).is_ok());
    }

    #[test]
    fn assert_no_vat_is_noop_when_klein_inactive() {
        // Bei Regelbesteuerung läuft hier nichts; die normale USt-Logik
        // greift woanders.
        let items = [item(1, "S", 1900, 19.0)];
        assert!(assert_no_vat(false, &items).is_ok());
    }

    #[test]
    fn assert_no_vat_flags_non_exempt_category() {
        let items = [item(1, "S", 0, 0.0)];
        let err = assert_no_vat(true, &items).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].position, 1);
        match &err[0].reason {
            NoVatReason::NonExemptCategoryCode(c) => assert_eq!(c, "S"),
            other => panic!("expected NonExemptCategoryCode, got {other:?}"),
        }
    }

    #[test]
    fn assert_no_vat_flags_tax_amount_and_rate_per_item() {
        let items = [item(1, "E", 100, 19.0)];
        let err = assert_no_vat(true, &items).unwrap_err();
        // Zwei Verstöße auf demselben Item: Betrag != 0 UND Rate != 0.
        assert_eq!(err.len(), 2);
        assert!(err
            .iter()
            .any(|v| matches!(v.reason, NoVatReason::TaxAmountGreaterZero(100))));
        assert!(err
            .iter()
            .any(|v| matches!(v.reason, NoVatReason::TaxRateGreaterZero(r) if r == 19.0)));
    }

    #[test]
    fn assert_no_vat_aggregates_violations_across_items() {
        let items = [
            item(1, "S", 0, 0.0),   // 1 Verstoß
            item(2, "E", 0, 0.0),   // ok
            item(3, "E", 100, 0.0), // 1 Verstoß
        ];
        let err = assert_no_vat(true, &items).unwrap_err();
        assert_eq!(err.len(), 2);
        let positions: Vec<u32> = err.iter().map(|v| v.position).collect();
        assert_eq!(positions, vec![1, 3]);
    }
}
