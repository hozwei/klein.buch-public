//! Stub-Dokumentation der GoBD-Triggers.
//!
//! Die Triggers werden in den Migrations-SQL-Files definiert (forward-only):
//!
//! - `trg_audit_no_update` / `trg_audit_no_delete`: audit_log ist append-only.
//! - `trg_invoices_immutable`: locked invoices erlauben keine Updates auf
//!   Kernfelder (invoice_number, date, beträge, contact_id, fiscal_year,
//!   is_kleinunternehmer, direction). Erlaubt sind State-Transitions
//!   (status, paid_*, sent_*, canceled_*, validation_*, notes, archive_ids).
//! - `trg_archive_no_update`: archive_entries.hash/path/size sind immutable.
//!
//! Tests, die diese Triggers verifizieren, leben in `tests/triggers_test.rs`.

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
