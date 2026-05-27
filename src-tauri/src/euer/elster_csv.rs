//! ELSTER-Ausfüllhilfe für die **Anlage EÜR** (Functional Core, Block 14a).
//!
//! Zielgruppe: der **selbst steuernde** §19-Kleinunternehmer, der seine EÜR im
//! Mein-ELSTER-Online-Formular abgibt. ELSTER hat **keinen** CSV-Import für die
//! Anlage EÜR (Direkt-Übermittlung ginge nur über die zertifizierte
//! ERiC-Schnittstelle, ein eigener späterer Block). Dieses Modul erzeugt deshalb
//! eine **Ausfüllhilfe**: jede EÜR-Position wird der offiziellen Zeile der Anlage
//! EÜR zugeordnet, damit der Betrag korrekt ins Formular übertragen werden kann.
//! Zusätzlich liefert [`to_csv`] dieselbe Zuordnung als CSV (Excel/Archiv).
//!
//! ## Rechtsgrundlage / Quelle
//!
//! Zeilenzuordnung nach der **Anleitung zur Anlage EÜR 2025** (ELSTER/BMF). Die
//! maßgeblichen Zeilen für den §19-Fall:
//! - **Zeile 12** — Betriebseinnahmen als umsatzsteuerlicher Kleinunternehmer
//!   (§ 19 Abs. 1 UStG); voller vereinnahmter Betrag, ohne die Beträge der
//!   Zeilen 18–22 (Veräußerung/Entnahmen).
//! - **Zeile 19** — Veräußerung oder Entnahme von Anlagevermögen (Verkaufserlös).
//! - **Zeile 23** — Summe der Betriebseinnahmen (von ELSTER berechnet).
//! - **Zeilen 27/29** — Waren / Bezogene Leistungen.
//! - **Zeile 33** — AfA auf bewegliche Wirtschaftsgüter.
//! - **Zeile 36** — geringwertige Wirtschaftsgüter (§ 6 Abs. 2 EStG, GWG-Sofort).
//! - **Zeile 38** — Restbuchwert ausgeschiedener Anlagegüter.
//! - **Zeile 60** — übrige unbeschränkt abziehbare Betriebsausgaben (Auffang).
//!
//! ## Leitlinie
//!
//! **Default = gesetzliches Minimum:** Es werden nur Positionen mit einem Betrag
//! ≠ 0 ausgegeben (die Anleitung verlangt ausdrücklich, nicht betroffene Zeilen
//! *nicht* — auch nicht mit 0,00 — zu füllen). Die Kategorie→Zeile-Zuordnung ist
//! ein **Vorschlag** und vor Abgabe vom Steuerberater zu prüfen (siehe
//! `docs/reference/elster-euer-formular-schema.md`).

use serde::{Deserialize, Serialize};

use crate::euer::aggregate::EuerReport;

// ============================================================================
// AfA-Aufteilung (von der Shell aus depreciation_entries × assets befüllt)
// ============================================================================

/// AfA des Geschäftsjahres, aufgeteilt nach Anlage-EÜR-Zeile.
///
/// - `beweglich_cents` → Zeile 33 (lineare AfA + Computer-Sonderregel auf
///   bewegliche/abnutzbare Wirtschaftsgüter),
/// - `gwg_cents` → Zeile 36 (GWG-Sofortabschreibung, § 6 Abs. 2 EStG).
///
/// Die Summe entspricht `EuerReport::depreciation_total_cents`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AfaSplit {
    pub beweglich_cents: i64,
    pub gwg_cents: i64,
}

impl AfaSplit {
    pub fn total_cents(&self) -> i64 {
        self.beweglich_cents + self.gwg_cents
    }
}

// ============================================================================
// Formular-Modell
// ============================================================================

/// Eine Zeile der Ausfüllhilfe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsterLine {
    /// Offizielle Zeilennummer der Anlage EÜR. `0` = keine feste Zeile
    /// (reine Kontrollsumme, die ELSTER selbst berechnet).
    pub zeile: u16,
    /// Amtliche Bezeichnung der Position.
    pub bezeichnung: String,
    /// Betrag in Cent (kann negativ sein, z. B. Verlust).
    pub amount_cents: i64,
    /// `true` = Wert wird im ELSTER-Formular eingetragen; `false` =
    /// Kontrollsumme (von ELSTER automatisch berechnet, nur zum Abgleich).
    pub is_entry: bool,
}

impl ElsterLine {
    fn entry(zeile: u16, amount_cents: i64) -> Self {
        Self {
            zeile,
            bezeichnung: zeile_label(zeile).to_string(),
            amount_cents,
            is_entry: true,
        }
    }

    fn sum(zeile: u16, bezeichnung: &str, amount_cents: i64) -> Self {
        Self {
            zeile,
            bezeichnung: bezeichnung.to_string(),
            amount_cents,
            is_entry: false,
        }
    }
}

/// Komplette Ausfüllhilfe eines Geschäftsjahres.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsterForm {
    pub fiscal_year: i32,
    pub is_kleinunternehmer: bool,
    pub lines: Vec<ElsterLine>,
    /// Summe Betriebseinnahmen (= Zeile 23).
    pub income_total_cents: i64,
    /// Summe Betriebsausgaben.
    pub expense_total_cents: i64,
    /// Steuerpflichtiger Gewinn/Verlust (Einnahmen − Ausgaben).
    pub surplus_cents: i64,
}

// ============================================================================
// Zeilen-Zuordnung
// ============================================================================

/// Ordnet eine interne Kosten-Kategorie der offiziellen Anlage-EÜR-Zeile zu.
///
/// **Vorschlag, vom Steuerberater zu prüfen.** Kategorien ohne dedizierte,
/// eindeutige Zeile landen bewusst in Zeile 60 (übrige unbeschränkt abziehbare
/// Betriebsausgaben) — der amtliche Auffangposten („soweit nicht in den Zeilen
/// 24 bis 59 berücksichtigt"). Das ist immer rechtlich zulässig.
pub fn category_zeile(category: &str) -> u16 {
    match category {
        "goods" => 27,    // Waren, Rohstoffe und Hilfsstoffe
        "services" => 29, // Bezogene Leistungen
        "rent" => 39,     // Raumkosten und sonstige Grundstücksaufwendungen
        "travel" => 44,   // Übernachtungs- und Reisenebenkosten
        "vehicle" => 68,  // Kraftfahrzeugkosten und andere Fahrtkosten
        // office, software, hardware, communications, insurance, training,
        // fees, marketing, other → Auffangposten:
        _ => 60,
    }
}

/// Amtliche Bezeichnung je Zeile (für die emittierten Positionen).
fn zeile_label(zeile: u16) -> &'static str {
    match zeile {
        12 => "Betriebseinnahmen als umsatzsteuerlicher Kleinunternehmer (§ 19 Abs. 1 UStG)",
        15 => "Umsatzsteuerpflichtige Betriebseinnahmen (netto)",
        19 => "Veräußerung oder Entnahme von Anlagevermögen",
        23 => "Summe der Betriebseinnahmen",
        27 => "Waren, Rohstoffe und Hilfsstoffe einschließlich der Nebenkosten",
        29 => "Bezogene Leistungen",
        33 => "Absetzung für Abnutzung (AfA) auf bewegliche Wirtschaftsgüter",
        36 => "Aufwendungen für geringwertige Wirtschaftsgüter (§ 6 Abs. 2 EStG)",
        38 => "Restbuchwert der ausgeschiedenen Anlagegüter",
        39 => "Raumkosten und sonstige Grundstücksaufwendungen",
        44 => "Übernachtungs- und Reisenebenkosten bei Geschäftsreisen",
        60 => "Übrige unbeschränkt abziehbare Betriebsausgaben",
        68 => "Kraftfahrzeugkosten und andere Fahrtkosten",
        _ => "",
    }
}

// ============================================================================
// Aufbau der Ausfüllhilfe
// ============================================================================

/// Baut die Ausfüllhilfe aus einem [`EuerReport`] und der AfA-Aufteilung.
///
/// `is_kleinunternehmer` steuert die Einnahmen-Zeile: §19 → Zeile 12 (Brutto),
/// Regelbesteuerung → Zeile 15 (mit Steuerberater-Caveat, da die USt-Aufteilung
/// auf der Einnahmenseite hier nicht modelliert ist).
///
/// Nur Eingabe-Positionen mit Betrag ≠ 0 werden ausgegeben (gesetzliches
/// Minimum); Summen-Zeilen erscheinen immer zum Abgleich.
pub fn build_form(report: &EuerReport, afa: &AfaSplit, is_kleinunternehmer: bool) -> ElsterForm {
    let mut lines: Vec<ElsterLine> = Vec::new();

    // ---- Betriebseinnahmen ----
    // R2-012 (Cross-Year-Storno-Schutz): Storno-Erstattungen werden im
    // Storno-Jahr gebucht, das Original kann aber aus einem Vorjahr stammen.
    // Wenn die Stornos die Einnahmen des Jahres übersteigen (typisch bei
    // einem großen Vorjahres-Storno und wenig Neugeschäft), darf Zeile 12/15
    // NICHT negativ ausgewiesen werden — die Anlage EÜR kennt keine negative
    // Einnahmen-Zeile. Stattdessen wird der Überhang als Berichtigungs-
    // Hinweis (Zeile 18 „Berichtigungen Vorjahre") separat genannt.
    let raw_income_line = report
        .invoice_income_cents
        .saturating_sub(report.storno_refunds_cents);
    let income_zeile = if is_kleinunternehmer { 12 } else { 15 };
    if raw_income_line >= 0 {
        if raw_income_line != 0 {
            lines.push(ElsterLine::entry(income_zeile, raw_income_line));
        }
    } else {
        // Same-year-Stornos verrechnen wir bis auf 0; den Überhang aus
        // Vorjahren weisen wir separat aus, der STB trägt ihn manuell in
        // Anlage EÜR Zeile 18 ein.
        if report.invoice_income_cents != 0 {
            lines.push(ElsterLine::entry(income_zeile, report.invoice_income_cents));
        }
        lines.push(ElsterLine::sum(
            18,
            "Berichtigung Vorjahres-Einnahmen (Storno > Einnahmen — manuell in Anlage EÜR Zeile 18 erfassen)",
            raw_income_line,
        ));
    }
    if report.disposal_proceeds_cents != 0 {
        lines.push(ElsterLine::entry(19, report.disposal_proceeds_cents));
    }
    lines.push(ElsterLine::sum(
        23,
        "Summe der Betriebseinnahmen",
        report.total_income_cents,
    ));

    // ---- Betriebsausgaben: Kosten je Zeile gruppiert ----
    let mut by_zeile: std::collections::BTreeMap<u16, i64> = std::collections::BTreeMap::new();
    for c in &report.expenses_by_category {
        *by_zeile.entry(category_zeile(&c.category)).or_insert(0) += c.amount_cents;
    }
    // AfA + Restbuchwert als eigene amtliche Zeilen.
    if afa.beweglich_cents != 0 {
        *by_zeile.entry(33).or_insert(0) += afa.beweglich_cents;
    }
    if afa.gwg_cents != 0 {
        *by_zeile.entry(36).or_insert(0) += afa.gwg_cents;
    }
    if report.disposal_book_value_cents != 0 {
        *by_zeile.entry(38).or_insert(0) += report.disposal_book_value_cents;
    }
    // Aufsteigend nach Zeilennummer (= Formular-Reihenfolge).
    for (zeile, amount) in by_zeile {
        if amount != 0 {
            lines.push(ElsterLine::entry(zeile, amount));
        }
    }

    // ---- Kontrollsummen ----
    lines.push(ElsterLine::sum(
        0,
        "Summe der Betriebsausgaben (von ELSTER berechnet)",
        report.total_expenses_cents,
    ));
    lines.push(ElsterLine::sum(
        0,
        "Steuerpflichtiger Gewinn / Verlust (von ELSTER berechnet)",
        report.surplus_cents,
    ));

    ElsterForm {
        fiscal_year: report.fiscal_year,
        is_kleinunternehmer,
        lines,
        income_total_cents: report.total_income_cents,
        expense_total_cents: report.total_expenses_cents,
        surplus_cents: report.surplus_cents,
    }
}

// ============================================================================
// CSV-Serialisierung
// ============================================================================

/// Formatiert Cent als deutschen Dezimalbetrag ohne Tausendertrenner
/// (z. B. `-123456` → `"-1234,56"`).
fn fmt_eur_de(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100).replace('.', ",")
}

/// Escapet ein CSV-Feld (Semikolon-getrennt): bei `;`, `"` oder Zeilenumbruch in
/// Anführungszeichen setzen und `"` verdoppeln.
fn csv_field(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Serialisiert die Ausfüllhilfe als semikolon-getrennte CSV.
///
/// Spalten: `Zeile;Position;Betrag;Art`. UTF-8 **mit BOM** (öffnet in deutschem
/// Excel mit korrekten Umlauten); Dezimaltrenner Komma. Eine Kommentarzeile
/// nennt Geschäftsjahr und Quelle.
pub fn to_csv(form: &ElsterForm) -> String {
    let mut out = String::from("\u{FEFF}");
    out.push_str(&format!(
        "# Anlage EÜR {} — Ausfüllhilfe (Klein.Buch). Vorschlag, vom Steuerberater zu prüfen.\n",
        form.fiscal_year
    ));
    out.push_str("Zeile;Position;Betrag;Art\n");
    for l in &form.lines {
        let zeile = if l.zeile == 0 {
            String::new()
        } else {
            l.zeile.to_string()
        };
        let art = if l.is_entry {
            "Eingabe"
        } else {
            "Kontrollsumme"
        };
        out.push_str(&format!(
            "{};{};{};{}\n",
            csv_field(&zeile),
            csv_field(&l.bezeichnung),
            csv_field(&fmt_eur_de(l.amount_cents)),
            art
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::euer::aggregate::{CategoryExpense, EuerReport};

    fn report_with(
        invoice: i64,
        storno: i64,
        disposal_proceeds: i64,
        disposal_bv: i64,
        depreciation: i64,
        cats: &[(&str, i64)],
    ) -> EuerReport {
        let expenses_by_category: Vec<CategoryExpense> = cats
            .iter()
            .map(|(c, a)| CategoryExpense {
                category: c.to_string(),
                amount_cents: *a,
            })
            .collect();
        let expenses_total: i64 = expenses_by_category.iter().map(|c| c.amount_cents).sum();
        let total_income = invoice - storno + disposal_proceeds;
        let total_expenses = expenses_total + depreciation + disposal_bv;
        EuerReport {
            fiscal_year: 2026,
            invoice_income_cents: invoice,
            storno_refunds_cents: storno,
            disposal_proceeds_cents: disposal_proceeds,
            total_income_cents: total_income,
            expenses_by_category,
            expenses_total_cents: expenses_total,
            depreciation_total_cents: depreciation,
            disposal_book_value_cents: disposal_bv,
            total_expenses_cents: total_expenses,
            disposal_gain_loss_cents: disposal_proceeds - disposal_bv,
            surplus_cents: total_income - total_expenses,
        }
    }

    fn line(form: &ElsterForm, zeile: u16) -> Option<&ElsterLine> {
        form.lines.iter().find(|l| l.zeile == zeile && l.is_entry)
    }

    #[test]
    fn euro_formatting_german_decimal() {
        assert_eq!(fmt_eur_de(0), "0,00");
        assert_eq!(fmt_eur_de(5), "0,05");
        assert_eq!(fmt_eur_de(123_456), "1234,56");
        assert_eq!(fmt_eur_de(-100_000), "-1000,00");
        assert_eq!(fmt_eur_de(7), "0,07");
    }

    #[test]
    fn category_mapping_known_and_fallback() {
        assert_eq!(category_zeile("goods"), 27);
        assert_eq!(category_zeile("services"), 29);
        assert_eq!(category_zeile("rent"), 39);
        assert_eq!(category_zeile("travel"), 44);
        assert_eq!(category_zeile("vehicle"), 68);
        // Auffang:
        assert_eq!(category_zeile("office"), 60);
        assert_eq!(category_zeile("software"), 60);
        assert_eq!(category_zeile("communications"), 60);
        assert_eq!(category_zeile("unknown-future"), 60);
    }

    #[test]
    fn kleinunternehmer_income_goes_to_zeile_12() {
        let r = report_with(500_000, 0, 0, 0, 0, &[]);
        let form = build_form(&r, &AfaSplit::default(), true);
        let l12 = line(&form, 12).unwrap();
        assert_eq!(l12.amount_cents, 500_000);
        assert!(l12.is_entry);
        assert!(line(&form, 15).is_none());
    }

    #[test]
    fn regelbesteuerung_income_goes_to_zeile_15() {
        let r = report_with(500_000, 0, 0, 0, 0, &[]);
        let form = build_form(&r, &AfaSplit::default(), false);
        assert!(line(&form, 12).is_none());
        assert_eq!(line(&form, 15).unwrap().amount_cents, 500_000);
    }

    #[test]
    fn storno_reduces_income_line() {
        let r = report_with(500_000, 120_000, 0, 0, 0, &[]);
        let form = build_form(&r, &AfaSplit::default(), true);
        assert_eq!(line(&form, 12).unwrap().amount_cents, 380_000);
    }

    /// R2-012 — Cross-Year-Storno: wenn die Storno-Erstattungen des Jahres
    /// die laufenden Einnahmen übersteigen (typisch bei großem Vorjahres-Storno
    /// und wenig Neugeschäft), wird Zeile 12 NICHT negativ ausgewiesen. Der
    /// Überhang erscheint als gesonderter Hinweis (Zeile 18 „Berichtigungen
    /// Vorjahre") in der Ausfüllhilfe.
    #[test]
    fn cross_year_storno_does_not_make_zeile_12_negative() {
        // Großer Vorjahres-Storno (200.000) bei wenig laufenden Einnahmen
        // (30.000): die Differenz -170.000 darf nicht in Zeile 12 landen.
        let r = report_with(30_000, 200_000, 0, 0, 0, &[]);
        let form = build_form(&r, &AfaSplit::default(), true);

        // Zeile 12 trägt nur die laufenden Einnahmen, nicht negativ.
        assert_eq!(line(&form, 12).unwrap().amount_cents, 30_000);

        // Der Überhang erscheint als Kontrollsumme (is_entry = false) auf
        // Zeile 18 mit negativem Betrag.
        let z18 = form
            .lines
            .iter()
            .find(|l| l.zeile == 18 && !l.is_entry)
            .expect("Berichtigungs-Hinweis Zeile 18 fehlt");
        assert_eq!(z18.amount_cents, -170_000);
    }

    #[test]
    fn zero_entry_lines_are_omitted_but_sums_remain() {
        // Keine Bewegungen → keine Eingabe-Zeilen, aber 3 Kontrollsummen.
        let r = report_with(0, 0, 0, 0, 0, &[]);
        let form = build_form(&r, &AfaSplit::default(), true);
        assert!(form.lines.iter().all(|l| !l.is_entry));
        // Summe Einnahmen (23) + Summe Ausgaben (0) + Gewinn (0).
        assert_eq!(form.lines.iter().filter(|l| !l.is_entry).count(), 3);
        assert_eq!(form.income_total_cents, 0);
        assert_eq!(form.surplus_cents, 0);
    }

    #[test]
    fn afa_split_maps_to_33_and_36() {
        let r = report_with(0, 0, 0, 0, 80_000, &[]);
        let afa = AfaSplit {
            beweglich_cents: 50_000,
            gwg_cents: 30_000,
        };
        let form = build_form(&r, &afa, true);
        assert_eq!(line(&form, 33).unwrap().amount_cents, 50_000);
        assert_eq!(line(&form, 36).unwrap().amount_cents, 30_000);
        assert_eq!(afa.total_cents(), r.depreciation_total_cents);
    }

    #[test]
    fn disposal_proceeds_19_and_residual_38() {
        let r = report_with(0, 0, 30_000, 10_000, 0, &[]);
        let form = build_form(&r, &AfaSplit::default(), true);
        assert_eq!(line(&form, 19).unwrap().amount_cents, 30_000);
        assert_eq!(line(&form, 38).unwrap().amount_cents, 10_000);
    }

    #[test]
    fn same_zeile_categories_are_summed() {
        // office + software + communications fallen alle auf Zeile 60.
        let r = report_with(
            0,
            0,
            0,
            0,
            0,
            &[
                ("office", 5_000),
                ("software", 30_000),
                ("communications", 1_000),
            ],
        );
        let form = build_form(&r, &AfaSplit::default(), true);
        assert_eq!(line(&form, 60).unwrap().amount_cents, 36_000);
        // genau eine Zeile-60-Eingabe (keine drei).
        assert_eq!(
            form.lines
                .iter()
                .filter(|l| l.zeile == 60 && l.is_entry)
                .count(),
            1
        );
    }

    #[test]
    fn expense_lines_sorted_by_zeile_ascending() {
        let r = report_with(
            0,
            0,
            0,
            0,
            0,
            &[("office", 1_000), ("goods", 2_000), ("vehicle", 3_000)],
        );
        let form = build_form(&r, &AfaSplit::default(), true);
        let entry_zeilen: Vec<u16> = form
            .lines
            .iter()
            .filter(|l| l.is_entry && l.zeile != 23)
            .map(|l| l.zeile)
            .collect();
        // 27 (goods) < 60 (office) < 68 (vehicle)
        assert_eq!(entry_zeilen, vec![27, 60, 68]);
    }

    #[test]
    fn csv_has_bom_header_and_german_decimals() {
        let r = report_with(123_456, 0, 0, 0, 0, &[("goods", 20_000)]);
        let form = build_form(&r, &AfaSplit::default(), true);
        let csv = to_csv(&form);
        assert!(csv.starts_with('\u{FEFF}'));
        assert!(csv.contains("Zeile;Position;Betrag;Art"));
        assert!(csv.contains("1234,56"));
        assert!(csv.contains("200,00"));
        assert!(csv.contains("Eingabe"));
        assert!(csv.contains("Kontrollsumme"));
    }

    #[test]
    fn full_example_income_minus_expenses() {
        let r = report_with(
            800_000,
            100_000,
            20_000,
            5_000,
            50_000,
            &[("goods", 60_000), ("office", 40_000)],
        );
        let afa = AfaSplit {
            beweglich_cents: 50_000,
            gwg_cents: 0,
        };
        let form = build_form(&r, &afa, true);
        // Einnahmen: 800.000 − 100.000 = 700.000 (Zeile 12) + 20.000 (Zeile 19)
        assert_eq!(line(&form, 12).unwrap().amount_cents, 700_000);
        assert_eq!(line(&form, 19).unwrap().amount_cents, 20_000);
        assert_eq!(form.income_total_cents, 720_000);
        // Ausgaben: goods 60.000 (27) + office 40.000 (60) + AfA 50.000 (33)
        //           + Restbuchwert 5.000 (38) = 155.000
        assert_eq!(line(&form, 27).unwrap().amount_cents, 60_000);
        assert_eq!(line(&form, 60).unwrap().amount_cents, 40_000);
        assert_eq!(line(&form, 33).unwrap().amount_cents, 50_000);
        assert_eq!(line(&form, 38).unwrap().amount_cents, 5_000);
        assert_eq!(form.expense_total_cents, 155_000);
        assert_eq!(form.surplus_cents, 565_000);
    }

    #[test]
    fn it_compiles() {}
}
