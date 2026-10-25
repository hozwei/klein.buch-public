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

## Amendment 2026-10-25 — Lauf F5b

Eine als Anlage aktivierte Kostenposition ist keine Betriebsausgabe mehr (sie zählte bisher
zusätzlich zur AfA); die Anschaffungskosten wirken nur über die AfA (§ 4 Abs. 3 Satz 3
EStG). Der Erinnerungswert von 1 Cent bleibt als Buchführungspraxis (keine Pflicht für die
EÜR, steuerneutral über den Restbuchwert beim Abgang). Einzelheiten: ADR 0039 Nr. 135.

## Amendment 2026-10-25 — Lauf F8

Anschaffungskosten folgen § 9b Abs. 1 EStG: bei §19-Kleinunternehmern brutto (die gezahlte
Umsatzsteuer ist nicht abziehbar), bei Regelbesteuerung netto; die frühere Vereinfachung
„immer netto“ entfällt, weil die aktivierte Kostenposition seit F5b nicht mehr selbst in die
EÜR eingeht. Die GWG-Grenze gilt für den Betrag ohne Umsatzsteuer (§ 6 Abs. 2 Satz 1 EStG,
R 9b Abs. 2 EStR). Aktivieren nur aus nicht stornierten Kosten in offenen Jahren, in einer
Transaktion. Die Abgangsart `given_away` („Privatentnahme / Geschenk“) trägt den Entnahmewert
(Teilwert, § 6 Abs. 1 Nr. 4 EStG) als Erlös. Einzelheiten: ADR 0039 Nr. 145.

## Amendment 2026-10-25 — Lauf F8d

Ob brutto (§19) oder netto (Regelbesteuerung), entscheidet der Steuerstatus bei
**Leistungsbezug** (Jahr des Belegdatums der Quell-Kosten), nicht der des Zahlungsjahres
(§ 15 Abs. 1 UStG, Abschn. 19.5 UStAE). Ein späterer Verzicht auf §19 (wirkt ab Beginn des
Kalenderjahres) oder eine Rückkehr wird nicht blockiert; Inbox und Ausfüllhilfe nennen
betroffene Anlagen mit dem Handlungsweg (Anschaffungskosten korrigieren, solange keine AfA in
einem abgeschlossenen Jahr festgeschrieben ist; sonst Steuerberatung). Einzelheiten: ADR 0039
Nr. 148.
