//! Anlagen-Domain (Functional Core) — Phase 2C, Block 12.
//!
//! ## Inhalte
//!
//! - [`DepreciationMethod`] — AfA-Methode (1:1 zum CHECK in `0010_assets.sql`).
//! - [`DisposalType`] — Veräußerungsart (1:1 zum CHECK).
//! - [`AssetInput`] — User-Eingabe für eine Anlage (create/update).
//! - [`validate_asset`] — Struktur-/Wert-Checks (pure).
//! - [`business_book_value_start_cents`] — Start-Restbuchwert (anteilig Privatanteil).
//! - [`suggest_method`] — schlägt AfA-Methode + Nutzungsdauer vor (§6.17 PRD).
//!
//! Alle Funktionen pure, keine I/O. Belegnummer-Allokation und Persistenz liegen
//! in [`crate::db::numbering`] und [`crate::db::repo::assets`]; die eigentliche
//! AfA-Rechnung in [`crate::domain::depreciation`].
//!
//! ## Privatanteil
//!
//! `business_share_percent` (0–100) ist der betriebliche Anteil. Der
//! Start-Restbuchwert wird anteilig angesetzt (`business_book_value_start_cents`);
//! die AfA läuft auf diesem betrieblichen Wert. Der private Anteil wird nie
//! aktiviert (keine Betriebsausgabe).
//!
//! ## Anschaffungskosten = NETTO
//!
//! `acquisition_cost_cents` ist der **Netto**-Wert. Für §19-Kleinunternehmer ist
//! die gezahlte Vorsteuer nicht abziehbar und gehört steuerlich eigentlich in die
//! AfA-Bemessungsgrundlage; in v0.1 setzen wir bewusst den Netto-Wert an und
//! dokumentieren das als Vereinfachung (Steuerberater-Gegencheck, PRD-Caveat).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// AfA-Methode — synchron zum CHECK-Constraint in `0010_assets.sql`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepreciationMethod {
    /// GWG-Sofortabschreibung (≤ 800 € netto) — Vollabschreibung im Anschaffungsjahr.
    GwgSofort,
    /// Lineare AfA über die Nutzungsdauer, monatsgenau im Anschaffungsjahr.
    Linear,
    /// Digitale Wirtschaftsgüter (BMF 2021-02-26): Nutzungsdauer 1 Jahr →
    /// faktisch Sofortabschreibung im Anschaffungsjahr.
    ComputerSpecial2021,
}

impl DepreciationMethod {
    pub fn as_db(self) -> &'static str {
        match self {
            DepreciationMethod::GwgSofort => "gwg_sofort",
            DepreciationMethod::Linear => "linear",
            DepreciationMethod::ComputerSpecial2021 => "computer_special_2021",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "gwg_sofort" => Some(DepreciationMethod::GwgSofort),
            "linear" => Some(DepreciationMethod::Linear),
            "computer_special_2021" => Some(DepreciationMethod::ComputerSpecial2021),
            _ => None,
        }
    }

    /// Sofortabschreibung im Anschaffungsjahr (GWG oder Computer-Sonderregel)?
    pub fn is_immediate_writeoff(self) -> bool {
        matches!(
            self,
            DepreciationMethod::GwgSofort | DepreciationMethod::ComputerSpecial2021
        )
    }
}

/// Veräußerungsart — synchron zum CHECK-Constraint in `0010_assets.sql`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisposalType {
    /// Verkauf — `disposal_proceeds_cents` ist der Erlös.
    Sale,
    /// Verschrottung/Entsorgung — Erlös 0.
    Scrap,
    /// Verschenkt/Privatentnahme — Erlös 0.
    GivenAway,
}

impl DisposalType {
    pub fn as_db(self) -> &'static str {
        match self {
            DisposalType::Sale => "sale",
            DisposalType::Scrap => "scrap",
            DisposalType::GivenAway => "given_away",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "sale" => Some(DisposalType::Sale),
            "scrap" => Some(DisposalType::Scrap),
            "given_away" => Some(DisposalType::GivenAway),
            _ => None,
        }
    }
}

// ---- Input-Typ -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInput {
    /// Bezeichnung ("MacBook Pro 14, 2024").
    pub label: String,
    /// Anschaffungsdatum. Bestimmt das Geschäftsjahr der Belegnummer + AfA-Start.
    pub acquisition_date: NaiveDate,
    /// Netto-Anschaffungskosten in Cent (voll, vor Privatanteil-Kürzung).
    pub acquisition_cost_cents: i64,
    /// Optionaler Quell-Kosten-Beleg (wenn aus einer Kosten-Position aktiviert).
    pub expense_id: Option<String>,
    /// Optionaler Lieferanten-Kontakt.
    pub vendor_contact_id: Option<String>,
    /// 'gwg_sofort' | 'linear' | 'computer_special_2021'.
    pub depreciation_method: String,
    /// Nutzungsdauer in Jahren — bei 'linear' Pflicht; sonst ignoriert.
    pub useful_life_years: Option<f64>,
    /// BMF-Kategorie-Code aus der AfA-Tabelle (optional, nur informativ).
    pub afa_category: Option<String>,
    /// Betrieblicher Anteil 0–100 (%).
    pub business_share_percent: f64,
    pub notes: Option<String>,
}

// ---- Helpers ---------------------------------------------------------------

/// Start-Restbuchwert: betrieblicher Anteil der Netto-Anschaffungskosten,
/// kaufmännisch auf Cent gerundet. Der private Anteil wird nicht aktiviert.
pub fn business_book_value_start_cents(
    acquisition_cost_cents: i64,
    business_share_percent: f64,
) -> i64 {
    let share = business_share_percent.clamp(0.0, 100.0);
    ((acquisition_cost_cents as f64) * share / 100.0).round() as i64
}

// ---- Validation ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum AssetValidationError {
    LabelMissing,
    AcquisitionCostNotPositive {
        cost: i64,
    },
    MethodInvalid {
        value: String,
    },
    BusinessShareOutOfRange {
        value: f64,
    },
    /// Bei `linear` ist die Nutzungsdauer Pflicht.
    UsefulLifeMissingForLinear,
    /// Nutzungsdauer (falls gesetzt) muss > 0 sein.
    UsefulLifeNotPositive {
        value: f64,
    },
    AcquisitionDateInFuture {
        date: NaiveDate,
        today: NaiveDate,
    },
}

/// Validiert eine [`AssetInput`]. Aggregiert alle Fehler (kein Fail-Fast).
/// `today` (Europe/Berlin) wird injiziert, damit der Functional Core pur bleibt.
pub fn validate_asset(
    input: &AssetInput,
    today: NaiveDate,
) -> Result<(), Vec<AssetValidationError>> {
    use AssetValidationError as E;
    let mut errs = Vec::new();

    if input.label.trim().is_empty() {
        errs.push(E::LabelMissing);
    }
    if input.acquisition_cost_cents <= 0 {
        errs.push(E::AcquisitionCostNotPositive {
            cost: input.acquisition_cost_cents,
        });
    }
    if input.acquisition_date > today {
        errs.push(E::AcquisitionDateInFuture {
            date: input.acquisition_date,
            today,
        });
    }
    if !(0.0..=100.0).contains(&input.business_share_percent) {
        errs.push(E::BusinessShareOutOfRange {
            value: input.business_share_percent,
        });
    }

    match DepreciationMethod::from_db(&input.depreciation_method) {
        None => errs.push(E::MethodInvalid {
            value: input.depreciation_method.clone(),
        }),
        Some(DepreciationMethod::Linear) => match input.useful_life_years {
            None => errs.push(E::UsefulLifeMissingForLinear),
            Some(v) if v <= 0.0 => errs.push(E::UsefulLifeNotPositive { value: v }),
            Some(_) => {}
        },
        Some(_) => {
            // GWG/Computer-Sonderregel: Nutzungsdauer wird ignoriert/gesetzt;
            // falls dennoch ein unsinniger Wert mitkommt, nicht hart blocken.
            if let Some(v) = input.useful_life_years {
                if v <= 0.0 {
                    errs.push(E::UsefulLifeNotPositive { value: v });
                }
            }
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI).
pub fn message(e: &AssetValidationError) -> String {
    use AssetValidationError as E;
    fn eur(c: i64) -> String {
        format!("{:.2} €", c as f64 / 100.0).replace('.', ",")
    }
    match e {
        E::LabelMissing => "Bezeichnung ist erforderlich.".into(),
        E::AcquisitionCostNotPositive { cost } => {
            format!(
                "Anschaffungskosten müssen größer als 0 sein ({}).",
                eur(*cost)
            )
        }
        E::MethodInvalid { value } => format!("Ungültige AfA-Methode: {value}."),
        E::BusinessShareOutOfRange { value } => {
            format!("Betrieblicher Anteil muss zwischen 0 und 100 % liegen (war {value}).")
        }
        E::UsefulLifeMissingForLinear => {
            "Bei linearer AfA ist die Nutzungsdauer (Jahre) erforderlich.".into()
        }
        E::UsefulLifeNotPositive { value } => {
            format!("Nutzungsdauer muss größer als 0 sein (war {value}).")
        }
        E::AcquisitionDateInFuture { date, today } => {
            format!("Anschaffungsdatum ({date}) liegt in der Zukunft (heute {today}).")
        }
    }
}

/// Maschinenlesbarer Variantenname für DTOs/Logs.
pub fn variant_name(e: &AssetValidationError) -> &'static str {
    use AssetValidationError as E;
    match e {
        E::LabelMissing => "LabelMissing",
        E::AcquisitionCostNotPositive { .. } => "AcquisitionCostNotPositive",
        E::MethodInvalid { .. } => "MethodInvalid",
        E::BusinessShareOutOfRange { .. } => "BusinessShareOutOfRange",
        E::UsefulLifeMissingForLinear => "UsefulLifeMissingForLinear",
        E::UsefulLifeNotPositive { .. } => "UsefulLifeNotPositive",
        E::AcquisitionDateInFuture { .. } => "AcquisitionDateInFuture",
    }
}

// ---- AfA-Methoden-Vorschlag (PRD §6.17) ------------------------------------

/// Vorschlag für die AfA-Methode beim Anlegen einer Anlage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodSuggestion {
    /// Vorgeschlagene Methode (DB-Slug, z. B. "computer_special_2021").
    pub method: String,
    /// Nutzungsdauer in Jahren, falls die Methode sie festlegt (sonst null →
    /// der Nutzer wählt die AfA-Kategorie und damit die Nutzungsdauer).
    pub useful_life_years: Option<f64>,
    /// Vorgeschlagene AfA-Kategorie (BMF-Code), falls ableitbar.
    pub afa_category: Option<String>,
    /// Kurzbegründung für die UI.
    pub reason: String,
}

/// Schlägt eine AfA-Methode vor (PRD §6.17):
/// - Kategorie hardware/software → Computer-Sonderregel (Nutzungsdauer 1 Jahr).
/// - Sonst Preis ≤ GWG-Grenze (netto) → GWG-Sofortabschreibung.
/// - Sonst lineare AfA (Nutzungsdauer wählt der Nutzer über die AfA-Kategorie).
///
/// `expense_category` ist die EÜR-Kategorie der Quell-Kosten (falls aus einer
/// Kosten-Position aktiviert), `gwg_threshold_cents` die GWG-Grenze aus der
/// AfA-Tabellen-Datei.
pub fn suggest_method(
    expense_category: Option<&str>,
    acquisition_cost_cents: i64,
    gwg_threshold_cents: i64,
) -> MethodSuggestion {
    match expense_category {
        Some("hardware") => MethodSuggestion {
            method: DepreciationMethod::ComputerSpecial2021.as_db().into(),
            useful_life_years: Some(1.0),
            afa_category: Some("computer_hardware".into()),
            reason: "Computer-Hardware: digitale Wirtschaftsgüter werden seit BMF \
                     2021 über 1 Jahr abgeschrieben (faktisch sofort)."
                .into(),
        },
        Some("software") => MethodSuggestion {
            method: DepreciationMethod::ComputerSpecial2021.as_db().into(),
            useful_life_years: Some(1.0),
            afa_category: Some("computer_software".into()),
            reason: "Software: digitale Wirtschaftsgüter werden seit BMF 2021 über \
                     1 Jahr abgeschrieben (faktisch sofort)."
                .into(),
        },
        _ if acquisition_cost_cents > 0 && acquisition_cost_cents <= gwg_threshold_cents => {
            MethodSuggestion {
                method: DepreciationMethod::GwgSofort.as_db().into(),
                useful_life_years: None,
                afa_category: None,
                reason: format!(
                    "Geringwertiges Wirtschaftsgut (≤ {:.0} € netto): \
                     Sofortabschreibung im Anschaffungsjahr.",
                    gwg_threshold_cents as f64 / 100.0
                ),
            }
        }
        _ => MethodSuggestion {
            method: DepreciationMethod::Linear.as_db().into(),
            useful_life_years: None,
            afa_category: None,
            reason: "Lineare Abschreibung über die Nutzungsdauer — bitte AfA-Kategorie \
                     wählen, damit die Nutzungsdauer übernommen wird."
                .into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    fn good() -> AssetInput {
        AssetInput {
            label: "MacBook Pro 14".into(),
            acquisition_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            acquisition_cost_cents: 250_000,
            expense_id: None,
            vendor_contact_id: None,
            depreciation_method: "linear".into(),
            useful_life_years: Some(3.0),
            afa_category: Some("computer_hardware".into()),
            business_share_percent: 100.0,
            notes: None,
        }
    }

    #[test]
    fn valid_asset_passes() {
        assert!(validate_asset(&good(), today()).is_ok());
    }

    #[test]
    fn linear_without_useful_life_is_flagged() {
        let mut a = good();
        a.useful_life_years = None;
        let err = validate_asset(&a, today()).unwrap_err();
        assert!(err.contains(&AssetValidationError::UsefulLifeMissingForLinear));
    }

    #[test]
    fn gwg_without_useful_life_is_ok() {
        let mut a = good();
        a.depreciation_method = "gwg_sofort".into();
        a.useful_life_years = None;
        a.acquisition_cost_cents = 50_000;
        assert!(validate_asset(&a, today()).is_ok());
    }

    #[test]
    fn zero_cost_is_flagged() {
        let mut a = good();
        a.acquisition_cost_cents = 0;
        let err = validate_asset(&a, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, AssetValidationError::AcquisitionCostNotPositive { .. })));
    }

    #[test]
    fn future_acquisition_date_is_flagged() {
        let mut a = good();
        a.acquisition_date = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let err = validate_asset(&a, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, AssetValidationError::AcquisitionDateInFuture { .. })));
    }

    #[test]
    fn business_share_out_of_range_is_flagged() {
        let mut a = good();
        a.business_share_percent = 120.0;
        let err = validate_asset(&a, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, AssetValidationError::BusinessShareOutOfRange { .. })));
    }

    #[test]
    fn invalid_method_is_flagged() {
        let mut a = good();
        a.depreciation_method = "degressive".into();
        let err = validate_asset(&a, today()).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, AssetValidationError::MethodInvalid { .. })));
    }

    #[test]
    fn business_book_value_respects_private_share() {
        // 80 % betrieblich von 1000,00 € = 800,00 €.
        assert_eq!(business_book_value_start_cents(100_000, 80.0), 80_000);
        // 100 % → voller Wert.
        assert_eq!(business_book_value_start_cents(250_000, 100.0), 250_000);
        // Rundung (33,33 % von 100,00 € = 33,33 €).
        assert_eq!(business_book_value_start_cents(10_000, 33.33), 3_333);
    }

    #[test]
    fn method_db_roundtrip() {
        for m in [
            DepreciationMethod::GwgSofort,
            DepreciationMethod::Linear,
            DepreciationMethod::ComputerSpecial2021,
        ] {
            assert_eq!(DepreciationMethod::from_db(m.as_db()), Some(m));
        }
        assert_eq!(DepreciationMethod::from_db("nope"), None);
    }

    #[test]
    fn disposal_type_db_roundtrip() {
        for d in [
            DisposalType::Sale,
            DisposalType::Scrap,
            DisposalType::GivenAway,
        ] {
            assert_eq!(DisposalType::from_db(d.as_db()), Some(d));
        }
        assert_eq!(DisposalType::from_db("nope"), None);
    }

    #[test]
    fn suggest_hardware_is_computer_special() {
        let s = suggest_method(Some("hardware"), 250_000, 80_000);
        assert_eq!(s.method, "computer_special_2021");
        assert_eq!(s.useful_life_years, Some(1.0));
        assert_eq!(s.afa_category.as_deref(), Some("computer_hardware"));
    }

    #[test]
    fn suggest_cheap_non_hardware_is_gwg() {
        // 600,00 € Büromöbel → unter GWG-Grenze → Sofortabschreibung.
        let s = suggest_method(Some("office"), 60_000, 80_000);
        assert_eq!(s.method, "gwg_sofort");
        assert_eq!(s.useful_life_years, None);
    }

    #[test]
    fn suggest_expensive_non_hardware_is_linear() {
        // 2.000,00 € Maschine → über GWG-Grenze → linear.
        let s = suggest_method(Some("goods"), 200_000, 80_000);
        assert_eq!(s.method, "linear");
        assert_eq!(s.useful_life_years, None);
    }

    #[test]
    fn suggest_at_gwg_threshold_is_gwg_inclusive() {
        // Genau 800,00 € netto → noch GWG (≤-Grenze).
        let s = suggest_method(Some("office"), 80_000, 80_000);
        assert_eq!(s.method, "gwg_sofort");
    }
}
