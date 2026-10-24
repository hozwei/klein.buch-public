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
   markiert. `DELETE` auf Rechnungen, Angeboten, Kosten, Privatbewegungen und
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
`direction`, `delivery_date`, `due_date`, `currency_code`, `pdf_template`,
`is_storno_for`, `derived_from_quote_id`, `paid_to_account_id`, `payment_note`,
`pdf_archive_id`, `xml_archive_id`, alle `buyer_*`- und `seller_*`-Snapshot-
Spalten.

**Änderbar** (Status-Übergänge): `status`, `paid_amount_cents`, `paid_at`,
`payment_history_json` (Zahlung, auch im Folge-GJ), `sent_at`, `canceled_at`,
`canceled_by_storno_id`, `cancel_reason`, `updated_at`. Das Festschreiben selbst
(Setzen von `locked_at`, Validierung, Archiv-IDs) feuert nicht, weil
`OLD.locked_at` dabei noch `NULL` ist.

**`trg_invoices_no_delete`:** jedes `DELETE` scheitert, auch bei Entwürfen.
Ein Entwurf wird über `status = 'canceled'` neutralisiert.

Tests: `tests/migration_0028_test.rs` (je geschütztem Feld bzw. Feldgruppe ein
Fall, Unlock-Schutz, erlaubte Übergänge, `DELETE` auf Entwurf und Beleg),
`tests/invoice_lock_pipeline_test.rs`
(`gobd_trigger_blocks_core_field_update_after_lock`).

### 2.2 `trg_quotes_immutable` + `trg_quotes_no_delete`

Analog für `quotes`: geschützt sind u. a. `quote_number`, `quote_date`,
Beträge, `contact_id`, `fiscal_year`, `is_kleinunternehmer`, `valid_until`,
`currency_code`, `pdf_template` und die Snapshots. Änderbar bleiben Status und
Zeitpunkte von Annahme, Ablehnung, Storno und Konvertierung
(`converted_invoice_id`) sowie `pdf_archive_id` (Re-Render-Pfad von
`ensure_quote_pdf`). Kein `DELETE`.

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
Kein `DELETE`.

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
`package_revisions`; dazu `legal_documents` und `quote_legal_documents`
(Kernfelder unveränderlich, kein `DELETE`; bei `legal_documents` bleibt das
Aktivieren/Deaktivieren möglich). Korrigiert wird nur per Insert eines neueren
Datensatzes (z. B. neue Paket-Revision).

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
   `create_new`), `fsync`, Schreibschutz `0o400` **vor** dem Sichtbarwerden,
   dann `hard_link` an den Zielnamen. Der Hardlink scheitert, wenn das Ziel
   schon existiert — überschrieben wird nie (Ausnahme: Dateisysteme ohne
   Hardlinks, dort `rename` nach vorheriger Prüfung unter der Pfad-Sperre;
   deshalb gehören DB und Archiv nicht auf NFS/SMB/exFAT). Temp-Name entfernen,
   Ordner-`fsync`.
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
  `backup.mirror.failed`, `backup.rotation.prune`, `backup.pre_migration`,
  `backup.restore.requested` (steht im Pre-Restore-Backup des ersetzten
  Stands), `backup.restore.applied` (in der wiederhergestellten DB),
  `backup.passphrase.setup`, `backup.passphrase.changed`.
- Anmeldung: `auth.setup.ok`, `auth.login.ok`, `auth.reauth.ok`,
  `auth.totp.setup`, `auth.totp.renewed`, `auth.recovery.renewed`.
  **Fehlversuche stehen nicht im Audit-Log**, nur im Server-Log (ADR 0039 E5).
- DSGVO und Exporte: `dsgvo.export`, `contact.anonymize`, `migration.export`,
  `euer.stb_export`.
- Mailkonten: `mail_account.create`, `mail_account.update`,
  `mail_account.delete`, `mail_account.test_send`; jeder Versand zusätzlich im
  append-only `email_log`.

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
- Selber Buyer-Snapshot, selbe Klausel-Lage, selbes `delivery_date`,
  selbes `fiscal_year` wie das Original (auch bei einem Storno im Folgejahr;
  fachliche Prüfung offen, ADR 0039 Nr. 18).
- Beim Festschreiben werden Storno-Lock und `canceled` am Original in **einer**
  Transaktion gesetzt (`tests/migration_0028_test.rs`,
  `lock_with_pair_cancel_*`).

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
- **Nicht nötig bei Entwürfen.** Eine Rechnung im Status `draft` lässt sich
  ändern (`locked_at IS NULL`, kein Beleg draußen) und über `status =
  'canceled'` verwerfen. Löschen geht auch hier nicht: `trg_invoices_no_delete`
  blockt jedes `DELETE` (Migration 0028, R1-001).

---

## 6. Aufbewahrung und kein Löschen-UI

§147 AO + UStG ⇒ 10 Jahre (Rechnungen, Buchungen, alles was zur Aufstellung
beigetragen hat). Klein.Buch hat:

- **Kein** UI-Button, um eine Rechnung, Kosten-Buchung, AfA-Zeile oder
  Privatbewegung zu **löschen**; die DB verbietet `DELETE` auf diesen Tabellen
  (ungelockte AfA-Buchungen ausgenommen, Reset-Pfad). Entwürfe werden storniert
  bzw. verworfen, nicht gelöscht.
- **Kein** UI-Button für „Datenbank leeren" oder „nur dieses Jahr löschen",
  und im Server keinen Factory Reset (E7).
- **Kein** Auto-Pruning von Belegen. Auch 12 Jahre alte Belege werden nicht
  automatisch gelöscht — das wäre eine Pflichtverletzung gegen den, der die App
  betreibt. (Rotiert werden nur Backups, §3.5 in `security.md`.)

Die Aufbewahrung über 10 Jahre stützt sich zusätzlich auf Off-Site-Backups und
die Sicherungen des Betreibers (`../operations-guide.md` §7).

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
  Schema-Version und die Prüfsummen der angewendeten Migrationen, nicht aber
  den Bestand der Trigger.
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
  Storno-Paar atomar), `tests/invoice_lock_pipeline_test.rs`
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
  Für `audit_log` und `package_revisions` gibt es keinen eigenen Trigger-Test;
  die Trigger stehen in `0001_init.sql` bzw. `0019_packages.sql`.
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
  (GJ aus dem Belegdatum in Europe/Berlin, Client-GJ ignoriert).
- **Kein Factory Reset im Server:** `tests/server_startup_test.rs`
  (`factory_reset_marker_is_refused_not_applied`).

---

## Letzte Verifikation

Stand: 2026-10-24 (Phase W, Block W13; Schema v31). Quellen:
`src-tauri/migrations/0001_init.sql`, `0006_legal_documents.sql`,
`0011_depreciation.sql`, `0013_fiscal_year_locks.sql`, `0015_email_log.sql`,
`0019_packages.sql`, `0026_backup_log.sql`, `0028_append_only_hardening.sql`;
`src/archive/store.rs`, `src/archive/integrity_check.rs`, `src/db/mod.rs`
(`prepare_filesystem`), `src/locks.rs`; Testliste per Verzeichnis-Listing von
`src-tauri/tests/` und den Test-Namen geprüft.
