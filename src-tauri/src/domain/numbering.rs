//! Belegnummer-Formate (Functional Core, ohne I/O).
//!
//! Format pro Doc-Typ:
//! - Angebot:           `AN-{YYYY}-{NNNN}`
//! - Rechnung:          `RE-{YYYY}-{NNNN}`
//! - Storno-Rechnung:   `ST-{YYYY}-{NNNN}`
//! - Kosten:            `KO-{YYYY}-{NNNN}`
//! - Privatbewegung:    `PV-{YYYY}-{NNNN}`
//! - Anlage (Phase 2C): `AV-{YYYY}-{NNNN}`
//!
//! `{YYYY}` = Geschäftsjahr (in v0.1 == Kalenderjahr, siehe
//! [`crate::domain::fiscal_year`]). `{NNNN}` = mindestens 4-stellig,
//! 0-padded. Längere Werte erlaubt (5-stellig ab 10 000).
//!
//! Counter-Allokation (atomic `UPDATE … RETURNING`) lebt in
//! [`crate::db::numbering`] — diese Datei stellt nur den Format-Layer
//! bereit, damit Domain-Code testbar ohne DB bleibt.

use serde::{Deserialize, Serialize};

/// Maschinen-Slug für jeden Doc-Typ. Wird als Primärschlüssel-Komponente in
/// `doc_number_counters.doc_type` benutzt. **Niemals ändern**, sonst
/// kollidieren historische Counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocType {
    Quote,
    Invoice,
    StornoInvoice,
    Expense,
    PrivateMovement,
    Asset,
}

impl DocType {
    pub fn prefix(self) -> &'static str {
        match self {
            DocType::Quote => "AN",
            DocType::Invoice => "RE",
            DocType::StornoInvoice => "ST",
            DocType::Expense => "KO",
            DocType::PrivateMovement => "PV",
            DocType::Asset => "AV",
        }
    }

    /// Slug, der in `doc_number_counters.doc_type` gespeichert wird.
    pub fn db_slug(self) -> &'static str {
        match self {
            DocType::Quote => "quote",
            DocType::Invoice => "invoice",
            DocType::StornoInvoice => "storno_invoice",
            DocType::Expense => "expense",
            DocType::PrivateMovement => "private_movement",
            DocType::Asset => "asset",
        }
    }
}

/// Baut eine vollständige Belegnummer aus Typ, GJ und Counter-Wert.
///
/// `seq` muss `>= 1` sein — Counter starten in der DB bei 0 und werden vor
/// dem Format-Aufruf inkrementiert. Bei `seq == 0` panics — das wäre ein
/// Bug in der Counter-Schicht.
pub fn format(doc_type: DocType, fiscal_year: i32, seq: u32) -> String {
    debug_assert!(seq >= 1, "doc-number sequences start at 1");
    format!("{}-{}-{:04}", doc_type.prefix(), fiscal_year, seq)
}

/// Bequemer Wrapper für den häufigsten Fall.
pub fn format_invoice(fiscal_year: i32, seq: u32) -> String {
    format(DocType::Invoice, fiscal_year, seq)
}

pub fn format_storno(fiscal_year: i32, seq: u32) -> String {
    format(DocType::StornoInvoice, fiscal_year, seq)
}

pub fn format_quote(fiscal_year: i32, seq: u32) -> String {
    format(DocType::Quote, fiscal_year, seq)
}

pub fn format_expense(fiscal_year: i32, seq: u32) -> String {
    format(DocType::Expense, fiscal_year, seq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoice_format_is_padded_to_four_digits() {
        assert_eq!(format_invoice(2026, 1), "RE-2026-0001");
        assert_eq!(format_invoice(2026, 42), "RE-2026-0042");
        assert_eq!(format_invoice(2026, 9999), "RE-2026-9999");
    }

    #[test]
    fn five_digit_seq_overflows_padding_gracefully() {
        // 10_000+ ist weiterhin gültig, nur nicht mehr 0-padded auf 4.
        assert_eq!(format_invoice(2026, 10_000), "RE-2026-10000");
    }

    #[test]
    fn each_doc_type_has_unique_prefix_and_slug() {
        let types = [
            DocType::Quote,
            DocType::Invoice,
            DocType::StornoInvoice,
            DocType::Expense,
            DocType::PrivateMovement,
            DocType::Asset,
        ];
        let prefixes: std::collections::HashSet<_> = types.iter().map(|t| t.prefix()).collect();
        let slugs: std::collections::HashSet<_> = types.iter().map(|t| t.db_slug()).collect();
        assert_eq!(prefixes.len(), types.len(), "prefixes must be unique");
        assert_eq!(slugs.len(), types.len(), "db slugs must be unique");
    }

    #[test]
    fn storno_uses_st_prefix_per_gobd() {
        // Storno-Nummern werden pro GJ getrennt gezählt — gleicher Counter
        // wie für Rechnungen wäre verwirrend in Buchhaltung.
        assert_eq!(format_storno(2026, 1), "ST-2026-0001");
    }

    #[test]
    fn quote_and_expense_formats() {
        assert_eq!(format_quote(2026, 7), "AN-2026-0007");
        assert_eq!(format_expense(2025, 123), "KO-2025-0123");
    }

    #[test]
    #[should_panic(expected = "doc-number sequences start at 1")]
    fn format_panics_on_zero_seq() {
        let _ = format(DocType::Invoice, 2026, 0);
    }
}
