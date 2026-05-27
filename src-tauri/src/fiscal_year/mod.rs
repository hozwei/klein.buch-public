//! Geschäftsjahr-Modul (Phase 2D, Block 15).
//! - `guard`: Festschreibungs-Guard — blockt neue Buchungen in abgeschlossenen GJ.
//! - `lock`: GJ-Abschluss (AfA festschreiben, Snapshot, Protokoll, Backup).
//! - `transition`: GJ-Übergang / Carry-over offener Forderungen.

pub mod guard;
pub mod lock;
pub mod transition;

pub use guard::{ensure_year_open, is_closed};
pub use lock::{close_year, FiscalYearLock};

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
