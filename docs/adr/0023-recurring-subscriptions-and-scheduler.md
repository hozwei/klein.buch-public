# ADR 0023 — Wiederkehrende Belege + In-App-Scheduler

**Status:** Akzeptiert · 2026-05-20 · Block 10. Migration `0009_recurring`.

## Kontext

Kleinunternehmer haben wiederkehrende Posten (monatliche Wartung als Einnahme,
Abos/Mieten als Kosten). Diese sollen automatisch fällig werden, ohne dass
Manuel jeden Monat manuell anlegt. Klein.Buch ist eine **local-first
Desktop-App ohne Server** — es gibt keinen externen Cron, der laufen könnte,
während die App geschlossen ist. Außerdem ist die DB bis zum Passphrase-Unlock
gesperrt (Backup-Hardline), es darf also nicht vor dem Unlock geschrieben werden.

## Entscheidung

1. **`recurring_subscriptions`** (Migration `0009`) hält die Definitionen
   (Intervall, nächstes Fälligkeitsdatum, Vorlage für den zu erzeugenden Beleg).
2. **In-App-Scheduler** `scheduler::{tick, recurring}`: ein **5-Minuten-Tick**,
   der **erst nach dem Bootstrap** startet und **Unlock-gated** ist (kein
   Schreiben, solange die DB gesperrt ist).
3. **Auto-Anlage als Entwurf/aufgezeichnet mit `paid_date = NULL`** — die
   Maschine erzeugt den fälligen Beleg, der Mensch bestätigt Zahlung/Versand
   später (kein stilles Festschreiben oder Versenden).
4. **Catch-up:** war die App länger zu, holt der Tick alle verpassten Perioden
   nach (mehrere Belege, je eigenes Fälligkeitsdatum).
5. Weitere periodische Jobs (Reminder, Integritäts-Check, Auto-AfA) werden
   später in denselben `run_tick` eingehängt (siehe ADR 0027).

## Konsequenzen

- Kein plattformspezifischer OS-Scheduler nötig → bleibt local-first und
  portabel; Preis: Automatik läuft nur, **während die App offen ist** — der
  Catch-up gleicht Ausfallzeiten beim nächsten Start aus.
- Unlock-Gate verhindert Schreibzugriffe vor der Passphrase und respektiert die
  Backup-Hardline.
- `paid_date = NULL` hält die EÜR-Cash-Basis sauber (ADR 0022): ein auto-erzeugter
  Beleg zählt erst, wenn der Mensch die Zahlung erfasst.

## Alternativen

| Option | Contra |
|---|---|
| OS-Task-Scheduler (Windows Task Scheduler / cron) | plattformspezifisch, bricht local-first; läuft ohne entsperrte DB ins Leere |
| Nur On-Demand („jetzt fällige anlegen"-Knopf) | vergisst der Nutzer den Knopf, fehlen Belege; kein echter Automatik-Nutzen |
| Sofort festschreiben/versenden | verletzt das Prinzip „Maschine schlägt vor, Mensch bestätigt"; GoBD-/Versand-Risiko |

## Referenzen

`scheduler::{tick, recurring}`, `db::repo::recurring`, Migration
`0009_recurring`, Frontend `routes/expenses/recurring/*`; ADR 0027 (weitere
Cron-Jobs in `run_tick`), ADR 0022 (Cash-Basis). Commit `block-10` (`a743bb25`).
