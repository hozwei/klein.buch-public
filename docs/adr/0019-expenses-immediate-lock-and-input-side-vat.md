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

## Amendment 2026-10-25 — §13b und aktivierte Kosten (Lauf F7b/F8)

- **§13b ist kein reines Hinweis-Flag mehr:** Mit `reverse_charge_13b` muss die USt der
  Kosten 0 sein (`ExpenseValidationError::ReverseChargeWithTax`); auch Kleinunternehmer
  schulden die Steuer nach § 13b UStG und melden sie per Voranmeldung, Klein.Buch berechnet
  sie nicht. Die Sätze oben („erzwingt KEINE USt-Freiheit“, „reines Hinweis-Flag“) gelten
  insoweit nicht mehr. Einzelheiten: ADR 0039 Nr. 143.
- **Aktivierte Kosten** (Verweis `capitalized_as_asset_id`) lassen sich nicht stornieren;
  eine Rückabwicklung des Kaufs ist ein Abgang der Anlage (ADR 0039 Nr. 145).

## Amendment 2026-10-25 — Steuerstatus der Kosten nach dem Leistungsbezug (Lauf F8d)

- **Vorsteuer und Anschaffungskosten folgen dem Status bei Leistungsbezug:** Brutto
  (Kleinunternehmer) bzw. netto mit abziehbarer Vorsteuer (Regelbesteuerung) richtet sich
  nach dem §19-Status im Jahr des **Belegdatums** der Kosten, nicht nach dem Zahlungsjahr
  (§ 15 Abs. 1 Satz 1 Nr. 1 UStG; Abschn. 19.5 UStAE zum Wechsel der Besteuerungsform).
  Kosten haben kein eigenes Leistungsdatum; das Belegdatum ist ein festgeschriebenes
  Kernfeld (`trg_expenses_immutable`), deshalb kann keine spätere Änderung der Kosten den
  Status verschieben. Das Jahr der Ausgabe bleibt das Zahlungsjahr (§ 11 Abs. 2 EStG).
  Einzelheiten: ADR 0039 Nr. 148. Die Konsequenz oben („bei späterem §19-Verzicht müsste
  die Vorsteuer-Logik ergänzt werden“) ist seit Lauf F5b/F8b/F8d umgesetzt.
- **Aktivierte Kosten (Lauf F8e):** Liegen Anschaffungsdatum der Anlage (Lieferung, § 9a
  EStDV) und Belegdatum in Jahren mit unterschiedlichem Status, wird das Aktivieren
  abgelehnt (Steuerberatung). Einen Wegfall von §19 mitten im Jahr (100.000-€-Grenze)
  bildet Klein.Buch nicht ab; ADR 0039 Nr. 148 (5) und Bekannte Grenze (b).
