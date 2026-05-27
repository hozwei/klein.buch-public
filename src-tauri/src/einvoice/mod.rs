//! E-Rechnung-Schicht (XRechnung + ZUGFeRD).
//!
//! - `parser` (Block 11): Liest XRechnung-XML und extrahiert XML aus ZUGFeRD-PDFs.
//! - `generator` (Block 3): Erzeugt EN-16931-konformes XRechnung-XML aus Invoice-Daten.
//! - `validator` (Block 3): Bridge zum KoSIT-Sidecar.
//! - `mustang_bridge` (Block 3): Bridge zum Mustang-Sidecar (ZUGFeRD-PDF/A-3-Erzeugung).
//! - `types`: gemeinsame Datentypen (Profile, BT-Felder, Cardinality).

pub mod generator;
pub mod mustang_bridge;
pub mod parser;
pub mod types;
pub mod validator;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
