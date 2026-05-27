//! §19-UStG-Logik (Functional Core). Pure, kein I/O.
//!
//! ## Anwendung
//!
//! - Im `seller_profile` steuert `is_kleinunternehmer` (0/1) den Modus.
//! - `is_active()` ist der einzige autoritative Check, ob §19 gilt.
//! - `hinweis_text()` liefert den exakten Klausel-Text, der laut PRD-Hardline
//!   **wortgleich** auf XRechnung (BT-22 Note) UND PDF erscheinen muss.
//! - `waiver_deadline()` berechnet das Ende der 5-Jahres-Bindung nach
//!   §19 Abs. 2 UStG (Verzicht auf die Kleinunternehmerregelung).
//!
//! `assert_no_vat()` (§14c-Schutz auf `invoice_items`) prüft seit Block 3,
//! dass bei aktivem §19 weder ein USt-Betrag noch ein nicht-Exempt-Steuer-
//! Kategorie-Code auf einem Item steht. Das ist der Defense-in-Depth-Layer
//! unter der UI-Sperre.

use chrono::{Datelike, NaiveDate};

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

/// Minimal-Sicht auf das, was wir aus dem `seller_profile` für §19-Entscheidungen
/// brauchen. Hält den Domain-Code testbar ohne den vollen DB-Row-Typ.
#[derive(Debug, Clone, Copy)]
pub struct KleinunternehmerStatus {
    pub is_kleinunternehmer: bool,
    /// Datum des Verzichts auf §19 (Wechsel zur Regelbesteuerung). Setzt
    /// die 5-Jahres-Bindung in Gang.
    pub waived_since: Option<NaiveDate>,
}

/// §19 aktiv?
pub fn is_active(status: &KleinunternehmerStatus) -> bool {
    status.is_kleinunternehmer
}

/// Period-aware: war §19 im Geschäftsjahr `fiscal_year` aktiv?
///
/// Anders als [`is_active`] entscheidet das nicht anhand des aktuellen
/// Profil-Stands, sondern berücksichtigt den Verzichts-Stichtag
/// (`waived_since`). Wichtig fürs ELSTER-Formular und das EÜR-PDF eines
/// **zurückliegenden** Jahres: wer 2027 auf Regelbesteuerung wechselt,
/// hat 2026 trotzdem als Kleinunternehmer abgerechnet — die Anlage EÜR
/// für 2026 gehört in Zeile 12 (§19), nicht Zeile 15 (Regel).
///
/// Logik:
/// - Kein Profil bzw. `is_kleinunternehmer = true` ohne Verzichts-Datum:
///   §19 gilt für jedes Jahr (Default in Klein.Buch).
/// - Verzichts-Datum gesetzt: für Jahre **vor** dem Verzichts-Jahr gilt
///   weiter §19; ab dem Verzichts-Jahr gilt der aktuelle Status (typisch
///   Regelbesteuerung).
/// - Mehrfacher Wechsel (Rückkehr zu §19) ist im Schema heute nicht
///   modelliert — die Rückkehr setzt `waived_since` auf `NULL`, womit
///   die History für Vor-Jahre verloren geht. Bekannte Lücke
///   (REVIEW-V2026.5 R6/Post-v1.0).
pub fn is_active_for_year(status: &KleinunternehmerStatus, fiscal_year: i32) -> bool {
    match status.waived_since {
        Some(d) if fiscal_year < d.year() => true,
        _ => status.is_kleinunternehmer,
    }
}

/// Muss der §19-Hinweis auf das Dokument? — derzeit identisch mit `is_active`,
/// als eigene Funktion gehalten falls Sonderfälle (Storno, Auslandsumsatz)
/// später unterschieden werden müssen.
pub fn must_show_hinweis(status: &KleinunternehmerStatus) -> bool {
    is_active(status)
}

/// 5-Jahres-Bindung nach Verzicht: Der Unternehmer ist ab `since` für mindestens
/// 5 volle Kalenderjahre an die Regelbesteuerung gebunden (§19 Abs. 2 UStG).
///
/// Wir geben das **erste Datum** zurück, an dem ein Rückwechsel zu §19 wieder
/// möglich ist — konservativ der 01.01. des 6. Folgejahres.
///
/// Beispiel: Verzicht am 2026-03-15 → Rückwechsel frühestens 2032-01-01.
/// Auch bei Verzicht am 2026-12-31 → frühestens 2032-01-01 (volle 5 Kalenderjahre
/// 2027–2031 sind verbindlich gebunden).
pub fn waiver_deadline(since: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(since.year() + 6, 1, 1).expect("valid year")
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
    status: &KleinunternehmerStatus,
    items: &[ItemVatCheck<'_>],
) -> Result<(), Vec<NoVatViolation>> {
    if !is_active(status) {
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

    #[test]
    fn is_active_reflects_flag() {
        let on = KleinunternehmerStatus {
            is_kleinunternehmer: true,
            waived_since: None,
        };
        let off = KleinunternehmerStatus {
            is_kleinunternehmer: false,
            waived_since: Some(NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()),
        };
        assert!(is_active(&on));
        assert!(!is_active(&off));
        assert!(must_show_hinweis(&on));
        assert!(!must_show_hinweis(&off));
    }

    #[test]
    fn waiver_deadline_is_jan_first_six_years_later() {
        let since = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        assert_eq!(
            waiver_deadline(since),
            NaiveDate::from_ymd_opt(2032, 1, 1).unwrap()
        );
    }

    #[test]
    fn waiver_deadline_year_end_edge() {
        let since = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        // Auch hier: Rückwechsel frühestens 2032-01-01, nicht 2031 —
        // 5 volle Kalenderjahre 2027–2031 sind gebunden.
        assert_eq!(
            waiver_deadline(since),
            NaiveDate::from_ymd_opt(2032, 1, 1).unwrap()
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

    fn klein_on() -> KleinunternehmerStatus {
        KleinunternehmerStatus {
            is_kleinunternehmer: true,
            waived_since: None,
        }
    }

    fn klein_off() -> KleinunternehmerStatus {
        KleinunternehmerStatus {
            is_kleinunternehmer: false,
            waived_since: Some(NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()),
        }
    }

    #[test]
    fn assert_no_vat_passes_when_all_items_exempt_and_zero() {
        let items = [item(1, "E", 0, 0.0), item(2, "E", 0, 0.0)];
        assert!(assert_no_vat(&klein_on(), &items).is_ok());
    }

    #[test]
    fn assert_no_vat_is_noop_when_klein_inactive() {
        // Bei Regelbesteuerung läuft hier nichts; die normale USt-Logik
        // greift woanders.
        let items = [item(1, "S", 1900, 19.0)];
        assert!(assert_no_vat(&klein_off(), &items).is_ok());
    }

    #[test]
    fn assert_no_vat_flags_non_exempt_category() {
        let items = [item(1, "S", 0, 0.0)];
        let err = assert_no_vat(&klein_on(), &items).unwrap_err();
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
        let err = assert_no_vat(&klein_on(), &items).unwrap_err();
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
        let err = assert_no_vat(&klein_on(), &items).unwrap_err();
        assert_eq!(err.len(), 2);
        let positions: Vec<u32> = err.iter().map(|v| v.position).collect();
        assert_eq!(positions, vec![1, 3]);
    }

    #[test]
    fn it_compiles() {}

    // R2-002 — period-aware §19-Check.
    #[test]
    fn period_aware_no_waiver_returns_current_flag() {
        let klein = KleinunternehmerStatus {
            is_kleinunternehmer: true,
            waived_since: None,
        };
        assert!(is_active_for_year(&klein, 2025));
        assert!(is_active_for_year(&klein, 2026));
        assert!(is_active_for_year(&klein, 2030));

        let regel = KleinunternehmerStatus {
            is_kleinunternehmer: false,
            waived_since: None,
        };
        assert!(!is_active_for_year(&regel, 2026));
    }

    #[test]
    fn period_aware_pre_waiver_year_keeps_paragraph_19() {
        // Verzicht zum 2027-04-01 → 2025/2026 waren §19, 2027ff Regelbesteuerung.
        let status = KleinunternehmerStatus {
            is_kleinunternehmer: false,
            waived_since: Some(NaiveDate::from_ymd_opt(2027, 4, 1).unwrap()),
        };
        assert!(
            is_active_for_year(&status, 2025),
            "2025 vor Verzicht → war Kleinunternehmer"
        );
        assert!(
            is_active_for_year(&status, 2026),
            "2026 vor Verzicht → war Kleinunternehmer"
        );
        assert!(
            !is_active_for_year(&status, 2027),
            "2027 ab Verzicht → Regelbesteuerung"
        );
        assert!(
            !is_active_for_year(&status, 2030),
            "2030 lange nach Verzicht → Regelbesteuerung"
        );
    }

    #[test]
    fn period_aware_year_of_waiver_is_regelbesteuerung() {
        // Verzicht zum 2026-12-15 → noch im Jahr 2026 gilt Regelbesteuerung
        // (kein anteiliges Jahres-Splitting in v1.0; STB sagt: Wechsel zum 01.01.).
        let status = KleinunternehmerStatus {
            is_kleinunternehmer: false,
            waived_since: Some(NaiveDate::from_ymd_opt(2026, 12, 15).unwrap()),
        };
        assert!(!is_active_for_year(&status, 2026));
        assert!(is_active_for_year(&status, 2025));
    }
}
