//! AfA-Berechnung (Functional Core) — Phase 2C, Block 12.
//!
//! Reine Jahres-AfA-Rechnung pro Anlage. Keine I/O — die Buchung ins DB liegt in
//! [`crate::depreciation::accrue_yearly`], die Stammdaten in
//! [`crate::db::repo::assets`].
//!
//! ## Methoden (PRD §6.7)
//!
//! - **`gwg_sofort`** & **`computer_special_2021`**: Sofortabschreibung im
//!   Anschaffungsjahr — die volle (betriebliche) Bemessungsgrundlage wird
//!   abgeschrieben, Restbuchwert danach 0. In Folgejahren kein Eintrag.
//! - **`linear`**: monatsgenau. Erste Jahres-AfA =
//!   (betrieblicher Wert / Nutzungsdauer) × (genutzte Monate / 12). Folgejahre
//!   volle Jahres-AfA; das letzte (Teil-)Jahr nimmt den verbleibenden
//!   Restbuchwert auf, sodass die Reihe sauber auf 0 ausläuft.
//!
//! ## Privatanteil
//!
//! Die Bemessungsgrundlage ist der **betriebliche** Anteil
//! ([`crate::domain::asset::business_book_value_start_cents`]). `previous_book_value_cents`
//! ist immer der bereits anteilige Restbuchwert.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::domain::asset::{business_book_value_start_cents, DepreciationMethod};
use crate::domain::money;

/// Stammdaten, die die Jahres-AfA-Rechnung braucht (Ausschnitt der Anlage).
#[derive(Debug, Clone)]
pub struct DepreciationAsset {
    pub depreciation_method: DepreciationMethod,
    pub acquisition_date: NaiveDate,
    /// Anschaffungskosten (voll, vor Privatanteil; bei §19 brutto, sonst netto,
    /// § 9b Abs. 1 EStG) — für die lineare
    /// Jahresrate.
    pub acquisition_cost_cents: i64,
    pub business_share_percent: f64,
    /// Bei `linear` Pflicht; sonst ignoriert.
    pub useful_life_years: Option<f64>,
}

/// Ergebnis einer Jahres-AfA-Rechnung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepreciationCalc {
    pub depreciation_amount_cents: i64,
    pub months_in_year: i32,
    pub book_value_before_cents: i64,
    pub book_value_after_cents: i64,
    /// Sofort-Vollabschreibung (GWG / Computer-Sonderregel) im Anschaffungsjahr.
    pub is_full_writeoff: bool,
}

impl DepreciationCalc {
    /// Es gibt nichts zu buchen (kein Betrag).
    pub fn is_noop(&self) -> bool {
        self.depreciation_amount_cents == 0
    }

    fn noop(book_value: i64) -> Self {
        DepreciationCalc {
            depreciation_amount_cents: 0,
            months_in_year: 0,
            book_value_before_cents: book_value,
            book_value_after_cents: book_value,
            is_full_writeoff: false,
        }
    }
}

/// Genutzte Monate im Anschaffungsjahr: ab dem Anschaffungsmonat bis Dezember
/// (Anschaffung im Januar → 12, im Juli → 6, im Dezember → 1).
pub fn months_used_in_acquisition_year(acquisition_date: NaiveDate) -> i32 {
    13 - acquisition_date.month() as i32
}

/// Volle lineare Jahres-AfA (betrieblicher Wert / Nutzungsdauer), kaufmännisch
/// auf Cent gerundet. Ganzzahlig (R1-S2-FLOAT): die Nutzungsdauer geht auf
/// 1/100 Jahr genau ein, gerechnet wird `Wert × 100 / Dauer_in_Hundertstel` in
/// `i128` mit Half-up-Rundung.
pub fn annual_linear_cents(business_value_cents: i64, useful_life_years: f64) -> i64 {
    money::mul_div_round(
        business_value_cents,
        100,
        life_hundredths(useful_life_years),
    )
}

/// Nutzungsdauer in Hundertstel Jahren, mindestens 1 (Schutz vor Division durch
/// 0; NaN und negative Werte lehnt die Validierung der Anlage vorher ab).
fn life_hundredths(useful_life_years: f64) -> i64 {
    let h = (useful_life_years * 100.0).round();
    if h.is_finite() && h >= 1.0 {
        h.min(i64::MAX as f64) as i64
    } else {
        1
    }
}

/// Anteilige AfA für `months` von 12 Monaten: `annual × months / 12`, ganzzahlig
/// mit Half-up-Rundung (R1-S2-FLOAT).
pub fn pro_rata_cents(annual_cents: i64, months: i32) -> i64 {
    money::mul_div_round(annual_cents, i64::from(months), 12)
}

/// Genutzte Monate im Veräußerungsjahr: vom Jahresanfang (oder dem Anschaffungs-
/// monat, falls im selben Jahr) bis einschließlich dem Veräußerungsmonat.
/// `disposal_date.month()` zählt **voll mit** — R 7.4 Abs. 7 EStR.
///
/// - Disposal nach Vorjahr (typisch): Jan..disposal_month → `disposal_month`.
/// - Disposal im Anschaffungsjahr: acq_month..disposal_month inkl. →
///   `disposal_month - acq_month + 1` (mindestens 1).
pub fn months_until_disposal(acquisition_date: NaiveDate, disposal_date: NaiveDate) -> i32 {
    let disp_m = disposal_date.month() as i32;
    if acquisition_date.year() == disposal_date.year() {
        let acq_m = acquisition_date.month() as i32;
        // Disposal vor Anschaffung sollte vom Validator schon abgefangen sein;
        // defensiv mindestens 1 Monat (sonst rutscht Pro-rata auf 0 und der
        // Buchwert wandert ungekürzt in die Disposal-Rechnung).
        (disp_m - acq_m + 1).max(1)
    } else {
        disp_m
    }
}

/// Berechnet die AfA einer Anlage für ein Geschäftsjahr.
///
/// `previous_book_value_cents` ist der Restbuchwert, mit dem die Anlage **in**
/// dieses Jahr geht (im Anschaffungsjahr = betrieblicher Start-Restbuchwert).
pub fn compute_yearly(
    asset: &DepreciationAsset,
    fiscal_year: i32,
    previous_book_value_cents: i64,
) -> DepreciationCalc {
    let acq_year = asset.acquisition_date.year();

    // Restbuchwert bereits am Erinnerungswert (1 Cent, R2-020/KB-0027) oder darunter
    // → nichts mehr abzuschreiben. Erst Disposal löst den Erinnerungswert auf.
    // GWG/Computer-Sonderregel sind oben spezialisiert und nicht betroffen.
    if previous_book_value_cents <= 1 {
        return DepreciationCalc::noop(previous_book_value_cents.max(0));
    }
    // Vor dem Anschaffungsjahr gibt es keine AfA.
    if fiscal_year < acq_year {
        return DepreciationCalc::noop(previous_book_value_cents);
    }

    match asset.depreciation_method {
        DepreciationMethod::GwgSofort | DepreciationMethod::ComputerSpecial2021 => {
            if fiscal_year == acq_year {
                // Volle Sofortabschreibung im Anschaffungsjahr.
                DepreciationCalc {
                    depreciation_amount_cents: previous_book_value_cents,
                    months_in_year: months_used_in_acquisition_year(asset.acquisition_date),
                    book_value_before_cents: previous_book_value_cents,
                    book_value_after_cents: 0,
                    is_full_writeoff: true,
                }
            } else {
                // In Folgejahren bereits abgeschrieben.
                DepreciationCalc::noop(previous_book_value_cents)
            }
        }
        DepreciationMethod::Linear => {
            let business_value = business_book_value_start_cents(
                asset.acquisition_cost_cents,
                asset.business_share_percent,
            );
            let life = asset.useful_life_years.unwrap_or(1.0);
            let annual = annual_linear_cents(business_value, life);

            let months = if fiscal_year == acq_year {
                months_used_in_acquisition_year(asset.acquisition_date)
            } else {
                12
            };

            let mut amount = pro_rata_cents(annual, months);

            // Safety: bei sehr kleinen Werten/zu großer Nutzungsdauer könnte die
            // Rate auf 0 runden — dann den Rest abschreiben, statt zu stocken.
            if amount <= 0 {
                amount = previous_book_value_cents;
            }
            // Letztes (Teil-)Jahr: nie mehr als der Restbuchwert. Erinnerungswert
            // (R2-020, KB-0027 in ADR 0039 entschieden): Solange die Anlage genutzt
            // wird, bleibt 1 Cent im Bestand, damit sie im Anlageverzeichnis
            // (§ 4 Abs. 3 Satz 5 EStG) sichtbar bleibt. Das ist Buchführungs-
            // praxis, keine Pflicht aus EStG oder EStR für die EÜR; steuerlich
            // neutral, weil der Cent beim Abgang (`assets_dispose`) über den
            // Restbuchwert (Anlage EÜR Kz 135) als Betriebsausgabe wirkt.
            // GWG/Computer-Sonderregel (Sofortabschreibung) bleibt davon unberührt
            // und läuft auf 0 — die Anlage gilt steuerlich sofort als verbraucht.
            if amount >= previous_book_value_cents {
                amount = if previous_book_value_cents > 1 {
                    previous_book_value_cents - 1
                } else {
                    previous_book_value_cents
                };
            }

            DepreciationCalc {
                depreciation_amount_cents: amount,
                months_in_year: months,
                book_value_before_cents: previous_book_value_cents,
                book_value_after_cents: previous_book_value_cents - amount,
                is_full_writeoff: false,
            }
        }
    }
}

/// Pro-rata-AfA im Veräußerungsjahr (R2-021).
///
/// R 7.4 Abs. 7 EStR + §7 Abs. 1 Satz 4 EStG: Im Jahr der Veräußerung wird
/// noch anteilig für die Monate bis zum Veräußerungsmonat AfA gebucht. Der
/// gekürzte Restbuchwert geht dann in die Disposal-Rechnung. Ohne diese
/// Buchung wäre der Veräußerungsgewinn systematisch zu hoch (bzw. der
/// Verlust zu niedrig).
///
/// Aufrufer: `assets_dispose`, **bevor** `assets::dispose` ausgeführt wird,
/// und nur falls für (Anlage, disposal_year) noch keine AfA-Zeile existiert
/// (Idempotenz; reguläre Jahres-AfA hat dann Vorrang).
pub fn compute_disposal_year_partial(
    asset: &DepreciationAsset,
    previous_book_value_cents: i64,
    disposal_date: NaiveDate,
) -> DepreciationCalc {
    let acq_year = asset.acquisition_date.year();
    let disp_year = disposal_date.year();

    // Disposal vor Anschaffung ist ein Datenfehler — defensiv noop.
    if disp_year < acq_year {
        return DepreciationCalc::noop(previous_book_value_cents);
    }
    if previous_book_value_cents <= 0 {
        return DepreciationCalc::noop(previous_book_value_cents.max(0));
    }

    match asset.depreciation_method {
        DepreciationMethod::GwgSofort | DepreciationMethod::ComputerSpecial2021 => {
            // Sofortabschreibung greift nur im Anschaffungsjahr; danach ist
            // der Restbuchwert per Definition 0 → kein partielles AfA.
            if disp_year == acq_year {
                DepreciationCalc {
                    depreciation_amount_cents: previous_book_value_cents,
                    months_in_year: months_used_in_acquisition_year(asset.acquisition_date),
                    book_value_before_cents: previous_book_value_cents,
                    book_value_after_cents: 0,
                    is_full_writeoff: true,
                }
            } else {
                DepreciationCalc::noop(previous_book_value_cents)
            }
        }
        DepreciationMethod::Linear => {
            let business_value = business_book_value_start_cents(
                asset.acquisition_cost_cents,
                asset.business_share_percent,
            );
            let life = asset.useful_life_years.unwrap_or(1.0);
            let annual = annual_linear_cents(business_value, life);
            let months = months_until_disposal(asset.acquisition_date, disposal_date);

            let mut amount = pro_rata_cents(annual, months);
            if amount < 0 {
                amount = 0;
            }
            // Niemals mehr als der vorhandene Restbuchwert; das letzte Cent
            // (Erinnerungswert aus R2-020) darf hier ruhig ausgebucht werden,
            // weil das Disposal die Anlage aus dem Bestand entfernt.
            if amount > previous_book_value_cents {
                amount = previous_book_value_cents;
            }

            DepreciationCalc {
                depreciation_amount_cents: amount,
                months_in_year: months,
                book_value_before_cents: previous_book_value_cents,
                book_value_after_cents: previous_book_value_cents - amount,
                is_full_writeoff: false,
            }
        }
    }
}

/// Gebuchtes AfA-Jahr einer Anlage (Ausschnitt aus `depreciation_entries`)
/// für [`plan_disposal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookedYear {
    pub fiscal_year: i64,
    pub amount_cents: i64,
    pub book_value_after_cents: i64,
    /// Festgeschrieben (Jahresabschluss).
    pub locked: bool,
}

/// Ergebnis von [`plan_disposal`]: was ein Abgang zum Datum buchen würde.
#[derive(Debug, Clone, PartialEq)]
pub struct DisposalPlan {
    /// Offene AfA-Jahre ab dem Abgangsjahr, die der Abgang ersetzt
    /// (Jahr, Betrag).
    pub replaced: Vec<(i64, i64)>,
    /// Nachgeholte AfA der Vorjahre ohne Buchung (Jahr, Betrag).
    pub catch_up: Vec<(i64, i64)>,
    /// Restbuchwert zu Beginn des Abgangsjahres (nach Ersetzen und Nachholen).
    pub book_value_before_cents: i64,
    /// Anteilige AfA im Abgangsjahr (R 7.4 Abs. 8 EStR).
    pub disposal_year: DepreciationCalc,
    /// Ein abgeschlossenes Jahr nach dem Abgangsjahr bzw. festgeschriebene AfA
    /// ab dem Abgangsjahr — der Abgang würde abgelehnt (spätestes solches Jahr).
    pub closed_conflict_year: Option<i64>,
}

impl DisposalPlan {
    /// Restbuchwert zum Abgang (geht als Ausgabe in die EÜR).
    pub fn residual_book_value_cents(&self) -> i64 {
        self.disposal_year.book_value_after_cents
    }
}

/// Plant einen Abgang zum `disposal_date`, **ohne** zu buchen (Lauf 2026.11
/// U4, Vorschau des Restbuchwerts). Spiegelt die Schritte von
/// `commands::assets::assets_dispose`:
///
/// 1. Offene AfA ab dem Abgangsjahr wird ersetzt; der Restbuchwert steht danach
///    auf dem letzten Stand davor (bzw. dem betrieblichen Startwert). Ohne
///    solche Einträge gilt der aktuelle Restbuchwert der Anlage.
/// 2. Vorjahre ohne gebuchte AfA werden nachgeholt ([`compute_yearly`]);
///    abgeschlossene Jahre bleiben unberührt.
/// 3. Anteilige AfA im Abgangsjahr ([`compute_disposal_year_partial`]).
///
/// `closed_years` sind die abgeschlossenen Geschäftsjahre (`fiscal_year_locks`).
pub fn plan_disposal(
    asset: &DepreciationAsset,
    acquisition_fiscal_year: i64,
    current_book_value_cents: i64,
    booked: &[BookedYear],
    closed_years: &[i64],
    disposal_date: NaiveDate,
) -> DisposalPlan {
    let disposal_year = i64::from(disposal_date.year());

    // 1. Ab dem Abgangsjahr gebuchte AfA.
    let mut from_disposal: Vec<&BookedYear> = booked
        .iter()
        .filter(|b| b.fiscal_year >= disposal_year)
        .collect();
    from_disposal.sort_by_key(|b| b.fiscal_year);
    let closed_after = closed_years
        .iter()
        .copied()
        .filter(|y| *y > disposal_year && *y >= acquisition_fiscal_year)
        .max();
    let locked_after = from_disposal
        .iter()
        .filter(|b| b.locked)
        .map(|b| b.fiscal_year)
        .max();
    let closed_conflict_year = closed_after.max(locked_after);

    let mut running = if from_disposal.is_empty() {
        current_book_value_cents
    } else {
        booked
            .iter()
            .filter(|b| b.fiscal_year < disposal_year)
            .max_by_key(|b| b.fiscal_year)
            .map(|b| b.book_value_after_cents)
            .unwrap_or_else(|| {
                business_book_value_start_cents(
                    asset.acquisition_cost_cents,
                    asset.business_share_percent,
                )
            })
    };
    let replaced = from_disposal
        .iter()
        .map(|b| (b.fiscal_year, b.amount_cents))
        .collect();

    // 2. Vorjahre ohne Buchung nachholen (wie `accrue_asset_through_in`).
    let mut catch_up = Vec::new();
    if disposal_year > acquisition_fiscal_year {
        for year in acquisition_fiscal_year..disposal_year {
            if running <= 0 {
                break;
            }
            let already = booked.iter().any(|b| b.fiscal_year == year);
            if already || closed_years.contains(&year) {
                continue;
            }
            let calc = compute_yearly(asset, year as i32, running);
            if calc.is_noop() {
                continue;
            }
            running = calc.book_value_after_cents;
            catch_up.push((year, calc.depreciation_amount_cents));
        }
    }

    // 3. Anteilige AfA im Abgangsjahr.
    let disposal_year_calc = compute_disposal_year_partial(asset, running, disposal_date);
    DisposalPlan {
        replaced,
        catch_up,
        book_value_before_cents: running,
        disposal_year: disposal_year_calc,
        closed_conflict_year,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // R1-S2-FLOAT: AfA ganzzahlig mit Half-up-Rundung.
    #[test]
    fn annual_and_pro_rata_are_integer_half_up() {
        // 1.000,00 € / 3 Jahre = 333,333… → 333,33 €.
        assert_eq!(annual_linear_cents(100_000, 3.0), 33_333);
        // 1.000,05 € / 2 = 500,025 → 500,03 € (Halbcent rundet auf).
        assert_eq!(annual_linear_cents(100_005, 2.0), 50_003);
        // Bruchteil-Nutzungsdauer auf 1/100 Jahr: 1.000 € / 2,5 = 400 €.
        assert_eq!(annual_linear_cents(100_000, 2.5), 40_000);
        // Unsinnige Dauer teilt nie durch 0.
        assert_eq!(annual_linear_cents(100, 0.0), 10_000);
        // 333,33 € × 7/12 = 194,4425 → 194,44 €; × 6/12 = 166,665 → 166,67 €.
        assert_eq!(pro_rata_cents(33_333, 7), 19_444);
        assert_eq!(pro_rata_cents(33_333, 6), 16_667);
        assert_eq!(pro_rata_cents(33_333, 12), 33_333);
        // Betrieblicher Anteil: 1.234,57 € × 33,33 % = 411,48…  → 411,48 €.
        assert_eq!(business_book_value_start_cents(123_457, 33.33), 41_148);
        // 0,05 € × 50 % = 0,025 € → 0,03 €.
        assert_eq!(business_book_value_start_cents(5, 50.0), 3);
        assert_eq!(business_book_value_start_cents(10_000, 150.0), 10_000);
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn asset(
        method: DepreciationMethod,
        acq: NaiveDate,
        cost: i64,
        share: f64,
        life: Option<f64>,
    ) -> DepreciationAsset {
        DepreciationAsset {
            depreciation_method: method,
            acquisition_date: acq,
            acquisition_cost_cents: cost,
            business_share_percent: share,
            useful_life_years: life,
        }
    }

    #[test]
    fn gwg_writes_off_fully_in_acquisition_year() {
        let a = asset(
            DepreciationMethod::GwgSofort,
            d(2026, 3, 10),
            60_000,
            100.0,
            None,
        );
        let start =
            business_book_value_start_cents(a.acquisition_cost_cents, a.business_share_percent);
        let calc = compute_yearly(&a, 2026, start);
        assert_eq!(calc.depreciation_amount_cents, 60_000);
        assert_eq!(calc.book_value_after_cents, 0);
        assert!(calc.is_full_writeoff);
        // Folgejahr: nichts mehr.
        let next = compute_yearly(&a, 2027, calc.book_value_after_cents);
        assert!(next.is_noop());
    }

    #[test]
    fn computer_special_writes_off_fully_in_acquisition_year() {
        let a = asset(
            DepreciationMethod::ComputerSpecial2021,
            d(2026, 7, 1),
            250_000,
            100.0,
            Some(1.0),
        );
        let calc = compute_yearly(&a, 2026, 250_000);
        assert_eq!(calc.depreciation_amount_cents, 250_000);
        assert_eq!(calc.book_value_after_cents, 0);
        assert!(calc.is_full_writeoff);
    }

    #[test]
    fn linear_july_acquisition_is_six_twelfths_first_year() {
        // 3.600,00 € / 3 Jahre = 1.200,00 € p.a.; Juli → 6/12 = 600,00 €.
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 7, 1),
            360_000,
            100.0,
            Some(3.0),
        );
        let calc = compute_yearly(&a, 2026, 360_000);
        assert_eq!(calc.months_in_year, 6);
        assert_eq!(calc.depreciation_amount_cents, 60_000);
        assert_eq!(calc.book_value_after_cents, 300_000);
        assert!(!calc.is_full_writeoff);
    }

    #[test]
    fn linear_full_year_after_acquisition() {
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 7, 1),
            360_000,
            100.0,
            Some(3.0),
        );
        let calc = compute_yearly(&a, 2027, 300_000);
        assert_eq!(calc.months_in_year, 12);
        assert_eq!(calc.depreciation_amount_cents, 120_000);
        assert_eq!(calc.book_value_after_cents, 180_000);
    }

    #[test]
    fn linear_keeps_one_cent_memo_value_in_last_year() {
        // R2-020 / §7 EStG + R 7.4 Abs. 5 EStR: bei laufender Nutzung bleibt
        // im letzten AfA-Jahr ein Erinnerungswert von 1 Cent stehen, damit die
        // Anlage im Anlagenverzeichnis sichtbar ist. Juli-Anschaffung, 3 Jahre
        // Nutzungsdauer → AfA verteilt sich über 4 Kalenderjahre, im letzten
        // wird auf 1 Cent (statt 0) abgeschrieben.
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 7, 1),
            360_000,
            100.0,
            Some(3.0),
        );
        let mut book =
            business_book_value_start_cents(a.acquisition_cost_cents, a.business_share_percent);
        let mut total = 0i64;
        let mut last_year = 2025;
        for year in 2026..=2035 {
            let calc = compute_yearly(&a, year, book);
            total += calc.depreciation_amount_cents;
            book = calc.book_value_after_cents;
            if !calc.is_noop() {
                last_year = year;
            }
            if book <= 1 {
                break;
            }
        }
        assert_eq!(book, 1, "Erinnerungswert 1 Cent bleibt im Bestand");
        assert_eq!(
            total, 359_999,
            "Summe der AfA = betrieblicher Wert − 1 Cent Erinnerungswert"
        );
        assert_eq!(
            last_year, 2029,
            "6+12+12+6 Monate → 4 Kalenderjahre 2026..2029"
        );
    }

    // R2-021 — Pro-rata-AfA im Veräußerungsjahr (R 7.4 Abs. 7 EStR).
    #[test]
    fn disposal_partial_linear_full_acquisition_year_disposal_in_followup() {
        // Anschaffung 2026-01-01, 5 Jahre Nutzungsdauer, Disposal 2027-08-15.
        // Folge-Jahr Disposal: 8 Monate (Jan–Aug) Pro-rata-AfA.
        // Annual = 360_000 / 5 = 72_000. 8/12 davon = 48_000.
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 1, 1),
            360_000,
            100.0,
            Some(5.0),
        );
        // Buchwert nach 1 vollem AfA-Jahr 2026 = 360_000 - 72_000 = 288_000.
        let calc = compute_disposal_year_partial(&a, 288_000, d(2027, 8, 15));
        assert_eq!(calc.months_in_year, 8);
        assert_eq!(calc.depreciation_amount_cents, 48_000);
        assert_eq!(calc.book_value_after_cents, 288_000 - 48_000);
        assert!(!calc.is_full_writeoff);
    }

    #[test]
    fn disposal_partial_linear_same_year_acquisition_and_disposal() {
        // Anschaffung 2026-03-10, Disposal 2026-09-30 — gleiche Jahr.
        // Erwartete Monate: März..September = 7 Monate (3..9 inkl.).
        // Annual = 360_000 / 3 = 120_000. 7/12 davon = 70_000.
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 3, 10),
            360_000,
            100.0,
            Some(3.0),
        );
        let bv_start =
            business_book_value_start_cents(a.acquisition_cost_cents, a.business_share_percent);
        let calc = compute_disposal_year_partial(&a, bv_start, d(2026, 9, 30));
        assert_eq!(calc.months_in_year, 7);
        assert_eq!(calc.depreciation_amount_cents, 70_000);
        assert_eq!(calc.book_value_after_cents, 360_000 - 70_000);
    }

    #[test]
    fn disposal_partial_caps_at_book_value() {
        // Annual größer als Restbuchwert — der Pro-rata-Betrag darf den
        // verbleibenden Buchwert nie überschreiten.
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 1, 1),
            360_000,
            100.0,
            Some(3.0),
        );
        // Vorgegaukelter Restbuchwert von nur noch 10_000.
        let calc = compute_disposal_year_partial(&a, 10_000, d(2030, 12, 1));
        assert!(calc.depreciation_amount_cents <= 10_000);
        assert_eq!(
            calc.book_value_after_cents,
            10_000 - calc.depreciation_amount_cents
        );
    }

    #[test]
    fn disposal_partial_gwg_in_acquisition_year_writes_off_fully() {
        let a = asset(
            DepreciationMethod::GwgSofort,
            d(2026, 6, 1),
            60_000,
            100.0,
            None,
        );
        let bv =
            business_book_value_start_cents(a.acquisition_cost_cents, a.business_share_percent);
        let calc = compute_disposal_year_partial(&a, bv, d(2026, 11, 30));
        assert!(calc.is_full_writeoff);
        assert_eq!(calc.book_value_after_cents, 0);
    }

    #[test]
    fn disposal_partial_gwg_in_followup_year_is_noop() {
        // GWG ist im Anschaffungsjahr komplett abgeschrieben — Restbuchwert
        // ist 0; auch im Disposal-Folgejahr gibt es keine zusätzliche AfA.
        let a = asset(
            DepreciationMethod::GwgSofort,
            d(2026, 1, 15),
            60_000,
            100.0,
            None,
        );
        let calc = compute_disposal_year_partial(&a, 0, d(2027, 4, 1));
        assert!(calc.is_noop());
        assert_eq!(calc.book_value_after_cents, 0);
    }

    #[test]
    fn disposal_partial_before_acquisition_is_noop() {
        // Defensiv: Disposal vor Anschaffung ist ein Datenfehler — nichts buchen.
        let a = asset(
            DepreciationMethod::Linear,
            d(2027, 6, 1),
            120_000,
            100.0,
            Some(3.0),
        );
        let calc = compute_disposal_year_partial(&a, 120_000, d(2026, 1, 1));
        assert!(calc.is_noop());
        assert_eq!(calc.book_value_after_cents, 120_000);
    }

    #[test]
    fn gwg_writes_off_to_zero_without_memo_value() {
        // GWG-Sofortabschreibung kennt keinen Erinnerungswert — die Anlage
        // gilt steuerlich sofort als verbraucht (§6 Abs. 2 EStG).
        let a = asset(
            DepreciationMethod::GwgSofort,
            d(2026, 3, 1),
            60_000,
            100.0,
            None,
        );
        let book =
            business_book_value_start_cents(a.acquisition_cost_cents, a.business_share_percent);
        let calc = compute_yearly(&a, 2026, book);
        assert_eq!(calc.book_value_after_cents, 0);
        assert_eq!(calc.depreciation_amount_cents, 60_000);
        assert!(calc.is_full_writeoff);
    }

    #[test]
    fn private_share_reduces_basis_proportionally() {
        // 80 % betrieblich von 1.000,00 € = 800,00 €; linear 4 Jahre, Jan → 200,00 €/J.
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 1, 1),
            100_000,
            80.0,
            Some(4.0),
        );
        let start =
            business_book_value_start_cents(a.acquisition_cost_cents, a.business_share_percent);
        assert_eq!(start, 80_000);
        let calc = compute_yearly(&a, 2026, start);
        assert_eq!(calc.months_in_year, 12);
        assert_eq!(calc.depreciation_amount_cents, 20_000);
        assert_eq!(calc.book_value_after_cents, 60_000);
    }

    #[test]
    fn nothing_to_depreciate_when_book_value_zero() {
        let a = asset(
            DepreciationMethod::Linear,
            d(2026, 1, 1),
            100_000,
            100.0,
            Some(5.0),
        );
        let calc = compute_yearly(&a, 2030, 0);
        assert!(calc.is_noop());
        assert_eq!(calc.book_value_after_cents, 0);
    }

    #[test]
    fn january_acquisition_uses_twelve_months() {
        assert_eq!(months_used_in_acquisition_year(d(2026, 1, 1)), 12);
        assert_eq!(months_used_in_acquisition_year(d(2026, 12, 31)), 1);
        assert_eq!(months_used_in_acquisition_year(d(2026, 7, 15)), 6);
    }

    fn booked(fiscal_year: i64, amount: i64, after: i64, locked: bool) -> BookedYear {
        BookedYear {
            fiscal_year,
            amount_cents: amount,
            book_value_after_cents: after,
            locked,
        }
    }

    // U4 (Warteliste 5): Vorschau des Abgangs — Nachholen der Vorjahre und
    // anteilige AfA im Abgangsjahr (wie `assets_dispose`, Lauf F2c).
    #[test]
    fn plan_disposal_catches_up_and_books_pro_rata() {
        let a = asset(
            DepreciationMethod::Linear,
            d(2025, 1, 1),
            120_000,
            100.0,
            Some(4.0),
        );
        let plan = plan_disposal(&a, 2025, 120_000, &[], &[], d(2026, 6, 30));
        assert!(plan.replaced.is_empty());
        assert_eq!(plan.catch_up, vec![(2025, 30_000)]);
        assert_eq!(plan.book_value_before_cents, 90_000);
        assert_eq!(plan.disposal_year.depreciation_amount_cents, 15_000);
        assert_eq!(plan.disposal_year.months_in_year, 6);
        assert_eq!(plan.residual_book_value_cents(), 75_000);
        assert_eq!(plan.closed_conflict_year, None);
        // Abgangsjahr abgeschlossen davor: 2025 wird nicht nachgeholt.
        let plan = plan_disposal(&a, 2025, 120_000, &[], &[2025], d(2026, 6, 30));
        assert!(plan.catch_up.is_empty());
        assert_eq!(plan.residual_book_value_cents(), 120_000 - 15_000);
    }

    // U4: Gebuchte offene AfA ab dem Abgangsjahr wird ersetzt; festgeschriebene
    // AfA bzw. ein später abgeschlossenes Jahr melden den Konflikt.
    #[test]
    fn plan_disposal_replaces_open_later_years_and_flags_conflicts() {
        let a = asset(
            DepreciationMethod::Linear,
            d(2024, 1, 1),
            120_000,
            100.0,
            Some(3.0),
        );
        let entries = [
            booked(2024, 40_000, 80_000, false),
            booked(2025, 40_000, 40_000, false),
            booked(2026, 40_000, 0, false),
        ];
        let plan = plan_disposal(&a, 2024, 0, &entries, &[], d(2025, 10, 15));
        assert_eq!(plan.replaced, vec![(2025, 40_000), (2026, 40_000)]);
        assert!(plan.catch_up.is_empty());
        assert_eq!(plan.book_value_before_cents, 80_000);
        assert_eq!(plan.disposal_year.depreciation_amount_cents, 33_333);
        assert_eq!(plan.residual_book_value_cents(), 46_667);
        assert_eq!(plan.closed_conflict_year, None);

        let mut locked = entries.clone();
        locked[2].locked = true;
        let plan = plan_disposal(&a, 2024, 0, &locked, &[], d(2025, 10, 15));
        assert_eq!(plan.closed_conflict_year, Some(2026));
        let plan = plan_disposal(&a, 2024, 0, &entries, &[2026], d(2025, 10, 15));
        assert_eq!(plan.closed_conflict_year, Some(2026));
        // Abgang im Jahr nach dem letzten Abschluss: kein Konflikt.
        let plan = plan_disposal(&a, 2024, 40_000, &entries[..2], &[2025], d(2026, 3, 1));
        assert_eq!(plan.closed_conflict_year, None);
        assert_eq!(plan.disposal_year.depreciation_amount_cents, 10_000);
        assert_eq!(plan.residual_book_value_cents(), 30_000);
    }
}
