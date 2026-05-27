# ADR 0026 — EÜR-Export: ELSTER-Ausfüllhilfe, DATEV-Buchungsstapel, Steuerberater-Paket

**Status:** Akzeptiert · 2026-05-21 · Block 14. Keine Migration (Schema v11).

## Kontext

Die fertige EÜR (ADR 0022) muss raus — und es gibt **zwei verschiedene
Zielgruppen** mit unterschiedlichen Anforderungen: Manuel gibt entweder **selbst
über ELSTER** ab oder reicht beim **Steuerberater** ein. ELSTER hat **keinen
CSV-Import** für die Anlage EÜR; DATEV ist der De-facto-Standard für Berater.
Leitprinzip (Manuel): **mehrere Optionen anbieten, der Default ist das
gesetzliche Minimum** — und der Export muss „den gesetzlichen Anspruch erfüllen",
nicht nur Summen liefern.

## Entscheidung

Drei aufeinander abgestimmte Export-Wege:

1. **ELSTER (Selbst-Abgabe) = Ausfüllhilfe.** Da kein CSV-Import existiert:
   `euer::elster_csv` mappt auf die **Anlage-EÜR-Zeilen** (§19; Einnahmen Z. 12,
   Veräußerung 19, AfA beweglich 33 / GWG 36, Restbuchwert 38, Auffang 60; nur
   Positionen ≠ 0) **plus** ein Typst-PDF **„Anlage EÜR {Jahr}"** (Anlage EÜR +
   AVEÜR + Einzelaufstellung **inkl. Beschreibung**). Bewusst **kein
   `window.print()`** — sauberes, app-chrome-freies Dokument.
2. **DATEV (Steuerberater) = EXTF-Buchungsstapel.** `euer::datev_csv`: Vorlauf
   `"EXTF";700;21;"Buchungsstapel";…`, **CP1252 + CRLF + Komma-Dezimal**,
   Belegdatum TTMM. **SKR03-Default**, SKR04 per Toggle. **Voller Buchungssatz
   inkl. AfA** (Einnahmen, Ausgaben je Kategorie, Storno, Verkauf, AfA zum
   31.12.). Konten sind ein **Vorschlag** (der Berater prüft).
3. **Steuerberater-Paket = ZIP:** Deckblatt-PDF + EÜR-PDF + DATEV-CSV +
   Einzel-CSVs (Einnahmen/Ausgaben/Anlageverzeichnis) + `stammdaten.json`.
4. **AfA-Safeguard:** Banner + „AfA jetzt buchen", wenn aktive Anlagen für das
   Jahr ungebucht sind — sonst fehlte die AfA still im Export.

## Konsequenzen

- Beide Zielgruppen werden bedient, Default bleibt das gesetzliche Minimum
  (ELSTER-Selbstabgabe).
- **ERiC-Direktübermittlung** an ELSTER bleibt **v0.2+** (großer Scope).
- **Bewusste Vereinfachungen (Steuerberater-Caveat):** Geldkonto fest
  Standard-Bank (Follow-up, bis `paid_*_account_id` verdrahtet ist),
  Anlage-/AfA-Gegenkonto generische BGA. Die Konten-Vorschläge sind vor der
  Verbuchung zu prüfen.

## Alternativen

| Option | Contra |
|---|---|
| Nur Summen-Export | erfüllt den „gesetzlichen Anspruch" der Anlage EÜR nicht (Einzelaufstellung/AVEÜR fehlen) |
| ERiC-Direktübermittlung jetzt | großer Integrations- + Zertifizierungs-Scope; auf v0.2+ vertagt |
| `window.print()` fürs EÜR-PDF | bringt App-Chrome/Seitenränder mit; Typst liefert ein sauberes, reproduzierbares Dokument |

## Referenzen

`euer::{elster_csv, datev_csv, detail}`, `pdf::templates::{DEFAULT_EUER_TEMPLATE,
DEFAULT_COVER_TEMPLATE}`, `typst_render::{render_euer, render_pdf}`,
`docs/reference/{elster-euer-formular-schema, datev-format}.md`, Frontend
`routes/euer/export`; ADR 0022 (EÜR-Aggregation). Commits `8d029732` / `85731b1e`
/ `31f78815` / `3d161c86`, Tag `v0.1.0-phase2c`.
