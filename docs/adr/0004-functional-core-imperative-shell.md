# ADR 0004 — Functional Core / Imperative Shell

**Status:** Akzeptiert · 2026-05-19 · Block 1. (Decision-Log D-08)

## Kontext

Die fachlich kritischen Teile (Pflichtangaben-Prüfung, Totals, §19-Logik,
XRechnung-Aufbau, AfA, EÜR) müssen lückenlos und schnell testbar sein — ohne
DB, Dateisystem oder Java-Sidecar.

## Entscheidung

**Functional Core / Imperative Shell.** Reine, I/O-freie Funktionen bilden den
Kern: `domain::*`, `einvoice::generator`, `pdf::klausel_check`,
`depreciation::compute`, `euer::aggregate`. Alles mit Seiteneffekten lebt in der
Schale: `commands`, `db`, `archive`, `mail`, `scheduler`, Sidecar-Bridges,
`backup`, `migration_export`.

## Konsequenzen

- Der Kern ist mit gewöhnlichen Unit-Tests ohne Fixtures abdeckbar; die Schale
  wird mit Integration-Tests + Mock-Sidecar (`KLEIN_BUCH_SIDECAR_MOCK`) geprüft.
- Bridges trennen pures Bauen (`build_args`/`build_message`) vom I/O-Wrapper, so
  bleibt selbst die Schale teilweise pure-testbar (z. B. `mail::smtp::build_message`).
- Mehr Boilerplate an der Naht (Views/DTOs zwischen Core und Shell).

## Alternativen

| Option | Contra |
|---|---|
| Logik in Commands/Repos | Schwer testbar, DB-gebunden, vermischt I/O und Regeln |
| Aktive Records | Kopplung an DB, schlechte Reinheit der Geldlogik |
