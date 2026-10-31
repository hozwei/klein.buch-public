# GoBD-Implementation

> Vertiefung zu „GoBD" in `../ARCHITECTURE.md` §7. Wie die GoBD-Hardline
> tatsächlich im Code umgesetzt ist: was passiert wann, welcher Trigger feuert,
> welche Tabellen sind betroffen, wo verboten ist was. Diese Datei ist die
> Audit-/Maintainer-Referenz und beantwortet die Frage „Wo greift Klein.Buch
> ein, wenn jemand etwas Verbotenes versucht?"

ADR-Basis: 0006 (Immutability via DB-Trigger, Storno statt Löschung).
Ergänzungen: 0027 (GJ-Lock), 0036 (Factory Reset; im Server-Betrieb
abgeschaltet und entfernt, ADR 0039 E7), 0039 (Write-once unter Linux).
Trigger-Stand nach Migration `0028_append_only_hardening` (R1-Review).

---

## 1. Hardline auf einer Seite

1. **Festgeschriebene Belege sind unveränderlich.** Sobald `locked_at` gesetzt
   ist (Rechnung, Angebot, Storno-Beleg, Kosten und Privatbewegungen ab dem
   Anlegen, Anlage und AfA-Buchung nach dem GJ-Lock), lässt der DB-Trigger nur
   noch Status-Übergänge zu, keine Änderung der Kernfelder, und `locked_at`
   selbst lässt sich weder zurücksetzen noch ändern.
2. **Storno statt Löschung.** Ein fehlerhafter Beleg wird durch einen Storno-
   Beleg neutralisiert. Das Original bleibt erhalten und wird als `canceled`
   markiert. Der Storno-Beleg bekommt Geschäftsjahr und Nummer
   (`ST-JJJJ-NNNN`) aus seinem eigenen Datum (GoBD Rz. 51) und verweist auf
   Nummer und Datum des Originals (XML BT-25/BT-26, PDF; ADR 0039 Nr. 122). `DELETE` auf Rechnungen, Angeboten, Kosten, Privatbewegungen und
   Anlagen scheitert immer am Trigger, auch bei Entwürfen.
3. **Archive ist write-once.** Beleg-PDF und -XML werden mit SHA-256 in
   `archive/` abgelegt, `chmod 0o400`, `trg_archive_no_update` schützt Hash,
   Pfad und Größe. Re-Hash bei jedem Lesen erkennt Manipulation. Unter Linux
   heißt das Erkennung, nicht Verhinderung (§3.4).
4. **Audit-Log ist append-only.** Trigger verbieten UPDATE und DELETE auf
   `audit_log`.
5. **Aufbewahrung 10 Jahre.** Es gibt kein Löschen-UI für Belege.
6. **Keine Totallöschung in der App.** Der Factory Reset der Desktop-Fassung
   (ADR 0036) ist im Server abgeschaltet und sein Code entfernt (ADR 0039, E7);
   ein vorgefundener Reset-Marker verweigert den Start, gelöscht wird nichts.
   Eine neue, leere Instanz legt der Betreiber mit einem neuen Volume an; was mit
   dem alten geschieht, verantwortet er.

---

## 2. Lock-Trigger im Detail

Die Trigger zählen die **geschützten** Spalten auf (`WHEN OLD.locked_at IS NOT
NULL AND (NEW.x != OLD.x OR …)`); nicht aufgezählte Spalten bleiben für die
Status-Übergänge der App änderbar. Wer eine neue Kernspalte einführt, muss sie
im Trigger ergänzen.

### 2.1 `trg_invoices_immutable` + `trg_invoices_no_delete`

**Greift bei:** UPDATE auf `invoices`, wenn `OLD.locked_at IS NOT NULL`.

**Geschützt:** `locked_at` (kein Zurücksetzen, kein neuer Wert),
`invoice_number`, `invoice_date`, `net_amount_cents`, `gross_amount_cents`,
`tax_amount_cents`, `contact_id`, `fiscal_year`, `is_kleinunternehmer`,
`direction`, `delivery_date`, `service_period_start`, `service_period_end`,
`buyer_reference` (seit Migration 0035), `due_date`, `currency_code`, `pdf_template`,
`is_storno_for`, `derived_from_quote_id`, `paid_to_account_id`, `payment_note`,
`pdf_archive_id`, `xml_archive_id`, alle `buyer_*`- und `seller_*`-Snapshot-
Spalten.

**Änderbar** (Status-Übergänge): `status`, `paid_amount_cents`, `paid_at`
(seit Migration 0034 nur passend zum Zahlungsbuch, §2.9), `sent_at`,
`canceled_at`, `canceled_by_storno_id`, `cancel_reason`, `updated_at`.
`payment_history_json` ist seit 0034 Altbestand und unveränderlich
(`trg_invoices_payment_history_frozen`); `paid_to_account_id` bleibt gesperrt,
das Konto steht je Zahlung im Zahlungsbuch. Das Festschreiben selbst
(Setzen von `locked_at`, Validierung, Archiv-IDs) feuert nicht, weil
`OLD.locked_at` dabei noch `NULL` ist.

**`trg_invoices_no_delete`:** jedes `DELETE` scheitert, auch bei Entwürfen.
Ein nicht benötigter Entwurf wird **verworfen** (§5.5, Migration 0032).

**Seit Migration 0032 zusätzlich** (Lauf F, Block F2a):

- `trg_invoices_canceled_final` — ein storniertes Original bleibt storniert:
  `status`, `canceled_at`, `canceled_by_storno_id` und `cancel_reason` ändern
  sich nach dem ersten Setzen nicht mehr (R1-S2-MARKCANCELED). Zusammen mit dem
  Unique-Index `ux_invoices_one_locked_storno` (höchstens ein festgeschriebener
  Storno je Original, LE-STORNO-UNIQUE) ist das Storno-Paar auch bei einem
  zweiten Lauf idempotent.
- `trg_invoices_voided_final`, `trg_invoices_void_draft_only`,
  `trg_invoices_void_not_on_insert` — Verwerfen nur für offene Entwürfe, nur
  mit Grund; danach ist der Entwurf unveränderlich und lässt sich nicht
  festschreiben (§5.5). Einzige Ausnahme (Lauf F2c): Die Anonymisierung des
  Kontakts darf den Empfänger-Snapshot (`buyer_*` außer Land) und `notes`
  eines verworfenen Entwurfs auf den Platzhalter bzw. `NULL` setzen — nur in
  diese Richtung, nur bei anonymisiertem Kontakt, alle übrigen Spalten
  einzeln als unverändert geprüft (Test
  `voided_final_trigger_lists_every_invoice_column`).
- `trg_invoice_items_frozen_insert`/`_update`/`_delete` — Positionen einer
  festgeschriebenen oder verworfenen Rechnung lassen sich weder ändern noch
  löschen noch ergänzen (KB-0002); vorher schützten die Kopf-Trigger nur die
  Summen.

Tests: `tests/migration_0028_test.rs` (je geschütztem Feld bzw. Feldgruppe ein
Fall, Unlock-Schutz, erlaubte Übergänge, `DELETE` auf Entwurf und Beleg),
`tests/migration_0032_test.rs` (Positionen, Storno-Paar, Verwerfen),
`tests/invoice_lock_pipeline_test.rs`
(`gobd_trigger_blocks_core_field_update_after_lock`).

### 2.2 `trg_quotes_immutable` + `trg_quotes_no_delete`

Analog für `quotes`: geschützt sind u. a. `quote_number`, `quote_date`,
Beträge, `contact_id`, `fiscal_year`, `is_kleinunternehmer`, `valid_until`,
`currency_code`, `pdf_template` und die Snapshots. Änderbar bleiben Status und
Zeitpunkte von Annahme, Ablehnung, Storno und Konvertierung
(`converted_invoice_id`) sowie `pdf_archive_id` (Re-Render-Pfad von
`ensure_quote_pdf`). Kein `DELETE`. Seit Migration 0032: ein zurückgezogenes
Angebot (`canceled`, auch ein zurückgezogener Entwurf = verworfen) bleibt
zurückgezogen (`trg_quotes_canceled_final`), und Positionen festgeschriebener
oder zurückgezogener Angebote sind unveränderlich
(`trg_quote_items_frozen_*`). Seit Lauf F2c bleibt ein umgewandeltes Angebot
umgewandelt (`trg_quotes_converted_final`): kein Zurückziehen, kein anderer
Rechnungsbezug; nur das Verwerfen des daraus entstandenen Entwurfs setzt es in
derselben Transaktion auf `accepted` zurück (Audit
`quote.conversion_reverted`). Das Zurückziehen verlangt einen Grund (mindestens
10 Zeichen) und schreibt Statuswechsel und Audit `quote.cancel` in einer
Transaktion mit Status-Bedingung im `UPDATE`.

### 2.3 `trg_expenses_immutable` + `trg_expenses_no_delete`

Kosten sind **ab dem Anlegen gelockt**, kein separates Festschreiben. Geschützt:
Nummer, Datum, Beträge, Lieferant und Lieferanten-Snapshot, `fiscal_year`,
`category`, `description`, `currency_code`, `reverse_charge_13b`. Änderbar:
Storno (`status`, `canceled_*`), Zahlung (`paid_date`, Zahlungskonto),
Beleg-Archiv-ID, E-Rechnungs-Prüfergebnis, Verknüpfungen zu Abo und Anlage.
Kein `DELETE`.

### 2.4 `trg_assets_immutable` + `trg_assets_no_delete`

Greift, sobald `locked_at` gesetzt ist (GJ-Abschluss, `fiscal_year::lock`).
Vorher sind Anlagen Stammdaten. Geschützt: Nummer, Anschaffungsdatum, -kosten,
-GJ, AfA-Methode, Nutzungsdauer, betrieblicher Anteil, AfA-Kategorie,
Verknüpfungen, Bezeichnung. Änderbar bleiben Buchwert-Fortschreibung
(`book_value_cents`, `last_depreciation_year`) und Veräußerung (`disposal_*`).
Kein `DELETE`. Die Veräußerung geschieht genau einmal: danach sind `disposed`
und alle `disposal_*`-Felder unveränderlich (`trg_assets_disposal_final`,
Migration 0032; `assets::dispose` mit `AND disposed = 0`). Seit Lauf F2c hält
derselbe Trigger auch Bezeichnung, Anschaffung, AfA-Grundlage und Restbuchwert
einer veräußerten Anlage fest, `trg_depreciation_entries_no_delete_disposed`
verbietet das Löschen ihrer AfA-Buchungen (Reset und Bearbeiten lehnt schon
die App ab), `assets_dispose` holt fehlende Vorjahres-AfA vor der anteiligen
AfA des Abgangsjahres nach, und der Jahresabschluss schreibt alle bis zum Jahr
angeschafften Anlagen fest, auch veräußerte (vorher nur `disposed = 0`).
Seit Lauf F3b läuft der Abgang in einer Transaktion: Abgangsdatum vor der
Anschaffung, festgeschriebene AfA ab dem Abgangsjahr und ein später
abgeschlossenes Jahr, das die Anlage im Bestand führt, lehnen ab; offene
AfA-Buchungen ab dem Abgangsjahr (spätere Jahre, volle Jahres-AfA des
Abgangsjahres) entfernt er mit ihren Werten im Audit
(`depreciation.reset_for_disposal`) und bucht die anteilige AfA bis zum
Abgangsmonat — AfA und Restbuchwert ergeben zusammen die Anschaffungskosten
(ADR 0039 Nr. 128).

### 2.5 `trg_depreciation_immutable` + `trg_depreciation_entries_no_delete_locked`

Eine AfA-Buchung ist ab `locked_at` (GJ-Abschluss, `lock_for_year`) vollständig
unveränderlich und nicht löschbar. Ungelockte Buchungen darf der Reset-Pfad
(`depreciation::reset_unlocked_for_asset`) löschen.

### 2.6 `trg_private_movements_immutable` + `trg_private_movements_no_delete`

Ab dem Anlegen gelockt. Geschützt: Nummer, Datum, Betrag, Art, `fiscal_year`,
Konto, Beschreibung. Offen bleiben nur Beleg-Archiv-ID und Notiz. Wer sich
vertippt, bucht eine Gegenbewegung. Kein `DELETE`.

### 2.7 Append-only-Tabellen (no-update + no-delete)

`audit_log`, `email_log`, `backup_log`, `fiscal_year_locks`,
`package_revisions`, `invoice_payments` (§2.9; Korrektur per Gegenbuchung);
dazu `legal_documents` und `quote_legal_documents`
(Kernfelder unveränderlich, kein `DELETE`; bei `legal_documents` bleibt das
Aktivieren/Deaktivieren möglich). Korrigiert wird nur per Insert eines neueren
Datensatzes (z. B. neue Paket-Revision). `archive_entries` ist seit Migration
0032 ebenfalls ohne `DELETE` (`trg_archive_no_delete`, KB-0004; Hash, Pfad und
Größe waren schon unveränderlich).

### 2.8 Steuerstatus nach § 19 UStG (`tax_status_changes`, Migration 0033)

Der Steuerstatus ist eine append-only Historie (ADR 0042 Nr. 1; § 146 Abs. 4 AO,
GoBD Rz. 58 ff.): Wirkungsdatum, Status, Grund, Bindungsende, auslösender Beleg,
Notiz. `trg_tax_status_changes_no_delete` verbietet `DELETE`,
`trg_tax_status_changes_revoke_only` erlaubt `UPDATE` nur einmal und nur als
Widerruf (`revoked_at` + `revoke_reason`, Grund Pflicht); alle übrigen Spalten
bleiben danach unveränderlich. Der widerrufene Eintrag bleibt sichtbar (Verlauf
in „Meine Firmendaten“, durchgestrichen mit Datum und Grund). Höchstens ein
gültiger Eintrag je Wirkungsdatum (`uq_tax_status_changes_active`).

- **Festgeschriebene Belege bleiben unberührt.** Der Status einer Rechnung bzw.
  eines Angebots wird beim Festschreiben wie bisher eingefroren
  (`invoices.is_kleinunternehmer`, `quotes.is_kleinunternehmer`; Trigger 2.1
  und 2.2 unverändert). Ein späterer Eintrag oder Widerruf ändert sie nicht;
  Belege, deren eingefrorener Status nicht mehr passt, meldet Klein.Buch als
  Hinweis (Berichtigung nach § 31 Abs. 5 UStDV bzw. Storno und Neuausstellung,
  ADR 0042 Nr. 4).
- **Abgeschlossene Jahre.** Ein Eintrag oder Widerruf, der den Status in einem
  abgeschlossenen Geschäftsjahr ändern würde, wird abgelehnt
  (`domain::tax_status::validate_change`/`validate_revoke`): Der
  Jahres-Snapshot hängt über die Vorsteuer aktivierter Kosten vom Status ab
  (6.1). Eintrag und Widerruf laufen mit einer Booking-Permit (6.2), damit kein
  Abschluss dazwischenkommt.
- **Audit.** Eintrag (`tax_status.changed`), Widerruf (`tax_status.revoked`) und
  Übernahme des Bestands (`tax_status.migrated`) stehen im Audit-Log, in derselben
  Transaktion wie der Eintrag; Re-Auth vorher (Registry).
- **Kein Überschneiden, kein Umschreiben.** Neue Einträge wirken nach dem
  jüngsten; widerrufen lässt sich nur der jüngste gültige Eintrag. Eine
  Korrektur älterer Einträge geht nur über Widerruf der jüngeren und neue
  Einträge — jeder Schritt bleibt nachvollziehbar.

### 2.9 Zahlungsbuch (`invoice_payments`, Migration 0034)

Zahlungen auf Ausgangsrechnungen stehen in der append-only Tabelle
`invoice_payments` (ADR 0042 Nr. 7–9; § 146 Abs. 4 AO, § 239 Abs. 3 HGB,
GoBD Rz. 58–60, 64): je Buchung eine Zeile mit laufender Nummer je Rechnung,
Art (`payment` > 0 oder `reversal` < 0), Betrag, Zahldatum, Konto, Notiz,
Erfassungszeit; eine Gegenbuchung zusätzlich mit Verweis auf die stornierte
Zahlung und Grund (10–500 Zeichen).

- **Append-only:** `trg_invoice_payments_no_update`/`_no_delete`.
- **`trg_invoice_payments_insert_checks`** (Prüfungen in fester Reihenfolge):
  nur auf festgeschriebene, nicht verworfene, nicht stornierte
  Ausgangsrechnungen ohne festgeschriebenen Storno und nicht auf Storno-Belege;
  nicht im abgeschlossenen Geschäftsjahr des Zahldatums (zweite Linie hinter
  `fiscal_year::guard`, 6.2); Gegenbuchung nur auf eine noch nicht stornierte
  Zahlung derselben Rechnung mit genau umgekehrtem Betrag, gleichem Datum und
  Konto und ohne offenen Storno-Entwurf; laufende Nummer lückenlos; Summe
  zwischen 0 und dem Bruttobetrag. `ux_invoice_payments_one_reversal`: höchstens
  eine Gegenbuchung je Zahlung.
- **Cache gebunden:** `trg_invoices_payment_cache_bound` lässt
  `paid_amount_cents` nur als Summe des Buchs, `paid_at` nur als jüngstes
  Zahldatum der nicht stornierten Zahlungen ändern; ändert sich Summe oder
  Status, muss der Status zum Buch passen (`draft` nur unfestgeschrieben ohne
  Zahlung, `issued`/`sent` ohne Zahlung, `partially_paid` darunter, `paid`
  voll; `canceled` frei, aus `canceled` heraus wacht
  `trg_invoices_canceled_final`). Die Migration setzt `paid_at` des Bestands
  einmal auf das jüngste Zahldatum (vorher: Datum der zuletzt erfassten
  Zahlung, teils mit Uhrzeit; altes und neues Datum im Audit
  `invoice.payments_migrated`). Die EÜR liest die Storno-Erstattung aus der
  Summe des Buchs, nicht aus dem Cache. `trg_invoices_payment_cache_insert`: eine neue
  Rechnung beginnt ohne Zahlung. `trg_invoices_payment_history_frozen`: die
  JSON-Historie des Altbestands bleibt unverändert.
- **Zahlung stornieren** (`invoices_reverse_payment`, Re-Auth): Gegenbuchung am
  ursprünglichen Zahldatum (eine nie geflossene Zahlung war kein Zufluss nach
  § 11 EStG), nur im offenen Geschäftsjahr dieses Datums — sonst Ablehnung mit
  Hinweis auf § 153 AO —, Belegsperre `locks::INVOICES` und Booking-Permit wie
  beim Erfassen und beim Storno, Buchzeile, Cache und Audit
  (`invoice.payment_reversed`; beim Erfassen `invoice.payment_recorded`) in
  einer Transaktion. Echte Rückzahlungen bleiben Storno mit Erstattung (§5).
- **Oberfläche** (`routes/invoices/[id]/+page.svelte`, `paymentHistory.ts`):
  Die Karte „Zahlungen“ zeigt jede Buchung aus `InvoiceDetail.payments`, auch
  nach einer vollen Gegenbuchung (bezahlt 0 €). Eine stornierte Zahlung bleibt
  stehen und trägt das Badge „Storniert“ mit dem Zeitpunkt der Gegenbuchung;
  die Gegenbuchung ist als „Gegenbuchung“ mit „Storno der Zahlung vom …“ und
  Grund benannt (Text, nicht nur Farbe). Je nicht stornierter Zahlung steht
  „Zahlung stornieren …“ im Menü „Mehr“ der Zeile; ist es nicht möglich, bleibt
  der Eintrag gesperrt und nennt `notReversibleReason`. Der Gefahr-Dialog
  (`alertdialog`, Startfokus „Abbrechen“) nennt Betrag, Zahldatum und Konto,
  die Wirkung am ursprünglichen Zahldatum und die Abgrenzung zur Rückzahlung;
  der Grund ist Pflicht (Fehler am Feld), Ablehnungen des Servers stehen als
  Banner im Dialog. Nach Erfolg bietet der Toast „Zahlung neu erfassen“ an
  (`/invoices/[id]/payment?ersetzt=<id>`, Datum/Konto/Notiz als Vorschlag).
  Es gibt keinen Weg, eine Buchung zu ändern oder zu löschen. Die Storno-Seite
  rechnet das früheste Storno-Datum ohne stornierte Paare (wie der Server), die
  EÜR-Einzelaufstellung kennzeichnet Gegenbuchung und stornierte Zahlung.
- **Übernahme:** Migration 0034 übernimmt die JSON-Historie und den Altbestand
  ohne Historie mit festen IDs im UUIDv7-Format
  (`00000000-0000-7034-8000-…`, deterministisch wie 0033) und bricht bei
  nicht lesbarer Historie, Summenabweichung, fehlendem Zahldatum oder
  unbekanntem Konto mit benannter Meldung ab (ADR 0039 Nr. 124); dieselbe
  Prüfung läuft vor dem Pflicht-Backup (`db::payment_ledger_problem`) und in
  der Restore-Prüfung.

---

## 3. Archive

### 3.1 Schreiben

`archive::store::store_bytes(pool, archive_root, fiscal_year, kind,
file_name, mime_type, bytes)`:

1. Dateiname prüfen (nicht leer, kein `/`, `\`, `:`); SHA-256 und Größe
   berechnen.
2. Zielpfad `<archive_dir>/<Geschäftsjahr>/<kind>/<file_name>`, z. B.
   `2026/invoices/pdf/RE-2026-0001.pdf`; im Container liegt `archive_dir` unter
   `/data/archive`.
3. Sperre je Zielpfad (`locks::ARCHIVE_PATHS`): Prüfen, Schreiben und Insert
   laufen am Stück.
4. Schreiben (W2): Temp-Datei mit eindeutigem Namen (`.<name>.<uuid>.tmp`,
   `create_new`), `fsync`, **Rücklese-Prüfung** (Lauf F, KB-0040: die
   Temp-Datei wird gelesen und gegen den SHA-256 des Puffers geprüft, bevor sie
   sichtbar wird), Schreibschutz `0o400` **vor** dem Sichtbarwerden, dann
   `hard_link` an den Zielnamen. Der Hardlink scheitert, wenn das Ziel schon
   existiert — überschrieben wird nie. Ohne Hardlinks (Lauf F, W2-S2-4)
   `renameat2` mit `RENAME_NOREPLACE` (ebenfalls atomar ohne Überschreiben);
   kennt das Dateisystem das Flag nicht, wird das Ziel unmittelbar vorher
   geprüft und unter der Pfad-Sperre umbenannt — deshalb gehören DB und Archiv
   nicht auf NFS/SMB/exFAT. Temp-Name entfernen, Ordner-`fsync`. Restrisiko: die
   Rücklese-Prüfung liest in der Regel aus dem Seiten-Cache, nicht vom Medium;
   Fehler des Mediums erkennen Re-Hash beim Lesen und der monatliche
   Integritätslauf (§3.2, §3.3).
5. `archive_entries`-Insert `(id UUIDv7, file_path UNIQUE, file_name,
   file_hash_sha256, file_size_bytes, mime_type, source)` + Audit
   `archive.store`.

Der MIME-Typ hochgeladener Dateien kommt aus der Prüfung am Inhalt (Magic
Bytes), nicht vom Browser (ADR 0039 Nr. 70).

**Datei am Zielpfad schon vorhanden:**

- gleicher Hash, registriert → der bestehende Eintrag wird zurückgegeben
  (idempotent, keine Duplikate);
- gleicher Hash, nicht registriert → nur der Insert wird nachgeholt;
- anderer Hash, nicht registriert (Rest eines abgebrochenen Laufs) → die Datei
  wird als `.<name>.orphan-<uuid>` beiseitegelegt, **nie gelöscht**, neu
  geschrieben und im Audit vermerkt;
- anderer Hash, registriert → Fehler „existiert bereits mit anderem Hash".
  Beim Festschreiben einer Rechnung (PDF und XML) weicht der Code stattdessen
  auf eine neue Dateiversion aus (`RE-2026-0001-2.pdf`, …): ein neuer Versuch
  nach einem Abbruch erzeugt andere Bytes (Zeitstempel). Der alte Eintrag
  bleibt unverändert im Archiv, der Beleg verweist auf die neue Version.

### 3.2 Lesen + Re-Hash

`archive::store::read_and_verify(pool, archive_id)`:

1. `archive_entries` lookup → `(file_path, hash, size)`.
2. Datei lesen.
3. Größe und SHA-256 vergleichen.
4. Abweichung ⇒ Fehler („archive tamper"), keine Bytes.
5. Erfolg ⇒ Audit `archive.read` (Integritätslauf und Roh-XML-Ansicht nutzen
   eine Variante ohne diesen Audit).

Im Server läuft jeder Abruf einer Archivdatei darüber
(`GET /files/archive/{id}`, ADR 0039 Nr. 66): Hash-Prüfung vor dem ersten Byte,
Pfad per `canonicalize` gegen `archive_dir`, eigene PDFs inline, alle anderen
Dateien als `attachment` mit CSP `sandbox`.

**Wichtig** (G1-HARDEN.4, W2): Manipuliert, fehlt und nicht lesbar sind
**getrennte** Befunde. Eine fehlende Datei ist nicht automatisch eine
Manipulation — sie kann auch ein fehlendes Volume sein; eine nicht lesbare
Datei ist meist ein Rechteproblem (im Container etwa eine falsche UID). Der
Integritätslauf meldet sie getrennt.

### 3.3 Re-Hash-Cron

`scheduler::integrity_check_cron` läuft höchstens einmal je Kalendermonat
(Europe/Berlin), nur bei entsperrter Instanz. Je Eintrag entsteht genau ein
Audit-Event (`archive.integrity_pass`, `_fail`, `_missing`, `_unreadable`); das
Aggregat landet in `archive_integrity_checks`. Manuell über
`archive_integrity_run`. Details in `security.md` §8.

### 3.4 Write-once unter Linux (Phase W)

Im Server-Betrieb (Docker, ADR 0039) ist `0o400` nur ein Schreibschutz für den
Inhalt. Derselbe Benutzer, unter dem der Server läuft, kann den Modus
zurücksetzen und Dateien löschen oder umbenennen (dafür zählt das Schreibrecht
auf den Ordner, das der Server zum Ablegen neuer Belege braucht); root auf dem
Host ohnehin. Klein.Buch verspricht unter Linux deshalb keine physische
Unveränderbarkeit. Der Schutz beruht auf **Erkennung und Wiederherstellbarkeit**:
SHA-256 beim Schreiben, Re-Hash beim Lesen, monatlicher Integritätslauf,
Off-Site-Backups und schreibgeschützte Snapshots des Dateisystems beim
Betreiber. Der Start prüft per Selbsttest, ob `0o400` im Archiv greift, und
verweigert sonst den Dienst (auch als root). Begründung und Einzelheiten:
`security.md`, Kapitel „Write-once unter Linux"; Betrieb:
`../operations-guide.md` §5.

---

## 4. Audit-Log

Die `audit_log`-Tabelle ist das **forensische** Protokoll. Nicht-triviale
Mutationen schreiben einen Eintrag (Auswahl, Namen wie im Code):

- Rechnungen: `invoice.draft_create`, `invoice.lock`, `invoice.sent`,
  `invoice.payment_recorded`, `invoice.cancel`, `storno.create`.
- Angebote: `quote.draft_create`, `quote.issue`, `quote.sent`,
  `quote.accepted`, `quote.rejected`, `quote.converted`, `quote.cancel`,
  `quote.pdf_generated`.
- Kosten und Privatbewegungen: `expense.create`, `expense.import_einvoice`,
  `expense.payment_set`, `expense.cancel`, `private_movement.create`.
- Archiv: `archive.store`, `archive.read`, `archive.integrity_pass`,
  `archive.integrity_fail`, `archive.integrity_missing`,
  `archive.integrity_unreadable`.
- Geschäftsjahr und AfA: `fiscal_year.close`, `depreciation.accrue`,
  `depreciation.lock`, `depreciation.reset`. Ein „Wieder-Öffnen" gibt es nicht.
- Backup und Restore: `backup.created`, `backup.failed`, `backup.mirror.ok`,
  `backup.mirror.failed`, `backup.mirror.skipped` (Pause nach Restore),
  `backup.rotation.prune`, `backup.pre_migration`, `backup.daily_time_set`,
  `backup.history_rescanned`, `backup.restore.requested` (steht im
  Pre-Restore-Backup des ersetzten Stands), `backup.restore.applied` (in der
  wiederhergestellten DB), `backup.passphrase.setup`, `backup.passphrase.changed`.
- Nummernkreise und Pause (Lauf F): `numbering.raised_after_restore`,
  `numbering.raised` (mit Begründung), `scheduler.hold_set`,
  `scheduler.hold_released`.
- Anmeldung: `auth.setup.ok`, `auth.login.ok`, `auth.reauth.ok`,
  `auth.totp.setup`, `auth.totp.renewed`, `auth.recovery.renewed`.
  **Fehlversuche stehen nicht im Audit-Log**, nur im Server-Log (ADR 0039 E5).
- DSGVO und Exporte: `dsgvo.export`, `contact.anonymize`, `migration.export`
  und **jeder** EÜR-Export (Lauf F, R2-016 / ASVS 8.3.5): `euer.pdf_export`,
  `euer.datev_export`, `euer.elster_export`, `euer.detail_export`,
  `euer.stb_export` — mit Geschäftsjahr, Dateiname, Zählwerten und
  Snapshot-Status (`open`/`match`/`mismatch`), ohne Inhalte; bei einer
  Abweichung vom Festschreibungs-Snapshot zusätzlich `euer.snapshot_mismatch`
  (§6.1).
- Mailkonten: `mail_account.create`, `mail_account.update`,
  `mail_account.delete`, `mail_account.test_send`; jeder Versand zusätzlich im
  append-only `email_log`.

**Details als JSON (Lauf F, W14-010).** `audit_log::append` nimmt die Details
nur als `serde_json::Value` entgegen und serialisiert sie selbst — handgebautes
JSON per `format!` ist damit ausgeschlossen. Anführungs- und Steuerzeichen in
Namen, Gründen oder Pfaden ergeben immer gültiges JSON; über `details_json`
lässt sich nichts in Auswertungen einschleusen (Tests
`db::repo::audit_log::tests::details_are_serialized_as_valid_json`,
`tests/audit_export_test.rs`).

**Akteur (Lauf F, R1-S3-ACTOR).** `actor` ist `owner` für alles, was über die
Sitzung des Inhabers läuft (Commands, Jobs, erfolgreiche Schritte der
Anmeldung), `scheduler` für die Abläufe des Schedulers (Abo-Läufe, Auto-AfA,
Tages-Backup, Integritätslauf) und `system` für den Rest (Start, Migrationen,
Nacharbeit nach einem Restore). Klein.Buch hat genau einen Benutzer (E1); der
Akteur unterscheidet daher Handlung und Automatik, keine Personen.

**Hardline.** Trigger blocken UPDATE und DELETE; `audit_log` wächst monoton.
Bei Performance-Bedenken: 10 Jahre × erwartetes Volumen einer §19-Praxis
ergibt typischerweise < 1 GB; SQLite handhabt das problemlos.

**Sensitive Daten.** Im `details`-JSON niemals:

- Daten-Passwort (Passphrase), TOTP-Geheimnis oder -Codes, Einmal-Codes,
  Setup-Token, Sitzungs-IDs.
- SMTP- und SFTP-Passwörter, OAuth-Token.
- Serverpfade aus Uploads oder Exporten (Audit nennt Dateinamen, ADR 0039 Nr. 82).
- Klartext-PII, die für die Audit-Aussage nicht gebraucht wird.

Ein Audit-Event soll sagen *was* passiert ist, nicht *die Geheimnisse zeigen,
die dabei verwendet wurden*.

---

## 5. Storno statt Löschung

### 5.1 Wann ist Storno notwendig?

- Falsche Beträge in einer **bereits gelockten** Rechnung.
- Falscher Empfänger.
- Falsche Klausel-Auswahl (z. B. §19 vs. Regelbesteuerung).
- Generell jeder fachliche Fehler nach `locked_at`.

### 5.2 Mechanik

`domain::storno::make_storno(original)` erzeugt eine **neue** Rechnungs-Draft:

- `invoice_number = ST-{YYYY}-{NNNN}` (eigener Nummernkreis).
- `is_storno_for = original.id`.
- Alle Positionen mit **negierten Mengen** (also negativem `quantity_thousandths`).
- Selber Buyer-Snapshot und selbe Klausel-Lage wie das Original;
  Geschäftsjahr und Nummernkreis folgen dem **Storno-Datum** (ADR 0039
  Nr. 122, löst Nr. 18 ab).
- Beim Festschreiben werden Storno-Lock und `canceled` am Original in **einer**
  Transaktion gesetzt (`lock_with_pair_cancel`; `tests/migration_0028_test.rs`,
  `tests/migration_0032_test.rs` `storno_pair_is_final_and_idempotent`).
  Höchstens ein festgeschriebener Storno je Original
  (`ux_invoices_one_locked_storno`), und das storniert markierte Original ist
  endgültig (`trg_invoices_canceled_final`), beides seit Migration 0032.

Der Storno-Beleg durchläuft die **gleiche Lock-Pipeline** wie das Original
(Domain-Validate → CII-XML → KoSIT → Klausel-Check → PDF/A-3 → Mustang →
Archive → DB-Lock → Audit → Auto-Backup), nur mit negativen Beträgen. Damit
ist er ein vollwertiger §14-konformer Beleg, der den ursprünglichen
neutralisiert.

### 5.3 Was Storno **nicht** ist

- **Kein Löschen.** Das Original bleibt für immer in der DB, im Archiv und im
  Audit.
- **Keine Korrektur.** Wer den Storno auch noch falsch macht, schreibt
  einen *zweiten* Storno auf den Storno (Anti-Storno) und danach die
  korrekte neue Rechnung. Selten, aber möglich.
- **Nicht nötig bei Entwürfen.** Eine Rechnung im Status `draft` ist noch kein
  Beleg (`locked_at IS NULL`, nichts draußen). Ein nicht benötigter Entwurf wird
  verworfen (§5.5). Löschen geht auch hier nicht: `trg_invoices_no_delete`
  blockt jedes `DELETE` (Migration 0028, R1-001).

### 5.4a Entwurf bearbeiten (Lauf F5a, ADR 0039 Nr. 134)

Ein offener Rechnungs-Entwurf ist noch kein Beleg im Sinn der Festschreibung;
`invoices_update_draft` überschreibt Kopf, Snapshots, Summen und Positionen.
Die Nummer bleibt — deshalb bleibt das Rechnungsdatum im Geschäftsjahr der
Nummer, und das Jahr muss offen sein. Nie bearbeitbar: festgeschriebene
Rechnungen (Korrektur nur per Storno), verworfene Entwürfe, Storno-Entwürfe
und Entwürfe eines laufenden Abo-Laufs. Jede Änderung schreibt in derselben
Transaktion das Audit `invoice.draft_updated` (Nummer, Brutto vorher und
nachher, Positionen, Empfängerwechsel). Beim Festschreiben friert der Server
den §19-Status am Beleg ein; ein Storno übernimmt den Status des Originals.

### 5.5 Entwurf verwerfen (Lauf F, W14-032)

Ein Rechnungs- oder Storno-Entwurf trägt seine Nummer ab dem Anlegen (der
Entwurf teilt den Nummernkreis mit den festgeschriebenen Belegen). Bisher gab es
keinen Weg, einen nicht benötigten Entwurf loszuwerden — er blockierte auf
Dauer die DSGVO-Anonymisierung seines Kontakts.

- **`invoices_void_draft`** (Re-Auth, Begründung mindestens 10 Zeichen) setzt
  `voided_at` (UTC) und `void_reason` und schreibt in derselben Transaktion das
  Audit `invoice.draft_voided` (Nummer, Geschäftsjahr, Betrag, Bezug bei
  Stornos, Grund). **Kein `DELETE`:** Die Nummer bleibt verbraucht und ist über
  den verworfenen Entwurf und das Audit nachvollziehbar — eine Lücke im
  Nummernkreis ist zulässig, wenn sie dokumentiert ist; eine erneute Vergabe
  wäre es nicht (§14 Abs. 4 Nr. 4 UStG, GoBD Rz. 58 ff. Vollständigkeit,
  Rz. 103 ff. Nachvollziehbarkeit).
- **Endgültig.** Ein verworfener Entwurf ändert sich nicht mehr, lässt sich
  nicht festschreiben und wird beim nächsten Stornieren nicht wiederverwendet
  (DB-Trigger, §2.1) und blockiert die Anonymisierung des Kontakts nicht
  mehr. Die Anonymisierung entfernt seinen Empfänger-Snapshot (Lauf F2c, ADR
  0039 Nr. 124 (3)): Ein nie ausgestellter Entwurf ist weder Buchungsbeleg noch
  Handelsbrief (§147 Abs. 1 AO, §14b UStG), Art. 17 Abs. 3 lit. b DSGVO trägt
  seine Aufbewahrung nicht; für den Nachweis der Nummer genügen Nummer, Datum,
  Betrag, Positionen, Zeitpunkt und Grund. Dasselbe gilt für zurückgezogene, nie
  festgeschriebene Angebote.
- **Nur Entwürfe.** Festgeschriebene Rechnungen korrigiert weiterhin nur ein
  Storno (§5.1).
- **Nebenwirkungen in derselben Transaktion:** ein Fortsetzungs-Marker eines
  Abo-Laufs, der auf den Entwurf zeigt, und eine Restore-Vormerkung (§5.4)
  werden entfernt.
- **Angebote:** Ein Angebots-Entwurf wird über das bestehende „Zurückziehen“
  verworfen (`status = 'canceled'`, Zeitpunkt und Grund, Audit
  `quote.cancel`); ein Angebot ist kein Buchungsbeleg, Re-Auth ist dafür nicht
  nötig.
- **Aus einem Angebot entstanden:** Das Verwerfen nimmt die Umwandlung zurück
  (Angebot wieder `accepted`, ohne Rechnungsbezug, Audit
  `quote.conversion_reverted`), sonst bliebe das angenommene Angebot auf einen
  verworfenen Beleg zeigend blockiert (Lauf F2c).
- **Steuerberater-Übergabe:** Das Paket listet in
  `nicht-festgeschriebene-nummern.csv` jede Rechnungs- und Storno-Nummer des
  Jahres ohne festgeschriebenen Beleg (verworfen mit Zeitpunkt und Grund, oder
  offen), die `LIESMICH.txt` nennt die Anzahl (Lauf F2c).
- Auch in einem bereits abgeschlossenen Geschäftsjahr erlaubt: Ein Entwurf ist
  nicht Teil der EÜR, das Verwerfen ändert den Abschluss-Snapshot nicht.

---

### 5.4 Nummernkreise nach einem Restore (Lauf F, D-24)

Eine Rechnungsnummer darf nur **einmal** vergeben werden (§14 Abs. 4 Nr. 4
UStG); eine dokumentierte Lücke ist zulässig, eine Doppelvergabe nicht. Spielt
der Inhaber ein **älteres** Backup ein, wären die seither vergebenen Nummern
sonst erneut frei. Deshalb:

- **Restore in dieselbe Instanz:** Phase A hält die Stände aller
  `doc_number_counters` der laufenden DB im Restore-Marker fest. Das erste
  Entsperren danach hebt jeden Kreis der eingespielten DB auf diesen Stand an,
  wo er höher war — in derselben Transaktion wie `backup.restore.applied`, je
  Kreis ein Audit `numbering.raised_after_restore` (alt/neu). Ein Hinweis nennt
  die angehobenen Kreise; die fehlenden Belege dazwischen sind die
  dokumentierte Lücke.
- **Disaster Recovery in eine leere Instanz:** Die späteren Stände sind
  unbekannt. Der Hinweis „Nummernkreise nach der Wiederherstellung“ und das
  Handbuch (`disaster-recovery`) verlangen, die zuletzt vergebene Nummer aus
  Mail-Ausgang bzw. Korrespondenz zu ermitteln und den Kreis unter
  Einstellungen → Backups → Nummernkreise anzuheben (`numbering_raise`,
  Re-Auth, Begründung Pflicht, Audit `numbering.raised`).
- **Fremder Stand (F3b).** Liegen Zähler der eingespielten DB über denen der
  ersetzten (oder fehlen dort), oder trägt ein festgeschriebener Beleg eine
  Nummer der ersetzten DB mit anderer ID (Marker `locked_ids`), stammt die
  ersetzte DB von einer anderen Instanz (z. B. neue Instanz mit Testdaten): Der
  Hinweis verlangt wie bei der leeren Instanz das Anheben. „Keine Lücke, keine
  Doppelvergabe“ meldet er nur bei genau gleichen Ständen (ADR 0040 Nr. 54).
- **Nur aufwärts.** `db::numbering::raise_in` senkt nie ab; `numbering_raise`
  lehnt gleiche oder kleinere Werte ab. Belege werden nicht verändert.
- Bis zur Freigabe der Pause nach dem Restore entstehen keine automatischen
  Belege (ADR 0040 Nr. 5); der Inhaber kann die Kreise vorher prüfen.
- **Entwürfe mit bereits vergebener Nummer (F1d).** Ein Entwurf trägt seine
  Nummer ab dem Anlegen. Phase A hält die Nummern aller festgeschriebenen
  Rechnungen, Stornos und Angebote der laufenden DB fest; ein Entwurf des
  eingespielten Stands mit einer dieser Nummern ist ein älterer Stand eines
  bereits ausgestellten (meist versandten) Belegs. Er wird vorgemerkt (Audit
  `numbering.restored_draft_flagged`, dringender Hinweis) und lässt sich erst
  festschreiben, nachdem der Inhaber ihn an das Original angepasst und das mit
  Re-Auth und Begründung bestätigt hat (`numbering_restored_draft_confirm`,
  Audit `numbering.restored_draft_confirmed`). Nicht neu nummeriert: Der Beleg
  soll unter der Nummer, die der Kunde hat, genau einmal in den Büchern stehen
  (ADR 0040 Entscheidung 35).
- **Abo-Termine (F1d).** Die nächsten Termine der Abo-Vorlagen und
  Kosten-Abos der ersetzten DB werden wie die Zähler nur vorwärts übernommen
  (Audit `…advanced_after_restore`); ein bereits abgerechneter Zeitraum wird
  nicht erneut in Rechnung gestellt oder gebucht. Die Kosten vorgerückter
  Kosten-Abos sind mit der ersetzten DB verworfen; der Hinweis nennt sie
  getrennt und verlangt das Nachtragen anhand des Kontoauszugs (Vollständigkeit,
  ADR 0040 Nr. 55). Ohne bekannten Stand verlangt jede Reihe mit offenem Termin,
  für die ein Lauf etwas erzeugen würde (Vorlage in der Laufzeit, Kosten-Abo mit
  Auto-Buchung), die Entscheidung des Inhabers — pausierte beim Fortsetzen;
  nachgeholte Rechnungen entstehen dann nur als Entwurf (ADR 0040 Nr. 56).

## 6. Aufbewahrung und kein Löschen-UI

§147 AO + UStG ⇒ 10 Jahre (Rechnungen, Buchungen, alles was zur Aufstellung
beigetragen hat). Klein.Buch hat:

- **Kein** UI-Button, um eine Rechnung, Kosten-Buchung, AfA-Zeile oder
  Privatbewegung zu **löschen**; die DB verbietet `DELETE` auf diesen Tabellen
  (ungelockte AfA-Buchungen ausgenommen, Reset-Pfad). Entwürfe werden verworfen
  (§5.5), nicht gelöscht. Gebuchte Zahlungen lassen sich weder ändern noch
  löschen, nur durch eine Gegenbuchung stornieren (§2.9).
- **Kein** UI-Button für „Datenbank leeren" oder „nur dieses Jahr löschen",
  und im Server keinen Factory Reset (E7).
- **Kein** Auto-Pruning von Belegen. Auch 12 Jahre alte Belege werden nicht
  automatisch gelöscht — das wäre eine Pflichtverletzung gegen den, der die App
  betreibt. (Rotiert werden nur Backups, §3.5 in `security.md`.)

Die Aufbewahrung über 10 Jahre stützt sich zusätzlich auf Off-Site-Backups und
die Sicherungen des Betreibers (`../operations-guide.md` §7).

**Keine automatische Löschung nach Fristablauf (Lauf F, ASVS 8.3.8,
entschieden).** Die Aufbewahrungsfrist beginnt mit dem Schluss des
Kalenderjahres der letzten Eintragung (§147 Abs. 4 AO) und verlängert sich,
solange die Unterlagen für noch nicht festsetzungsverjährte Steuern von
Bedeutung sind (§147 Abs. 3 Satz 5 AO, z. B. laufende Außenprüfung oder
Einspruch). Das kann die App nicht beurteilen; eine Automatik, die nach zehn
Jahren löscht, könnte gerade benötigte Unterlagen vernichten. Klein.Buch löscht
deshalb nie selbst. Nach Fristablauf entscheidet der Inhaber — mit klarer
Grenze (Korrektur Lauf F2c): Die Anonymisierung eines Kontakts entfernt nur
seine Stammdaten, den Empfänger-Snapshot verworfener Entwürfe und nie
ausgestellter, zurückgezogener Angebote sowie Bezeichnung und Notiz seiner
Abo-Vorlagen. Der Empfänger-Snapshot festgeschriebener Rechnungen, Stornos und
Angebote, deren PDF- und XML-Dateien im write-once-Archiv, `email_log.to_email`
und Audit-Details (etwa die Empfängeradresse bei `invoice.sent`) bleiben —
durch Trigger und `0o400` unveränderlich — bis zur Stilllegung der Instanz
(Volume und Backups beim Betreiber). Eine Beleg-Pseudonymisierung nach
Fristablauf ist eine Post-v1.0-Aufgabe (ADR 0039 Nr. 126 (9)). Das Handbuch
sagt das in „Aufbewahrung 10 Jahre“ und „DSGVO-Auskunft und Anonymisierung“.

### 6.1 Abgeschlossene Jahre und Festschreibungs-Snapshot (Lauf F, KB-0031)

Der Abschluss hält die EÜR-Eckwerte im unveränderlichen
`fiscal_year_locks` fest (Einnahmen, Ausgaben, AfA, Überschuss). Auswertungen
und Exporte eines abgeschlossenen Jahres rechnen weiter aus den Einzelbuchungen
— nur daraus entstehen Einzelaufstellung, DATEV-Stapel und Belege — und
gleichen die Eckwerte mit dem Snapshot ab (`fiscal_year::snapshot`). Eine
Abweichung bedeutet: Der Bestand eines festgeschriebenen Jahres hat sich
verändert (Eingriff von außen, Fehler) oder die Rechenregel einer neueren
Version rechnet anders. Beides muss sichtbar sein (GoBD Rz. 107 ff.
Unveränderbarkeit, Rz. 145 ff. Nachvollziehbarkeit); blockiert wird nichts,
denn gerade dann braucht der Steuerberater die Unterlagen.

- EÜR-Paket (`snapshotCheck`) und Geschäftsjahres-Übersicht
  (`snapshotMatches`) zeigen den Abgleich; die Anzeige schreibt nichts.
- Jeder Export trägt den Status ins Audit; bei einer Abweichung zusätzlich
  `euer.snapshot_mismatch` (Eckwerte beider Seiten, abweichende Felder, Version
  des Abschlusses) und ein dringender Hinweis, einmal je Jahr
  (`euer_snapshot_mismatch:<Jahr>`).
- Das Steuerberater-Paket stellt den LIESMICH einen Block voran: beide
  Eckwerte nebeneinander, maßgeblich für die abgegebene Erklärung ist der
  festgeschriebene Stand.

Aus dem Snapshot gerechnet wird bewusst nicht: er enthält nur Summen, und eine
EÜR, deren Summen nicht zu ihren Einzelposten passen, wäre selbst nicht
nachvollziehbar.

### 6.2 Abschluss gegen parallele Buchungen (Lauf F, R2-S2-GJLOCK, W14-034)

Zwischen der Prüfung „Jahr offen“ und dem Schreiben einer Buchung darf kein
Abschluss liegen, sonst stünde eine Buchung im geschlossenen Jahr, die im
Snapshot fehlt. `fiscal_year::guard::ensure_year_open` liefert deshalb eine
Buchungserlaubnis, die jede buchende Stelle bis nach dem Schreiben hält
(Rechnung anlegen und festschreiben — auch im Abo-Lauf —, Zahlung, Storno,
Kosten, Kosten-Abo, Anlage, Abgang, Privatbewegung, Angebot festschreiben,
Vertrag zum Angebot, AfA buchen und zurücksetzen). Der Abschluss nimmt die
exklusive Gegenseite: er beginnt erst, wenn keine Buchung mehr läuft, und
solange er läuft, wartet jede neue Buchung und prüft danach erneut (ADR 0040
Nr. 48). AfA-Buchungen prüfen das Jahr zusätzlich in ihrer Schreib-Transaktion.
Festschreiben der AfA und Anlagen, Snapshot, Protokoll und Audit laufen in
**einer** Transaktion; ein Abbruch hinterlässt keine festgeschriebene AfA ohne
Abschluss. Offene Entwürfe des Jahres blockieren den Abschluss nicht (sie sind
nicht Teil der EÜR und lassen sich danach verwerfen, §5.5); die Übersicht
meldet sie vorab, Antwort und Audit `fiscal_year.close` nennen sie zusammen mit
den Belegzahlen (Rechnungen, Stornos, Kosten, Privatbewegungen).

---

## 7. Factory Reset (nur Desktop-Fassung, entfernt)

ADR 0036 beschrieb für die Desktop-Fassung einen zweiphasigen Komplett-Nuke der
lokalen Instanz mit Export-First-Pflicht bzw. getippter Aufbewahrungs-Quittung,
Tipp-Bestätigung und Passphrase-Prüfung; Off-Site-Backups blieben bestehen.

Im Server-Betrieb ist er **abgeschaltet** (ADR 0039, E7, Amendment ADR 0036):
Über das Netz wäre eine Totallöschung die gefährlichste Einzelaktion — eine
gekaperte Sitzung samt Passphrase könnte die zehnjährige Aufbewahrung
vernichten. Die Commands waren ab W5 gesperrt, der Code ist seit W11 entfernt.
Liegt `FACTORY_RESET_PENDING.json` im Datenverzeichnis, bricht der Start ab und
nichts wird gelöscht (`tests/server_startup_test.rs`,
`factory_reset_marker_is_refused_not_applied`).

Selektives Löschen einzelner Belege gab und gibt es nicht.

---

## 8. Was passiert wenn jemand die DB direkt manipuliert?

Realistische Bedrohungs-Annahme: jemand öffnet die SQLite-Datei mit dem
SQLCipher-CLI und der Passphrase (die er kennt) und versucht einen Beleg zu
ändern.

**Was greift:**

- **DB-Trigger.** `trg_invoices_immutable` etc. feuern in der SQLite-Engine,
  nicht in der App. Wer in der SQLCipher-CLI einen UPDATE auf einer gelockten
  Rechnung versucht, bekommt einen RAISE-Fehler.
- **Archive-Hash.** Wer das gelagerte PDF/XML ändert, fliegt beim nächsten
  Abruf oder Integritätslauf auf — Audit-Eintrag `archive.integrity_fail`. Der
  gelagerte Beleg ist dann zwar manipuliert, aber das System merkt es und sagt
  es laut.
- **Audit-Append-only.** Eine direkte Manipulation der Audit-Tabelle scheitert
  am `trg_audit_no_update`/`_no_delete`. Wer die Trigger löscht, hinterlässt eine
  Abweichung vom Schema der Migrationen; Klein.Buch prüft beim Entsperren die
  Schema-Version, die Prüfsummen der angewendeten Migrationen und die
  Fremdschlüssel der ganzen DB (`PRAGMA foreign_key_check`, R1-012 — ein
  verwaister Verweis blockiert das Entsperren mit 409 `db_integrity_failed`;
  stehen Migrationen an, schon **vor** der Umstellung und vor dem
  Pflicht-Backup, damit die DB auf dem alten Schema bleibt und das vorige Image
  sie öffnet, Lauf F2c), nicht aber den Bestand der Trigger.
- **Passphrase = DB-Schlüssel.** Ohne Passphrase kein Pool-Open. Die DB ist per
  SQLCipher verschlüsselt; Offline-Zugriff ohne Passphrase ergibt keine
  lesbaren Daten.

**Was nicht 100 % verhindert werden kann.** Wer root auf dem Host ist bzw. Code
als Server-Benutzer ausführt und alle Mechanismen umgeht (Trigger löscht, Hash
neu berechnet, Audit-Tabelle neu schreibt), kann konsistente Spuren
hinterlassen — dann ist es kein Software-Problem mehr, sondern ein Problem der
Host-Sicherheit. Klein.Buch dokumentiert, was es schützt; alles darüber hinaus
ist betrieblicher Schutz (Host-Zugriff, Volume-Verschlüsselung, Snapshots,
Off-Site-Backups).

---

## 9. Tests

GoBD-Tests sind verteilt (Integrationstests unter `src-tauri/tests/`,
Unit-Tests in den Modulen):

- **Lock- und No-Delete-Trigger:** `tests/migration_0028_test.rs` (Rechnung:
  geschützte Felder einzeln, Unlock-Schutz, erlaubte Übergänge, `DELETE` auf
  Entwurf und Beleg; Angebot, Kosten, Privatbewegung, Anlage, AfA-Buchung;
  Storno-Paar atomar), `tests/migration_0032_test.rs` (Positionen
  festgeschriebener/verworfener Belege, Archiv ohne `DELETE`, höchstens ein
  festgeschriebener Storno je Original samt Abbruch der Migration bei
  Bestandsduplikaten, Storno-Paar endgültig und idempotent, ein
  Standard-Zahlungskonto, Anlagen-Abgang endgültig, Verwerfen von Entwürfen,
  Integritäts-Historie, Fremdschlüssel-Prüfung vor und nach der Umstellung;
  F2c: Anonymisierung verworfener Entwürfe, Spaltenliste des Triggers,
  Umwandlung zurücknehmen, Angebot zurückziehen, halbes Storno-Paar,
  veräußerte Anlage, AfA-Nachholen beim Abgang), `tests/invoice_lock_pipeline_test.rs`
  (`gobd_trigger_blocks_core_field_update_after_lock`),
  `tests/quotes_repo_test.rs` (`locked_quote_core_fields_are_immutable`),
  `tests/expenses_repo_test.rs` (`locked_expense_core_fields_are_immutable`),
  `tests/private_movements_repo_test.rs`
  (`locked_movement_core_fields_are_immutable`), `tests/assets_repo_test.rs`
  (`locked_asset_core_fields_immutable_via_trigger`,
  `depreciation_entry_mutable_until_locked_then_immutable`).
- **Append-only-Protokolle:** `tests/backup_log_test.rs` und
  `tests/email_log_test.rs` (`append_only_rejects_update_and_delete`),
  `tests/fiscal_year_lock_test.rs` (`protocol_is_immutable_and_lock_for_year_idempotent`),
  `tests/legal_documents_repo_test.rs` (`core_fields_immutable_and_no_delete`).
  `tests/migration_0032_test.rs` (`audit_log_is_append_only`,
  `package_revisions_are_immutable_and_append_only` — W13-TRIGGER-TESTS).
- **Steuerstatus-Historie:** `tests/migration_0033_test.rs`
  (`append_only_delete_and_update_blocked_revoke_once`,
  `checks_reason_status_date_and_binding`,
  `backfill_matches_old_result_for_every_year`), `tests/tax_status_flow_test.rs`
  (Eintrag und Widerruf, abgeschlossene Jahre, festgeschriebene Rechnungen
  bleiben), Unit-Tests in `src/domain/tax_status.rs`.
- **Zahlungsbuch:** `tests/payment_ledger_test.rs` (Übernahme und Abbruch der
  Migration 0034, append-only, Gegenbuchung passend und einmalig, keine Buchung
  im abgeschlossenen Jahr bzw. auf Entwurf, Storno-Beleg oder stornierte
  Rechnung, Cache gebunden, Erfassen/Stornieren/neu Erfassen, Ablehnungen,
  EÜR/ELSTER/DATEV/Gesamtumsatz mit Gegenbuchung), `tests/server_restore_test.rs`
  (Restore eines Backups mit Schema 33), Unit-Tests in
  `src/db/repo/invoice_payments.rs` und `src/euer/datev_csv.rs`; Oberfläche:
  `src/routes/invoices/paymentHistory.test.ts` (Verweise Zahlung ↔
  Gegenbuchung, wirksame Zahlungen, frühestes Storno-Datum ohne stornierte
  Paare, Grund wie im Server).
- **Archiv:** Unit-Tests in `src/archive/store.rs` (Schreiben mit `0o400`,
  idempotent, Waise beiseitelegen, neue Dateiversion, anderer Inhalt abgelehnt,
  Tamper beim Lesen, Pfadtrenner im Namen) und `src/archive/integrity_check.rs`
  (intakt, manipuliert, fehlend, nicht lesbar);
  `tests/invoice_lock_pipeline_test.rs`
  (`archive_tamper_after_lock_is_detected`),
  `tests/receive_einvoice_xml_view_test.rs`
  (`detects_tamper_on_archive_modification`), `tests/server_files_test.rs`
  (Abruf über HTTP, Audit), `tests/server_startup_test.rs`
  (`archive_selftest_refuses_ineffective_chmod`).
- **Storno:** Unit-Tests in `src/domain/storno.rs` (negierte Preise, Positionen,
  Summe, Referenz, `is_storno_for`); `tests/invoice_lock_pipeline_test.rs`
  (`storno_creates_new_invoice_and_marks_original`), `tests/cores_test.rs`
  (`storno_core_cancels_original_and_locks_storno`),
  `tests/euer_aggregation_test.rs`
  (`storno_reverses_in_storno_year_without_touching_prior_year`).
- **Geschäftsjahr:** `tests/fiscal_year_lock_test.rs`
  (`close_year_books_locks_and_blocks_reposting`,
  `close_year_requires_unlocked_session`), `tests/fiscal_year_from_date_test.rs`
  (GJ aus dem Belegdatum in Europe/Berlin, Client-GJ ignoriert),
  `tests/fiscal_year_guard_test.rs` (Lauf F: alle buchenden Commands nach dem
  Abschluss — KB-0032 —, Abschluss wartet auf Buchungen, Buchung wartet auf den
  Abschluss und prüft erneut, Audit mit Belegzahlen und offenen Entwürfen),
  `tests/audit_export_test.rs` (Audit jedes Exports, Snapshot-Abweichung),
  Unit-Tests in `src/fiscal_year/guard.rs` und `src/fiscal_year/snapshot.rs`.
- **Kein Factory Reset im Server:** `tests/server_startup_test.rs`
  (`factory_reset_marker_is_refused_not_applied`).

---

## Letzte Verifikation

Stand: 2026-10-25 (Lauf F, Blöcke F2a/F2b/F2c; Schema v32). Quellen:
`src-tauri/migrations/0001_init.sql`, `0006_legal_documents.sql`,
`0011_depreciation.sql`, `0013_fiscal_year_locks.sql`, `0015_email_log.sql`,
`0019_packages.sql`, `0026_backup_log.sql`, `0028_append_only_hardening.sql`,
`0032_gobd_hardening.sql`; `src/commands/invoices.rs` (`void_draft_core`);
`src/archive/store.rs`, `src/archive/integrity_check.rs`, `src/db/mod.rs`
(`prepare_filesystem`), `src/locks.rs`, `src/db/repo/audit_log.rs`,
`src/fiscal_year/{guard,lock,snapshot}.rs`, `src/commands/euer.rs`; Testliste per Verzeichnis-Listing von
`src-tauri/tests/` und den Test-Namen geprüft. Nachtrag 2026-10-27 (Lauf 2026.11.1, V1; Schema v33):
§2.8 und §9 um `tax_status_changes` ergänzt, gegen
`migrations/0033_tax_status_history.sql`, `src/domain/tax_status.rs`,
`src/commands/tax_status.rs` und `tests/migration_0033_test.rs` geprüft. Nachtrag
2026-10-31 (Lauf 2026.11.1, V2; Schema v34): §2.1 und §2.9 um das Zahlungsbuch
ergänzt, gegen `migrations/0034_invoice_payment_ledger.sql`,
`src/db/repo/invoice_payments.rs`, `src/commands/invoices.rs`
(`reverse_payment_core`) und `tests/payment_ledger_test.rs` geprüft; §2.9
„Oberfläche“ gegen `src/routes/invoices/[id]/+page.svelte`,
`src/routes/invoices/paymentHistory.ts` und die Seiten `payment`/`cancel`
geprüft.
