//! EÜR-Modul (Phase 2C, Block 13+14).
//! Cash-Basis: Einnahmen am paid_at, Ausgaben am paid_date.
//! Stornos und Privatbewegungen werden ausgeklammert.
//!
//! - `aggregate`: Functional Core, pro GJ.
//! - `elster_csv`, `datev_csv`, `stb_package`: Exporte.

pub mod aggregate;
pub mod datev_csv;
pub mod detail;
pub mod elster_csv;
pub mod stb_package;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
