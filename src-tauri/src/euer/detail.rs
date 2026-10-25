//! Einzelaufstellung + Anlageverzeichnis (AVEÜR) für die EÜR (Functional Core,
//! Block 14a).
//!
//! Liefert die **prüfungssichere** Detailebene hinter der Anlage EÜR: jede
//! Einnahme, jede Kostenposition, jede AfA/Anlage, jede Veräußerung als eigene
//! Zeile (GoBD-Einzelaufzeichnung, § 4 Abs. 3 Satz 5 EStG für die Anlagen). Die
//! DTOs werden von der Shell ([`crate::db::repo::euer`]) befüllt; hier liegen nur
//! die DB-agnostischen Typen + die reinen CSV-Builder.
//!
//! Datumsfelder sind ISO-Strings (`YYYY-MM-DD`, von der Shell durchgereicht).
//! Beträge in Cent (i64).

use serde::{Deserialize, Serialize};

// ============================================================================
// Detail-Positionen
// ============================================================================

/// Anteil eines Zahlungsbetrags an einer Steuergruppe (Steuerart + Satz) der
/// zugrunde liegenden Rechnung (Lauf F5b, KB-0021). Eine Zahlung wird im
/// Verhältnis der Brutto-Gruppensummen auf die Gruppen verteilt, die Steuer je
/// Gruppe im Verhältnis Steuer/Brutto der Gruppe (Ist-Prinzip: vereinnahmte
/// Umsatzsteuer, § 11 Abs. 1 EStG, Anleitung zur Anlage EÜR Zeile 17).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxShare {
    /// Steuerart nach EN 16931 (UNTDID 5305): `S`, `Z`, `E`, `AE`, `K`, `G`, `O`, …
    pub category_code: String,
    /// Steuersatz in Basispunkten (19 % = 1900).
    pub rate_basis_points: i64,
    pub gross_cents: i64,
    pub net_cents: i64,
    pub tax_cents: i64,
}

/// Eine Steuergruppe einer Rechnung (Summe je Steuerart und Satz), Eingabe für
/// [`split_payment`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceTaxGroup {
    pub category_code: String,
    pub rate_basis_points: i64,
    pub net_cents: i64,
    pub tax_cents: i64,
}

/// Verteilt einen Zahlungsbetrag auf die Steuergruppen einer Rechnung
/// (größte Restbeträge an die letzte Gruppe, damit die Summe exakt stimmt).
/// Ohne Gruppen oder ohne Bruttosumme entsteht ein Anteil ohne Steuer
/// (Steuerart `E`).
pub fn split_payment(groups: &[InvoiceTaxGroup], amount_cents: i64) -> Vec<TaxShare> {
    let total_gross: i128 = groups
        .iter()
        .map(|g| g.net_cents as i128 + g.tax_cents as i128)
        .sum();
    if groups.is_empty() || total_gross == 0 {
        return vec![TaxShare {
            category_code: "E".into(),
            rate_basis_points: 0,
            gross_cents: amount_cents,
            net_cents: amount_cents,
            tax_cents: 0,
        }];
    }
    let mut out = Vec::with_capacity(groups.len());
    let mut allocated: i64 = 0;
    for (i, g) in groups.iter().enumerate() {
        let group_gross = g.net_cents as i128 + g.tax_cents as i128;
        let share = if i + 1 == groups.len() {
            amount_cents - allocated
        } else {
            div_round(amount_cents as i128 * group_gross, total_gross) as i64
        };
        allocated += share;
        let tax = if group_gross == 0 {
            0
        } else {
            div_round(share as i128 * g.tax_cents as i128, group_gross) as i64
        };
        out.push(TaxShare {
            category_code: g.category_code.clone(),
            rate_basis_points: g.rate_basis_points,
            gross_cents: share,
            net_cents: share - tax,
            tax_cents: tax,
        });
    }
    out
}

/// Ganzzahlige Division mit kaufmännischer Rundung (halbe Cent weg von 0).
fn div_round(num: i128, den: i128) -> i128 {
    if den == 0 {
        return 0;
    }
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    if num >= 0 {
        (num + den / 2) / den
    } else {
        -((-num + den / 2) / den)
    }
}

/// Ein Zahlungseingang (Betriebseinnahme) — Teilzahlungen je eigener Eintrag.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeItem {
    pub paid_date: String,
    pub invoice_number: String,
    pub customer: String,
    /// Leistungsbeschreibung (aus den Rechnungspositionen zusammengefasst).
    pub description: String,
    pub amount_cents: i64,
    /// Beim Festschreiben eingefrorener §19-Status der Rechnung (F5a). Er
    /// entscheidet über die Zeile der Anlage EÜR (12 bzw. 15–17) und das
    /// DATEV-Erlöskonto — unabhängig vom Status des Zahlungsjahres
    /// (Anleitung zur Anlage EÜR, Zeile 12).
    pub is_kleinunternehmer: bool,
    /// Aufteilung des Zahlungsbetrags auf die Steuergruppen der Rechnung.
    pub shares: Vec<TaxShare>,
    /// Typ des Zahlungskontos (`bank`, `cash`, `paypal`, `stripe`, `other`);
    /// `None` = kein Konto hinterlegt (DATEV: Bank, R2-011).
    pub account_type: Option<String>,
}

/// Eine Storno-Erstattung (negative Betriebseinnahme im Storno-Jahr).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StornoItem {
    pub storno_date: String,
    pub storno_number: String,
    pub original_number: String,
    pub refunded_cents: i64,
    /// §19-Status des Originals (ein Storno übernimmt ihn, F5a).
    pub is_kleinunternehmer: bool,
    /// Aufteilung der Erstattung auf die Steuergruppen des Originals.
    pub shares: Vec<TaxShare>,
    /// Typ des Zahlungskontos des Originals (R2-011).
    pub account_type: Option<String>,
}

/// Eine bezahlte Kostenposition (Betriebsausgabe, brutto).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseItem {
    pub paid_date: String,
    pub expense_number: String,
    pub vendor: String,
    /// Interner Kategorie-Code (Frontend mappt auf deutsches Label).
    pub category: String,
    /// Beschreibung des Geschäftsvorfalls.
    pub description: String,
    pub gross_cents: i64,
    /// Netto laut Beleg (bei Regelbesteuerung Zeilenwert, Anleitung zur
    /// Anlage EÜR „Betriebsausgaben“: netto, Vorsteuer in Zeile 57).
    pub net_cents: i64,
    /// Umsatzsteuer laut Beleg (bei Regelbesteuerung abziehbare Vorsteuer).
    pub tax_cents: i64,
    /// Typ des Zahlungskontos (R2-011); `None` = Bank.
    pub account_type: Option<String>,
    /// Nur für aktivierte Kosten (Anlage aus dieser Position): AfA-Methode
    /// der Anlage. Aktivierte Kosten sind keine Betriebsausgabe (die
    /// Anschaffungskosten wirken über die AfA, § 4 Abs. 3 Satz 3 EStG) und
    /// erscheinen nur im DATEV-Stapel auf dem Anlagekonto.
    pub capitalized_asset_method: Option<String>,
}

/// Eine Privatbewegung (Entnahme/Einlage) — EÜR-neutral, aber für den DATEV-
/// Buchungsstapel relevant, damit der Bank-Saldo beim Steuerberater stimmt
/// (SKR03 1800/1890, SKR04 2100/2180). R2-009.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementItem {
    pub movement_date: String,
    pub movement_number: String,
    /// `"entnahme"` (Geld aus dem Betrieb in den privaten Bereich) oder
    /// `"einlage"` (privates Geld in den Betrieb).
    pub movement_type: String,
    pub description: String,
    pub amount_cents: i64,
    /// Typ des Zahlungskontos (R2-011); `None` = Bank.
    pub account_type: Option<String>,
}

/// Eine Anlagen-Veräußerung/-Verschrottung.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisposalItem {
    pub disposal_date: String,
    pub asset_number: String,
    pub label: String,
    pub proceeds_cents: i64,
    pub residual_book_value_cents: i64,
    pub gain_loss_cents: i64,
    /// AfA-Methode der Anlage (DATEV: GWG- oder allgemeines Anlagekonto).
    pub depreciation_method: String,
}

/// ISO-Datum (`YYYY-MM-DD`, ggf. mit Zeitanteil) als `TT.MM.JJJJ` für PDF und
/// Anzeige (KB-0018). Nicht parsbare Werte bleiben unverändert.
pub fn de_date(iso: &str) -> String {
    let head = iso.get(..10).unwrap_or(iso);
    chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d")
        .map(|d| d.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

/// Eine Zeile des Anlageverzeichnisses (AVEÜR) für ein Geschäftsjahr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AveeurItem {
    pub asset_number: String,
    pub label: String,
    pub acquisition_date: String,
    /// Anschaffungs-/Herstellungskosten (netto, voll — vor Privatanteil).
    pub acquisition_cost_cents: i64,
    pub depreciation_method: String,
    pub useful_life_years: Option<f64>,
    pub business_share_percent: f64,
    /// AfA des betrachteten Geschäftsjahres.
    pub afa_year_cents: i64,
    pub book_value_start_cents: i64,
    pub book_value_end_cents: i64,
    pub disposed_in_year: bool,
    pub disposal_date: Option<String>,
    pub disposal_proceeds_cents: Option<i64>,
}

/// Eine Rechnungs- oder Storno-Nummer des Geschäftsjahres, die nie
/// festgeschrieben wurde (Lauf F2c): verworfener Entwurf (W14-032) oder noch
/// offener Entwurf. Erklärt dem Steuerberater die Lücken in der Nummernfolge
/// der beigelegten Belege (GoBD Rz. 58 ff.: Lücken dokumentiert).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnissuedNumberItem {
    pub number: String,
    pub is_storno: bool,
    pub invoice_date: String,
    /// Zeitpunkt des Verwerfens (Europe/Berlin, `YYYY-MM-DD HH:MM`); `None` =
    /// offener Entwurf.
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    pub gross_cents: i64,
}

// ============================================================================
// CSV-Helfer (deutsche Notation, BOM für Excel)
// ============================================================================

/// Cent → deutscher Dezimalbetrag ohne Tausendertrenner (`-123456` → `-1234,56`).
fn eur_de(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100).replace('.', ",")
}

/// CSV-Feld escapen (Semikolon-getrennt).
fn field(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn opt_life(v: Option<f64>) -> String {
    match v {
        Some(y) => format!("{y}").replace('.', ","),
        None => String::new(),
    }
}

/// Deutsches Label der AfA-Methode (für CSV/Anzeige).
pub fn method_label(method: &str) -> &'static str {
    match method {
        "linear" => "linear",
        "gwg_sofort" => "GWG-Sofortabschreibung",
        "computer_special_2021" => "Computer/Software (1 Jahr)",
        _ => "—",
    }
}

/// Cent → deutscher Eurobetrag **mit Tausenderpunkten + Symbol** für PDF/Anzeige
/// (`1234567` → `"12.345,67 €"`, `-100000` → `"-1.000,00 €"`).
pub fn eur_grouped(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let euros = abs / 100;
    let rem = abs % 100;
    let digits = euros.to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut grouped = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(*b as char);
    }
    format!("{sign}{grouped},{rem:02} €")
}

/// Deutsches Label einer Kosten-Kategorie (spiegelt `EXPENSE_CATEGORIES` im
/// Frontend). Kategorien sind durch den DB-CHECK fix; Unbekanntes → „Sonstiges".
pub fn category_label(code: &str) -> &'static str {
    match code {
        "office" => "Bürobedarf",
        "software" => "Software / Lizenzen",
        "hardware" => "Hardware",
        "travel" => "Reisekosten",
        "services" => "Fremdleistungen",
        "goods" => "Wareneinkauf",
        "communications" => "Telefon / Internet",
        "vehicle" => "Kfz / Fahrzeug",
        "rent" => "Miete / Raumkosten",
        "insurance" => "Versicherungen / Beiträge",
        "training" => "Fortbildung",
        "fees" => "Gebühren / Bankspesen",
        "marketing" => "Werbung / Marketing",
        _ => "Sonstiges",
    }
}

// ============================================================================
// CSV-Builder
// ============================================================================

/// Nicht festgeschriebene Nummern als CSV
/// (`Nummer;Art;Belegdatum;Status;Verworfen am;Grund;Betrag`, Lauf F2c).
pub fn unissued_numbers_csv(items: &[UnissuedNumberItem]) -> String {
    let mut out = String::from("\u{FEFF}Nummer;Art;Belegdatum;Status;Verworfen am;Grund;Betrag\n");
    for it in items {
        out.push_str(&format!(
            "{};{};{};{};{};{};{}\n",
            field(&it.number),
            if it.is_storno { "Storno" } else { "Rechnung" },
            field(&it.invoice_date),
            if it.voided_at.is_some() {
                "verworfen (nie ausgestellt)"
            } else {
                "offener Entwurf"
            },
            field(it.voided_at.as_deref().unwrap_or("")),
            field(it.void_reason.as_deref().unwrap_or("")),
            field(&eur_de(it.gross_cents)),
        ));
    }
    out
}

/// Einnahmen-Einzelaufstellung als CSV
/// (`Datum;Rechnungsnr.;Kunde;Beschreibung;Betrag`).
pub fn income_csv(items: &[IncomeItem]) -> String {
    let mut out = String::from("\u{FEFF}Datum;Rechnungsnr.;Kunde;Beschreibung;Betrag\n");
    let mut total = 0i64;
    for it in items {
        total += it.amount_cents;
        out.push_str(&format!(
            "{};{};{};{};{}\n",
            field(&it.paid_date),
            field(&it.invoice_number),
            field(&it.customer),
            field(&it.description),
            field(&eur_de(it.amount_cents)),
        ));
    }
    out.push_str(&format!("Summe;;;;{}\n", field(&eur_de(total))));
    out
}

/// Ausgaben-Einzelaufstellung als CSV
/// (`Datum;Beleg-Nr.;Lieferant;Kategorie;Beschreibung;Betrag`).
pub fn expenses_csv(items: &[ExpenseItem]) -> String {
    let mut out = String::from("\u{FEFF}Datum;Beleg-Nr.;Lieferant;Kategorie;Beschreibung;Betrag\n");
    let mut total = 0i64;
    for it in items {
        total += it.gross_cents;
        out.push_str(&format!(
            "{};{};{};{};{};{}\n",
            field(&it.paid_date),
            field(&it.expense_number),
            field(&it.vendor),
            field(&it.category),
            field(&it.description),
            field(&eur_de(it.gross_cents)),
        ));
    }
    out.push_str(&format!("Summe;;;;;{}\n", field(&eur_de(total))));
    out
}

/// Anlageverzeichnis (AVEÜR) als CSV.
pub fn aveeur_csv(items: &[AveeurItem], fiscal_year: i32) -> String {
    let mut out = String::from("\u{FEFF}");
    out.push_str(&format!(
        "AV-Nr.;Bezeichnung;Anschaffung;AK/HK (netto);Methode;Nutzungsdauer (Jahre);Betrieblich %;AfA {fiscal_year};Restwert Jahresanfang;Restwert Jahresende;Abgang am;Veräußerungserlös\n"
    ));
    let mut afa_total = 0i64;
    for it in items {
        afa_total += it.afa_year_cents;
        out.push_str(&format!(
            "{};{};{};{};{};{};{};{};{};{};{};{}\n",
            field(&it.asset_number),
            field(&it.label),
            field(&it.acquisition_date),
            field(&eur_de(it.acquisition_cost_cents)),
            field(method_label(&it.depreciation_method)),
            field(&opt_life(it.useful_life_years)),
            field(&format!("{}", it.business_share_percent).replace('.', ",")),
            field(&eur_de(it.afa_year_cents)),
            field(&eur_de(it.book_value_start_cents)),
            field(&eur_de(it.book_value_end_cents)),
            field(it.disposal_date.as_deref().unwrap_or("")),
            field(&it.disposal_proceeds_cents.map(eur_de).unwrap_or_default()),
        ));
    }
    out.push_str(&format!("Summe;;;;;;;{};;;;\n", field(&eur_de(afa_total))));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lauf F2c: verworfene und offene Nummern mit Status, Zeitpunkt, Grund.
    #[test]
    fn unissued_numbers_csv_lists_voided_and_open_drafts() {
        let csv = unissued_numbers_csv(&[
            UnissuedNumberItem {
                number: "R-2026-0007".into(),
                is_storno: false,
                invoice_date: "2026-03-01".into(),
                voided_at: Some("2026-03-02 09:15".into()),
                void_reason: Some("Kunde hat abgesagt; neu angelegt".into()),
                gross_cents: 12_345,
            },
            UnissuedNumberItem {
                number: "ST-2026-0002".into(),
                is_storno: true,
                invoice_date: "2026-04-01".into(),
                voided_at: None,
                void_reason: None,
                gross_cents: -500,
            },
        ]);
        let lines: Vec<&str> = csv.trim_start_matches('\u{FEFF}').lines().collect();
        assert_eq!(
            lines[0],
            "Nummer;Art;Belegdatum;Status;Verworfen am;Grund;Betrag"
        );
        assert_eq!(
            lines[1],
            "R-2026-0007;Rechnung;2026-03-01;verworfen (nie ausgestellt);2026-03-02 09:15;\
             \"Kunde hat abgesagt; neu angelegt\";123,45"
        );
        assert_eq!(
            lines[2],
            "ST-2026-0002;Storno;2026-04-01;offener Entwurf;;;-5,00"
        );
    }

    fn group(code: &str, bp: i64, net: i64, tax: i64) -> InvoiceTaxGroup {
        InvoiceTaxGroup {
            category_code: code.into(),
            rate_basis_points: bp,
            net_cents: net,
            tax_cents: tax,
        }
    }

    /// KB-0021: Vollzahlung ergibt exakt die Gruppenwerte, Teilzahlungen
    /// verteilen proportional, die Summe der Anteile ist immer der Betrag.
    #[test]
    fn split_payment_is_exact_and_proportional() {
        let groups = vec![
            group("S", 1900, 10_000, 1_900),
            group("S", 700, 10_000, 700),
        ];
        let full = split_payment(&groups, 22_600);
        assert_eq!(full[0].gross_cents, 11_900);
        assert_eq!((full[0].net_cents, full[0].tax_cents), (10_000, 1_900));
        assert_eq!((full[1].net_cents, full[1].tax_cents), (10_000, 700));

        for amount in [1, 333, 10_001, 22_599] {
            let parts = split_payment(&groups, amount);
            assert_eq!(parts.iter().map(|p| p.gross_cents).sum::<i64>(), amount);
            for p in &parts {
                assert_eq!(p.net_cents + p.tax_cents, p.gross_cents);
            }
        }
        let half = split_payment(&groups, 11_300);
        assert_eq!(half[0].gross_cents, 5_950);
        assert_eq!(half[0].tax_cents, 950);
        assert_eq!(half[1].gross_cents, 5_350);
        assert_eq!(half[1].tax_cents, 350);

        // §19-Rechnung bzw. ohne Gruppen: ein Anteil ohne Steuer.
        let none = split_payment(&[], 5_000);
        assert_eq!(none.len(), 1);
        assert_eq!((none[0].net_cents, none[0].tax_cents), (5_000, 0));
    }

    #[test]
    fn de_date_formats_iso_dates() {
        assert_eq!(de_date("2026-03-01"), "01.03.2026");
        assert_eq!(de_date("2026-12-31 10:00:00"), "31.12.2026");
        assert_eq!(de_date("kaputt"), "kaputt");
        assert_eq!(de_date(""), "");
    }

    #[test]
    fn eur_de_german_decimals() {
        assert_eq!(eur_de(0), "0,00");
        assert_eq!(eur_de(123_456), "1234,56");
        assert_eq!(eur_de(-5), "-0,05");
    }

    #[test]
    fn method_label_maps_known() {
        assert_eq!(method_label("linear"), "linear");
        assert_eq!(method_label("gwg_sofort"), "GWG-Sofortabschreibung");
        assert_eq!(
            method_label("computer_special_2021"),
            "Computer/Software (1 Jahr)"
        );
        assert_eq!(method_label("???"), "—");
    }

    #[test]
    fn eur_grouped_thousands_and_symbol() {
        assert_eq!(eur_grouped(0), "0,00 €");
        assert_eq!(eur_grouped(123_456), "1.234,56 €");
        assert_eq!(eur_grouped(1_234_567), "12.345,67 €");
        assert_eq!(eur_grouped(100_000_000), "1.000.000,00 €");
        assert_eq!(eur_grouped(-100_000), "-1.000,00 €");
        assert_eq!(eur_grouped(5), "0,05 €");
    }

    #[test]
    fn category_label_maps_known_and_fallback() {
        assert_eq!(category_label("software"), "Software / Lizenzen");
        assert_eq!(category_label("vehicle"), "Kfz / Fahrzeug");
        assert_eq!(category_label("other"), "Sonstiges");
        assert_eq!(category_label("future-unknown"), "Sonstiges");
    }

    #[test]
    fn income_csv_has_header_rows_and_sum() {
        let items = vec![
            IncomeItem {
                paid_date: "2026-01-15".into(),
                invoice_number: "RE-2026-0001".into(),
                customer: "ACME GmbH".into(),
                description: "Beratung".into(),
                amount_cents: 100_000,
                ..Default::default()
            },
            IncomeItem {
                paid_date: "2026-06-01".into(),
                invoice_number: "RE-2026-0002".into(),
                customer: "Beispiel AG".into(),
                description: "Wartung".into(),
                amount_cents: 50_000,
                ..Default::default()
            },
        ];
        let csv = income_csv(&items);
        assert!(csv.starts_with('\u{FEFF}'));
        assert!(csv.contains("Datum;Rechnungsnr.;Kunde;Beschreibung;Betrag"));
        assert!(csv.contains("RE-2026-0001"));
        assert!(csv.contains("Beratung"));
        assert!(csv.contains("1000,00"));
        assert!(csv.contains("Summe;;;;1500,00"));
    }

    #[test]
    fn expenses_csv_escapes_semicolons() {
        let items = vec![ExpenseItem {
            paid_date: "2026-03-01".into(),
            expense_number: "KO-2026-0001".into(),
            vendor: "Müller; Söhne".into(),
            category: "software".into(),
            description: "Lizenz".into(),
            gross_cents: 30_000,
            ..Default::default()
        }];
        let csv = expenses_csv(&items);
        assert!(csv.contains("Datum;Beleg-Nr.;Lieferant;Kategorie;Beschreibung;Betrag"));
        // Lieferant mit Semikolon muss gequotet sein.
        assert!(csv.contains("\"Müller; Söhne\""));
        assert!(csv.contains("Summe;;;;;300,00"));
    }

    #[test]
    fn aveeur_csv_has_year_in_header_and_afa_sum() {
        let items = vec![AveeurItem {
            asset_number: "AV-2026-0001".into(),
            label: "Notebook".into(),
            acquisition_date: "2026-01-10".into(),
            acquisition_cost_cents: 120_000,
            depreciation_method: "linear".into(),
            useful_life_years: Some(3.0),
            business_share_percent: 100.0,
            afa_year_cents: 40_000,
            book_value_start_cents: 120_000,
            book_value_end_cents: 80_000,
            disposed_in_year: false,
            disposal_date: None,
            disposal_proceeds_cents: None,
        }];
        let csv = aveeur_csv(&items, 2026);
        assert!(csv.contains("AfA 2026"));
        assert!(csv.contains("AV-2026-0001"));
        assert!(csv.contains("linear"));
        assert!(csv.contains("1200,00")); // AK
        assert!(csv.contains("400,00")); // AfA
        assert!(csv.contains("Summe;;;;;;;400,00"));
    }
}
