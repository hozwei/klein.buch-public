# ADR 0025 — Anlagenverzeichnis + AfA als Functional Core

**Status:** Akzeptiert · 2026-05-21 · Block 12. Migrationen `0010_assets` /
`0011_depreciation`.

## Kontext

Für die EÜR braucht es ein Anlagenverzeichnis und planmäßige Abschreibung (AfA,
§7 EStG): lineare AfA über die Nutzungsdauer, **GWG-Sofortabschreibung**
(geringwertige Wirtschaftsgüter), die **Computer-Sonderregel** (digitale
Wirtschaftsgüter seit 2021 auf 1 Jahr abschreibbar), anteilige betriebliche
Nutzung (**Privatanteil**) und **Veräußerung** (Erlös + Restbuchwert-Abgang).
AfA-Sätze/Nutzungsdauern ändern sich per BMF-Tabelle und müssen pflegbar sein.

## Entscheidung

1. **AfA-Berechnung ist Functional Core** (`domain::depreciation`): pure,
   I/O-freie Funktionen → vollständig unit-testbar. Die Schale (`db::repo`,
   Commands) lädt/persistiert nur.
2. **AfA-Tabellen als JSON in `inputs/afa-tabellen.json`** (BMF-Stand) —
   menschen-maintained, von Manuel bei BMF-Updates editierbar (Hard-Rule:
   `inputs/` ist Maschinen-tabu).
3. **`assets` + `depreciation_entries`** (`0010`/`0011`). `UNIQUE(asset_id,
   fiscal_year)` ⇒ ein AfA-Lauf ist **idempotent**.
4. **Manueller AfA-Lauf** (Mensch stößt an) + `reset_asset` (Korrektur vor
   Festschreibung); automatischer Lauf zum GJ-Wechsel kommt in Block 15
   (ADR 0027).
5. **Immutability gestaffelt:** eine Anlage wird erst nach der **ersten
   gebuchten AfA** gesperrt; `disposal_*`/`book_value` sind **nicht** in der
   Immutability-Whitelist, damit eine spätere Veräußerung möglich bleibt. Die
   endgültige **GoBD-Festschreibung** erfolgt zum **Geschäftsjahres-Abschluss**
   (§146 AO, ADR 0027), nicht schon bei der Buchung.

## Konsequenzen

- Rechenlogik ist deterministisch testbar (GWG-Grenze, Computer-1-Jahr,
  Privatanteil, Monats-Pro-rata) ohne DB.
- BMF-Änderungen erfordern keinen Code-Release — nur die JSON in `inputs/`.
- Idempotenz schützt vor Doppelbuchung bei wiederholtem Lauf.
- Veräußerung bleibt nach AfA-Buchung möglich, ohne die Immutability zu brechen.

## Alternativen

| Option | Contra |
|---|---|
| Degressive AfA als Default | für KU unnötig komplex; linear/GWG/Computer deckt den Regelfall; degressiv ggf. v0.2 |
| AfA-Sätze hartkodiert | BMF-Updates erzwängen Code-Releases; widerspricht der `inputs/`-Pflege-Idee |
| Anlage sofort bei Anschaffung sperren | verhindert Korrekturen vor dem ersten Abschluss; Festschreibung gehört an den GJ-Abschluss |

## Referenzen

`domain::depreciation` (Functional Core), `assets::afa_tabellen`,
`db::repo::{assets, depreciation}`, Migrationen `0010_assets`/`0011_depreciation`,
`inputs/afa-tabellen.json`; ADR 0027 (Auto-AfA + GJ-Festschreibung), ADR 0022
(AfA in der EÜR). Commit `block-12` (`39e2ddee`).
