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
