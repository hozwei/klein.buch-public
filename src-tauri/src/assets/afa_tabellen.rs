//! BMF-AfA-Tabellen-Loader (Imperative Shell) — Phase 2C, Block 12.
//!
//! Liest `inputs/specs/afa-tabellen.json` (menschen-maintained, `inputs/` ist
//! tabu für Maschinen-Writes), validiert das Schema und liefert die Kategorien
//! und die GWG-Grenze. Die Datei ist klein (~1 KB) und wird bei Bedarf geladen
//! — so wirkt eine Manuel-Pflege (BMF-Update) ohne App-Neustart. Die reine
//! Parse-Logik ([`parse`]) ist I/O-frei und testbar.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Eine AfA-Tabellen-Kategorie (BMF).
///
/// Serialisiert **camelCase** (Frontend-Konvention), deserialisiert aber auch die
/// **snake_case**-Keys der menschen-maintained `inputs/specs/afa-tabellen.json`
/// (via `alias`). So bleibt die Datei in der gewohnten snake_case-Form und das
/// Frontend bekommt trotzdem camelCase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AfaCategory {
    /// Maschinen-Code (z. B. "computer_hardware").
    pub code: String,
    /// Anzeige-Label (deutsch).
    pub label: String,
    /// Betriebsgewöhnliche Nutzungsdauer in Jahren.
    #[serde(alias = "useful_life_years")]
    pub useful_life_years: f64,
    /// Optionale Sonderregel-Kennung (z. B. "BMF_2021_02_26").
    #[serde(default, alias = "special_rule")]
    pub special_rule: Option<String>,
    /// Beispiel-Wirtschaftsgüter (informativ).
    #[serde(default, alias = "applies_to")]
    pub applies_to: Vec<String>,
}

/// Die geladene AfA-Tabelle. Unbekannte Felder (z. B. Sammelposten-Parameter,
/// die wir bewusst NICHT umsetzen — PRD NG6) werden ignoriert. Deserialisiert
/// snake_case (Datei) via `alias`, serialisiert camelCase (Frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AfaTabellen {
    pub version: String,
    #[serde(default, alias = "source_url")]
    pub source_url: Option<String>,
    pub categories: Vec<AfaCategory>,
    /// GWG-Grenze in Cent (netto), z. B. 80000 = 800,00 €.
    #[serde(alias = "gwg_threshold_cents")]
    pub gwg_threshold_cents: i64,
}

impl AfaTabellen {
    /// Findet eine Kategorie über ihren Code.
    pub fn category(&self, code: &str) -> Option<&AfaCategory> {
        self.categories.iter().find(|c| c.code == code)
    }
}

/// Parst + validiert den JSON-Inhalt (pure, ohne I/O).
pub fn parse(json: &str) -> Result<AfaTabellen> {
    let table: AfaTabellen = serde_json::from_str(json)
        .map_err(|e| Error::Config(format!("afa-tabellen.json ist ungültig: {e}")))?;

    if table.categories.is_empty() {
        return Err(Error::Config(
            "afa-tabellen.json enthält keine Kategorien.".into(),
        ));
    }
    if table.gwg_threshold_cents <= 0 {
        return Err(Error::Config(
            "afa-tabellen.json: gwg_threshold_cents muss > 0 sein.".into(),
        ));
    }
    if let Some(bad) = table.categories.iter().find(|c| c.useful_life_years <= 0.0) {
        return Err(Error::Config(format!(
            "afa-tabellen.json: Kategorie '{}' hat eine ungültige Nutzungsdauer ({}).",
            bad.code, bad.useful_life_years
        )));
    }
    Ok(table)
}

/// Lädt die Datei aus `inputs/specs/afa-tabellen.json`.
pub fn load(inputs_dir: &Path) -> Result<AfaTabellen> {
    let path = inputs_dir.join("specs").join("afa-tabellen.json");
    let json = std::fs::read_to_string(&path).map_err(|e| {
        Error::Config(format!(
            "afa-tabellen.json nicht lesbar ({}): {e}",
            path.display()
        ))
    })?;
    parse(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "version": "BMF-2024-12",
        "source_url": "https://example.test/afa",
        "categories": [
            { "code": "computer_hardware", "label": "Computer-Hardware", "useful_life_years": 1, "special_rule": "BMF_2021_02_26", "applies_to": ["PC","Laptop"] },
            { "code": "office_furniture", "label": "Büromöbel", "useful_life_years": 13 }
        ],
        "gwg_threshold_cents": 80000,
        "sammelposten_lower_cents": 25000
    }"#;

    #[test]
    fn parses_valid_table_and_ignores_unknown_fields() {
        let t = parse(SAMPLE).unwrap();
        assert_eq!(t.version, "BMF-2024-12");
        assert_eq!(t.gwg_threshold_cents, 80_000);
        assert_eq!(t.categories.len(), 2);
        let cat = t.category("computer_hardware").unwrap();
        assert_eq!(cat.useful_life_years, 1.0);
        assert_eq!(cat.special_rule.as_deref(), Some("BMF_2021_02_26"));
        assert_eq!(cat.applies_to.len(), 2);
        assert!(t.category("does_not_exist").is_none());
    }

    #[test]
    fn rejects_empty_categories() {
        let json = r#"{ "version": "x", "categories": [], "gwg_threshold_cents": 80000 }"#;
        assert!(parse(json).is_err());
    }

    #[test]
    fn rejects_non_positive_threshold() {
        let json = r#"{ "version": "x", "categories": [{"code":"a","label":"A","useful_life_years":5}], "gwg_threshold_cents": 0 }"#;
        assert!(parse(json).is_err());
    }

    #[test]
    fn rejects_non_positive_useful_life() {
        let json = r#"{ "version": "x", "categories": [{"code":"a","label":"A","useful_life_years":0}], "gwg_threshold_cents": 80000 }"#;
        assert!(parse(json).is_err());
    }
}
