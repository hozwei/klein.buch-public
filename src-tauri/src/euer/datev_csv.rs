//! DATEV-Buchungsstapel (EXTF) — **Functional Core**, Block 14b (Schritt 1).
//!
//! Erzeugt aus den cash-wirksamen Bewegungen eines Geschäftsjahres einen
//! DATEV-Format-**Buchungsstapel** für die Steuerberater-Übergabe. Reine
//! Rechnung ohne I/O: nimmt die Einzelaufstellungs-Views (aus
//! [`crate::euer::detail`]) + den gewählten Kontenrahmen und liefert die fertigen
//! Bytes (CP1252) zurück.
//!
//! ## Format (verifiziert)
//!
//! - **Kopf (Vorlauf):** `"EXTF";700;21;"Buchungsstapel";12;…` — Datenkategorie 21,
//!   Formatversion 12, Versionsnummer 700. Berater-/Mandantennummer bleiben leer
//!   (der Steuerberater setzt sie beim Import).
//! - **Spaltenüberschrift + Buchungszeilen:** die führenden Standard-Spalten des
//!   Buchungsstapels (Umsatz, S/H-Kennzeichen, WKZ, Kurs, Basis-Umsatz, WKZ-Basis,
//!   Konto, Gegenkonto, BU-Schlüssel, Belegdatum, Belegfeld 1+2, Skonto,
//!   Buchungstext). DATEV liest positionsbasiert; eine Datei mit den führenden
//!   Spalten in dieser Reihenfolge ist importierbar.
//! - **Kodierung:** CP1252 (Windows-1252), CRLF-Zeilenenden, Dezimal-Komma.
//!
//! ## Kontenrahmen (SKR03 Default / SKR04)
//!
//! Die Konten sind ein **Vorschlag** und vor Verbuchung vom Steuerberater zu
//! prüfen (Doku: `docs/reference/datev-format.md`). Buchungslogik (alle als
//! „Konto im Soll", Gegenkonto im Haben, Umsatz positiv):
//! - Einnahme: Geldkonto (Soll) an §19-Erlöse (Haben)
//! - Ausgabe: Aufwandskonto (Soll) an Geldkonto (Haben)
//! - Storno-Erstattung: §19-Erlöse (Soll) an Geldkonto (Haben)
//! - Anlagen-Verkauf: Geldkonto (Soll) an Erlöse Anlagenverkauf (Haben)
//! - AfA: Abschreibungsaufwand (Soll) an Anlagekonto (Haben, direkte AfA)

use chrono::NaiveDate;

use crate::euer::detail::{
    AveeurItem, DisposalItem, ExpenseItem, IncomeItem, PrivateMovementItem, StornoItem,
};

// ============================================================================
// Kontenrahmen
// ============================================================================

/// Standard-Kontenrahmen. SKR03 ist Default für Einzelunternehmer/§19.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skr {
    Skr03,
    Skr04,
}

impl Skr {
    /// Aus einem (Settings-)String; alles außer „skr04" ⇒ SKR03 (sicherer Default).
    pub fn from_code(s: &str) -> Skr {
        if s.trim().eq_ignore_ascii_case("skr04") {
            Skr::Skr04
        } else {
            Skr::Skr03
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Skr::Skr03 => "SKR03",
            Skr::Skr04 => "SKR04",
        }
    }

    /// §19-Kleinunternehmer-Erlöse.
    pub fn revenue_kleinunternehmer(&self) -> u32 {
        match self {
            Skr::Skr03 => 8195,
            Skr::Skr04 => 4185,
        }
    }

    /// Erlöse aus dem Verkauf von Anlagevermögen.
    pub fn revenue_disposal(&self) -> u32 {
        match self {
            Skr::Skr03 => 8820,
            Skr::Skr04 => 4845,
        }
    }

    /// Standard-Bankkonto (Gegenkonto, wenn kein konkretes Geldkonto bekannt).
    pub fn bank(&self) -> u32 {
        match self {
            Skr::Skr03 => 1200,
            Skr::Skr04 => 1800,
        }
    }

    /// Kassenkonto.
    pub fn cash(&self) -> u32 {
        match self {
            Skr::Skr03 => 1000,
            Skr::Skr04 => 1600,
        }
    }

    /// Abschreibungsaufwand auf Sachanlagen (lineare AfA).
    pub fn afa_regular(&self) -> u32 {
        match self {
            Skr::Skr03 => 4830,
            Skr::Skr04 => 6220,
        }
    }

    /// Sofortabschreibung geringwertiger Wirtschaftsgüter (GWG).
    pub fn afa_gwg(&self) -> u32 {
        match self {
            Skr::Skr03 => 4855,
            Skr::Skr04 => 6260,
        }
    }

    /// Anlagekonto (Gegenkonto der AfA bei direkter Abschreibung) — Betriebs- und
    /// Geschäftsausstattung als generischer Default.
    pub fn asset_account(&self) -> u32 {
        match self {
            Skr::Skr03 => 420,
            Skr::Skr04 => 650,
        }
    }

    /// GWG-Anlagekonto (Gegenkonto der GWG-Sofortabschreibung).
    pub fn asset_account_gwg(&self) -> u32 {
        match self {
            Skr::Skr03 => 480,
            Skr::Skr04 => 670,
        }
    }

    /// Privatentnahme — Geld aus dem Betriebsvermögen in den privaten Bereich.
    /// SKR03 1800 „Privatentnahmen allgemein"; SKR04 2100 (R2-009).
    pub fn private_withdrawal(&self) -> u32 {
        match self {
            Skr::Skr03 => 1800,
            Skr::Skr04 => 2100,
        }
    }

    /// Privateinlage — privates Geld in den Betrieb. SKR03 1890 „Privateinlagen
    /// allgemein"; SKR04 2180 (R2-009).
    pub fn private_deposit(&self) -> u32 {
        match self {
            Skr::Skr03 => 1890,
            Skr::Skr04 => 2180,
        }
    }

    /// Aufwandskonto je Kosten-Kategorie. **Vorschlag, STB prüft.** Unbekanntes →
    /// „Sonstige betriebliche Aufwendungen".
    pub fn expense_account(&self, category: &str) -> u32 {
        match self {
            Skr::Skr03 => match category {
                "goods" => 3200,          // Wareneingang
                "services" => 3100,       // Fremdleistungen
                "office" => 4930,         // Bürobedarf
                "travel" => 4670,         // Reisekosten Unternehmer
                "communications" => 4920, // Telefon
                "vehicle" => 4530,        // Kfz-Kosten
                "rent" => 4210,           // Miete/Raumkosten
                "insurance" => 4360,      // Versicherungen
                "training" => 4945,       // Fortbildung
                "fees" => 4970,           // Nebenkosten des Geldverkehrs
                "marketing" => 4600,      // Werbekosten
                _ => 4980,                // hardware/software/other → sonstige betr. Aufw.
            },
            Skr::Skr04 => match category {
                "goods" => 5200,
                "services" => 5900,
                "office" => 6815,
                "travel" => 6670,
                "communications" => 6805,
                "vehicle" => 6530,
                "rent" => 6310,
                "insurance" => 6400,
                "training" => 6821,
                "fees" => 6855,
                "marketing" => 6600,
                _ => 6800,
            },
        }
    }
}

// ============================================================================
// Buchung
// ============================================================================

/// Eine DATEV-Buchungszeile (vereinfacht: Umsatz immer positiv, „Konto im Soll").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatevBooking {
    pub amount_cents: i64,
    /// Soll/Haben-Kennzeichen bezogen auf `konto` (hier durchgehend 'S').
    pub soll_haben: char,
    pub konto: u32,
    pub gegenkonto: u32,
    pub belegdatum: NaiveDate,
    /// Belegfeld 1 (Rechnungs-/Belegnummer), max. 36 Zeichen.
    pub belegfeld1: String,
    /// Belegfeld 2 (Bezug-Beleg, z. B. Original-Nummer bei Storno-Buchungen),
    /// max. 12 Zeichen. Leerstring = keine Bezugsnummer (R2-010).
    pub belegfeld2: String,
    /// Buchungstext, max. 60 Zeichen.
    pub buchungstext: String,
}

/// Kopf-Parameter für den Buchungsstapel.
#[derive(Debug, Clone)]
pub struct DatevHeader {
    pub fiscal_year: i32,
    pub skr: Skr,
    /// Zeitstempel „Erzeugt am" im Format `YYYYMMDDHHMMSSFFF` (von der Shell).
    pub generated_at: String,
}

// ============================================================================
// Buchungs-Aufbau (aus den Einzelaufstellungs-Views)
// ============================================================================

fn parse_iso(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&s.chars().take(10).collect::<String>(), "%Y-%m-%d").ok()
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Baut alle Buchungen eines Geschäftsjahres. Geldkonto-Gegenkonto ist hier das
/// Standard-Bankkonto (die Auflösung auf das konkrete Zahlungskonto ist als
/// R2-011 in der R6-Queue/Post-v1.0 vermerkt). AfA-Buchungen werden auf den
/// 31.12. datiert. Privatbewegungen (R2-009) und Storno-Belegfeld 2 (R2-010)
/// sind seit R2 enthalten.
#[allow(clippy::too_many_arguments)]
pub fn build_bookings(
    skr: Skr,
    fiscal_year: i32,
    income: &[IncomeItem],
    storno: &[StornoItem],
    expenses: &[ExpenseItem],
    disposals: &[DisposalItem],
    assets: &[AveeurItem],
    private_movements: &[PrivateMovementItem],
) -> Vec<DatevBooking> {
    let bank = skr.bank();
    let mut out = Vec::new();

    // Einnahmen: Bank (Soll) an §19-Erlöse (Haben).
    for i in income {
        if i.amount_cents <= 0 {
            continue;
        }
        if let Some(d) = parse_iso(&i.paid_date) {
            out.push(DatevBooking {
                amount_cents: i.amount_cents,
                soll_haben: 'S',
                konto: bank,
                gegenkonto: skr.revenue_kleinunternehmer(),
                belegdatum: d,
                belegfeld1: truncate(&i.invoice_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(format!("{} {}", i.customer, i.description).trim(), 60),
            });
        }
    }

    // Storno-Erstattung: §19-Erlöse (Soll) an Bank (Haben).
    // R2-010: Belegfeld 2 = Original-Belegnummer, damit der STB die Storno-
    // Buchung dem Original eindeutig zuordnen kann.
    for s in storno {
        if s.refunded_cents <= 0 {
            continue;
        }
        if let Some(d) = parse_iso(&s.storno_date) {
            out.push(DatevBooking {
                amount_cents: s.refunded_cents,
                soll_haben: 'S',
                konto: skr.revenue_kleinunternehmer(),
                gegenkonto: bank,
                belegdatum: d,
                belegfeld1: truncate(&s.storno_number, 36),
                belegfeld2: truncate(&s.original_number, 12),
                buchungstext: truncate(&format!("Storno zu {}", s.original_number), 60),
            });
        }
    }

    // Ausgaben: Aufwandskonto (Soll) an Bank (Haben).
    for e in expenses {
        if e.gross_cents <= 0 {
            continue;
        }
        if let Some(d) = parse_iso(&e.paid_date) {
            out.push(DatevBooking {
                amount_cents: e.gross_cents,
                soll_haben: 'S',
                konto: skr.expense_account(&e.category),
                gegenkonto: bank,
                belegdatum: d,
                belegfeld1: truncate(&e.expense_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(format!("{} {}", e.vendor, e.description).trim(), 60),
            });
        }
    }

    // Anlagen-Verkauf: Bank (Soll) an Erlöse Anlagenverkauf (Haben).
    for d in disposals {
        if d.proceeds_cents <= 0 {
            continue;
        }
        if let Some(date) = parse_iso(&d.disposal_date) {
            out.push(DatevBooking {
                amount_cents: d.proceeds_cents,
                soll_haben: 'S',
                konto: bank,
                gegenkonto: skr.revenue_disposal(),
                belegdatum: date,
                belegfeld1: truncate(&d.asset_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(&format!("Verkauf {}", d.label), 60),
            });
        }
    }

    // AfA (31.12.): Abschreibungsaufwand (Soll) an Anlagekonto (Haben).
    if let Some(year_end) = NaiveDate::from_ymd_opt(fiscal_year, 12, 31) {
        for a in assets {
            if a.afa_year_cents <= 0 {
                continue;
            }
            let is_gwg = a.depreciation_method == "gwg_sofort";
            out.push(DatevBooking {
                amount_cents: a.afa_year_cents,
                soll_haben: 'S',
                konto: if is_gwg {
                    skr.afa_gwg()
                } else {
                    skr.afa_regular()
                },
                gegenkonto: if is_gwg {
                    skr.asset_account_gwg()
                } else {
                    skr.asset_account()
                },
                belegdatum: year_end,
                belegfeld1: truncate(&a.asset_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(&format!("AfA {}", a.label), 60),
            });
        }
    }

    // Privatbewegungen (R2-009): EÜR-neutral, aber für die Bankabstimmung des
    // STB Pflicht. Entnahme = Privatkonto im Soll, Bank im Haben; Einlage
    // umgekehrt. Unbekannte movement_types werden übersprungen (Schema-
    // Garantie: 'entnahme' | 'einlage').
    for p in private_movements {
        if p.amount_cents <= 0 {
            continue;
        }
        let Some(date) = parse_iso(&p.movement_date) else {
            continue;
        };
        let (konto, gegenkonto, prefix) = match p.movement_type.as_str() {
            "entnahme" => (skr.private_withdrawal(), bank, "Privatentnahme"),
            "einlage" => (bank, skr.private_deposit(), "Privateinlage"),
            _ => continue,
        };
        out.push(DatevBooking {
            amount_cents: p.amount_cents,
            soll_haben: 'S',
            konto,
            gegenkonto,
            belegdatum: date,
            belegfeld1: truncate(&p.movement_number, 36),
            belegfeld2: String::new(),
            buchungstext: truncate(format!("{prefix} {}", p.description).trim_end(), 60),
        });
    }

    out
}

// ============================================================================
// Serialisierung (DATEV-Format, CP1252)
// ============================================================================

/// Standard-Spaltenüberschrift (führende Buchungsstapel-Spalten).
const COLUMN_HEADER: &str = "\"Umsatz (ohne Soll/Haben-Kz)\";\"Soll/Haben-Kennzeichen\";\"WKZ Umsatz\";\"Kurs\";\"Basis-Umsatz\";\"WKZ Basis-Umsatz\";\"Konto\";\"Gegenkonto (ohne BU-Schlüssel)\";\"BU-Schlüssel\";\"Belegdatum\";\"Belegfeld 1\";\"Belegfeld 2\";\"Skonto\";\"Buchungstext\"";

/// DATEV-Textfeld: in Anführungszeichen, eingebettete `"` verdoppelt.
fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Umsatz für DATEV: positiv, Komma-Dezimal, ohne Tausendertrenner (`123456` →
/// `1234,56`). Negative Beträge werden über S/H bzw. Konten abgebildet.
fn fmt_amount(cents: i64) -> String {
    let abs = cents.unsigned_abs();
    format!("{},{:02}", abs / 100, abs % 100)
}

fn fmt_belegdatum(d: NaiveDate) -> String {
    use chrono::Datelike;
    format!("{:02}{:02}", d.day(), d.month())
}

fn build_header_line(header: &DatevHeader) -> String {
    let year = header.fiscal_year;
    let wj = format!("{year}0101");
    let von = format!("{year}0101");
    let bis = format!("{year}1231");
    let bez = format!("EÜR {year}");
    let fields: Vec<String> = vec![
        q("EXTF"),                   // 1 Kennzeichen
        "700".into(),                // 2 Versionsnummer
        "21".into(),                 // 3 Datenkategorie (Buchungsstapel)
        q("Buchungsstapel"),         // 4 Formatname
        "12".into(),                 // 5 Formatversion
        header.generated_at.clone(), // 6 Erzeugt am (YYYYMMDDHHMMSSFFF)
        String::new(),               // 7 Importiert
        String::new(),               // 8 Herkunft
        String::new(),               // 9 Exportiert von
        String::new(),               // 10 Importiert von
        String::new(),               // 11 Berater (STB setzt beim Import)
        String::new(),               // 12 Mandant
        wj,                          // 13 WJ-Beginn
        "4".into(),                  // 14 Sachkontenlänge
        von,                         // 15 Datum von
        bis,                         // 16 Datum bis
        q(&bez),                     // 17 Bezeichnung
        String::new(),               // 18 Diktatkürzel
        "1".into(),                  // 19 Buchungstyp (1 = Finanzbuchführung)
        "0".into(),                  // 20 Rechnungslegungszweck
        "0".into(),                  // 21 Festschreibung (0 = nicht festgeschrieben)
        q("EUR"),                    // 22 WKZ
        String::new(),               // 23
        String::new(),               // 24
        String::new(),               // 25
        String::new(),               // 26
        String::new(),               // 27
        String::new(),               // 28
        String::new(),               // 29
        String::new(),               // 30
        String::new(),               // 31
    ];
    fields.join(";")
}

fn build_booking_line(b: &DatevBooking) -> String {
    let fields: Vec<String> = vec![
        fmt_amount(b.amount_cents),   // 1 Umsatz
        q(&b.soll_haben.to_string()), // 2 S/H
        q("EUR"),                     // 3 WKZ Umsatz
        String::new(),                // 4 Kurs
        String::new(),                // 5 Basis-Umsatz
        String::new(),                // 6 WKZ Basis-Umsatz
        // 7/8 Konto/Gegenkonto: Sachkontenlänge 4 (DATEV-Header Feld 14).
        // `{:04}` padded 3-stellige Anlagekonten (SKR03 420/480, SKR04 650/670)
        // mit führender 0. Personenkonten (=Sachkontenlänge+1, also 5-stellig)
        // werden vom Format nicht beschnitten (R2-001).
        format!("{:04}", b.konto),
        format!("{:04}", b.gegenkonto),
        String::new(),                // 9 BU-Schlüssel
        fmt_belegdatum(b.belegdatum), // 10 Belegdatum (TTMM)
        q(&b.belegfeld1),             // 11 Belegfeld 1
        if b.belegfeld2.is_empty() {
            String::new()
        } else {
            q(&b.belegfeld2)
        }, // 12 Belegfeld 2 (R2-010: Original-Nummer bei Storno)
        String::new(),                // 13 Skonto
        q(&b.buchungstext),           // 14 Buchungstext
    ];
    fields.join(";")
}

/// Serialisiert Kopf + Spaltenüberschrift + Buchungszeilen als DATEV-Format,
/// CP1252-kodiert mit CRLF.
pub fn to_datev(header: &DatevHeader, bookings: &[DatevBooking]) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&build_header_line(header));
    s.push_str("\r\n");
    s.push_str(COLUMN_HEADER);
    s.push_str("\r\n");
    for b in bookings {
        s.push_str(&build_booking_line(b));
        s.push_str("\r\n");
    }
    to_cp1252(&s)
}

/// Minimaler Windows-1252-Encoder: 0x00–0xFF direkt (Latin-1, deckt deutsche
/// Umlaute), einige typografische Sonderzeichen gemappt, alles andere → `?`.
fn to_cp1252(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        if code <= 0xFF {
            out.push(code as u8);
        } else {
            let mapped: u8 = match ch {
                '€' => 0x80,
                '‚' => 0x82,
                '„' => 0x84,
                '…' => 0x85,
                '‘' => 0x91,
                '’' => 0x92,
                '“' => 0x93,
                '”' => 0x94,
                '–' => 0x96,
                '—' => 0x97,
                _ => b'?',
            };
            out.push(mapped);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn skr_from_code_defaults_to_skr03() {
        assert_eq!(Skr::from_code("skr04"), Skr::Skr04);
        assert_eq!(Skr::from_code("SKR04"), Skr::Skr04);
        assert_eq!(Skr::from_code("skr03"), Skr::Skr03);
        assert_eq!(Skr::from_code("irgendwas"), Skr::Skr03);
        assert_eq!(Skr::from_code(""), Skr::Skr03);
    }

    #[test]
    fn account_mapping_skr03_and_skr04() {
        assert_eq!(Skr::Skr03.revenue_kleinunternehmer(), 8195);
        assert_eq!(Skr::Skr04.revenue_kleinunternehmer(), 4185);
        assert_eq!(Skr::Skr03.bank(), 1200);
        assert_eq!(Skr::Skr04.bank(), 1800);
        assert_eq!(Skr::Skr03.expense_account("goods"), 3200);
        assert_eq!(Skr::Skr04.expense_account("goods"), 5200);
        assert_eq!(Skr::Skr03.expense_account("office"), 4930);
        assert_eq!(Skr::Skr03.expense_account("unknown"), 4980); // Auffang
        assert_eq!(Skr::Skr04.expense_account("unknown"), 6800);
        assert_eq!(Skr::Skr03.afa_regular(), 4830);
        assert_eq!(Skr::Skr03.afa_gwg(), 4855);
    }

    #[test]
    fn fmt_amount_uses_comma_no_thousands() {
        assert_eq!(fmt_amount(0), "0,00");
        assert_eq!(fmt_amount(5), "0,05");
        assert_eq!(fmt_amount(123_456), "1234,56");
        assert_eq!(fmt_amount(-100_000), "1000,00"); // immer positiv
    }

    #[test]
    fn fmt_belegdatum_is_ttmm() {
        assert_eq!(fmt_belegdatum(d(2026, 3, 1)), "0103");
        assert_eq!(fmt_belegdatum(d(2026, 12, 31)), "3112");
    }

    #[test]
    fn header_line_has_extf_and_format_markers() {
        let h = DatevHeader {
            fiscal_year: 2026,
            skr: Skr::Skr03,
            generated_at: "20260521120000000".into(),
        };
        let line = build_header_line(&h);
        assert!(line.starts_with("\"EXTF\";700;21;\"Buchungsstapel\";12;"));
        assert!(line.contains("20260101")); // WJ-Beginn / Datum von
        assert!(line.contains("20261231")); // Datum bis
        assert!(line.contains("\"EÜR 2026\""));
        assert!(line.contains("\"EUR\""));
        assert_eq!(line.split(';').count(), 31);
    }

    #[test]
    fn booking_line_field_layout() {
        let b = DatevBooking {
            amount_cents: 100_000,
            soll_haben: 'S',
            konto: 1200,
            gegenkonto: 8195,
            belegdatum: d(2026, 3, 1),
            belegfeld1: "RE-2026-0001".into(),
            belegfeld2: String::new(),
            buchungstext: "ACME Beratung".into(),
        };
        let line = build_booking_line(&b);
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols.len(), 14);
        assert_eq!(cols[0], "1000,00"); // Umsatz
        assert_eq!(cols[1], "\"S\""); // S/H
        assert_eq!(cols[6], "1200"); // Konto (4-stellig, unverändert)
        assert_eq!(cols[7], "8195"); // Gegenkonto (4-stellig, unverändert)
        assert_eq!(cols[9], "0103"); // Belegdatum TTMM
        assert_eq!(cols[10], "\"RE-2026-0001\""); // Belegfeld 1
        assert_eq!(cols[11], ""); // Belegfeld 2 leer
        assert_eq!(cols[13], "\"ACME Beratung\""); // Buchungstext
    }

    /// R2-010 — Storno-Buchung schreibt `belegfeld2 = original_number`, damit
    /// der STB die Generalumkehr eindeutig zuordnen kann.
    #[test]
    fn storno_booking_carries_original_number_in_belegfeld2() {
        let b = DatevBooking {
            amount_cents: 100_000,
            soll_haben: 'S',
            konto: Skr::Skr03.revenue_kleinunternehmer(),
            gegenkonto: Skr::Skr03.bank(),
            belegdatum: d(2027, 2, 14),
            belegfeld1: "ST-2027-0001".into(),
            belegfeld2: "RE-2026-0042".into(),
            buchungstext: "Storno zu RE-2026-0042".into(),
        };
        let line = build_booking_line(&b);
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols[10], "\"ST-2027-0001\"");
        assert_eq!(
            cols[11], "\"RE-2026-0042\"",
            "Original-Belegnummer in Belegfeld 2"
        );
    }

    /// R2-001 — Anlage-Sachkonten 420/480 (SKR03) bzw. 650/670 (SKR04)
    /// werden gegen die im Header deklarierte Sachkontenlänge 4 mit führender
    /// Null gepaddet ausgegeben (`0420`/`0480`/`0650`/`0670`), sonst routen
    /// strenge DATEV-Importer das auf Personenkonten.
    #[test]
    fn booking_line_pads_three_digit_asset_accounts_to_four_digits() {
        let afa = DatevBooking {
            amount_cents: 50_000,
            soll_haben: 'S',
            konto: Skr::Skr03.afa_regular(), // 4830 → bleibt 4-stellig
            gegenkonto: Skr::Skr03.asset_account(), // 420 → soll 0420 werden
            belegdatum: d(2026, 12, 31),
            belegfeld1: "AN-2026-0001".into(),
            belegfeld2: String::new(),
            buchungstext: "AfA Notebook".into(),
        };
        let line = build_booking_line(&afa);
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols[6], "4830", "AfA-Konto 4-stellig unverändert");
        assert_eq!(cols[7], "0420", "Anlage-Konto 420 → 0420 gepaddet");

        let gwg = DatevBooking {
            amount_cents: 12_000,
            soll_haben: 'S',
            konto: Skr::Skr03.afa_gwg(),                // 4855
            gegenkonto: Skr::Skr03.asset_account_gwg(), // 480 → 0480
            belegdatum: d(2026, 12, 31),
            belegfeld1: "AN-2026-0002".into(),
            belegfeld2: String::new(),
            buchungstext: "GWG Maus".into(),
        };
        let line = build_booking_line(&gwg);
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols[6], "4855");
        assert_eq!(cols[7], "0480", "GWG-Anlage-Konto 480 → 0480 gepaddet");

        // SKR04-Pendant
        let skr04_afa = DatevBooking {
            amount_cents: 25_000,
            soll_haben: 'S',
            konto: Skr::Skr04.afa_regular(),
            gegenkonto: Skr::Skr04.asset_account(), // 650 → 0650
            belegdatum: d(2026, 12, 31),
            belegfeld1: "AN-2026-0003".into(),
            belegfeld2: String::new(),
            buchungstext: "AfA Möbel".into(),
        };
        let line = build_booking_line(&skr04_afa);
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols[7], "0650", "SKR04 Anlage 650 → 0650 gepaddet");
    }

    #[test]
    fn cp1252_encodes_umlauts_and_euro() {
        let bytes = to_cp1252("Müller€?");
        // M, ü(0xFC), l, l, e, r, €(0x80), ?(0x3F)
        assert_eq!(bytes[0], b'M');
        assert_eq!(bytes[1], 0xFC);
        assert_eq!(bytes[6], 0x80);
        assert_eq!(bytes[7], b'?');
    }

    #[test]
    fn cp1252_unknown_char_becomes_questionmark() {
        let bytes = to_cp1252("\u{2212}"); // Minuszeichen − ist nicht in CP1252
        assert_eq!(bytes, vec![b'?']);
    }

    fn income(date: &str, nr: &str, cust: &str, cents: i64) -> IncomeItem {
        IncomeItem {
            paid_date: date.into(),
            invoice_number: nr.into(),
            customer: cust.into(),
            description: "Leistung".into(),
            amount_cents: cents,
        }
    }

    fn expense(date: &str, nr: &str, cat: &str, cents: i64) -> ExpenseItem {
        ExpenseItem {
            paid_date: date.into(),
            expense_number: nr.into(),
            vendor: "Lieferant".into(),
            category: cat.into(),
            description: "Kosten".into(),
            gross_cents: cents,
        }
    }

    fn aveeur(method: &str, afa: i64) -> AveeurItem {
        AveeurItem {
            asset_number: "AV-2026-0001".into(),
            label: "Notebook".into(),
            acquisition_date: "2026-01-10".into(),
            acquisition_cost_cents: 120_000,
            depreciation_method: method.into(),
            useful_life_years: Some(3.0),
            business_share_percent: 100.0,
            afa_year_cents: afa,
            book_value_start_cents: 120_000,
            book_value_end_cents: 80_000,
            disposed_in_year: false,
            disposal_date: None,
            disposal_proceeds_cents: None,
        }
    }

    #[test]
    fn build_bookings_covers_all_movement_types() {
        let income = vec![income("2026-03-01", "RE-2026-0001", "ACME", 100_000)];
        let storno = vec![StornoItem {
            storno_date: "2026-04-01".into(),
            storno_number: "ST-2026-0001".into(),
            original_number: "RE-2026-0009".into(),
            refunded_cents: 20_000,
        }];
        let expenses = vec![expense("2026-05-01", "KO-2026-0001", "goods", 30_000)];
        let disposals = vec![DisposalItem {
            disposal_date: "2026-06-01".into(),
            asset_number: "AV-2026-0002".into(),
            label: "Drucker".into(),
            proceeds_cents: 5_000,
            residual_book_value_cents: 1_000,
            gain_loss_cents: 4_000,
        }];
        let assets = vec![aveeur("linear", 40_000), aveeur("gwg_sofort", 80_000)];

        let private_movements = vec![
            PrivateMovementItem {
                movement_date: "2026-08-15".into(),
                movement_number: "PR-2026-0001".into(),
                movement_type: "entnahme".into(),
                description: "Lebenshaltung".into(),
                amount_cents: 50_000,
            },
            PrivateMovementItem {
                movement_date: "2026-09-01".into(),
                movement_number: "PR-2026-0002".into(),
                movement_type: "einlage".into(),
                description: "Sparbuch".into(),
                amount_cents: 100_000,
            },
        ];

        let bookings = build_bookings(
            Skr::Skr03,
            2026,
            &income,
            &storno,
            &expenses,
            &disposals,
            &assets,
            &private_movements,
        );
        // 1 Einnahme + 1 Storno + 1 Ausgabe + 1 Verkauf + 2 AfA + 2 Privat = 8
        assert_eq!(bookings.len(), 8);

        // Einnahme: Bank an §19-Erlöse
        assert_eq!(bookings[0].konto, 1200);
        assert_eq!(bookings[0].gegenkonto, 8195);
        // Storno: §19-Erlöse an Bank, Original-Nummer im Belegfeld 2
        assert_eq!(bookings[1].konto, 8195);
        assert_eq!(bookings[1].gegenkonto, 1200);
        assert_eq!(bookings[1].belegfeld2, "RE-2026-0009");
        // Ausgabe goods: Wareneingang an Bank
        assert_eq!(bookings[2].konto, 3200);
        assert_eq!(bookings[2].gegenkonto, 1200);
        // Verkauf: Bank an Erlöse Anlagenverkauf
        assert_eq!(bookings[3].konto, 1200);
        assert_eq!(bookings[3].gegenkonto, 8820);
        // AfA linear: 4830 an Anlagekonto 420
        assert_eq!(bookings[4].konto, 4830);
        assert_eq!(bookings[4].gegenkonto, 420);
        assert_eq!(bookings[4].belegdatum, d(2026, 12, 31));
        // AfA GWG: 4855 an GWG-Anlagekonto 480
        assert_eq!(bookings[5].konto, 4855);
        assert_eq!(bookings[5].gegenkonto, 480);
        // Privatentnahme: 1800 (Soll) an Bank
        assert_eq!(bookings[6].konto, 1800);
        assert_eq!(bookings[6].gegenkonto, 1200);
        assert_eq!(bookings[6].amount_cents, 50_000);
        // Privateinlage: Bank (Soll) an 1890
        assert_eq!(bookings[7].konto, 1200);
        assert_eq!(bookings[7].gegenkonto, 1890);
        assert_eq!(bookings[7].amount_cents, 100_000);
    }

    #[test]
    fn build_bookings_skips_zero_and_unparsable() {
        let income = vec![
            income("2026-03-01", "RE-1", "A", 0),    // 0 → raus
            income("kaputt", "RE-2", "B", 1000),     // Datum unparsbar → raus
            income("2026-03-02", "RE-3", "C", 5000), // ok
        ];
        let b = build_bookings(Skr::Skr03, 2026, &income, &[], &[], &[], &[], &[]);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].belegfeld1, "RE-3");
    }

    #[test]
    fn to_datev_emits_header_columns_and_rows() {
        let header = DatevHeader {
            fiscal_year: 2026,
            skr: Skr::Skr03,
            generated_at: "20260521120000000".into(),
        };
        let income = vec![income("2026-03-01", "RE-2026-0001", "ACME", 100_000)];
        let bookings = build_bookings(Skr::Skr03, 2026, &income, &[], &[], &[], &[], &[]);
        let bytes = to_datev(&header, &bookings);
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.split("\r\n").filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3); // Vorlauf + Spaltenkopf + 1 Buchung
        assert!(lines[0].starts_with("\"EXTF\";700;21;"));
        assert!(lines[1].starts_with("\"Umsatz (ohne Soll/Haben-Kz)\";"));
        assert!(lines[2].starts_with("1000,00;"));
    }
}
