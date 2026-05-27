# ADR 0022 — EÜR-Einnahmen-Erfassung: Zufluss/Abfluss + Storno als negative Einnahme

**Status:** Akzeptiert · 2026-05-21 · Block 13.
**Korrigiert** die Storno-Behandlung aus ADR 0010 (siehe dort).

## Kontext

Block 13 baut die EÜR-Aggregation (`euer::aggregate`). ADR 0010 (Block-1-Planung),
PRD §6.19 und das frühe Memo `euer-cash-basis.md` gingen alle davon aus, dass ein
**Storno** sich „über seinen eigenen negativen `paid_amount` in derselben
Einnahmen-Summe verrechnet" — eine bereits bezahlte Rechnung würde durch eine
Rückzahlung mit negativem `paid_amount` im Storno-Beleg neutralisiert.

Diese Annahme ist im real gebauten Code **unmöglich**:

- `db::repo::invoices::record_payment` verlangt `amount_cents > 0` und verbietet
  Überzahlung (`new_paid <= gross`). Ein Storno-Beleg hat einen **negativen**
  Brutto-Betrag ⇒ er kann nie eine Zahlung tragen; sein `paid_amount_cents`
  bleibt immer 0.
- Eine **bezahlte** Rechnung **kann** storniert werden (`invoices_cancel` blockt
  nur Drafts, bereits Stornierte und Storno-von-Storno). Das Original behält
  `paid_amount`/`paid_at`, bekommt aber `status='canceled'`.

Damit ist offen, wie eine bezahlte, später stornierte Rechnung in der EÜR wirkt —
eine fachlich/rechtlich relevante Entscheidung (Manuel via AskUserQuestion am
2026-05-21, mit der Vorgabe „deine Empfehlung, wenn rechtskonform").

## Entscheidung

EÜR strikt nach **§11 EStG (Zufluss-/Abfluss-Prinzip)** in Verbindung mit
**§4 Abs. 3 EStG**:

1. **Einnahmen = tatsächliche Zahlungseingänge.** Quelle: ausgehende Rechnungen
   mit `is_storno_for IS NULL` und `paid_amount_cents > 0`. Jede Zahlung aus
   `payment_history_json` zählt im Jahr ihres `paid_date`
   (Teilzahlungen über den Jahreswechsel landen pro Zahlung im jeweiligen Jahr).
   Fallback ohne Historie: `paid_amount_cents` am `paid_at`. **Der Status
   (`canceled`) filtert NICHT raus** — wer in einem Jahr Geld erhalten hat, hat
   in dem Jahr einen Zufluss.
2. **Storno = negative Einnahme zum Storno-Datum.** Für jeden Storno-Beleg
   (`is_storno_for` gesetzt) wird der auf dem **Original** tatsächlich gezahlte
   Betrag (`o.paid_amount_cents`) als negativer Zufluss im Jahr des
   `invoice_date` des Storno-Belegs gegengerechnet (Abfluss/Erstattung).
3. **Kosten** = `expenses.gross_amount_cents` (brutto; §19-KU zieht keine
   Vorsteuer) mit `status='recorded'` und `paid_date IS NOT NULL`, am
   Zahlungsausgang, gruppiert nach `category`.
4. **AfA** = `depreciation_entries.depreciation_amount_cents` mit `fiscal_year`
   (Jahres-Größe, §7 EStG), separate Position.
5. **Anlagen-Veräußerung** = Erlös als Einnahme + Restbuchwert-Abgang als
   Ausgabe (Differenz = Veräußerungsgewinn/-verlust), zum `disposal_date`.
6. **Privatentnahmen/-einlagen** bleiben EÜR-neutral (ADR 0020).

Read-only Auswertung, **keine Migration** (Schema bleibt v11).

## Konsequenzen

- **Vergangene Jahre bleiben stabil.** Ein Storno in einem Folgejahr ändert die
  EÜR des Ursprungsjahres **nicht** rückwirkend — konsistent mit der
  GoBD-Periodenfestschreibung (§146 Abs. 4 AO). Der Same-Year-Fall (Zahlung und
  Storno im selben Jahr) saldiert sich korrekt auf 0.
- **Refund-Proxy:** Der erstattete Betrag wird mit `original.paid_amount` zum
  Storno-Datum angesetzt (man erstattet, was man erhalten hat; eine separate
  Refund-Cash-Erfassung gibt es in v0.1 nicht). Robust auch bei Korrektur per
  Storno + Neu-Rechnung, weil die neue Rechnung ihre eigene Zahlung trägt.
- **Anlagen-Verkauf nicht doppelt erfassen:** Der Erlös läuft über das Disposal;
  wird derselbe Verkauf zusätzlich als bezahlte Rechnung erfasst, zählt er
  doppelt. Die EÜR-UI weist darauf hin.
- Steuerberater-Caveat bleibt (PRD §10): Logik intern verifiziert (Unit- +
  Integrationstests), aber vor Echteinsatz fachlich gegenzuprüfen.

## Alternativen

| Option | Contra |
|---|---|
| Storno über Status ausschließen (PRD-§6.19-Wortlaut) | ein Storno im Folgejahr ändert die EÜR des Ursprungsjahres rückwirkend — GoBD-heikel, sobald das Jahr eingereicht ist |
| Storno ignorieren (nur Geldeingang) | überzeichnet die Einnahmen nach einer echten Erstattung; Rückzahlung müsste manuell als Kosten erfasst werden |
| Refund als eigenes Cash-Event modellieren | sauberste Lösung, aber Schema-/Zahlungs-Pfad-Erweiterung — Scope über Block 13 hinaus, auf später vertagt |

## Referenzen

`euer::aggregate` (Functional Core), `db::repo::euer`, `commands::euer`;
`memory/klein-buch/euer-cash-basis.md` (gegen den alten Stand korrigiert);
ADR 0010 (EÜR Cash-Basis, dessen Storno-Annahme hier korrigiert wird),
ADR 0020 (Privatbewegungen EÜR-neutral); §4 Abs. 3 EStG, §11 EStG,
§146 Abs. 4 AO. Commit `block-13: euer aggregate` (`6f5f3561`).
