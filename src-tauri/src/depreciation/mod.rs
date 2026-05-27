//! AfA-Buchungs-Modul (Phase 2C).
//! - `compute`: Functional Core, pro Anlage pro GJ.
//! - `accrue_yearly`: Shell — schreibt AfA-Buchungen für den GJ ins DB.

pub mod accrue_yearly;
pub mod compute;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
