# ADR 0005 — §19-Kleinunternehmer als Hardline-Default

**Status:** Akzeptiert · 2026-05-19 · Block 2/3.

## Kontext

Zielgruppe sind §19-Kleinunternehmer (UStG). Sie weisen **keine** Umsatzsteuer
aus. Ein versehentlicher USt-Ausweis löst §14c UStG aus (Schuld der unrichtig
ausgewiesenen Steuer). Die Software muss das aktiv verhindern, nicht nur dem
Nutzer überlassen.

## Entscheidung

- Default `seller_profile.is_kleinunternehmer = true`. USt-Felder im UI gesperrt,
  alle Items `tax_category_code = 'E'` (Exempt), `tax_amount_cents = 0`.
- **§19-Klausel ist Pflichtangabe**, wortgleich: „Gemäß §19 UStG wird keine
  Umsatzsteuer ausgewiesen." — als BT-22-Note + BT-120 ExemptionReason in der
  CII-XML **und** sichtbar auf dem PDF.
- `pdf::klausel_check` lehnt vor dem Render jedes Template ohne §19-Marker ab.
- Backend erzwingt `assert_no_vat` (§14c-Schutz) in `validate_for_issue` —
  unabhängig von der UI.
- Verzicht auf §19 (Regelbesteuerung) nur über Settings-Toggle mit
  5-Jahres-Bindungs-Warndialog; danach läuft normale USt-Logik.

## Konsequenzen

- Versehentlicher USt-Ausweis ist mehrfach abgesichert (UI + Domain + Template).
- Der wortgleiche Hinweistext lebt zentral in `domain::kleinunternehmer`, nicht
  im Frontend.
- Regelbesteuerung ist möglich, aber bewusst reibungsbehaftet (Bindungs-Warnung).

## Referenzen

§19 UStG, §14c UStG, EN 16931 (BT-22/BT-120).

## Amendment 2026-10-31 — Statushistorie (ADR 0042, Lauf 2026.11.1)

Der Settings-Toggle ist abgelöst. Der Status nach § 19 UStG steht in der append-only Tabelle
`tax_status_changes` (Migration 0033) mit Wirkungsdatum, Grund und Bindungsende; ein Irrtum
wird mit Grund widerrufen, nicht gelöscht. Maßgeblich ist der Status am Leistungsdatum des
Belegs (bei Leistungszeitraum dessen Ende, sonst Rechnungsdatum; Kosten: Belegdatum).
Ein Verzicht nach § 19 Abs. 3 UStG bindet fünf Kalenderjahre einschließlich des
Wirkungsjahres, ein gesetzlicher Wegfall bindet nicht. Die Hardline bleibt: Solange ein
Beleg unter § 19 fällt, weist er keine Umsatzsteuer aus, trägt die Klausel (PDF und BT-22)
und der §14c-Schutz greift. `seller_profile.is_kleinunternehmer` und
`waived_paragraph_19_since` sind nur noch Spiegel. Einzelheiten: ADR 0042 Nr. 1–6, 17–21.
