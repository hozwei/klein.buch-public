# ADR 0027 — Notifications, prüfungssicherer GJ-Abschluss, Integritäts-Cron

**Status:** Akzeptiert · 2026-05-22 · Block 15. Migrationen `0012_notifications`
/ `0013_fiscal_year_locks` (Schema v13).

## Kontext

Drei zusammenhängende Themen: (1) **Erinnerungen** (Belege erfassen, Backup
überfällig, überfällige Rechnungen, GJ-Abschluss fällig); (2) ein
**prüfungssicherer Geschäftsjahres-Abschluss** nach §146 Abs. 4 AO (ein
abgeschlossenes Jahr darf nicht mehr verändert werden); (3) ein **Archiv-
Integritäts-Check** als wiederkehrender Job (SHA-256-Re-Hash, Tamper-Detection).

## Entscheidung

1. **Notifications** (`0012`): `notification_rules` + `notifications` mit
   **Dedup-Unique-Index**. Die **In-App-Inbox ist die Quelle der Wahrheit**,
   zusätzlich OS-native über `tauri-plugin-notification`. Zentrales `notify::emit`
   mit **`dedup_key`** (INSERT OR IGNORE → kein Doppel-Hinweis). 4 aktive
   Default-Regeln (monatlicher Beleg-Check, GJ-Abschluss-fällig, Backup
   überfällig, Rechnung überfällig).
2. **Prüfungssicherer GJ-Lock** (`0013`, no-update/no-delete-Trigger).
   `fiscal_year::close_year`: AfA buchen → Abschreibung + Anlagen sperren →
   EÜR-Snapshot ins Festschreibungsprotokoll → Audit → Auto-Critical-Backup.
   **Nur abgelaufene Jahre**, Backup-Unlock-Pflicht, **unumkehrbar**.
   `guard::ensure_year_open` sitzt in den **Command-Wrappern** (Kosten,
   Rechnungs-Issue + Zahlung, Privatbewegungen, Angebote). **Storno bleibt
   möglich** (cancel/run_lock_pipeline ungeguarded) — Korrekturen müssen auch
   nach Abschluss gehen.
3. **Auto-AfA am 01.01.** ist **Default an, abschaltbar**
   (`depreciation_auto_year_close`).
4. **Scheduler-Jobs** in `run_tick` (ADR 0023): Reminder, monatlicher
   Integritäts-Check (Dedup über `started_at`, urgent-Notification bei Fail),
   Auto-AfA zum Jahreswechsel — jeder Job isoliert.
5. `os_native::push` ist im Test-Build per `cfg(test)` gegated (der WinRT-Toast-
   Pfad ließ sonst das Lib-Test-Binary unter Windows nicht laden —
   `STATUS_ENTRYPOINT_NOT_FOUND`).

## Konsequenzen

- Abgeschlossene Jahre sind DB-seitig unveränderlich → konsistent mit der
  GoBD-Periodenfestschreibung und der EÜR-Stabilität aus ADR 0022.
- Storno nach Abschluss bleibt erlaubt (wirkt als negative Einnahme im
  Storno-Jahr, ADR 0022) — der prüfungssichere Lock verhindert nur stille
  Änderungen, nicht legitime Korrekturen über neue Belege.
- Dedup verhindert Hinweis-Spam; OS-native ist additiv, die App bleibt ohne
  OS-Berechtigung voll funktionsfähig (Inbox).

## Alternativen

| Option | Contra |
|---|---|
| Soft-Lock (nur UI-Sperre) | nicht prüfungssicher; DB-Schreibzugriff bliebe technisch möglich |
| Storno nach Abschluss ebenfalls sperren | verhindert legitime Korrekturen; Storno ist GoBD-konform (neuer Beleg, keine Änderung) |
| Nur OS-Notifications | ohne OS-Berechtigung/Fokus verloren; eine persistente In-App-Inbox ist die belastbare Quelle |

## Referenzen

`notify::{store, rules, os_native, emit}`, `fiscal_year::{lock, transition,
guard}`, `scheduler::{reminders, integrity_check_cron, depreciation_year_close}`,
`db::repo::app_settings`, `system::{audit_trail_list, archive_integrity_*}`,
Migrationen `0012`/`0013`; ADR 0006 (GoBD-Immutability), ADR 0023 (Scheduler),
ADR 0025 (AfA), ADR 0022 (EÜR). Commit `block-15`.

## Amendment 2026-10-24 — Dauerbetrieb, nur noch die Inbox (Lauf F, ADR 0040)

**Status:** Akzeptiert · Lauf F (Block F1). Keine Migration. Ändert Pt. 1, 3, 4
und 5 für den Server-Betrieb (ADR 0039):

- **Nur die Inbox (Pt. 1, Pt. 5).** Die OS-Benachrichtigung über
  `tauri-plugin-notification` ist mit Block W5 entfallen; seit Lauf F (D-16)
  gibt es auch den übrig gebliebenen Weiterleitungs-Trait `Notifier`
  (`InboxOnly`) nicht mehr. `notify::emit` schreibt nur in die Inbox, idempotent
  über `dedup_key`. Die Spalte `notification_rules.deliver_os_native` bleibt
  (Migrationen sind forward-only), wird aber nicht gelesen. Pt. 5
  (`cfg(test)`-Gate des WinRT-Pfads) ist gegenstandslos.
- **Integritäts-Cron (Pt. 4)** mit gemeinsamer Sperre für Monats-Cron und
  „Jetzt prüfen“, 6 h Backoff nach einem Fehlschlag, Hinweis je Monat und
  Abbruch je Datei beim Sperren oder Herunterfahren; der erste Tick nach dem
  Entsperren lässt den Vollscan aus (ADR 0040 Nr. 17 und 18).
- **Auto-AfA (Pt. 3, Pt. 4)** je Anlage und Jahr atomar (Eintrag, Restbuchwert,
  Audit), Fehler mit Hinweis und persistiertem 6-h-Backoff; während einer Pause
  der automatischen Belegerzeugung (Uhrsprung, Restore) ruht sie (ADR 0040
  Nr. 5, Umsetzung 6 und 9).
- **Erinnerungen:** „Backup überfällig“ rechnet mit dem Berliner Datum des
  Backups; nach mehr als zwei Tagen ohne Aktivität meldet das Entsperren die
  Lücke als Hinweis (ADR 0040 Nr. 18). Kosten-Abos im Modus „Erinnerung“
  erzeugen am Stichtag den Hinweis „Abo fällig“ (ADR 0040, Umsetzung 8).
