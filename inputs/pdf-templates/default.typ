// Klein.Buch — Default-Rechnungs-Template
//
// Erwartet als injizierte Variable `data` mit folgendem Schema:
//   data.invoice.{number, date, delivery_date, due_date, currency,
//                 is_kleinunternehmer, net_amount, tax_amount, gross_amount}
//   data.invoice.items[].{position, description, quantity, unit, unit_price, net_amount, tax_rate, tax_category}
//   data.seller.{name, street, postal_code, city, tax_number, vat_id, email, phone, iban, bic}
//   data.buyer.{name, street, postal_code, city, vat_id, email}
//   data.kleinunternehmer.{hinweis_text}

// Geldformat in Cent → "1.234,56 €". Korrekt für negative Beträge (Storno):
// Vorzeichen wird abgespalten, dann der Absolutbetrag formatiert.
#let format_euro(cents) = {
  let neg = cents < 0
  let a = calc.abs(cents)
  let euros = calc.floor(a / 100)
  let rest = a - euros * 100
  [#if neg [-]#str(euros),#if rest < 10 [0]#str(rest) €]
}

// Menge ohne unnötige Nachkommastellen: 1.0 → "1", 2.5 → "2,5".
#let format_qty(q) = {
  let s = if calc.fract(q) == 0 { str(calc.round(q)) } else { str(q) }
  s.replace(".", ",")
}

// ISO-Datum "YYYY-MM-DD" → deutsches "DD.MM.YYYY".
#let format_date(d) = {
  let p = str(d).split("-")
  if p.len() == 3 { p.at(2) + "." + p.at(1) + "." + p.at(0) } else { str(d) }
}

// Block-3a-Hotfix: `json.decode(...)` parst In-Memory-Strings; `json(...)`
// würde den Inhalt als Dateipfad missinterpretieren.
#let data = json.decode(sys.inputs.at("data-json"))

#set page(
  paper: "a4",
  margin: (top: 2.5cm, bottom: 3.4cm, left: 2cm, right: 2cm),
  // Kleiner footer-descent: nutzbare Fußhöhe = bottom − footer-descent. Klein
  // halten, damit mehrere Konten-Zeilen im Impressum nicht beschnitten werden.
  footer-descent: 0.45cm,
  // Echter Seiten-Footer — sitzt auf JEDER Seite unten (nicht fester
  // Abstand zum letzten Text). GoBD: Pflichtangaben auf jeder Seite.
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
#show heading.where(level: 2): set text(size: 11pt, weight: "bold")

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
  // Logo-Platzhalter — lädt aus inputs/branding/logo.png falls vorhanden.
  [],
)

#v(1.5cm)

// === Empfänger-Adressblock ===
#text(size: 8pt)[#data.seller.name, #data.seller.street, #data.seller.postal_code #data.seller.city] \
#data.buyer.name \
#data.buyer.street \
#data.buyer.postal_code #data.buyer.city

#v(1.5cm)

// === Rechnungs-Header ===
#grid(
  columns: (1fr, auto),
  align: (left, right),
  [
    = Rechnung
    Rechnungsnummer: *#data.invoice.number* \
    Rechnungsdatum: #format_date(data.invoice.date) \
    #if data.invoice.at("delivery_date", default: none) != none [
      Leistungsdatum: #format_date(data.invoice.delivery_date) \
    ] else if data.invoice.at("delivery_date_fallback", default: none) != none [
      Leistungsdatum #data.invoice.delivery_date_fallback \
    ]
    #if data.invoice.at("due_date", default: none) != none [
      Fällig am: #format_date(data.invoice.due_date)
    ]
  ],
  [],
)

#v(1cm)

// === Item-Tabelle ===
#table(
  columns: (auto, 1fr, auto, auto, auto, auto),
  align: (right, left, right, right, right, right),
  stroke: 0.5pt,
  [*Pos*], [*Beschreibung*], [*Menge*], [*Einh.*], [*Einzelpreis*], [*Gesamt*],
  ..data.invoice.items.map(item => {
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

#v(1cm)

// === Beträge-Block ===
// §19: kein USt-Ausweis → nur Rechnungsbetrag. Regelbesteuerung → Netto/USt/Brutto.
#align(right)[
  #if data.invoice.is_kleinunternehmer [
    #table(
      columns: (auto, auto),
      align: (left, right),
      stroke: none,
      [*Rechnungsbetrag:*], [*#format_euro(data.invoice.gross_amount)*],
    )
  ] else [
    #table(
      columns: (auto, auto),
      align: (left, right),
      stroke: none,
      [Netto-Betrag:], [#format_euro(data.invoice.net_amount)],
      [USt-Betrag:], [#format_euro(data.invoice.tax_amount)],
      [*Brutto-Betrag:*], [*#format_euro(data.invoice.gross_amount)*],
    )
  ]
]

#v(1cm)

// §19-KLAUSEL-BLOCK: REQUIRED
#if data.invoice.is_kleinunternehmer [
  #text(weight: "bold")[#data.kleinunternehmer.hinweis_text]
]

#v(0.6cm)

// Bezahlt-/Zahlungshinweis (manuell je Rechnung, reiner Text) — z. B.
// „Betrag dankend bar erhalten am …". Keine EÜR-/XML-Wirkung.
#if data.invoice.at("payment_note", default: none) != none [
  #text(size: 9pt, weight: "bold")[#data.invoice.payment_note]
  #v(0.4cm)
]

// Kurzer Zahlungshinweis — die Bankdaten stehen im Fuß-Impressum. Wird AUCH bei
// gesetztem Bezahlt-Hinweis gezeigt (z. B. Teilzahlung: Restbetrag bleibt offen).
#if data.at("payment_accounts", default: ()).any(acc => acc.at("type", default: "other") == "bank" and acc.at("iban", default: none) != none) [
  #text(size: 9pt)[Bitte überweisen Sie den Rechnungsbetrag#if data.invoice.at("due_date", default: none) != none [ bis zum #format_date(data.invoice.due_date)] auf das unten genannte Konto. Verwendungszweck: *#data.invoice.number*.]
]

#v(0.6cm)
#text(size: 9pt, fill: luma(110))[Vielen Dank für Ihren Auftrag.]
