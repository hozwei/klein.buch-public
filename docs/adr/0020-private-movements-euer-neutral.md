# ADR 0020 — Privatbewegungen: EÜR-neutral, append-only, kein Storno

**Status:** Akzeptiert · 2026-05-20 · Block 9.

## Kontext

Privatentnahmen/-einlagen (`private_movements`) bilden Geldflüsse zwischen
Geschäft und Privat ab. Sie sind **EÜR-neutral** — keine Betriebsausgabe/-einnahme
— und dienen nur der Vollständigkeit der Kasse. Das PRD-Schema definiert für
`private_movements` bewusst **keine** Status-/Storno-Spalten und (im Original)
keinen Immutability-Trigger.

## Entscheidung

- **EÜR-neutral:** Privatbewegungen werden in der EÜR-Aggregation (Block 13)
  ausgeklammert. Bestätigt im Domain-Doc und im UI-Hinweis.
- **Sofort-Festschreiben + Lock-Event:** `private_movements::create` setzt
  `locked_at = now`; Auto-Critical-Backup-Hook nach Anlegen.
- **Immutability-Trigger ergänzt (Abweichung vom PRD, die die GoBD-Hardline
  stärkt):** `trg_private_movements_immutable` schützt nach Lock die Kernfelder
  (movement_number, movement_date, amount_cents, movement_type, fiscal_year,
  account_id). Defense-in-depth; es existiert ohnehin keine Update-Route.
- **Kein Storno/Cancel:** Das Schema hat keine Status-Spalte. Eine Fehleingabe
  wird durch eine **Gegenbewegung** neutralisiert (eine Einlage gleicht eine
  versehentliche Entnahme aus) — append-only, kein Löschen.
- **Belegnummern** `PV-{YYYY}-{NNNN}` (eigener Counter `private_movement`).
  `amount_cents` ist für entnahme UND einlage positiv; die Richtung steckt im
  `movement_type`.

## Konsequenzen

- Saubere Trennung Geschäfts-/Privatgeld ohne EÜR-Verfälschung.
- Korrektur per Gegenbewegung ist etwas umständlicher als ein Edit, aber
  prüfungssicher und konsistent mit der GoBD-Hardline.
- Da kein Status existiert, taucht jede (auch korrigierte) Bewegung dauerhaft in
  der Liste auf — gewollt (Kassen-Vollständigkeit).

## Alternativen

| Option | Contra |
|---|---|
| Edit/Delete erlauben | verletzt GoBD-Hardline (kein Löschen, append-only) |
| Status-Spalte + Storno wie bei Kosten | PRD-Schema bewusst minimal; EÜR-neutral → geringeres Risiko |
| Kein Immutability-Trigger (PRD-Original) | schwächer; Trigger ist billig und blockt nichts (keine Update-Route) |

## Referenzen

`domain::private_movement`, `db::repo::private_movements`,
`commands::private_movements`, `0008_private_movements.sql`;
`memory/klein-buch/block-9-notes.md`.
