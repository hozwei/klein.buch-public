//! §33 UStDV — Kleinbetragsrechnung (≤ 250 € brutto).
//!
//! Vereinfachte Pflichtangaben:
//! - Empfänger-Anschrift entfällt
//! - Fortlaufende Rechnungsnummer entfällt
//! - Steuernummer/USt-IdNr. des Leistenden entfällt
//!
//! Die §19-Klausel bleibt verpflichtend, wenn der Aussteller Klein-
//! unternehmer ist.
//!
//! Klein.Buch erzeugt **trotzdem** vollständige Rechnungen mit Nummer,
//! Empfänger-Adresse und Steuernummer (Defense-in-Depth + Schwellen-
//! sicherheit). Diese Modul-Funktion ist die Wahrheits-Quelle für den
//! milderen §14-Check.

/// Schwelle in Cent (entspricht 250,00 € brutto).
pub const KLEINBETRAG_THRESHOLD_CENTS: i64 = 25_000;

/// `true`, wenn der Bruttobetrag unter oder gleich der Schwelle liegt.
pub fn is_applicable(gross_amount_cents: i64) -> bool {
    gross_amount_cents <= KLEINBETRAG_THRESHOLD_CENTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_is_25000_cents() {
        assert_eq!(KLEINBETRAG_THRESHOLD_CENTS, 25_000);
    }

    #[test]
    fn boundary_is_inclusive() {
        // 250,00 € → noch Kleinbetrag (≤, nicht <).
        assert!(is_applicable(25_000));
    }

    #[test]
    fn one_cent_over_is_not_applicable() {
        assert!(!is_applicable(25_001));
    }

    #[test]
    fn zero_and_negative_are_applicable() {
        // Edge: Negativ-Beträge (z. B. Storno) — formal Kleinbetrag.
        // Storno-Belege haben aber eh den Original-Status; das ist
        // hier kein Praxis-Fall, nur Robustheit.
        assert!(is_applicable(0));
        assert!(is_applicable(-100));
    }
}
