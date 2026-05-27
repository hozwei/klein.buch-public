# ADR 0002 — App-Shell Tauri 2 + Rust-Backend

**Status:** Akzeptiert · 2026-05-19 · Block 1. (Decision-Log D-01, D-02)

## Kontext

Klein.Buch verarbeitet Steuerdaten und muss local-first, offline und mit kleinem
Footprint laufen. Optionen für die Desktop-Shell: Electron (Chromium gebündelt,
~150 MB Binary, JS-Backend), Tauri 2 (System-WebView, Rust-Backend) oder native
Toolkits. Für die Geschäftslogik braucht es Determinismus (Geldbeträge, GoBD,
E-Rechnung-Format).

## Entscheidung

**Tauri 2 als Shell, Rust als Backend.** Frontend in der System-WebView
(Svelte 5, ADR 0003-Umfeld). Geschäftslogik, DB-Zugriff, Krypto und Sidecar-
Aufrufe in Rust.

## Konsequenzen

- Kleine Bundles, geringer RAM-Bedarf, kein gebündeltes Chromium.
- Rust gibt Compile-Zeit-Sicherheit (sqlx-Checks, Typsystem) für die kritische
  Buchhaltungslogik.
- Preis: WebView-Unterschiede je OS müssen getestet werden; native-Cross-Builds
  brauchen CI-Matrix (siehe ADR 0001 für den Sidecar-Teil).
- IPC-Grenze Frontend↔Backend ist explizit (`commands::*`), was die
  Functional-Core/Shell-Trennung (ADR 0004) stützt.

## Alternativen

| Option | Contra |
|---|---|
| Electron | Großes Binary, JS-Backend ohne Compile-Sicherheit für Geldlogik |
| Reines Web + lokaler Server | Kein echtes local-first, Port-/Prozess-Management |
| Natives GUI (egui/GTK) | Höherer UI-Aufwand für formularlastige Masken |
