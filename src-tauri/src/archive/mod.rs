//! Archive-Schicht: write-once GoBD-konformes Ablegen.
//!
//! - [`store`]: schreibt Datei + SHA-256 + Eintrag in `archive_entries`,
//!   markiert die Datei nach Schreiben als read-only und emittiert ein
//!   `archive.store`-Event ins Audit-Log.
//! - [`audit`]: typisierter Wrapper um `audit_log::append` für alle
//!   Archive-Events (Store / Read / IntegrityPass / IntegrityFail).
//! - [`integrity_check`]: Re-Hash beim Read und vollständiger Scan über
//!   alle Einträge mit Aggregat in `archive_integrity_checks`. In Phase 2D
//!   vom Scheduler getriggert; in Phase 1 manuell aus Settings.

pub mod audit;
pub mod integrity_check;
pub mod store;

pub use audit::ArchiveAction;
pub use integrity_check::{run_full_scan, verify_one, IntegrityCheckSummary};
pub use store::{read_and_verify, store_bytes, ArchiveKind, StoredArchive};

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
