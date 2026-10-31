//! Storno-Logik (Functional Core).
//!
//! Storno = **neuer** Storno-Beleg, **niemals** Löschung des Originals.
//! Buchhalterisch: Original und Storno bleiben beide unverändert in der
//! DB. Beide werden gelockt; der Original-Status wechselt zu `canceled`
//! und referenziert den Storno (`canceled_by_storno_id`), der Storno
//! referenziert das Original (`is_storno_for`).
//!
//! Diese Datei baut das `InvoiceInput` für den Storno-Beleg aus der
//! Original-Rechnung — ohne DB-Zugriff.

use chrono::NaiveDate;

use crate::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};

/// Read-only View der Original-Rechnung, die der Caller (Block 3b: commands)
/// aus DB-Rows baut. Hält den Functional Core frei von DB-Strukturen.
#[derive(Debug, Clone)]
pub struct OriginalInvoiceView<'a> {
    pub invoice_number: &'a str,
    pub currency_code: &'a str,
    pub pdf_template: &'a str,
    /// Leistungszeitraum des Originals (Beginn, Ende), falls gesetzt — der
    /// Storno übernimmt ihn (ADR 0042 Nr. 10).
    pub service_period: Option<(NaiveDate, NaiveDate)>,
    /// Leistungsdatum des Originals, falls gesetzt — der Storno übernimmt es
    /// (DOC-12).
    pub delivery_date: Option<NaiveDate>,
    /// Rechnungsdatum des Originals: Leistungszeitpunkt des Originals, wenn es
    /// weder Leistungsdatum noch Zeitraum trägt (Rückfall wie beim Original).
    pub invoice_date: NaiveDate,
    pub items: Vec<OriginalItemView<'a>>,
}

/// Bezug eines Storno-Belegs auf die stornierte Rechnung: Nummer (BT-25) und
/// Rechnungsdatum (BT-26) des Originals, auf dem PDF als „Storno zur Rechnung
/// <Nr> vom <Datum>“ (§31 Abs. 5 UStDV, GoBD Rz. 64; Lauf E). Die interne ID
/// (`is_storno_for`) taugt dafür nicht — der Empfänger kennt nur die Nummer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecedingInvoice {
    pub number: String,
    pub issue_date: NaiveDate,
}

/// Geschäftsjahr und Nummernkreis eines Storno-Belegs folgen dem
/// Storno-Datum, nicht dem Original (GoBD Rz. 51: Zuordnung zum
/// Geschäftsjahr, in dem der Vorgang anfällt; Lauf E, ADR 0039 Nr. 122).
pub fn storno_fiscal_year(storno_date: &NaiveDate) -> i32 {
    crate::domain::fiscal_year::fiscal_year_for(storno_date)
}

#[derive(Debug, Clone)]
pub struct OriginalItemView<'a> {
    pub position: u32,
    pub description: &'a str,
    pub quantity: f64,
    pub unit_code: &'a str,
    pub unit_price_cents: i64,
    pub tax_rate_percent: f64,
    pub tax_category_code: &'a str,
}

/// Baut den Input für eine Storno-Rechnung. Items werden mit negativem
/// `unit_price_cents` übernommen — Mengen, Einheit, Steuersatz, Kategorie
/// bleiben identisch (Cash-Basis-Aufrechnung).
///
/// Description bekommt einen Storno-Präfix mit der Original-Belegnummer,
/// damit die GoBD-Spur direkt lesbar ist.
pub fn build_storno_input<'a>(
    original: &OriginalInvoiceView<'a>,
    original_invoice_id: String,
    storno_date: NaiveDate,
    reason: Option<String>,
) -> InvoiceInput {
    let items: Vec<InvoiceItemInput> = original
        .items
        .iter()
        .map(|it| InvoiceItemInput {
            position: it.position,
            description: format!(
                "Storno zu {} Pos. {}: {}",
                original.invoice_number, it.position, it.description
            ),
            quantity: it.quantity,
            unit_code: it.unit_code.to_string(),
            unit_price_cents: -it.unit_price_cents,
            tax_rate_percent: it.tax_rate_percent,
            tax_category_code: it.tax_category_code.to_string(),
            // Storno = schlichte negierte Zeile; kein Paket-Block, keine Provenienz.
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        })
        .collect();

    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: storno_date,
        // DOC-12: Der Storno korrigiert die ursprüngliche Leistung (§ 31
        // Abs. 5 UStDV, § 17 UStG); er nennt darum einheitlich deren
        // Leistungszeitpunkt: den Leistungszeitraum (ADR 0042 Nr. 10: BG-14
        // wie im Original, kein BT-72), sonst das Leistungsdatum, sonst das
        // Rechnungsdatum des Originals (dessen Rückfall). Das Storno-Datum ist
        // das Datum des Belegs, kein Tag einer Leistung. Die EÜR setzt die
        // Erstattung weiter am Storno-Datum an (Cash-Basis), der Status folgt
        // dem Original (`invoice_tax_status`).
        delivery_date: original
            .service_period
            .is_none()
            .then(|| original.delivery_date.unwrap_or(original.invoice_date)),
        service_period_start: original.service_period.map(|(s, _)| s),
        service_period_end: original.service_period.map(|(_, e)| e),
        // BT-10 liest der Storno aus der archivierten E-Rechnung des
        // Originals (ADR 0041 Nr. 250); das Festschreiben setzt ihn.
        buyer_reference: None,
        due_date: None, // Storno hat keine Fälligkeit (Gutschrift)
        currency_code: original.currency_code.to_string(),
        items,
        notes: reason.clone(),
        // Storno trägt keinen Bezahlt-Hinweis (Gutschrift).
        payment_note: None,
        pdf_template: original.pdf_template.to_string(),
        is_storno_for: Some(original_invoice_id),
        cancel_reason: reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::invoice::compute_totals;

    fn original() -> OriginalInvoiceView<'static> {
        OriginalInvoiceView {
            invoice_number: "RE-2026-0001",
            currency_code: "EUR",
            pdf_template: "default",
            service_period: None,
            delivery_date: None,
            invoice_date: NaiveDate::from_ymd_opt(2026, 3, 12).unwrap(),
            items: vec![
                OriginalItemView {
                    position: 1,
                    description: "Beratung 2h",
                    quantity: 2.0,
                    unit_code: "HUR",
                    unit_price_cents: 5_000,
                    tax_rate_percent: 0.0,
                    tax_category_code: "E",
                },
                OriginalItemView {
                    position: 2,
                    description: "Anfahrt",
                    quantity: 1.0,
                    unit_code: "C62",
                    unit_price_cents: 1_500,
                    tax_rate_percent: 0.0,
                    tax_category_code: "E",
                },
            ],
        }
    }

    #[test]
    fn storno_negates_unit_prices() {
        let storno = build_storno_input(
            &original(),
            "uuid-of-orig".into(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            Some("Falscher Empfänger".into()),
        );
        assert_eq!(storno.items[0].unit_price_cents, -5_000);
        assert_eq!(storno.items[1].unit_price_cents, -1_500);
    }

    #[test]
    fn storno_preserves_positions_and_meta() {
        let storno = build_storno_input(
            &original(),
            "uuid".into(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            None,
        );
        assert_eq!(storno.items[0].position, 1);
        assert_eq!(storno.items[1].position, 2);
        assert_eq!(storno.items[0].tax_category_code, "E");
        assert_eq!(storno.currency_code, "EUR");
        assert_eq!(storno.pdf_template, "default");
        assert_eq!(
            storno.direction,
            crate::domain::invoice::InvoiceDirection::Issued
        );
    }

    #[test]
    fn storno_total_is_negative_of_original() {
        let original = original();
        let original_totals = {
            let items: Vec<InvoiceItemInput> = original
                .items
                .iter()
                .map(|it| InvoiceItemInput {
                    position: it.position,
                    description: it.description.into(),
                    quantity: it.quantity,
                    unit_code: it.unit_code.into(),
                    unit_price_cents: it.unit_price_cents,
                    tax_rate_percent: it.tax_rate_percent,
                    tax_category_code: it.tax_category_code.into(),
                    description_title: None,
                    description_markup: None,
                    source_package_id: None,
                    source_package_revision: None,
                })
                .collect();
            compute_totals(&items)
        };
        let storno = build_storno_input(
            &original,
            "uuid".into(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            None,
        );
        let storno_totals = compute_totals(&storno.items);
        assert_eq!(
            storno_totals.net_amount_cents,
            -original_totals.net_amount_cents
        );
        assert_eq!(storno_totals.tax_amount_cents, 0);
    }

    // R1-S2-TESTS: Storno einer regelbesteuerten Rechnung (19 % und 7 %,
    // Bruchteil-Mengen mit Halbcent-Rundung) ergibt exakt die negierten Netto-,
    // Steuer- und Bruttowerte.
    #[test]
    fn storno_with_vat_mirrors_original_exactly() {
        let items = vec![
            OriginalItemView {
                position: 1,
                description: "Beratung",
                quantity: 2.5,
                unit_code: "HUR",
                unit_price_cents: 1_999,
                tax_rate_percent: 19.0,
                tax_category_code: "S",
            },
            OriginalItemView {
                position: 2,
                description: "Material",
                quantity: 0.333,
                unit_code: "C62",
                unit_price_cents: 333,
                tax_rate_percent: 19.0,
                tax_category_code: "S",
            },
            OriginalItemView {
                position: 3,
                description: "Buch",
                quantity: 1.0,
                unit_code: "C62",
                unit_price_cents: 50,
                tax_rate_percent: 7.0,
                tax_category_code: "S",
            },
        ];
        let original = OriginalInvoiceView {
            invoice_number: "RE-2026-0002",
            currency_code: "EUR",
            pdf_template: "default",
            service_period: None,
            delivery_date: None,
            invoice_date: NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
            items,
        };
        let original_items: Vec<InvoiceItemInput> = original
            .items
            .iter()
            .map(|it| InvoiceItemInput {
                position: it.position,
                description: it.description.into(),
                quantity: it.quantity,
                unit_code: it.unit_code.into(),
                unit_price_cents: it.unit_price_cents,
                tax_rate_percent: it.tax_rate_percent,
                tax_category_code: it.tax_category_code.into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            })
            .collect();
        let o = compute_totals(&original_items);
        let storno = build_storno_input(
            &original,
            "uuid".into(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            None,
        );
        let s = compute_totals(&storno.items);
        assert!(o.tax_amount_cents > 0);
        assert_eq!(s.net_amount_cents, -o.net_amount_cents);
        assert_eq!(s.tax_amount_cents, -o.tax_amount_cents);
        assert_eq!(s.gross_amount_cents, -o.gross_amount_cents);
        assert_eq!(storno.items[0].tax_category_code, "S");
        assert_eq!(storno.items[2].tax_rate_percent, 7.0);
    }

    #[test]
    fn storno_description_carries_original_reference() {
        let storno = build_storno_input(
            &original(),
            "uuid".into(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            None,
        );
        assert!(storno.items[0]
            .description
            .starts_with("Storno zu RE-2026-0001 Pos. 1:"));
    }

    #[test]
    fn storno_takes_the_service_period_of_the_original() {
        // ADR 0042 Nr. 10: Zeitraum vom Original, kein Leistungsdatum (BT-72).
        let d = |m, t| NaiveDate::from_ymd_opt(2026, m, t).unwrap();
        let mut orig = original();
        orig.service_period = Some((d(4, 1), d(6, 30)));
        let storno = build_storno_input(&orig, "uuid".into(), d(7, 2), None);
        assert_eq!(storno.service_period_start, Some(d(4, 1)));
        assert_eq!(storno.service_period_end, Some(d(6, 30)));
        assert_eq!(storno.delivery_date, None);
        assert_eq!(storno.invoice_date, d(7, 2));
    }

    /// DOC-12: Der Storno nennt einheitlich den Leistungszeitpunkt des
    /// Originals (§ 31 Abs. 5 UStDV): sein Leistungsdatum, ohne Leistungsdatum
    /// und Zeitraum dessen Rechnungsdatum — nie das Storno-Datum.
    #[test]
    fn storno_takes_the_delivery_date_of_the_original() {
        let d = |m, t| NaiveDate::from_ymd_opt(2026, m, t).unwrap();
        let mut orig = original();
        orig.delivery_date = Some(d(3, 10));
        let storno = build_storno_input(&orig, "uuid".into(), d(6, 15), None);
        assert_eq!(storno.delivery_date, Some(d(3, 10)));
        assert_eq!(storno.invoice_date, d(6, 15));
        assert_eq!(storno.service_period_start, None);
        // Ohne Leistungsdatum: Rechnungsdatum des Originals.
        let plain = build_storno_input(&original(), "uuid".into(), d(6, 15), None);
        assert_eq!(plain.delivery_date, Some(d(3, 12)));
        assert_eq!(plain.service_period_start, None);
    }

    #[test]
    fn storno_carries_is_storno_for_id() {
        let storno = build_storno_input(
            &original(),
            "uuid-orig".into(),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            Some("Reason".into()),
        );
        assert_eq!(storno.is_storno_for.as_deref(), Some("uuid-orig"));
        assert_eq!(storno.cancel_reason.as_deref(), Some("Reason"));
    }

    #[test]
    fn storno_fiscal_year_follows_the_storno_date() {
        // Original vom Dezember, Storno im Januar: Geschäftsjahr des Stornos.
        let jan = NaiveDate::from_ymd_opt(2027, 1, 15).unwrap();
        assert_eq!(storno_fiscal_year(&jan), 2027);
        let dec = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        assert_eq!(storno_fiscal_year(&dec), 2026);
    }
}
