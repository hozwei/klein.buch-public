//! ELSTER-Ausfüllhilfe für die **Anlage EÜR** (Functional Core, Block 14a,
//! überarbeitet in Lauf F5b).
//!
//! Zielgruppe: der **selbst steuernde** §19-Kleinunternehmer, der seine EÜR im
//! Mein-ELSTER-Online-Formular abgibt. ELSTER hat **keinen** CSV-Import für die
//! Anlage EÜR (Direkt-Übermittlung ginge nur über die zertifizierte
//! ERiC-Schnittstelle). Dieses Modul erzeugt deshalb eine **Ausfüllhilfe**: jede
//! EÜR-Position wird der amtlichen Zeile **und Kennzahl** der Anlage EÜR des
//! jeweiligen Jahres zugeordnet. [`to_csv`] liefert dieselbe Zuordnung als CSV.
//!
//! ## Rechtsgrundlage / Quelle
//!
//! Amtliche Vordrucke (BMF-Schreiben nach § 60 Abs. 4 EStDV), am Vordruck
//! geprüft (Lauf F5b, 25.10.2026):
//! - **Anlage EÜR 2024** (BMF vom 02.09.2024) und **2025** (BMF vom
//!   29.08.2025): gleiche Zeilennummern.
//! - **Anlage EÜR 2026** (BMF vom 01.09.2026, GZ IV C 6 - S 2142/00024/010/001):
//!   neue Zeile 27 (Anlage LuF), „Waren“ (Kz 100) rückt nach dem Übertrag in
//!   Zeile 29, alle folgenden Betriebsausgaben-Zeilen um eins (Bezogene
//!   Fremdleistungen 30, Summe Betriebsausgaben 76).
//!
//! Die **Kennzahlen** (Kz) sind über die Jahre gleich geblieben; sie sind der
//! verlässliche Schlüssel. Für Jahre ohne geprüften Vordruck (vor 2024, nach
//! 2026) folgt die Zeilennummer dem nächstgelegenen Vordruck, und die
//! Ausfüllhilfe weist darauf hin.
//!
//! ## Leitlinie
//!
//! - Nur Positionen mit Betrag ≠ 0 (die Anleitung verlangt, nicht betroffene
//!   Zeilen nicht auszufüllen). Summen sind Kontrollsummen.
//! - §19-Einnahmen brutto in Zeile 12 (Kz 111); Einnahmen aus Rechnungen mit
//!   Regelbesteuerung netto in Zeile 15 (Kz 112), die vereinnahmte
//!   Umsatzsteuer in Zeile 17 (Kz 140), Einnahmen ohne Umsatzsteuer
//!   (Steuerart Z/AE/K/G/O) in Zeile 16 (Kz 103). Maßgeblich ist der beim
//!   Festschreiben eingefrorene Status der Rechnung (Anleitung zu Zeile 12:
//!   vor dem Wechsel ausgeführte Umsätze bleiben in ihrer Besteuerung).
//! - Betriebsausgaben: Kleinunternehmer brutto; bei Regelbesteuerung netto,
//!   die Vorsteuer in Zeile 57/58 (Kz 185).
//! - Storno-Erstattungen mindern die Einnahmen-Zeile; ein Überhang (Erstattung
//!   einer Vorjahres-Einnahme bei geringen Einnahmen im Jahr) ist eine
//!   Betriebsausgabe im Abflussjahr (§ 11 Abs. 2 EStG) und steht in „übrige
//!   Betriebsausgaben“ (Kz 183) — die Einnahmen-Zeilen kennen keinen negativen
//!   Wert.
//! - Die Kategorie→Zeile-Zuordnung ist ein **Vorschlag** und vor Abgabe vom
//!   Steuerberater zu prüfen (`docs/reference/elster-euer-formular-schema.md`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::euer::aggregate::EuerReport;
use crate::euer::detail::{ExpenseItem, IncomeItem, StornoItem, TaxShare};

// ============================================================================
// AfA-Aufteilung (von der Shell aus depreciation_entries × assets befüllt)
// ============================================================================

/// AfA des Geschäftsjahres, aufgeteilt nach Anlage-EÜR-Position.
///
/// - `beweglich_cents` → AfA auf bewegliche Wirtschaftsgüter (Kz 130),
/// - `gwg_cents` → geringwertige Wirtschaftsgüter (Kz 132, § 6 Abs. 2 EStG).
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
// Positionen, Kennzahlen und Zeilen je Vordruck
// ============================================================================

/// Eine Position der Anlage EÜR, die Klein.Buch befüllt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pos {
    Kleinunternehmer,
    UstPflichtig,
    UstFrei,
    VereinnahmteUst,
    Veraeusserung,
    SummeEinnahmen,
    Waren,
    Fremdleistungen,
    AfaBeweglich,
    Gwg,
    Restbuchwert,
    Miete,
    Telekommunikation,
    Reisekosten,
    Fortbildung,
    Beitraege,
    Edv,
    Arbeitsmittel,
    Werbung,
    Vorsteuer,
    Uebrige,
    KfzSonstige,
    SummeAusgaben,
    Gewinn,
}

/// Vordruck-Generation (Zeilennummern).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Anlage EÜR 2024 und 2025.
    Bis2025,
    /// Anlage EÜR 2026 (neue Zeile 27, „Waren“ in Zeile 29, danach +1).
    Ab2026,
}

impl Layout {
    /// Vordruck für ein Geschäftsjahr und ob dieser Vordruck für genau dieses
    /// Jahr geprüft ist (2024, 2025, 2026).
    pub fn for_year(year: i32) -> (Layout, bool) {
        match year {
            ..=2025 => (Layout::Bis2025, year >= 2024),
            2026 => (Layout::Ab2026, true),
            _ => (Layout::Ab2026, false),
        }
    }

    /// Jahr des Vordrucks, dem die Zeilennummern folgen.
    pub fn form_year(self) -> i32 {
        match self {
            Layout::Bis2025 => 2025,
            Layout::Ab2026 => 2026,
        }
    }
}

impl Pos {
    /// Amtliche Kennzahl (Kz) im Vordruck.
    pub fn kennzahl(self) -> u16 {
        match self {
            Pos::Kleinunternehmer => 111,
            Pos::UstPflichtig => 112,
            Pos::UstFrei => 103,
            Pos::VereinnahmteUst => 140,
            Pos::Veraeusserung => 102,
            Pos::SummeEinnahmen => 159,
            Pos::Waren => 100,
            Pos::Fremdleistungen => 110,
            Pos::AfaBeweglich => 130,
            Pos::Gwg => 132,
            Pos::Restbuchwert => 135,
            Pos::Miete => 150,
            Pos::Telekommunikation => 280,
            Pos::Reisekosten => 221,
            Pos::Fortbildung => 281,
            Pos::Beitraege => 223,
            Pos::Edv => 228,
            Pos::Arbeitsmittel => 229,
            Pos::Werbung => 224,
            Pos::Vorsteuer => 185,
            Pos::Uebrige => 183,
            Pos::KfzSonstige => 146,
            Pos::SummeAusgaben => 199,
            Pos::Gewinn => 219,
        }
    }

    /// Zeilennummer im Vordruck. Einnahmen (Zeilen 12–23) sind in beiden
    /// Generationen gleich; 2026 steht „Waren“ in Zeile 29 (statt 27) und jede
    /// weitere Betriebsausgaben-Zeile eins tiefer.
    pub fn zeile(self, layout: Layout) -> u16 {
        let base: u16 = match self {
            Pos::Kleinunternehmer => 12,
            Pos::UstFrei => 16,
            Pos::UstPflichtig => 15,
            Pos::VereinnahmteUst => 17,
            Pos::Veraeusserung => 19,
            Pos::SummeEinnahmen => 23,
            Pos::Waren => 27,
            Pos::Fremdleistungen => 29,
            Pos::AfaBeweglich => 33,
            Pos::Gwg => 36,
            Pos::Restbuchwert => 38,
            Pos::Miete => 39,
            Pos::Telekommunikation => 43,
            Pos::Reisekosten => 44,
            Pos::Fortbildung => 45,
            Pos::Beitraege => 49,
            Pos::Edv => 50,
            Pos::Arbeitsmittel => 51,
            Pos::Werbung => 54,
            Pos::Vorsteuer => 57,
            Pos::Uebrige => 60,
            Pos::KfzSonstige => 70,
            Pos::SummeAusgaben => 75,
            Pos::Gewinn => 97,
        };
        let is_income = matches!(
            self,
            Pos::Kleinunternehmer
                | Pos::UstFrei
                | Pos::UstPflichtig
                | Pos::VereinnahmteUst
                | Pos::Veraeusserung
                | Pos::SummeEinnahmen
        );
        match layout {
            // 2026: Zeile 27 ist neu (Anlage LuF), „Waren“ steht nach dem
            // Übertrag in Zeile 29; alle folgenden Zeilen liegen eins tiefer.
            Layout::Ab2026 if self == Pos::Waren => 29,
            Layout::Ab2026 if !is_income => base + 1,
            _ => base,
        }
    }

    /// Amtliche Bezeichnung (gekürzt auf den Kern des Vordrucktexts).
    pub fn label(self) -> &'static str {
        match self {
            Pos::Kleinunternehmer => {
                "Betriebseinnahmen als umsatzsteuerlicher Kleinunternehmer (§ 19 Abs. 1 UStG)"
            }
            Pos::UstPflichtig => "Umsatzsteuerpflichtige Betriebseinnahmen (netto)",
            Pos::UstFrei => {
                "Umsatzsteuerfreie oder nicht umsatzsteuerbare Betriebseinnahmen, 0 %-Umsätze, § 13b UStG"
            }
            Pos::VereinnahmteUst => "Vereinnahmte Umsatzsteuer",
            Pos::Veraeusserung => "Veräußerung oder Entnahme von Anlagevermögen",
            Pos::SummeEinnahmen => "Summe Betriebseinnahmen",
            Pos::Waren => "Waren, Rohstoffe und Hilfsstoffe einschließlich der Nebenkosten",
            Pos::Fremdleistungen => "Bezogene Fremdleistungen",
            Pos::AfaBeweglich => "AfA auf bewegliche Wirtschaftsgüter",
            Pos::Gwg => "Aufwendungen für geringwertige Wirtschaftsgüter (§ 6 Abs. 2 EStG)",
            Pos::Restbuchwert => "Restbuchwerte der ausgeschiedenen Anlagegüter",
            Pos::Miete => "Miete/Pacht für Geschäftsräume und betrieblich genutzte Grundstücke",
            Pos::Telekommunikation => "Aufwendungen für Telekommunikation (z. B. Telefon, Internet)",
            Pos::Reisekosten => "Übernachtungs- und Reisenebenkosten bei Geschäftsreisen",
            Pos::Fortbildung => "Fortbildungskosten (ohne Reisekosten)",
            Pos::Beitraege => "Beiträge, Gebühren, Abgaben und Versicherungen (ohne Gebäude und Kfz)",
            Pos::Edv => "Laufende EDV-Kosten",
            Pos::Arbeitsmittel => "Arbeitsmittel (z. B. Bürobedarf, Porto, Fachliteratur)",
            Pos::Werbung => "Werbekosten",
            Pos::Vorsteuer => "Gezahlte und nach § 15 UStG abziehbare Vorsteuerbeträge",
            Pos::Uebrige => "Übrige unbeschränkt abziehbare Betriebsausgaben",
            Pos::KfzSonstige => "Kfz-Kosten: sonstige tatsächliche Fahrtkosten ohne AfA und Zinsen",
            Pos::SummeAusgaben => "Summe Betriebsausgaben",
            Pos::Gewinn => "Steuerpflichtiger Gewinn/Verlust",
        }
    }
}

/// Ordnet eine interne Kosten-Kategorie der Position der Anlage EÜR zu.
///
/// **Vorschlag, vom Steuerberater zu prüfen.** Kategorien mit eindeutiger
/// amtlicher Zeile gehen dorthin; „übrige Betriebsausgaben“ ist nach der
/// Anleitung nur der Auffang „soweit nicht in den Zeilen 24 bis 59
/// berücksichtigt“ (hardware, fees, other).
pub fn category_pos(category: &str) -> Pos {
    match category {
        "goods" => Pos::Waren,
        "services" => Pos::Fremdleistungen,
        "rent" => Pos::Miete,
        "communications" => Pos::Telekommunikation,
        "travel" => Pos::Reisekosten,
        "training" => Pos::Fortbildung,
        "insurance" => Pos::Beitraege,
        "software" => Pos::Edv,
        "office" => Pos::Arbeitsmittel,
        "marketing" => Pos::Werbung,
        "vehicle" => Pos::KfzSonstige,
        _ => Pos::Uebrige,
    }
}

// ============================================================================
// Formular-Modell
// ============================================================================

/// Eine Zeile der Ausfüllhilfe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsterLine {
    /// Zeilennummer im Vordruck des Jahres.
    pub zeile: u16,
    /// Amtliche Kennzahl (Kz).
    pub kennzahl: Option<u16>,
    /// Amtliche Bezeichnung der Position.
    pub bezeichnung: String,
    /// Betrag in Cent (kann bei Gewinn/Verlust negativ sein).
    pub amount_cents: i64,
    /// `true` = Wert wird im ELSTER-Formular eingetragen; `false` =
    /// Kontrollsumme (von ELSTER berechnet, nur zum Abgleich).
    pub is_entry: bool,
}

impl ElsterLine {
    fn of(pos: Pos, layout: Layout, amount_cents: i64, is_entry: bool) -> Self {
        Self {
            zeile: pos.zeile(layout),
            kennzahl: Some(pos.kennzahl()),
            bezeichnung: pos.label().to_string(),
            amount_cents,
            is_entry,
        }
    }
}

/// Komplette Ausfüllhilfe eines Geschäftsjahres.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsterForm {
    pub fiscal_year: i32,
    /// §19-Status des Geschäftsjahres (bestimmt brutto/netto der Ausgaben).
    pub is_kleinunternehmer: bool,
    /// Jahr des Vordrucks, dem die Zeilennummern folgen.
    pub form_year: i32,
    /// `true` = Zeilennummern am Vordruck genau dieses Jahres geprüft.
    pub layout_verified: bool,
    pub lines: Vec<ElsterLine>,
    /// Summe der eingetragenen Betriebseinnahmen (= Zeile 23).
    pub income_total_cents: i64,
    /// Summe der eingetragenen Betriebsausgaben.
    pub expense_total_cents: i64,
    /// Gewinn/Verlust (Einnahmen − Ausgaben).
    pub surplus_cents: i64,
    /// Hinweise zur Ausfüllhilfe (Vordruck, Regelbesteuerung, Storno-Überhang).
    pub notes: Vec<String>,
}

/// Eingaben der Ausfüllhilfe. Die Einzelposten liefern die Aufteilung nach
/// Steuerstatus (Einnahmen) bzw. brutto/netto (Ausgaben); der Report liefert
/// AfA, Anlagenabgänge und den Gewinn zum Abgleich.
pub struct ElsterInputs<'a> {
    pub report: &'a EuerReport,
    pub afa: &'a AfaSplit,
    pub income: &'a [IncomeItem],
    pub storno: &'a [StornoItem],
    /// Bezahlte, nicht aktivierte Kosten des Jahres.
    pub expenses: &'a [ExpenseItem],
    /// §19-Status des Geschäftsjahres.
    pub is_kleinunternehmer: bool,
}

// ============================================================================
// Aufbau der Ausfüllhilfe
// ============================================================================

/// Summen der Einnahmen-Positionen (vor dem Abschneiden negativer Werte).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct IncomeBuckets {
    klein: i64,
    pflichtig_net: i64,
    frei: i64,
    ust: i64,
}

impl IncomeBuckets {
    fn add_shares(&mut self, is_klein: bool, amount: i64, shares: &[TaxShare], sign: i64) {
        if is_klein {
            self.klein += sign * amount;
            return;
        }
        if shares.is_empty() {
            self.frei += sign * amount;
            return;
        }
        for s in shares {
            if s.category_code == "S" && s.rate_basis_points > 0 {
                self.pflichtig_net += sign * s.net_cents;
                self.ust += sign * s.tax_cents;
            } else {
                self.frei += sign * s.gross_cents;
            }
        }
    }

    fn has_regel(&self) -> bool {
        self.pflichtig_net != 0 || self.frei != 0 || self.ust != 0
    }
}

fn fmt_eur_note(cents: i64) -> String {
    crate::euer::detail::eur_grouped(cents)
}

/// Baut die Ausfüllhilfe.
pub fn build_form(inp: &ElsterInputs<'_>) -> ElsterForm {
    let report = inp.report;
    let year = report.fiscal_year;
    let (layout, verified) = Layout::for_year(year);
    let mut notes: Vec<String> = Vec::new();
    if !verified {
        notes.push(format!(
            "Für {year} ist in Klein.Buch kein geprüfter Vordruck hinterlegt; die Zeilennummern folgen der Anlage EÜR {}. Maßgeblich sind die Kennzahlen (Kz) — bitte am Vordruck {year} abgleichen.",
            layout.form_year()
        ));
    }

    // ---- Betriebseinnahmen ----
    let mut b = IncomeBuckets::default();
    for i in inp.income {
        b.add_shares(i.is_kleinunternehmer, i.amount_cents, &i.shares, 1);
    }
    for s in inp.storno {
        b.add_shares(s.is_kleinunternehmer, s.refunded_cents, &s.shares, -1);
    }
    let regel_income = b.has_regel();
    // Negative Einnahmen-Zeilen gibt es nicht: Überhang = Rückzahlung von
    // Betriebseinnahmen → Betriebsausgabe im Abflussjahr (§ 11 Abs. 2 EStG).
    let mut overflow: i64 = 0;
    let mut clamp = |v: i64| -> i64 {
        if v < 0 {
            overflow += -v;
            0
        } else {
            v
        }
    };
    let klein = clamp(b.klein);
    let pflichtig = clamp(b.pflichtig_net);
    let frei = clamp(b.frei);
    let ust = clamp(b.ust);

    let mut lines: Vec<ElsterLine> = Vec::new();
    let mut income_total: i64 = 0;
    for (pos, amount) in [
        (Pos::Kleinunternehmer, klein),
        (Pos::UstPflichtig, pflichtig),
        (Pos::UstFrei, frei),
        (Pos::VereinnahmteUst, ust),
        (Pos::Veraeusserung, report.disposal_proceeds_cents),
    ] {
        if amount != 0 {
            lines.push(ElsterLine::of(pos, layout, amount, true));
            income_total += amount;
        }
    }
    lines.push(ElsterLine::of(
        Pos::SummeEinnahmen,
        layout,
        income_total,
        false,
    ));

    // ---- Betriebsausgaben ----
    let mut by_pos: BTreeMap<u16, (Pos, i64)> = BTreeMap::new();
    let mut add = |pos: Pos, amount: i64| {
        if amount != 0 {
            by_pos.entry(pos.zeile(layout)).or_insert((pos, 0)).1 += amount;
        }
    };
    let mut vorsteuer: i64 = 0;
    for e in inp.expenses {
        if inp.is_kleinunternehmer {
            add(category_pos(&e.category), e.gross_cents);
        } else {
            // Regelbesteuerung: netto in der Kategorie, Vorsteuer gesondert.
            add(category_pos(&e.category), e.gross_cents - e.tax_cents);
            vorsteuer += e.tax_cents;
        }
    }
    add(Pos::Vorsteuer, vorsteuer);
    add(Pos::AfaBeweglich, inp.afa.beweglich_cents);
    add(Pos::Gwg, inp.afa.gwg_cents);
    add(Pos::Restbuchwert, report.disposal_book_value_cents);
    add(Pos::Uebrige, overflow);
    let mut expense_total: i64 = 0;
    for (_, (pos, amount)) in by_pos {
        if amount != 0 {
            lines.push(ElsterLine::of(pos, layout, amount, true));
            expense_total += amount;
        }
    }
    lines.push(ElsterLine::of(
        Pos::SummeAusgaben,
        layout,
        expense_total,
        false,
    ));
    let surplus = income_total - expense_total;
    lines.push(ElsterLine::of(Pos::Gewinn, layout, surplus, false));

    if overflow != 0 {
        notes.push(format!(
            "Die Storno-Erstattungen übersteigen die Einnahmen des Jahres. Der Überhang von {} ist als Rückzahlung von Betriebseinnahmen in Zeile {} (übrige Betriebsausgaben, Kz {}) enthalten (§ 11 Abs. 2 EStG).",
            fmt_eur_note(overflow),
            Pos::Uebrige.zeile(layout),
            Pos::Uebrige.kennzahl()
        ));
    }
    if !inp.is_kleinunternehmer || regel_income {
        notes.push(regel_note(layout));
    }

    ElsterForm {
        fiscal_year: year,
        is_kleinunternehmer: inp.is_kleinunternehmer,
        form_year: layout.form_year(),
        layout_verified: verified,
        lines,
        income_total_cents: income_total,
        expense_total_cents: expense_total,
        surplus_cents: surplus,
        notes,
    }
}

/// Grenze der Ausfüllhilfe bei Regelbesteuerung (KB-0021): was Klein.Buch
/// aufteilt und was der Steuerberater ergänzen muss.
fn regel_note(layout: Layout) -> String {
    format!(
        "Regelbesteuerung: Einnahmen stehen netto in Zeile {} (Kz 112), die vereinnahmte Umsatzsteuer in Zeile {} (Kz 140), Ausgaben netto mit der Vorsteuer in Zeile {} (Kz 185). Nicht abgebildet sind die an das Finanzamt gezahlte und die vom Finanzamt erstattete Umsatzsteuer (Kz 186 und 141), die Umsatzsteuer auf Anlagenverkäufe und Entnahmen, nicht abziehbare Vorsteuer und die Vorsteuer aus Anlagenkäufen; die Umsatzsteuer-Voranmeldung erstellt Klein.Buch nicht. Bitte mit dem Steuerberater ergänzen.",
        Pos::UstPflichtig.zeile(layout),
        Pos::VereinnahmteUst.zeile(layout),
        Pos::Vorsteuer.zeile(layout),
    )
}

// ============================================================================
// CSV-Serialisierung
// ============================================================================

/// Formatiert Cent als deutschen Dezimalbetrag ohne Tausendertrenner
/// (z. B. `-123456` → `"-1234,56"`).
fn fmt_eur_de(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}{},{:02}", abs / 100, abs % 100)
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
/// Spalten: `Zeile;Kennzahl;Position;Betrag;Art`. UTF-8 **mit BOM** (öffnet in
/// deutschem Excel mit korrekten Umlauten); Dezimaltrenner Komma. Kommentar-
/// zeilen (`# …`) vor dem Spaltenkopf nennen Geschäftsjahr, Vordruck und die
/// Hinweise der Ausfüllhilfe. Die CSV ist eine Abtipphilfe und Archivdatei,
/// kein Importformat — ELSTER kennt für die Anlage EÜR keinen CSV-Import.
pub fn to_csv(form: &ElsterForm) -> String {
    to_csv_with_notes(form, &[])
}

/// Wie [`to_csv`], mit zusätzlichen Kommentarzeilen (`# …`) nach der
/// Titelzeile — z. B. der Hinweis auf eine Abweichung vom Jahresabschluss
/// (Lauf F2c). Zeilenumbrüche in einem Hinweis werden zu Leerzeichen.
pub fn to_csv_with_notes(form: &ElsterForm, notes: &[String]) -> String {
    let mut out = String::from("\u{FEFF}");
    out.push_str(&format!(
        "# Anlage EÜR {} — Ausfüllhilfe (Klein.Buch), Zeilen nach Vordruck {}. Vorschlag, vom Steuerberater zu prüfen.\n",
        form.fiscal_year, form.form_year
    ));
    for note in notes.iter().chain(form.notes.iter()) {
        let one_line: String = note
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        out.push_str(&format!("# {one_line}\n"));
    }
    out.push_str("Zeile;Kennzahl;Position;Betrag;Art\n");
    for l in &form.lines {
        let art = if l.is_entry {
            "Eingabe"
        } else {
            "Kontrollsumme"
        };
        out.push_str(&format!(
            "{};{};{};{};{}\n",
            l.zeile,
            l.kennzahl.map(|k| k.to_string()).unwrap_or_default(),
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

    fn report(
        year: i32,
        disposal_proceeds: i64,
        disposal_bv: i64,
        depreciation: i64,
    ) -> EuerReport {
        EuerReport {
            fiscal_year: year,
            invoice_income_cents: 0,
            storno_refunds_cents: 0,
            disposal_proceeds_cents: disposal_proceeds,
            total_income_cents: disposal_proceeds,
            expenses_by_category: Vec::<CategoryExpense>::new(),
            expenses_total_cents: 0,
            depreciation_total_cents: depreciation,
            disposal_book_value_cents: disposal_bv,
            total_expenses_cents: depreciation + disposal_bv,
            disposal_gain_loss_cents: disposal_proceeds - disposal_bv,
            surplus_cents: disposal_proceeds - depreciation - disposal_bv,
        }
    }

    fn klein_income(cents: i64) -> IncomeItem {
        IncomeItem {
            amount_cents: cents,
            is_kleinunternehmer: true,
            shares: vec![TaxShare {
                category_code: "E".into(),
                rate_basis_points: 0,
                gross_cents: cents,
                net_cents: cents,
                tax_cents: 0,
            }],
            ..Default::default()
        }
    }

    fn regel_income(net: i64, tax: i64) -> IncomeItem {
        IncomeItem {
            amount_cents: net + tax,
            is_kleinunternehmer: false,
            shares: vec![TaxShare {
                category_code: "S".into(),
                rate_basis_points: 1900,
                gross_cents: net + tax,
                net_cents: net,
                tax_cents: tax,
            }],
            ..Default::default()
        }
    }

    fn storno_klein(cents: i64) -> StornoItem {
        StornoItem {
            refunded_cents: cents,
            is_kleinunternehmer: true,
            ..Default::default()
        }
    }

    fn expense(cat: &str, gross: i64, tax: i64) -> ExpenseItem {
        ExpenseItem {
            category: cat.into(),
            gross_cents: gross,
            net_cents: gross - tax,
            tax_cents: tax,
            ..Default::default()
        }
    }

    fn form(
        r: &EuerReport,
        afa: &AfaSplit,
        income: &[IncomeItem],
        storno: &[StornoItem],
        expenses: &[ExpenseItem],
        klein: bool,
    ) -> ElsterForm {
        build_form(&ElsterInputs {
            report: r,
            afa,
            income,
            storno,
            expenses,
            is_kleinunternehmer: klein,
        })
    }

    fn entry(f: &ElsterForm, kz: u16) -> Option<&ElsterLine> {
        f.lines
            .iter()
            .find(|l| l.kennzahl == Some(kz) && l.is_entry)
    }

    #[test]
    fn euro_formatting_german_decimal() {
        assert_eq!(fmt_eur_de(0), "0,00");
        assert_eq!(fmt_eur_de(5), "0,05");
        assert_eq!(fmt_eur_de(123_456), "1234,56");
        assert_eq!(fmt_eur_de(-100_000), "-1000,00");
    }

    /// Zeilennummern am amtlichen Vordruck 2025 und 2026 belegt (Lauf F5b).
    #[test]
    fn line_numbers_follow_the_official_form_of_the_year() {
        let l25 = Layout::Bis2025;
        let l26 = Layout::Ab2026;
        // Einnahmen unverändert.
        for (pos, z) in [
            (Pos::Kleinunternehmer, 12),
            (Pos::UstPflichtig, 15),
            (Pos::UstFrei, 16),
            (Pos::VereinnahmteUst, 17),
            (Pos::Veraeusserung, 19),
            (Pos::SummeEinnahmen, 23),
        ] {
            assert_eq!(pos.zeile(l25), z);
            assert_eq!(pos.zeile(l26), z);
        }
        // 2024/2025.
        assert_eq!(Pos::Waren.zeile(l25), 27);
        assert_eq!(Pos::Fremdleistungen.zeile(l25), 29);
        assert_eq!(Pos::AfaBeweglich.zeile(l25), 33);
        assert_eq!(Pos::Gwg.zeile(l25), 36);
        assert_eq!(Pos::Restbuchwert.zeile(l25), 38);
        assert_eq!(Pos::Miete.zeile(l25), 39);
        assert_eq!(Pos::Telekommunikation.zeile(l25), 43);
        assert_eq!(Pos::Arbeitsmittel.zeile(l25), 51);
        assert_eq!(Pos::Vorsteuer.zeile(l25), 57);
        assert_eq!(Pos::Uebrige.zeile(l25), 60);
        assert_eq!(Pos::KfzSonstige.zeile(l25), 70);
        assert_eq!(Pos::SummeAusgaben.zeile(l25), 75);
        assert_eq!(Pos::Gewinn.zeile(l25), 97);
        // 2026: neue Zeile 27 (Anlage LuF); „Waren“ nach dem Übertrag in 29, danach +1.
        assert_eq!(Pos::Waren.zeile(l26), 29);
        assert_eq!(Pos::Fremdleistungen.zeile(l26), 30);
        assert_eq!(Pos::AfaBeweglich.zeile(l26), 34);
        assert_eq!(Pos::Gwg.zeile(l26), 37);
        assert_eq!(Pos::Restbuchwert.zeile(l26), 39);
        assert_eq!(Pos::Telekommunikation.zeile(l26), 44);
        assert_eq!(Pos::Vorsteuer.zeile(l26), 58);
        assert_eq!(Pos::Uebrige.zeile(l26), 61);
        assert_eq!(Pos::KfzSonstige.zeile(l26), 71);
        assert_eq!(Pos::SummeAusgaben.zeile(l26), 76);
        assert_eq!(Pos::Gewinn.zeile(l26), 98);
        // Kennzahlen sind jahresunabhängig.
        assert_eq!(Pos::Kleinunternehmer.kennzahl(), 111);
        assert_eq!(Pos::Uebrige.kennzahl(), 183);
    }

    #[test]
    fn layout_for_year_marks_unverified_years() {
        assert_eq!(Layout::for_year(2024), (Layout::Bis2025, true));
        assert_eq!(Layout::for_year(2025), (Layout::Bis2025, true));
        assert_eq!(Layout::for_year(2026), (Layout::Ab2026, true));
        assert_eq!(Layout::for_year(2027), (Layout::Ab2026, false));
        assert_eq!(Layout::for_year(2023), (Layout::Bis2025, false));
        let f = form(
            &report(2027, 0, 0, 0),
            &AfaSplit::default(),
            &[],
            &[],
            &[],
            true,
        );
        assert!(!f.layout_verified);
        assert!(f.notes.iter().any(|n| n.contains("Kennzahlen")));
    }

    #[test]
    fn category_mapping_uses_dedicated_lines() {
        assert_eq!(category_pos("goods"), Pos::Waren);
        assert_eq!(category_pos("services"), Pos::Fremdleistungen);
        assert_eq!(category_pos("rent"), Pos::Miete);
        assert_eq!(category_pos("communications"), Pos::Telekommunikation);
        assert_eq!(category_pos("travel"), Pos::Reisekosten);
        assert_eq!(category_pos("training"), Pos::Fortbildung);
        assert_eq!(category_pos("insurance"), Pos::Beitraege);
        assert_eq!(category_pos("software"), Pos::Edv);
        assert_eq!(category_pos("office"), Pos::Arbeitsmittel);
        assert_eq!(category_pos("marketing"), Pos::Werbung);
        assert_eq!(category_pos("vehicle"), Pos::KfzSonstige);
        // Auffang:
        assert_eq!(category_pos("hardware"), Pos::Uebrige);
        assert_eq!(category_pos("fees"), Pos::Uebrige);
        assert_eq!(category_pos("other"), Pos::Uebrige);
        assert_eq!(category_pos("unknown-future"), Pos::Uebrige);
    }

    #[test]
    fn kleinunternehmer_income_goes_to_zeile_12() {
        let r = report(2026, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[klein_income(500_000)],
            &[],
            &[],
            true,
        );
        let l = entry(&f, 111).unwrap();
        assert_eq!((l.zeile, l.amount_cents), (12, 500_000));
        assert!(entry(&f, 112).is_none());
        assert!(f.notes.is_empty());
    }

    /// KB-0021: Regelbesteuerung netto in Zeile 15, USt in Zeile 17, Ausgaben
    /// netto mit Vorsteuer in Zeile 57; der Gewinn bleibt derselbe.
    #[test]
    fn regelbesteuerung_splits_net_and_tax() {
        let r = report(2025, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[regel_income(100_000, 19_000)],
            &[],
            &[expense("office", 11_900, 1_900)],
            false,
        );
        assert!(entry(&f, 111).is_none());
        assert_eq!(entry(&f, 112).unwrap().amount_cents, 100_000);
        assert_eq!(entry(&f, 112).unwrap().zeile, 15);
        assert_eq!(entry(&f, 140).unwrap().amount_cents, 19_000);
        assert_eq!(entry(&f, 140).unwrap().zeile, 17);
        assert_eq!(entry(&f, 229).unwrap().amount_cents, 10_000); // netto
        assert_eq!(entry(&f, 185).unwrap().amount_cents, 1_900); // Vorsteuer
        assert_eq!(entry(&f, 185).unwrap().zeile, 57);
        assert_eq!(f.income_total_cents, 119_000);
        assert_eq!(f.expense_total_cents, 11_900);
        assert_eq!(f.surplus_cents, 107_100);
        assert!(f.notes.iter().any(|n| n.starts_with("Regelbesteuerung")));
    }

    /// Eine vor dem Wechsel zu §19 ausgestellte Rechnung bleibt in Zeile 15/17,
    /// auch wenn sie im §19-Jahr bezahlt wird (Anleitung zu Zeile 12).
    #[test]
    fn invoice_status_decides_the_income_line() {
        let r = report(2026, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[klein_income(30_000), regel_income(10_000, 1_900)],
            &[],
            &[],
            true,
        );
        assert_eq!(entry(&f, 111).unwrap().amount_cents, 30_000);
        assert_eq!(entry(&f, 112).unwrap().amount_cents, 10_000);
        assert_eq!(entry(&f, 140).unwrap().amount_cents, 1_900);
        assert!(f.notes.iter().any(|n| n.starts_with("Regelbesteuerung")));
    }

    #[test]
    fn zero_rated_regel_income_goes_to_zeile_16() {
        let r = report(2026, 0, 0, 0);
        let item = IncomeItem {
            amount_cents: 40_000,
            is_kleinunternehmer: false,
            shares: vec![TaxShare {
                category_code: "AE".into(),
                rate_basis_points: 0,
                gross_cents: 40_000,
                net_cents: 40_000,
                tax_cents: 0,
            }],
            ..Default::default()
        };
        let f = form(&r, &AfaSplit::default(), &[item], &[], &[], false);
        let l = entry(&f, 103).unwrap();
        assert_eq!((l.zeile, l.amount_cents), (16, 40_000));
    }

    #[test]
    fn storno_reduces_income_line() {
        let r = report(2026, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[klein_income(500_000)],
            &[storno_klein(120_000)],
            &[],
            true,
        );
        assert_eq!(entry(&f, 111).unwrap().amount_cents, 380_000);
    }

    /// R2-012 / Lauf F5b: Überhang aus Vorjahres-Stornos ist eine
    /// Betriebsausgabe (Kz 183), keine negative Einnahmen-Zeile und nicht die
    /// Zeile 18 (die ist die vom Finanzamt erstattete Umsatzsteuer).
    #[test]
    fn cross_year_storno_overflow_becomes_expense() {
        let r = report(2026, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[klein_income(30_000)],
            &[storno_klein(200_000)],
            &[],
            true,
        );
        assert!(entry(&f, 111).is_none(), "Zeile 12 nicht negativ");
        assert!(f.lines.iter().all(|l| l.zeile != 18));
        let u = entry(&f, 183).unwrap();
        assert_eq!((u.zeile, u.amount_cents), (61, 170_000));
        assert_eq!(f.surplus_cents, -170_000);
        assert!(f.notes.iter().any(|n| n.contains("Überhang")));
    }

    #[test]
    fn zero_entry_lines_are_omitted_but_sums_remain() {
        let r = report(2026, 0, 0, 0);
        let f = form(&r, &AfaSplit::default(), &[], &[], &[], true);
        assert!(f.lines.iter().all(|l| !l.is_entry));
        // Summe Einnahmen + Summe Ausgaben + Gewinn.
        assert_eq!(f.lines.len(), 3);
        assert_eq!(f.surplus_cents, 0);
    }

    #[test]
    fn afa_and_disposal_lines() {
        let r = report(2025, 30_000, 10_000, 80_000);
        let afa = AfaSplit {
            beweglich_cents: 50_000,
            gwg_cents: 30_000,
        };
        let f = form(&r, &afa, &[], &[], &[], true);
        assert_eq!(entry(&f, 130).unwrap().zeile, 33);
        assert_eq!(entry(&f, 130).unwrap().amount_cents, 50_000);
        assert_eq!(entry(&f, 132).unwrap().zeile, 36);
        assert_eq!(entry(&f, 102).unwrap().amount_cents, 30_000);
        assert_eq!(entry(&f, 135).unwrap().amount_cents, 10_000);
        assert_eq!(f.surplus_cents, r.surplus_cents);
    }

    #[test]
    fn same_position_categories_are_summed_and_sorted() {
        let r = report(2025, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[],
            &[],
            &[
                expense("vehicle", 3_000, 0),
                expense("hardware", 5_000, 0),
                expense("fees", 1_000, 0),
                expense("goods", 2_000, 0),
            ],
            true,
        );
        assert_eq!(entry(&f, 183).unwrap().amount_cents, 6_000);
        let zeilen: Vec<u16> = f
            .lines
            .iter()
            .filter(|l| l.is_entry)
            .map(|l| l.zeile)
            .collect();
        assert_eq!(zeilen, vec![27, 60, 70]);
    }

    #[test]
    fn csv_has_bom_kennzahl_column_and_notes() {
        let r = report(2027, 0, 0, 0);
        let f = form(
            &r,
            &AfaSplit::default(),
            &[klein_income(123_456)],
            &[],
            &[expense("goods", 20_000, 0)],
            true,
        );
        let csv = to_csv_with_notes(&f, &["ACHTUNG: Abweichung\nzweite Zeile".to_string()]);
        assert!(csv.starts_with('\u{FEFF}'));
        let lines: Vec<&str> = csv.trim_start_matches('\u{FEFF}').lines().collect();
        assert!(lines[0].starts_with("# Anlage EÜR 2027"));
        assert!(lines[0].contains("Vordruck 2026"));
        assert_eq!(lines[1], "# ACHTUNG: Abweichung zweite Zeile");
        assert!(lines[2].starts_with("# Für 2027"));
        assert_eq!(lines[3], "Zeile;Kennzahl;Position;Betrag;Art");
        assert!(csv.contains("12;111;"));
        assert!(csv.contains("1234,56;Eingabe"));
        assert!(csv.contains("29;100;"));
        assert!(csv.contains("Kontrollsumme"));
    }
}
