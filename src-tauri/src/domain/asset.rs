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
//! ## Anschaffungskosten (Lauf F8, § 9b Abs. 1 EStG)
//!
//! `acquisition_cost_cents` sind die Anschaffungskosten, auf denen die AfA läuft:
//! bei Regelbesteuerung der **Netto**-Wert (die Vorsteuer ist abziehbar), bei
//! §19-Kleinunternehmern der **Brutto**-Wert — die gezahlte Umsatzsteuer ist dort
//! nicht abziehbar und gehört zu den Anschaffungskosten. Seit Lauf F5b zählt eine
//! aktivierte Kostenposition nicht mehr selbst in der EÜR; die Anschaffung wirkt
//! nur über die AfA. [`acquisition_cost_from_expense`] liefert den richtigen
//! Wert, [`check_cost_against_expense`] verlangt ihn genau (Lauf F8b): Weil die
//! EÜR und der DATEV-Stapel die **ganze** Kostenposition herausnehmen, würde ein
//! abweichender Betrag einen Rest nie abziehen (Teil-Aktivierung) oder das
//! Anlagekonto nie auf 0 bringen. Bei Regelbesteuerung bleibt die gezahlte
//! Vorsteuer der Position Betriebsausgabe im Zahlungsjahr (EÜR „gezahlte
//! Vorsteuerbeträge“, DATEV Vorsteuerkonto); nur der Nettobetrag wirkt über die AfA.
//! Ob §19 oder Regelbesteuerung gilt, entscheidet der Status bei Leistungsbezug
//! (Jahr des Belegdatums der Kosten, Lauf F8d, ADR 0039 Nr. 148).
//!
//! **§13b bei §19 (Lauf F8c):** Die selbst geschuldete Steuer steht nicht auf der
//! Lieferantenrechnung; sie wird bei der Zahlung an das Finanzamt als eigene
//! Kostenposition erfasst und als eigene Anlage übernommen (siehe
//! [`check_cost_against_expense`]). Beide Kostenpositionen fallen aus der EÜR
//! heraus, beide Anlagen zusammen schreiben Rechnung + Steuer ab.
//!
//! Die **GWG-Grenze** (§ 6 Abs. 2 Satz 1 EStG) gilt dagegen immer für den Betrag
//! **ohne** enthaltene Umsatzsteuer, auch wenn sie nicht abziehbar ist (R 9b
//! Abs. 2 EStR): Der Methodenvorschlag nimmt bei Aktivierung aus Kosten deren
//! Nettobetrag.

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
    /// Privatentnahme oder Geschenk (Lauf F8) — `disposal_proceeds_cents` ist der
    /// Entnahmewert: bei einer Privatentnahme der Teilwert (§ 6 Abs. 1 Nr. 4
    /// EStG), als Betriebseinnahme wie ein Erlös (Anlage EÜR „Veräußerung oder
    /// Entnahme von Anlagevermögen“); bei einem betrieblichen Geschenk an Dritte 0
    /// (§ 4 Abs. 5 Nr. 1 EStG beachten). Die DB-Kennung bleibt `given_away`.
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
    /// Anschaffungskosten in Cent (voll, vor Privatanteil-Kürzung): netto bei
    /// Regelbesteuerung, brutto bei §19 (siehe Moduldoku).
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
/// Ganzzahlig (R1-S2-FLOAT): der Anteil geht auf 0,01 % genau ein
/// (`Kosten × Basispunkte / 10 000` in `i128`, Half-up).
pub fn business_book_value_start_cents(
    acquisition_cost_cents: i64,
    business_share_percent: f64,
) -> i64 {
    let share = business_share_percent.clamp(0.0, 100.0);
    // NaN bleibt nach clamp NaN; der Cast macht daraus 0 (die Validierung
    // lehnt NaN vorher ab).
    let basis_points = (share * 100.0).round() as i64;
    crate::domain::money::mul_div_round(acquisition_cost_cents, basis_points, 10_000)
}

/// Anschaffungskosten einer aus Kosten aktivierten Anlage (Lauf F8, § 9b Abs. 1
/// EStG): bei §19 der Bruttobetrag — die Umsatzsteuer ist nicht abziehbar —, bei
/// Regelbesteuerung der Nettobetrag. Maßgeblich ist der Status bei
/// Leistungsbezug (Jahr des Belegdatums, § 15 Abs. 1 UStG; Lauf F8d), nicht der
/// des Zahlungsjahres.
pub fn acquisition_cost_from_expense(
    net_amount_cents: i64,
    gross_amount_cents: i64,
    is_kleinunternehmer: bool,
) -> i64 {
    if is_kleinunternehmer {
        gross_amount_cents
    } else {
        net_amount_cents
    }
}

/// Lauf F8b (§ 9b Abs. 1, § 4 Abs. 3 Satz 3 EStG, GoBD-Nachvollziehbarkeit):
/// Die Anschaffungskosten einer aus Kosten übernommenen Anlage müssen genau
/// [`acquisition_cost_from_expense`] entsprechen. EÜR und DATEV-Stapel nehmen
/// die ganze Kostenposition heraus; ein kleinerer Betrag (Teil-Aktivierung einer
/// Sammelrechnung) ließe den Rest in keinem Jahr wirken, ein größerer
/// (Nebenkosten aus einem anderen Beleg) würde ohne Ausgabe abgeschrieben, und
/// das Anlagekonto ginge nie auf 0. Sammelrechnungen werden vorher in getrennte
/// Kostenpositionen aufgeteilt; Nebenkosten auf demselben Beleg gehören in
/// dieselbe Kostenposition, Nebenkosten aus einem eigenen Beleg werden als eigene
/// Kostenposition erfasst und als eigene Anlage (gleiche Methode und
/// Nutzungsdauer) übernommen. `None` = in Ordnung, sonst die Meldung.
///
/// Lauf F8c (§ 13b Abs. 5 Satz 1, Abs. 8 UStG; § 9b Abs. 1 EStG; ADR 0039
/// Nr. 147): Bei §19 und `reverse_charge_13b` gehört die selbst geschuldete,
/// nicht abziehbare Umsatzsteuer zu den Anschaffungskosten, steht aber nicht
/// auf der Lieferantenrechnung (netto = brutto). Sie wird bei der Zahlung an
/// das Finanzamt als eigene Kostenposition erfasst und als eigene Anlage
/// (gleiches Anschaffungsdatum, gleiche Methode und Nutzungsdauer) übernommen;
/// die Rechnungsposition selbst bleibt beim Rechnungsbetrag. Ein höherer Betrag
/// bekommt deshalb eine eigene Meldung, die diesen Weg nennt.
pub fn check_cost_against_expense(
    acquisition_cost_cents: i64,
    net_amount_cents: i64,
    gross_amount_cents: i64,
    is_kleinunternehmer: bool,
    reverse_charge_13b: bool,
) -> Option<String> {
    use crate::domain::money::eur;
    let expected =
        acquisition_cost_from_expense(net_amount_cents, gross_amount_cents, is_kleinunternehmer);
    if acquisition_cost_cents == expected {
        return None;
    }
    if reverse_charge_13b && is_kleinunternehmer && acquisition_cost_cents > expected {
        return Some(format!(
            "Bei Reverse-Charge (§ 13b UStG) schuldest Du als Kleinunternehmer die \
             Umsatzsteuer selbst und kannst sie nicht als Vorsteuer abziehen; sie gehört zu \
             den Anschaffungskosten (§ 9b Abs. 1 EStG). Diese Kostenposition enthält sie \
             nicht: Übernimm hier den Rechnungsbetrag {} statt {}. Die selbst geschuldete \
             Steuer erfasst Du bei der Zahlung an das Finanzamt als eigene Kostenposition \
             (nur der Anteil dieser Anlage, USt-Betrag 0, ohne §13b-Schalter) und \
             übernimmst sie als eigene Anlage mit gleichem Anschaffungsdatum, gleicher \
             Methode und Nutzungsdauer. So wirkt sie über die AfA und nicht zusätzlich als \
             Betriebsausgabe.",
            eur(expected),
            eur(acquisition_cost_cents)
        ));
    }
    if net_amount_cents != gross_amount_cents
        && is_kleinunternehmer
        && acquisition_cost_cents == net_amount_cents
    {
        return Some(format!(
            "Als Kleinunternehmer (§ 19 UStG) kannst Du die Umsatzsteuer nicht als \
             Vorsteuer abziehen; sie gehört zu den Anschaffungskosten (§ 9b Abs. 1 EStG). \
             Bitte den Bruttobetrag {} statt des Nettobetrags {} übernehmen.",
            eur(gross_amount_cents),
            eur(net_amount_cents)
        ));
    }
    if net_amount_cents != gross_amount_cents
        && !is_kleinunternehmer
        && acquisition_cost_cents == gross_amount_cents
    {
        return Some(format!(
            "Bei Regelbesteuerung ziehst Du die Umsatzsteuer als Vorsteuer ab; \
             Anschaffungskosten sind der Nettobetrag {} statt des Bruttobetrags {}.",
            eur(net_amount_cents),
            eur(gross_amount_cents)
        ));
    }
    let basis = if is_kleinunternehmer {
        "als Kleinunternehmer (§ 19 UStG) der Bruttobetrag"
    } else {
        "bei Regelbesteuerung der Nettobetrag"
    };
    Some(format!(
        "Die Anschaffungskosten einer aus Kosten übernommenen Anlage müssen dem Betrag \
         der Kosten entsprechen: {} {} statt {}. Die EÜR nimmt die ganze Kostenposition \
         heraus; ein abweichender Betrag würde einen Rest nie oder einen Mehrbetrag ohne \
         Ausgabe abschreiben. Ist nur ein Teil der Rechnung eine Anlage, teile sie vorher \
         in getrennte Kostenpositionen auf. Nebenkosten auf demselben Beleg gehören in \
         dieselbe Kostenposition; Nebenkosten aus einem eigenen Beleg erfasst Du als eigene \
         Kostenposition und übernimmst sie als eigene Anlage mit gleicher Methode und \
         Nutzungsdauer.",
        basis,
        eur(expected),
        eur(acquisition_cost_cents)
    ))
}

// ---- Validation ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum AssetValidationError {
    LabelMissing,
    AcquisitionCostNotPositive {
        cost: i64,
    },
    /// Anschaffungskosten über der Domänengrenze (F5-fix).
    AcquisitionCostOutOfRange,
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
    if !crate::domain::money::amount_in_range(input.acquisition_cost_cents) {
        errs.push(E::AcquisitionCostOutOfRange);
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
    use crate::domain::money::eur;
    use AssetValidationError as E;
    match e {
        E::LabelMissing => "Bezeichnung ist erforderlich.".into(),
        E::AcquisitionCostNotPositive { cost } => {
            format!(
                "Anschaffungskosten müssen größer als 0 sein ({}).",
                eur(*cost)
            )
        }
        E::AcquisitionCostOutOfRange => crate::domain::money::amount_out_of_range_message(),
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
        E::AcquisitionCostOutOfRange => "AcquisitionCostOutOfRange",
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
                    "Geringwertiges Wirtschaftsgut (≤ {} netto): \
                     Sofortabschreibung im Anschaffungsjahr.",
                    crate::domain::money::eur_whole(gwg_threshold_cents)
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

// ---- Veräußerung (R1-020) ---------------------------------------------------

/// Fehler beim Prüfen eines Anlagen-Abgangs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisposalValidationError {
    /// Abgangsdatum liegt nach heute (Europe/Berlin).
    DateInFuture { disposal_date: NaiveDate },
    /// Abgangsdatum liegt vor der Anschaffung.
    DateBeforeAcquisition {
        disposal_date: NaiveDate,
        acquisition_date: NaiveDate,
    },
    /// Negativer Verkaufserlös.
    ProceedsNegative { proceeds_cents: i64 },
    /// Verkaufserlös über der Domänengrenze (F5-fix).
    ProceedsOutOfRange,
}

/// Prüft einen Abgang im Functional Core (R1-020) und liefert den zu buchenden
/// Erlös: beim Verkauf den Erlös, bei Privatentnahme/Geschenk den Entnahmewert
/// (Lauf F8; beide nicht negativ), bei Verschrottung 0. Das Datum muss
/// zwischen Anschaffung und heute liegen — ein Abgang vor der Anschaffung hätte
/// keine anteilige AfA und schöbe den Restbuchwert in ein Jahr vor dem Zugang.
pub fn validate_disposal(
    disposal_type: DisposalType,
    proceeds_cents: i64,
    disposal_date: NaiveDate,
    acquisition_date: NaiveDate,
    today: NaiveDate,
) -> Result<i64, DisposalValidationError> {
    if disposal_date > today {
        return Err(DisposalValidationError::DateInFuture { disposal_date });
    }
    if disposal_date < acquisition_date {
        return Err(DisposalValidationError::DateBeforeAcquisition {
            disposal_date,
            acquisition_date,
        });
    }
    match disposal_type {
        DisposalType::Scrap => Ok(0),
        DisposalType::Sale | DisposalType::GivenAway if proceeds_cents < 0 => {
            Err(DisposalValidationError::ProceedsNegative { proceeds_cents })
        }
        DisposalType::Sale | DisposalType::GivenAway
            if !crate::domain::money::amount_in_range(proceeds_cents) =>
        {
            Err(DisposalValidationError::ProceedsOutOfRange)
        }
        DisposalType::Sale | DisposalType::GivenAway => Ok(proceeds_cents),
    }
}

/// Deutsche Meldung zu [`DisposalValidationError`].
pub fn disposal_message(e: &DisposalValidationError) -> String {
    match e {
        DisposalValidationError::DateInFuture { .. } => {
            "Das Veräußerungsdatum darf nicht in der Zukunft liegen.".into()
        }
        DisposalValidationError::DateBeforeAcquisition {
            disposal_date,
            acquisition_date,
        } => format!(
            "Das Abgangsdatum {} liegt vor dem Anschaffungsdatum {} — bitte das Datum prüfen.",
            disposal_date.format("%d.%m.%Y"),
            acquisition_date.format("%d.%m.%Y")
        ),
        DisposalValidationError::ProceedsNegative { .. } => {
            "Der Verkaufserlös bzw. Entnahmewert darf nicht negativ sein.".into()
        }
        DisposalValidationError::ProceedsOutOfRange => {
            crate::domain::money::amount_out_of_range_message()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // R1-020: Abgang im Core geprüft — Datum zwischen Anschaffung und heute,
    // Erlös nur beim Verkauf und nie negativ.
    #[test]
    fn validate_disposal_dates_and_proceeds() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        let (acq, today) = (d(2025, 3, 10), d(2026, 5, 20));
        assert_eq!(
            validate_disposal(DisposalType::Sale, 12_000, d(2026, 1, 5), acq, today),
            Ok(12_000)
        );
        assert_eq!(
            validate_disposal(DisposalType::Scrap, 12_000, acq, acq, today),
            Ok(0)
        );
        assert_eq!(
            validate_disposal(DisposalType::GivenAway, 5, today, acq, today),
            Ok(5)
        );
        // Lauf F8: Privatentnahme mit Teilwert; betriebliches Geschenk mit 0.
        assert_eq!(
            validate_disposal(DisposalType::GivenAway, 30_000, today, acq, today),
            Ok(30_000)
        );
        assert_eq!(
            validate_disposal(DisposalType::GivenAway, 0, today, acq, today),
            Ok(0)
        );
        assert_eq!(
            validate_disposal(DisposalType::GivenAway, -1, today, acq, today),
            Err(DisposalValidationError::ProceedsNegative { proceeds_cents: -1 })
        );
        assert!(matches!(
            validate_disposal(DisposalType::Sale, 1, d(2026, 5, 21), acq, today),
            Err(DisposalValidationError::DateInFuture { .. })
        ));
        let before = validate_disposal(DisposalType::Sale, 1, d(2025, 3, 9), acq, today);
        assert!(matches!(
            before,
            Err(DisposalValidationError::DateBeforeAcquisition { .. })
        ));
        assert!(disposal_message(&before.unwrap_err()).contains("09.03.2025"));
        assert_eq!(
            validate_disposal(DisposalType::Sale, -1, today, acq, today),
            Err(DisposalValidationError::ProceedsNegative { proceeds_cents: -1 })
        );
    }

    // KB-0006/KB-0007: Fehlertexte rein über i64, Vorzeichen und Cent exakt.
    #[test]
    fn message_formats_amounts_without_float() {
        let m = message(&AssetValidationError::AcquisitionCostNotPositive { cost: -1_005 });
        assert!(m.contains("(-10,05 €)"), "{m}");
        let m = message(&AssetValidationError::AcquisitionCostNotPositive { cost: 0 });
        assert!(m.contains("(0,00 €)"), "{m}");
    }

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
        assert!(s.reason.contains("(≤ 800 € netto)"), "{}", s.reason);
    }

    /// F5-fix: Anschaffungskosten und Abgangserlös über 1 Mrd. € werden
    /// abgelehnt (Überlaufschutz der Jahressummen).
    #[test]
    fn amounts_above_limit_are_rejected() {
        use crate::domain::money::MAX_AMOUNT_CENTS;
        let mut a = good();
        a.acquisition_cost_cents = MAX_AMOUNT_CENTS + 1;
        let err = validate_asset(&a, today()).unwrap_err();
        assert!(err.contains(&AssetValidationError::AcquisitionCostOutOfRange));
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        let (acq, now) = (d(2025, 3, 10), d(2026, 5, 20));
        assert_eq!(
            validate_disposal(
                DisposalType::Sale,
                MAX_AMOUNT_CENTS,
                d(2026, 1, 5),
                acq,
                now
            ),
            Ok(MAX_AMOUNT_CENTS)
        );
        assert_eq!(
            validate_disposal(DisposalType::Sale, i64::MAX, d(2026, 1, 5), acq, now),
            Err(DisposalValidationError::ProceedsOutOfRange)
        );
    }

    // Lauf F8/F8b (§ 9b Abs. 1 EStG): bei §19 brutto, bei Regelbesteuerung netto;
    // jeder andere Betrag wird abgelehnt (auch Teil-Aktivierung und Nebenkosten).
    #[test]
    fn acquisition_cost_from_expense_by_tax_status() {
        assert_eq!(
            acquisition_cost_from_expense(100_000, 119_000, true),
            119_000
        );
        assert_eq!(
            acquisition_cost_from_expense(100_000, 119_000, false),
            100_000
        );
        assert!(check_cost_against_expense(119_000, 100_000, 119_000, true, false).is_none());
        let m = check_cost_against_expense(100_000, 100_000, 119_000, true, false).unwrap();
        assert!(m.contains("1190,00") && m.contains("1000,00"), "{m}");
        assert!(check_cost_against_expense(100_000, 100_000, 119_000, false, false).is_none());
        assert!(check_cost_against_expense(119_000, 100_000, 119_000, false, false).is_some());
        // Lauf F8b: Teil-Aktivierung und Nebenkosten (Mehrbetrag) abgelehnt, mit
        // Hinweis auf das Aufteilen; Kosten ohne USt: genau der Betrag.
        let m = check_cost_against_expense(50_000, 100_000, 119_000, true, false).unwrap();
        assert!(
            m.contains("1190,00") && m.contains("500,00") && m.contains("getrennte"),
            "{m}"
        );
        let m = check_cost_against_expense(105_000, 100_000, 119_000, false, false).unwrap();
        assert!(
            m.contains("Nettobetrag") && m.contains("Nebenkosten"),
            "{m}"
        );
        assert!(check_cost_against_expense(80_000, 80_000, 80_000, true, false).is_none());
        assert!(check_cost_against_expense(80_000, 80_000, 80_000, false, false).is_none());
        assert!(check_cost_against_expense(79_999, 80_000, 80_000, true, false).is_some());
    }

    // Lauf F8c (§ 13b Abs. 8 UStG, § 9b Abs. 1 EStG): §13b-Kosten stehen netto
    // (= brutto) in der Kostenposition. Bei §19 bleibt die Anlage beim
    // Rechnungsbetrag; ein Betrag mit der selbst geschuldeten Steuer wird mit
    // dem Weg über eine eigene Kostenposition abgelehnt. Bei Regelbesteuerung ist
    // die Steuer Vorsteuer, dort gilt die allgemeine Meldung.
    #[test]
    fn reverse_charge_under_par19_points_to_separate_tax_asset() {
        assert!(check_cost_against_expense(100_000, 100_000, 100_000, true, true).is_none());
        let m = check_cost_against_expense(119_000, 100_000, 100_000, true, true).unwrap();
        assert!(
            m.contains("§ 13b")
                && m.contains("1000,00")
                && m.contains("1190,00")
                && m.contains("eigene Anlage")
                && m.contains("nicht zusätzlich als")
                && m.contains("Betriebsausgabe"),
            "{m}"
        );
        // Teilbetrag: allgemeine Meldung (Aufteilen).
        let m = check_cost_against_expense(50_000, 100_000, 100_000, true, true).unwrap();
        assert!(m.contains("getrennte") && !m.contains("§ 13b"), "{m}");
        // Regelbesteuerung: §13b-Steuer ist abziehbar, Anschaffungskosten netto.
        assert!(check_cost_against_expense(100_000, 100_000, 100_000, false, true).is_none());
        let m = check_cost_against_expense(119_000, 100_000, 100_000, false, true).unwrap();
        assert!(!m.contains("§ 13b"), "{m}");
    }
}
