# Modul-Referenz

> Vertiefung zu §2 in `../ARCHITECTURE.md`. Eine Zeile pro Datei wäre zu wenig
> und ein Doc-Comment-Dump aus dem Code zu viel. Diese Datei beschreibt jedes
> Modul-Verzeichnis des Backends (`src-tauri/src/`; der Ordnername stammt aus der
> Desktop-Fassung, das Crate baut seit Phase W das Binary `klein-buch-server`,
> ADR 0039) auf der Ebene, auf der ein neuer Maintainer arbeitsfähig wird:
> Verantwortung, öffentliche Schnittstelle, Invarianten, Tests, ADR-Verweise.

Konvention: Verzeichnisse mit `mod.rs` sind eigenständige Module. Der
HTTP-Server `server/` hat eine eigene Sektion (§15); die einzelnen
`*.rs`-Dateien im `src/`-Wurzel-Verzeichnis (`app_ctx.rs`, `branding.rs`,
`build_info.rs`, `clock.rs`, `config.rs`, `downloads.rs`, `error.rs`, `lib.rs`,
`limits.rs`, `locks.rs`, `main.rs`, `nettest.rs`, `secrets.rs`, `seed.rs`,
`uploads.rs`) stehen gesammelt in §16.

**Mit Phase W entfallen** (ADR 0039): Tauri samt `Paths::from_handle` und den
`%APPDATA%`-Pfaden (W5), `mail::keyring` mit dem OS-Keychain (W8, E6),
`notify::os_native` mit den OS-Benachrichtigungen (W5), der Factory Reset
(`domain::factory_reset`, `backup::factory_reset`, `commands::factory_reset`;
E7) und der Drop-Folder (`domain::drop_folder`, `scheduler::drop_folder`; E11),
beide entfernt in W11. Der Microsoft-Graph-Versand (`mail::oauth_ms`) ist im
Server gesperrt, der Code ist noch vorhanden (E9).

---

## 1. `domain/` — Functional Core (pur)

`domain/` enthält die fachliche Wahrheit ohne I/O: Validierungen, Berechnungen,
Transformationen. Jede Funktion ist als `pub fn` testbar ohne DB/Sidecar/
Filesystem; alle Effekte fließen über Argumente rein und Rückgabewerte raus.

| Modul | Verantwortung | Wichtige Funktionen / Typen |
|---|---|---|
| `domain::invoice` | Rechnungs-Validierung (§14/§19/§33 UStG, §14c-Schutz) und Totals-Berechnung in Cent | `validate_for_issue`, `compute_totals`, `is_kleinbetragsrechnung` |
| `domain::contact` | Kontakt-Validierung | `validate`, `validate_vat_id` |
| `domain::kleinunternehmer` | §19-Hinweistext (wortgleich), 5-Jahres-Bindung nach Verzicht, USt-Sperre je Position | `HINWEIS_TEXT`, `KleinunternehmerStatus`, `waiver_deadline`, `assert_no_vat` |
| `domain::kleinbetragsrechnung` | §33 UStDV — vereinfachte Pflichtangaben bei ≤ 250 € Brutto | `KLEINBETRAG_THRESHOLD_CENTS`, `is_applicable` |
| `domain::storno` | Storno-Beleg aus dem Original ableiten (Einzelpreise negiert, Storno-Präfix mit Original-Nummer); Geschäftsjahr aus dem Storno-Datum, Bezug aufs Original (Nummer, Datum) für XML und PDF (Lauf E) | `build_storno_input`, `storno_fiscal_year`, `PrecedingInvoice` |
| `domain::numbering` | Doc-Number-Format `{TYP}-{YYYY}-{NNNN}` (ADR 0012) | `DocType`, `format`, `format_invoice`, `format_storno`, `format_quote`, `format_expense` |
| `domain::quote` | Angebots-Validierung und -Totals, Konvertierung zu Rechnung | `validate_quote`, `compute_totals`, `convert_to_invoice` |
| `domain::expense` | Kosten-Validierung, §13b nur als Hinweis-Flag, Brutto in die EÜR | `validate_expense`, `compute_gross` |
| `domain::private_movement` | Privatentnahme/-einlage als EÜR-neutrale Bewegung | `validate_private_movement` |
| `domain::fiscal_year` | Geschäftsjahres-Grenzen (GJ = Kalenderjahr) | `fiscal_year_for`, `fiscal_year_bounds`, `soft_lock_deadline` |
| `domain::asset` | Anlagen-Validierung (Anschaffungsdatum, Nutzungsdauer, GWG-Grenze), Methoden-Vorschlag | `validate_asset`, `suggest_method` |
| `domain::depreciation` | AfA-Berechnung (linear/GWG/Computer/Privatanteil), idempotent | `compute_yearly`, `compute_disposal_year_partial`, `annual_linear_cents` |
| `domain::package` | Revisions-Validierung, Markdown-Subset-Parser, AST→Typst (Injection-Schutz), Plaintext-Flatten für BT-154 | `validate_revision`, `parse_markup`, `to_typst`, `to_plaintext` |
| `domain::travel` | Anfahrt km × Satz → Beleg-Position (kein Geocoding) | `compute_travel`, `TravelLine` |
| `domain::recurring` | Recurring-Kosten-Vorlagen-Validierung, Fälligkeitsraster | `validate_recurring`, `compute_next_due_date`, `is_due` |
| `domain::recurring_invoice` | Abo-Rechnungs-Vorlagen-Validierung + `AutoMode` (draft/issue/issue_send) | `validate_recurring_invoice`, `AutoMode` |
| `domain::dsgvo` | Art.-15-Report-Aufbau (read-only, interne Notizen ausgelassen) | `assemble`, `DataSubjectReport` |
| `domain::anonymize` | Art.-17-Regel: Platzhalter-Name `Anonymisiert #<hex>`, Sperre bei offenen Entwürfen, Namensprüfung für Buchungsvorlagen von Kosten-Abos (F3b; das Leeren der PII macht `db::repo::contacts`) | `anonymized_name`, `short_id`, `anonymization_blocker`, `mentions_name` |

**Invarianten.** Kein `tokio::fs`, kein `sqlx`, kein `tokio::process` in
`domain/`. Wer einen Treffer findet, hat einen Bug. **Tests** liegen je Modul
inline in `#[cfg(test)] mod tests`; Coverage-Anspruch ist „jede Hardline-
Regel mit eigenem Negativtest".

ADRs: 0004 (FC/IS), 0005 (§19), 0007 (CII), 0012 (Doc-Number), 0016/0017
(Quote-Lifecycle), 0019 (Expense), 0020 (Private Movements), 0022 (Zufluss/
Abfluss), 0025 (AfA), 0031 (Packages/Travel), 0032 (DSGVO), 0033 (Abo-RI).

---

## 2. `einvoice/` — XRechnung + ZUGFeRD

Zweigeteilt: **Generator** (Core, pur) baut CII-XML aus Domain-Daten,
**Validator + Mustang-Bridge** (Shell) sprechen mit dem Java-Sidecar.

| Datei | Schicht | Verantwortung |
|---|---|---|
| `einvoice::types` | Core | Datentypen für die Pipeline (`ValidationReport`, `ValidationFinding`, `ValidationStatus`) |
| `einvoice::generator` | Core | `to_xrechnung`: UN/CEFACT-CII (ADR 0007), §19-Klausel als BT-22-Note, BT-120-Tax-Category `E` |
| `einvoice::parser` | Core | CII + UBL (Empfang, ADR 0024), ZUGFeRD-PDF-Extract (XML aus PDF/A-3-Embedded-File), ZUGFeRD-Profile-Whitelist `check_profile_whitelist` (akzeptiert `en16931`/`extended`/`xrechnung`, lehnt `minimum`/`basic-wl` mit `ParseError::UnsupportedProfile` ab — ADR 0037 D-73) |
| `einvoice::validator` | Shell | `validate(xml, sidecar_dir)`: KoSIT-Sidecar-Aufruf, Rückgabe als strukturierter `ValidationReport` |
| `einvoice::mustang_bridge` | Shell | `create_zugferd` (PDF + XML → PDF/A-3 mit eingebettetem XML), `extract_xml` (Empfang, Roh-XML-Viewer) |

**Pipeline** (Festschreiben, `../ARCHITECTURE.md` §4.1):
`generator::to_xrechnung` → `validator::validate` (KoSIT) →
`klausel_check::verify_for_kleinunternehmer` → `typst_render::render_invoice`
→ `mustang_bridge::create_zugferd` → `archive::store::store_bytes`.

**Sidecar-Aufruf.** Beide Shell-Module starten `klein-buch-java.sh` über
`/bin/sh` mit Temp-Dateien unter `TMPDIR` (`/data/tmp`), Timeout und
`kill_on_drop`; höchstens zwei JVM-Läufe gleichzeitig (`limits::JVM`).

**Mock-Mode.** `KLEIN_BUCH_SIDECAR_MOCK=1` schaltet beide Shell-Module auf
Stubs, aber nur in Debug-/Test-Builds (`einvoice::sidecar_mock_active`); ein
Release-Build mit gesetztem Schalter startet nicht (`server::mod`).

**Tests.** Generator hat einen vollständigen Golden-XML-Test (CII-Roundtrip).
Parser hat Fixtures aus echten XRechnungen + ZUGFeRD-PDFs. Die
Pipeline-Tests setzen den Mock selbst; `tests/sidecar_real_test.rs` läuft lokal
gegen eine jlink-JRE, der Smoke-Test des Images gegen den echten Sidecar.

ADRs: 0001 (Sidecar, Amendment: nur Linux), 0007, 0008, 0024, 0037
(Profile-Whitelist beim Empfang).

---

## 3. `pdf/` — Typst-Rendering und §19-Klausel-Check

| Datei | Schicht | Verantwortung |
|---|---|---|
| `pdf::klausel_check` | Core | Strenger §19-Marker-Check vor jedem Render (Rechnung + Angebot): `verify_for_kleinunternehmer`, `inspect`. Schlägt fehl, wenn ein Template ohne `// §19-KLAUSEL-BLOCK: REQUIRED`-Marker für eine §19-Rechnung verwendet werden soll. |
| `pdf::bundle` | Core | `merge_pdfs` (lopdf) für das Angebots-Druck-Bundle (Angebot + AGB + Datenschutz → ein Druck-PDF, ADR 0018). |
| `pdf::templates` | Core+Shell | Built-in-Vorlagen + doctype-bewusster Resolver (`resolve_invoice_template`, `resolve_quote_template`) + `list_templates`. Auflösung: `/inputs/pdf-templates/{name}.typ`-Override → Built-in; Sentinel `'default'` → `seller.default_pdf_template` (ADR 0030). |
| `pdf::typst_render` | Shell | Typst-`World`-Impl: `render_invoice` (PDF/A-3b-fähig), `render_quote` (Plain-PDF), `render_euer` (Anlage EÜR), `render_package_catalog` (Paket-Broschüre); Logo aus `/data/branding` via `World::file()`. Läuft außerhalb der Runtime (`compile_async` über `limits::RENDER`). |

**Wichtig.** `pdf/` macht **nicht** ZUGFeRD-Embedding. Das ist
`einvoice::mustang_bridge`. `typst_render` liefert ein PDF/A-3b-Basis-Dokument,
das `mustang_bridge` mit dem XML als Anhang versiegelt.

**Klausel-Check ist obligatorisch.** Vor jedem `render_invoice` läuft der
Klausel-Check. Schlägt er fehl, wird das Render abgebrochen (kein still-broken
Beleg). Das fängt versehentlich gelöschte Klausel-Blöcke in editierten
Templates ab (siehe §19-Logik in `paragraph-19.md`).

**Tests.** Klausel-Check hat einen Test pro Template-Variante; Render-Tests
arbeiten gegen Snapshot-PDFs. Bundle-Test verifiziert dass die Anhänge in
Reihenfolge erscheinen.

ADRs: 0008, 0018, 0030.

---

## 4. `archive/` — write-once-Beleg-Archiv

| Datei | Schicht | Verantwortung |
|---|---|---|
| `archive::store` | Shell | `store_bytes`: SHA-256 berechnen, in eine Temp-Datei schreiben (`fsync`), zurücklesen und gegen den Hash prüfen (Lauf F, KB-0040), schreibschützen (`0o400`), ohne Überschreiben (`hard_link`, sonst `renameat2` mit `RENAME_NOREPLACE`) an den Zielpfad `<archive_dir>/<fiscal_year>/<kind>/<file_name>` bringen (im Container unter `/data/archive`), Eintrag in `archive_entries` (Pfad `UNIQUE`), Audit `archive.store`. `read_and_verify` (Re-Hash, Audit `archive.read`) und `read_and_verify_silent` (Re-Hash ohne Audit, D-72). |
| `archive::audit` | Shell | Typisierte Audit-Events: `archive.store`, `archive.read`, `archive.integrity_pass`, `archive.integrity_fail`, `archive.integrity_missing`, `archive.integrity_unreadable`. |
| `archive::integrity_check` | Shell | `verify_one`, `run_full_scan`: Re-Hash aller Archiv-Dateien; manipuliert, fehlt und nicht lesbar getrennt (G1-HARDEN.4), Aggregat in `archive_integrity_checks`. |

**Hardline (ADR 0006).** Eine `ArchiveEntry` ist nach dem Schreiben
**unveränderlich** (`trg_archive_no_update`). Eine Datei am Zielpfad ohne
`archive_entries`-Eintrag ist der Rest eines abgebrochenen Laufs und wird
ersetzt; mit Eintrag ist ein anderer Inhalt ein harter Fehler. Tamper wird
sofort gemeldet, nicht ignoriert. Re-Hash läuft monatlich
(`scheduler::integrity_check_cron`) und manuell
(`commands::system::archive_integrity_run`). Unter Linux heißt write-once
Erkennung statt Verhinderung (`security.md`, „Write-once unter Linux"); der
Server prüft `0o400` beim Serverstart per Selbsttest (`server::startup`).

**Tests.** Roundtrip (store + read + hash-match), Tamper-Detection (Datei
modifizieren → next read schlägt fehl), Waisen-Detection (Datei löschen →
`integrity_missing`-Event, kein Tamper-Fehlalarm), idempotenter Re-Store.

ADRs: 0006.

---

## 5. `db/` — Persistence-Layer

| Datei / Verzeichnis | Schicht | Verantwortung |
|---|---|---|
| `db::mod` | Shell | `open_pool(db_file, key)` (SQLCipher-Pool, `PRAGMA key`), `open_and_init`, `begin_immediate` (Schreib-Transaktion, W2), `prepare_filesystem` (vor jedem Pool-Open: Seed von `/inputs`, vorgemerkter Factory Reset → Start-Abbruch (E7), Restore-Staging aufräumen, vorgemerkten Restore anwenden), `run_migrations` (`sqlx::migrate!` aus `migrations/`, Schema-Check, Restore-Audit), `rekey_db_file` (Passphrase-Wechsel, W6), `db_file_is_plaintext` |
| `db::models` | Shell | sqlx-Row-Typen (DB-nahe), nicht zu verwechseln mit Domain-Typen — Mapping in den Repos |
| `db::numbering` | Shell | Atomare Nummern-Allokation aus `doc_number_counters` je (Typ, GJ): `next_number`, `next_seq`, `peek_seq`; `snapshot` und `raise_in` (nur aufwärts) für Restore und `numbering_raise` (Lauf F, D-24) |
| `db::schema_version` | Shell | `EXPECTED_SCHEMA_VERSION` (32), `MIN_RESTORE_SCHEMA_VERSION` (30), `read`, `check_compatible`; geprüft beim Entsperren — neuere DB ⇒ kein Entsperren (Down-Migration-Schutz) |
| `db::triggers` | Shell-Stub | Dokumentiert die in den Migrations-SQLs definierten Trigger (Code-Stub, die Wahrheit lebt in `migrations/*.sql`) |
| `db::repo::*` | Shell | Repo pro Domäne (21 Dateien), CRUD + State-Transitions; alle Inserts/Updates über parametrisierte Queries; Locked-Belege werden über DB-Trigger geschützt |

**Repos:** `invoices`, `quotes`, `expenses`, `private_movements`,
`payment_accounts`, `assets`, `depreciation`, `recurring`, `recurring_invoice`,
`packages`, `legal_documents`, `attachments`, `contacts`, `seller_profile`,
`audit_log`, `email_log`, `backup_log`, `mail_accounts`, `euer`, `dsgvo`,
`app_settings`. Zugangsdaten liegen nicht in einem Repo, sondern in
`crate::secrets` (§16). `audit_log::append`/`append_in` nehmen die Details nur
als `serde_json::Value` und setzen den Akteur aus dem Task-Kontext
(`with_actor`: `owner`/`scheduler`, sonst `system`; Lauf F, ADR 0039 Nr. 125).

**Locked-Belege-Pattern.** Repos haben getrennte Funktionen für „Draft-Update"
(erlaubt Kernfeld-Änderungen) und „State-Transition" (erlaubt nur die in
Trigger-Whitelists genannten Felder zu setzen). Versuche, gelockte Kernfelder
zu ändern, scheitern am Trigger, nicht erst an einem Repo-Check.

**Tests.** Repos haben Integrations-Tests gegen eine echte SQLCipher-DB
(In-Memory geht für STRICT+Trigger nicht ausreichend), siehe `tests/*_test.rs`.
Schema-Version-Check hat eigenen Test (Mismatch → Bootstrap-Fehler).

ADRs: 0003, 0006, 0035 (SQLCipher), 0039 (Pflicht-Backup vor Migration, E15).

---

## 6. `scheduler/` — 5-Minuten-Tick

| Datei | Schicht | Verantwortung |
|---|---|---|
| `scheduler` (`mod.rs`) | Shell | `JobCtx`: Pool, Pfade, Sitzung, Notifier, Uhr (`clock::Clock`), kooperative Abbruch-Anforderung, im Tick gesammelte Lock-Backups (`lock_backup`) und Markierung „erster Tick nach dem Entsperren“ (ADR 0040) |
| `scheduler::tick` | Shell | Tokio-Interval (300 s), `MissedTickBehavior::Skip`, `ensure_started` (genau ein Start pro Prozess, nach dem ersten Entsperren), `request_stop`/`stop`/`stop_within`/`resume` über `AppCtx::scheduler_stop` und `AppCtx::scheduler_gate`. Je Tick (isoliert, Reihenfolge `../ARCHITECTURE.md` §5): Kosten-Abos, Abo-Rechnungen, Auto-AfA, Backup (Tages-Backup kalenderfest bzw. genau ein Lock-Backup, D-19), Integritäts-Cron (nicht im ersten Tick nach dem Entsperren), Hinweise, danach `update_check`. Erster Tick frühestens `UNLOCK_SETTLE` (60 s) nach dem Entsperren. |
| `scheduler::hold` | Shell | Pause der automatischen Belegerzeugung (`app_settings.scheduler_hold`), Uhr-Plausibilität (`scheduler_last_seen_at`), Freigabe mit Audit |
| `scheduler::catch_up` | Shell | Nachhol-Regeln (bis 3 selbst, bestätigt bis 24), Bestätigungs-Marker, `ResumeMode` |
| `scheduler::backoff` | Shell | Persistierter Backoff nach Fehlschlägen (`app_settings`, UTC-Zeitpunkt) |
| `scheduler::recurring` | Shell | Fällige Kosten-Abos buchen (`process_due`, `run_now`, `apply_resume_mode`): Sperre je Abo, Beleg + Audit + Vorrücken (CAS) in einer Transaktion, Fehler je Abo |
| `scheduler::depreciation_year_close` | Shell | Auto-AfA zur GJ-Wende, default-an per `app_settings.depreciation_auto_year_close` (Commands `fiscal_year_auto_close_get`/`_set`); Pause, Hinweis und 6 h Backoff bei Fehler |
| `scheduler::integrity_check_cron` | Shell | Monatlicher Archiv-Hash-Verify (Monat in Europe/Berlin), gemeinsame Sperre `integrity_check::SCAN_LOCK` mit „Jetzt prüfen“, 6 h Backoff nach Fehlschlag (`integrity_last_failure_at`), Abbruch je Datei; Hinweis über `rule_archive_integrity_failed` |
| `scheduler::reminders` | Shell | Regel-getriebene Hinweise: Monats-Doku-Check, GJ-Abschluss fällig, `backup_overdue` (Off-Site-bewusst: mit Off-Site-Ziel zählt das letzte erfolgreiche Off-Site-Backup aus `backup_log`), überfällige Rechnungen, „Abo fällig“ (Kosten-Abos im Modus Erinnerung) |
| `scheduler::recurring_invoice` | Shell | Fällige ausgangsseitige Abo-Rechnungen über die normale draft→lock-Pipeline materialisieren (`process_due`, `run_now`, `resume_pending`, `apply_resume_mode`), Belegdatum = heute, Sperre je Vorlage und Termin, Entwurf + Vorrücken in einer Transaktion, Fortsetzen nach Abbruch |
| `scheduler::update_check` | Shell | Opt-in Versionsprüfung (`app_settings.update_check_enabled`, Standard aus): höchstens alle 24 h ein GET auf die GitHub-Releases, ohne Weiterleitungen und ohne Instanzdaten; neuere Version → Inbox-Eintrag nur als Text (E14) |

**Gating.** Der Scheduler startet erst nach dem ersten erfolgreichen
Entsperren; ohne Pool (gesperrt) tut ein Tick nichts. Restore, Herunterfahren
und „Datenbank sperren" pausieren ihn über das Gate; vorher bitten sie laufende
Abo-Läufe, nach dem aktuellen Termin aufzuhören („Datenbank sperren“ wartet
höchstens 10 s). Nach einem Neustart läuft nichts, bis jemand entsperrt (E3);
fällige Jobs holt der erste Tick nach (Abo-Läufe mit Grenze). Details:
`../ARCHITECTURE.md` §3 + §5, ADR 0040.

Der Drop-Folder-Job (`scheduler::drop_folder`, ADR 0037) ist im Server
entfallen (E11, Code entfernt in W11); Eingangsrechnungen kommen per Upload
(`../ARCHITECTURE.md` §4.18).

**Tests.** Jeder Job hat Tests mit fester Uhr (`clock::FixedClock`) +
Fixture-DB; `tests/recurring_scheduler_test.rs` prüft parallele Auslöser,
Abbruch, Nachhol-Grenze, Uhrsprung, Zeitumstellung und die Job-Isolation im
Tick. Tick-Loop hat einen `scheduler_starts_only_once`-Test (Einmal-Guard).

ADRs: 0023, 0027, 0033, 0039 (Tages-Backup im Tick, Versionsprüfung E14), 0040
(Dauerbetrieb).

---

## 7. `backup/` — Verschlüsselte Backups und Restore

| Datei | Schicht | Verantwortung |
|---|---|---|
| `backup::mod` | Shell | `BackupSession` (Passphrase der laufenden Sitzung, nur im RAM), `setup_passphrase`, `store_verifier`, `unlock`, `verify_passphrase`, `create_now`/`create_with` (prozessweit serialisiert: Snapshot → Hülle → Floor → Off-Site-Ziele → Rotation; Zeit aus der injizierten Uhr; während der Pause nach einem Restore nur Floor, während jeder Pause keine Rotation), `request_auto_backup` (Lock-Events: Vermerk mit Zähler, Audit `backup.requested`, weckt den Scheduler; Lauf F3a), `requested_backup_now` (geweckter Lauf), `auto_backup_at` (sofort: Tick, Nachholen), `auto_daily_if_due` (Tick, kalenderfest ab `backup_daily_time`, zuletzt fälliger Slot, Backoff persistiert), `mark_pending`/`pending_backup_if_due` (ausgefallenes Lock-Backup nachholen), `rescan_floor` (Historie nach Restore), `verify_backup_file`, Hinweis `rule_backup_result` (Inbox-only) |
| `backup::after_restore` | Shell | Abgleich im ersten Entsperren nach einem Restore (Lauf F, F1d): Abo-Termine nur vorwärts, Prüfung für Reihen ohne bekannten Stand, Marker aus dem Backup verwerfen, Entwürfe mit vor dem Restore festgeschriebener Nummer vormerken (`ensure_no_conflict` in der Lock-Pipeline und bei `quotes_issue`, `confirm_conflict`) |
| `backup::snapshot` | Shell | Content-ZIP: DB-Snapshot per `VACUUM INTO` über eine eigene Verbindung (bleibt SQLCipher-verschlüsselt) + `archive/` + `branding/`; ohne versteckte `.tmp`/`.part`-Dateien und Symlinks, verschwundene Dateien werden übersprungen (Lauf F3a) |
| `backup::manifest` | Shell | Klartext-Kopf der `.kbk` (Magic + einzeiliges Manifest-JSON: Format, Schema- und App-Version, Zeitpunkt, KDF-Parameter, Salt, Nonce, SHA-256 und Größe des Inhalts), damit der Restore die Angaben ohne Passphrase zeigen kann; ab Format 3 als AAD an AES-GCM gebunden (`header_bytes`, `parse_header`, Lauf F3a) |
| `backup::encrypt` | Shell | Argon2id-Schlüsselableitung + AES-256-GCM für die Backup-Hülle; **getrennt** von der SQLCipher-Kette (die ist im DB-Header) |
| `backup::target` | Shell | Off-Site-Ziele als JSON-Liste in `app_settings.backup_targets` (höchstens 10): Ordner unter `/backup-targets` (relativ gespeichert, beim Schreiben per `canonicalize` gegen die Wurzel geprüft) oder SFTP; `write_backup`/`write_backup_shared` (ein Puffer für alle Ziele), `offsite_status` (missing/failing/stale/ok). Keine Cloud-Ordner-Erkennung mehr (Amendment ADR 0034). |
| `backup::sftp` | Shell | `russh`/`russh-sftp`, Host-Key-Pinning (SHA-256, TOFU über `probe`), Passwort je Ziel in `secrets` (`sftp:<ziel-id>`), nur Passwort-Auth, Upload unter `.<name>.part` + `rename` |
| `backup::rotation` | Shell | Kalender-Eimer (jüngstes Backup je Berliner Tag/Monat/Jahr); Floor 7/3/1 bei erfolgreichem Off-Site-Ziel, sonst 30/12/7; Ordner-Ziele je Ziel 30/12/7; SFTP nie; `manual`/`pre_restore` nie rotiert (Tag nur informativ). Planung pur (`plan_tiered_deletions`), Löschen in `run` nur für reguläre `.kbk` unter Floor bzw. `/backup-targets` (`resolve_local_backup`, auch für Restore und Download, Lauf F3a). |
| `backup::restore` | Shell | Phase A (`apply`: Schema `30 ≤ s ≤ EXPECTED`, Entschlüsseln in place (`decrypt_backup_owned`, Format 1–3), gehärtet ins Staging entpacken und gestagte DB prüfen, danach Pre-Restore-Backup und zweite Platzprüfung, Nummernkreise, Abo-Termine und festgeschriebene Nummern festhalten, Marker) und Phase B (`apply_pending` beim nächsten Start in `db::prepare_filesystem`: Tausch von DB samt WAL, `archive/` und `branding/` mit Zustandsdatei und Rückabwicklung, Angaben in `RESTORE_APPLIED.json`) |

**Wichtig.** Phase A ändert die Live-DB nie; nach Erfolg fährt der Server
geordnet herunter (Exit 0), die Container-Verwaltung startet ihn neu.
Destruktive FS-Ops laufen in Phase B beim nächsten Start, vor jedem Pool-Open.
Das Audit `backup.restore.applied` kommt über `db::PendingRestoreAudit` bzw.
`RESTORE_APPLIED.json` beim ersten Entsperren in die wiederhergestellte DB.
Einzelheiten: `security.md` §6.

**Tests.** Restore-Roundtrip mit 1000 Zeilen (G1-HARDEN.1); Rotation-
Invarianten (G1-HARDEN.5); Pre-Restore-Backup wird erzwungen; SFTP-Arm hat
einen Mock-Server-Test.

ADRs: 0009, 0034, 0035, 0039 (mehrere Off-Site-Ziele E8, Restore älterer
Backups E15).

---

## 8. `mail/` — Versand (SMTP; Microsoft Graph im Server gesperrt)

| Datei | Schicht | Verantwortung |
|---|---|---|
| `mail::smtp` | Shell | `lettre`-Async-Sender (`send`, `test_connection`, `build_message`), TLS/STARTTLS, Multi-Attachment, neutrales Result-Mapping |
| `mail::oauth_ms` | Core+Shell | PKCE-S256, Autorisierungs-URL, Token-Tausch und -Refresh, Graph-Versand (ADR 0028). **Im Server gesperrt:** `mail_oauth_*` haben in der Registry `Route::Blocked`, neue oder geänderte OAuth-Konten werden abgelehnt (E9); der Code ist noch vorhanden. |
| `mail::templates` | Shell | Tera-Render für Betreff und Text aus `/inputs/mail-templates/{name}.txt` (erste Zeile `Subject: …`); Kontext je Beleg-Typ |

**Zugangsdaten.** SMTP-Passwörter liegen seit W8 in der Tabelle `secrets`
(`crate::secrets`, Schlüssel `smtp:<konto-id>`), gebunden an (Host, Port,
Benutzer): Ändert sich eines davon, verfällt das gespeicherte Passwort
(Amendment ADR 0011).

**Dispatch.** `commands::mail::dispatch_send` wählt den Kanal anhand
`auth_type` (SMTP; Graph nur für ein Konto mit gespeichertem OAuth-Token). Beide
Wege landen am Ende in `db::repo::email_log` mit der Provider-Antwort.
Audit-Log enthält das Versand-Event **ohne** Passwort oder Token.

**Tests.** OAuth hat End-to-End-Test mit Mock-Authority. SMTP hat Mock-Server-
Test (offline).

ADRs: 0011 (+ Amendment 2026-10-23), 0028 (+ Amendment), 0039 (E6, E9).

---

## 9. `migration_export/` — Steuerberater-/Migrations-Export

| Datei | Schicht | Verantwortung |
|---|---|---|
| `migration_export::export` | Shell | `export_all`: offenes ZIP mit allen Tabellen, Archiv-Dateien, Migrationen, Manifest, README und Standalone-Reader (`read_export.py`); `commands::migration_export` legt es in der Download-Ablage an (`<data>/tmp/downloads/<uuid>/klein-buch-export-<Datum>.zip`) und gibt ein Download-Ticket zurück (E10) |
| `migration_export::json_dump` | Shell | JSON-Dump aller Tabellen (kanonische Struktur, nicht migrations-versioned) für maschinelle Weiterverarbeitung; `EXCLUDED_TABLES` = `secrets` |

**Inhalt des ZIPs.** `tables/<table>.json` je Tabelle (außer `secrets`),
`archive/` mit allen Original-Belegen, `schema/` (Migrationen + `erd.md`),
`manifest.json` (App-/Schema-Version, Counts, Hashes), `README.md`,
`read_export.py`. **Keine** Passphrase, **keine** Zugangsdaten.

**Tests.** Roundtrip: aus dem ZIP lassen sich Beleg-PDFs öffnen und mit dem
gespeicherten Hash verifizieren. Anonymisierte Kontakte erscheinen anonymisiert.

ADRs: 0026 (im Kontext EÜR-Export), 0039 (E10, Downloads statt Server-Pfade).

---

## 10. `notify/` — In-App-Inbox

| Datei | Schicht | Verantwortung |
|---|---|---|
| `notify::store` | Shell | CRUD auf `notifications`: `create` (mit `dedup_key` idempotent, `None` bei Treffer), `list`, `count_unread`, `dismiss`, `dismiss_all` |
| `notify::rules` | Shell | Schaltbare Regeln aus `notification_rules` (per Migration geseedet, u. a. `rule_backup_overdue`, `rule_backup_result`, `rule_invoice_overdue`, `rule_archive_integrity_failed`, `rule_fiscal_year_lock_pending`, `rule_monthly_doc_check`): `list`, `get`, `get_enabled_by_type`, `set_enabled` |
| `notify::mod` | Shell | `emit` (schreibt in die Inbox, idempotent über `dedup_key`); einen Kanal außerhalb der Inbox gibt es nicht (Trait `Notifier`/`InboxOnly` mit Lauf F entfernt, D-16) |

Ausgewertet werden die Regeln in `scheduler::reminders`,
`scheduler::integrity_check_cron` und `backup::mod` (`rule_backup_result`).
OS-Benachrichtigungen gibt es im Server nicht (W5, ADR 0027 Amendment
2026-10-24); die Spalte `deliver_os_native` und die Regeln des Drop-Folders aus
Migration 0029 bleiben in der DB stehen (forward-only), wirken aber nicht.

**Disziplin.** Manuelle Backups → Erfolgs-Hinweis ist OK (der Nutzer hat
explizit ausgelöst). Auto-Backups (Lock-Events, Tages-Backup) → nur
Fehler-Hinweis, sonst zu viel Lärm. `backup_overdue` zählt mit Off-Site-Ziel
die **Off-Site**-Backups, nicht den Floor; ohne Off-Site-Ziel warnt es, wenn gar
keine Sicherung mehr lief. Die Schwelle steht in der Regel-Konfiguration
(`max_age_days`, Standard 7); die Regel selbst ist abschaltbar.

**Tests.** Dedup-Key verhindert Doppel-Hinweise; Rule-Trigger gegen
Fixture-State.

ADRs: 0027, 0034, 0039.

---

## 11. `fiscal_year/` — Geschäftsjahres-Abschluss

| Datei | Schicht | Verantwortung |
|---|---|---|
| `fiscal_year::lock` | Shell | `close_year(year)` — nur abgelaufene Jahre, nur entsperrt, exklusiv gegen Buchungen (`guard::closing`): AfA buchen; in **einer** Transaktion Anlagen + Abschreibung sperren, EÜR-Snapshot ins `fiscal_year_locks` (append-only), Audit mit Belegzahlen und offenen Entwürfen; danach Auto-Backup; `open_drafts`, `get`, `list_closed` (Lauf F) |
| `fiscal_year::snapshot` | Core + Shell | Abgleich abgeschlossener Jahre mit dem Festschreibungs-Snapshot: `compare` (pure), `check`, `report_mismatch` (Audit `euer.snapshot_mismatch`, Hinweis je Jahr), `readme_block` (Lauf F, KB-0031) |
| `fiscal_year::transition` | Shell | GJ-Übergang: `open_receivables` (offene Forderungen, die der Abschluss-Dialog anzeigt); Nummernkreise starten lazy je GJ |
| `fiscal_year::guard` | Shell | `ensure_year_open(year)`-Guard in den Command-Kernen, `is_closed`. Liefert eine `BookingPermit`, die bis nach dem Schreiben gehalten wird; `closing()` ist die exklusive Gegenseite des Abschlusses (Lauf F, ADR 0040 Nr. 48). Ein Storno zu einer Rechnung aus einem abgeschlossenen Jahr bleibt möglich; er entsteht im (offenen) Jahr seines Storno-Datums (ADR 0039 Nr. 122). |

**Hardline (ADR 0027).** `close_year` ist **unumkehrbar**. Versuche, danach
Belege im geschlossenen Jahr zu ändern, scheitern am Guard auf Command-Ebene
und am Trigger auf DB-Ebene.

**Tests.** `close_year` lehnt nicht-abgelaufene Jahre ab; setzt `fiscal_year_locks`
und blockiert Folge-Edits; Storno bleibt zugelassen. `tests/fiscal_year_guard_test.rs`
deckt alle buchenden Commands nach dem Abschluss und die Sperre gegen parallele
Buchungen ab, `tests/audit_export_test.rs` den Snapshot-Abgleich.

ADRs: 0027.

---

## 12. `assets/` und `depreciation/` — Anlagenverzeichnis + AfA

| Datei | Schicht | Verantwortung |
|---|---|---|
| `assets::afa_tabellen` | Shell | `load(inputs_dir)`/`parse`: AfA-Tabellen aus `/inputs/specs/afa-tabellen.json` (vom Menschen gepflegt, beim Serverstart aus dem Image geseedet, nie überschrieben; ohne Cache, jede Verwendung liest die Datei) |
| `depreciation::compute` | Core | Re-Export aus `domain::depreciation` (`compute_yearly`, `annual_linear_cents`, …) unter dem historischen Pfad |
| `depreciation::accrue_yearly` | Shell | `accrue_for_year`: bucht die AfA bis einschließlich GJ für alle aktiven Anlagen als `depreciation_entries` (holt fehlende Jahre nach), Audit; `reset_asset` setzt ungelockte AfA einer Anlage revisionssicher zurück; `accrue_asset_through_in` holt die Vorjahre beim Abgang in dessen Transaktion nach (F3b) |

**Festschreibung.** AfA-Zeilen sind erst nach GJ-Abschluss unveränderlich
(`trg_depreciation_immutable` greift mit dem Abschluss von `assets`).

**Tests.** AfA-Compute-Tests sind das Kron-Beispiel für FC: pure Funktionen,
parametrisierte Tests pro Methode + Edge-Case (GWG-Grenze, Computer-1/3-Übergang,
Privatanteil-Mischfall, unterjähriger Kauf).

ADRs: 0025.

---

## 13. `euer/` — EÜR-Aggregation und Export

| Datei | Schicht | Verantwortung |
|---|---|---|
| `euer::aggregate` | Core | `aggregate(fiscal_year, inputs)`: EÜR-Aggregation Cash-Basis §11 EStG (ADR 0022); Storno als negative Einnahme zum Storno-Datum (kein paid_amount-Netting möglich). Die Eingaben lädt `db::repo::euer::load_inputs`. |
| `euer::elster_csv` | Core | `build_form`, `to_csv`: Zeilen-Mapping für die ELSTER-Ausfüllhilfe |
| `euer::datev_csv` | Core | `build_bookings`, `to_datev`: DATEV-EXTF-Buchungsstapel (SKR03-Default, SKR04 wählbar), CP1252 |
| `euer::detail` | Core | Einzelaufstellungen als CSV (`income_csv`, `expenses_csv`, `aveeur_csv`) |
| `euer::stb_package` | — | Nur Doc-Stub. Das Steuerberater-ZIP (Anlage-EÜR-PDF + DATEV-CSV + Einzelaufstellungen + Belege) baut `commands::euer::euer_export_stb_zip` in der Download-Ablage. |

**Cash-Basis.** Einnahmen am `paid_at`, Ausgaben am `paid_date`, Privatbewegungen
EÜR-neutral. Teilzahlung pro Zahlungsjahr separat.

**Tests.** Aggregate-Tests gegen Fixture-Datasets (klein, mit Storno, mit
Teilzahlung über Jahresgrenze, mit GJ-Lock).

ADRs: 0010, 0022, 0026.

---

## 14. `commands/` — Command-Kerne + Registry

25 Dateien: ein Modul pro Domäne (`assets`, `attachments`, `backup`,
`contacts`, `depreciation`, `dsgvo`, `euer`, `expenses`, `fiscal_year`,
`invoices`, `legal_documents`, `mail`, `migration_export`, `notifications`,
`packages`, `payment_accounts`, `pdf`, `private_movements`, `quotes`,
`recurring`, `recurring_invoice`, `settings`, `system`), dazu `registry` und
`mod.rs`. Bis W4 waren das Tauri-Commands; seit W4/W5 sind es Kerne, die der
HTTP-Dispatcher (`server::dispatch`) aufruft (ADR 0039 Nr. 19–21).

**Registry** (`commands::registry`). Jeder Eintrag nennt Name, Weg (`Route`:
`Cmd` = über `POST /api/cmd/{name}` erreichbar; `Replaced` = eigene Route für
Auth, Upload oder Download; `Dropped` = Desktop-Funktion, entfällt; `Blocked` =
im Server gesperrt, z. B. `mail_oauth_*`), `Auth` (`Public`/`Session`/`Reauth`),
`Cost` (`Cheap`/`Render`/`Jvm`/`Kdf`/`Backup`), `Kind` (`Read`/`Write`/`Job`)
und `body_limit`. Was nicht `Route::Cmd` hat, beantwortet der Dispatcher mit 404.

**Disziplin.** Jeder Kern ist dünn:

1. Signatur `async fn name(ctx: &Arc<AppCtx>, args: NameArgs) -> Result<T>`;
   das Args-Struct übernimmt die camelCase-Schlüssel des Frontends.
2. Pool über `ctx.pool()` — gesperrt ⇒ `Error::Locked` (HTTP 423).
3. Guard-Calls (`ensure_year_open` etc.).
4. Domain-Validate → Repo-Aufruf → Result mappen.
5. Audit-Log + ggf. Auto-Backup-Trigger.

Kein Kern nimmt einen Server-Pfad an: Dateien kommen als `uploadId` aus der
Upload-Ablage, Exporte gehen als Download-Ticket zurück (E10). Keine fachliche
Logik in `commands/`. Wer in einem Kern eine Berechnung findet, sollte sie in
`domain/` verschieben.

**Tests.** Kerne haben Integrations-Tests, die die ganze Pipeline durchlaufen
(Repo-Layer + Trigger + Audit). Reine Args-Validierung wird über Domain-Tests
abgedeckt.

ADRs: 0004, 0039.

---

## 15. `server/` — HTTP-Server (Phase W)

Der Server (axum + tower-http, Verbindungen über hyper) ist der einzige
Einstieg. Überblick und Start-/Entsperr-Reihenfolge: `../ARCHITECTURE.md` §2f
und §3; Bedrohungsmodell: `security.md`.

| Datei | Verantwortung |
|---|---|
| `server::mod` | `main_entry` (Subcommands `serve`, `healthcheck`, `totp-reset`, `version`), Router, Zusammenbau vor dem Port-Bind |
| `server::config` | `ServerConfig::from_env` (`KLEIN_BUCH_LISTEN`, `…_PUBLIC_URL`, `…_TRUSTED_PROXIES`, `…_INSECURE_HTTP`, `…_ALLOW_UNMOUNTED`); Upload- und Port-Grenzen |
| `server::policy` | öffentliche URL, vertraute Proxys, Client-IP, Host-/Origin-Prüfung (421/403), Transport-Regel für Einrichten/Anmelden; kein TLS im Prozess |
| `server::startup` | fail-closed-Prüfungen vor dem Port-Bind (Sperre `/data/.klein-buch.lock`, Mounts, `db::prepare_filesystem`, Instanz-Marker, keine Klartext-DB, `0o400`-Selbsttest, `<data>/tmp` leeren, Backup-Teildateien entfernen); `sweep_tmp`/`sweep_partial_backups` für den stündlichen Sweeper (Reste älter als 6 h, D-13); Archivpfad-Stichprobe beim Entsperren |
| `server::instance` | Instanz-Marker `/data/.klein-buch-instance` |
| `server::state` | Zustand `setup`/`locked`/`unlocked`, Sitzungen, Jobs, Lastgrenzen, Setup-Token (nur stderr) |
| `server::auth` | Einrichtung, Anmeldung = Entsperren, Re-Auth, „Datenbank sperren", Passphrase- und TOTP-Wechsel |
| `server::totp`, `server::session`, `server::ratelimit` | TOTP (RFC 6238) + Einmal-Codes in `/data/auth/`; Cookie `__Host-kb_session`; Wartezeiten je Gerät/IP ohne Selbstaussperrung |
| `server::dispatch`, `server::jobs`, `server::idempotency` | `POST /api/cmd/{name}` nach Registry; Jobs `202` + `GET /api/jobs/{id}`; Idempotenz-Schlüssel |
| `server::uploads`, `server::files` | `POST /api/upload/{art}` (multipart, gestreamt); `GET /files/download/{id}`, `GET /files/archive/{id}` (hash-geprüft, auditiert) |
| `server::headers`, `server::limits`, `server::http` | CSP mit Skript-Hashes, HSTS, Cache-Control, SPA-Auslieferung; Semaphoren je Kostenklasse, globale Grenze, Timeouts; Annahme-Schleife mit Header-Timeout und geordnetem Herunterfahren |
| `server::errors`, `server::logging` | Fehlerabbildung auf HTTP (z. B. `Locked` → 423); Log ohne Geheimnisse (kein `PRAGMA key` im Log) |
| `server::health`, `server::legal` | `/healthz` (über Loopback mit Zustand und `offsite`); `/legal/` (Lizenzen, Quellen) ohne Sitzung |

**Tests.** `tests/server_*_test.rs` (u. a. `auth`, `contract`, `files`,
`jobs`, `legal`, `limits`, `restore`, `secrets`, `startup`, `targets`,
`uploads`) mit gemeinsamer Hilfe in `tests/server_support/`; dazu Unit-Tests
je Modul.

ADRs: 0039 (E1–E17), 0014 (AGPL §13, `/legal/`).

---

## 16. Wurzel-Module (`src/*.rs`)

| Datei | Verantwortung |
|---|---|
| `main.rs` | Binary `klein-buch-server`; ruft `server::main_entry` mit den Kommandozeilen-Argumenten |
| `lib.rs` | Modul-Deklarationen der Bibliothek `klein_buch_lib`; Einstieg ist `server` |
| `app_ctx.rs` | `AppCtx` — Anwendungskontext der Kerne (früher `State`/`AppHandle` der Desktop-Shell): Pfade, Pool (`None`, solange gesperrt), `BackupSession`, `PendingRestoreAudit`, Uhr (`clock::Clock`, ADR 0040), `NetTests`, `DesktopActions` (`NoDesktop`), Scheduler-Guard und -Gate, Download- und Upload-Ablage (ADR 0039 Nr. 19–23) |
| `config.rs` | `Paths` (`data_dir`, `db_file` = `klein-buch.sqlite`, `archive_dir`, `backups_dir`, `inputs_dir`, `sidecar_dir`, `resource_dir`, `web_dir`, `backup_targets_dir`); `Paths::from_env` (Pflicht: `KLEIN_BUCH_DATA_DIR`, `…_INPUTS_DIR`, `…_RESOURCE_DIR`, `…_WEB_DIR`; optional `…_BACKUP_TARGETS_DIR`); `ensure_inputs_seeded` |
| `seed.rs` | Seed von `/inputs` aus dem Image mit `.seed-manifest.json`: fehlende Dateien kopieren, unveränderte ersetzen, vom Nutzer geänderte nie anfassen (neue Fassung je Fassung einmal als `<name>.neu`, gemerkt in `offered`, Hinweis mit `dedup_key` in die Inbox; D-10) |
| `secrets.rs` | Zugangsdaten in der Tabelle `secrets` der verschlüsselten DB (Migration 0031, E6): `get`, `set`, `delete`; Schlüssel `smtp:<konto-id>`, `oauth:<konto-id>`, `sftp:<ziel-id>`; nie in Log, Audit, Migrations- oder DSGVO-Export |
| `uploads.rs`, `downloads.rs` | Upload-Ablage (`<data>/tmp/uploads/<uuid>`, Typprüfung am Inhalt, Größe je Art, Platzprüfung, höchstens 1 h) und Download-Ablage (`<data>/tmp/downloads/<uuid>/<Dateiname>`, Exporte einmal abrufbar, Vorschauen mehrfach) — E10 |
| `nettest.rs` | Verbindungstests (SMTP, SFTP): Port-Allowlist, höchstens 10 Tests je Minute, Fehler ohne Unterscheidung, SFTP-Fingerprint aus der letzten Probe (10 min) |
| `build_info.rs` | Version, Commit, Tag, Quell-URL (AGPL §13; `GET /api/public/info`, `app_info`) |
| `clock.rs` | Zentrale Uhr in Europe/Berlin (E12); kein `chrono::Local` im Produktivcode; in Tests je Thread fixierbar |
| `limits.rs` | Lastgrenzen für Typst, JVM-Starts und Passphrase-KDF außerhalb der Runtime (`run_blocking`, Semaphoren) |
| `locks.rs` | Prozessweite Sperren je Archivpfad, Beleg und Abo-Vorlage (`tokio`-Mutexe) |
| `branding.rs` | Logo und Unterschrift unter `/data/branding` (nie `inputs/`): Speichern, Finden, Löschen, Data-URL für die Vorschau |
| `error.rs` | `enum Error` (`thiserror`) + `Result<T>`; `server::errors` bildet jede Variante vor dem Serialisieren auf Status und Antwort ab |

ADRs: 0035 (Bootstrap-Inversion), 0039 (Server-Betrieb).

---

## Letzte Verifikation

Stand: 2026-10-24 (Phase W, Block W13), Schema v31, ADRs 0001–0039. Quelle:
Verzeichnis-Listing von `klein-buch/src-tauri/src/**/*.rs`, die
`mod.rs`-/Datei-Header-Kommentare und die öffentlichen Funktionsnamen der
genannten Module; `server/` abgeglichen mit `../ARCHITECTURE.md` §2f. Die
Absätze „Tests" sind nicht einzeln nachgeprüft. Bei strukturellen
Modul-Änderungen diese Datei mit aktualisieren.
