# Klein.Buch — Architektur (Stand v1.0-RC)

Klein.Buch ist eine lokale EÜR-Buchhaltung für deutsche §19-Kleinunternehmer.
Local-first, Single-User, offline. Dieses Dokument ist die konsolidierte
Architektur-Referenz: Schichten, Module, Daten- und Kontrollflüsse,
Bootstrap-Reihenfolge, Sidecar-Bridge, Scheduler-Modell und die nicht
verhandelbaren Hard-Rules.

**Implementierungs-Stand:** Phasen 1–2D (Blöcke 0–17, v0.1.0) + Phase 3 (Pakete/
Anfahrt, P1–P4, ADR 0031) + DSGVO Auskunft/Anonymisierung (Blöcke 18/19, ADR
0032) + Phase 4 (Abo-Rechnungen, RI-1..RI-3, ADR 0033) + **G1 Security/Backup-
Härtung** (G1-ENC/BKP/LOG/NOTIFY/RESET/HARDEN, grün 2026-05-25; ADRs 0034–0036).

**Schema-Stand v30** (Migrationen 0001–0030 forward-only; 0028 = R1-Review
Append-only-Hardening, 0029 = ADR 0037 Drop-Folder-Settings, 0030 = UI-
Sprach-Politur „Drop-Folder → Rechnungs-Eingang", Notification-Labels).
**ADRs 0001–0037** in `docs/adr/`. Implementierungs-Notizen je Themenkreis
in `…\Buchhaltung\memory\klein-buch\`. Die vollständige Produkt-Spec liegt
in `PRD-klein-buch.md`.

---

## 1. Schichten

```
┌───────────────────────────────────────────────────────────────────┐
│  Frontend — Svelte 5 Runes (routes/ + lib/: api, types, format)   │
└────────────────────────┬──────────────────────────────────────────┘
                         │ Tauri IPC (invoke, camelCase-Args)
┌────────────────────────▼──────────────────────────────────────────┐
│  Tauri Commands (Imperative Shell) — src-tauri/src/commands/*      │
│  dünn: Args → Domain/Repo → Result<T, Error>                       │
└────────────────────────┬──────────────────────────────────────────┘
              ┌──────────┴───────────┐
   ┌──────────▼─────────┐  ┌─────────▼──────────────────────────────┐
   │  Functional Core   │  │  Imperative Shell (I/O)                │
   │  (pure, testbar)   │  │                                        │
   │  - domain::invoice │  │  - db (sqlx + SQLCipher, repo/*)       │
   │  - domain::contact │  │  - archive (write-once + SHA-256)      │
   │  - kleinunternehmer│  │  - mail (smtp/keyring/templates/oauth) │
   │  - storno          │  │  - pdf::typst_render (World-Impl)      │
   │  - numbering       │  │  - einvoice::validator (KoSIT)         │
   │  - einvoice::gen   │  │  - einvoice::mustang_bridge            │
   │  - pdf::klausel    │  │  - backup (target/sftp/snapshot/       │
   │  - euer::aggregate │  │             rotation/encrypt/restore/  │
   │  - domain::package │  │             factory_reset)             │
   │  - domain::travel  │  │  - migration_export, scheduler         │
   │  - domain::dsgvo   │  │  - notify (store/rules/emit/os_native) │
   │  - domain::anon.   │  │  - fiscal_year, assets, depreciation   │
   │  - recurring_inv.  │  │                                        │
   └────────────────────┘  └─────────┬──────────────────────────────┘
                                     │ stdin/args, Temp-Files
                          ┌──────────▼───────────────┐
                          │  Java-Sidecar (jlink-JRE) │
                          │  KoSIT-Validator + Mustang │
                          └──────────────────────────┘

  ┌─────────────────────────────────────────────────────────────┐
  │ Datei-Layer (lokal, %APPDATA%\de.wildbach.kleinbuch\)        │
  │  • SQLite-Datei (SQLCipher-verschlüsselt seit G1-ENC)       │
  │  • archive/  write-once PDF + XML, SHA-256, chmod 0o400     │
  │  • backups/  Argon2id + AES-256-GCM (lokaler Floor)         │
  │  • + Off-Site-Ziele: Verzeichnis (USB/NAS/Cloud) oder SFTP  │
  └─────────────────────────────────────────────────────────────┘
```

**Functional Core / Imperative Shell** (ADR 0004). Validierung, Totals-
Berechnung, XML-Generierung, Klausel-Check, AfA-Berechnung, EÜR-Aggregation,
Markdown→Typst-AST und Anonymize-Regeln sind pure Funktionen ohne I/O. Alles,
was Dateien, DB, Netzwerk oder den Sidecar berührt, lebt in der Schale. Tests
des Cores laufen ohne DB/Sidecar/Filesystem.

**Verschlüsselung at Rest** (ADR 0035, G1-ENC). Die SQLite-Datei ist
SQLCipher-verschlüsselt. Der Schlüssel wird via PBKDF2-HMAC-SHA512 (Salt im
DB-Header) aus der App-Passphrase abgeleitet. Die App-Passphrase ist
gleichzeitig App-Login und Backup-Schlüssel-Wurzel (eine Geheimnis-Kette).
Ohne Passphrase kein Pool-Open und damit kein App-Zugriff.

---

## 2. Module

### 2a. Phase 1 (Rechnungen, Backup, Mail)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `domain::invoice` | Core | `validate_for_issue` (§14/§19/§33), `compute_totals` (Cent, kaufm. Rundung), §14c-Schutz |
| `domain::contact` | Core | Kontakt-Validierung |
| `domain::kleinunternehmer` | Core | §19-Hinweistext (wortgleich), 5-Jahres-Bindung |
| `domain::storno` | Core | Storno-Beleg aus Original ableiten (Mengen negiert) |
| `domain::numbering` | Core | Doc-Number-Format `{TYP}-{YYYY}-{NNNN}` (ADR 0012) |
| `einvoice::generator` | Core | XRechnung als UN/CEFACT **CII** (ADR 0007), BT-22/BT-120 §19 |
| `pdf::klausel_check` | Core | strenger §19-Marker-Check vor Render (Rechnung + Angebot) |
| `db` + `db::repo::*` | Shell | sqlx/SQLite-via-SQLCipher, STRICT, Numbering, GoBD-Trigger |
| `archive` | Shell | write-once, SHA-256, read-only, Tamper-Detection |
| `einvoice::validator` | Shell | KoSIT-Bridge (Sidecar), Mock-Mode |
| `einvoice::mustang_bridge` | Shell | ZUGFeRD-PDF/A-3 (Sidecar), Mock-Mode |
| `pdf::typst_render` | Shell | Typst-`World`; `render_invoice` (PDF/A-3b) + `render_quote` (Plain-PDF) |
| `mail::smtp` | Shell | lettre Async, Multi-Attachment, TLS/STARTTLS |
| `mail::keyring` | Shell | OS-Keychain für SMTP-Passphrase (ADR 0011) |
| `mail::templates` | Shell | Tera-Render Subject/Body aus `inputs/mail-templates/` |
| `backup::{target,snapshot,manifest,rotation,encrypt,restore,sftp,factory_reset}` | Shell | Argon2id + AES-256-GCM, Tiers, Floor + Off-Site, SFTP, SQLCipher-Migration, Restore-/Reset-Phase-B (ADR 0009/0034–36) |
| `migration_export` | Shell | offenes ZIP für Steuerberater/Migration |
| `commands::*` | Shell | Tauri-IPC-Bindings pro Domäne |

### 2b. Phase 2A–2D (Angebote, Kosten, Recurring, E-Rechnung-Empfang, Anlagen/AfA/EÜR, Notifications, OAuth, Design, PDF-Vorlagen)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `domain::quote` | Core | Angebots-Validierung/Totals (reuse Invoice-Totals), §19-Schutz, `convert_to_invoice` (ADR 0016/0017) |
| `domain::expense` | Core | Kosten-Validierung, §13b-Reverse-Charge, brutto-in-EÜR (ADR 0019) |
| `domain::depreciation` | Core | AfA-Berechnung (linear/GWG/Computer/Privatanteil), idempotent (ADR 0025) |
| `domain::fiscal_year` | Core | GJ-Grenzen, Cash-Basis-Periodisierung |
| `euer::aggregate` | Core | EÜR-Aggregation Cash-Basis §11 EStG (ADR 0022) |
| `euer::{elster_csv, datev_csv, detail}` | Core | ELSTER-Zeilen-Mapping, DATEV-EXTF (SKR03/04), Einzelaufstellungen (ADR 0026) |
| `einvoice::parser` | Core | XRechnung CII+UBL + ZUGFeRD-Extract (Empfang, ADR 0024) |
| `pdf::bundle` | Core | PDF-Merge (lopdf) für das Angebots-Druck-Bundle (ADR 0018) |
| `pdf::templates` | Core+Shell | Built-in-Vorlagen + doctype-bewusster Resolver + `list_templates` (ADR 0030) |
| `mail::oauth_ms` | Core+Shell | PKCE/Authorize/Callback (Core) + Graph-Send/Refresh (Shell) (ADR 0028) |
| `notify::{store,rules,emit,os_native}` | Shell | In-App-Inbox + OS-Notifications, Dedup (ADR 0027) |
| `fiscal_year::{lock,transition,guard}` | Shell | prüfungssicherer GJ-Abschluss + `ensure_year_open`-Guard (ADR 0027) |
| `scheduler::{tick,recurring,reminders,integrity_check_cron,depreciation_year_close}` | Shell | 5-Min-Tick, Unlock-gated, Catch-up (ADR 0023/0027) |
| `assets::afa_tabellen` | Shell | AfA-Tabellen-Laden aus `inputs/afa-tabellen.json` |
| `db::repo::{expenses,private_movements,payment_accounts,recurring,assets,depreciation,euer,email_log,app_settings,legal_documents,attachments,quotes}` | Shell | Repos der Phase-2-Domänen, AGB/Datenschutz-Versionierung (ADR 0018) |
| `pdf::typst_render::render_euer` | Shell | „Anlage EÜR"-PDF + Deckblatt (ADR 0026) |

### 2c. Phase 3–4 + DSGVO (nach v0.1.0)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `domain::package` | Core | Markdown-Subset-Parser + AST→Typst (Injection-Schutz), Plaintext-Flatten für BT-154, Revisions-Validierung (ADR 0031) |
| `domain::travel` | Core | Anfahrt km × Satz → Beleg-Position, deutsches Format, kein Geocoding (ADR 0031) |
| `domain::dsgvo` | Core | Art.-15-Report-Aufbau (read-only; interne Notizen ausgelassen) (ADR 0032) |
| `domain::anonymize` | Core | Art.-17-Regel (`Anonymisiert #<hex>`, PII → NULL) (ADR 0032) |
| `domain::recurring_invoice` | Core | Abo-Vorlagen-Validierung + `AutoMode` (draft/issue/issue_send) (ADR 0033) |
| `db::repo::packages` | Shell | Paket-Revisionen append-only (`update_as_new_revision`, `rollback`), Provenienz-Snapshot (ADR 0031) |
| `db::repo::dsgvo` | Shell | Daten-Sammlung je Kontakt (`gather`) für Auskunft/Anonymisierung (ADR 0032) |
| `db::repo::recurring_invoice` | Shell | Abo-Vorlagen-CRUD (Stammdaten), `advance` (ADR 0033) |
| `scheduler::recurring_invoice` | Shell | Abo-Rechnungen am Stichtag materialisieren (`process_due`/`catch_up`/`run_now`) (ADR 0033) |
| `commands::{packages,dsgvo,recurring_invoice}` | Shell | Tauri-Bindings; `recurring_invoice` erzwingt §19 serverseitig (KB-0053) |
| `pdf::typst_render::render_package_catalog` | Shell | Paket-Broschüre (kein §14-Beleg, kein Archiv) (ADR 0031) |

### 2d. G1 — Security/Backup-Härtung (post-v0.1.0, vor v1.0)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `db::open_pool(path, key)` | Shell | SQLCipher-Pool-Open mit `PRAGMA key`, PBKDF2-HMAC-SHA512 (ADR 0035) |
| `backup::encrypt::{encrypt_db_file_in_place,migrate_plaintext_to_encrypted}` | Shell | `sqlcipher_export` + atomarer Swap mit Pflicht-Pre-Migration-Backup (G1-ENC.4) |
| `backup::target::{BackupTarget,resolve_target,write_backup}` | Shell | Ziel-Abstraktion Verzeichnis (USB/NAS/Cloud-Ordner) + SFTP, Auto-Detect OneDrive/iCloud (ADR 0034) |
| `backup::sftp` | Shell | `russh`/`russh-sftp`, Host-Key-Pinning (SHA-256, TOFU), Passwort im Keychain |
| `backup::rotation` | Shell | Floor (lokal, 7/3/1) + Off-Site-Spiegelung „immer zweifach" mit langer Aufbewahrung (30/12/7) |
| `db::repo::backup_log` | Shell | Append-only `backup_log` (Migration 0026): Datum, Name, Größe, voller Pfad, Ziel-Typ, Status, Auslöser. **Keine Passphrase.** |
| `notify::rules::rule_backup_result` | Shell | Off-Site-bewusster `backup_overdue` + Erfolg/Fehler-Hinweis, Inbox-only (Migration 0027) |
| `backup::factory_reset::{factory_reset_request,apply_pending}` | Shell | Zweiphasiger Factory Reset (Marker + restart → Nuke vor Pool-Open), ADR 0036 |
| `commands::factory_reset` | Shell | Gating: GoBD-Warnung, Export-First, Tipp-Bestätigung, Passphrase-Verify, Aufbewahrungs-Quittung |

### 2e. Phase 7 — E-Rechnungs-Empfangs-Politur (post-v1.0, ADR 0037)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `einvoice::parser::check_profile_whitelist` | Core | ZUGFeRD-Profile-Whitelist beim Empfang (`en16931`/`extended`/`xrechnung`); `minimum`/`basic-wl` → `ParseError::UnsupportedProfile`, weil seit der E-Rechnungs-Pflicht 2025 keine gültigen E-Rechnungen mehr (PV1-A2, ADR 0037 D-73) |
| `commands::expenses::expenses_receipt_xml_text` | Shell | Roh-XML-Viewer im Eingangsbeleg-Detail; bei ZUGFeRD via `mustang_bridge::extract_xml`, bei XRechnung direkt aus dem Archiv. Liest still über `archive::store::read_and_verify_silent` (kein `archive.read`-Audit pro Klick, PV1-A5, ADR 0037 D-72) |
| `domain::drop_folder` | Core | Datei-Klassifikation (XML/PDF/Hidden/Other) + Monats-Sub-Ordner für `processed/` (PV1-DROP) |
| `scheduler::drop_folder` | Shell | Watched-Folder-Sync als sechster Tick-Job + App-Start-Sweep (kein `notify`-Crate, OneDrive-Quirks); Pipeline-Reuse über die bestehenden Empfangs-Helfer; `processed/YYYY-MM/` vs. `failed/`; Inbox-only via `notify::store::create` (ADR 0037 D-71/D-74/D-75/D-77/D-78) |
| `notify::rules::{rule_drop_folder_import_ok, _failed}` | Shell | Default off für Erfolg (Inbox-only), default on für Fehler (Inbox + OS-Toast); Migration 0029 |

---

## 3. Bootstrap-Reihenfolge

Bis G1-ENC öffnete der `setup`-Closure den DB-Pool direkt beim App-Start. Seit
G1-ENC Schritt 2 ist das **invertiert**: ohne Passphrase kein Pool, ohne Pool
keine Migrationen, kein Scheduler, kein DB-Command. Die Reihenfolge sieht so
aus:

1. **Tauri-Setup-Closure.** Logging initialisieren. Tauri-State befüllen:
   `backup::BackupSession` (Memory-Halter für die Passphrase der laufenden
   Session), `db::PendingRestoreAudit` (Zwischenspeicher für einen beim Start
   angewendeten Restore-Audit), `scheduler::tick::SchedulerStarted`
   (Einmal-Guard für den Scheduler).
2. **`db::prepare_filesystem`.** Verzeichnis-Struktur unter `%APPDATA%\
   de.wildbach.kleinbuch\` anlegen, **vorgemerkte Restore-/Factory-Reset-Marker
   abarbeiten** (Phase B von Restore und Factory Reset — siehe §4.4 und §4.6).
   Der DB-Pool wird hier explizit **nicht** geöffnet. Fehler werden geloggt,
   aber die App beendet sich nicht — das Backup-Gate macht den Fehler im UI
   sichtbar.
3. **Frontend startet.** Die App prüft über `backup_needs_onboarding`, ob das
   eine Erstinstallation ist (Klartext-DB ohne Verifier, oder gar keine DB-
   Datei). Routet entweder zum Onboarding-Wizard oder zum Unlock-Screen.
4. **Onboarding-Wizard (Erstinstallation).** Pflicht-Passphrase setzen
   (Totalverlust-Warnung, Passwort-Manager-Empfehlung), Verifier-Datei
   schreiben, danach DB anlegen (verschlüsselt) oder eine vorhandene
   Klartext-DB via `migrate_plaintext_to_encrypted` umziehen (mit Pflicht-
   Pre-Migration-Backup). Bei Erfolg: Passphrase landet in `BackupSession`,
   Pool wird geöffnet, Schema-Migrationen laufen, Scheduler startet
   (`scheduler::tick::ensure_started`).
5. **Unlock-Screen (Folgestarts).** Passphrase-Eingabe → SQLCipher-Pool-Open →
   Schema-Version-Check (Mismatch ⇒ App stoppt, kein Down-Migration). Bei
   Erfolg: `BackupSession::set_passphrase`, Scheduler startet einmalig.
6. **`scheduler::tick::ensure_started`.** Startet die 5-Min-Tick-Schleife.
   Der erste Tick feuert **sofort**, weitere alle 300 s. `SchedulerStarted`
   ist ein `AtomicBool`-Einmal-Guard, mehrfache Unlock-Aufrufe (Theorie) sind
   no-ops.

**Konsequenz:** kein DB-Command kann vor Schritt 4/5 laufen. Alle Repos und
Commands holen den Pool aus dem Tauri-State; ist er nicht da, ist die App nicht
entsperrt und der Command sollte gar nicht erst aufrufbar sein. Die einzigen
Befehle, die vor Unlock erlaubt sind, gehören zum Onboarding-/Unlock-Pfad und
zur Backup-/Restore-Vorprüfung.

---

## 4. Daten- und Kontrollflüsse

### 4.1 Rechnung erstellen → Issue (Block 3)
UI-Form → `commands::invoices::create_draft` → DB-Draft (Nummer allokiert) →
User klickt *Lock & Issue* → `run_lock_pipeline`: Domain-Validate → CII-XML
generieren → KoSIT validate (Sidecar) → §19-Klausel-Check → Typst-PDF/A-3b →
Mustang ZUGFeRD → Archive write (PDF + XML, SHA-256, read-only) → DB-Lock
(`status='issued'`, `locked_at`) → Audit-Log → Auto-Critical-Backup (Floor +
Off-Site, siehe §4.5).

### 4.2 Rechnung versenden (Block 5)
Detail → *Senden* → `routes/invoices/[id]/send` lädt Account-Liste +
Body-Vorschau (`mail_invoice_preview` rendert `invoice-de`-Template) → User
wählt Konto, prüft Empfänger/Betreff/Body → `mail_send_invoice` →
`send_invoice_core`: Account laden → Empfänger auflösen (Buyer-Snapshot →
Kontakt) → Body rendern (Override gewinnt) → ZUGFeRD-PDF aus Archiv lesen
**+ SHA-256-Verify** → Passphrase aus Keychain (optional) → `mail::smtp::send`
oder Graph `/sendMail` (Multi-Attachment) → `invoices::mark_sent`
(`status='sent'`, `sent_at`) → `mail_accounts::touch_last_used` → Audit-Log
`invoice.sent` (ohne Passphrase) → `email_log` (append-only, Migration 0015).
Versand nur für gelockte, nicht-stornierte Rechnungen mit archiviertem PDF.

### 4.3 Storno (Block 3)
Storno erzeugt eine **neue** Rechnung (`is_storno_for`), durchläuft dieselbe
Lock-Pipeline, markiert das Original `canceled`. Keine Löschung (ADR 0006).

### 4.4 Restore (Block 4, mit G1-Anpassung)
Zweiphasig, damit kein Pool-Close im Live-Command nötig ist und Windows-File-
Locks nicht aufschlagen.

**Phase A (Live, in der laufenden App):** Settings → Backup wählen → Manifest
lesen → GoBD-Warnung → Pre-Restore-Snapshot → Passphrase → Decrypt + Hash-
Verify → Schema-Version-Check → **Marker schreiben**, der DB-Datei und Archiv
für den nächsten Start vormerkt → `AppHandle::restart()`.

**Phase B (nächster App-Start, in `db::prepare_filesystem`, vor Pool-Open):**
vorgemerkten Restore atomar anwenden (Swap der DB-Datei + Archiv). Audit landet
zwischengespeichert in `PendingRestoreAudit` und wird nach erfolgreichem Pool-
Open ins `audit_log` geschrieben. `inputs/` bleibt tabu.

### 4.5 Backup erstellen (G1-BKP/LOG)
`backup::create_now` → BackupSession entsperrt? → Snapshot der DB-Datei
(SQLCipher-AS-IS, gleicher Header-Salt wie Live) + Manifest +
Argon2id-abgeleiteter Hüllen-Key → AES-256-GCM-Container → `resolve_target`
liefert beides: Floor (lokal, `%APPDATA%\…\backups\`, Retention 7/3/1) **und**
Off-Site (Verzeichnis ODER SFTP, Retention 30/12/7). Schreiben über
`write_backup` (async, SFTP-Naht trennt Sync- und Net-Pfade) → `backup_log`-
Insert je Ziel (Erfolg/Fehler, voller Pfad, Größe, Auslöser; **niemals**
Passphrase) → Rotation pro Klasse (`keep_n_newest`, das global neueste Backup
ist nie löschbar) → optional Notification über
`rule_backup_result` (Fehler immer; Erfolg nur bei manuellem Backup, um
Auto-Lock-Backups nicht in Spam zu verwandeln).

### 4.6 Factory Reset (G1-RESET, ADR 0036)
Zweiphasig wie Restore.

**Phase A (Live):** `commands::factory_reset::factory_reset_check` listet
festgeschriebene Belege auf (über alle `locked_at`-Tabellen) und entscheidet,
ob Export-First **oder** Aufbewahrungs-Quittung Pflicht ist. UI: GoBD-Warnung →
optional `migration_export_run` → Tipp-Bestätigung („LÖSCHEN") → Passphrase-
Eingabe (server-verifiziert über `backup::verify_passphrase`) → finaler
confirmDialog. Bei OK: Marker schreiben → `AppHandle::restart()`. **Kein**
Pool-Close, **kein** Webview-Reload.

**Phase B (nächster Start, in `db::prepare_filesystem`, vor Pool-Open):**
gesamten `data_dir`-Inhalt nuken (DB inkl. WAL/SHM, Archiv, Floor-Backups,
Branding, Exporte, Restore-Staging), Kern-Verzeichnisse leer neu anlegen,
Keychain-Geheimnisse best-effort wipen (SMTP-/OAuth-je-Konto + SFTP-Backup-
Passwort). Off-Site-Backups (Cloud-Ordner, SFTP) bleiben — die löscht der
Nutzer selbst. `inputs/` liegt außerhalb und bleibt unberührt. Beim Neu-Start
greift wieder der Onboarding-Wizard.

### 4.7 Encryption-Migration (G1-ENC.4)
Beim ersten Entsperren einer noch im Klartext liegenden DB:
`migrate_plaintext_to_encrypted` legt ein Pflicht-Pre-Migration-Backup an, ruft
SQLCipher's `sqlcipher_export` mit der neu abgeleiteten Schlüssel, prüft die
verschlüsselte Kopie via Probe-Lesen, swappt atomar. Bei jedem Fehler bleibt
die Klartext-DB intakt. Audit-Log dokumentiert den Übergang.

### 4.8 Angebote: Lebenszyklus + Konvertierung (Block 6/7)
Eigener Belegkreis `AN-{YYYY}-{NNNN}`. Lifecycle:
`draft → (Festschreiben = Lock → sent) → accepted | rejected → converted | canceled`.
Festschreiben setzt `locked_at`; ab da greift `trg_quotes_immutable` auf den
Kernfeldern (Angebote sind keine E-Rechnungen → kein KoSIT/Mustang). Annahme
archiviert optional den unterschriebenen Vertrag write-once (`attachments`,
`parent_type='quote'`). Konvertierung läuft **nur aus `accepted`**:
`convert_to_invoice` (pure) → gemeinsamer Draft-Helper (setzt
`derived_from_quote_id`) → `mark_converted` (`accepted → converted`). Die
erzeugte Rechnung ist eine Draft und durchläuft die normale Lock-Pipeline (§4.1).
ADR 0016/0017.

### 4.9 Angebots-PDF, Rechtsdokumente, Bundle (Block 8)
`ensure_quote_pdf` rendert beim ersten Mal über `pdf::typst_render::render_quote`
(eingebettetes Quote-Template, §19-Klausel-Check) ein **Plain-PDF**, archiviert
es write-once (`ArchiveKind::QuotePdf`), setzt `quotes.pdf_archive_id`
(idempotent). AGB + Datenschutz als versioniertes `legal_documents` (write-once,
`ArchiveKind::LegalDocument`), höchstens eine aktive Version pro Typ, append-
only und immutable (DB-Trigger). `prepare_quote_dispatch` bindet die aktiven
Legal-Versionen append-only in `quote_legal_documents` (idempotent). Druck =
`pdf::bundle::merge_pdfs` (lopdf). Mail = `send_quote_core` hängt Angebot + AGB
+ Datenschutz als **drei** Anhänge an, Audit `quote.sent` mit den gebundenen
Versionen, kein Status-Wechsel. ADR 0018.

### 4.10 Kosten + E-Rechnung-Empfang (Block 9/11)
`expenses_create` → `domain::expense`-Validierung → Beleg-Upload write-once →
sofort gelockt (ADR 0019). §19 wirkt nur ausgangsseitig → Kosten gehen
**brutto** in die EÜR. Empfang: Datei-Upload → `einvoice::parser` (CII/UBL/
ZUGFeRD) → KoSIT-Prüfung **beratend** → Import als normale Kostenposition
(`paid_date = None`) + Original write-once archiviert (`ReceivedEinvoice`).
ADR 0024.

### 4.11 Anlagen/AfA → EÜR → Export (Block 12–14)
Anlage anlegen → manueller AfA-Lauf (`domain::depreciation`, idempotent) →
`depreciation_entries`. EÜR: `euer::load_inputs` (Shell) → `euer::aggregate`
(Core, Cash-Basis §11 EStG, ADR 0022) → read-only Report. Export:
`euer_export_*` → ELSTER-Ausfüllhilfe + „Anlage EÜR"-PDF (Typst), DATEV-EXTF
(SKR03-Default), Steuerberater-ZIP. AfA-Safeguard warnt bei ungebuchter AfA.
ADR 0025/0026.

### 4.12 Geschäftsjahres-Abschluss + Hinweise (Block 15)
`fiscal_year::close_year` (nur abgelaufene Jahre, Backup-Unlock-Pflicht): AfA
buchen → Anlagen + Abschreibung sperren → EÜR-Snapshot ins
Festschreibungsprotokoll (Migration 0013, no-update/no-delete) → Audit →
Auto-Backup; **unumkehrbar**. `guard::ensure_year_open` in den Command-Wrappern
(Storno bleibt erlaubt). Scheduler-Jobs siehe §5.

### 4.13 OAuth-Versand über Microsoft Graph (Block 16)
`mail_oauth_connect` → Authorize-URL (PKCE-S256) → Loopback-Capture →
`exchange_code` → **Refresh-Token gechunkt im OS-Keychain** (Windows-2560-
Limit), nicht-geheime Felder in `mail_accounts` (Migration 0014). Versand:
`dispatch_send` wählt SMTP vs. Graph; Graph holt Access-Token frisch via
`refresh_tokens` → `graph_send` (`/me/sendMail`). Jeder Versuch landet im
`email_log` (append-only, Migration 0015) mit Provider-Antwort. ADR 0028.

### 4.14 PDF-Vorlagen-Auswahl (Block 17a)
Globale Auswahl in `settings/pdf-templates` (`seller_default_template_set`,
§19-Schutz). Beim Render löst `pdf::templates::resolve_invoice_template` /
`resolve_quote_template` auf: `inputs/{name}.typ`-Override → Built-in;
Sentinel `'default'` → `seller.default_pdf_template`. Logo via `World::file()`.
Vorschau: `pdf_template_preview` rendert ein Dummy-PDF („Max Mustermann").
ADR 0030.

### 4.15 Pakete + Anfahrt (Phase 3)
Pakete leben versioniert in `package_revisions` (append-only, Trigger).
„Bearbeiten" = neue Revision, „Rollback" = neue Revision aus alter. Eine ins
Angebot/Rechnung übernommene Position ist ein **Snapshot** + Soft-Zeiger
(`source_package_*`) und ändert sich nie nachträglich; „Paket anpassen" löst
den Zeiger (→ NULL, Custom). `description_markup` (Markdown-Subset) treibt nur
den PDF-Block; das XML (BT-154) bekommt geplätteten Klartext (AST-only,
Injection-Schutz). Die Katalog-Broschüre ist **kein** Beleg (kein
Nummernkreis/Archiv, nur `email_log`). Anfahrt = km × Satz als normale
Position, ohne Geocoding. ADR 0031.

### 4.16 DSGVO: Auskunft + Anonymisierung (Block 18/19)
**Auskunft (Art. 15):** `dsgvo.export` sammelt alle Kontakt-Daten read-only und
schreibt ein ZIP (PDF + JSON + archivierte Originale + LIESMICH), genau ein
Audit, keine Beleg-Mutation. **Anonymisierung (Art. 17):** überschreibt die
Kontakt-Stammdaten (`Anonymisiert #<hex>`, PII → NULL); die in Belege
eingefrorenen Buyer-Snapshots bleiben (GoBD gewinnt). Guard: keine offenen
Entwürfe; einmalig + irreversibel. ADR 0032.

### 4.17 Abo-Rechnungen / Ausgangsseite (Phase 4)
Abo-Vorlage (`recurring_invoices`, Stammdatum) → der Scheduler-Job im
`run_tick` (Unlock-gated, plus Fälligkeits-Check beim Öffnen der Abo-Seite)
materialisiert am Stichtag über **dieselbe** draft→lock-Pipeline (§4.1):
Belegdatum = **heute** (nie rückdatiert, §14 Abs. 4 Nr. 3),
Leistungszeitraum als `delivery_date`. `auto_mode` steuert je Vorlage Entwurf /
Festschreiben / Festschreiben+Versand. §19 wird beim Vorlagen-Speichern
serverseitig erzwungen. Catch-up holt verpasste Perioden nach (alle mit
heutigem Belegdatum). ADR 0033.

### 4.18 Rechnungs-Eingang (Drop-Folder) für eingehende E-Rechnungen (Phase 7, ADR 0037)
UI-Label „Rechnungs-Eingang" (Settings-Menü); Code-Identifier `drop_folder_*`
(Module, Settings-Keys, Route, Rule-IDs) bleiben englisch nach CLAUDE.md-
Hard-Rule. Settings → Rechnungs-Eingang → Pfad wählen + Toggle aktivieren.
Der Scheduler-Job
`scheduler::drop_folder::run_sync` (Tick + App-Start-Sweep, plus manueller
„Jetzt synchronisieren"-Button) liest periodisch jede Top-Level-Datei,
klassifiziert sie über `domain::drop_folder::classify_file` (XML/PDF/
Hidden/Other) und schickt XML/PDF durch dieselbe Empfangs-Pipeline wie der
UI-Import (`commands::expenses::{parse_einvoice_with_paths,
create_from_einvoice_with}`, ADR 0024). Erfolg → Datei wandert nach
`processed/{YYYY-MM}/{name}`; Fehler oder unbekannte Endung → `failed/{name}`,
ohne Auto-Delete (Diagnose-Pfad). Hidden Files (`.DS_Store`, `Thumbs.db`,
`*.tmp`) werden ignoriert. Notifications laufen Inbox-only via
`notify::store::create` (`rule_drop_folder_import_ok` default off,
`rule_drop_folder_import_failed` default on). Polling statt `notify`-Crate
wegen OneDrive-Sync-Quirks (D-71).

### 4.19 Roh-XML-Viewer im Eingangsbeleg-Detail (Phase 7, ADR 0037)
Eingangsbeleg → Detail-Seite → „Roh-XML anzeigen" (nur sichtbar, wenn der
Beleg eine archivierte E-Rechnung als Quelle hat). `expenses_receipt_xml_text`
lädt die XML aus dem write-once-Archiv: bei `source_format=zugferd` via
`mustang_bridge::extract_xml` (XML steckt im PDF/A-3-Embedded-File); bei
`xrechnung-cii`/`xrechnung-ubl` direkt als UTF-8. Anzeige in einem globalen
`XmlViewerDialog` mit Pretty-Print, Copy-Button, Hash und Byte-Größe.
**Kein** `archive.read`-Audit pro Klick (`read_and_verify_silent`, D-72);
Tamper-Detection bleibt im Archiv-Modul.

### 4.20 ZUGFeRD-Profile-Whitelist beim Empfang (Phase 7, ADR 0037)
Der Parser ruft `einvoice::parser::check_profile_whitelist` direkt nach dem
CII-Walk und vor dem Currency-Check auf. Akzeptiert werden Profile mit
Substring `en16931`, `extended` oder `xrechnung` im
`GuidelineSpecifiedDocumentContextParameter.ID`; `minimum` und `basic-wl`
werfen `ParseError::UnsupportedProfile(<URN>)`. Begründung in ADR 0037 D-73:
seit der E-Rechnungs-Pflicht ab 2025 sind beide Profile keine gültigen
E-Rechnungen mehr und werden mit klarer Fehlermeldung abgewiesen statt
indirekt über einen KoSIT-Fail.

---

## 5. Scheduler-Modell

Ein einziger Hintergrund-Task (`scheduler::tick::start`), Tokio-Interval
**5 Minuten** (`TICK_INTERVAL_SECS = 300`). Verpasste Ticks werden **nicht
nachgefeuert** (`MissedTickBehavior::Skip`) — der nächste reguläre Tick deckt
den Rückstand ab, weil alle Jobs idempotent über ihr Stichtags-Raster sind.

**Gating.** Der Scheduler startet erst **nach** dem ersten erfolgreichen
Entsperren (siehe Bootstrap §3). `SchedulerStarted` ist ein `AtomicBool`-
Einmal-Guard pro Prozess: mehrfache Unlock-Aufrufe sind no-ops. Vor dem Unlock
gibt es keinen DB-Pool, also auch keinen sinnvollen Tick.

**Jobs pro Tick (in dieser Reihenfolge, jeder Job isoliert):**

1. **`scheduler::recurring`** — fällige eingangsseitige Recurring-Kosten
   materialisieren (Block 10). Auto-Anlage ist zusätzlich session-gated
   (defensiv).
2. **`scheduler::depreciation_year_close`** — Auto-AfA zur GJ-Wende, default-an
   per `app_settings.fiscal_year_auto_close`, abschaltbar (Block 15).
3. **`scheduler::integrity_check_cron`** — monatlicher Archiv-Hash-Verify;
   Hash-Mismatch erzeugt Audit-Event `archive.integrity_tamper`, fehlende
   Dateien `archive.integrity_missing` (G1-HARDEN.4).
4. **`scheduler::reminders`** — Regel-getriebene Hinweise (`notify::rules`),
   inkl. `backup_overdue` Off-Site-bewusst (G1-NOTIFY) und
   `rule_backup_result` (Fehler immer; Erfolg nur bei manuell ausgelöstem
   Backup).
5. **`scheduler::recurring_invoice`** — fällige ausgangsseitige Abo-Rechnungen
   materialisieren (Phase 4, RI-2).
6. **`scheduler::drop_folder`** — Watched-Folder-Sync (Phase 7, ADR 0037):
   neue Dateien klassifizieren, XML/PDF durch die Empfangs-Pipeline schicken,
   nach `processed/YYYY-MM/` bzw. `failed/` verschieben. Inbox-only via
   `notify::store::create` (R4-007). Zusätzlich ein App-Start-Sweep, damit
   frisch eingegangene Dateien nicht bis zu fünf Minuten warten müssen.

Manuelle Trigger: jede UI-Seite, die einen Job kennt, bietet einen
„Jetzt prüfen"-Knopf (`recurring_run_due_check`,
`recurring_invoices_run_due_check`, `archive_integrity_run`,
`notifications_run_checks`, `drop_folder_sync_now`). Diese Wege rufen die
Job-Funktionen direkt auf, unabhängig vom Tick.

---

## 6. Sidecar-Bridge

Ein **einziger** Java-Sidecar-Prozess (jlink-JRE), eingebettet ins Tauri-
Bundle als per-OS-Resource. Er hostet zwei Kommandozeilen-Aufrufe:

- **KoSIT-Validator** für E-Rechnungen (XRechnung CII + UBL, ZUGFeRD-XML).
- **Mustang Project** für PDF/A-3 + ZUGFeRD-XML-Embedding.

**Warum Sidecar.** Beide Tools sind Java und in Reife/Konformität unschlagbar.
Eine Reimplementierung in Rust ist Out-of-Scope (Risiko § 14/EN-16931-Konformität).
Sie laufen `stdin`/`args` + Temp-Files, ohne offene Sockets, ohne
Netzwerk-Outbound. ADR 0001 (Cross-OS-Sidecar-Bundles), ADR 0008 (Typst +
Mustang als kompletter PDF/A-3-Pfad).

**Lifecycle.**

- **Build.** `klein-buch/build-sidecar.ps1` baut auf Windows einen Minimal-JRE
  via `jlink` und packt die KoSIT- und Mustang-Jars dazu. Output landet als
  per-OS-Resource in `src-tauri/resources/sidecar/<os>/`. macOS und Linux sind
  **bewusst nicht** in v1.0 (siehe `RELEASE-1.0-GUIDE.md`).
- **Resolver zur Laufzeit.** `einvoice::validator` und
  `einvoice::mustang_bridge` finden den Launcher per-OS (`.cmd`/`.sh`/`.bat`)
  im Resource-Verzeichnis. Spawning via `tokio::process::Command`, Args über
  CLI, Input über Temp-Files unter `%TEMP%`. Stdout/Stderr getrennt eingelesen.
- **Mock-Mode.** Beide Module haben einen `MOCK_*`-Modus für Tests und CI ohne
  Sidecar (siehe Modul-Doc). In Produktion ist Mock aus.
- **Health-Check.** Block 0 Setup-Verifikation (Build-Disziplin) führt einen
  Aufruf jedes Tools mit einem Sample-Input aus. Schlägt der fehl, startet der
  Build nicht.
- **Outbound.** Keiner. KoSIT und Mustang arbeiten offline. Der einzige
  Outbound-Pfad in der App ist Mail-Versand (SMTP/Graph) und der ist
  user-getriggert.

**Trennung Core/Shell.** XML-Generierung und Klausel-Check sind Core (pur).
Validierung und PDF/A-3-Erzeugung sind Shell, weil sie den Sidecar als
Effekt brauchen. Wird der Sidecar getauscht (z. B. eine andere KoSIT-Version),
ändert sich nur die Shell-Schnittstelle, nicht der Core.

---

## 7. Hard-Rules

Diese Regeln sind nicht verhandelbar; jeder Block, der sie verletzen würde,
hält an und meldet sich (CLAUDE.md). Hier eine konsolidierte Sicht; Details
in den verlinkten ADRs.

- **GoBD** (ADR 0006). Festgeschriebene Belege unveränderlich (DB-Trigger),
  Storno statt Löschung, Archive write-once + SHA-256 + Re-Hash + chmod 0o400,
  Audit-Log append-only, 10 Jahre Aufbewahrung. Einzige sanktionierte Total-
  Löschung ist Factory Reset (ADR 0036, hartes Multi-Stage-Gating).
- **§19 Kleinunternehmer** (ADR 0005). Default `is_kleinunternehmer = true`,
  USt-Felder im UI gesperrt, BT-22-Klausel Pflicht im XML + sichtbar im PDF,
  §14c-Schutz im Backend, Verzicht-Toggle mit 5-Jahres-Bindungs-Warn-Dialog.
- **Backup** (ADR 0009 + 0034 + 0035). Passphrase-Setup im Onboarding
  erzwungen; Auto-Backup bei jedem Lock + täglich; **Floor (lokal, Retention
  7/3/1) + Off-Site (Verzeichnis ODER SFTP, Retention 30/12/7) immer zweifach**
  (best-effort); `backup_log` append-only mit voller Pfad/Größe/Status (Mig.
  0026); Passphrase **niemals** in DB, Logs, Audit oder Backup-Log.
- **Verschlüsselung at Rest** (ADR 0035, G1-ENC). Eine Geheimnis-Kette:
  App-Login = SQLCipher-Key (PBKDF2-HMAC-SHA512, Salt im DB-Header) =
  Backup-Schlüssel-Wurzel (Argon2id-Backup-Hülle). Bootstrap-Reihenfolge
  Passphrase vor Pool-Open ist Pflicht (§3). Verlust = Totalverlust by design.
- **Credentials** (ADR 0011/0028). SMTP-Passwort + OAuth-Refresh-Token nur im
  OS-Keychain (Refresh-Token gechunkt wegen Windows-2560-Limit); Access-Token
  nie persistiert; SFTP-Backup-Passwort im Keychain.
- **GJ-Abschluss** (ADR 0027). Abgeschlossene Geschäftsjahre unveränderlich
  (no-update/no-delete-Trigger), Abschluss nur für abgelaufene Jahre,
  unumkehrbar; Storno als Korrektur bleibt möglich.
- **Schema-Disziplin** (ADR 0003). STRICT-Tabellen, forward-only Migrationen
  (0001–0027, Version v27), Schema-Version-Check beim Start (Mismatch ⇒ App
  startet nicht), Geld als Integer-Cents, UUIDv7-PKs.
- **`inputs/` tabu.** Menschen-maintained Specs/Mockups/Logos. Maschinen
  schreiben nach `data/` und `build/`. Einzige stehende Ausnahme:
  `inputs/pdf-templates/` darf direkt editiert werden (Manuel-Erlaubnis
  2026-05-23) und ist über alle Vorlagen inhaltlich einheitlich zu halten.
- **Local-first strict.** Keine Telemetrie, kein Auto-Update, kein Cloud-Sync.
  Outbound nur auf explizitem User-Trigger (Versand) und über den Sidecar
  oder die Mail-Pfade.

---

## 8. Phase-Plan

18 Blöcke in 5 Phasen plus Post-v0.1.0-Erweiterungen plus v1.0-Härtung.

- **Phase 1 (Blöcke 0–5).** Setup, Foundation, Invoice-Pipeline, Backup,
  Mail. Produktiv nutzbar ab Ende Phase 1.
- **Phase 2A (6–8).** Angebote.
- **Phase 2B (9–11).** Kosten, Recurring-Kosten, E-Rechnung-Empfang.
- **Phase 2C (12–14).** Anlagen, AfA, EÜR, EÜR-Export.
- **Phase 2D (15–17).** Notifications, OAuth, Design-System, PDF-Vorlagen,
  E2E, Release.
- **Phase 3 (P1–P4, nach v0.1.0).** Anfahrt + Paket-Katalog + Paket-in-Beleg
  + Katalog-Broschüre (ADR 0031).
- **DSGVO (Blöcke 18/19, nach Phase 3).** Auskunft Art. 15 + Anonymisierung
  Art. 17 (ADR 0032).
- **Phase 4 (RI-1..RI-3, nach DSGVO).** Wiederkehrende Ausgangsrechnungen
  (ADR 0033).
- **G1 Security/Backup-Härtung (post-v0.1.0, vor v1.0).** ENC (SQLCipher,
  Passphrase-Kette) + BKP (Targets, SFTP, Tiers) + LOG (`backup_log`) +
  NOTIFY (Off-Site-bewusst, `rule_backup_result`) + RESET (Factory Reset,
  zweiphasig) + HARDEN (Restore-Roundtrip, Keyring, Archive-Integritäts-
  Unterscheidung, Rotation-Invariante, DSGVO-Tests). ADRs 0034–0036. Grün
  2026-05-25.
- **G2-DOC (v1.0-Blocker, läuft).** Tech-Doku-Konsolidierung (dieses
  Dokument + Sub-Blöcke .1.2–.1.9) + User-Handbuch (Markdown im Bundle) +
  In-App-`/help`-Renderer + Kontext-Hilfe-Anker. Details in
  `docs/RELEASE-1.0-GUIDE.md`.
- **G3 + G4.** Disclaimer-Grep + Release-Durchführung Windows-only.
- **Phase 7 (PV1-A2 + PV1-A5 + PV1-DROP + PV1-RENAME + PV1-DOC, post-v1.0).**
  E-Rechnungs-Empfangs-Politur: ZUGFeRD-Profile-Whitelist (MIN/BASIC-WL als
  „kein gültiger E-Beleg" abweisen), Roh-XML-Viewer auf dem Eingangsbeleg-
  Detail (still gelesen, kein Audit-Spam), Rechnungs-Eingang (intern
  `drop_folder`) mit Polling + App-Start-Sweep (Pipeline-Reuse mit dem
  UI-Import, `processed/`/`failed/`-Routing) plus UI-Sprach-Politur „Drop-
  Folder → Rechnungs-Eingang" (Settings-Card, PageBar, Notification-Labels;
  Code-Identifier englisch). ADR 0037. Migrationen `0028_append_only_
  hardening`, `0029_drop_folder`, `0030_rename_drop_folder_labels`.

Vollständige Block-Definitionen in `PRD-klein-buch.md` §7, ADR-Liste in
`docs/adr/`, v1.0-Plan in `docs/RELEASE-1.0-GUIDE.md`.

---

## 9. Vertiefende Architektur-Dokumente

Diese Datei ist die Top-Level-Übersicht. Tiefere Architektur-Themen leben als
eigene Dateien in `docs/architecture/`, damit weder dieses Dokument zum
Mehrtausend-Zeilen-Monolith aufquillt noch die Themen sich gegenseitig
verdrängen. Stand der einzelnen Dateien (Sub-Blocks von G2-DOC.1, siehe
`docs/RELEASE-1.0-GUIDE.md`):

| Datei | Inhalt | Sub-Block | Status |
|---|---|---|---|
| [`architecture/modules.md`](architecture/modules.md) | Modul-Referenz je Verzeichnis (`domain/`, `einvoice/`, `pdf/`, `archive/`, `db/`, `scheduler/`, `backup/`, `mail/`, `migration_export/`, `notify/`, `dsgvo/`, `factory_reset/`, `commands/`) mit öffentlicher Schnittstelle, Invarianten, Tests pro Modul | G2-DOC.1.2 | **grün** (2026-05-26) |
| [`architecture/data-model.md`](architecture/data-model.md) | Datenmodell-Referenz: alle Tabellen (Spalten, Typen, Constraints), GoBD-Trigger, Schema-Versionen, Migrations-Liste `0001..0030` mit Kurz-Zweck | G2-DOC.1.3 | **grün** (2026-05-27) |
| [`architecture/gobd.md`](architecture/gobd.md) | GoBD-Implementation: wie Lock-Trigger, Archiv-write-once + SHA-256 + Re-Hash, Storno-Pfad, Audit-Log-Append-only, Aufbewahrung und Factory Reset im Code realisiert sind | G2-DOC.1.4 | **grün** (2026-05-26) |
| [`architecture/paragraph-19.md`](architecture/paragraph-19.md) | §19-Logik: UI-Sperren, BT-22-Pfad in der XRechnung, PDF-Klausel-Check, Verzicht/5-Jahres-Bindung, §14c-Schutz | G2-DOC.1.5 | **grün** (2026-05-26) |
| [`architecture/security.md`](architecture/security.md) | Sicherheits-Modell: SQLCipher-Kette (PBKDF2-HMAC-SHA512), Backup-Hülle (Argon2id + AES-256-GCM), Keychain-Topologie, Bootstrap vor Pool-Open, Restore-Marker-Phasen, Factory-Reset-Phasen, Tamper-vs-Waisen-Unterscheidung | G2-DOC.1.6 | **grün** (2026-05-26) |
| [`architecture/build-and-release.md`](architecture/build-and-release.md) | Build- und Release-Prozess (Windows-only ab v1.0): Sidecar-Build (`build-sidecar.ps1`), Tauri-Bundle (NSIS+MSI), CI-Workflows, Tag-Konvention, SmartScreen-Hinweis | G2-DOC.1.7 | **grün** (2026-05-26) |
| [`architecture/adr-index.md`](architecture/adr-index.md) | ADR-Index mit Stand-Tabelle (0001–0037), Querverweise auf die jeweiligen Architektur-Kapitel | G2-DOC.1.8 | **grün** (2026-05-27) |
| [`architecture/conventions.md`](architecture/conventions.md) | Doku-Konventionen: Markdown, deutsche Doku/Konversation, Code/Identifier/Commits englisch, Aufbau pro Modul-Datei, „Letzte Verifikation"-Footer-Datum | G2-DOC.1.9 | **grün** (2026-05-26) |

**Lese-Reihenfolge.** Wer das System zum ersten Mal versteht, liest ARCHITECTURE.md
(diese Datei) komplett, dann gezielt das Vertiefungs-Dokument zum eigenen Thema.
Wer ein konkretes Modul implementiert/ändert, springt direkt in `modules.md`
und das jeweilige ADR.

**Verzeichnis-Wahl Begründung.** Verzeichnis statt One-File-Monolith, weil
das Pattern bei Klein.Buch bereits etabliert ist (`adr/`, `reference/`) und
1) Edits/Diffs klein bleiben, 2) Cross-Reference zu spezifischen Themen
sauber bleibt, 3) Onboarding eines Lesers nicht von einem 3000-Zeilen-File
überfordert wird, 4) die Struktur in einer v1.1/v2 nicht umgeworfen werden
muss, wenn neue Module/Themen dazukommen — eine neue Datei in
`architecture/`, fertig.
