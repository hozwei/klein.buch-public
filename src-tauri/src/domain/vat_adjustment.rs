//! Übersicht der Vorsteuerberichtigung nach § 15a UStG (ADR 0042 Nr. 14).
//! **Functional Core**, kein I/O: eine reine Rechnung je Anlage aus der
//! Vorsteuer, dem Beginn der Verwendung und der Steuerstatus-Historie.
//!
//! Klein.Buch bucht daraus nichts. Die Übersicht ist eine Hilfe für die
//! Umsatzsteuererklärung; die Berichtigung selbst gehört in die Erklärung
//! bzw. Voranmeldung und zur Steuerberatung.
//!
//! Regeln (Fundstellen am Gesetzestext geprüft, gesetze-im-internet.de,
//! 31.10.2026):
//! - **Berichtigungszeitraum** fünf Jahre ab der erstmaligen Verwendung
//!   (§ 15a Abs. 1 Satz 1 UStG), ersatzweise ab dem Anschaffungsdatum (ein
//!   eigenes Datum der ersten Verwendung kennt Klein.Buch nicht). Monatsgenau:
//!   Ein Zeitraum, der vor dem 16. eines Monats endet, lässt diesen Monat aus,
//!   sonst zählt er voll (§ 45 UStDV). Fünf Jahre ab dem Beginn enden am
//!   Vortag des entsprechenden Datums (§ 108 AO i. V. m. §§ 187 Abs. 2, 188
//!   Abs. 2 BGB): Beginn am 16.03. → Ende am 15.03., dieser März entfällt.
//!   Darum zählt der Monat des Beginns, wenn die Verwendung bis zum 16.
//!   beginnt, sonst beginnt der Zeitraum mit dem Folgemonat. So umfasst er
//!   immer 60 Monate (ADR 0042 Nr. 14).
//! - **Kürzere Verwendungsdauer** verkürzt den Zeitraum (§ 15a Abs. 5 Satz 2);
//!   maßgeblich ist die betriebsgewöhnliche Nutzungsdauer (UStAE 15a.3 Abs. 2).
//!   Welche Dauer gilt, entscheidet der Aufrufer ([`AssetFacts::use_months`]).
//! - **Jährlicher Anteil:** für jedes Kalenderjahr ein Fünftel (§ 15a Abs. 5
//!   Satz 1), monatsgenau: Vorsteuer × geänderte Monate ÷ Monate des Zeitraums.
//! - **Änderung der Verhältnisse:** Seit 2025 sind die Umsätze eines
//!   Kleinunternehmers steuerfrei (§ 19 Abs. 1 UStG n. F.) und schließen den
//!   Vorsteuerabzug aus (§ 15 Abs. 2 Nr. 1 UStG); ein Statuswechsel ändert damit
//!   die für den ursprünglichen Abzug maßgebenden Verhältnisse (§ 15a Abs. 1).
//!   Maßgeblich für den ursprünglichen Abzug ist der Status bei Leistungsbezug.
//!   Wechsel zur Regelbesteuerung: Berichtigung **zugunsten**; Rückkehr zu
//!   § 19: **zulasten**. Ein Monat zählt mit dem Status, der am 16. gilt (ein
//!   Wechsel bis zum 16. wirkt für den ganzen Monat, denn der alte Zustand
//!   endet dann vor dem 16.; analog § 45 UStDV).
//! - **Bagatellgrenzen** § 44 UStDV: Vorsteuer je Wirtschaftsgut bis 1.000 €
//!   → keine Berichtigung (Abs. 1); Änderung in einem Kalenderjahr um weniger
//!   als zehn Prozentpunkte → keine Berichtigung für dieses Jahr, außer der
//!   Betrag übersteigt 1.000 € (Abs. 2).
//! - **Zeitpunkt** § 44 Abs. 3 UStDV: Jahresbetrag bis 6.000 € erst in der
//!   Jahreserklärung, darüber in der Voranmeldung; bei Verkauf oder Entnahme im
//!   Zeitraum für das Jahr der Lieferung und die Folgejahre schon in der
//!   Voranmeldung der Lieferung.
//! - **Abgang** im Zeitraum (§ 15a Abs. 8, 9): Verkauf oder Entnahme zählen,
//!   als würde das Wirtschaftsgut bis zum Ende des Zeitraums unter den
//!   Verhältnissen der Lieferung weiter verwendet. Verkauf unter § 19 ist
//!   steuerfrei, unter Regelbesteuerung steuerpflichtig. Eine Entnahme ist nur
//!   steuerbar, wenn die Anlage zum Vorsteuerabzug berechtigt hat (§ 3 Abs. 1b
//!   Satz 2 UStG). Nach einer Verschrottung wird die Anlage nicht mehr verwendet;
//!   die Monate danach entfallen.

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use crate::domain::tax_status::TaxStatusHistory;

/// Berichtigungszeitraum in Monaten (§ 15a Abs. 1 Satz 1 UStG, fünf Jahre).
pub const PERIOD_MONTHS: u32 = 60;
/// § 44 Abs. 1 UStDV: Vorsteuer bis 1.000 € → keine Berichtigung.
pub const BAGATELLE_INPUT_TAX_CENTS: i64 = 100_000;
/// § 44 Abs. 2 UStDV: Änderung unter zehn Prozentpunkten …
pub const MIN_CHANGE_PERCENT_POINTS: u32 = 10;
/// … entfällt, solange der Jahresbetrag 1.000 € nicht übersteigt.
pub const BAGATELLE_YEAR_CENTS: i64 = 100_000;
/// § 44 Abs. 3 UStDV: Jahresbetrag bis 6.000 € erst in der Jahreserklärung.
pub const ANNUAL_RETURN_LIMIT_CENTS: i64 = 600_000;

/// Art eines Abgangs im Berichtigungszeitraum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisposalKind {
    /// Verkauf (§ 15a Abs. 8 UStG).
    Sale,
    /// Entnahme (Lieferung nach § 3 Abs. 1b UStG).
    Withdrawal,
    /// Verschrottung: keine Verwendung mehr.
    Scrap,
}

impl DisposalKind {
    /// Aus `assets.disposal_type` (`sale` | `given_away` | `scrap`).
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "sale" => Some(Self::Sale),
            "given_away" => Some(Self::Withdrawal),
            "scrap" => Some(Self::Scrap),
            _ => None,
        }
    }
}

/// Eingaben je Anlage.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetFacts {
    /// Beginn der Verwendung; ersatzweise das Anschaffungsdatum.
    pub first_use: NaiveDate,
    /// Vorsteuer auf die Anschaffungskosten (Cent, ≥ 0).
    pub input_tax_cents: i64,
    /// Status bei Leistungsbezug war § 19 (kein ursprünglicher Abzug).
    pub original_klein: bool,
    /// Kürzere Verwendungsdauer in Monaten (§ 15a Abs. 5 Satz 2); `None` oder
    /// ≥ 60 = fünf Jahre.
    pub use_months: Option<u32>,
    /// Abgang im oder nach dem Zeitraum.
    pub disposal: Option<(NaiveDate, DisposalKind)>,
}

/// Richtung der Berichtigung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Vorsteuer nachträglich abziehbar (Wechsel zur Regelbesteuerung).
    Zugunsten,
    /// Vorsteuer zurückzuzahlen (Rückkehr zu § 19).
    Zulasten,
}

/// Warum für ein Jahr keine Berichtigung anfällt, obwohl sich etwas ändert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Exemption {
    /// § 44 Abs. 1 UStDV: Vorsteuer bis 1.000 €.
    InputTaxBagatelle,
    /// § 44 Abs. 2 UStDV: unter zehn Prozentpunkten und höchstens 1.000 €.
    BelowTenPoints,
}

/// Wo die Berichtigung eines Jahres hingehört (§ 44 Abs. 3 UStDV).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Filing {
    /// Bis 6.000 €: Jahreserklärung des Jahres.
    AnnualReturn,
    /// Über 6.000 €: Voranmeldung.
    AdvanceReturn,
    /// Verkauf oder Entnahme im Zeitraum: Voranmeldung der Lieferung.
    AdvanceReturnOfDisposal,
}

/// Berichtigungszeitraum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    /// Erster Tag des ersten berücksichtigten Monats.
    pub from: NaiveDate,
    /// Letzter Tag des letzten berücksichtigten Monats.
    pub to: NaiveDate,
    pub months: u32,
}

/// Ein Kalenderjahr im Berichtigungszeitraum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearShare {
    pub year: i32,
    /// Monate des Zeitraums in diesem Jahr (ohne Monate nach einer Verschrottung).
    pub months: u32,
    /// Davon Monate unter Regelbesteuerung (bzw. wie steuerpflichtig verwendet).
    pub months_regular: u32,
    /// Monate mit anderen Verhältnissen als beim ursprünglichen Abzug.
    pub months_changed: u32,
    /// Änderung in Prozentpunkten (gerundet, für § 44 Abs. 2 genau geprüft).
    pub change_points: u32,
    /// Rechnerischer Betrag (Cent): positiv zugunsten, negativ zulasten.
    pub amount_cents: i64,
    /// Nach § 44 UStDV zu berichtigender Betrag (0 bei einer Ausnahme).
    pub adjustment_cents: i64,
    pub direction: Option<Direction>,
    pub exemption: Option<Exemption>,
    pub filing: Option<Filing>,
    /// Jahr nach dem laufenden: unter dem heute bekannten Status vorausgerechnet.
    pub projected: bool,
}

/// Ergebnis je Anlage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetAdjustment {
    pub period: Period,
    pub input_tax_cents: i64,
    pub original_klein: bool,
    /// § 44 Abs. 1 UStDV greift (Vorsteuer bis 1.000 €).
    pub input_tax_bagatelle: bool,
    pub years: Vec<YearShare>,
    /// Summe der zu berichtigenden Beträge über alle Jahre.
    pub total_adjustment_cents: i64,
}

impl AssetAdjustment {
    /// Zeile eines Kalenderjahres, wenn es im Zeitraum liegt.
    pub fn year(&self, year: i32) -> Option<&YearShare> {
        self.years.iter().find(|y| y.year == year)
    }

    /// Ändern sich die Verhältnisse irgendwann im Zeitraum?
    pub fn has_change(&self) -> bool {
        self.years.iter().any(|y| y.months_changed > 0)
    }
}

/// Monat als fortlaufende Zahl (Jahr × 12 + Monat − 1).
fn month_index(d: NaiveDate) -> i32 {
    d.year() * 12 + d.month0() as i32
}

fn month_start(idx: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(idx.div_euclid(12), idx.rem_euclid(12) as u32 + 1, 1)
        .expect("gültiger Monat")
}

fn month_end(idx: i32) -> NaiveDate {
    month_start(idx + 1).pred_opt().expect("gültiges Datum")
}

/// Erster berücksichtigter Monat zu einem Tag: derselbe Monat bis zum 16.
/// (der Zeitraum endet dann am 15. oder früher, der Endmonat entfällt), sonst
/// der Folgemonat (§ 45 UStDV, § 108 AO, §§ 187 Abs. 2, 188 Abs. 2 BGB).
fn first_month_from(d: NaiveDate) -> i32 {
    month_index(d) + i32::from(d.day() > 16)
}

/// Berichtigungszeitraum zum Beginn der Verwendung.
pub fn period(first_use: NaiveDate, use_months: Option<u32>) -> Period {
    let months = use_months
        .map_or(PERIOD_MONTHS, |m| m.min(PERIOD_MONTHS))
        .max(1);
    let start = first_month_from(first_use);
    let end = start + months as i32 - 1;
    Period {
        from: month_start(start),
        to: month_end(end),
        months,
    }
}

/// Rundet `num / den` kaufmännisch (halbe Cent vom Nullpunkt weg).
fn div_round(num: i128, den: i128) -> i64 {
    let q = (num.abs() * 2 + den) / (den * 2);
    (if num < 0 { -q } else { q }) as i64
}

/// Rechnet die Übersicht einer Anlage. `today` trennt bekannte von
/// vorausgerechneten Jahren.
pub fn compute(facts: &AssetFacts, hist: &TaxStatusHistory, today: NaiveDate) -> AssetAdjustment {
    let p = period(facts.first_use, facts.use_months);
    let start = month_index(p.from);
    let end = month_index(p.to);
    let input_tax = facts.input_tax_cents.max(0);
    let bagatelle = input_tax <= BAGATELLE_INPUT_TAX_CENTS;

    // Abgang: ab dem ersten Monat danach gelten die Verhältnisse der Lieferung
    // (§ 15a Abs. 9 UStG) bzw. entfällt die Verwendung (Verschrottung).
    let tail = facts.disposal.map(|(date, kind)| {
        let from = first_month_from(date);
        let regular_tail = match kind {
            DisposalKind::Sale => Some(!hist.is_klein_at(date)),
            DisposalKind::Withdrawal => Some(!hist.is_klein_at(date) && !facts.original_klein),
            DisposalKind::Scrap => None,
        };
        (from, date.year(), kind, regular_tail)
    });

    // Status eines Monats: der am 16. geltende (analog § 45 UStDV).
    let regular_in = |idx: i32| -> Option<bool> {
        if let Some((from, _, _, regular_tail)) = tail {
            if idx >= from {
                return regular_tail;
            }
        }
        let mid = month_start(idx).with_day(16).expect("16. eines Monats");
        Some(!hist.is_klein_at(mid))
    };

    let mut years = Vec::new();
    for year in p.from.year()..=p.to.year() {
        let first = start.max(year * 12);
        let last = end.min(year * 12 + 11);
        let (mut months, mut regular) = (0u32, 0u32);
        for idx in first..=last {
            if let Some(r) = regular_in(idx) {
                months += 1;
                regular += u32::from(r);
            }
        }
        if months == 0 {
            continue;
        }
        let changed = if facts.original_klein {
            regular
        } else {
            months - regular
        };
        let raw = div_round(
            i128::from(input_tax) * i128::from(changed),
            i128::from(p.months),
        );
        let amount = if facts.original_klein { raw } else { -raw };
        let direction = (changed > 0).then_some(if facts.original_klein {
            Direction::Zugunsten
        } else {
            Direction::Zulasten
        });
        // § 44 Abs. 2: „um weniger als zehn Prozentpunkte“ — exakt in Monaten.
        let below_ten = changed * 100 < MIN_CHANGE_PERCENT_POINTS * months;
        let exemption = if changed == 0 {
            None
        } else if bagatelle {
            Some(Exemption::InputTaxBagatelle)
        } else if below_ten && amount.abs() <= BAGATELLE_YEAR_CENTS {
            Some(Exemption::BelowTenPoints)
        } else {
            None
        };
        let adjustment = if exemption.is_some() { 0 } else { amount };
        let filing = (adjustment != 0).then(|| match tail {
            Some((_, disposal_year, DisposalKind::Sale | DisposalKind::Withdrawal, _))
                if year >= disposal_year =>
            {
                Filing::AdvanceReturnOfDisposal
            }
            _ if adjustment.abs() <= ANNUAL_RETURN_LIMIT_CENTS => Filing::AnnualReturn,
            _ => Filing::AdvanceReturn,
        });
        years.push(YearShare {
            year,
            months,
            months_regular: regular,
            months_changed: changed,
            change_points: (changed * 100 + months / 2) / months,
            amount_cents: amount,
            adjustment_cents: adjustment,
            direction,
            exemption,
            filing,
            projected: year > today.year(),
        });
    }
    let total = years.iter().map(|y| y.adjustment_cents).sum();
    AssetAdjustment {
        period: p,
        input_tax_cents: input_tax,
        original_klein: facts.original_klein,
        input_tax_bagatelle: bagatelle,
        years,
        total_adjustment_cents: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tax_status::{ChangeReason, TaxStatus, TaxStatusEntry};

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn entry(id: &str, from: NaiveDate, status: TaxStatus, reason: ChangeReason) -> TaxStatusEntry {
        TaxStatusEntry {
            id: id.into(),
            effective_from: from,
            status,
            reason,
            binding_until: None,
            revoked: false,
        }
    }

    /// § 19 bis zum Wechsel, danach Regelbesteuerung.
    fn klein_until(change: NaiveDate) -> TaxStatusHistory {
        TaxStatusHistory::new([
            entry(
                "a",
                crate::domain::tax_status::anfang(),
                TaxStatus::Kleinunternehmer,
                ChangeReason::Initial,
            ),
            entry(
                "b",
                change,
                TaxStatus::Regelbesteuerung,
                ChangeReason::Verzicht,
            ),
        ])
    }

    fn facts(first_use: NaiveDate, tax: i64, original_klein: bool) -> AssetFacts {
        AssetFacts {
            first_use,
            input_tax_cents: tax,
            original_klein,
            use_months: None,
            disposal: None,
        }
    }

    fn today() -> NaiveDate {
        d(2026, 10, 31)
    }

    /// Beispiel der Kartierung: Kauf 01.07.2024 unter § 19, Vorsteuer
    /// 1.900 €, Verzicht ab 01.01.2026 → 42/60 = 1.330 € zugunsten: je 380 €
    /// für 2026–2028, 190 € für 2029; 2024/2025 unverändert.
    #[test]
    fn switch_to_regular_taxation_is_in_favour() {
        let r = compute(
            &facts(d(2024, 7, 1), 190_000, true),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        assert_eq!(r.period.from, d(2024, 7, 1));
        assert_eq!(r.period.to, d(2029, 6, 30));
        assert_eq!(r.period.months, 60);
        let amounts: Vec<(i32, i64)> = r
            .years
            .iter()
            .map(|y| (y.year, y.adjustment_cents))
            .collect();
        assert_eq!(
            amounts,
            vec![
                (2024, 0),
                (2025, 0),
                (2026, 38_000),
                (2027, 38_000),
                (2028, 38_000),
                (2029, 19_000)
            ]
        );
        assert_eq!(r.total_adjustment_cents, 133_000);
        let y26 = r.year(2026).unwrap();
        assert_eq!(y26.direction, Some(Direction::Zugunsten));
        assert_eq!(
            (y26.months, y26.months_changed, y26.change_points),
            (12, 12, 100)
        );
        assert_eq!(y26.filing, Some(Filing::AnnualReturn));
        assert!(!y26.projected && r.year(2027).unwrap().projected);
        assert!(r.has_change());
        assert_eq!(r.year(2024).unwrap().direction, None);
    }

    /// Rückkehr zu § 19 nach Kauf unter Regelbesteuerung: zulasten.
    #[test]
    fn return_to_paragraph_19_is_a_charge() {
        let hist = TaxStatusHistory::new([
            entry(
                "a",
                crate::domain::tax_status::anfang(),
                TaxStatus::Regelbesteuerung,
                ChangeReason::Initial,
            ),
            entry(
                "b",
                d(2027, 1, 1),
                TaxStatus::Kleinunternehmer,
                ChangeReason::Rueckkehr,
            ),
        ]);
        let r = compute(&facts(d(2025, 1, 10), 600_000, false), &hist, today());
        let y27 = r.year(2027).unwrap();
        assert_eq!(y27.adjustment_cents, -120_000);
        assert_eq!(y27.direction, Some(Direction::Zulasten));
        assert_eq!(r.year(2026).unwrap().adjustment_cents, 0);
        // 2027–2029 je 12 Monate: 3 × 1.200 €.
        assert_eq!(r.total_adjustment_cents, -360_000);
    }

    /// § 45 UStDV: Beginn bis zum 16. zählt den Monat (Ende am 15. oder
    /// früher, der Endmonat entfällt), ab dem 17. erst den Folgemonat.
    #[test]
    fn month_rule_of_section_45() {
        let p = period(d(2024, 3, 15), None);
        assert_eq!(
            (p.from, p.to, p.months),
            (d(2024, 3, 1), d(2029, 2, 28), 60)
        );
        let p = period(d(2024, 3, 16), None);
        assert_eq!(
            (p.from, p.to, p.months),
            (d(2024, 3, 1), d(2029, 2, 28), 60)
        );
        let p = period(d(2024, 3, 17), None);
        assert_eq!(
            (p.from, p.to, p.months),
            (d(2024, 4, 1), d(2029, 3, 31), 60)
        );
        let p = period(d(2024, 12, 31), None);
        assert_eq!(p.from, d(2025, 1, 1));
    }

    /// Beginn 16.03.2024, Vorsteuer 6.000 €, Verzicht ab 01.01.2026: Zeitraum
    /// 03/2024–02/2029; 2029 zwei Monate = 200 €, gesamt 38/60 = 3.800 €.
    #[test]
    fn start_on_the_16th_counts_the_start_month() {
        let r = compute(
            &facts(d(2024, 3, 16), 600_000, true),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        assert_eq!(
            (r.period.from, r.period.to),
            (d(2024, 3, 1), d(2029, 2, 28))
        );
        let y29 = r.year(2029).unwrap();
        assert_eq!((y29.months, y29.adjustment_cents), (2, 20_000));
        assert_eq!(r.year(2026).unwrap().adjustment_cents, 120_000);
        assert_eq!(r.total_adjustment_cents, 380_000);
    }

    /// § 44 Abs. 1 UStDV: genau 1.000,00 € → keine Berichtigung; 1.000,01 € → doch.
    #[test]
    fn input_tax_bagatelle_at_exactly_1000_euro() {
        let hist = klein_until(d(2026, 1, 1));
        let r = compute(&facts(d(2025, 1, 1), 100_000, true), &hist, today());
        assert!(r.input_tax_bagatelle);
        let y = r.year(2026).unwrap();
        assert_eq!((y.amount_cents, y.adjustment_cents), (20_000, 0));
        assert_eq!(y.exemption, Some(Exemption::InputTaxBagatelle));
        assert_eq!(y.filing, None);
        assert_eq!(r.total_adjustment_cents, 0);

        let r = compute(&facts(d(2025, 1, 1), 100_001, true), &hist, today());
        assert!(!r.input_tax_bagatelle);
        assert_eq!(r.year(2026).unwrap().adjustment_cents, 20_000);
    }

    /// GWG (bis 800 € netto, Vorsteuer bis 152 €) fallen immer unter § 44 Abs. 1.
    #[test]
    fn gwg_always_falls_under_bagatelle() {
        let r = compute(
            &facts(d(2025, 6, 1), 15_200, true),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        assert!(r.input_tax_bagatelle);
        assert_eq!(r.total_adjustment_cents, 0);
    }

    /// Wechsel mitten im Jahr: Monate anteilig; bis zum 16. zählt der Monat
    /// schon neu. § 44 Abs. 2: ein Monat von zwölf (8,3 Punkte) entfällt bis
    /// 1.000 €, darüber nicht; zwei Monate (16,7 Punkte) immer.
    #[test]
    fn mid_year_change_and_ten_point_rule() {
        // Wechsel 15.12.2026: Dezember zählt schon unter Regelbesteuerung.
        let hist = klein_until(d(2026, 12, 15));
        let r = compute(&facts(d(2025, 1, 1), 300_000, true), &hist, today());
        let y = r.year(2026).unwrap();
        assert_eq!((y.months, y.months_changed, y.change_points), (12, 1, 8));
        assert_eq!(y.amount_cents, 5_000);
        assert_eq!(y.exemption, Some(Exemption::BelowTenPoints));
        assert_eq!(y.adjustment_cents, 0);
        // Wechsel am 16.12.: der alte Zustand endet am 15., Dezember zählt neu.
        let r = compute(
            &facts(d(2025, 1, 1), 300_000, true),
            &klein_until(d(2026, 12, 16)),
            today(),
        );
        assert_eq!(r.year(2026).unwrap().months_changed, 1);
        // Wechsel am 17.12.: Dezember noch § 19.
        let r = compute(
            &facts(d(2025, 1, 1), 300_000, true),
            &klein_until(d(2026, 12, 17)),
            today(),
        );
        assert_eq!(r.year(2026).unwrap().months_changed, 0);
        // Ein Monat, aber über 1.000 €: Abs. 2 Satz 2 → Berichtigung.
        let r = compute(&facts(d(2025, 1, 1), 6_000_100, true), &hist, today());
        let y = r.year(2026).unwrap();
        assert_eq!(y.amount_cents, 100_002);
        assert_eq!((y.exemption, y.adjustment_cents), (None, 100_002));
        // Genau 1.000 € bei einem Monat: entfällt noch.
        let r = compute(&facts(d(2025, 1, 1), 6_000_000, true), &hist, today());
        assert_eq!(
            r.year(2026).unwrap().exemption,
            Some(Exemption::BelowTenPoints)
        );
        // Wechsel 01.11.: zwei Monate = 16,7 Punkte → Berichtigung.
        let r = compute(
            &facts(d(2025, 1, 1), 300_000, true),
            &klein_until(d(2026, 11, 1)),
            today(),
        );
        let y = r.year(2026).unwrap();
        assert_eq!(
            (y.months_changed, y.change_points, y.adjustment_cents),
            (2, 17, 10_000)
        );
    }

    /// Kürzere Verwendungsdauer (§ 15a Abs. 5 Satz 2): drei Jahre → 36 Monate,
    /// je Jahr ein Drittel.
    #[test]
    fn shorter_use_period() {
        let mut f = facts(d(2025, 1, 1), 360_000, true);
        f.use_months = Some(36);
        let r = compute(&f, &klein_until(d(2026, 1, 1)), today());
        assert_eq!((r.period.months, r.period.to), (36, d(2027, 12, 31)));
        let amounts: Vec<i64> = r.years.iter().map(|y| y.adjustment_cents).collect();
        assert_eq!(amounts, vec![0, 120_000, 120_000]);
        // Längere Dauer als fünf Jahre bleibt bei 60 Monaten.
        f.use_months = Some(96);
        assert_eq!(
            compute(&f, &klein_until(d(2026, 1, 1)), today())
                .period
                .months,
            60
        );
    }

    /// Wechsel vor oder nach dem Zeitraum ändert nichts.
    #[test]
    fn change_outside_the_period_changes_nothing() {
        let r = compute(
            &facts(d(2019, 1, 1), 500_000, true),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        assert_eq!(r.period.to, d(2023, 12, 31));
        assert!(!r.has_change());
        assert_eq!(r.total_adjustment_cents, 0);
        let r = compute(
            &facts(d(2026, 2, 1), 500_000, false),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        assert!(!r.has_change());
    }

    /// Verkauf unter Regelbesteuerung im Zeitraum (§ 15a Abs. 8, 9): Rest des
    /// Zeitraums wie steuerpflichtig verwendet, alles in der Voranmeldung der
    /// Lieferung (§ 44 Abs. 3 Satz 2).
    #[test]
    fn sale_in_period_counts_the_rest_under_the_sale() {
        let mut f = facts(d(2024, 1, 1), 600_000, true);
        f.disposal = Some((d(2026, 6, 30), DisposalKind::Sale));
        let r = compute(&f, &klein_until(d(2026, 1, 1)), today());
        let y26 = r.year(2026).unwrap();
        assert_eq!(y26.adjustment_cents, 120_000);
        assert_eq!(y26.filing, Some(Filing::AdvanceReturnOfDisposal));
        assert_eq!(
            r.year(2028).unwrap().filing,
            Some(Filing::AdvanceReturnOfDisposal)
        );
        assert_eq!(r.total_adjustment_cents, 360_000);
        // Verkauf noch unter § 19 vor dem Wechsel: steuerfrei, keine Änderung.
        f.disposal = Some((d(2025, 6, 30), DisposalKind::Sale));
        let r = compute(&f, &klein_until(d(2026, 1, 1)), today());
        assert!(!r.has_change());
    }

    /// Entnahme einer unter § 19 gekauften Anlage ist nicht steuerbar (§ 3
    /// Abs. 1b Satz 2 UStG): keine Berichtigung für den Rest.
    #[test]
    fn withdrawal_of_asset_bought_under_paragraph_19() {
        let mut f = facts(d(2024, 1, 1), 600_000, true);
        f.disposal = Some((d(2026, 7, 10), DisposalKind::Withdrawal));
        let r = compute(&f, &klein_until(d(2026, 1, 1)), today());
        // Januar bis Juni 2026 unter Regelbesteuerung, ab Juli wie § 19.
        let y = r.year(2026).unwrap();
        assert_eq!((y.months_changed, y.adjustment_cents), (6, 60_000));
        assert_eq!(r.year(2027).unwrap().months_changed, 0);
    }

    /// Verschrottung: Monate danach entfallen.
    #[test]
    fn scrapping_ends_the_use() {
        let mut f = facts(d(2024, 1, 1), 600_000, true);
        f.disposal = Some((d(2026, 3, 20), DisposalKind::Scrap));
        let r = compute(&f, &klein_until(d(2026, 1, 1)), today());
        let y = r.year(2026).unwrap();
        assert_eq!(
            (y.months, y.months_changed, y.adjustment_cents),
            (3, 3, 30_000)
        );
        assert!(r.year(2027).is_none());
        assert_eq!(y.filing, Some(Filing::AnnualReturn));
    }

    /// § 44 Abs. 3: über 6.000 € je Jahr in die Voranmeldung.
    #[test]
    fn large_yearly_amount_goes_to_the_advance_return() {
        let r = compute(
            &facts(d(2025, 1, 1), 3_000_005, true),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        let y = r.year(2026).unwrap();
        assert_eq!(y.adjustment_cents, 600_001);
        assert_eq!(y.filing, Some(Filing::AdvanceReturn));
        let r = compute(
            &facts(d(2025, 1, 1), 3_000_000, true),
            &klein_until(d(2026, 1, 1)),
            today(),
        );
        assert_eq!(r.year(2026).unwrap().filing, Some(Filing::AnnualReturn));
    }

    /// Ohne Eintrag gilt § 19 durchgehend: nichts zu berichtigen.
    #[test]
    fn no_history_means_no_change() {
        let r = compute(
            &facts(d(2025, 1, 1), 500_000, true),
            &TaxStatusHistory::default(),
            today(),
        );
        assert!(!r.has_change());
    }

    #[test]
    fn disposal_kind_from_db() {
        assert_eq!(DisposalKind::from_db("sale"), Some(DisposalKind::Sale));
        assert_eq!(
            DisposalKind::from_db("given_away"),
            Some(DisposalKind::Withdrawal)
        );
        assert_eq!(DisposalKind::from_db("scrap"), Some(DisposalKind::Scrap));
        assert_eq!(DisposalKind::from_db("x"), None);
    }
}
