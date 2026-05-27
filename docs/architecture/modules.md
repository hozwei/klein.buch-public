# Modul-Referenz

> Vertiefung zu §2 in `../ARCHITECTURE.md`. Eine Zeile pro Datei wäre zu wenig
> und ein Doc-Comment-Dump aus dem Code zu viel. Diese Datei beschreibt jedes
> Modul-Verzeichnis im Tauri-Backend (`src-tauri/src/`) auf der Ebene, auf der
> ein neuer Maintainer arbeitsfähig wird: Verantwortung, öffentliche
> Schnittstelle, Invarianten, Tests, ADR-Verweise.

Konvention: Verzeichnisse mit `mod.rs` sind eigenständige Module; einzelne
`*.rs`-Dateien im `src/`-Wurzel-Verzeichnis (`branding.rs`, `config.rs`,
`error.rs`, `lib.rs`, `main.rs`) werden in der entsprechenden Sektion am Ende
zusammengefasst.

---

## 1. `domain/` — Functional Core (pur)

`domain/` enthält die fachliche Wahrheit ohne I/O: Validierungen, Berechnungen,
Transformationen. Jede Funktion ist als `pub fn` testbar ohne DB/Sidecar/
Filesystem; alle Effekte fließen über Argumente rein und Rückgabewerte raus.

| Modul | Verantwortung | Wichtige Funktionen / Typen |
|---|---|---|
| `domain::invoice` | Rechnungs-Validierung (§14/§19/§33 UStG, §14c-Schutz) und Totals-Berechnung in Cent | `validate_for_issue`, `compute_totals` |
| `domain::contact` | Kontakt-Validierung | `validate_contact` |
| `domain::kleinunternehmer` | §19-Hinweistext (wortgleich), 5-Jahres-Bindungs-Logik | `paragraph_19_clause`, `waiver_window` |
| `domain::kleinbetragsrechnung` | §33 UStDV — vereinfachte Pflichtangaben bei ≤ 250 € Brutto | `is_small_amount` |
| `domain::storno` | Storno-Beleg aus Original ableiten (Mengen negiert) | `make_storno` |
| `domain::numbering` | Doc-Number-Format `{TYP}-{YYYY}-{NNNN}` (ADR 0012) | `format_number`, `next_for_year` |
| `domain::quote` | Angebots-Validierung, Konvertierung zu Rechnung | `validate_for_issue`, `convert_to_invoice` |
| `domain::expense` | Kosten-Validierung, §13b-Reverse-Charge, Brutto-in-EÜR | `validate_for_create` |
| `domain::private_movement` | Privatentnahme/-einlage als EÜR-neutrale Bewegung | `validate_for_create` |
| `domain::fiscal_year` | Geschäftsjahres-Grenzen, Cash-Basis-Periodisierung | `fiscal_year_of`, `is_locked_for` |
| `domain::asset` | Anlagen-Validierung (Anschaffungsdatum, Nutzungsdauer, GWG-Grenze) | `validate_for_create` |
| `domain::depreciation` | AfA-Berechnung (linear/GWG/Computer/Privatanteil), idempotent | `compute_year` |
| `domain::package` | Markdown-Subset-Parser, AST→Typst (Injection-Schutz), Plaintext-Flatten für BT-154 | `parse_markup`, `to_typst`, `to_plaintext` |
| `domain::travel` | Anfahrt km × Satz → Beleg-Position (deutsches Format, kein Geocoding) | `compute_travel_line` |
| `domain::recurring` | Recurring-Kosten-Vorlagen-Validierung, Fälligkeitsraster | `validate_for_create`, `due_periods_until` |
| `domain::recurring_invoice` | Abo-Rechnungs-Vorlagen-Validierung + `AutoMode` (draft/issue/issue_send) | `validate_for_create`, `due_periods_until` |
| `domain::dsgvo` | Art.-15-Report-Aufbau (read-only, interne Notizen ausgelassen) | `build_report` |
| `domain::anonymize` | Art.-17-Regel (`Anonymisiert #<hex>`, PII → NULL) | `make_anonymized_name`, `redact_contact` |
| `domain::factory_reset` | Server-Gating-Logik (welche Aufbewahrungs-Beweise sind nötig?) | `check_reset_allowed` |
| `domain::drop_folder` | Datei-Klassifikation (XML/PDF/Hidden/Other) + Monats-Sub-Ordner-Ableitung für den Drop-Folder-Sync, deterministisch über Dateinamen | `classify_file`, `processed_subdir` |

**Invarianten.** Kein `tokio::fs`, kein `sqlx`, kein `tokio::process` in
`domain/`. Wer einen Treffer findet, hat einen Bug. **Tests** liegen je Modul
inline in `#[cfg(test)] mod tests`; Coverage-Anspruch ist „jede Hardline-
Regel mit eigenem Negativtest".

ADRs: 0004 (FC/IS), 0005 (§19), 0007 (CII), 0012 (Doc-Number), 0016/0017
(Quote-Lifecycle), 0019 (Expense), 0020 (Private Movements), 0022 (Zufluss/
Abfluss), 0025 (AfA), 0031 (Packages/Travel), 0032 (DSGVO), 0033 (Abo-RI),
0037 (Drop-Folder-Klassifikation).

---

## 2. `einvoice/` — XRechnung + ZUGFeRD

Zweigeteilt: **Generator** (Core, pur) baut CII-XML aus Domain-Daten,
**Validator + Mustang-Bridge** (Shell) sprechen mit dem Java-Sidecar.

| Datei | Schicht | Verantwortung |
|---|---|---|
| `einvoice::types` | Core | Datentypen für die Pipeline (Validation-Ergebnisse, KoSIT-Meldungen) |
| `einvoice::generator` | Core | UN/CEFACT-CII (ADR 0007), §19-Klausel als BT-22-Note, BT-120-Tax-Category `E` |
| `einvoice::parser` | Core | CII + UBL (Empfang, ADR 0024), ZUGFeRD-PDF-Extract (XML aus PDF/A-3-Embedded-File), ZUGFeRD-Profile-Whitelist `check_profile_whitelist` (akzeptiert `en16931`/`extended`/`xrechnung`, lehnt `minimum`/`basic-wl` mit `ParseError::UnsupportedProfile` ab — ADR 0037 D-73) |
| `einvoice::validator` | Shell | KoSIT-Sidecar-Aufruf (Mock-Mode für CI), Rückgabe als strukturierte `KositReport` |
| `einvoice::mustang_bridge` | Shell | Mustang-Sidecar-Aufruf für ZUGFeRD-PDF/A-3-Erzeugung (PDF + XML → PDF/A-3 mit XML embedded) |

**Pipeline.** `generator::build_cii(invoice)` → `validator::validate(xml)` (im
KoSIT) → `klausel_check::verify(typst_template, paragraph_19=true)` →
`typst_render::render_invoice(pdf)` → `mustang_bridge::embed(pdf, xml)` →
`archive::store`.

**Mock-Mode.** Beide Shell-Module schalten über `MOCK_KOSIT=1` /
`MOCK_MUSTANG=1` auf In-Memory-Stubs, damit CI ohne Sidecar grün läuft. In
Produktion sind die Mocks aus (Sidecar-Health-Check in Block 0).

**Tests.** Generator hat einen vollständigen Golden-XML-Test (CII-Roundtrip).
Parser hat Fixtures aus echten XRechnungen + ZUGFeRD-PDFs. Validator hat
Integrations-Tests, die der Host beim Build laufen lässt (CI nutzt Mock).

ADRs: 0007, 0008, 0024, 0037 (Profile-Whitelist beim Empfang).

---

## 3. `pdf/` — Typst-Rendering und §19-Klausel-Check

| Datei | Schicht | Verantwortung |
|---|---|---|
| `pdf::klausel_check` | Core | Strenger §19-Marker-Check vor jedem Render (Rechnung + Angebot). Schlägt fehl, wenn ein Template ohne `// §19-KLAUSEL-BLOCK: REQUIRED`-Marker für eine §19-Rechnung verwendet werden soll. |
| `pdf::bundle` | Core | PDF-Merge (lopdf) für das Angebots-Druck-Bundle (Angebot + AGB + Datenschutz → ein Druck-PDF, ADR 0018). |
| `pdf::templates` | Core+Shell | Built-in-Vorlagen + doctype-bewusster Resolver + `list_templates`. Auflösung: `inputs/{name}.typ`-Override → Built-in; Sentinel `'default'` → `seller.default_pdf_template` (ADR 0030). |
| `pdf::typst_render` | Shell | Typst-`World`-Impl, ruft `typst_lib` für Rechnungen (PDF/A-3b-fähig), Angebote (Plain-PDF), Anlage-EÜR-PDF, Paket-Katalog-Broschüre. Logo via `World::file()`. |

**Wichtig.** `pdf/` macht **nicht** ZUGFeRD-Embedding. Das ist
`einvoice::mustang_bridge`. `typst_render` liefert ein PDF/A-3b-Basis-Dokument,
das `mustang_bridge` mit dem XML als Anhang versiegelt.

**Klausel-Check ist obligatorisch.** Vor jedem `render_invoice` läuft
`klausel_check::verify`. Schlägt der Check fehl, wird das Render abgebrochen
(kein still-broken Beleg). Das fängt versehentlich gelöschte Klausel-Blöcke
in editierten Templates ab (siehe §19-Logik in `paragraph-19.md`).

**Tests.** Klausel-Check hat einen Test pro Template-Variante; Render-Tests
arbeiten gegen Snapshot-PDFs. Bundle-Test verifiziert dass die Anhänge in
Reihenfolge erscheinen.

ADRs: 0008, 0018, 0030.

---

## 4. `archive/` — write-once-Beleg-Archiv

| Datei | Schicht | Verantwortung |
|---|---|---|
| `archive::store` | Shell | `store(kind, bytes) -> ArchiveId`. Schreibt Datei nach `%APPDATA%\…\archive\{YYYY}\{kind}\{uuidv7}.{ext}`, berechnet SHA-256, persistiert in `archive_entries`, setzt `chmod 0o400` (read-only). Idempotent über Hash-Lookup. |
| `archive::audit` | Shell | Audit-Events für Archive-Operationen (`archive.stored`, `archive.integrity_tamper`, `archive.integrity_missing`). |
| `archive::integrity_check` | Shell | Re-Hash aller Archiv-Dateien, Mismatch → Tamper-Event; Datei-fehlt → Waisen-Event (G1-HARDEN.4). |

**Hardline (ADR 0006).** Eine `ArchiveEntry` ist nach `store` **unveränderlich**
(`trg_archive_no_update` schützt `hash`/`path`/`size`). Tamper wird sofort
gemeldet, nicht ignoriert. Re-Hash läuft monatlich (`scheduler::
integrity_check_cron`) und manuell (`commands::system::archive_integrity_run`).

**Tests.** Roundtrip (store + read + hash-match), Tamper-Detection (Datei
modifizieren → next read schlägt fehl), Waisen-Detection (Datei löschen →
`integrity_missing`-Event, kein Tamper-Fehlalarm), idempotenter Re-Store.

ADRs: 0006.

---

## 5. `db/` — Persistence-Layer

| Datei / Verzeichnis | Schicht | Verantwortung |
|---|---|---|
| `db::mod` | Shell | `open_pool(path, key)` (SQLCipher-Pool, `PRAGMA key`), `prepare_filesystem` (Verzeichnisse + Phase-B-Marker für Restore und Factory Reset), Migrations laufen über `sqlx::migrate!` aus `migrations/` |
| `db::models` | Shell | sqlx-Row-Typen (DB-nahe), nicht zu verwechseln mit Domain-Typen — Mapping in den Repos |
| `db::numbering` | Shell | Atomare Nummern-Allokation aus `doc_number_counters` pro `{typ, year}` |
| `db::schema_version` | Shell | Liest/schreibt `app_settings.schema_version`; Mismatch beim Start ⇒ App stoppt (Down-Migration-Schutz) |
| `db::triggers` | Shell-Stub | Dokumentiert die in den Migrations-SQLs definierten Triggers (Code-Stub, die Wahrheit lebt in `migrations/*.sql`) |
| `db::repo::*` | Shell | Repo pro Domäne (21 Files), CRUD + State-Transitions; alle Inserts/Updates über parametrisierte Queries; Locked-Belege werden über DB-Trigger geschützt |

**Wichtigste Repos:** `invoices`, `quotes`, `expenses`, `private_movements`,
`payment_accounts`, `assets`, `depreciation`, `recurring`, `recurring_invoice`,
`packages`, `legal_documents`, `attachments`, `contacts`, `seller_profile`,
`audit_log`, `email_log`, `backup_log`, `mail_accounts`, `euer`, `dsgvo`,
`app_settings`.

**Locked-Belege-Pattern.** Repos haben getrennte Funktionen für „Draft-Update"
(erlaubt Kernfeld-Änderungen) und „State-Transition" (erlaubt nur die in
Trigger-Whitelists genannten Felder zu setzen). Versuche, gelockte Kernfelder
zu ändern, scheitern am Trigger, nicht erst an einem Repo-Check.

**Tests.** Repos haben Integrations-Tests gegen eine echte SQLCipher-DB
(In-Memory geht für STRICT+Trigger nicht ausreichend), siehe `tests/*_test.rs`.
Schema-Version-Check hat eigenen Test (Mismatch → Bootstrap-Fehler).

ADRs: 0003, 0006, 0035 (SQLCipher).

---

## 6. `scheduler/` — 5-Minuten-Tick

| Datei | Schicht | Verantwortung |
|---|---|---|
| `scheduler::tick` | Shell | Tokio-Interval (300 s), `MissedTickBehavior::Skip`, `ensure_started`-Einmal-Guard (post-Unlock). Ruft die sechs Jobs in fester Reihenfolge. |
| `scheduler::recurring` | Shell | Fällige eingangsseitige Recurring-Kosten materialisieren (`process_due`), idempotent über Stichtags-Raster |
| `scheduler::recurring_invoice` | Shell | Fällige ausgangsseitige Abo-Rechnungen materialisieren über die normale draft→lock-Pipeline, Belegdatum=heute, Catch-up holt verpasste Perioden |
| `scheduler::reminders` | Shell | Regel-getriebene Hinweise (`notify::rules`), inkl. `backup_overdue` Off-Site-bewusst und `rule_backup_result` |
| `scheduler::integrity_check_cron` | Shell | Monatlicher Archiv-Hash-Verify, Tamper- und Waisen-Events |
| `scheduler::depreciation_year_close` | Shell | Auto-AfA zur GJ-Wende, default-an per `app_settings.fiscal_year_auto_close` |
| `scheduler::drop_folder` | Shell | Watched-Folder-Sync (ADR 0037): liest periodisch (Polling, kein `notify`-Crate — D-71) den konfigurierten Drop-Folder, schickt jede XML/PDF durch dieselbe Empfangs-Pipeline wie der UI-Import (`commands::expenses::{parse_einvoice_with_paths, create_from_einvoice_with}` — Pipeline-Reuse, D-74/D-77) und verschiebt die Datei nach `processed/YYYY-MM/` oder `failed/` (D-75/D-79). Inbox-only via `notify::store::create` (R4-007, ADR 0027 Pt. 5). |

**Gating.** Der Scheduler startet erst nach erfolgreichem Unlock; ohne Pool im
Tauri-State kein sinnvoller Tick. Mehrfache Unlocks sind no-ops
(`AtomicBool`-Einmal-Guard). Details: `../ARCHITECTURE.md` §3 + §5.

**Tests.** Jeder Job hat einen `process_due`-Unit-Test mit gestelltem
`today` + Fixture-DB. Tick-Loop hat einen `scheduler_starts_only_once`-Test
(Einmal-Guard).

ADRs: 0023, 0027, 0033, 0037 (Drop-Folder als sechster Job).

---

## 7. `backup/` — Verschlüsselte Backups und Reset-Maschinerie

| Datei | Schicht | Verantwortung |
|---|---|---|
| `backup::mod` | Shell | `BackupSession` (Memory-Halter der Passphrase der laufenden Session), `verify_passphrase`, `backup_setup_passphrase`, `backup_unlock`, `create_now` (entry point, ruft Snapshot + Manifest + Target) |
| `backup::snapshot` | Shell | SQLCipher-DB-Snapshot (`VACUUM INTO`/Datei-Kopie unter Lock; Header-Salt bleibt, Backup ist as-is verschlüsselt) |
| `backup::manifest` | Shell | Manifest-JSON (App-Version, Schema-Version, Hash, Erzeugungs-Datum, Ziel-Typ), wird in der Hülle mit verschlüsselt |
| `backup::target` | Shell | `BackupTarget`-Enum (Verzeichnis ODER SFTP), `resolve_target` (mit Auto-Detect OneDrive/iCloud), `write_backup` (async, SFTP-Naht) |
| `backup::sftp` | Shell | `russh`/`russh-sftp`, Host-Key-Pinning (SHA-256, TOFU), Passwort im Keychain, nur Passwort-Auth |
| `backup::rotation` | Shell | Floor (lokal, 7/3/1) + Off-Site-Spiegelung „immer zweifach", `keep_n_newest` pro Klasse, **global neuestes Backup nie löschen** |
| `backup::encrypt` | Shell | Argon2id-Hüllen-Key-Derivation + AES-256-GCM für die Backup-Hülle; **getrennt** von der SQLCipher-Kette (die ist im DB-Header) |
| `backup::restore` | Shell | Phase-A-Live-Logik (Manifest lesen, Hash-Verify, Schema-Version-Check, Marker schreiben), Phase-B-Apply (im `db::prepare_filesystem`) |
| `backup::factory_reset` | Shell | `factory_reset_request` (Phase A: Marker + Keychain-Wipe-Vorbereitung), `apply_pending` (Phase B: gesamten `data_dir` nuken, Kern-Verzeichnisse leer neu anlegen) |

**Wichtig.** Live-Commands schließen den Pool **nicht** (sonst „closed pool"-
Bug). Destruktive FS-Ops laufen immer in Phase B beim nächsten Start, vor
Pool-Open. Vorgemerktes Audit (Restore) kommt aus `db::PendingRestoreAudit`
ins `audit_log`, sobald der Pool wieder offen ist.

**Tests.** Restore-Roundtrip mit 1000 Zeilen (G1-HARDEN.1); Rotation-
Invarianten (G1-HARDEN.5); Pre-Restore-Backup wird erzwungen; SFTP-Arm hat
einen Mock-Server-Test; Encryption-Migration (Klartext→SQLCipher) ist getestet.

ADRs: 0009, 0034, 0035, 0036.

---

## 8. `mail/` — Versand (SMTP + Microsoft Graph)

| Datei | Schicht | Verantwortung |
|---|---|---|
| `mail::smtp` | Shell | `lettre`-Async-Sender, TLS/STARTTLS, Multi-Attachment, neutrales Result-Mapping |
| `mail::oauth_ms` | Core+Shell | PKCE-S256 + Loopback-Capture (Core: URL-Bau, State/Verifier-Generierung), Token-Exchange + Refresh + Graph-Send (Shell) |
| `mail::keyring` | Shell | OS-Keychain-Adapter (`keyring`-Crate, native Backends): SMTP-Passphrase + OAuth-Refresh-Token (gechunkt wegen Windows-2560-Limit) + SFTP-Backup-Passwort |
| `mail::templates` | Shell | Tera-Render für Subject/Body aus `inputs/mail-templates/*.tera`; Variablen pro Beleg-Typ |

**Dispatch.** `mail::dispatch_send(account, msg)` wählt SMTP vs. Graph anhand
des Account-Typs. Beide Wege landen am Ende in `db::repo::email_log` mit der
Provider-Antwort. Audit-Log enthält das Versand-Event **ohne** Passphrase
oder Token.

**Tests.** OAuth hat End-to-End-Test mit Mock-Authority. SMTP hat Mock-Server-
Test (offline). Keyring-Tests laufen über native OS-Backends (Host-Smoke);
der Mock-Store wird **nur** unter `#[cfg(test)]` gesetzt — in Produktion gibt
es keinen In-Memory-Fallback.

ADRs: 0011, 0028.

---

## 9. `migration_export/` — Steuerberater-/Migrations-Export

| Datei | Schicht | Verantwortung |
|---|---|---|
| `migration_export::export` | Shell | Erzeugt ZIP unter `%TEMP%` mit allen relevanten Daten read-only |
| `migration_export::json_dump` | Shell | JSON-Dump aller Tabellen (kanonische Struktur, nicht migrations-versioned) für maschinelle Weiterverarbeitung |

**Inhalt des ZIPs.** Aktive `seller_profile`, alle `contacts` (inkl. anonyme),
alle `invoices` + `items` + Archiv-Pfade, `quotes`, `expenses`, `assets`,
`depreciation_entries`, `legal_documents`, alle archivierten Dateien (PDFs,
XMLs, Anhänge), `audit_log` Auszug. **Keine** Passphrase, **keine** Token.

**Tests.** Roundtrip: aus dem ZIP lassen sich Beleg-PDFs öffnen und mit dem
gespeicherten Hash verifizieren. Anonymisierte Kontakte erscheinen anonymisiert.

ADRs: 0026 (im Kontext EÜR-Export).

---

## 10. `notify/` — In-App-Inbox und OS-Notifications

| Datei | Schicht | Verantwortung |
|---|---|---|
| `notify::store` | Shell | CRUD auf `notifications` (DB-Tabelle), Dedup-Key, Ungelesen-Zähler |
| `notify::rules` | Shell | Regel-Engine: `backup_overdue` (Off-Site-bewusst), `rule_backup_result` (Fehler immer, Erfolg nur manuell), `recurring_due_soon`, `integrity_warning` u. a. |
| `notify::emit` | Shell | Erzeugt eine Notification (Store + ggf. OS-Native), Dedup-Schutz |
| `notify::os_native` | Shell | Tauri-OS-Notification (Win-Toast / macOS Notification Center); unter `#[cfg(test)]` no-op (vermeidet `TaskDialogIndirect`-Crash in Integrationstests) |

**Disziplin.** Manuelle Backups → Erfolgs-Notification ist OK (User hat
explizit getriggert). Auto-Backups (Lock-Events) → nur Fehler-Notification,
sonst zu viel Lärm. `backup_overdue` zählt **Off-Site**-Backups, nicht den
Floor — wer keinen Off-Site-Pfad hat, kriegt aber trotzdem keine
Dauer-Warnung (Schwelle ist nutzerseitig schaltbar).

**Tests.** Dedup-Key verhindert Doppel-Notification; OS-Native ist im Test
no-op; Rule-Trigger gegen Fixture-State.

ADRs: 0027, 0034.

---

## 11. `fiscal_year/` — Geschäftsjahres-Abschluss

| Datei | Schicht | Verantwortung |
|---|---|---|
| `fiscal_year::lock` | Shell | `close_year(year)` — Backup-Unlock-Pflicht, AfA buchen, Anlagen + Abschreibung sperren, EÜR-Snapshot ins `fiscal_year_locks` (append-only), Audit, Auto-Backup |
| `fiscal_year::transition` | Shell | GJ-Übergangs-Helfer (Jahreswechsel-Logik fürs Auto-AfA-Cron) |
| `fiscal_year::guard` | Shell | `ensure_year_open(year)`-Guard in Command-Wrappern. Storno-Pfad bleibt explizit erlaubt (Korrektur in Folgejahr möglich). |

**Hardline (ADR 0027).** `close_year` ist **unumkehrbar**. Versuche, danach
Belege im geschlossenen Jahr zu ändern, scheitern am Guard auf Command-Ebene
und am Trigger auf DB-Ebene.

**Tests.** `close_year` lehnt nicht-abgelaufene Jahre ab; setzt `fiscal_year_locks`
und blockiert Folge-Edits; Storno bleibt zugelassen.

ADRs: 0027.

---

## 12. `assets/` und `depreciation/` — Anlagenverzeichnis + AfA

| Datei | Schicht | Verantwortung |
|---|---|---|
| `assets::afa_tabellen` | Shell | Laden der AfA-Tabellen aus `inputs/afa-tabellen.json` (menschen-maintained, Steuerberater-Wartung) |
| `depreciation::compute` | Core | `compute_year(asset, year, today)` — idempotente AfA-Berechnung (linear/GWG/Computer-1/3/Privatanteil) |
| `depreciation::accrue_yearly` | Shell | Schreibt das Compute-Ergebnis als `depreciation_entries`-Zeile (idempotent über `(asset_id, year)`), Audit |

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
| `euer::aggregate` | Core | EÜR-Aggregation Cash-Basis §11 EStG (ADR 0022); Storno als negative Einnahme zum Storno-Datum (kein paid_amount-Netting möglich) |
| `euer::elster_csv` | Core | Zeilen-Mapping für ELSTER-Ausfüllhilfe |
| `euer::datev_csv` | Core | DATEV-EXTF-Buchungsstapel (SKR03-Default, SKR04 wählbar) |
| `euer::detail` | Core | Einzelaufstellung pro Position für die Steuerberater-ZIP |
| `euer::stb_package` | Shell | Steuerberater-ZIP (Anlage-EÜR-PDF + DATEV-CSV + Einzelaufstellungen + Belege) |

**Cash-Basis.** Einnahmen am `paid_at`, Ausgaben am `paid_date`, Privatbewegungen
EÜR-neutral. Teilzahlung pro Zahlungsjahr separat.

**Tests.** Aggregate-Tests gegen Fixture-Datasets (klein, mit Storno, mit
Teilzahlung über Jahresgrenze, mit GJ-Lock).

ADRs: 0010, 0022, 0026.

---

## 14. `commands/` — Tauri-IPC-Bindings

24 Dateien, eine pro Domäne (`invoices`, `quotes`, `expenses`, `assets`,
`depreciation`, `payment_accounts`, `recurring`, `recurring_invoice`,
`packages`, `legal_documents`, `dsgvo`, `factory_reset`, `attachments`,
`backup`, `mail`, `notifications`, `fiscal_year`, `euer`, `migration_export`,
`pdf`, `private_movements`, `settings`, `system`, `contacts`, plus `mod.rs`).

**Disziplin.** Jeder Command ist dünn:

1. Args (camelCase aus dem Frontend) deserialisieren.
2. Pool aus Tauri-State holen — vor Unlock nicht aufrufbar (außer Backup-/
   Onboarding-Path).
3. Guard-Calls (`ensure_year_open` etc.).
4. Domain-Validate → Repo-Aufruf → Result mappen.
5. Audit-Log + ggf. Auto-Backup-Trigger.

Keine fachliche Logik in `commands/`. Wer in einem Command eine Berechnung
findet, sollte sie in `domain/` verschieben.

**Tests.** Commands haben Integrations-Tests, die die ganze Pipeline durch-
laufen (Repo-Layer + Trigger + Audit). Reine Args-Validierung wird über
Domain-Tests abgedeckt.

ADRs: 0002, 0004.

---

## 15. Wurzel-Module (`src/*.rs`)

| Datei | Verantwortung |
|---|---|
| `main.rs` | `tauri::Builder::default()`-Setup, ruft `lib::run` (siehe `lib.rs`) |
| `lib.rs` | Plugin-Init (`opener`, `notification`, `dialog`), Tauri-State (`BackupSession`, `PendingRestoreAudit`, `SchedulerStarted`), `setup`-Closure (nur `prepare_filesystem`, kein Pool-Open), `invoke_handler!` mit allen Commands |
| `config.rs` | `Paths::from_handle(app)` — Auflösung aller `%APPDATA%`-Pfade (`data_dir`, `archive_dir`, `backups_dir`, `temp_dir`, `inputs_dir`) |
| `branding.rs` | Logo/Banner für PDF-Templates (Resolver für `seller_profile.logo_archive_id`) |
| `error.rs` | `enum Error` + `Result<T> = std::result::Result<T, Error>` — alle Module geben über diese typisierten Fehler zurück |

ADRs: 0002 (Tauri-Setup), 0035 (Bootstrap-Inversion).

---

## Letzte Verifikation

Stand: 2026-05-27, Schema v30, ADRs 0001–0037. Quelle:
`klein-buch/src-tauri/src/**/*.rs` (Verzeichnis-Listing) und die jeweiligen
`mod.rs`-Header-Doc-Kommentare. Bei strukturellen Modul-Änderungen diese
Datei mit aktualisieren.
