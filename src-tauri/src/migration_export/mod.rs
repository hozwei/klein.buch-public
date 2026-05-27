//! Migrations-Export (Block 4) — kompletter Daten-Dump als JSON+Files-ZIP,
//! mit Standalone-Reader-Beispiel-Script. Dient als Vendor-Lock-in-Schutz.

pub mod export;
pub mod json_dump;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
