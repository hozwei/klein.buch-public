//! Anfahrtskosten (Functional Core) — Block P1.
//!
//! Reine Berechnung einer Anfahrts-Position aus Kilometern und Kilometersatz.
//! Erzeugt die Felder einer ganz normalen Beleg-Position (km × Satz). Kein I/O,
//! kein Geocoding — `km` kommt aus der UI.
//!
//! ## Geld-Konvention
//! Identisch zu [`crate::domain::invoice::compute_totals`]:
//! `net = round(quantity * unit_price_cents)` (kaufmännische Rundung). Damit
//! stimmt der Lock-Recompute exakt mit der hier berechneten Vorschau überein.

use serde::{Deserialize, Serialize};

/// UN/ECE-Rec-20-Einheitencode für Kilometer (EN 16931 / XRechnung-tauglich).
pub const UNIT_CODE_KM: &str = "KMT";

/// Eine berechnete Anfahrts-Position, fertig zum Einfügen als Beleg-Position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelLine {
    pub description: String,
    /// Effektive Kilometer (bei Hin & Rück bereits verdoppelt).
    pub quantity: f64,
    pub unit_code: String,
    /// Kilometersatz in Netto-Cent (= Einzelpreis der Position).
    pub unit_price_cents: i64,
    /// `round(quantity * unit_price_cents)`.
    pub net_amount_cents: i64,
    /// 'E' (steuerfrei) bei §19 — Default wie eine neue Position.
    pub tax_category_code: String,
}

/// Reine Berechnung. `km` sind die einfachen (gefahrenen) Kilometer; bei
/// `round_trip` wird verdoppelt.
pub fn compute_travel(km: f64, cost_per_km_cents: i64, round_trip: bool) -> TravelLine {
    let effective_km = if round_trip { km * 2.0 } else { km };
    let net = (effective_km * cost_per_km_cents as f64).round() as i64;
    let rate = format_eur_cents(cost_per_km_cents);
    let km_str = format_km(effective_km);
    let description = if round_trip {
        format!("Anfahrt (Hin & Rück): {km_str} km × {rate} €/km")
    } else {
        format!("Anfahrt: {km_str} km × {rate} €/km")
    };
    TravelLine {
        description,
        quantity: effective_km,
        unit_code: UNIT_CODE_KM.to_string(),
        unit_price_cents: cost_per_km_cents,
        net_amount_cents: net,
        tax_category_code: "E".to_string(),
    }
}

/// "0,50" aus 50 Cent — deutsche Komma-Schreibweise.
fn format_eur_cents(cents: i64) -> String {
    format!("{},{:02}", cents / 100, (cents % 100).abs())
}

/// Kilometer ohne überflüssige Nullen: 42 statt 42,00; 12,5 statt 12,50.
fn format_km(km: f64) -> String {
    if km.fract().abs() < 1e-9 {
        format!("{}", km.round() as i64)
    } else {
        let s = format!("{km:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.replace('.', ",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_distance_rounds_to_cents() {
        // 12,5 km × 50 ct = 625 ct
        let l = compute_travel(12.5, 50, false);
        assert_eq!(l.quantity, 12.5);
        assert_eq!(l.unit_price_cents, 50);
        assert_eq!(l.net_amount_cents, 625);
        assert_eq!(l.unit_code, "KMT");
        assert_eq!(l.tax_category_code, "E");
        assert!(l.description.starts_with("Anfahrt: 12,5 km"));
    }

    #[test]
    fn round_trip_doubles_km_and_sum() {
        let one = compute_travel(20.0, 70, false);
        let two = compute_travel(20.0, 70, true);
        assert_eq!(one.net_amount_cents, 1400);
        assert_eq!(two.quantity, 40.0);
        assert_eq!(two.net_amount_cents, 2800);
        assert!(two.description.contains("Hin & Rück"));
        assert!(two.description.contains("40 km"));
    }

    #[test]
    fn zero_km_is_zero() {
        let l = compute_travel(0.0, 50, false);
        assert_eq!(l.net_amount_cents, 0);
        assert_eq!(l.quantity, 0.0);
    }

    #[test]
    fn matches_invoice_item_net_formula() {
        // Muss bit-genau zur Item-Net-Formel passen: round(qty * price).
        let l = compute_travel(33.3, 42, false);
        let expected = (33.3_f64 * 42.0).round() as i64;
        assert_eq!(l.net_amount_cents, expected);
    }

    #[test]
    fn rate_formatting_german_comma() {
        let l = compute_travel(10.0, 50, false);
        assert!(
            l.description.contains("0,50 €/km"),
            "got: {}",
            l.description
        );
        let l2 = compute_travel(10.0, 100, false);
        assert!(
            l2.description.contains("1,00 €/km"),
            "got: {}",
            l2.description
        );
    }
}
