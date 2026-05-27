//! Re-Export der AfA-Berechnung (Phase 2C, Block 12).
//!
//! Der Functional Core der AfA-Rechnung lebt — der Architektur (Functional Core
//! in `domain/`) folgend — in [`crate::domain::depreciation`] (PRD §6.7). Dieses
//! Modul hält nur den historischen Pfad `depreciation::compute` stabil, damit die
//! Shell ([`crate::depreciation::accrue_yearly`]) eine sprechende Heimat hat.

pub use crate::domain::depreciation::{
    annual_linear_cents, compute_yearly, months_used_in_acquisition_year, DepreciationAsset,
    DepreciationCalc,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
