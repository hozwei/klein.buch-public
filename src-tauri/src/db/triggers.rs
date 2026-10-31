//! Stub-Dokumentation der GoBD-Triggers.
//!
//! Die Triggers werden in den Migrations-SQL-Files definiert (forward-only);
//! die vollständige Liste mit Wirkung steht in
//! `docs/architecture/data-model.md` §3 und `docs/architecture/gobd.md` §2.
//!
//! - `trg_audit_no_update` / `trg_audit_no_delete`: audit_log ist append-only.
//! - `trg_invoices_immutable` / `trg_quotes_immutable` / … (0001, Whitelist
//!   seit 0028): festgeschriebene Belege erlauben keine Updates auf Kern- und
//!   Snapshot-Felder; erlaubt sind die Status-Übergänge. `*_no_delete` (0028):
//!   kein DELETE auf Belegen.
//! - `trg_archive_no_update` (0001) / `trg_archive_no_delete` (0032):
//!   archive_entries sind write-once.
//! - Migration 0032 (Lauf F, F2a): Positionen festgeschriebener bzw.
//!   verworfener Belege (`trg_invoice_items_frozen_*`,
//!   `trg_quote_items_frozen_*`), verworfene Entwürfe
//!   (`trg_invoices_voided_final`, `trg_invoices_void_draft_only`,
//!   `trg_invoices_void_not_on_insert`), Storno endgültig
//!   (`trg_invoices_canceled_final`, `trg_quotes_canceled_final`),
//!   Anlagen-Abgang endgültig (`trg_assets_disposal_final`, seit F2c samt
//!   Stammdaten und Restbuchwert; `trg_depreciation_entries_no_delete_disposed`),
//!   Umwandlung eines Angebots endgültig außer beim Verwerfen des Entwurfs
//!   (`trg_quotes_converted_final`, F2c); dazu die Indizes
//!   `ux_invoices_one_locked_storno` und `uq_payment_accounts_default`.
//!   `trg_invoices_voided_final` lässt als einzige Änderung die Anonymisierung
//!   des Empfänger-Snapshots zu (F2c, ADR 0039 Nr. 124 (3)).
//! - Migration 0034 (Lauf 2026.11.1, V2, ADR 0042 Nr. 7–9): Zahlungsbuch
//!   `invoice_payments` append-only (`trg_invoice_payments_no_update`/
//!   `_no_delete`, `trg_invoice_payments_insert_checks`,
//!   `ux_invoice_payments_one_reversal`); der Zahlungsstand der Rechnung ist an
//!   das Buch gebunden (`trg_invoices_payment_cache_bound`,
//!   `trg_invoices_payment_cache_insert`), die JSON-Historie eingefroren
//!   (`trg_invoices_payment_history_frozen`).
//!
//! Tests, die diese Triggers verifizieren: `tests/migration_0028_test.rs`,
//! `tests/migration_0032_test.rs`, `tests/payment_ledger_test.rs` sowie die Repo-Tests je Beleg-Art
//! (Übersicht in `gobd.md` §9).

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
