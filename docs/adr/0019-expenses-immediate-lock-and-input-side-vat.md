# ADR 0019 — Kosten: Sofort-Festschreiben + Eingangsseitige USt

**Status:** Akzeptiert · 2026-05-20 · Block 9.

## Kontext

Kosten (Betriebsausgaben) sind die **Eingangsseite** der Buchhaltung. Zwei Fragen
mussten geklärt werden: (1) Wann wird eine Kosten-Position GoBD-fest? (2) Wie
verhält sich die §19-Hardline auf der Eingangsseite?

Das Schema (`0007_expenses.sql`) kennt für `expenses.status` nur
`('recorded','canceled')` — **keinen** Draft-Zustand wie bei Rechnungen
(draft → issued). Gleichzeitig fordert die Backup-Hardline einen Lock-Event je
Kosten-Erfassung.

## Entscheidung

- **Sofort-Festschreiben:** `expenses::create` setzt `status='recorded'` UND
  `locked_at = now` in einem Schritt. Ab da greift `trg_expenses_immutable` auf
  den Kernfeldern (expense_number, expense_date, net/gross, vendor_contact_id).
  Es gibt **keine** Edit-Route. Korrektur = **Storno** (`expenses_cancel` →
  `status='canceled'` + Grund) + neue, korrigierte Kosten. (Storno statt
  Löschung, GoBD-Hardline.)
- **Auto-Critical-Backup-Hook** nach `create` und nach `cancel`
  (`backup::auto_backup_if_unlocked`, best-effort), da beides Lock-Events sind.
- **§19 gilt NICHT auf der Eingangsseite:** Die §19-Klausel/USt-Sperre betrifft
  ausschließlich **ausgehende** Belege (Rechnungen/Angebote). Eine
  Eingangsrechnung DARF USt enthalten — der Kleinunternehmer zahlt sie, kann aber
  keine Vorsteuer ziehen. Für die EÜR (Cash-Basis, Block 13) ist deshalb der
  **Brutto**-Betrag die Betriebsausgabe. `domain::expense::validate_expense`
  erzwingt daher KEINE USt-Freiheit — es prüft nur Konsistenz
  (`gross == net + tax`, Kategorie, Beträge ≥ 0, Pflichtfelder).
- **§13b Reverse-Charge** ist ein reines **Hinweis-Flag** (`reverse_charge_13b`)
  — keine USt-Auto-Berechnung (PRD G16: „Berechnung extern").
- **Beleg:** primärer Beleg write-once als `ArchiveKind::ExpenseOriginal` →
  `expenses.receipt_archive_id` (beim Anlegen, kein Kernfeld → nachträglich
  setzbar). Zusätzliche Anhänge über die generische `attachments`-Tabelle
  (`parent_type='expense'`, `ArchiveKind::Attachment`) via `attachments_add`.

## Konsequenzen

- Keine versehentliche Nachbearbeitung erfasster Kosten; sauberer GoBD-Belegkreis
  `KO-{YYYY}-{NNNN}`.
- Tippfehler erzwingen einen Storno-Vorgang (kleiner Reibungsverlust, dafür
  prüfungssicher).
- Kosten mit USt werden voll (brutto) als Ausgabe geführt — korrekt für §19, aber
  bei späterem §19-Verzicht (Regelbesteuerung) müsste die Vorsteuer-Logik ergänzt
  werden (heute Out-of-Scope; EÜR ist §19-zentriert).

## Alternativen

| Option | Contra |
|---|---|
| Draft-Phase für Kosten (editierbar) | Schema hat keinen Draft-Status; weicht von „zeitnah + unveränderbar" ab |
| §19-USt-Sperre auch auf Kosten | fachlich falsch — Eingangsseite darf USt tragen |
| §13b automatisch USt berechnen | PRD: Berechnung extern; Reverse-Charge-Fälle zu vielfältig für Auto-Logik |

## Referenzen

`domain::expense`, `db::repo::expenses`, `commands::expenses`,
`0007_expenses.sql`; `memory/klein-buch/block-9-notes.md`,
`memory/klein-buch/euer-cash-basis.md`.
