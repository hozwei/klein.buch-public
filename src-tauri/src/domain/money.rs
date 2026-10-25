//! Geld-Helfer (Functional Core). Pure, kein I/O.
//!
//! - Formatierung von Cent-Beträgen für Fehlertexte, rein über `i64`
//!   (Schema-Hardline: kein Float für Geld; KB-0006/KB-0007).
//! - Positions-Arithmetik `Menge × Einzelpreis` und USt mit kaufmännischer
//!   Rundung. Die Menge ist kein Geldbetrag (bis zu 4 Nachkommastellen, wie
//!   CII/XRechnung sie übertragen) und wird deshalb als `f64` multipliziert;
//!   bis 2^53 Cent ist das Produkt exakt (dokumentierte Ausnahme,
//!   `docs/architecture/data-model.md` und ADR 0003, Amendment R1-S2-FLOAT).
//!   Gerundet wird kaufmännisch auf Cent: das Netto je Position, die USt je
//!   (Steuerart, Satz) aus der Summe der Nettobeträge der Gruppe (EN 16931
//!   BR-CO-17, R1-S2-TESTS; `domain::invoice::tax_groups`).
//! - Domänengrenzen für Positionen und Belegsummen (ASVS V5.4.3 / 14.1.2):
//!   endliche Menge `0 < q ≤ 1 000 000` mit höchstens 4 Nachkommastellen,
//!   `|Einzelpreis| ≤ 1 Mrd. €`, Steuersatz `0 ≤ s ≤ 100 %`, Summen ohne
//!   Überlauf. Dieselbe Obergrenze von 1 Mrd. € gilt für jeden einzeln
//!   erfassten Betrag (Kosten netto/USt/brutto, Zahlung, Anschaffungskosten,
//!   Abgangserlös, Privatbewegung, Kosten-Abo; F5-fix), damit Jahressummen in
//!   EÜR, ELSTER und DATEV nie überlaufen. Die Validierung lehnt alles
//!   außerhalb ab; die Rechenfunktionen sättigen statt überzulaufen, damit ein
//!   ungeprüfter Aufruf nie still ein negatives Vorzeichen erzeugt.

/// Größte zulässige Menge je Position.
pub const MAX_QUANTITY: f64 = 1_000_000.0;
/// Höchstens so viele Nachkommastellen hat eine Menge (CII-Format der
/// XRechnung, `einvoice::generator::fmt_quantity`).
pub const QUANTITY_DECIMALS: i32 = 4;
/// Größter zulässiger Einzelpreis (Betrag) in Cent: 1 Mrd. €.
pub const MAX_UNIT_PRICE_CENTS: i64 = 100_000_000_000;
/// Größter zulässiger einzeln erfasster Betrag in Cent (Kosten, Zahlung,
/// Anschaffung, Abgangserlös, Privatbewegung, Kosten-Abo): 1 Mrd. € wie der
/// Einzelpreis einer Position (F5-fix, Überlaufschutz der Jahressummen).
pub const MAX_AMOUNT_CENTS: i64 = MAX_UNIT_PRICE_CENTS;

/// `|Betrag| ≤ MAX_AMOUNT_CENTS` (auch für `i64::MIN` ohne Überlauf).
pub fn amount_in_range(cents: i64) -> bool {
    cents.checked_abs().is_some_and(|a| a <= MAX_AMOUNT_CENTS)
}

/// Text zu den Betragsgrenzen (alle Belegarten).
pub fn amount_out_of_range_message() -> String {
    format!(
        "Der Betrag ist zu groß (höchstens {}).",
        eur_whole(MAX_AMOUNT_CENTS)
    )
}

/// Größter zulässiger Steuersatz in Prozent.
pub const MAX_TAX_RATE_PERCENT: f64 = 100.0;
/// Regelsteuersatz nach §12 Abs. 1 UStG (Vorgabe für neue Positionen bei
/// Regelbesteuerung, KB-0047).
pub const STANDARD_VAT_RATE_PERCENT: f64 = 19.0;

/// Cent → deutscher Eurobetrag ohne Tausenderpunkte (`-1234` → `"-12,34 €"`).
pub fn eur(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}{},{:02} €", abs / 100, abs % 100)
}

/// Cent → ganze Euro ohne Nachkommastellen, wenn der Betrag glatt ist, sonst
/// wie [`eur`] (`80_000` → `"800 €"`, `80_050` → `"800,50 €"`).
pub fn eur_whole(cents: i64) -> String {
    if cents % 100 == 0 {
        let sign = if cents < 0 { "-" } else { "" };
        format!("{sign}{} €", cents.unsigned_abs() / 100)
    } else {
        eur(cents)
    }
}

/// Netto einer Position: `round(Menge × Einzelpreis)` (kaufmännisch). Ein
/// Ergebnis außerhalb von `i64` sättigt (Rust-Float-Casts sättigen, NaN → 0);
/// die Validierung lehnt solche Eingaben vorher ab.
pub fn line_net_cents(quantity: f64, unit_price_cents: i64) -> i64 {
    (quantity * unit_price_cents as f64).round() as i64
}

/// USt einer Position: `round(Netto × Satz / 100)` (kaufmännisch), sättigend.
pub fn line_tax_cents(net_cents: i64, tax_rate_percent: f64) -> i64 {
    (net_cents as f64 * tax_rate_percent / 100.0).round() as i64
}

/// Menge endlich, `0 < q ≤ MAX_QUANTITY`, höchstens 4 Nachkommastellen.
pub fn quantity_in_range(quantity: f64) -> bool {
    if !quantity.is_finite() || quantity <= 0.0 || quantity > MAX_QUANTITY {
        return false;
    }
    let scaled = quantity * 10f64.powi(QUANTITY_DECIMALS);
    // Absolute Toleranz auf der skalierten Menge (≤ 1e10): weit über der
    // f64-Auflösung, weit unter einer fünften Nachkommastelle.
    (scaled - scaled.round()).abs() < 1e-3
}

/// `|Einzelpreis| ≤ MAX_UNIT_PRICE_CENTS`.
pub fn unit_price_in_range(unit_price_cents: i64) -> bool {
    unit_price_cents
        .checked_abs()
        .is_some_and(|a| a <= MAX_UNIT_PRICE_CENTS)
}

/// Steuersatz endlich und `0 ≤ s ≤ MAX_TAX_RATE_PERCENT`.
pub fn tax_rate_in_range(rate: f64) -> bool {
    rate.is_finite() && (0.0..=MAX_TAX_RATE_PERCENT).contains(&rate)
}

/// Alle drei Positions-Grenzen zugleich.
pub fn item_in_range(quantity: f64, unit_price_cents: i64, tax_rate_percent: f64) -> bool {
    quantity_in_range(quantity)
        && unit_price_in_range(unit_price_cents)
        && tax_rate_in_range(tax_rate_percent)
}

/// Summiert Netto und USt aller Positionen mit Überlaufprüfung. `None`, wenn
/// eine Position außerhalb der Grenzen liegt oder eine Summe (auch Brutto)
/// `i64` verließe.
pub fn checked_document_totals<I>(items: I) -> Option<(i64, i64, i64)>
where
    I: IntoIterator<Item = (f64, i64, f64)>,
{
    let mut net_total: i64 = 0;
    let mut tax_total: i64 = 0;
    for (quantity, unit_price_cents, rate) in items {
        if !item_in_range(quantity, unit_price_cents, rate) {
            return None;
        }
        let net = line_net_cents(quantity, unit_price_cents);
        let tax = line_tax_cents(net, rate);
        net_total = net_total.checked_add(net)?;
        tax_total = tax_total.checked_add(tax)?;
    }
    let gross = net_total.checked_add(tax_total)?;
    Some((net_total, tax_total, gross))
}

/// `a × num / den` mit kaufmännischer Rundung (half away from zero) in
/// `i128`; Ergebnis sättigt auf `i64`. `den` muss positiv sein.
pub fn mul_div_round(a: i64, num: i64, den: i64) -> i64 {
    debug_assert!(den > 0);
    let p = i128::from(a) * i128::from(num);
    let d = i128::from(den.max(1));
    let q = if p >= 0 {
        (p + d / 2) / d
    } else {
        -((-p + d / 2) / d)
    };
    q.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eur_formats_integer_only() {
        assert_eq!(eur(0), "0,00 €");
        assert_eq!(eur(5), "0,05 €");
        assert_eq!(eur(123_456), "1234,56 €");
        assert_eq!(eur(-1_999), "-19,99 €");
        assert_eq!(eur(i64::MIN), "-92233720368547758,08 €");
    }

    #[test]
    fn eur_whole_drops_zero_cents_only() {
        assert_eq!(eur_whole(80_000), "800 €");
        assert_eq!(eur_whole(100_000), "1000 €");
        assert_eq!(eur_whole(80_050), "800,50 €");
    }

    #[test]
    fn quantity_bounds_and_decimals() {
        assert!(quantity_in_range(1.0));
        assert!(quantity_in_range(0.333));
        assert!(quantity_in_range(0.0001));
        assert!(quantity_in_range(MAX_QUANTITY));
        assert!(quantity_in_range(999_999.999_9));
        assert!(!quantity_in_range(0.0));
        assert!(!quantity_in_range(-1.0));
        assert!(!quantity_in_range(f64::NAN));
        assert!(!quantity_in_range(f64::INFINITY));
        assert!(!quantity_in_range(1e300));
        assert!(!quantity_in_range(MAX_QUANTITY + 1.0));
        assert!(!quantity_in_range(0.33333));
        assert!(!quantity_in_range(1.0 / 3.0));
    }

    #[test]
    fn price_and_rate_bounds() {
        assert!(unit_price_in_range(MAX_UNIT_PRICE_CENTS));
        assert!(unit_price_in_range(-MAX_UNIT_PRICE_CENTS));
        assert!(!unit_price_in_range(MAX_UNIT_PRICE_CENTS + 1));
        assert!(!unit_price_in_range(i64::MIN));
        assert!(tax_rate_in_range(0.0));
        assert!(tax_rate_in_range(19.0));
        assert!(!tax_rate_in_range(-0.5));
        assert!(!tax_rate_in_range(100.5));
        assert!(!tax_rate_in_range(f64::NAN));
    }

    #[test]
    fn line_arithmetic_saturates_instead_of_wrapping() {
        assert_eq!(line_net_cents(1e300, i64::MAX), i64::MAX);
        assert_eq!(line_net_cents(1e300, -5), i64::MIN);
        assert_eq!(line_net_cents(f64::NAN, 100), 0);
        assert_eq!(line_tax_cents(i64::MAX, 100.0), i64::MAX);
    }

    #[test]
    fn checked_totals_rejects_overflow_and_out_of_range() {
        assert_eq!(
            checked_document_totals([(2.0, 10_000, 19.0)]),
            Some((20_000, 3_800, 23_800))
        );
        assert_eq!(checked_document_totals([(1e300, 100, 0.0)]), None);
        // Je Position in den Grenzen, die Summe aber über i64::MAX.
        let near_max = (MAX_QUANTITY, MAX_UNIT_PRICE_CENTS, 0.0);
        let many = std::iter::repeat_n(near_max, 100);
        assert_eq!(checked_document_totals(many), None);
    }

    #[test]
    fn mul_div_round_half_away_from_zero() {
        assert_eq!(mul_div_round(100, 1, 3), 33);
        assert_eq!(mul_div_round(5, 1, 2), 3);
        assert_eq!(mul_div_round(-5, 1, 2), -3);
        assert_eq!(mul_div_round(1_000, 7, 12), 583);
        assert_eq!(mul_div_round(i64::MAX, 2, 1), i64::MAX);
    }

    #[test]
    fn amount_bounds_without_overflow() {
        assert!(amount_in_range(MAX_AMOUNT_CENTS));
        assert!(amount_in_range(-MAX_AMOUNT_CENTS));
        assert!(!amount_in_range(MAX_AMOUNT_CENTS + 1));
        assert!(!amount_in_range(i64::MIN));
        assert!(!amount_in_range(i64::MAX));
    }
}
