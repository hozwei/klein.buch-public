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
    pub items: Vec<OriginalItemView<'a>>,
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
        // Leistungsdatum entspricht dem Storno-Datum — wir machen die
        // Stornierung "heute", die Rück-Leistung ist nicht das Original-
        // Leistungsdatum (Cash-Basis).
        delivery_date: Some(storno_date),
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
}
