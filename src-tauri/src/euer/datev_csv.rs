//! DATEV-Buchungsstapel (EXTF) — **Functional Core**, Block 14b, überarbeitet
//! in Lauf F5b.
//!
//! Erzeugt aus den cash-wirksamen Bewegungen eines Geschäftsjahres einen
//! DATEV-Format-**Buchungsstapel** für die Steuerberater-Übergabe. Reine
//! Rechnung ohne I/O: nimmt die Einzelaufstellungs-Views (aus
//! [`crate::euer::detail`]) + den gewählten Kontenrahmen und liefert die fertigen
//! Bytes (CP1252) zurück.
//!
//! ## Format (DATEV-Format, Formatbeschreibung „Buchungsstapel“, Version 700)
//!
//! - **Kopf (Vorlauf):** `"EXTF";700;21;"Buchungsstapel";12;…` — Datenkategorie
//!   21, Formatversion 12, 31 Felder. Feld 27 trägt den Sachkontenrahmen
//!   (`"03"`/`"04"`). Berater- (Feld 11) und Mandantennummer (Feld 12) sind
//!   optional übergebbar; leer übernimmt DATEV beim Import über die
//!   Stapelverarbeitung den geöffneten Mandanten (vom Steuerberater bestätigt).
//! - **Spaltenüberschrift + Buchungszeilen:** die führenden Standard-Spalten des
//!   Buchungsstapels (Umsatz, S/H-Kennzeichen, WKZ, Kurs, Basis-Umsatz, WKZ-Basis,
//!   Konto, Gegenkonto, BU-Schlüssel, Belegdatum, Belegfeld 1+2, Skonto,
//!   Buchungstext). DATEV liest positionsbasiert.
//! - **Kodierung:** Windows-1252 (vollständige Tabelle, nicht darstellbare
//!   Zeichen und C1-Steuerzeichen werden `?`), CRLF-Zeilenenden, Dezimal-Komma.
//!   Steuerzeichen (Zeilenumbrüche) in Textfeldern werden zu Leerzeichen.
//!
//! ## Kontenrahmen (SKR03 Default / SKR04)
//!
//! Die Konten sind ein **Vorschlag** und vor Verbuchung vom Steuerberater zu
//! prüfen (Doku: `docs/reference/datev-format.md`, Kontenbezeichnungen am
//! DATEV-Kontenrahmen SKR03/SKR04 geprüft). Buchungslogik (Umsatz positiv,
//! Soll/Haben-Kennzeichen bezogen auf „Konto“). Bei jeder Geldbewegung ist
//! „Konto“ das Geldkonto und „Gegenkonto“ das Sachkonto, das einen
//! BU-Schlüssel trägt (DATEV ordnet den Steuerschlüssel dem Gegenkonto zu;
//! F5-fix):
//! - Einnahme: Geldkonto (S) an Erlöskonto (§19: 8195/4185; Regelbesteuerung je
//!   Steuergruppe: 19 % 8400/4400, 7 % 8300/4300 — DATEV-Automatikkonten mit
//!   Bruttobetrag; § 4 Nr. 1b 8125/4125, § 4 Nr. 1a 8120/4120; übrige 8200/4200);
//!   ein negativer Gruppenanteil wird als Gegenbuchung (H) ausgegeben
//! - Ausgabe: Geldkonto (H) an Aufwandskonto (bei Regelbesteuerung BU-Schlüssel
//!   9 bzw. 8 für 19 %/7 % Vorsteuer am Gegenkonto, sonst Vorsteuer gesondert
//!   auf 1570/1400)
//! - Aktivierte Kosten: Geldkonto (H) an Anlagekonto (Zugang)
//! - Storno-Erstattung: Geldkonto (H) an Erlöskonto des Originals
//! - Anlagen-Abgang: Geldkonto an Erlöse Anlagenverkauf ohne USt (bei
//!   Buchgewinn 8829/4849, bei Buchverlust 8800/6889) und Restbuchwert
//!   (2315/4855 bzw. 2310/6895) an Anlagekonto
//! - AfA (31.12.): Abschreibungsaufwand an Anlagekonto (direkte AfA)
//! - Privatbewegungen: Geldkonto (Entnahme H, Einlage S) an Privatkonto
//!
//! Geldkonto je Zahlungskonto (R2-011): Bank 1200/1800, Kasse 1000/1600,
//! PayPal/Stripe/sonstige 1360/1460 (Geldtransit); ohne hinterlegtes Konto Bank.

use chrono::NaiveDate;

use crate::euer::detail::{
    AveeurItem, DisposalItem, ExpenseItem, IncomeItem, PrivateMovementItem, StornoItem, TaxShare,
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

    /// Kennzeichen für Feld 27 des DATEV-Kopfs (Sachkontenrahmen).
    pub fn header_code(&self) -> &'static str {
        match self {
            Skr::Skr03 => "03",
            Skr::Skr04 => "04",
        }
    }

    fn pick(&self, skr03: u32, skr04: u32) -> u32 {
        match self {
            Skr::Skr03 => skr03,
            Skr::Skr04 => skr04,
        }
    }

    /// §19-Kleinunternehmer-Erlöse.
    pub fn revenue_kleinunternehmer(&self) -> u32 {
        self.pick(8195, 4185)
    }

    /// Erlöskonto für eine Steuergruppe einer Rechnung mit Regelbesteuerung.
    /// `None` = keine passende Automatik; der Nettoanteil geht dann auf
    /// „Erlöse“ (8200/4200) und die Steuer gesondert auf „Umsatzsteuer“.
    pub fn revenue_regular(&self, category: &str, rate_basis_points: i64) -> Option<u32> {
        match (category, rate_basis_points) {
            ("S", 1900) => Some(self.pick(8400, 4400)),
            ("S", 700) => Some(self.pick(8300, 4300)),
            ("K", _) => Some(self.pick(8125, 4125)),
            ("G", _) => Some(self.pick(8120, 4120)),
            ("S", _) => None,
            _ => Some(self.revenue_plain()),
        }
    }

    /// „Erlöse“ ohne Steuerautomatik.
    pub fn revenue_plain(&self) -> u32 {
        self.pick(8200, 4200)
    }

    /// „Umsatzsteuer“ (allgemein, für Sätze ohne Automatikkonto).
    pub fn output_tax(&self) -> u32 {
        self.pick(1770, 3800)
    }

    /// „Abziehbare Vorsteuer“ (allgemein, für Sätze ohne BU-Schlüssel).
    pub fn input_tax(&self) -> u32 {
        self.pick(1570, 1400)
    }

    /// Erlöse aus dem Verkauf von Sachanlagevermögen **ohne** Umsatzsteuer
    /// (bei Buchgewinn 8829/4849, bei Buchverlust 8800/6889). Die Konten
    /// 8820/4845 sind 19-%-Automatikkonten und für §19 falsch.
    pub fn revenue_disposal(&self, gain: bool) -> u32 {
        if gain {
            self.pick(8829, 4849)
        } else {
            self.pick(8800, 6889)
        }
    }

    /// Anlagenabgang Sachanlagen, Restbuchwert (bei Buchgewinn 2315/4855,
    /// bei Buchverlust 2310/6895).
    pub fn disposal_residual(&self, gain: bool) -> u32 {
        if gain {
            self.pick(2315, 4855)
        } else {
            self.pick(2310, 6895)
        }
    }

    /// Standard-Bankkonto.
    pub fn bank(&self) -> u32 {
        self.pick(1200, 1800)
    }

    /// Kassenkonto.
    pub fn cash(&self) -> u32 {
        self.pick(1000, 1600)
    }

    /// Geldtransit (Zahlungsdienstleister wie PayPal/Stripe; der
    /// Steuerberater löst auf das eigene Verrechnungskonto auf).
    pub fn transit(&self) -> u32 {
        self.pick(1360, 1460)
    }

    /// Geldkonto zu einem Zahlungskonto-Typ (R2-011). Ohne Konto: Bank.
    pub fn money_account(&self, account_type: Option<&str>) -> u32 {
        match account_type {
            Some("cash") => self.cash(),
            Some("paypal") | Some("stripe") | Some("other") => self.transit(),
            _ => self.bank(),
        }
    }

    /// Abschreibungsaufwand auf Sachanlagen (lineare AfA).
    pub fn afa_regular(&self) -> u32 {
        self.pick(4830, 6220)
    }

    /// Sofortabschreibung geringwertiger Wirtschaftsgüter (GWG).
    pub fn afa_gwg(&self) -> u32 {
        self.pick(4855, 6260)
    }

    /// Anlagekonto (Gegenkonto der AfA bei direkter Abschreibung) —
    /// Büroeinrichtung (0420/0650) als generischer Default.
    pub fn asset_account(&self) -> u32 {
        self.pick(420, 650)
    }

    /// GWG-Anlagekonto (Gegenkonto der GWG-Sofortabschreibung).
    pub fn asset_account_gwg(&self) -> u32 {
        self.pick(480, 670)
    }

    /// Anlagekonto zur AfA-Methode.
    pub fn asset_account_for(&self, method: &str) -> u32 {
        if method == "gwg_sofort" {
            self.asset_account_gwg()
        } else {
            self.asset_account()
        }
    }

    /// Privatentnahme — SKR03 1800 „Privatentnahmen allgemein"; SKR04 2100 (R2-009).
    pub fn private_withdrawal(&self) -> u32 {
        self.pick(1800, 2100)
    }

    /// Privateinlage — SKR03 1890 „Privateinlagen"; SKR04 2180 (R2-009).
    pub fn private_deposit(&self) -> u32 {
        self.pick(1890, 2180)
    }

    /// Aufwandskonto je Kosten-Kategorie (Konten ohne Steuerautomatik).
    /// **Vorschlag, STB prüft.** Unbekanntes → „Sonstige betriebliche
    /// Aufwendungen“ (4900/6300).
    pub fn expense_account(&self, category: &str) -> u32 {
        match category {
            "goods" => self.pick(3200, 5200),          // Wareneingang
            "services" => self.pick(3100, 5900),       // Fremdleistungen
            "office" => self.pick(4930, 6815),         // Bürobedarf
            "travel" => self.pick(4670, 6670),         // Reisekosten Unternehmer
            "communications" => self.pick(4920, 6805), // Telefon
            "vehicle" => self.pick(4530, 6530),        // Laufende Kfz-Betriebskosten
            "rent" => self.pick(4210, 6310),           // Miete (unbewegliche WG)
            "insurance" => self.pick(4360, 6400),      // Versicherungen
            "training" => self.pick(4945, 6821),       // Fortbildungskosten
            "fees" => self.pick(4970, 6855),           // Nebenkosten des Geldverkehrs
            "marketing" => self.pick(4600, 6600),      // Werbekosten
            "software" => self.pick(4806, 6495),       // Wartungskosten Hard- und Software
            "hardware" => self.pick(4980, 6850),       // Sonstiger Betriebsbedarf
            _ => self.pick(4900, 6300),                // Sonstige betriebliche Aufwendungen
        }
    }
}

// ============================================================================
// Buchung
// ============================================================================

/// Eine DATEV-Buchungszeile (Umsatz immer positiv, Richtung über das
/// Soll/Haben-Kennzeichen; bei Geldbewegungen ist `konto` das Geldkonto).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatevBooking {
    pub amount_cents: i64,
    /// Soll/Haben-Kennzeichen bezogen auf `konto` ('S' oder 'H').
    pub soll_haben: char,
    pub konto: u32,
    pub gegenkonto: u32,
    /// BU-Schlüssel (Steuerschlüssel) zum `gegenkonto`, leer = keiner. `9` =
    /// 19 % Vorsteuer, `8` = 7 % Vorsteuer (nur Regelbesteuerung, nie auf
    /// Automatikkonten).
    pub bu_schluessel: String,
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
    /// Beraternummer (Feld 11, 1001–9999999) — optional.
    pub consultant_number: Option<u32>,
    /// Mandantennummer (Feld 12, 1–99999) — optional.
    pub client_number: Option<u32>,
}

/// Prüft Berater- und Mandantennummer gegen die Wertebereiche der
/// DATEV-Formatbeschreibung (Berater 1001–9999999, Mandant 1–99999).
pub fn validate_header_numbers(
    consultant: Option<u32>,
    client: Option<u32>,
) -> std::result::Result<(), String> {
    if let Some(c) = consultant {
        if !(1001..=9_999_999).contains(&c) {
            return Err("Beraternummer muss zwischen 1001 und 9999999 liegen.".into());
        }
    }
    if let Some(m) = client {
        if !(1..=99_999).contains(&m) {
            return Err("Mandantennummer muss zwischen 1 und 99999 liegen.".into());
        }
    }
    Ok(())
}

/// Eingaben für [`build_bookings`] (Einzelposten eines Geschäftsjahres).
pub struct DatevInputs<'a> {
    pub income: &'a [IncomeItem],
    pub storno: &'a [StornoItem],
    /// Bezahlte, nicht aktivierte Kosten.
    pub expenses: &'a [ExpenseItem],
    /// Bezahlte, als Anlage aktivierte Kosten (Zugang auf dem Anlagekonto).
    pub capitalized: &'a [ExpenseItem],
    pub disposals: &'a [DisposalItem],
    pub assets: &'a [AveeurItem],
    pub private_movements: &'a [PrivateMovementItem],
}

// ============================================================================
// Buchungs-Aufbau (aus den Einzelaufstellungs-Views)
// ============================================================================

fn parse_iso(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&s.chars().take(10).collect::<String>(), "%Y-%m-%d").ok()
}

/// Kürzt auf `max` Zeichen und ersetzt Steuerzeichen (Zeilenumbrüche,
/// Tabulatoren) durch Leerzeichen — DATEV-Textfelder sind einzeilig.
fn truncate(s: &str, max: usize) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max)
        .collect()
}

/// Erlösbuchungen einer Zahlung bzw. (mit `reverse`) einer Erstattung. Bei
/// §19 eine Buchung auf 8195/4185, sonst je Steuergruppe.
///
/// Konto ist immer das Geldkonto, Gegenkonto das Erlöskonto (F5-fix): Zahlung
/// `S`, Erstattung `H`. Ein negativer Anteil einer Steuergruppe (z. B.
/// Gutschrift-Position in einer anderen Gruppe) kehrt das Kennzeichen um, statt
/// wegzufallen — so ist die Summe der Geldkonto-Bewegungen gleich dem
/// Zahlungsbetrag. Nullbeträge entfallen.
#[allow(clippy::too_many_arguments)]
fn revenue_bookings(
    out: &mut Vec<DatevBooking>,
    skr: Skr,
    money: u32,
    is_klein: bool,
    amount: i64,
    shares: &[TaxShare],
    reverse: bool,
    date: NaiveDate,
    belegfeld1: &str,
    belegfeld2: &str,
    text: &str,
) {
    let mut push = |amount_cents: i64, revenue: u32| {
        if amount_cents == 0 {
            return;
        }
        // Geldkonto im Soll bei einem Zugang (Zahlung, positiver Anteil).
        let money_debit = (amount_cents > 0) != reverse;
        out.push(DatevBooking {
            amount_cents: amount_cents.saturating_abs(),
            soll_haben: if money_debit { 'S' } else { 'H' },
            konto: money,
            gegenkonto: revenue,
            bu_schluessel: String::new(),
            belegdatum: date,
            belegfeld1: truncate(belegfeld1, 36),
            belegfeld2: truncate(belegfeld2, 12),
            buchungstext: truncate(text.trim(), 60),
        });
    };
    if is_klein || shares.is_empty() {
        let revenue = if is_klein {
            skr.revenue_kleinunternehmer()
        } else {
            skr.revenue_plain()
        };
        push(amount, revenue);
        return;
    }
    for s in shares {
        match skr.revenue_regular(&s.category_code, s.rate_basis_points) {
            Some(acc) => push(s.gross_cents, acc),
            None => {
                push(s.net_cents, skr.revenue_plain());
                push(s.tax_cents, skr.output_tax());
            }
        }
    }
}

/// BU-Schlüssel für eine Kostenposition bei Regelbesteuerung: 9 = 19 %,
/// 8 = 7 % Vorsteuer, wenn die Belegsteuer dem Satz auf den Cent entspricht.
fn input_tax_key(net: i64, tax: i64) -> Option<&'static str> {
    if tax <= 0 || net <= 0 {
        return None;
    }
    let at = |bp: i128| -> i64 { ((net as i128 * bp + 5_000) / 10_000) as i64 };
    if (tax - at(1900)).abs() <= 1 {
        Some("9")
    } else if (tax - at(700)).abs() <= 1 {
        Some("8")
    } else {
        None
    }
}

/// Aufwands- bzw. Zugangsbuchung einer Kostenposition: Konto = Geldkonto im
/// Haben, Gegenkonto = Aufwands- bzw. Anlagekonto mit BU-Schlüssel (F5-fix —
/// DATEV ordnet den Steuerschlüssel dem Gegenkonto zu; Feld „Gegenkonto (ohne
/// BU-Schlüssel)“). Vorsteuer nur bei Regelbesteuerung im Zeitpunkt des
/// Leistungsbezugs (`ExpenseItem::is_kleinunternehmer`, Lauf F8d, § 15 Abs. 1
/// UStG), nicht nach dem Status des Zahlungsjahres.
fn expense_bookings(
    out: &mut Vec<DatevBooking>,
    skr: Skr,
    e: &ExpenseItem,
    konto: u32,
    text: &str,
) {
    if e.gross_cents <= 0 {
        return;
    }
    let Some(date) = parse_iso(&e.paid_date) else {
        return;
    };
    let money = skr.money_account(e.account_type.as_deref());
    let mk = |amount_cents: i64, gegenkonto: u32, bu: &str| DatevBooking {
        amount_cents,
        soll_haben: 'H',
        konto: money,
        gegenkonto,
        bu_schluessel: bu.to_string(),
        belegdatum: date,
        belegfeld1: truncate(&e.expense_number, 36),
        belegfeld2: String::new(),
        buchungstext: truncate(text.trim(), 60),
    };
    if e.is_kleinunternehmer || e.tax_cents <= 0 {
        out.push(mk(e.gross_cents, konto, ""));
        return;
    }
    let net = e.gross_cents - e.tax_cents;
    match input_tax_key(net, e.tax_cents) {
        Some(bu) => out.push(mk(e.gross_cents, konto, bu)),
        None => {
            if net > 0 {
                out.push(mk(net, konto, ""));
            }
            out.push(mk(e.tax_cents, skr.input_tax(), ""));
        }
    }
}

/// Baut alle Buchungen eines Geschäftsjahres. AfA-Buchungen werden auf den
/// 31.12. datiert.
pub fn build_bookings(skr: Skr, fiscal_year: i32, inp: &DatevInputs<'_>) -> Vec<DatevBooking> {
    let mut out = Vec::new();

    // Einnahmen: Geldkonto (Soll) an Erlöskonto (Haben). Gegenbuchungen
    // („Zahlung stornieren“, ADR 0042 Nr. 9) sind negativ: dieselben Konten mit
    // umgekehrtem Soll/Haben, Belegdatum und Belegfeld 1 wie die stornierte
    // Zahlung — sonst wiche der Stapel still von der EÜR ab.
    for i in inp.income {
        if i.amount_cents == 0 {
            continue;
        }
        if let Some(d) = parse_iso(&i.paid_date) {
            let text = if i.is_reversal || i.amount_cents < 0 {
                format!("Storno Zahlung {} {}", i.invoice_number, i.customer)
            } else {
                format!("{} {}", i.customer, i.description)
            };
            revenue_bookings(
                &mut out,
                skr,
                skr.money_account(i.account_type.as_deref()),
                i.is_kleinunternehmer,
                i.amount_cents,
                &i.shares,
                false,
                d,
                &i.invoice_number,
                "",
                &text,
            );
        }
    }

    // Storno-Erstattung: Geldkonto (Haben) an Erlöskonto des Originals.
    // R2-010: Belegfeld 2 = Original-Belegnummer.
    for s in inp.storno {
        if s.refunded_cents <= 0 {
            continue;
        }
        if let Some(d) = parse_iso(&s.storno_date) {
            revenue_bookings(
                &mut out,
                skr,
                skr.money_account(s.account_type.as_deref()),
                s.is_kleinunternehmer,
                s.refunded_cents,
                &s.shares,
                true,
                d,
                &s.storno_number,
                &s.original_number,
                &format!("Storno zu {}", s.original_number),
            );
        }
    }

    // Ausgaben: Geldkonto (Haben) an Aufwandskonto (Soll, BU-Schlüssel am Gegenkonto).
    for e in inp.expenses {
        expense_bookings(
            &mut out,
            skr,
            e,
            skr.expense_account(&e.category),
            &format!("{} {}", e.vendor, e.description),
        );
    }

    // Aktivierte Kosten: Geldkonto (Haben) an Anlagekonto (Soll) — Zugang. Bei
    // Regelbesteuerung (bei Leistungsbezug, F8d) nur der Nettobetrag (BU-Schlüssel bzw. gesonderte
    // Vorsteuerbuchung); die Vorsteuer bleibt Aufwand des Zahlungsjahres (F8b).
    for e in inp.capitalized {
        let method = e.capitalized_asset_method.as_deref().unwrap_or("linear");
        expense_bookings(
            &mut out,
            skr,
            e,
            skr.asset_account_for(method),
            &format!("Zugang {} {}", e.vendor, e.description),
        );
    }

    // Anlagen-Abgang: Erlös (Geld an Erlöse Anlagenverkauf) und Restbuchwert
    // (Anlagenabgang an Anlagekonto), Konten je nach Buchgewinn/-verlust.
    for d in inp.disposals {
        let Some(date) = parse_iso(&d.disposal_date) else {
            continue;
        };
        let gain = d.proceeds_cents >= d.residual_book_value_cents;
        if d.proceeds_cents > 0 {
            // Lauf F8: Bei einer Privatentnahme fließt kein Geld — der
            // Entnahmewert (Teilwert, § 6 Abs. 1 Nr. 4 EStG) geht vom
            // Privatkonto an die Erlöse aus dem Anlagenabgang.
            let withdrawal = d.disposal_type == "given_away";
            out.push(DatevBooking {
                amount_cents: d.proceeds_cents,
                soll_haben: 'S',
                konto: if withdrawal {
                    skr.private_withdrawal()
                } else {
                    skr.bank()
                },
                gegenkonto: skr.revenue_disposal(gain),
                bu_schluessel: String::new(),
                belegdatum: date,
                belegfeld1: truncate(&d.asset_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(
                    &format!(
                        "{} {}",
                        if withdrawal { "Entnahme" } else { "Verkauf" },
                        d.label
                    ),
                    60,
                ),
            });
        }
        if d.residual_book_value_cents > 0 {
            out.push(DatevBooking {
                amount_cents: d.residual_book_value_cents,
                soll_haben: 'S',
                konto: skr.disposal_residual(gain),
                gegenkonto: skr.asset_account_for(&d.depreciation_method),
                bu_schluessel: String::new(),
                belegdatum: date,
                belegfeld1: truncate(&d.asset_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(&format!("Restbuchwert {}", d.label), 60),
            });
        }
    }

    // AfA (31.12.): Abschreibungsaufwand (Soll) an Anlagekonto (Haben).
    if let Some(year_end) = NaiveDate::from_ymd_opt(fiscal_year, 12, 31) {
        for a in inp.assets {
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
                gegenkonto: skr.asset_account_for(&a.depreciation_method),
                bu_schluessel: String::new(),
                belegdatum: year_end,
                belegfeld1: truncate(&a.asset_number, 36),
                belegfeld2: String::new(),
                buchungstext: truncate(&format!("AfA {}", a.label), 60),
            });
        }
    }

    // Privatbewegungen (R2-009): EÜR-neutral, aber für die Abstimmung der
    // Geldkonten beim STB Pflicht. Unbekannte movement_types werden
    // übersprungen (Schema-Garantie: 'entnahme' | 'einlage').
    for p in inp.private_movements {
        if p.amount_cents <= 0 {
            continue;
        }
        let Some(date) = parse_iso(&p.movement_date) else {
            continue;
        };
        let money = skr.money_account(p.account_type.as_deref());
        // Konto = Geldkonto wie bei allen Geldbewegungen: Entnahme im Haben,
        // Einlage im Soll; Gegenkonto = Privatkonto.
        let (soll_haben, gegenkonto, prefix) = match p.movement_type.as_str() {
            "entnahme" => ('H', skr.private_withdrawal(), "Privatentnahme"),
            "einlage" => ('S', skr.private_deposit(), "Privateinlage"),
            _ => continue,
        };
        out.push(DatevBooking {
            amount_cents: p.amount_cents,
            soll_haben,
            konto: money,
            gegenkonto,
            bu_schluessel: String::new(),
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

/// DATEV-Textfeld: in Anführungszeichen, eingebettete `"` verdoppelt,
/// Steuerzeichen als Leerzeichen.
fn q(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    format!("\"{}\"", clean.replace('"', "\"\""))
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
    let num = |n: Option<u32>| n.map(|v| v.to_string()).unwrap_or_default();
    let fields: Vec<String> = vec![
        q("EXTF"),                     // 1 Kennzeichen
        "700".into(),                  // 2 Versionsnummer
        "21".into(),                   // 3 Datenkategorie (Buchungsstapel)
        q("Buchungsstapel"),           // 4 Formatname
        "12".into(),                   // 5 Formatversion
        header.generated_at.clone(),   // 6 Erzeugt am (YYYYMMDDHHMMSSFFF)
        String::new(),                 // 7 Importiert
        String::new(),                 // 8 Herkunft
        String::new(),                 // 9 Exportiert von
        String::new(),                 // 10 Importiert von
        num(header.consultant_number), // 11 Beraternummer (optional)
        num(header.client_number),     // 12 Mandantennummer (optional)
        wj,                            // 13 WJ-Beginn
        "4".into(),                    // 14 Sachkontenlänge
        von,                           // 15 Datum von
        bis,                           // 16 Datum bis
        q(&bez),                       // 17 Bezeichnung
        String::new(),                 // 18 Diktatkürzel
        "1".into(),                    // 19 Buchungstyp (1 = Finanzbuchführung)
        "0".into(),                    // 20 Rechnungslegungszweck
        "0".into(),                    // 21 Festschreibung (0 = nicht festgeschrieben)
        q("EUR"),                      // 22 WKZ
        String::new(),                 // 23 reserviert
        String::new(),                 // 24 Derivatskennzeichen
        String::new(),                 // 25 reserviert
        String::new(),                 // 26 reserviert
        q(header.skr.header_code()),   // 27 Sachkontenrahmen
        String::new(),                 // 28 ID der Branchenlösung
        String::new(),                 // 29 reserviert
        String::new(),                 // 30 reserviert
        String::new(),                 // 31 Anwendungsinformation
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
        // mit führender 0 (R2-001).
        format!("{:04}", b.konto),
        format!("{:04}", b.gegenkonto),
        if b.bu_schluessel.is_empty() {
            String::new()
        } else {
            q(&b.bu_schluessel)
        }, // 9 BU-Schlüssel
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

/// Windows-1252-Encoder (R2-S2-CP1252): ASCII und U+00A0–U+00FF direkt, die 27
/// belegten Zeichen des Bereichs 0x80–0x9F über die Tabelle der Codepage
/// (WHATWG Encoding Standard, „windows-1252“), alles andere — auch die
/// C1-Steuerzeichen U+0080–U+009F — wird `?`.
fn to_cp1252(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let code = ch as u32;
        let byte = match code {
            0x00..=0x7F => code as u8,
            0xA0..=0xFF => code as u8,
            _ => match ch {
                '€' => 0x80,
                '‚' => 0x82,
                'ƒ' => 0x83,
                '„' => 0x84,
                '…' => 0x85,
                '†' => 0x86,
                '‡' => 0x87,
                'ˆ' => 0x88,
                '‰' => 0x89,
                'Š' => 0x8A,
                '‹' => 0x8B,
                'Œ' => 0x8C,
                'Ž' => 0x8E,
                '‘' => 0x91,
                '’' => 0x92,
                '“' => 0x93,
                '”' => 0x94,
                '•' => 0x95,
                '–' => 0x96,
                '—' => 0x97,
                '˜' => 0x98,
                '™' => 0x99,
                'š' => 0x9A,
                '›' => 0x9B,
                'œ' => 0x9C,
                'ž' => 0x9E,
                'Ÿ' => 0x9F,
                _ => b'?',
            },
        };
        out.push(byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn header(skr: Skr) -> DatevHeader {
        DatevHeader {
            fiscal_year: 2026,
            skr,
            generated_at: "20260521120000000".into(),
            consultant_number: None,
            client_number: None,
        }
    }

    fn booking(konto: u32, gegenkonto: u32) -> DatevBooking {
        DatevBooking {
            amount_cents: 100_000,
            soll_haben: 'S',
            konto,
            gegenkonto,
            bu_schluessel: String::new(),
            belegdatum: d(2026, 3, 1),
            belegfeld1: "RE-2026-0001".into(),
            belegfeld2: String::new(),
            buchungstext: "ACME Beratung".into(),
        }
    }

    fn empty_inputs<'a>() -> DatevInputs<'a> {
        DatevInputs {
            income: &[],
            storno: &[],
            expenses: &[],
            capitalized: &[],
            disposals: &[],
            assets: &[],
            private_movements: &[],
        }
    }

    #[test]
    fn skr_from_code_defaults_to_skr03() {
        assert_eq!(Skr::from_code("skr04"), Skr::Skr04);
        assert_eq!(Skr::from_code("SKR04"), Skr::Skr04);
        assert_eq!(Skr::from_code("skr03"), Skr::Skr03);
        assert_eq!(Skr::from_code("irgendwas"), Skr::Skr03);
        assert_eq!(Skr::from_code(""), Skr::Skr03);
    }

    /// Kontennummern am DATEV-Kontenrahmen SKR03/SKR04 geprüft (Lauf F5b).
    #[test]
    fn account_mapping_skr03_and_skr04() {
        assert_eq!(Skr::Skr03.revenue_kleinunternehmer(), 8195);
        assert_eq!(Skr::Skr04.revenue_kleinunternehmer(), 4185);
        assert_eq!(Skr::Skr03.bank(), 1200);
        assert_eq!(Skr::Skr04.bank(), 1800);
        assert_eq!(Skr::Skr03.expense_account("goods"), 3200);
        assert_eq!(Skr::Skr04.expense_account("goods"), 5200);
        assert_eq!(Skr::Skr03.expense_account("office"), 4930);
        assert_eq!(Skr::Skr03.expense_account("software"), 4806);
        assert_eq!(Skr::Skr04.expense_account("software"), 6495);
        assert_eq!(Skr::Skr03.expense_account("hardware"), 4980);
        assert_eq!(Skr::Skr04.expense_account("hardware"), 6850);
        // Auffang: Sonstige betriebliche Aufwendungen (SKR04 6800 ist Porto).
        assert_eq!(Skr::Skr03.expense_account("unknown"), 4900);
        assert_eq!(Skr::Skr04.expense_account("unknown"), 6300);
        assert_eq!(Skr::Skr03.afa_regular(), 4830);
        assert_eq!(Skr::Skr03.afa_gwg(), 4855);
        // Anlagenverkauf ohne USt (8820/4845 wären 19-%-Automatikkonten).
        assert_eq!(Skr::Skr03.revenue_disposal(true), 8829);
        assert_eq!(Skr::Skr03.revenue_disposal(false), 8800);
        assert_eq!(Skr::Skr04.revenue_disposal(true), 4849);
        assert_eq!(Skr::Skr04.revenue_disposal(false), 6889);
        assert_eq!(Skr::Skr03.disposal_residual(true), 2315);
        assert_eq!(Skr::Skr03.disposal_residual(false), 2310);
        assert_eq!(Skr::Skr04.disposal_residual(true), 4855);
        assert_eq!(Skr::Skr04.disposal_residual(false), 6895);
        // Regelbesteuerung.
        assert_eq!(Skr::Skr03.revenue_regular("S", 1900), Some(8400));
        assert_eq!(Skr::Skr04.revenue_regular("S", 700), Some(4300));
        assert_eq!(Skr::Skr03.revenue_regular("K", 0), Some(8125));
        assert_eq!(Skr::Skr03.revenue_regular("G", 0), Some(8120));
        assert_eq!(Skr::Skr03.revenue_regular("AE", 0), Some(8200));
        assert_eq!(Skr::Skr03.revenue_regular("S", 1600), None);
    }

    /// R2-011: Geldkonto je Zahlungskonto-Typ.
    #[test]
    fn money_account_by_payment_account_type() {
        assert_eq!(Skr::Skr03.money_account(None), 1200);
        assert_eq!(Skr::Skr03.money_account(Some("bank")), 1200);
        assert_eq!(Skr::Skr03.money_account(Some("cash")), 1000);
        assert_eq!(Skr::Skr03.money_account(Some("paypal")), 1360);
        assert_eq!(Skr::Skr03.money_account(Some("stripe")), 1360);
        assert_eq!(Skr::Skr03.money_account(Some("other")), 1360);
        assert_eq!(Skr::Skr04.money_account(Some("cash")), 1600);
        assert_eq!(Skr::Skr04.money_account(Some("paypal")), 1460);
        assert_eq!(Skr::Skr04.money_account(None), 1800);
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

    /// R2-S2-DATEV-HEADER: 31 Felder, Feld 27 = Sachkontenrahmen, Berater- und
    /// Mandantennummer optional.
    #[test]
    fn header_line_has_extf_markers_skr_and_optional_numbers() {
        let line = build_header_line(&header(Skr::Skr03));
        assert!(line.starts_with("\"EXTF\";700;21;\"Buchungsstapel\";12;"));
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols.len(), 31);
        assert_eq!(cols[10], "");
        assert_eq!(cols[11], "");
        assert_eq!(cols[12], "20260101");
        assert_eq!(cols[15], "20261231");
        assert_eq!(cols[16], "\"EÜR 2026\"");
        assert_eq!(cols[21], "\"EUR\"");
        assert_eq!(cols[26], "\"03\"");

        let mut h = header(Skr::Skr04);
        h.consultant_number = Some(29098);
        h.client_number = Some(55003);
        let cols_line = build_header_line(&h);
        let cols: Vec<&str> = cols_line.split(';').collect();
        assert_eq!(cols[10], "29098");
        assert_eq!(cols[11], "55003");
        assert_eq!(cols[26], "\"04\"");
    }

    #[test]
    fn header_numbers_are_range_checked() {
        assert!(validate_header_numbers(None, None).is_ok());
        assert!(validate_header_numbers(Some(1001), Some(1)).is_ok());
        assert!(validate_header_numbers(Some(9_999_999), Some(99_999)).is_ok());
        assert!(validate_header_numbers(Some(1000), None).is_err());
        assert!(validate_header_numbers(Some(10_000_000), None).is_err());
        assert!(validate_header_numbers(None, Some(0)).is_err());
        assert!(validate_header_numbers(None, Some(100_000)).is_err());
    }

    #[test]
    fn booking_line_field_layout() {
        let line = build_booking_line(&booking(1200, 8195));
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols.len(), 14);
        assert_eq!(cols[0], "1000,00"); // Umsatz
        assert_eq!(cols[1], "\"S\""); // S/H
        assert_eq!(cols[6], "1200"); // Konto
        assert_eq!(cols[7], "8195"); // Gegenkonto
        assert_eq!(cols[8], ""); // BU-Schlüssel leer
        assert_eq!(cols[9], "0103"); // Belegdatum TTMM
        assert_eq!(cols[10], "\"RE-2026-0001\""); // Belegfeld 1
        assert_eq!(cols[11], ""); // Belegfeld 2 leer
        assert_eq!(cols[13], "\"ACME Beratung\""); // Buchungstext

        let mut b = booking(4930, 1200);
        b.bu_schluessel = "9".into();
        let line = build_booking_line(&b);
        assert_eq!(line.split(';').nth(8), Some("\"9\""));
    }

    /// Steuerzeichen in Textfeldern brechen die Zeile nicht um.
    #[test]
    fn text_fields_are_single_line() {
        let mut b = booking(1200, 8195);
        b.buchungstext = "Zeile 1\r\nZeile 2\t\"zitiert\"".into();
        let line = build_booking_line(&b);
        assert!(!line.contains('\n') && !line.contains('\r') && !line.contains('\t'));
        assert!(line.ends_with("\"Zeile 1  Zeile 2 \"\"zitiert\"\"\""));
        assert_eq!(truncate("a\nb", 60), "a b");
    }

    /// R2-001 — Anlage-Sachkonten mit führender Null (Sachkontenlänge 4).
    #[test]
    fn booking_line_pads_three_digit_asset_accounts_to_four_digits() {
        let line = build_booking_line(&booking(
            Skr::Skr03.afa_regular(),
            Skr::Skr03.asset_account(),
        ));
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols[6], "4830");
        assert_eq!(cols[7], "0420");
        let line = build_booking_line(&booking(
            Skr::Skr03.afa_gwg(),
            Skr::Skr03.asset_account_gwg(),
        ));
        assert_eq!(line.split(';').nth(7), Some("0480"));
        let line = build_booking_line(&booking(
            Skr::Skr04.afa_regular(),
            Skr::Skr04.asset_account(),
        ));
        assert_eq!(line.split(';').nth(7), Some("0650"));
    }

    #[test]
    fn cp1252_encodes_umlauts_and_euro() {
        let bytes = to_cp1252("Müller€?");
        assert_eq!(bytes[0], b'M');
        assert_eq!(bytes[1], 0xFC);
        assert_eq!(bytes[6], 0x80);
        assert_eq!(bytes[7], b'?');
    }

    /// R2-S2-CP1252: vollständige Tabelle für 0x80–0x9F, C1-Steuerzeichen und
    /// nicht darstellbare Zeichen werden `?`.
    #[test]
    fn cp1252_full_table_and_c1_controls() {
        assert_eq!(
            to_cp1252("ŠŒŽšœžŸƒ‰‹›™•ˆ˜†‡"),
            vec![
                0x8A, 0x8C, 0x8E, 0x9A, 0x9C, 0x9E, 0x9F, 0x83, 0x89, 0x8B, 0x9B, 0x99, 0x95, 0x88,
                0x98, 0x86, 0x87
            ]
        );
        assert_eq!(
            to_cp1252("\u{0080}\u{0085}\u{009F}"),
            vec![b'?', b'?', b'?']
        );
        assert_eq!(to_cp1252("\u{2212}"), vec![b'?']); // Minuszeichen
        assert_eq!(to_cp1252("\u{00A0}ß"), vec![0xA0, 0xDF]);
    }

    fn klein_income(date: &str, nr: &str, cents: i64, acct: Option<&str>) -> IncomeItem {
        IncomeItem {
            paid_date: date.into(),
            invoice_number: nr.into(),
            customer: "ACME".into(),
            description: "Leistung".into(),
            amount_cents: cents,
            is_kleinunternehmer: true,
            shares: Vec::new(),
            account_type: acct.map(str::to_string),
            ..Default::default()
        }
    }

    fn expense(date: &str, nr: &str, cat: &str, gross: i64, tax: i64) -> ExpenseItem {
        ExpenseItem {
            paid_date: date.into(),
            expense_number: nr.into(),
            vendor: "Lieferant".into(),
            category: cat.into(),
            description: "Kosten".into(),
            gross_cents: gross,
            net_cents: gross - tax,
            tax_cents: tax,
            ..Default::default()
        }
    }

    /// Kosten mit Leistungsbezug unter Regelbesteuerung (Lauf F8d).
    fn regel(e: ExpenseItem) -> ExpenseItem {
        ExpenseItem {
            is_kleinunternehmer: false,
            ..e
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
        let income = vec![klein_income("2026-03-01", "RE-2026-0001", 100_000, None)];
        let storno = vec![StornoItem {
            storno_date: "2026-04-01".into(),
            storno_number: "ST-2026-0001".into(),
            original_number: "RE-2026-0009".into(),
            refunded_cents: 20_000,
            is_kleinunternehmer: true,
            shares: Vec::new(),
            account_type: None,
        }];
        let expenses = vec![expense("2026-05-01", "KO-2026-0001", "goods", 30_000, 0)];
        let disposals = vec![DisposalItem {
            disposal_date: "2026-06-01".into(),
            asset_number: "AV-2026-0002".into(),
            label: "Drucker".into(),
            proceeds_cents: 5_000,
            residual_book_value_cents: 1_000,
            gain_loss_cents: 4_000,
            depreciation_method: "linear".into(),
            disposal_type: "sale".into(),
        }];
        let assets = vec![aveeur("linear", 40_000), aveeur("gwg_sofort", 80_000)];
        let private_movements = vec![
            PrivateMovementItem {
                movement_date: "2026-08-15".into(),
                movement_number: "PR-2026-0001".into(),
                movement_type: "entnahme".into(),
                description: "Lebenshaltung".into(),
                amount_cents: 50_000,
                account_type: None,
            },
            PrivateMovementItem {
                movement_date: "2026-09-01".into(),
                movement_number: "PR-2026-0002".into(),
                movement_type: "einlage".into(),
                description: "Sparbuch".into(),
                amount_cents: 100_000,
                account_type: Some("cash".into()),
            },
        ];
        let inp = DatevInputs {
            income: &income,
            storno: &storno,
            expenses: &expenses,
            disposals: &disposals,
            assets: &assets,
            private_movements: &private_movements,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        // 1 Einnahme + 1 Storno + 1 Ausgabe + 2 Abgang + 2 AfA + 2 Privat = 9
        assert_eq!(b.len(), 9);
        assert_eq!((b[0].konto, b[0].gegenkonto), (1200, 8195));
        assert_eq!(b[0].soll_haben, 'S');
        // Erstattung: Geldkonto im Haben an Erlöskonto.
        assert_eq!(
            (b[1].konto, b[1].gegenkonto, b[1].soll_haben),
            (1200, 8195, 'H')
        );
        assert_eq!(b[1].belegfeld2, "RE-2026-0009");
        // Kosten: Geldkonto im Haben, Aufwandskonto als Gegenkonto.
        assert_eq!(
            (b[2].konto, b[2].gegenkonto, b[2].soll_haben),
            (1200, 3200, 'H')
        );
        // Abgang mit Buchgewinn: Erlös ohne USt + Restbuchwert an Anlagekonto.
        assert_eq!((b[3].konto, b[3].gegenkonto), (1200, 8829));
        assert_eq!((b[4].konto, b[4].gegenkonto), (2315, 420));
        assert_eq!(b[4].amount_cents, 1_000);
        assert_eq!((b[5].konto, b[5].gegenkonto), (4830, 420));
        assert_eq!(b[5].belegdatum, d(2026, 12, 31));
        assert_eq!((b[6].konto, b[6].gegenkonto), (4855, 480));
        assert_eq!(
            (b[7].konto, b[7].gegenkonto, b[7].soll_haben),
            (1200, 1800, 'H')
        );
        // Einlage in die Kasse.
        assert_eq!(
            (b[8].konto, b[8].gegenkonto, b[8].soll_haben),
            (1000, 1890, 'S')
        );
    }

    #[test]
    fn disposal_with_book_loss_uses_loss_accounts() {
        let disposals = vec![DisposalItem {
            disposal_date: "2026-06-01".into(),
            asset_number: "AV-2026-0002".into(),
            label: "Kamera".into(),
            proceeds_cents: 1_000,
            residual_book_value_cents: 5_000,
            gain_loss_cents: -4_000,
            depreciation_method: "gwg_sofort".into(),
            disposal_type: "sale".into(),
        }];
        let inp = DatevInputs {
            disposals: &disposals,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr04, 2026, &inp);
        assert_eq!((b[0].konto, b[0].gegenkonto), (1800, 6889));
        assert_eq!((b[1].konto, b[1].gegenkonto), (6895, 670));
    }

    /// Lauf F8: Privatentnahme einer Anlage — der Entnahmewert kommt vom
    /// Privatkonto (kein Geldfluss), nicht von der Bank.
    #[test]
    fn private_withdrawal_of_asset_books_from_private_account() {
        let disposals = vec![DisposalItem {
            disposal_date: "2026-06-01".into(),
            asset_number: "AV-2025-0003".into(),
            label: "Notebook".into(),
            proceeds_cents: 30_000,
            residual_book_value_cents: 40_000,
            gain_loss_cents: -10_000,
            depreciation_method: "linear".into(),
            disposal_type: "given_away".into(),
        }];
        let inp = DatevInputs {
            disposals: &disposals,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!((b[0].konto, b[0].gegenkonto), (1800, 8800));
        assert_eq!(b[0].amount_cents, 30_000);
        assert!(b[0].buchungstext.starts_with("Entnahme"));
        let b = build_bookings(Skr::Skr04, 2026, &inp);
        assert_eq!((b[0].konto, b[0].gegenkonto), (2100, 6889));
    }

    /// R2-011: Kasse und PayPal statt pauschal Bank.
    #[test]
    fn payment_accounts_drive_the_money_account() {
        let income = vec![klein_income("2026-03-01", "RE-1", 5_000, Some("paypal"))];
        let mut e = expense("2026-03-02", "KO-1", "office", 2_000, 0);
        e.account_type = Some("cash".into());
        let expenses = vec![e];
        let inp = DatevInputs {
            income: &income,
            expenses: &expenses,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!((b[0].konto, b[0].gegenkonto), (1360, 8195));
        assert_eq!((b[1].konto, b[1].gegenkonto), (1000, 4930));
    }

    /// Regelbesteuerung: Erlöse je Steuergruppe auf Automatikkonten, Kosten mit
    /// BU-Schlüssel; Sätze ohne Automatik getrennt nach Netto und Steuer.
    #[test]
    fn regular_taxation_uses_automatic_accounts_and_tax_keys() {
        let income = vec![IncomeItem {
            paid_date: "2026-03-01".into(),
            invoice_number: "RE-2026-0002".into(),
            amount_cents: 22_600,
            is_kleinunternehmer: false,
            shares: vec![
                TaxShare {
                    category_code: "S".into(),
                    rate_basis_points: 1900,
                    gross_cents: 11_900,
                    net_cents: 10_000,
                    tax_cents: 1_900,
                },
                TaxShare {
                    category_code: "S".into(),
                    rate_basis_points: 700,
                    gross_cents: 10_700,
                    net_cents: 10_000,
                    tax_cents: 700,
                },
            ],
            ..Default::default()
        }];
        // Leistungsbezug unter Regelbesteuerung (Lauf F8d: Status je Kosten).
        let expenses: Vec<ExpenseItem> = [
            expense("2026-03-02", "KO-1", "office", 11_900, 1_900),
            expense("2026-03-03", "KO-2", "training", 10_700, 700),
            expense("2026-03-04", "KO-3", "goods", 11_600, 1_600),
        ]
        .into_iter()
        .map(regel)
        .collect();
        let inp = DatevInputs {
            income: &income,
            expenses: &expenses,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!((b[0].gegenkonto, b[0].amount_cents), (8400, 11_900));
        assert_eq!((b[1].gegenkonto, b[1].amount_cents), (8300, 10_700));
        // BU-Schlüssel am Gegenkonto (Aufwandskonto), Konto = Geldkonto im Haben.
        assert_eq!(
            (b[2].konto, b[2].gegenkonto, b[2].bu_schluessel.as_str()),
            (1200, 4930, "9")
        );
        assert_eq!((b[2].soll_haben, b[2].amount_cents), ('H', 11_900));
        assert_eq!((b[3].gegenkonto, b[3].bu_schluessel.as_str()), (4945, "8"));
        // 16 %: Netto aufs Aufwandskonto, Vorsteuer gesondert auf 1570.
        assert_eq!((b[4].gegenkonto, b[4].amount_cents), (3200, 10_000));
        assert_eq!((b[5].gegenkonto, b[5].amount_cents), (1570, 1_600));
        assert!(b[2..]
            .iter()
            .all(|x| x.konto == 1200 && x.soll_haben == 'H'));
        assert_eq!(b.len(), 6);

        // Gleiche Kosten mit Leistungsbezug unter §19: brutto, ohne
        // BU-Schlüssel — auch wenn das Zahlungsjahr regelbesteuert ist.
        let klein = vec![ExpenseItem {
            is_kleinunternehmer: true,
            ..expenses[0].clone()
        }];
        let inp = DatevInputs {
            expenses: &klein,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(
            (b[0].amount_cents, b[0].bu_schluessel.as_str()),
            (11_900, "")
        );
    }

    /// Aktivierte Kosten gehen als Zugang aufs Anlagekonto, nicht in den Aufwand.
    #[test]
    fn capitalized_expense_is_booked_to_the_asset_account() {
        let mut e = expense("2026-02-01", "KO-2026-0005", "hardware", 150_000, 0);
        e.capitalized_asset_method = Some("linear".into());
        let mut g = expense("2026-02-02", "KO-2026-0006", "hardware", 60_000, 0);
        g.capitalized_asset_method = Some("gwg_sofort".into());
        let capitalized = vec![e, g];
        let inp = DatevInputs {
            capitalized: &capitalized,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!((b[0].konto, b[0].gegenkonto), (1200, 420));
        assert_eq!((b[1].konto, b[1].gegenkonto), (1200, 480));
    }

    /// Lauf F8b: Bei Regelbesteuerung geht auf das Anlagekonto nur der
    /// Nettobetrag (Zugang = Anschaffungskosten = AfA-Summe); die Vorsteuer
    /// zieht der BU-Schlüssel 9 bzw. die gesonderte Buchung aufs
    /// Vorsteuerkonto. Bei §19 geht der Bruttobetrag aufs Anlagekonto.
    #[test]
    fn capitalized_expense_under_regelbesteuerung_books_net_and_input_tax() {
        let mut e = regel(expense(
            "2026-02-01",
            "KO-2026-0005",
            "hardware",
            119_000,
            19_000,
        ));
        e.capitalized_asset_method = Some("linear".into());
        // 16 %: kein BU-Schlüssel → Netto aufs Anlagekonto, Vorsteuer gesondert.
        let mut g = regel(expense(
            "2026-02-02",
            "KO-2026-0006",
            "hardware",
            116_000,
            16_000,
        ));
        g.capitalized_asset_method = Some("linear".into());
        let capitalized = vec![e, g];
        let inp = DatevInputs {
            capitalized: &capitalized,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(
            (
                b[0].amount_cents,
                b[0].gegenkonto,
                b[0].bu_schluessel.as_str()
            ),
            (119_000, 420, "9")
        );
        assert_eq!((b[1].amount_cents, b[1].gegenkonto), (100_000, 420));
        assert_eq!(
            (b[2].amount_cents, b[2].gegenkonto),
            (16_000, Skr::Skr03.input_tax())
        );

        let klein = vec![ExpenseItem {
            is_kleinunternehmer: true,
            ..capitalized[0].clone()
        }];
        let inp = DatevInputs {
            capitalized: &klein,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(
            (
                b[0].amount_cents,
                b[0].gegenkonto,
                b[0].bu_schluessel.as_str()
            ),
            (119_000, 420, "")
        );
    }

    /// Summe der Geldkonto-Bewegungen (S positiv, H negativ).
    fn money_balance(b: &[DatevBooking], money: u32) -> i64 {
        b.iter()
            .filter(|x| x.konto == money)
            .map(|x| {
                if x.soll_haben == 'S' {
                    x.amount_cents
                } else {
                    -x.amount_cents
                }
            })
            .sum()
    }

    /// F5-fix: Ein negativer Anteil einer Steuergruppe (Gutschrift-Position)
    /// wird als Gegenbuchung ausgegeben, nicht verworfen — die Bank lässt sich
    /// abstimmen (Zahlung und Erstattung).
    #[test]
    fn negative_tax_group_share_is_booked_as_counter_entry() {
        let shares = vec![
            TaxShare {
                category_code: "S".into(),
                rate_basis_points: 1900,
                gross_cents: 11_900,
                net_cents: 10_000,
                tax_cents: 1_900,
            },
            TaxShare {
                category_code: "S".into(),
                rate_basis_points: 700,
                gross_cents: -1_070,
                net_cents: -1_000,
                tax_cents: -70,
            },
            TaxShare {
                category_code: "Z".into(),
                rate_basis_points: 0,
                gross_cents: 0,
                net_cents: 0,
                tax_cents: 0,
            },
        ];
        let income = vec![IncomeItem {
            paid_date: "2026-03-01".into(),
            invoice_number: "RE-2026-0003".into(),
            amount_cents: 10_830,
            is_kleinunternehmer: false,
            shares: shares.clone(),
            ..Default::default()
        }];
        let storno = vec![StornoItem {
            storno_date: "2026-04-01".into(),
            storno_number: "ST-2026-0001".into(),
            original_number: "RE-2026-0003".into(),
            refunded_cents: 10_830,
            is_kleinunternehmer: false,
            shares,
            account_type: None,
        }];
        let inp = DatevInputs {
            income: &income,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(b.len(), 2, "Nullanteil entfällt: {b:?}");
        assert_eq!((b[0].gegenkonto, b[0].soll_haben), (8400, 'S'));
        assert_eq!(
            (b[1].gegenkonto, b[1].soll_haben, b[1].amount_cents),
            (8300, 'H', 1_070)
        );
        assert_eq!(money_balance(&b, 1200), 10_830);

        let inp = DatevInputs {
            storno: &storno,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].gegenkonto, b[0].soll_haben), (8400, 'H'));
        assert_eq!((b[1].gegenkonto, b[1].soll_haben), (8300, 'S'));
        assert_eq!(money_balance(&b, 1200), -10_830);
    }

    #[test]
    fn build_bookings_skips_zero_and_unparsable() {
        let income = vec![
            klein_income("2026-03-01", "RE-1", 0, None),
            klein_income("kaputt", "RE-2", 1000, None),
            klein_income("2026-03-02", "RE-3", 5000, None),
        ];
        let inp = DatevInputs {
            income: &income,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].belegfeld1, "RE-3");
    }

    #[test]
    fn to_datev_emits_header_columns_and_rows() {
        let income = vec![klein_income("2026-03-01", "RE-2026-0001", 100_000, None)];
        let inp = DatevInputs {
            income: &income,
            ..empty_inputs()
        };
        let bookings = build_bookings(Skr::Skr03, 2026, &inp);
        let bytes = to_datev(&header(Skr::Skr03), &bookings);
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.split("\r\n").filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3); // Vorlauf + Spaltenkopf + 1 Buchung
        assert!(lines[0].starts_with("\"EXTF\";700;21;"));
        assert!(lines[1].starts_with("\"Umsatz (ohne Soll/Haben-Kz)\";"));
        assert!(lines[2].starts_with("1000,00;"));
    }

    /// ADR 0042 Nr. 9: Eine Gegenbuchung („Zahlung stornieren“) erscheint als
    /// Buchung mit umgekehrtem Soll/Haben auf dieselben Konten, mit Belegdatum
    /// und Belegfeld 1 der stornierten Zahlung; die Geldkonto-Bewegungen
    /// summieren sich wie die EÜR auf 0 — auch bei mehreren Steuergruppen.
    #[test]
    fn payment_reversal_books_inverse_with_same_accounts() {
        use crate::euer::detail::{split_payment, InvoiceTaxGroup};
        let mut payment = klein_income("2026-03-01", "RE-2026-0001", 50_000, Some("cash"));
        payment.reversed = true;
        let mut reversal = klein_income("2026-03-01", "RE-2026-0001", -50_000, Some("cash"));
        reversal.is_reversal = true;
        reversal.description = "Storno der Zahlung vom 01.03.2026: Betrag falsch".into();
        let income = vec![payment, reversal];
        let inp = DatevInputs {
            income: &income,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        assert_eq!(b.len(), 2, "die Gegenbuchung fällt nicht weg");
        assert_eq!(
            (b[0].konto, b[0].gegenkonto, b[0].soll_haben),
            (1000, 8195, 'S')
        );
        assert_eq!(
            (b[1].konto, b[1].gegenkonto, b[1].soll_haben),
            (1000, 8195, 'H')
        );
        assert_eq!(b[1].amount_cents, 50_000);
        assert_eq!(b[1].belegdatum, b[0].belegdatum);
        assert_eq!(b[1].belegfeld1, "RE-2026-0001");
        assert!(b[1].buchungstext.starts_with("Storno Zahlung RE-2026-0001"));

        // Regelbesteuerung, zwei Steuergruppen: Anteile exakt negiert.
        let groups = vec![
            InvoiceTaxGroup {
                category_code: "S".into(),
                rate_basis_points: 1900,
                net_cents: 10_001,
                tax_cents: 1_900,
            },
            InvoiceTaxGroup {
                category_code: "S".into(),
                rate_basis_points: 700,
                net_cents: 3_333,
                tax_cents: 233,
            },
        ];
        let regel = |cents: i64, rev: bool| IncomeItem {
            paid_date: "2026-04-01".into(),
            invoice_number: "RE-2026-0002".into(),
            amount_cents: cents,
            is_kleinunternehmer: false,
            shares: split_payment(&groups, cents),
            is_reversal: rev,
            reversed: !rev,
            ..Default::default()
        };
        let income = vec![regel(7_777, false), regel(-7_777, true)];
        let inp = DatevInputs {
            income: &income,
            ..empty_inputs()
        };
        let b = build_bookings(Skr::Skr03, 2026, &inp);
        let signed: i64 = b
            .iter()
            .map(|x| {
                if x.soll_haben == 'S' {
                    x.amount_cents
                } else {
                    -x.amount_cents
                }
            })
            .sum();
        assert_eq!(signed, 0, "Geldkonto netto wie die EÜR");
        assert_eq!(b.len() % 2, 0);
        let half = b.len() / 2;
        for k in 0..half {
            assert_eq!(b[k].gegenkonto, b[half + k].gegenkonto);
            assert_eq!(b[k].amount_cents, b[half + k].amount_cents);
            assert_ne!(b[k].soll_haben, b[half + k].soll_haben);
        }
    }
}
