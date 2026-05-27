//! Template-Discovery + Metadata.
//!
//! Listet alle `.typ`-Files in `inputs/pdf-templates/` und prüft jedes
//! gegen [`crate::pdf::klausel_check::inspect`]. Wird für die Settings-UI
//! gebraucht (Phase 2D) sowie für die Validierung beim Issue (Block 3b).

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::pdf::klausel_check::{inspect, TemplateKlauselStatus};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateMeta {
    /// Slug (filename ohne `.typ` bzw. Built-in-Name).
    pub name: String,
    /// Absoluter Pfad — leer (`""`) bei eingebetteten Built-ins.
    pub path: PathBuf,
    /// §19-Kompatibilität.
    pub klausel_status: TemplateKlauselStatusDto,
    /// `true` = mitgelieferte Built-in-Vorlage; `false` = eigene Datei aus
    /// `inputs/pdf-templates/`. Ein `inputs/`-Override eines Built-in-Namens
    /// erscheint als `builtin = false` (die Datei gewinnt).
    pub builtin: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateKlauselStatusDto {
    pub has_marker: bool,
    pub uses_data_field: bool,
    pub is_klein_compatible: bool,
}

impl From<TemplateKlauselStatus> for TemplateKlauselStatusDto {
    fn from(s: TemplateKlauselStatus) -> Self {
        Self {
            has_marker: s.has_marker,
            uses_data_field: s.uses_data_field,
            is_klein_compatible: s.is_klein_compatible(),
        }
    }
}

/// Liest `inputs/pdf-templates/{name}.typ` und gibt den Quelltext zurück.
pub fn load_source(inputs_dir: &Path, template_name: &str) -> Result<String> {
    let path = inputs_dir
        .join("pdf-templates")
        .join(format!("{template_name}.typ"));
    std::fs::read_to_string(&path).map_err(|e| {
        Error::Config(format!(
            "template '{template_name}' nicht ladbar ({}): {e}",
            path.display()
        ))
    })
}

/// Lädt das Angebots-Template (Block 8): bevorzugt
/// `inputs/pdf-templates/quote.typ` (menschen-maintained Override), sonst das
/// eingebettete [`DEFAULT_QUOTE_TEMPLATE`].
///
/// **Warum eingebettet statt als inputs/-Datei:** `inputs/` ist nach Block 1
/// für Maschinen tabu (CLAUDE.md). Das Quote-Template muss aber mitgeliefert
/// werden, also lebt das Default im Binary. Manuel kann es jederzeit per
/// `inputs/pdf-templates/quote.typ` selbst überschreiben.
pub fn load_quote_source(inputs_dir: &Path) -> String {
    let path = inputs_dir.join("pdf-templates").join("quote.typ");
    std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_QUOTE_TEMPLATE.to_string())
}

/// Eingebettetes Default-Angebots-Template. Trägt den §19-Klausel-Marker und
/// nutzt `kleinunternehmer.hinweis_text` — besteht also den
/// [`crate::pdf::klausel_check`] für §19-Angebote. Schema: `data.quote.*`
/// (siehe [`crate::pdf::typst_render::build_quote_data_json`]).
pub const DEFAULT_QUOTE_TEMPLATE: &str = r#"// Klein.Buch — Eingebettetes Default-Angebots-Template (Block 8)
//
// Erwartet `data` mit:
//   data.quote.{number, date, valid_until, currency, is_kleinunternehmer,
//               net_amount, tax_amount, gross_amount}
//   data.quote.items[].{position, description, quantity, unit, unit_price, net_amount, tax_rate, tax_category}
//   data.seller.{name, street, postal_code, city, tax_number, vat_id, email}
//   data.buyer.{name, street, postal_code, city, vat_id, email}
//   data.kleinunternehmer.{hinweis_text}

#let format_euro(cents) = {
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}

#let format_qty(q) = {
  let s = if calc.fract(q) == 0 { str(calc.round(q)) } else { str(q) }
  s.replace(".", ",")
}

// ISO-Datum "YYYY-MM-DD" → deutsches "DD.MM.YYYY".
#let format_date(d) = {
  let p = str(d).split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { str(d) }
}

#let data = json.decode(sys.inputs.at("data-json"))

#set page(
  paper: "a4",
  margin: (top: 2.5cm, bottom: 3.4cm, left: 2cm, right: 2cm),
  footer-descent: 0.45cm,
  // Fuß = vollständiges Firmen-Impressum (auf jeder Seite, GoBD).
  footer: align(center)[
    #line(length: 100%, stroke: 0.4pt + luma(210))
    #v(1.2mm)
    #text(size: 8pt, fill: luma(90))[
      #data.seller.name#if data.seller.at("owner_name", default: none) != none [ · Inhaber: #data.seller.owner_name] · #data.seller.street · #data.seller.postal_code #data.seller.city \
      #if data.seller.at("phone", default: none) != none [Tel.: #data.seller.phone · ]E-Mail: #data.seller.email#if data.seller.at("tax_number", default: none) != none [ · St.-Nr.: #data.seller.tax_number]#if data.seller.at("vat_id", default: none) != none [ · USt-IdNr.: #data.seller.vat_id] \
      #{
        let accs = data.at("payment_accounts", default: ())
        let lines = ()
        for acc in accs {
          let t = acc.at("type", default: "other")
          if t == "bank" and acc.at("iban", default: none) != none {
            let holder = acc.at("holder", default: data.seller.name)
            let bic = if acc.at("bic", default: none) != none [ · BIC: #acc.bic] else []
            lines.push([Kontoinhaber: #holder · IBAN: #acc.iban#bic])
          } else if acc.at("details", default: none) != none {
            lines.push([#acc.label: #acc.details])
          }
        }
        lines.join(linebreak())
      }
    ]
  ],
)
#set text(font: "Liberation Sans", size: 10pt, lang: "de")
#show heading.where(level: 1): set text(size: 14pt, weight: "bold")

// === Header: Seller-Block ===
#grid(
  columns: (1fr, auto),
  align: (left, right),
  [
    #text(weight: "bold")[#data.seller.name] \
    #if data.seller.at("owner_name", default: none) != none [Inhaber: #data.seller.owner_name \ ]
    #data.seller.street \
    #data.seller.postal_code #data.seller.city
  ],
  [
    #if data.seller.at("logo_path", default: none) != none [
      #image(data.seller.logo_path, width: 4cm)
    ]
  ],
)

#v(1.5cm)

// === Empfänger-Adressblock ===
#text(size: 8pt)[#data.seller.name, #data.seller.street, #data.seller.postal_code #data.seller.city] \
#data.buyer.name \
#data.buyer.street \
#data.buyer.postal_code #data.buyer.city

#v(1.5cm)

// === Angebots-Header ===
#grid(
  columns: (1fr, auto),
  align: (left, right),
  [
    = Angebot
    Angebotsnummer: *#data.quote.number* \
    Angebotsdatum: #format_date(data.quote.date) \
    Gültig bis: #format_date(data.quote.valid_until)
  ],
  [],
)

#v(1cm)

// === Positions-Tabelle ===
#table(
  columns: (auto, 1fr, auto, auto, auto, auto),
  align: (right, left, right, right, right, right),
  stroke: 0.5pt,
  [*Pos*], [*Beschreibung*], [*Menge*], [*Einh.*], [*Einzelpreis*], [*Gesamt*],
  ..data.quote.items.map(item => {
    // P3: Titel in der Positionszeile + formatierter Body-Block darunter.
    let has_block = item.at("description_typst", default: none) != none
    let title = item.at("description_title", default: none)
    let label = if has_block and title != none and title != "" { title } else { item.description }
    let cells = (
      str(item.position),
      label,
      format_qty(item.quantity),
      item.unit,
      format_euro(item.unit_price),
      format_euro(item.net_amount),
    )
    if has_block {
      cells + (table.cell(colspan: 6, align: left, eval(item.description_typst, mode: "markup")),)
    } else { cells }
  }).flatten()
)

#v(1cm)

// === Beträge-Block ===
// §19: kein USt-Ausweis → nur Gesamtbetrag. Regelbesteuerung → Netto/USt/Gesamt.
#align(right)[
  #if data.quote.is_kleinunternehmer [
    #table(
      columns: (auto, auto),
      align: (left, right),
      stroke: none,
      [*Gesamtbetrag:*], [*#format_euro(data.quote.gross_amount)*],
    )
  ] else [
    #table(
      columns: (auto, auto),
      align: (left, right),
      stroke: none,
      [Netto-Betrag:], [#format_euro(data.quote.net_amount)],
      [USt-Betrag:], [#format_euro(data.quote.tax_amount)],
      [*Gesamtbetrag:*], [*#format_euro(data.quote.gross_amount)*],
    )
  ]
]

#v(1cm)

// §19-KLAUSEL-BLOCK: REQUIRED
#if data.quote.is_kleinunternehmer [
  #text(weight: "bold")[#data.kleinunternehmer.hinweis_text]
]

// Unterschriften-Block — nur wenn aktiviert.
#if data.quote.at("signature_enabled", default: false) [
  #v(12mm)
  #grid(columns: (1fr, 1fr), column-gutter: 1cm, align: (left, left),
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[#if data.quote.at("signature_path", default: none) != none [#place(bottom + left)[#image(data.quote.signature_path, height: 1.4cm)]]]
      #line(length: 85%, stroke: 0.5pt)
      #text(size: 9pt)[#data.seller.city, #format_date(data.quote.date) \ #data.seller.name (Auftragnehmer)]
    ],
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[]
      #line(length: 85%, stroke: 0.5pt)
      #text(size: 9pt)[Ort, Datum \ #data.buyer.name (Auftraggeber)]
    ],
  )
]

#v(0.5cm)
#text(size: 9pt, fill: luma(80))[Dieses Angebot ist freibleibend und bis zum genannten Datum gültig.]
"#;

/// Lädt das EÜR-Dokument-Template (Block 14a, Schritt 2): bevorzugt
/// `inputs/pdf-templates/euer.typ` (Override), sonst das eingebettete
/// [`DEFAULT_EUER_TEMPLATE`]. Wie beim Angebot: Default lebt im Binary, weil
/// `inputs/` für Maschinen tabu ist.
pub fn load_euer_source(inputs_dir: &Path) -> String {
    let path = inputs_dir.join("pdf-templates").join("euer.typ");
    std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_EUER_TEMPLATE.to_string())
}

/// Eingebettetes EÜR-Dokument-Template. Reines Anzeige-/Archivdokument (kein
/// §19-Klausel-Marker nötig — es ist keine Rechnung). Alle Beträge kommen vom
/// Caller bereits als formatierte Strings; das Template rechnet nicht.
pub const DEFAULT_EUER_TEMPLATE: &str = r##"// Klein.Buch — Eingebettetes EÜR-Dokument-Template (Block 14a, Schritt 2)
//
// Erwartet `data` (Beträge bereits als formatierte Strings):
//   data.{year, generatedAt, isKleinunternehmer, kleinunternehmerHinweis}
//   data.seller.{name, street, postalCode, city, taxNumber|null, vatId|null, email}
//   data.form.lines[].{zeile, bezeichnung, betrag, isEntry}
//   data.form.{incomeTotal, expenseTotal, surplus}
//   data.assets[].{assetNumber, label, acquisitionDate, acquisitionCost, method, afaYear, bookValueEnd, disposalNote}
//   data.income[].{paidDate, invoiceNumber, customer, description, amount}
//   data.storno[].{stornoDate, stornoNumber, originalNumber, amount}
//   data.expenses[].{paidDate, expenseNumber, vendor, category, description, amount}
//   data.disposals[].{disposalDate, assetNumber, label, proceeds, residual, gainLoss}
//   data.{incomeSum, expenseSum}

#let data = json.decode(sys.inputs.at("data-json"))
#let muted = luma(120)

#set page(
  paper: "a4",
  margin: (top: 1.8cm, bottom: 2cm, left: 1.6cm, right: 1.6cm),
  numbering: "1 / 1",
  footer: align(center)[#text(size: 7.5pt, fill: muted)[
    Klein.Buch · Anlage EÜR #str(data.year) · #data.seller.name · Werkzeug, kein Steuerberater
  ]],
)
#set text(font: "Liberation Sans", size: 9pt, lang: "de")
#set table(stroke: 0.4pt + luma(200), inset: 4pt)
#show heading.where(level: 1): set text(size: 15pt, weight: "bold")
#show heading.where(level: 2): set text(size: 11pt, weight: "bold")
#set heading(numbering: none)

#grid(
  columns: (1fr, auto),
  align: (left + bottom, right + bottom),
  [
    #text(size: 15pt, weight: "bold")[Anlage EÜR #str(data.year)] \
    #text(size: 9pt, fill: muted)[Einnahmen-Überschuss-Rechnung nach § 4 Abs. 3 EStG]
  ],
  [#text(size: 8pt, fill: muted)[Erstellt am #data.generatedAt]],
)

#v(2mm)
#text(weight: "bold")[#data.seller.name] \
#data.seller.street, #data.seller.postalCode #data.seller.city \
#if data.seller.at("taxNumber", default: none) != none [Steuernummer: #data.seller.taxNumber \ ]
#if data.seller.at("vatId", default: none) != none [USt-IdNr.: #data.seller.vatId \ ]
#data.seller.email

#v(3mm)
#line(length: 100%, stroke: 0.5pt + luma(180))

== 1. Anlage EÜR — Übertrag ins ELSTER-Formular
#text(size: 8pt, fill: muted)[Positionen mit Zeilennummer in „Mein ELSTER" eintragen; die Summen berechnet ELSTER selbst.]
#v(1mm)
#table(
  columns: (auto, 1fr, auto),
  align: (center + horizon, left, right),
  [*Zeile*], [*Position*], [*Betrag*],
  ..data.form.lines.map(l => {
    let c = if l.isEntry { black } else { muted }
    ([#text(fill: c)[#l.zeile]], [#text(fill: c)[#l.bezeichnung]], [#text(fill: c)[#l.betrag]])
  }).flatten()
)

== 2. Anlageverzeichnis (AVEÜR)
#if data.assets.len() == 0 [
  #text(fill: muted)[Keine Anlagegüter im Geschäftsjahr.]
] else [
  #table(
    columns: (auto, 1fr, auto, auto, auto, auto, auto),
    align: (center, left, center, right, left, right, right),
    [*AV-Nr.*], [*Bezeichnung*], [*Anschaffung*], [*AK/HK*], [*Methode*], [*AfA #str(data.year)*], [*Restwert Ende*],
    ..data.assets.map(a => (
      a.assetNumber,
      [#a.label#if a.disposalNote != "" [ #text(size: 7.5pt, fill: muted)[(#a.disposalNote)]]],
      a.acquisitionDate, a.acquisitionCost, a.method, a.afaYear, a.bookValueEnd,
    )).flatten()
  )
]

== 3. Einzelaufstellung — Betriebseinnahmen
#if (data.income.len() == 0 and data.storno.len() == 0) [
  #text(fill: muted)[Keine Zahlungseingänge im Geschäftsjahr.]
] else [
  #table(
    columns: (auto, auto, 1fr, 1fr, auto),
    align: (left, left, left, left, right),
    [*Datum*], [*Rechnungsnr.*], [*Kunde*], [*Beschreibung*], [*Betrag*],
    ..data.income.map(i => (i.paidDate, i.invoiceNumber, i.customer, i.description, i.amount)).flatten(),
    ..data.storno.map(s => {
      let r = rgb("#b91c1c")
      ([#text(fill: r)[#s.stornoDate]], [#text(fill: r)[#s.stornoNumber]], [#text(fill: r)[Storno zu #s.originalNumber]], [#text(fill: r)[—]], [#text(fill: r)[−#s.amount]])
    }).flatten()
  )
  #align(right)[#text(weight: "bold")[Summe Zahlungseingänge: #data.incomeSum]]
]

== 4. Einzelaufstellung — Betriebsausgaben
#if data.expenses.len() == 0 [
  #text(fill: muted)[Keine bezahlten Kosten im Geschäftsjahr.]
] else [
  #table(
    columns: (auto, auto, 1fr, auto, 1fr, auto),
    align: (left, left, left, left, left, right),
    [*Datum*], [*Beleg-Nr.*], [*Lieferant*], [*Kategorie*], [*Beschreibung*], [*Betrag*],
    ..data.expenses.map(e => (e.paidDate, e.expenseNumber, e.vendor, e.category, e.description, e.amount)).flatten()
  )
  #align(right)[#text(weight: "bold")[Summe Betriebsausgaben: #data.expenseSum]]
]

#if data.disposals.len() > 0 [
== 5. Anlagen-Veräußerungen
#table(
  columns: (auto, auto, 1fr, auto, auto, auto),
  align: (left, center, left, right, right, right),
  [*Datum*], [*AV-Nr.*], [*Bezeichnung*], [*Erlös*], [*Restwert*], [*Gewinn/Verlust*],
  ..data.disposals.map(d => (d.disposalDate, d.assetNumber, d.label, d.proceeds, d.residual, d.gainLoss)).flatten()
)
]

#v(4mm)
#if data.at("kleinunternehmerHinweis", default: none) != none [
  #block(fill: luma(245), inset: 6pt, radius: 3pt, width: 100%)[
    #text(weight: "bold")[#data.kleinunternehmerHinweis]
  ]
  #v(2mm)
]
#text(size: 7.5pt, fill: muted)[
  Klein.Buch ist ein Werkzeug, kein Steuerberater. Die Zeilen-Zuordnung der Anlage EÜR ist ein Vorschlag und vor Abgabe mit dem Steuerberater abzugleichen. Privatentnahmen und -einlagen sind in der EÜR nicht enthalten. Maßgeblich ist der Zahlungszeitpunkt (§ 11 EStG).
]
"##;

/// Lädt das Deckblatt-Template des Steuerberater-Pakets (Block 14c): bevorzugt
/// `inputs/pdf-templates/stb-cover.typ`, sonst [`DEFAULT_COVER_TEMPLATE`].
pub fn load_cover_source(inputs_dir: &Path) -> String {
    let path = inputs_dir.join("pdf-templates").join("stb-cover.typ");
    std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_COVER_TEMPLATE.to_string())
}

/// Eingebettetes Deckblatt für das Steuerberater-Paket. Schema (Beträge als
/// formatierte Strings): `data.{year, generatedAt, skr, incomeTotal,
/// expenseTotal, surplus}`, `data.seller.{name, street, postalCode, city,
/// taxNumber|null, vatId|null, email}`, `data.contents[]` (Datei-Liste).
pub const DEFAULT_COVER_TEMPLATE: &str = r##"// Klein.Buch — Deckblatt Steuerberater-Paket (Block 14c)

#let data = json.decode(sys.inputs.at("data-json"))
#let muted = luma(110)

#set page(
  paper: "a4",
  margin: 2.2cm,
  numbering: none,
  footer: align(center)[#text(size: 7.5pt, fill: muted)[Klein.Buch · Werkzeug, kein Steuerberater]],
)
#set text(font: "Liberation Sans", size: 10pt, lang: "de")

#v(1cm)
#text(size: 20pt, weight: "bold")[Steuerberater-Paket]
#linebreak()
#text(size: 14pt)[Einnahmen-Überschuss-Rechnung #str(data.year)]

#v(9mm)
#text(weight: "bold")[Mandant]
#linebreak()
#data.seller.name
#linebreak()
#data.seller.street, #data.seller.postalCode #data.seller.city
#if data.seller.at("taxNumber", default: none) != none [#linebreak() Steuernummer: #data.seller.taxNumber]
#if data.seller.at("vatId", default: none) != none [#linebreak() USt-IdNr.: #data.seller.vatId]
#linebreak()
#data.seller.email

#v(7mm)
#text(weight: "bold")[Ergebnis #str(data.year) (Cash-Basis, § 4 Abs. 3 EStG)]
#v(1mm)
#table(
  columns: (1fr, auto),
  align: (left, right),
  stroke: none,
  inset: 3pt,
  [Summe Betriebseinnahmen:], [#data.incomeTotal],
  [Summe Betriebsausgaben:], [#data.expenseTotal],
  [*Überschuss / Verlust:*], [*#data.surplus*],
)

#v(7mm)
#text(weight: "bold")[Inhalt des Pakets]
#v(1mm)
#list(..data.contents.map(c => [#c]))

#v(9mm)
#line(length: 100%, stroke: 0.5pt + luma(180))
#v(2mm)
#text(size: 8.5pt, fill: muted)[
  Kontenrahmen des DATEV-Buchungsstapels: #data.skr. Die Konten-Zuordnung ist ein Vorschlag und vor der Verbuchung zu prüfen. Erstellt am #data.generatedAt mit Klein.Buch — einem Werkzeug, keinem Steuerberater.
]
"##;

/// Lädt die DSGVO-Auskunft-Vorlage (Block 18). Optional über
/// `inputs/pdf-templates/dsgvo.typ` überschreibbar; sonst die eingebaute
/// [`DEFAULT_DSGVO_TEMPLATE`].
pub fn load_dsgvo_source(inputs_dir: &Path) -> String {
    let path = inputs_dir.join("pdf-templates").join("dsgvo.typ");
    std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_DSGVO_TEMPLATE.to_string())
}

/// Eingebaute Vorlage für die **DSGVO-Auskunft nach Art. 15** (Block 18).
/// Reines Anzeige-/Auskunftsdokument (kein §14-Beleg, keine §19-Klausel nötig).
/// Datenschema = serialisierter [`crate::domain::dsgvo::DataSubjectReport`]
/// (camelCase). Geld kommt als Integer-Cent; die Vorlage formatiert selbst.
pub const DEFAULT_DSGVO_TEMPLATE: &str = r##"// Klein.Buch — DSGVO-Auskunft nach Art. 15 (Block 18) — KEIN §14-Beleg
#let data = json.decode(sys.inputs.at("data-json"))
#let muted = luma(110)
#let accent = rgb("#176b87")

#let format_euro(cents) = {
  if cents == none { return [—] }
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}
#let format_date(d) = {
  if d == none { return "—" }
  let s = if d.len() >= 10 { d.slice(0, 10) } else { d }
  let p = s.split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { d }
}
#let dash(v) = if v == none [—] else [#v]
#let yesno(b) = if b [Ja] else [Nein]

#set page(
  paper: "a4",
  margin: 2cm,
  numbering: "1 / 1",
  footer: align(center)[#text(size: 7.5pt, fill: muted)[
    Klein.Buch · Auskunft nach Art. 15 DSGVO · Werkzeug, keine Rechtsberatung
  ]],
)
#set text(font: "Liberation Sans", size: 9.5pt, lang: "de")
#set heading(numbering: none)
#show heading.where(level: 1): it => [#v(4mm)#text(size: 13pt, fill: accent, weight: "bold")[#it.body]#v(1mm)]
#show heading.where(level: 2): it => [#v(2mm)#text(size: 10.5pt, weight: "bold")[#it.body]]

#text(size: 19pt, weight: "bold")[Auskunft nach Art. 15 DSGVO]
#linebreak()
#text(size: 10pt, fill: muted)[Erstellt am #data.generatedAt (Europe/Berlin)]

= Verantwortliche Stelle
#data.controller.name
#linebreak()
#data.controller.address
#if data.controller.at("taxNumber", default: none) != none [#linebreak() Steuernummer: #data.controller.taxNumber]
#if data.controller.at("vatId", default: none) != none [#linebreak() USt-IdNr.: #data.controller.vatId]
#if data.controller.at("email", default: none) != none [#linebreak() #data.controller.email]
#if data.controller.at("phone", default: none) != none [ · #data.controller.phone]

= Betroffene Person (gespeicherte Stammdaten)
#table(
  columns: (auto, 1fr),
  stroke: none,
  inset: 3pt,
  align: (left, left),
  [*Name*], [#data.subject.name],
  [*Art*], [#data.subject.contactType],
  [*Rechtsform*], [#dash(data.subject.legalForm)],
  [*Anschrift*], [#dash(data.subject.street), #dash(data.subject.postalCode) #dash(data.subject.city) (#data.subject.countryCode)],
  [*E-Mail*], [#dash(data.subject.email)],
  [*Telefon*], [#dash(data.subject.phone)],
  [*USt-IdNr.*], [#dash(data.subject.vatId)],
  [*Steuernummer*], [#dash(data.subject.taxNumber)],
  [*IBAN*], [#dash(data.subject.iban)],
  [*BIC*], [#dash(data.subject.bic)],
  [*E-Rechnung erwünscht*], [#yesno(data.subject.acceptsEinvoice)],
  [*Archiviert*], [#yesno(data.subject.archived)],
  [*Angelegt am*], [#data.subject.createdAt],
  [*Zuletzt geändert*], [#data.subject.updatedAt],
)

= Informationen zur Verarbeitung (Art. 15 Abs. 1)
#text(weight: "bold")[Verarbeitungszwecke]
#list(..data.processingInfo.purposes.map(p => [#p]))
#text(weight: "bold")[Rechtsgrundlagen]
#list(..data.processingInfo.legalBases.map(p => [#p]))
#text(weight: "bold")[Empfänger / Kategorien von Empfängern]
#list(..data.processingInfo.recipients.map(p => [#p]))
#text(weight: "bold")[Speicherdauer]

#data.processingInfo.retention

#text(weight: "bold")[Ihre Rechte]
#list(..data.processingInfo.rights.map(p => [#p]))
#text(weight: "bold")[Herkunft der Daten]

#data.processingInfo.dataSource

= Rechnungen
#if data.invoices.len() == 0 [
  #text(fill: muted)[Keine Rechnungen gespeichert.]
] else [
  #for inv in data.invoices [
    == Rechnung #inv.invoiceNumber
    #table(
      columns: (auto, 1fr, auto, 1fr),
      stroke: none, inset: 2.5pt,
      [*Datum*], [#format_date(inv.invoiceDate)], [*Status*], [#inv.status#if inv.isStorno [ (Storno)]],
      [*Leistungsdatum*], [#format_date(inv.deliveryDate)], [*Fällig*], [#format_date(inv.dueDate)],
      [*Netto*], [#format_euro(inv.netAmountCents)], [*Brutto*], [#format_euro(inv.grossAmountCents)],
      [*Bezahlt*], [#format_euro(inv.paidAmountCents) (#format_date(inv.paidAt))], [*Versandt*], [#format_date(inv.sentAt)],
    )
    #if inv.at("buyerSnapshot", default: none) != none [
      #text(size: 8.5pt, fill: muted)[Empfänger-Stand zur Rechnungszeit: #dash(inv.buyerSnapshot.name), #dash(inv.buyerSnapshot.street), #dash(inv.buyerSnapshot.postalCode) #dash(inv.buyerSnapshot.city)]
    ]
    #if inv.items.len() > 0 [
      #table(
        columns: (auto, 1fr, auto, auto, auto),
        align: (right, left, right, right, right),
        inset: 3pt,
        table.header([Pos.], [Beschreibung], [Menge], [Einzel], [Netto]),
        ..inv.items.map(it => (
          [#it.position], [#it.description], [#it.quantity #it.unitCode], [#format_euro(it.unitPriceCents)], [#format_euro(it.netAmountCents)],
        )).flatten()
      )
    ]
    #if inv.at("paymentNote", default: none) != none [
      #text(size: 8.5pt, fill: muted)[Hinweis auf dem Beleg: #inv.paymentNote]
    ]
  ]
]

= Angebote
#if data.quotes.len() == 0 [
  #text(fill: muted)[Keine Angebote gespeichert.]
] else [
  #for q in data.quotes [
    == Angebot #q.quoteNumber
    #table(
      columns: (auto, 1fr, auto, 1fr),
      stroke: none, inset: 2.5pt,
      [*Datum*], [#format_date(q.quoteDate)], [*Status*], [#q.status],
      [*Gültig bis*], [#format_date(q.validUntil)], [*Brutto*], [#format_euro(q.grossAmountCents)],
    )
    #if q.items.len() > 0 [
      #table(
        columns: (auto, 1fr, auto, auto, auto),
        align: (right, left, right, right, right),
        inset: 3pt,
        table.header([Pos.], [Beschreibung], [Menge], [Einzel], [Netto]),
        ..q.items.map(it => (
          [#it.position], [#it.description], [#it.quantity #it.unitCode], [#format_euro(it.unitPriceCents)], [#format_euro(it.netAmountCents)],
        )).flatten()
      )
    ]
  ]
]

= Kosten (Sie als Lieferant)
#if data.expenses.len() == 0 [
  #text(fill: muted)[Keine Kosten-Positionen gespeichert.]
] else [
  #table(
    columns: (auto, auto, 1fr, auto, auto),
    align: (left, left, left, left, right),
    inset: 3pt,
    table.header([Beleg-Nr.], [Datum], [Beschreibung], [Status], [Brutto]),
    ..data.expenses.map(e => (
      [#e.expenseNumber], [#format_date(e.expenseDate)], [#e.description], [#e.status], [#format_euro(e.grossAmountCents)],
    )).flatten()
  )
]

= Archivierte Dokumente
#if data.documents.len() == 0 [
  #text(fill: muted)[Keine archivierten Dokumente.]
] else [
  #table(
    columns: (auto, auto, 1fr, auto, auto),
    align: (left, left, left, right, center),
    inset: 3pt,
    table.header([Art], [Bezug], [Datei], [Bytes], [im ZIP]),
    ..data.documents.map(d => (
      [#d.kind], [#dash(d.relatedLabel)], [#d.fileName], [#str(d.sizeBytes)], [#yesno(d.bundled)],
    )).flatten()
  )
  #v(1mm)
  #text(size: 8pt, fill: muted)[Die Original-Dateien liegen — soweit „im ZIP = Ja" — im Unterordner „dokumente/" dieser Auskunft bei. SHA-256-Prüfsummen stehen in der beigefügten auskunft.json.]
]

= Versandprotokoll (E-Mail)
#if data.emails.len() == 0 [
  #text(fill: muted)[Keine Versandvorgänge protokolliert.]
] else [
  #table(
    columns: (auto, 1fr, 1fr, auto),
    align: (left, left, left, left),
    inset: 3pt,
    table.header([Datum], [Betreff], [Empfänger], [Status]),
    ..data.emails.map(m => (
      [#format_date(m.createdAt)], [#m.subject], [#m.toEmail], [#m.status],
    )).flatten()
  )
]

= Protokoll-Bezüge (Audit-Log)
#if data.auditEvents.len() == 0 [
  #text(fill: muted)[Keine Protokoll-Einträge.]
] else [
  #table(
    columns: (auto, 1fr, auto),
    align: (left, left, left),
    inset: 3pt,
    table.header([Zeitpunkt], [Aktion], [Objekt]),
    ..data.auditEvents.map(a => (
      [#a.timestampUtc], [#a.action], [#dash(a.entityType)],
    )).flatten()
  )
]

#v(6mm)
#line(length: 100%, stroke: 0.5pt + luma(180))
#v(2mm)
#text(size: 8pt, fill: muted)[#data.disclaimer]
"##;

/// Paket-Vorschau (Block P2b). Reine Editor-Vorschau einer Paket-Position —
/// KEIN Beleg, kein Archiv. Der formatierte Block kommt aus `domain::package::to_typst`
/// (AST-only, escaped) und wird via `eval(mode: "markup")` eingebettet. Genau diese
/// Mechanik nutzt später auch die Beleg-Position (P3).
pub const PACKAGE_PREVIEW_TEMPLATE: &str = r##"// Klein.Buch — Paket-Vorschau (Block P2b)
#let data = json.decode(sys.inputs.at("data-json"))
#let muted = luma(110)

#set page(paper: "a4", margin: 2.2cm,
  footer: align(center)[#text(size: 7.5pt, fill: muted)[Klein.Buch · Paket-Vorschau (kein Beleg)]])
#set text(font: "Liberation Sans", size: 10.5pt, lang: "de")

#text(size: 8.5pt, fill: muted)[Vorschau einer Paket-Position]
#v(2mm)
#text(size: 16pt, weight: "bold")[#data.title]
#v(3mm)
#line(length: 100%, stroke: 0.5pt + luma(180))
#v(3mm)

// Formatierter Beschreibungs-Block aus dem Markdown-Subset (AST-only + escaped).
#eval(data.body_typst, mode: "markup")

#v(5mm)
#line(length: 100%, stroke: 0.5pt + luma(180))
#v(2mm)
#text(weight: "bold")[Preis: #data.price]
#if data.is_klein [
  #v(3mm)
  #text(size: 8.5pt, style: "italic", fill: muted)[#data.klausel]
]
"##;

// ===========================================================================
// Block P4 — Paket-Katalog-Broschüre.
//
// KEIN §14-Beleg: render-on-demand aus den aktuellen Revisionen der gewählten
// Pakete, kein Nummernkreis, kein write-once-Archiv. Zeigt je Paket Titel +
// formatierten Markup-Block (`eval(mode:"markup")` über den AST-only/escaped
// `domain::package::to_typst`-Output) + Netto-Preis und einen INFORMATIVEN
// §19-Hinweis (kein Pflicht-BT-22). Schema = `build_package_catalog_data_json`
// (`crate::pdf::typst_render`).
// ===========================================================================

/// Eingebaute Broschüren-Vorlage „paketübersicht". Marketing-Dokument, kein Beleg.
pub const PACKAGE_CATALOG_TEMPLATE: &str = r##"// Klein.Buch — Paket-Katalog-Broschüre (Block P4) — KEIN §14-Beleg
#let data = json.decode(sys.inputs.at("data-json"))
#let muted = luma(110)
#let accent = rgb("#176b87")
#let format_euro(cents) = {
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}
#let format_date(d) = {
  let p = str(d).split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { str(d) }
}

#set page(
  paper: "a4",
  margin: (top: 2.2cm, bottom: 3.2cm, left: 2cm, right: 2cm),
  footer-descent: 0.45cm,
  footer: align(center)[
    #line(length: 100%, stroke: 0.4pt + luma(220))
    #v(1.2mm)
    #text(size: 8pt, fill: muted)[
      #data.seller.name#if data.seller.at("owner_name", default: none) != none [ · Inhaber: #data.seller.owner_name] · #data.seller.street · #data.seller.postal_code #data.seller.city \
      #if data.seller.at("phone", default: none) != none [Tel.: #data.seller.phone · ]E-Mail: #data.seller.email#if data.seller.at("tax_number", default: none) != none [ · St.-Nr.: #data.seller.tax_number]#if data.seller.at("vat_id", default: none) != none [ · USt-IdNr.: #data.seller.vat_id] \
      Unverbindliche Paket-Übersicht · kein Rechnungsbeleg
    ]
  ],
)
#set text(font: ("Liberation Sans", "Libertinus Serif"), size: 10.5pt, lang: "de")

// === Kopf: Absender + Logo ===
#grid(
  columns: (1fr, auto),
  align: (left + top, right + top),
  [
    #text(weight: "bold", fill: accent)[#data.seller.name] \
    #if data.seller.at("owner_name", default: none) != none [Inhaber: #data.seller.owner_name \ ]
    #data.seller.street \
    #data.seller.postal_code #data.seller.city
  ],
  [
    #if data.seller.at("logo_path", default: none) != none [
      #image(data.seller.logo_path, width: 4cm)
    ]
  ],
)

#v(8mm)

// === Optionale persönliche Anrede (nur beim Versand an einen Kontakt) ===
#if data.at("contact_name", default: none) != none [
  #text(size: 9pt, fill: muted)[Für: #data.contact_name]
  #v(2mm)
]

#text(size: 20pt, weight: "bold", fill: accent)[#data.title]
#v(1mm)
#text(size: 9pt, fill: muted)[Stand: #format_date(data.generated_at)]
#v(5mm)

// === Pakete (je: Titel + formatierter Block + Netto-Preis) ===
#for p in data.packages [
  #line(length: 100%, stroke: 0.5pt + luma(180))
  #v(2.5mm)
  #text(size: 13pt, weight: "bold")[#p.title]
  #v(1.5mm)
  #eval(p.body_typst, mode: "markup")
  #v(2mm)
  #text(weight: "bold")[Preis: #format_euro(p.price_cents)]
  #v(4.5mm)
]

#line(length: 100%, stroke: 0.5pt + luma(180))
#v(3mm)

// === §19-/USt-Hinweis (informativ — KEIN Pflicht-BT-22) ===
#if data.is_klein [
  #text(size: 8.5pt, style: "italic", fill: muted)[#data.klausel]
] else [
  #text(size: 8.5pt, style: "italic", fill: muted)[Alle Preise verstehen sich netto zzgl. der gesetzlichen Umsatzsteuer.]
]
"##;

// ===========================================================================
// Block 17a — auswählbare Built-in-Vorlagen.
//
// Jede ist **unified**: dasselbe `.typ` rendert Rechnung (`data.invoice`) ODER
// Angebot (`data.quote`) — es verzweigt intern über `data.at("invoice", …)`.
// Schema identisch zu `build_data_json` / `build_quote_data_json`
// (`crate::pdf::typst_render`). Jede trägt den §19-Marker
// (`crate::pdf::klausel_check::MARKER_COMMENT`) und rendert
// `kleinunternehmer.hinweis_text` — besteht also den klausel_check.
//
// Gebündelte Fonts (typst-assets): "Libertinus Serif" + "New Computer Modern".
// Es gibt KEIN Sans im Bundle → die Vorlagen differenzieren über Layout/Farbe/
// Gewicht, nicht über die Fontfamilie. "Liberation Sans" wird nur genutzt, wenn
// Manuel sie als Branding-Font nachreicht, sonst greift der Serifen-Fallback.
// ===========================================================================

/// Built-in „Modern" — Petrol-Akzent (#176b87, Wildbach-Marke), gefüllte
/// Tabellenkopf-Zeile, hervorgehobener Gesamtbetrag, §19-Klausel als Box.
pub const TEMPLATE_MODERN: &str = r##"// Klein.Buch — Built-in-Vorlage „Modern" (Block 17a) — unified Rechnung/Angebot
#let format_euro(cents) = {
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}
#let format_qty(q) = {
  let s = if calc.fract(q) == 0 { str(calc.round(q)) } else { str(q) }
  s.replace(".", ",")
}
#let data = json.decode(sys.inputs.at("data-json"))
#let is_invoice = data.at("invoice", default: none) != none
#let doc = if is_invoice { data.invoice } else { data.quote }
#let doc_title = if is_invoice { "Rechnung" } else { "Angebot" }
#let num_label = if is_invoice { "Rechnungsnummer" } else { "Angebotsnummer" }
#let format_date(d) = {
  let p = str(d).split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { str(d) }
}
#let accent = rgb("#176b87")

#set page(
  paper: "a4",
  margin: (top: 2.2cm, bottom: 3.4cm, left: 2cm, right: 2cm),
  footer-descent: 0.45cm,
  // Fuß = vollständiges Firmen-Impressum (auf jeder Seite).
  footer: align(center)[
    #line(length: 100%, stroke: 0.4pt + luma(220))
    #v(1.2mm)
    #text(size: 8pt, fill: luma(110))[
      #data.seller.name#if data.seller.at("owner_name", default: none) != none [ · Inhaber: #data.seller.owner_name] · #data.seller.street · #data.seller.postal_code #data.seller.city \
      #if data.seller.at("phone", default: none) != none [Tel.: #data.seller.phone · ]E-Mail: #data.seller.email#if data.seller.at("tax_number", default: none) != none [ · St.-Nr.: #data.seller.tax_number]#if data.seller.at("vat_id", default: none) != none [ · USt-IdNr.: #data.seller.vat_id] \
      #{
        let accs = data.at("payment_accounts", default: ())
        let lines = ()
        for acc in accs {
          let t = acc.at("type", default: "other")
          if t == "bank" and acc.at("iban", default: none) != none {
            let holder = acc.at("holder", default: data.seller.name)
            let bic = if acc.at("bic", default: none) != none [ · BIC: #acc.bic] else []
            lines.push([Kontoinhaber: #holder · IBAN: #acc.iban#bic])
          } else if acc.at("details", default: none) != none {
            lines.push([#acc.label: #acc.details])
          }
        }
        lines.join(linebreak())
      }
    ]
  ],
)
#set text(font: ("Liberation Sans", "Libertinus Serif"), size: 10pt, lang: "de")
#show heading.where(level: 1): set text(size: 20pt, weight: "bold", fill: accent)

// === Kopf: Absender + Logo ===
#grid(
  columns: (1fr, auto),
  align: (left + top, right + top),
  [
    #text(weight: "bold", fill: accent)[#data.seller.name] \
    #if data.seller.at("owner_name", default: none) != none [Inhaber: #data.seller.owner_name \ ]
    #data.seller.street \
    #data.seller.postal_code #data.seller.city
  ],
  [
    #if data.seller.at("logo_path", default: none) != none [
      #image(data.seller.logo_path, width: 4cm)
    ]
  ],
)

#v(1.3cm)

// === Empfänger ===
#text(size: 8pt, fill: luma(120))[#data.seller.name, #data.seller.street, #data.seller.postal_code #data.seller.city]
#v(2mm)
#text(weight: "bold")[#data.buyer.name] \
#data.buyer.street \
#data.buyer.postal_code #data.buyer.city

#v(1.1cm)

// === Titel + Eckdaten ===
#grid(
  columns: (1fr, auto),
  align: (left + bottom, right + bottom),
  [ = #doc_title ],
  [
    #set text(size: 9.5pt)
    #align(right)[
      #num_label: *#doc.number* \
      #if is_invoice [
        Rechnungsdatum: #format_date(doc.date) \
        #if doc.at("delivery_date", default: none) != none [Leistungsdatum: #format_date(doc.delivery_date) \ ] else if doc.at("delivery_date_fallback", default: none) != none [Leistungsdatum #doc.delivery_date_fallback \ ]
        #if doc.at("due_date", default: none) != none [Fällig am: #format_date(doc.due_date)]
      ] else [
        Angebotsdatum: #format_date(doc.date) \
        Gültig bis: #format_date(doc.valid_until)
      ]
    ]
  ],
)

#v(6mm)

// === Positionen ===
#table(
  columns: (auto, 1fr, auto, auto, auto, auto),
  align: (right, left, right, right, right, right),
  inset: 7pt,
  stroke: (bottom: 0.5pt + luma(220)),
  fill: (x, y) => if y == 0 { accent.lighten(85%) } else { none },
  [*Pos*], [*Beschreibung*], [*Menge*], [*Einh.*], [*Einzelpreis*], [*Gesamt*],
  ..doc.items.map(item => {
    // P3: Titel in der Positionszeile (Beschreibungs-Spalte) + formatierter
    // Body-Block (volle Breite) darunter. Ohne Markup = schmale Zelle.
    let has_block = item.at("description_typst", default: none) != none
    let title = item.at("description_title", default: none)
    let label = if has_block and title != none and title != "" { title } else { item.description }
    let cells = (
      str(item.position),
      label,
      format_qty(item.quantity),
      item.unit,
      format_euro(item.unit_price),
      format_euro(item.net_amount),
    )
    if has_block {
      cells + (table.cell(colspan: 6, align: left, eval(item.description_typst, mode: "markup")),)
    } else { cells }
  }).flatten()
)

#v(8mm)

// === Summen ===
#align(right)[
  #block(width: 7cm)[
    // §19: kein USt-Ausweis → nur Gesamt. Regelbesteuerung → Netto/USt + Summe.
    #if not doc.is_kleinunternehmer [
      #grid(columns: (1fr, auto), row-gutter: 4pt, align: (left, right),
        [Netto-Betrag:], [#format_euro(doc.net_amount)],
        [USt-Betrag:], [#format_euro(doc.tax_amount)],
      )
      #v(2mm)
      #line(length: 100%, stroke: 0.6pt + accent)
      #v(2mm)
    ]
    #grid(columns: (1fr, auto), align: (left, right),
      [#text(weight: "bold")[#if doc.is_kleinunternehmer [#if is_invoice [Rechnungsbetrag:] else [Gesamtbetrag:]] else [#if is_invoice [Brutto-Betrag:] else [Gesamtbetrag:]]]],
      [#text(weight: "bold", fill: accent, size: 12pt)[#format_euro(doc.gross_amount)]],
    )
  ]
]

#v(9mm)

// §19-KLAUSEL-BLOCK: REQUIRED
#if doc.is_kleinunternehmer [
  #block(fill: accent.lighten(90%), inset: 8pt, radius: 4pt, width: 100%)[
    #text(weight: "bold")[#data.kleinunternehmer.hinweis_text]
  ]
]

// Bezahlt-/Zahlungshinweis (manuell je Rechnung, reiner Text) — z. B.
// „Betrag dankend bar erhalten am …". Reine PDF-Angabe, keine EÜR-/XML-Wirkung.
#if is_invoice and doc.at("payment_note", default: none) != none [
  #v(5mm)
  #text(size: 9pt, weight: "bold")[#doc.payment_note]
]

// Kurzer Zahlungshinweis — die Bankdaten stehen im Fuß-Impressum. Wird AUCH bei
// gesetztem Bezahlt-Hinweis gezeigt (z. B. Teilzahlung: Restbetrag bleibt offen).
#if is_invoice and data.at("payment_accounts", default: ()).any(acc => acc.at("type", default: "other") == "bank" and acc.at("iban", default: none) != none) [
  #v(5mm)
  #text(size: 9pt)[Bitte überweisen Sie den Rechnungsbetrag#if doc.at("due_date", default: none) != none [ bis zum #format_date(doc.due_date)] auf das unten genannte Konto. Verwendungszweck: *#doc.number*.]
]

// Unterschriften-Block — nur Angebot, nur wenn aktiviert.
#if (not is_invoice) and doc.at("signature_enabled", default: false) [
  #v(10mm)
  #grid(columns: (1fr, 1fr), column-gutter: 1cm, align: (left, left),
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[#if doc.at("signature_path", default: none) != none [#place(bottom + left)[#image(doc.signature_path, height: 1.4cm)]]]
      #line(length: 85%, stroke: 0.5pt + accent)
      #text(size: 9pt)[#data.seller.city, #format_date(doc.date) \ #data.seller.name (Auftragnehmer)]
    ],
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[]
      #line(length: 85%, stroke: 0.5pt + accent)
      #text(size: 9pt)[Ort, Datum \ #data.buyer.name (Auftraggeber)]
    ],
  )
]

#v(5mm)
#text(size: 9pt, fill: luma(110))[
  #if is_invoice [Vielen Dank für Ihren Auftrag.] else [Dieses Angebot ist freibleibend und bis zum genannten Datum gültig.]
]
"##;

/// Built-in „Klassisch" — formelle Serife (Libertinus), zentrierter Briefkopf,
/// gesperrter Titel, vollflächige Tabellenlinien, kein Farbakzent.
pub const TEMPLATE_KLASSISCH: &str = r##"// Klein.Buch — Built-in-Vorlage „Klassisch" (Block 17a) — unified Rechnung/Angebot
#let format_euro(cents) = {
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}
#let format_qty(q) = {
  let s = if calc.fract(q) == 0 { str(calc.round(q)) } else { str(q) }
  s.replace(".", ",")
}
#let data = json.decode(sys.inputs.at("data-json"))
#let is_invoice = data.at("invoice", default: none) != none
#let doc = if is_invoice { data.invoice } else { data.quote }
#let doc_title = if is_invoice { "Rechnung" } else { "Angebot" }
#let num_label = if is_invoice { "Rechnungsnummer" } else { "Angebotsnummer" }
#let format_date(d) = {
  let p = str(d).split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { str(d) }
}

#set page(
  paper: "a4",
  margin: (top: 2.5cm, bottom: 3.4cm, left: 2.2cm, right: 2.2cm),
  footer-descent: 0.45cm,
  // Fuß = vollständiges Firmen-Impressum (auf jeder Seite).
  footer: align(center)[
    #line(length: 100%, stroke: 0.4pt + luma(200))
    #v(1.2mm)
    #text(size: 8pt, fill: luma(90))[
      #data.seller.name#if data.seller.at("owner_name", default: none) != none [ · Inhaber: #data.seller.owner_name] · #data.seller.street · #data.seller.postal_code #data.seller.city \
      #if data.seller.at("phone", default: none) != none [Tel.: #data.seller.phone · ]E-Mail: #data.seller.email#if data.seller.at("tax_number", default: none) != none [ · St.-Nr.: #data.seller.tax_number]#if data.seller.at("vat_id", default: none) != none [ · USt-IdNr.: #data.seller.vat_id] \
      #{
        let accs = data.at("payment_accounts", default: ())
        let lines = ()
        for acc in accs {
          let t = acc.at("type", default: "other")
          if t == "bank" and acc.at("iban", default: none) != none {
            let holder = acc.at("holder", default: data.seller.name)
            let bic = if acc.at("bic", default: none) != none [ · BIC: #acc.bic] else []
            lines.push([Kontoinhaber: #holder · IBAN: #acc.iban#bic])
          } else if acc.at("details", default: none) != none {
            lines.push([#acc.label: #acc.details])
          }
        }
        lines.join(linebreak())
      }
    ]
  ],
)
#set text(font: "Libertinus Serif", size: 11pt, lang: "de")

// === Zentrierter Briefkopf ===
#if data.seller.at("logo_path", default: none) != none [
  #align(center)[#image(data.seller.logo_path, width: 3.5cm)]
  #v(3mm)
]
#align(center)[
  #text(size: 13pt, weight: "bold")[#data.seller.name] \
  #if data.seller.at("owner_name", default: none) != none [#text(size: 9pt)[Inhaber: #data.seller.owner_name] \ ]
  #text(size: 9pt)[#data.seller.street · #data.seller.postal_code #data.seller.city]
]
#v(4mm)
#line(length: 100%, stroke: 0.8pt)
#v(7mm)

// === Empfänger ===
#text(size: 8pt, fill: luma(110))[#data.seller.name, #data.seller.street, #data.seller.postal_code #data.seller.city] \
#text(weight: "bold")[#data.buyer.name] \
#data.buyer.street \
#data.buyer.postal_code #data.buyer.city
#v(8mm)

// === Titel ===
#align(center)[#text(size: 16pt, weight: "bold", tracking: 2pt)[#upper(doc_title)]]
#v(5mm)

// === Eckdaten ===
#grid(columns: (1fr, 1fr), align: (left, right),
  [#num_label: *#doc.number*],
  [#if is_invoice [Rechnungsdatum] else [Angebotsdatum]: #format_date(doc.date)],
)
#if is_invoice [
  #align(right)[
    #if doc.at("delivery_date", default: none) != none [Leistungsdatum: #format_date(doc.delivery_date) \ ] else if doc.at("delivery_date_fallback", default: none) != none [Leistungsdatum #doc.delivery_date_fallback \ ]
    #if doc.at("due_date", default: none) != none [Fällig am: #format_date(doc.due_date)]
  ]
] else [
  #align(right)[Gültig bis: #format_date(doc.valid_until)]
]
#v(6mm)

// === Positionen — volle Linien ===
#table(
  columns: (auto, 1fr, auto, auto, auto, auto),
  align: (right, left, right, right, right, right),
  stroke: 0.5pt, inset: 6pt,
  [*Pos*], [*Beschreibung*], [*Menge*], [*Einh.*], [*Einzelpreis*], [*Gesamt*],
  ..doc.items.map(item => {
    // P3: Titel in der Positionszeile (Beschreibungs-Spalte) + formatierter
    // Body-Block (volle Breite) darunter. Ohne Markup = schmale Zelle.
    let has_block = item.at("description_typst", default: none) != none
    let title = item.at("description_title", default: none)
    let label = if has_block and title != none and title != "" { title } else { item.description }
    let cells = (
      str(item.position),
      label,
      format_qty(item.quantity),
      item.unit,
      format_euro(item.unit_price),
      format_euro(item.net_amount),
    )
    if has_block {
      cells + (table.cell(colspan: 6, align: left, eval(item.description_typst, mode: "markup")),)
    } else { cells }
  }).flatten()
)
#v(6mm)

// §19: kein USt-Ausweis → nur Gesamt. Regelbesteuerung → Netto/USt/Brutto.
#align(right)[#if doc.is_kleinunternehmer [
  #table(columns: (auto, auto), align: (left, right), stroke: none, inset: 4pt,
    [#text(weight: "bold")[#if is_invoice [Rechnungsbetrag:] else [Gesamtbetrag:]]], [#text(weight: "bold")[#format_euro(doc.gross_amount)]],
  )
] else [
  #table(columns: (auto, auto), align: (left, right), stroke: none, inset: 4pt,
    [Netto-Betrag:], [#format_euro(doc.net_amount)],
    [USt-Betrag:], [#format_euro(doc.tax_amount)],
    [#text(weight: "bold")[#if is_invoice [Brutto-Betrag:] else [Gesamtbetrag:]]], [#text(weight: "bold")[#format_euro(doc.gross_amount)]],
  )
]]
#v(8mm)

// §19-KLAUSEL-BLOCK: REQUIRED
#if doc.is_kleinunternehmer [
  #text(weight: "bold")[#data.kleinunternehmer.hinweis_text]
]

// Bezahlt-/Zahlungshinweis (manuell je Rechnung, reiner Text) — z. B.
// „Betrag dankend bar erhalten am …". Reine PDF-Angabe, keine EÜR-/XML-Wirkung.
#if is_invoice and doc.at("payment_note", default: none) != none [
  #v(5mm)
  #text(size: 9pt, weight: "bold")[#doc.payment_note]
]

// Kurzer Zahlungshinweis — die Bankdaten stehen im Fuß-Impressum. Wird AUCH bei
// gesetztem Bezahlt-Hinweis gezeigt (z. B. Teilzahlung: Restbetrag bleibt offen).
#if is_invoice and data.at("payment_accounts", default: ()).any(acc => acc.at("type", default: "other") == "bank" and acc.at("iban", default: none) != none) [
  #v(5mm)
  #text(size: 9pt)[Bitte überweisen Sie den Rechnungsbetrag#if doc.at("due_date", default: none) != none [ bis zum #format_date(doc.due_date)] auf das unten genannte Konto. Verwendungszweck: *#doc.number*.]
]

// Unterschriften-Block — nur Angebot, nur wenn aktiviert.
#if (not is_invoice) and doc.at("signature_enabled", default: false) [
  #v(10mm)
  #grid(columns: (1fr, 1fr), column-gutter: 1cm, align: (left, left),
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[#if doc.at("signature_path", default: none) != none [#place(bottom + left)[#image(doc.signature_path, height: 1.4cm)]]]
      #line(length: 85%, stroke: 0.5pt)
      #text(size: 9pt)[#data.seller.city, #format_date(doc.date) \ #data.seller.name (Auftragnehmer)]
    ],
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[]
      #line(length: 85%, stroke: 0.5pt)
      #text(size: 9pt)[Ort, Datum \ #data.buyer.name (Auftraggeber)]
    ],
  )
]
#v(4mm)
#text(size: 9pt, style: "italic")[
  #if is_invoice [Vielen Dank für Ihren Auftrag.] else [Dieses Angebot ist freibleibend und bis zum genannten Datum gültig.]
]
"##;

/// Built-in „Minimal" — reduziert: New Computer Modern, Haarlinien, viel
/// Weißraum, gedämpfte Labels, kein Farbakzent.
pub const TEMPLATE_MINIMAL: &str = r##"// Klein.Buch — Built-in-Vorlage „Minimal" (Block 17a) — unified Rechnung/Angebot
#let format_euro(cents) = {
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}
#let format_qty(q) = {
  let s = if calc.fract(q) == 0 { str(calc.round(q)) } else { str(q) }
  s.replace(".", ",")
}
#let data = json.decode(sys.inputs.at("data-json"))
#let is_invoice = data.at("invoice", default: none) != none
#let doc = if is_invoice { data.invoice } else { data.quote }
#let doc_title = if is_invoice { "Rechnung" } else { "Angebot" }
#let num_label = if is_invoice { "Rechnungsnummer" } else { "Angebotsnummer" }
#let format_date(d) = {
  let p = str(d).split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { str(d) }
}
#let muted = luma(130)

#set page(
  paper: "a4",
  margin: (top: 2cm, bottom: 3.2cm, left: 1.8cm, right: 1.8cm),
  footer-descent: 0.45cm,
  // Fuß = vollständiges Firmen-Impressum (auf jeder Seite).
  footer: align(center)[
    #line(length: 100%, stroke: 0.4pt + luma(220))
    #v(1mm)
    #text(size: 7.5pt, fill: muted)[
      #data.seller.name#if data.seller.at("owner_name", default: none) != none [ · Inhaber: #data.seller.owner_name] · #data.seller.street · #data.seller.postal_code #data.seller.city \
      #if data.seller.at("phone", default: none) != none [Tel.: #data.seller.phone · ]E-Mail: #data.seller.email#if data.seller.at("tax_number", default: none) != none [ · St.-Nr.: #data.seller.tax_number]#if data.seller.at("vat_id", default: none) != none [ · USt-IdNr.: #data.seller.vat_id] \
      #{
        let accs = data.at("payment_accounts", default: ())
        let lines = ()
        for acc in accs {
          let t = acc.at("type", default: "other")
          if t == "bank" and acc.at("iban", default: none) != none {
            let holder = acc.at("holder", default: data.seller.name)
            let bic = if acc.at("bic", default: none) != none [ · BIC: #acc.bic] else []
            lines.push([Kontoinhaber: #holder · IBAN: #acc.iban#bic])
          } else if acc.at("details", default: none) != none {
            lines.push([#acc.label: #acc.details])
          }
        }
        lines.join(linebreak())
      }
    ]
  ],
)
#set text(font: "New Computer Modern", size: 10pt, lang: "de")

// === Kopf ===
#grid(
  columns: (1fr, auto),
  align: (left + top, right + top),
  [
    #text(size: 11pt, weight: "bold")[#data.seller.name] \
    #if data.seller.at("owner_name", default: none) != none [#text(size: 8.5pt)[Inhaber: #data.seller.owner_name] \ ]
    #text(size: 8.5pt, fill: muted)[#data.seller.street, #data.seller.postal_code #data.seller.city]
  ],
  [
    #if data.seller.at("logo_path", default: none) != none [
      #image(data.seller.logo_path, width: 3.2cm)
    ]
  ],
)

#v(1.4cm)

#text(size: 8pt, fill: muted)[#data.seller.name, #data.seller.street, #data.seller.postal_code #data.seller.city] \
#data.buyer.name \
#data.buyer.street \
#data.buyer.postal_code #data.buyer.city

#v(1.2cm)

// === Titel + Eckdaten ===
#grid(
  columns: (1fr, auto),
  align: (left + horizon, right + horizon),
  [#text(size: 15pt, weight: "bold")[#doc_title]],
  [#text(size: 8.5pt, fill: muted)[
    #num_label: #doc.number \
    #if is_invoice [
      Rechnungsdatum: #format_date(doc.date) \
      #if doc.at("delivery_date", default: none) != none [Leistungsdatum: #format_date(doc.delivery_date) \ ] else if doc.at("delivery_date_fallback", default: none) != none [Leistungsdatum #doc.delivery_date_fallback \ ]
      #if doc.at("due_date", default: none) != none [Fällig am: #format_date(doc.due_date)]
    ] else [
      Angebotsdatum: #format_date(doc.date) \
      Gültig bis: #format_date(doc.valid_until)
    ]
  ]],
)

#v(7mm)

// === Positionen — nur Kopf-Unterstrich ===
#table(
  columns: (auto, 1fr, auto, auto, auto, auto),
  align: (right, left, right, right, right, right),
  stroke: (x, y) => if y == 0 { (bottom: 0.5pt + muted) } else { none },
  inset: 6pt,
  [#text(fill: muted)[Pos]], [#text(fill: muted)[Beschreibung]], [#text(fill: muted)[Menge]], [#text(fill: muted)[Einh.]], [#text(fill: muted)[Einzelpreis]], [#text(fill: muted)[Gesamt]],
  ..doc.items.map(item => {
    // P3: Titel in der Positionszeile (Beschreibungs-Spalte) + formatierter
    // Body-Block (volle Breite) darunter. Ohne Markup = schmale Zelle.
    let has_block = item.at("description_typst", default: none) != none
    let title = item.at("description_title", default: none)
    let label = if has_block and title != none and title != "" { title } else { item.description }
    let cells = (
      str(item.position),
      label,
      format_qty(item.quantity),
      item.unit,
      format_euro(item.unit_price),
      format_euro(item.net_amount),
    )
    if has_block {
      cells + (table.cell(colspan: 6, align: left, eval(item.description_typst, mode: "markup")),)
    } else { cells }
  }).flatten()
)

#v(7mm)

// §19: kein USt-Ausweis → nur Gesamt. Regelbesteuerung → Netto/USt/Brutto.
#align(right)[#if doc.is_kleinunternehmer [
  #grid(columns: (auto, auto), column-gutter: 14pt, row-gutter: 3pt, align: (left, right),
    [#text(weight: "bold")[#if is_invoice [Rechnungsbetrag:] else [Gesamtbetrag:]]], [#text(weight: "bold")[#format_euro(doc.gross_amount)]],
  )
] else [
  #grid(columns: (auto, auto), column-gutter: 14pt, row-gutter: 3pt, align: (left, right),
    [#text(fill: muted)[Netto-Betrag:]], [#format_euro(doc.net_amount)],
    [#text(fill: muted)[USt-Betrag:]], [#format_euro(doc.tax_amount)],
    [#text(weight: "bold")[#if is_invoice [Brutto-Betrag:] else [Gesamtbetrag:]]], [#text(weight: "bold")[#format_euro(doc.gross_amount)]],
  )
]]

#v(9mm)

// §19-KLAUSEL-BLOCK: REQUIRED
#if doc.is_kleinunternehmer [
  #text(size: 9pt)[#data.kleinunternehmer.hinweis_text]
]

// Bezahlt-/Zahlungshinweis (manuell je Rechnung, reiner Text) — z. B.
// „Betrag dankend bar erhalten am …". Reine PDF-Angabe, keine EÜR-/XML-Wirkung.
#if is_invoice and doc.at("payment_note", default: none) != none [
  #v(5mm)
  #text(size: 9pt, weight: "bold")[#doc.payment_note]
]

// Kurzer Zahlungshinweis — die Bankdaten stehen im Fuß-Impressum. Wird AUCH bei
// gesetztem Bezahlt-Hinweis gezeigt (z. B. Teilzahlung: Restbetrag bleibt offen).
#if is_invoice and data.at("payment_accounts", default: ()).any(acc => acc.at("type", default: "other") == "bank" and acc.at("iban", default: none) != none) [
  #v(5mm)
  #text(size: 9pt)[Bitte überweisen Sie den Rechnungsbetrag#if doc.at("due_date", default: none) != none [ bis zum #format_date(doc.due_date)] auf das unten genannte Konto. Verwendungszweck: *#doc.number*.]
]

// Unterschriften-Block — nur Angebot, nur wenn aktiviert.
#if (not is_invoice) and doc.at("signature_enabled", default: false) [
  #v(10mm)
  #grid(columns: (1fr, 1fr), column-gutter: 1cm, align: (left, left),
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[#if doc.at("signature_path", default: none) != none [#place(bottom + left)[#image(doc.signature_path, height: 1.4cm)]]]
      #line(length: 85%, stroke: 0.5pt + muted)
      #text(size: 9pt)[#data.seller.city, #format_date(doc.date) \ #data.seller.name (Auftragnehmer)]
    ],
    [
      #block(height: 1.5cm, width: 100%, below: 0pt)[]
      #line(length: 85%, stroke: 0.5pt + muted)
      #text(size: 9pt)[Ort, Datum \ #data.buyer.name (Auftraggeber)]
    ],
  )
]
#v(4mm)
#text(size: 8pt, fill: muted)[
  #if is_invoice [Vielen Dank für Ihren Auftrag.] else [Dieses Angebot ist freibleibend und bis zum genannten Datum gültig.]
]
"##;

/// Auswählbare Built-in-Vorlagen (Reihenfolge = Anzeige im Switcher).
/// `"default"` ist das mitgelieferte Standard-Paar (Rechnung: `inputs/
/// pdf-templates/default.typ`, Angebot: [`DEFAULT_QUOTE_TEMPLATE`]) und wird
/// NICHT hier, sondern über den `inputs/`-Scan gelistet.
pub const BUILTIN_UNIFIED_NAMES: &[&str] = &["modern", "klassisch", "minimal"];

/// Quelltext einer eingebetteten Unified-Built-in-Vorlage (oder `None`).
pub fn builtin_unified(name: &str) -> Option<&'static str> {
    match name {
        "modern" => Some(TEMPLATE_MODERN),
        "klassisch" => Some(TEMPLATE_KLASSISCH),
        "minimal" => Some(TEMPLATE_MINIMAL),
        _ => None,
    }
}

/// Löst die **Rechnungs**-Vorlage auf. Reihenfolge: `inputs/pdf-templates/
/// {name}.typ`-Override (gewinnt immer, deckt auch `default` → `default.typ` ab)
/// → eingebettete Unified-Built-in → sonst [`load_source`] (Fehler, wenn weder
/// Datei noch Built-in existiert).
pub fn resolve_invoice_template(inputs_dir: &Path, name: &str) -> Result<String> {
    let override_path = inputs_dir.join("pdf-templates").join(format!("{name}.typ"));
    if let Ok(s) = std::fs::read_to_string(&override_path) {
        return Ok(s);
    }
    if let Some(src) = builtin_unified(name) {
        return Ok(src.to_string());
    }
    load_source(inputs_dir, name)
}

/// Löst die **Angebots**-Vorlage auf. `default` (und unbekannte Namen) gehen NIE
/// über `default.typ` (das ist das Rechnungs-Template) — sie fallen auf
/// [`load_quote_source`] zurück (`quote.typ`-Override oder
/// [`DEFAULT_QUOTE_TEMPLATE`]). Für `modern`/`klassisch`/`minimal` gilt:
/// `inputs/{name}.typ`-Override gewinnt, sonst die eingebettete Unified-Vorlage.
pub fn resolve_quote_template(inputs_dir: &Path, name: &str) -> String {
    if name != "default" {
        let override_path = inputs_dir.join("pdf-templates").join(format!("{name}.typ"));
        if let Ok(s) = std::fs::read_to_string(&override_path) {
            return s;
        }
        if let Some(src) = builtin_unified(name) {
            return src.to_string();
        }
    }
    load_quote_source(inputs_dir)
}

/// Listet alle auswählbaren Vorlagen: eingebettete Unified-Built-ins
/// (`modern`/`klassisch`/`minimal`) **plus** alle `.typ`-Dateien aus
/// `inputs/pdf-templates/`. Ein `inputs/`-Override eines Built-in-Namens
/// gewinnt (die Datei ersetzt das Built-in im Ergebnis, `builtin = false`).
/// Das mitgelieferte `default`-Paar erscheint über `inputs/default.typ`.
pub fn list_templates(inputs_dir: &Path) -> Result<Vec<TemplateMeta>> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, TemplateMeta> = BTreeMap::new();

    // 1. Eingebettete Built-ins.
    for name in BUILTIN_UNIFIED_NAMES {
        let source = builtin_unified(name).unwrap_or("");
        map.insert(
            (*name).to_string(),
            TemplateMeta {
                name: (*name).to_string(),
                path: PathBuf::new(),
                klausel_status: inspect(source).into(),
                builtin: true,
            },
        );
    }

    // 2. inputs/pdf-templates/*.typ — überschreibt gleichnamige Built-ins.
    let dir = inputs_dir.join("pdf-templates");
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_typ = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("typ"))
                .unwrap_or(false);
            if !is_typ {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // `quote`/`euer`/`stb-cover` sind dokumenttyp-gebundene Sonder-
            // Overrides, keine im Switcher wählbaren Rechnungs-/Angebots-Layouts.
            if matches!(stem, "quote" | "euer" | "stb-cover") {
                continue;
            }
            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            map.insert(
                stem.to_string(),
                TemplateMeta {
                    name: stem.to_string(),
                    path: path.clone(),
                    klausel_status: inspect(&source).into(),
                    builtin: false,
                },
            );
        }
    }

    Ok(map.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_templates_merges_builtins_and_inputs_and_classifies() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("pdf-templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(
            templates.join("good.typ"),
            "// §19-KLAUSEL-BLOCK: REQUIRED\nkleinunternehmer.hinweis_text",
        )
        .unwrap();
        std::fs::write(templates.join("bad.typ"), "// kein Marker\n#text").unwrap();
        std::fs::write(templates.join("readme.md"), "ignored").unwrap();
        // Sonder-Overrides dürfen NICHT als wählbare Layouts auftauchen.
        std::fs::write(templates.join("quote.typ"), "// override").unwrap();

        let list = list_templates(dir.path()).unwrap();
        // 3 Built-ins + good + bad (readme.md + quote.typ ignoriert).
        assert_eq!(
            list.len(),
            5,
            "{:?}",
            list.iter().map(|m| &m.name).collect::<Vec<_>>()
        );

        for name in BUILTIN_UNIFIED_NAMES {
            let m = list.iter().find(|m| &m.name == name).unwrap();
            assert!(m.builtin, "{name} should be flagged builtin");
            assert!(
                m.klausel_status.is_klein_compatible,
                "built-in {name} must be §19-compatible"
            );
        }
        let good = list.iter().find(|m| m.name == "good").unwrap();
        assert!(good.klausel_status.is_klein_compatible);
        assert!(!good.builtin);
        let bad = list.iter().find(|m| m.name == "bad").unwrap();
        assert!(!bad.klausel_status.is_klein_compatible);
        assert!(list.iter().all(|m| m.name != "quote"));
    }

    #[test]
    fn list_templates_returns_builtins_for_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let list = list_templates(dir.path()).unwrap();
        // Ohne inputs/ bleiben mindestens die eingebetteten Built-ins.
        assert_eq!(list.len(), BUILTIN_UNIFIED_NAMES.len());
        assert!(list.iter().all(|m| m.builtin));
    }

    #[test]
    fn inputs_override_replaces_builtin_in_listing() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("pdf-templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(
            templates.join("modern.typ"),
            "// meine eigene moderne Vorlage",
        )
        .unwrap();

        let list = list_templates(dir.path()).unwrap();
        let modern = list.iter().find(|m| m.name == "modern").unwrap();
        assert!(
            !modern.builtin,
            "inputs/modern.typ override wins → builtin=false"
        );
        // Trotzdem genau ein „modern"-Eintrag (kein Duplikat).
        assert_eq!(list.iter().filter(|m| m.name == "modern").count(), 1);
    }

    #[test]
    fn all_builtin_templates_pass_klausel_check() {
        // §19-Hardline: jede wählbare Built-in-Vorlage MUSS den Marker tragen
        // und den Hinweis-Text rendern, sonst würde ein §19-Beleg abgelehnt.
        for name in BUILTIN_UNIFIED_NAMES {
            let src = builtin_unified(name).unwrap();
            assert!(
                inspect(src).is_klein_compatible(),
                "built-in '{name}' must carry §19 marker + hinweis_text"
            );
            // Unified: muss beide Dokumenttypen erkennen.
            assert!(
                src.contains("data.invoice"),
                "{name} must branch on invoice"
            );
            assert!(src.contains("data.quote"), "{name} must branch on quote");
        }
    }

    #[test]
    fn resolve_invoice_template_prefers_inputs_override() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("pdf-templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(templates.join("modern.typ"), "// override invoice").unwrap();
        let src = resolve_invoice_template(dir.path(), "modern").unwrap();
        assert!(src.contains("override invoice"));
    }

    #[test]
    fn resolve_invoice_template_falls_back_to_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let src = resolve_invoice_template(dir.path(), "klassisch").unwrap();
        assert!(src.contains("Klassisch"));
        assert!(src.contains("§19-KLAUSEL-BLOCK"));
    }

    #[test]
    fn resolve_invoice_template_default_uses_inputs_file() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("pdf-templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(
            templates.join("default.typ"),
            "// mein default rechnungs-template",
        )
        .unwrap();
        let src = resolve_invoice_template(dir.path(), "default").unwrap();
        assert!(src.contains("mein default rechnungs-template"));
    }

    #[test]
    fn resolve_invoice_template_errors_on_unknown() {
        let dir = tempfile::tempdir().unwrap();
        assert!(resolve_invoice_template(dir.path(), "doesnotexist").is_err());
    }

    #[test]
    fn resolve_quote_template_default_never_hits_invoice_default() {
        // Kollisions-Schutz: ein Angebot mit pdf_template='default' darf NICHT
        // das Rechnungs-Template inputs/default.typ laden.
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("pdf-templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(
            templates.join("default.typ"),
            "// RECHNUNGS-default, data.invoice only",
        )
        .unwrap();
        let src = resolve_quote_template(dir.path(), "default");
        assert!(!src.contains("RECHNUNGS-default"));
        assert!(src.contains("Angebot")); // embedded DEFAULT_QUOTE_TEMPLATE
    }

    #[test]
    fn resolve_quote_template_uses_unified_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let src = resolve_quote_template(dir.path(), "minimal");
        assert!(src.contains("Minimal"));
        assert!(src.contains("data.quote"));
    }

    #[test]
    fn load_source_errors_on_missing_template() {
        let dir = tempfile::tempdir().unwrap();
        let r = load_source(dir.path(), "doesnotexist");
        assert!(r.is_err());
    }

    #[test]
    fn embedded_quote_template_is_klein_compatible() {
        // §19-Hardline: das mitgelieferte Angebots-Template MUSS den
        // klausel_check bestehen, sonst würde ein §19-Angebot beim Render
        // abgelehnt.
        let st = inspect(DEFAULT_QUOTE_TEMPLATE);
        assert!(
            st.is_klein_compatible(),
            "embedded quote template must carry §19 marker + hinweis_text"
        );
    }

    #[test]
    fn load_quote_source_falls_back_to_embedded() {
        let dir = tempfile::tempdir().unwrap();
        let src = load_quote_source(dir.path());
        assert!(src.contains("Angebot"));
        assert!(src.contains("§19-KLAUSEL-BLOCK"));
    }

    #[test]
    fn load_quote_source_prefers_inputs_override() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("pdf-templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(
            templates.join("quote.typ"),
            "// custom override\n= Mein Angebot",
        )
        .unwrap();
        let src = load_quote_source(dir.path());
        assert!(src.contains("Mein Angebot"));
    }

    #[test]
    fn load_euer_source_falls_back_to_embedded() {
        let dir = tempfile::tempdir().unwrap();
        let src = load_euer_source(dir.path());
        assert!(src.contains("Anlage EÜR"));
        assert!(src.contains("data-json"));
    }
}
