# ADR 0033 — Wiederkehrende Ausgangsrechnungen (Abo-Rechnungen)

**Status:** Akzeptiert · 2026-05-24 · Phase 4 (Blöcke RI-1..RI-3c). Migrationen `0024_recurring_invoices`, `0025_recurring_invoice_item_position_unique`.

## Kontext

Manuel stellt Kunden regelmäßig dieselbe Rechnung (z. B. „Wartung Server,
monatlich"). Das soll automatisch passieren. **Abgrenzung zu ADR 0023:**
`recurring_subscriptions` (Block 10) ist die **Eingangs**seite (wiederkehrende
**Kosten**); dieses ADR ist die **Ausgangs**seite (wiederkehrende **Rechnungen**
an Kunden). Beides nutzt denselben In-App-Scheduler (ADR 0023/0027), aber die
Ausgangsseite muss zusätzlich §14/§19-Pflichtangaben, Nummernkreis, XRechnung,
PDF, KoSIT und Archiv erfüllen.

## Entscheidung

1. **Vorlage = Stammdatum, kein Beleg.** `recurring_invoices` / `_items` sind
   editierbar und pausierbar (`active = 0`) → **kein** Immutability-Trigger. Die
   **daraus erzeugte** Rechnung ist nach dem Festschreiben unveränderlich
   (invoices-Trigger). `0025` erzwingt eindeutige Positionen je Vorlage.
2. **Belegdatum = Erstellungstag (heute), NIE rückdatiert** (§14 Abs. 4 Nr. 3
   UStG + GoBD: zeitnahe, lückenlose, fortlaufende Nummern). Der Leistungszeitraum
   der Periode wird **getrennt** geführt: `delivery_date` = Periodenstichtag +
   optional Klartext in Position 1 (§14 Abs. 4 Nr. 6). **Catch-up** legt pro
   verpasster Periode eine Rechnung an — alle mit **heutigem** Belegdatum.
3. **Automatik-Stufe je Vorlage** (`auto_mode`, im UI Klartext wählbar):
   `draft` (nur Entwurf + Hinweis, prüfungssicher) · `issue` (automatisch
   festschreiben) · `issue_send` (festschreiben + automatisch per E-Mail über das
   Standard-Mail-Konto, **best-effort**: Versandfehler/kein Konto → Hinweis, der
   Beleg bleibt festgeschrieben).
4. **Wiederverwendung der Pipeline.** Der Scheduler ruft den **bestehenden**
   `create_invoice_draft_from_input` + `run_lock_pipeline` — Nummer, §19-Klausel,
   Pflichtangaben, XRechnung, PDF, KoSIT, Archiv und Lock kommen 1:1 aus dem
   normalen Rechnungsweg. **Kein** eigener Nummernkreis, **kein** zweiter PDF/XML-Weg.
5. **§19-Vererbung + serverseitige Durchsetzung.** Vorlagen-Positionen tragen
   `'E'`/0; bei `is_kleinunternehmer = true` erzwingt **das Speichern der Vorlage**
   serverseitig USt-Freiheit (`kleinunternehmer::assert_no_vat`, KB-0053) — nicht
   nur die UI-Sperre.
6. **Nummern-Sicherheit.** Die Nummer wird beim Draft-Anlegen vergeben; schlägt das
   spätere Festschreiben fehl, bleibt der nummerierte Entwurf bestehen (retrybar),
   keine neue Lückenklasse.
7. **Scheduler.** Job im 5-Minuten-`run_tick` (Unlock-gated, ein Backup pro Burst)
   **plus** Fälligkeits-Check beim Öffnen der Abo-Seite — der erste Tick beim
   App-Start läuft, solange das Backup gesperrt ist, ins Leere.

## Konsequenzen

- Vollautomatische Abo-Rechnung (bis hin zum Versand) ist möglich **und**
  prüfungssicher: Belegdatum bleibt korrekt, §14c wird verhindert, GoBD-Lock greift.
- Durch Pipeline-Reuse kein Duplizierungs-/Drift-Risiko bei §19/Pflichtangaben.
- `draft` als Default hält die prüfungssichere „Mensch bestätigt"-Linie (ADR 0023);
  `issue`/`issue_send` sind bewusste Opt-ins.
- Tests laufen als **Lib-Unit-Tests** (nicht `tests/`), weil der Scheduler die
  OS-Notification-Bridge linkt und ein `tests/`-Binary unter Windows sonst nicht
  lädt (`cfg(test)`-No-op in `notify::os_native`).

## Alternativen

| Option | Contra |
|---|---|
| Belegdatum = Periodenstichtag (rückdatiert) | verletzt §14 Abs. 4 Nr. 3 + GoBD (keine zeitnahe/lückenlose Nummer) |
| Eigener Generierungsweg (nicht Pipeline-Reuse) | dupliziert §19/Pflichtangaben/XRechnung → Drift- und §14c-Lückenrisiko |
| `issue_send` ohne Fallback | Versandfehler = verlorener/unklarer Beleg-Zustand |
| §19 nur im UI sperren | Alt-/Direkt-Pfade könnten USt-Vorlagen anlegen (§14c) — daher Backend-Durchsetzung |

## Referenzen

`domain::recurring_invoice`, `scheduler::recurring_invoice`
(`process_due`/`catch_up`/`run_now`/`build_invoice_input`), `commands::recurring_invoice`,
`db::repo::recurring_invoice`, Migrationen `0024`/`0025`, Frontend
`routes/recurring-invoices/**`; ADR 0023 (Eingangsseite + Scheduler), ADR 0027
(Cron-Jobs in `run_tick`), ADR 0005 (§19). Review-Bereich Q, Befunde KB-0053..KB-0058.
