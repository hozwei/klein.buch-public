# GoBD-Implementation

> Vertiefung zu „GoBD" in `../ARCHITECTURE.md` §7. Wie die GoBD-Hardline
> tatsächlich im Code umgesetzt ist: was passiert wann, welcher Trigger feuert,
> welche Tabellen sind betroffen, wo verboten ich was. Diese Datei ist die
> Audit-/Maintainer-Referenz und beantwortet die Frage „Wo greift Klein.Buch
> ein, wenn jemand etwas Verbotenes versucht?"

ADR-Basis: 0006 (Immutability via DB-Trigger, Storno statt Löschung).
Ergänzungen: 0027 (GJ-Lock), 0036 (Factory Reset als einzige sanktionierte
Total-Löschung).

---

## 1. Hardline auf einer Seite

1. **Festgeschriebene Belege sind unveränderlich.** Sobald `locked_at` gesetzt
   ist (Rechnung, Angebot, Storno-Beleg, Kosten, AfA-Buchung nach GJ-Lock,
   Privatbewegungen ab Insert), erlaubt der DB-Trigger nur noch State-
   Transitionen, keine Änderung der Kernfelder.
2. **Storno statt Löschung.** Ein fehlerhafter Beleg wird durch einen Storno-
   Beleg neutralisiert. Das Original bleibt erhalten und wird als `canceled`
   markiert.
3. **Archive ist write-once.** Beleg-PDF und -XML werden mit SHA-256 in
   `archive/` abgelegt, `chmod 0o400` (read-only), `trg_archive_no_update`
   schützt `hash`/`path`/`size`. Re-Hash beim Lesen detektiert Tamper.
4. **Audit-Log ist append-only.** Triggers verbieten UPDATE und DELETE auf
   `audit_log`.
5. **Aufbewahrung 10 Jahre.** Es gibt kein Löschen-UI für Belege.
6. **Einzige sanktionierte Total-Löschung: Factory Reset** (ADR 0036).
   Selektives Belege-Löschen bleibt verboten.

---

## 2. Lock-Trigger im Detail

### 2.1 `trg_invoices_immutable`

**Greift bei:** UPDATE auf `invoices` wenn `OLD.locked_at IS NOT NULL`.

**Verboten:** Änderung von `invoice_number`, `date`, `delivery_date`,
`contact_id`, `is_kleinunternehmer`, `direction`, `fiscal_year`,
`subtotal_cents`, `tax_amount_cents`, `total_cents`, allen Buyer-Snapshot-
Spalten, allen Item-Beträgen.

**Erlaubt** (State-Transitions):
- `status` (`issued` → `sent` → `canceled`)
- `paid_amount_cents`, `paid_at` (Teilzahlung im Folge-GJ ist explizit OK)
- `sent_at`
- `canceled_at`, `is_storno_for`-Bindung (am Storno-Beleg)
- `notes`
- `pdf_archive_id`, `xml_archive_id` (werden beim Lock einmalig gesetzt;
  Re-Issue erzeugt neuen Beleg, nicht neues PDF)

Die Whitelist liegt im Trigger-SQL als `WHERE NEW.feld IS DISTINCT FROM
OLD.feld` für jedes erlaubte Feld. Wer hier ein Feld vergisst, lässt
versehentlich eine Mutation durch — also: Trigger-Tests in
`tests/triggers_test.rs` prüfen für **jedes** verbotene Feld einzeln,
dass der Trigger feuert.

### 2.2 `trg_quotes_immutable`

Analog für `quotes`, mit zusätzlicher Erlaubnis für `accepted_at`,
`rejected_at`, `converted_at`, `signature_archive_id` (signiertes PDF wird
nach `accepted` archiviert, das ist Status-Transition).

### 2.3 `trg_expenses_immutable`

Greift sofort beim Insert — Kosten sind **sofort gelockt**, kein separates
„Lock-Issue". `paid_date` ist Update-erlaubt (kann nachgetragen werden, wenn
beim Erfassen noch unbekannt). Alle Beträge sind sofort fixiert.

### 2.4 `trg_assets_immutable`

Greift erst nach Setzung von `locked_after_year` (durch `close_year` im
`fiscal_year::lock`). Vorher sind Anlagen Stammdaten; nachher gefroren.

### 2.5 `trg_depreciation_immutable`

Greift sobald `is_locked=1` gesetzt ist (gleicher Zeitpunkt:
`fiscal_year::close_year`).

### 2.6 `trg_private_movements_immutable`

Greift sofort beim Insert. Privatbewegungen sind ab dem Eintrag eingefroren;
keine Korrektur, keine nachträgliche Begründungs-Änderung. Wer sich vertippt,
schreibt eine Gegenbuchung und einen Kommentar in der Notiz — das
Ausgangs-Movement bleibt.

### 2.7 Append-only-Tabellen (no-update + no-delete)

`audit_log`, `email_log`, `backup_log`, `fiscal_year_locks`,
`legal_documents`, `quote_legal_documents`, `package_revisions`.

Diese sieben Tabellen haben **kein** UPDATE und **kein** DELETE.
Korrigieren funktioniert nur per Insert eines neueren Datensatzes.

---

## 3. Archive

### 3.1 Schreiben

`archive::store(kind, bytes)`:

1. UUIDv7 generieren.
2. Datei nach `%APPDATA%\…\archive\{YYYY}\{kind}\{uuid}.{ext}` schreiben.
3. SHA-256 berechnen.
4. `archive_entries`-Insert: `(id, kind, path, hash, size_bytes, created_at)`.
5. `chmod 0o400` (read-only) — auf Windows entsprechend
   `FILE_ATTRIBUTE_READONLY` + ACL.

**Idempotenz.** Vor dem Schreiben wird über den Hash gesucht. Wenn ein
Eintrag mit demselben Hash existiert, wird die existierende ArchiveId
zurückgegeben — keine Duplikate.

### 3.2 Lesen + Re-Hash

`archive::store::read(archive_id)`:

1. `archive_entries` lookup → `(path, hash)`.
2. Datei lesen.
3. SHA-256 neu berechnen.
4. Mismatch ⇒ `archive.integrity_tamper`-Audit + Fehler.
5. Datei nicht vorhanden ⇒ `archive.integrity_missing`-Audit + Fehler.

**Wichtig** (G1-HARDEN.4): Tamper und Waisen sind **getrennte** Events. Eine
fehlende Datei ist nicht automatisch eine Manipulation — sie kann auch ein
verschwundenes Backup-Volume sein. Der Cron meldet beides, aber getrennt.

### 3.3 Re-Hash-Cron

`scheduler::integrity_check_cron` läuft monatlich. Resultat landet in
`archive_integrity_checks` (Aggregat) und produziert pro Fund ein Audit-Event.
Manuelle Trigger über `commands::system::archive_integrity_run`.

---

## 4. Audit-Log

Die `audit_log`-Tabelle ist das **forensische** Protokoll. Alle nicht-
trivialen Mutationen schreiben einen Eintrag:

- Beleg-Mutationen: `invoice.created`, `invoice.issued`, `invoice.sent`,
  `invoice.canceled` (für Storno-Erzeugung), `quote.created`/`issued`/
  `accepted`/`rejected`/`converted`/`canceled`, `expense.created`,
  `private_movement.created`.
- Archiv: `archive.stored`, `archive.integrity_tamper`,
  `archive.integrity_missing`.
- Fiskal: `fiscal_year.closed`, `fiscal_year.reopened` (gibt es nicht — der
  Eintrag würde fehlen, weil close unumkehrbar ist).
- AfA: `depreciation.accrued`, `depreciation.year_locked`.
- Backup/Restore: `backup.created` (zusätzlich `backup_log`), `backup.restored`,
  `backup.factory_reset_applied`.
- DSGVO: `dsgvo.export`, `contact.anonymized`.
- Mail/OAuth: `mail.sent` (zusätzlich `email_log`), `mail.oauth_connected`,
  `mail.oauth_disconnected`.

**Hardline.** Trigger blocken UPDATE und DELETE; `audit_log` wächst monoton.
Bei Performance-Bedenken: 10 Jahre × erwartetes Volumen einer §19-Praxis
ergibt typischerweise < 1 GB; SQLite handhabt das problemlos.

**Sensitive Daten.** Im `details`-JSON niemals:

- App-/Daten-Passphrase.
- SMTP-Passwort, OAuth-Refresh-Token, OAuth-Access-Token, SFTP-Backup-Passwort.
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

- `invoice_number = ST-{YYYY}-{NNNN}` (eigener Counter im selben Jahr).
- `is_storno_for = original.id`.
- Alle Positionen mit **negierten Mengen** (also negativem `quantity_thousandths`).
- Selbe Buyer-Snapshot, selbe Klausel-Lage, selbes `delivery_date`,
  selber `fiscal_year`.
- Original wird parallel auf `status = 'canceled'`, `canceled_at = now`
  gesetzt (im selben Repo-Aufruf, eine Transaktion).

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
- **Kein Ersatz für Storno bei Drafts.** Eine Rechnung im Status `draft` kann
  einfach geändert oder gelöscht werden — `locked_at IS NULL`, kein Beleg
  draußen, GoBD greift noch nicht.

---

## 6. Aufbewahrung und kein Löschen-UI

§147 AO + UStG ⇒ 10 Jahre (Rechnungen, Buchungen, alles was zur Aufstellung
beigetragen hat). Klein.Buch hat:

- **Kein** UI-Button, um eine Rechnung, Kosten-Buchung, AfA-Zeile oder
  Privatbewegung zu **löschen**. Drafts können verworfen werden (`DELETE` ist
  vor `locked_at` erlaubt), Festgeschriebenes bleibt.
- **Kein** UI-Button für „Datenbank leeren" oder „nur dieses Jahr löschen".
- **Kein** Auto-Pruning. Auch wenn jemand 12 Jahre alte Belege hat, werden
  sie nicht automatisch gelöscht — das wäre eine Pflichtverletzung gegen den,
  der die App betreibt.

**Das einzige UI, das tatsächlich Daten zerstört, ist Factory Reset.** Siehe §7.

---

## 7. Factory Reset als einzige sanktionierte Total-Löschung

ADR 0036, G1-RESET. Details in `security.md` (Mechanik der Phasen).

**Was es ist:** ein zweiphasiger, race-freier Komplett-Nuke der lokalen
Instanz. DB + Archiv + lokale Backups + Settings → fertig. Beim nächsten Start
läuft wieder der Onboarding-Wizard.

**Warum es trotzdem GoBD-konform ist:**

- **Export-First-Pflicht.** Wenn festgeschriebene Belege existieren, muss
  vor dem Reset entweder ein vollständiger Steuerberater-Export
  (`migration_export_run`) gemacht **oder** eine getippte Aufbewahrungs-
  Quittung angegeben werden („Ich verwahre die Belege außerhalb dieser
  Software"). Diese Quittung landet in `audit_log` als
  `factory_reset.consent_given`.
- **Off-Site-Backups bleiben.** Cloud-Ordner und SFTP-Ziele werden **nicht**
  gelöscht. Wer dort liegende Backups loswerden will, muss das selbst tun —
  das ist der Punkt, an dem GoBD und Geräte-Weitergabe sich trennen.
- **Mehrstufige Absicherung.** GoBD-Warnung → Export → Tipp-Bestätigung
  („LÖSCHEN") → Passphrase-Eingabe (server-verifiziert über
  `backup::verify_passphrase`) → finaler confirmDialog. Vier explizite Hürden.
- **Audit nach Restart.** Phase B legt einen
  `factory_reset.applied`-Audit-Eintrag in das frisch erzeugte (leere) Audit-
  Log, sobald die Onboarding-DB existiert.

**Was es nicht ist:** selektives Löschen einzelner Belege. Dazu gibt es bewusst
keinen Weg. Wer „nur das Jahr 2023 weghaben" will, kann das nicht tun —
GoBD verbietet es, und Klein.Buch macht es auch technisch unmöglich.

---

## 8. Was passiert wenn jemand die DB direkt manipuliert?

Realistische Bedrohungs-Annahme: jemand öffnet die SQLite-Datei mit dem
SQLite-CLI, dem SQLCipher-Schlüssel (der ihm bekannt ist, weil er ihn aus
der App rauskramen konnte) und versucht einen Beleg zu ändern.

**Was greift:**

- **DB-Trigger.** `trg_invoices_immutable` etc. feuern in der SQLite-Engine,
  nicht in der App. Wer in SQLCipher-CLI einen UPDATE auf einer gelockten
  Rechnung versucht, bekommt einen RAISE-Fehler.
- **Archive-Hash.** Wer das gelagerte PDF/XML ändert, fliegt beim nächsten
  Re-Hash auf — Audit-Eintrag `archive.integrity_tamper`. Der gelagerte
  Beleg ist dann zwar manipuliert, aber das System merkt es und sagt es laut.
- **Audit-Append-only.** Eine direkte Manipulation der Audit-Tabelle
  scheitert am `trg_audit_no_update`/`_no_delete`. Wer die Trigger droppt,
  fliegt beim nächsten App-Start auf (Schema-Version-Mismatch oder
  fehlende Trigger werden über die Migrations-Hashes erkennbar).
- **App-Login = Daten-Login.** Ohne Passphrase kein Pool-Open. Die DB ist
  per SQLCipher verschlüsselt; offline-Zugriff ohne Passphrase ergibt eine
  Zufallszahl, keinen Beleg.

**Was nicht 100 % verhindert werden kann.** Wer privilegierter root auf dem
Rechner ist und alle Mechanismen umgeht (Trigger droppt, Hash neu berechnet,
Audit-Tabelle neu schreibt, eine neue Schema-Version-Migration einspielt),
kann konsistente Spuren hinterlassen — aber dann ist es kein
Software-Problem mehr, sondern ein Betriebs-System-Sicherheits-Problem.
Klein.Buch dokumentiert, was es schützt; alles darüber hinaus ist
betrieblicher Schutz (verschlüsselte Festplatte, Account-Trennung, etc.).

---

## 9. Tests

GoBD-Tests sind verteilt:

- **Trigger-Tests** (`tests/triggers_test.rs`): pro Trigger ein Positiv-
  und ein Negativtest. Lockt einen Beleg, versucht jedes verbotene Feld
  zu ändern, erwartet RAISE; ändert ein State-Feld, erwartet OK.
- **Archive-Tests** (`tests/archive_test.rs`): Roundtrip + Tamper-Detection
  + Waisen-Detection + idempotenter Re-Store.
- **Storno-Tests** (`tests/storno_test.rs`): Storno-Erzeugung negiert
  Mengen, lockt sich selbst, setzt Original auf `canceled`, fängt Anti-Storno
  korrekt ab.
- **Audit-Tests** (`tests/audit_log_test.rs`): UPDATE/DELETE auf `audit_log`
  scheitert; Audit-Events tauchen mit erwarteten Feldern auf.
- **Fiscal-Year-Tests** (`tests/fiscal_year_test.rs`): `close_year` lehnt
  laufendes Jahr ab; schreibt `fiscal_year_locks`-Snapshot; setzt Asset/AfA
  auf locked; Storno bleibt nach Lock möglich.
- **Factory-Reset-Tests** (`tests/factory_reset_test.rs`): Gating verlangt
  Export oder Quittung; Marker wird geschrieben; Phase B nukt das `data_dir`;
  Off-Site-Pfade bleiben unangetastet; Onboarding läuft beim nächsten Start.

---

## Letzte Verifikation

Stand: 2026-05-26, Schema v27, ADRs 0006/0027/0036. Quelle:
`klein-buch/src-tauri/migrations/*.sql` (Trigger-Definitionen) +
`src/archive/`, `src/fiscal_year/`, `src/backup/factory_reset.rs`.
