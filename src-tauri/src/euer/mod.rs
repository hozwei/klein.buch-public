//! EÜR-Modul (Phase 2C, Block 13+14).
//! Cash-Basis: Einnahmen am paid_at, Ausgaben am paid_date.
//! Stornos und Privatbewegungen werden ausgeklammert.
//!
//! - `aggregate`: Functional Core, pro GJ.
//! - `detail`: Einzelaufstellung und Anlageverzeichnis.
//! - `elster_csv`, `datev_csv`: Exporte. Das Steuerberater-Paket setzt
//!   `commands::euer::euer_export_stb_zip` aus diesen Bausteinen zusammen.

pub mod aggregate;
pub mod datev_csv;
pub mod detail;
pub mod elster_csv;
