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

## Amendment 2026-10-23 — Web-/Docker-Betrieb (ADR 0039)

**Status:** Akzeptiert · Phase W. Die Annahme „Desktop-App ohne Server“ aus dem
Kontext gilt nicht mehr; Unlock-Gate (Pt. 2) und Catch-up (Pt. 4) bleiben.

- **Scheduler im Server, 24/7.** Der Tick läuft als Hintergrund-Task des
  Server-Prozesses (`tokio::spawn` + Abbruch-Token), nicht mehr nur, solange die
  Desktop-App geöffnet ist.
- **Nach einem Neustart gesperrt.** Nach Neustart oder Update des Containers
  bleibt die Instanz gesperrt (kein Pool, kein Scheduler), bis der Nutzer im
  Browser entsperrt — kein Auto-Unlock, die Passphrase wird nirgends abgelegt
  (ADR 0039, E3). Danach holt der Tick alles Fällige nach (Catch-up wie bisher).
  „Datenbank sperren“ stoppt den Scheduler, Restore und Passphrase-Wechsel
  pausieren ihn (Blöcke W6/W7, ADR 0035).
- **Tages-Backup im Tick** (umgesetzt, Block W2.4): fällig, sobald das letzte
  Backup älter als 24 h ist, nicht nur beim Entsperren; nach einem Fehlschlag
  frühestens nach 1 h erneut.
- **Abo-Rechnungen ohne Doppel** (umgesetzt, Block W1.4, ADR 0033): Sperre je
  Abo-Vorlage zwischen Tick und „Jetzt ausführen“, Vorrücken per
  Compare-and-Swap (`UPDATE … WHERE id = ? AND next_due_date = ?`).
- **Drop-Folder-Job entfällt** (ADR 0039, E11; Amendment ADR 0037): Er verlässt
  `run_tick` in Block W5, der Code entfällt in Block W11.

## Amendment 2026-10-24 — Dauerbetrieb (ADR 0040, Lauf F)

**Status:** Akzeptiert. Pt. 4 (Catch-up) und der Satz „holt der Tick alles
Fällige nach“ im Amendment 2026-10-23 gelten nur noch mit Grenze:

- **Nachholen begrenzt.** Bis drei fällige Termine je Abo bucht der Lauf selbst
  (je Termin eine Position mit dem Stichtag als Belegdatum, Sammelhinweis); bei
  mehr Terminen nur nach Bestätigung des Inhabers („Rückstand …“: nachholen,
  höchstens 24, oder ab heute fortsetzen). Das Fortsetzen eines pausierten
  Auto-Abos mit verpassten Terminen verlangt dieselbe Wahl.
- **Sperre, Compare-and-Swap, Transaktion** je Abo wie bei den Abo-Rechnungen;
  ein Fehler betrifft nur sein Abo (Hinweis), das Backup entsteht trotzdem.
- **Abgeschlossenes Geschäftsjahr:** Hinweis statt Warnung bei jedem Tick;
  „nachholen“ überspringt Termine in abgeschlossenen Jahren ohne Beleg (Audit).
- **Modus „Erinnerung“** (`auto_create_expense = 0`) erzeugt am Stichtag einen
  Hinweis „Abo fällig“ (einmal je Abo und Stichtag).
- **Pause** bei unplausibler Uhr bzw. nach einem Restore (ADR 0040 Nr. 5/6).
- Die Uhr kommt aus dem `AppCtx` (`clock::Clock`); der Abbruch-Token des
  Amendments 2026-10-23 ist die kooperative Abbruch-Anforderung
  `AppCtx::scheduler_stop` (ADR 0040 Nr. 9).
- **Tages-Backup kalenderfest** statt „fällig, sobald das letzte Backup älter
  als 24 h ist“ (Amendment 2026-10-23): genau eines je Berliner Kalendertag ab
  einer einstellbaren Uhrzeit (Standard 02:30), Backoff persistiert (ADR 0040
  Nr. 15).
- **Kein Job hängt am Start.** Der Kontext „Automatik läuft nur, während die App
  offen ist“ und der Catch-up „beim nächsten Start“ (Konsequenzen) beschreiben
  die Desktop-Fassung; im Server bestimmt jeder Job seine Fälligkeit aus
  persistierten Werten und der Uhr, der erste Tick folgt 60 s nach dem
  Entsperren (ADR 0040 Nr. 18).
