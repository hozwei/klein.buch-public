# Changelog

Folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/) und
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2026.5.1] — Hotfix, veröffentlicht 2026-05-28

### Fixed

- **AfA-Formular crashte sofort beim Öffnen in V2026.5.0** mit
  `Konfiguration: afa-tabellen.json nicht lesbar
  (…\AppData\Local\Klein.Buch\inputs\specs\afa-tabellen.json): Das System
  kann den angegebenen Pfad nicht finden`. Ursache: `inputs/` war in
  `tauri.conf.json` `bundle.resources` nicht aufgeführt, und
  `Paths::inputs_dir` zeigte im Release auf `resource_dir().join("inputs")`,
  also ins Leere. `afa_tabellen::load` hatte — anders als die PDF-Vorlagen —
  keinen eingebetteten Fallback. (Block **R7-INPUTS** + Folge-Fix
  `block-r7-inputs-fix` für den `resources/`-Pfad-Prefix beim Lesen des
  Bundle-Mirrors.)

  Dieselbe Klasse Bug betraf latent `inputs/mail-templates/` (Mail-Versand)
  und `inputs/branding/logo.png` (PDF-Logo); PDF-Rechnungsvorlagen
  überlebten über die eingebetteten `builtin_unified`-Fallbacks.

### Changed

- **`inputs/` ist jetzt vom Bundle entkoppelt:**
  `klein-buch/src-tauri/build.rs` spiegelt
  `klein-buch/inputs/{specs,pdf-templates,mail-templates,branding}` nach
  `src-tauri/resources/inputs/` (gitignored), das landet via
  `bundle.resources` im NSIS-Setup. `Paths::inputs_dir` zeigt in Production
  jetzt auf `app_local_data_dir/inputs/` — der einzige Pfad, in den der
  User aus eigener Kraft schreiben kann (BMF-AfA-Tabellen-Updates, eigene
  PDF-/Mail-Templates). Im Dev-Build (`cfg!(debug_assertions)`) bleibt der
  direkte Repo-Pfad.
- **Neuer First-Run-Copy** (`config::ensure_inputs_seeded`,
  `db::prepare_filesystem`): kopiert beim ersten Start das gebundelte
  `resource_dir/inputs/` rekursiv in `app_local_data_dir/inputs/`,
  **idempotent + ohne Überschreiben** (`inputs/`-Hardline aus
  `CLAUDE.md`: User-Edits an `afa-tabellen.json` oder eigene Vorlagen
  überleben jeden App-Start unverändert). Schema bleibt v30.

### Internal

- Tests `klein-buch/src-tauri/tests/inputs_seed_test.rs` decken
  missing-files, no-overwrite, mixed-gaps, idempotent, nested-subdirs und
  empty-bundle ab.
- `docs/RELEASE-1.0-GUIDE.md` (privat) erhält die Sektion
  „G4-Pre-Tag-Gate — Install-Smoke": vor jedem Phase-Tag muss das
  NSIS-Setup auf einem sauberen Windows-Profil installiert + die
  Hauptseiten (AfA-Formular, Vorlagen, Branding, Mail) einmal getouched
  werden. R1–R6 (v2026.5) hatten diesen Schritt nicht, deshalb ist
  R7-INPUTS bis ins v1.0-GA durchgerutscht.

## [2026.5.0] — v1.0 GA, veröffentlicht 2026-05-27

Erster öffentlicher Release auf
<https://github.com/hozwei/klein.buch-public/releases/tag/v2026.5.0>
(Block G4.5). Inhalt = Stand auf `main` nach G4.3 (Release-Pipeline-
Härtung). Privates Build-Repo `hozwei/klein-buch` bleibt privat
(Build-Journal, Memory, QA-Logs werden bewusst nicht freigegeben);
veröffentlicht wurde aus dem separaten Public-Repo
`hozwei/klein.buch-public` (gleicher Source-Stand, ohne interne
Notes). Tag `v2026.5.0` zuerst lokal auf dem privaten `main` gesetzt
(Block G4.4); die Veröffentlichung in G4.5 erfolgte per
`workflow_dispatch` im public Repo (`release.yml` mit Tag-Input
`v2026.5.0`), Draft mit eigenem Release-Notes-Body (Highlights +
SmartScreen-Hinweis + SHA-256-Anleitung + AGPL-§13-Source-Link) als
„latest release" publiziert. Artefakte:
`klein-buch_2026.5.0_x64-setup.exe` + `SHA256SUMS.txt`. Pre-Publish-
Security-Audit grün (keine Tokens, keine echten Bank-/Steuer-/
Bestandsdaten, keine DB-Snapshots; AGPL-§13-Pflichtdaten im
AboutDialog bewusst öffentlich).

### Changed

- **Release-Bundle nur noch NSIS, MSI verworfen** (ADR 0038, Block G4.3).
  Hintergrund: Windows-Installer `ProductVersion` darf im `MAJOR`-Feld
  maximal 255 sein, CalVer-Jahr `2026` sprengt das. Tauri 2 hat keinen
  offiziellen Override-Hebel; eigene WiX-Templates wären Bürokratie für
  ein Format, das im §19-Kleinunternehmer-Use-Case keinen Mehrwert hat
  (MSI ist primär Active-Directory-/GPO-Massendeployment). NSIS deckt den
  Single-User-Install-Pfad voll ab. `tauri.conf.json::bundle.targets` ist
  jetzt `["nsis"]` statt `"all"`; der Release-Workflow sammelt SHA-256-
  Summen nur über `.exe`.

### Internal (Release-Pipeline-Härtung beim Dry-Run G4.3)

- `.github/workflows/release.yml` `workflow_dispatch`-Default-Tag von
  `v0.1.0` auf `v2026.5.0-rc1` gehoben.
- `src-tauri/about.toml::targets` als String-Liste statt Inline-Table-
  Form geschrieben (cargo-about 0.6 lehnt die `{ triple = "…" }`-Form
  ab).
- `src-tauri/Cargo.toml` mit `publish = false` markiert, damit
  `cargo about generate` die Eigen-Crate aus der Drittanbieter-Lizenz-
  Liste herausfiltert (`private = { ignore = true }` greift). Die
  Eigen-Lizenz `AGPL-3.0-or-later` taucht so korrekt nicht als
  „akzeptierte Drittanbieter-Lizenz" auf.
- `CDLA-Permissive-2.0` (Mozilla CA Root Bundle via `webpki-roots`,
  transitiv über `russh`/`rustls`) in `accepted` aufgenommen.

## [Unreleased] — Post-v1.0 (Phase 7, E-Rechnungs-Politur)

### Added

- Rechnungs-Eingang (intern `drop_folder`) für eingehende E-Rechnungen
  (PV1-DROP, ADR 0037). Ein überwachter Ordner in den Einstellungen;
  abgelegte XRechnung- oder ZUGFeRD-Dateien werden im 5-Minuten-
  Scheduler-Tick (plus App-Start-Sweep und manueller „Jetzt prüfen"-
  Button) eingelesen und über dieselbe Pipeline wie der manuelle
  E-Rechnungs-Import verarbeitet. Erfolgreiche Übernahmen wandern nach
  `processed/YYYY-MM/`, fehlerhafte nach `failed/` (kein Auto-Delete,
  Diagnose-Pfad). Inbox-only-Notifications (`rule_drop_folder_import_ok`
  default off, `rule_drop_folder_import_failed` default on). Polling
  statt `notify`-Crate wegen OneDrive-Sync-Quirks. Migration
  `0029_drop_folder.sql`, Schema `v28 → v29`.
- Roh-XML-Viewer auf der Eingangsbeleg-Detailseite (PV1-A5, ADR 0037).
  Neuer Button „Roh-XML anzeigen", öffnet einen Modal-Dialog mit der
  ursprünglichen XML aus dem Archiv (bei ZUGFeRD via Mustang-Bridge aus
  dem PDF/A-3 extrahiert). Pretty-Print, Copy-Button, SHA-256-Hash und
  Byte-Größe. Liest still über `archive::store::read_and_verify_silent`,
  kein `archive.read`-Audit pro Klick. Tamper-Detection bleibt aktiv.
- Zwei neue Handbuch-Seiten: „Eingangsrechnungen via Ordner" und
  „Probleme mit dem Rechnungs-Eingang". Updates an Kosten erfassen,
  Einstellungen, Glossar und FAQ. ADR `0037-einvoice-receive-polish-
  drop-folder.md`. Tech-Doku (`ARCHITECTURE.md`, `architecture/{modules,
  adr-index,data-model,handbook}.md`) auf Schema v30 + ADRs 0001–0037
  aktualisiert.

### Changed

- E-Rechnungs-Empfang lehnt ZUGFeRD-Profile `MINIMUM` und `BASIC-WL`
  jetzt mit einem klaren Fehler ab (`ParseError::UnsupportedProfile`),
  statt sie indirekt über einen KoSIT-Fail durchzuwinken (PV1-A2,
  ADR 0037 D-73). Akzeptierte Profile: `EN16931`, `EXTENDED`,
  `XRECHNUNG` (Substring-Match, case-insensitive). Beide abgelehnten
  Profile sind seit der E-Rechnungs-Pflicht 2025 keine gültigen
  E-Rechnungen mehr.
- UI-Label „Drop-Folder" → „Rechnungs-Eingang" in der Settings-Card,
  PageBar, Toggle, Eingabe-Hinweisen, Toast-Meldungen, Notification-
  Titeln und den zwei Notification-Regel-Labels (PV1-RENAME, CLAUDE.md-
  UI-Sprachregel + Plain-Language-Hardline aus G2-DOC.2). Button
  „Jetzt synchronisieren" → „Jetzt prüfen". Code-Identifier
  (`drop_folder_*`, Route `/settings/drop-folder/`, Module-Namen,
  Rule-IDs, Migration-Filenames) bleiben englisch nach der CLAUDE.md-
  Hard-Rule „Code/Identifier/Commits englisch". Migration
  `0030_rename_drop_folder_labels.sql`, Schema `v29 → v30`.

## [Unreleased] — Phase 2D (wird `v0.1.0`)

Notifications + prüfungssicherer Geschäftsjahres-Abschluss, OAuth-Versand über
Microsoft Graph, durchgängiges Design-System und wählbare PDF-Vorlagen. Tag
`v0.1.0` folgt nach dem Cross-OS-Release-Workflow (Block 17d).

### block-17c — Doku + ADRs (2026-05-22)

- ADRs `0023`–`0030` ergänzt (Recurring/Scheduler, E-Rechnung-Empfang,
  Anlagen/AfA, EÜR-Export, Notifications/GJ-Lock, OAuth/E-Mail-Protokoll,
  Design-System, wählbare PDF-Vorlagen).
- README auf den vollständigen Feature-Stand gebracht; ARCHITECTURE +
  user-guide + operations-guide nachgezogen.

### block-17b — Konsolidierter Happy-Path-E2E-Test (2026-05-22)

- `tests/happy_path_test.rs`: Stammdaten (§19) → Kontakt → Rechnung → echte
  `run_lock_pipeline` (Mock-Sidecar, inkl. XRechnung + §19-Klausel-Check +
  Typst-Render + ZUGFeRD-Mock + Archiv) → Vollzahlung → Kosten → **EÜR**.
  Cross-Modul-Assertion: Rechnung = Betriebseinnahme, Kosten = Ausgabe,
  Überschuss = Differenz. Läuft ohne MailHog/Java in jedem `cargo test`.
- **Entscheidung:** Rust-Integrationstest statt Playwright (passt nicht zu
  Tauri; offizieller UI-E2E-Weg wäre tauri-driver/WebdriverIO). Siehe TASKS.md.

### block-17a — Wählbare PDF-Vorlagen + Switcher (2026-05-22)

- 3 eingebettete Unified-Templates (`modern`/`klassisch`/`minimal`,
  `pdf::templates`): eine `.typ` rendert Rechnung **oder** Angebot via internem
  Branching, jede mit `// §19-KLAUSEL-BLOCK: REQUIRED` + Klauseltext + Logo.
- Doctype-bewusster Resolver (`resolve_invoice_template`/`resolve_quote_template`)
  mit `inputs/`-Override-Vorrang und `default`-Kollisions-Schutz; `list_templates`
  merged Built-ins + eigene Dateien (`TemplateMeta.builtin`).
- Globale Auswahl via `seller_profile.default_pdf_template` (Sentinel: Beleg-
  `'default'` → globaler Default beim Render; archiviertes PDF = GoBD-Snapshot).
- Switcher `settings/pdf-templates` mit §19-/Built-in-Badges, **Vorschau-PDF**
  je Vorlage (Dummy „Max Mustermann" + geometrisches Dummy-Logo). §19-Schutz:
  nicht-konforme Vorlagen bei Kleinunternehmer gesperrt. Logo-Rendering via
  Typst `World::file()`. Siehe ADR 0030.

### block-ds — Design-System + UI-Konsistenz (2026-05-22)

- Zentrale Design-Tokens (`src/lib/styles/tokens.css`: Petrol `#176b87` +
  SaaS-Look, System-Font-Stack/local-first) + Komponenten `Button`/`Card`/
  `Badge`/`FormField`/`Table`/`PageBar`/`ToastHost`/`ConfirmDialog`/`Banner`.
- Feedback vereinheitlicht: Toast als Standard, `confirmDialog()` statt
  Browser-`confirm()`, Banner für persistente Zustände, Modal für
  folgenreiche Aktionen. Sticky `PageBar` (Zurück links, Aktionen rechts) auf
  allen Seiten; Firmenlogo-Upload. Siehe ADR 0029 (+ ADR 0021).

### block-16b — E-Mail-Versandprotokoll + Suche (2026-05-22)

- Migration `0015_email_log` (append-only, `EXPECTED_SCHEMA_VERSION = 15`):
  protokolliert jeden Versand (Erfolg + Fehler) mit Provider-Antwort (SMTP-Code
  / Graph-`request-id`) + serverseitiger Suche/Filter. Seite
  `settings/mail-log` + Versand-Historie auf Rechnung/Angebot.

### block-16 — OAuth Microsoft Exchange Online (2026-05-22)

- Migration `0014_oauth` (5 nicht-geheime OAuth-Spalten, Schema v14). Versand
  über **Microsoft Graph `/me/sendMail`**, nutzer-eigene Azure-App (Loopback +
  Public-Client + PKCE). **Nur der Refresh-Token** im OS-Keychain, **gechunkt**
  (Windows-2560-Limit). `settings/mail` mit Verbinden/Trennen + Azure-Hilfe.
  Siehe ADR 0028.

### block-15 — Notifications + GJ-Lock + Integritäts-Cron (2026-05-22)

- Migrationen `0012_notifications` + `0013_fiscal_year_locks` (Schema v13).
- In-App-Inbox (Quelle der Wahrheit) + OS-Notifications mit Dedup; 4
  Reminder-Regeln. Prüfungssicherer GJ-Abschluss (§146 AO): AfA buchen → Anlagen
  + Abschreibung sperren → EÜR-Snapshot → Audit → Auto-Backup; nur abgelaufene
  Jahre, Backup-Unlock-Pflicht, unumkehrbar; `guard::ensure_year_open` in den
  Command-Wrappern (Storno bleibt möglich). Auto-AfA am 01.01. (Default an,
  abschaltbar). Monatlicher Archiv-Integritäts-Check. Audit-Trail-Ansicht.
  Siehe ADR 0027.

## [0.1.0-phase2c] - 2026-05-21

Phase 2C — Anlagenverzeichnis + AfA + EÜR + Steuerberater-Export (Blöcke 12–14).

### block-14 — EÜR-Export + Steuerberater-Paket (2026-05-21)

- Keine Migration (Schema v11). ELSTER-Ausfüllhilfe (Anlage-EÜR-Zeilen-Mapping)
  + Typst-PDF „Anlage EÜR" (Anlage EÜR + AVEÜR + Einzelaufstellung). DATEV-
  Buchungsstapel (EXTF, **SKR03-Default**/SKR04, CP1252/CRLF, voll inkl. AfA).
  Steuerberater-Paket als ZIP (Deckblatt + EÜR-PDF + DATEV + Einzel-CSVs +
  Stammdaten). AfA-Safeguard-Banner. Reference-Docs ELSTER + DATEV. Siehe
  ADR 0026. Tag `v0.1.0-phase2c`.

### block-13 — EÜR-Aggregation (Cash-Basis) (2026-05-21)

- Keine Migration (Schema v11). `euer::aggregate` (Functional Core) + Repo +
  Commands + `routes/euer`. Cash-Basis nach §11 EStG / §4 Abs. 3 EStG:
  Einnahmen am Zahlungseingang (bleiben im Zuflussjahr), Storno = negative
  Einnahme zum Storno-Datum, Kosten am Zahlungsausgang, AfA als Jahresgröße,
  Anlagen-Veräußerung, Privatbewegungen EÜR-neutral. Siehe ADR 0022 (korrigiert
  ADR 0010).

### block-12 — Anlagenverzeichnis + AfA (2026-05-21)

- Migrationen `0010_assets` + `0011_depreciation` (Schema v11). AfA-Berechnung
  als Functional Core (`domain::depreciation`), AfA-Tabellen als JSON in
  `inputs/`; linear + GWG-Sofortabschreibung + Computer-Sonderregel +
  Privatanteil + Veräußerung. Idempotenter AfA-Lauf (`UNIQUE(asset_id,
  fiscal_year)`); Festschreibung erst zum GJ-Abschluss. Siehe ADR 0025.

## [0.1.0-phase2b] - 2026-05-21

Phase 2B — Kosten, Wiederkehrendes, E-Rechnung-Empfang (Blöcke 9–11).

### block-11 — E-Rechnung-Empfang (2026-05-21)

- Keine Migration (Schema v9). `einvoice::parser` liest XRechnung (CII + UBL)
  und extrahiert CII aus ZUGFeRD-PDFs. KoSIT-Validierung beim Empfang
  **beratend** (nie blockierend); Import → normale Kostenposition; Original
  write-once archiviert (`ReceivedEinvoice`). `routes/expenses/import`. Siehe
  ADR 0024. Tag `v0.1.0-phase2b`.

### block-10 — Recurring + Scheduler-Foundation (2026-05-20)

- Migration `0009_recurring`. `scheduler::{tick, recurring}`: 5-Minuten-Tick
  nach Bootstrap, Unlock-gated; Auto-Anlage fälliger Belege mit `paid_date NULL`
  + Catch-up für Ausfallzeiten. Siehe ADR 0023.

### block-9 — Kosten + Anhänge + §13b + Privatbewegungen + Payment-Accounts (2026-05-20)

- Migrationen `0007_expenses` + `0008_private_movements`. Ausgaben mit
  Beleg-Upload, §13b-Reverse-Charge, sofort gelockt; §19 wirkt nur auf der
  Ausgangsseite → Kosten gehen **brutto** in die EÜR. Privatentnahmen/-einlagen
  EÜR-neutral. Zahlungskonten (`payment_accounts`). Siehe ADR 0019 + ADR 0020.

## [0.1.0-phase2a] - 2026-05-20

Phase 2A — Angebote (Blöcke 6–8): Angebote anlegen, festschreiben, annehmen
(mit Vertrags-Upload), in Rechnungen konvertieren, als PDF erzeugen und als
**Bundle** (Angebot + AGB + Datenschutz) versenden. Zentral versionierte
Rechtsdokumente mit fester, unveränderlicher Verknüpfung pro Angebot.

### block-8 — Angebote-PDF + Versand + Bundle + Rechtsdokumente (2026-05-20)

- Migration `0006_legal_documents.sql`, `EXPECTED_SCHEMA_VERSION = 6`:
  - `legal_documents` (versionierte AGB/Datenschutz-PDFs: `doc_type`, `version`
    monoton pro Typ, `title`, `archive_entry_id`, `is_active`). Partial-unique
    `uq_legal_documents_active` = höchstens eine aktive Version pro Typ. Trigger
    `no_delete` (append-only) + `immutable` (nur Aktiv-Status änderbar).
  - `quote_legal_documents` (append-only Bindung Angebot↔ausgegebene Version,
    `version`-Snapshot, unique pro (Angebot, doc_type), no-delete + immutable).
- `ArchiveKind::LegalDocument`; `db::repo::legal_documents`
  (create_version/activate/deactivate/get/get_active/list +
  `bind_active_for_quote` idempotent + `list_for_quote`);
  `db::repo::quotes::set_pdf_archive_id`.
- `pdf::typst_render::render_quote` + `build_quote_data_json`; Refactor
  `compile_pdf` (Rechnung = PDF/A-3b für Mustang, **Angebot = Plain-PDF**, keine
  E-Rechnung). `pdf::templates::{DEFAULT_QUOTE_TEMPLATE, load_quote_source}` —
  Quote-Template **eingebettet** (mit §19-Marker, durchläuft `klausel_check`),
  Override via `inputs/pdf-templates/quote.typ`, weil `inputs/` für Maschinen
  tabu ist.
- `pdf::bundle::merge_pdfs` — **neue Dependency `lopdf 0.40`**; führt Angebots-PDF
  + AGB + Datenschutz zu einem Druck-PDF zusammen (Merge-Rezept gegen das
  offizielle `lopdf/examples/merge.rs` verifiziert). Bundle wird nicht
  archiviert (abgeleitet); kanonisch sind Einzel-PDFs + Bindung.
- `mail::templates`: Angebots-Mail (`QuoteMailContext`, eingebettetes
  `DEFAULT_QUOTE_MAIL`, `load_quote_template`, `render_quote_mail`).
  `commands::mail::{mail_quote_preview, mail_send_quote, send_quote_core}` —
  Bundle als **Multi-Attachment** (3 Dateien), Audit `quote.sent` mit
  gebundenen Versionen, kein Status-Wechsel (Angebot ist ab Festschreiben `sent`).
- `commands::legal_documents::*` (Upload/List/Activate/Deactivate),
  `commands::quotes::{ensure_quote_pdf, bind_legal_docs_for_quote,
  prepare_quote_dispatch, quotes_generate_pdf, quotes_open_bundle,
  quotes_legal_bindings}`.
- Frontend: `settings/legal` (Upload + Versionen + Aktivieren), Angebots-Detail
  (PDF anzeigen / Bundle drucken / Versenden + gebundene Rechtsdokumente),
  `quotes/[id]/send` (mit Legal-Docs-Pflichtprüfung), `api.ts`/`types.ts`.
- **Design-Entscheidungen:** Legal-Docs als PDF-Upload pro Version (kein
  In-App-Editor); Druck = ein zusammengeführtes PDF, Mail = 3 Anhänge; Bindung
  bei Bundle-/Versand-Erzeugung (idempotent, append-only) + **Pflicht für
  Versand** (aktive AGB + Datenschutz). Siehe ADR 0018.
- Tests: `tests/legal_documents_repo_test.rs` (Versionierung, single-active,
  Immutability/No-Delete-Trigger, idempotente Bindung) + Unit-Tests
  `pdf::bundle`, `pdf::templates`, `pdf::typst_render` (render_quote),
  `mail::templates` (Quote-Mail).
- Juristik-Caveat: AGB-/Datenschutz-Texte vor Echtbetrieb anwaltlich prüfen.

### block-7 — Angebot → Rechnung-Konvertierung (2026-05-20)

- **Keine Migration** (nutzt `derived_from_quote_id` + `converted_*` aus
  `0005_quotes`; Schema bleibt v5 bis Block 8). `domain::quote::convert_to_invoice`
  (pure, 1:1-Item-Mapping inkl. §19-Carry-over). `db::repo::quotes::mark_converted`
  (Guard `accepted → converted`). Gemeinsamer Draft-Helper
  `commands::invoices::create_invoice_draft_from_input` (von Neuanlage UND
  Konvertierung genutzt, setzt `derived_from_quote_id`).
- **Hard-Rule:** Konvertierung nur aus `status='accepted'`. Die Konvertierung
  erzeugt eine Rechnungs-**Draft**, die über die normale Lock-Pipeline
  festgeschrieben wird (kein Issue-Duplikat). Siehe ADR 0017.
- Frontend `routes/quotes/[id]/convert` (Positionen vorbefüllt + anpassbar).

### block-6 — Angebote: Schema + CRUD + Annahme-Workflow (2026-05-20)

- Migration `0005_quotes.sql` (`quotes` + `quote_items` + `trg_quotes_immutable`
  + `invoices.derived_from_quote_id`), `EXPECTED_SCHEMA_VERSION = 5`;
  `seller_tax_number` nullbar (konsistent mit Rechnung/Seller, §33-Compat).
- Eigener Belegkreis `AN-{YYYY}-{NNNN}`. Lifecycle
  `draft → sent → accepted|rejected → converted | canceled`. `domain::quote`
  (Functional Core: `compute_totals` (reuse Invoice-Totals), `validate_quote` mit
  §19-`assert_no_vat`). `db::repo::quotes` (CRUD + State-Transitions mit
  Status-Guards) + `db::repo::attachments` (generische Eltern-Verknüpfung
  archivierter Dateien).
- **Festschreiben = Lock → `sent`** (kein eigener `issued`-Status für Angebote);
  ab Lock greift `trg_quotes_immutable`. Annahme archiviert optional den
  **unterschriebenen Vertrag** write-once als Attachment. Siehe ADR 0016.
- Frontend `routes/quotes/*`; `attachments_open`/`_reveal`-Commands.

## [0.1.0-phase1] - 2026-05-20

Erster produktiv nutzbarer Stand (Walking Skeleton, Blöcke 0–5): Kontakte,
Rechnungen mit ZUGFeRD-PDF/A-3, SMTP-Versand, Storno, GoBD-Archiv, verschlüsseltes
Backup/Restore, Migrations-Export.

### block-5 — SMTP-Versand + Phase-1-Polish (2026-05-20)

- **Keine Migration** (`mail_accounts` bereits aus `0001_init.sql`);
  `EXPECTED_SCHEMA_VERSION` bleibt 4.
- `mail::keyring`: SMTP-Postfach-**Passwort** im OS-Keychain, Service-ID-Schema
  `kleinbuch::mail::{account_id}`. **`keyring`-Crate-Features** ergänzt
  (`apple-native`/`windows-native`/`sync-secret-service`/`crypto-rust`) — ohne sie
  fiele keyring 3 auf einen flüchtigen Mock-Store zurück (Passwort nach Neustart
  weg). Passwort nie in DB/Logs/Audit. (Nicht zu verwechseln mit der
  Backup-Passphrase — die lebt nur im Session-Memory, nicht im Keychain.)
- `mail::smtp`: lettre (async, rustls), **Multi-Attachment von Anfang an**
  (Grundlage Angebots-Bundle Block 8), TLS/STARTTLS/Klartext je nach Konfiguration,
  `test_connection`. Anhanglose Mails werden als reine `text/plain` gebaut (kein
  `multipart/mixed`-mit-einem-Teil); mit Anhang `multipart/mixed`. Message-Building
  ist pure und unit-getestet. `tracing`-Logs
  für Verbindung + Versand (Host/Port/TLS/User/Empfänger/Ergebnis) — **nie das
  Passwort**; lettres rohes `tracing`-Feature bewusst aus (würde AUTH mitloggen).
- `mail::templates`: Tera-Render von `inputs/mail-templates/invoice-de.txt`
  (Subject/Body-Split, deutsche Betrags-/Datumsformatierung).
- `db::repo::mail_accounts` (CRUD-Subset + `touch_last_used`) +
  `invoices::mark_sent` (`status='issued'→'sent'`, `sent_at`; GoBD-konform, da nicht
  in der Immutability-Whitelist).
- `commands::mail`: `mail_accounts_list`, `mail_account_create`/`_update`/`_delete`
  (Passwort → Keychain; Delete entfernt auch den Keychain-Eintrag),
  `mail_account_test_connection`, `mail_send_test` (Test-Mail über ein
  gespeichertes Konto), `mail_invoice_preview`, `mail_send_invoice`,
  `mail_send_quote` (Guard bis Block 8). `send_invoice_core` liest das ZUGFeRD-PDF
  aus dem Archiv **mit SHA-256-Verify**, rendert den Body, versendet, setzt `sent`,
  schreibt Audit `invoice.sent`. Neue `Error::Mail`-Variante.
- Frontend: `routes/settings/mail` (Konten-Liste mit **Bearbeiten/Löschen** +
  Formular + Test-Connection + **Test-Mail senden** an beliebigen Empfänger),
  `routes/invoices/[id]/send`
  (Konto-Auswahl, Empfänger-Default aus Buyer-Snapshot/Kontakt, Body-Vorschau,
  ZUGFeRD-Auto-Anhang), „Senden"-Button auf der Rechnungs-Detailseite.
- E2E: `tests/e2e_test.rs` (Seller → Kontakt → Rechnung → Versand gegen MailHog,
  Assertions über die MailHog-API + DB-Status + Audit). MailHog als CI-Service in
  `ci.yml`; lokal ohne MailHog überspringt sich der Test (no-op).
- Doku: ARCHITECTURE.md ausgebaut, ADRs `0002`–`0015` (Decision-Log §9),
  `docs/user-guide.md` (§1–3), `docs/operations-guide.md` (Stub).

### block-4 — Backup + Restore + Migrations-Export (2026-05-20)

- Keine Migration nötig (`backup_history` bereits aus `0001_init.sql`);
  `EXPECTED_SCHEMA_VERSION` bleibt 4. Backup-Ziel + Passphrase-Verifier liegen
  als `app_settings`-Keys (kein Schema-Impact).
- `backup::manifest` + `backup::encrypt`: Argon2id (m=64 MB, t=3, p=4) +
  AES-256-GCM, frische Salt/Nonce pro Backup, eigener Hex-Codec (kein neues
  Cargo-Dep). Datei-Format: Plain-Header (MAGIC + Manifest-JSON) + verschlüsselter
  Body.
- `backup::snapshot`: konsistenter DB-Snapshot via `PRAGMA wal_checkpoint(TRUNCATE)`
  + Datei-Lesen (statt `VACUUM INTO`); ZIP aus DB + `archive/` + `inputs/branding/`.
- `backup::target` (konfigurierbares Ziel, OneDrive-tauglich) + `backup::rotation`
  (GVS: 30 daily / 12 monthly / 7 yearly; `manual`/`pre_restore` nie geprunt).
- Passphrase lebt nur im Session-Memory (`BackupSession`, Tauri-State) — nie in
  DB/Logs/Audit. Setup/Unlock über verschlüsselten Verifier (kein Klartext/Hash).
- `backup::create_now`-Pipeline (Snapshot → Encrypt → Manifest → Ziel →
  `backup_history` → Rotation). Auto-Critical-Backup nach Invoice-Lock und Storno
  (best-effort), Auto-Daily nach Unlock (letztes Backup > 24 h).
- Restore zweiphasig: Phase A (Pre-Restore-Backup-Pflicht + Decrypt + Hash-Verify
  + Staging + Marker), Phase B (DB-/Archive-Swap beim App-Start vor Pool-Open
  wegen Windows-File-Lock). `inputs/` bleibt beim Restore tabu (nur DB + Archive).
- `migration_export`: offenes ZIP (JSON pro Tabelle, `archive/`, Schema-SQL aus
  MIGRATOR, `erd.md`, `manifest.json`, `read_export.py`-Standalone-Reader).
- Tauri-Commands `backup_*` + `migration_export_run`, in `lib.rs` registriert.
- Frontend: `BackupGate` (Onboarding erzwingt Passphrase, Unlock beim Start),
  `routes/settings/backup` (Status/Ziel/manuell/Verlauf/Restore-Wizard mit
  GoBD-Warndialog), `routes/settings/migration-export`.
- Tests: Unit-Tests je Modul + `tests/backup_restore_test.rs` (Backup → Decrypt →
  Restore, Tamper-Detection, Export-Roundtrip).

### block-3 — Rechnung erstellen: Issue + Storno (3a–3d, 2026-05-19/20)

- Migrationen `0003_invoices_seller_tax_number_optional.sql` (§33-Kleinbetrags-
  rechnung) und `0004_invoices_buyer_snapshot.sql` (Empfänger-Snapshot auf
  Rechnungen für spätere DSGVO-Anonymisierung). `EXPECTED_SCHEMA_VERSION = 4`.
- `domain::{invoice,kleinunternehmer,kleinbetragsrechnung,storno,numbering}`:
  Functional Core mit `validate_for_issue` (§14 + §19 + §33), Cent-basierte
  kaufmännische Rundung, `assert_no_vat` §14c-Schutz.
- `einvoice::generator`: XRechnung als **UN/CEFACT CII** (ZUGFeRD/Mustang bettet
  ausschließlich CII ein), §19-Klausel als BT-22-Note + BT-120 ExemptionReason,
  Storno mit Type-Code 384 + BillingReference.
- `einvoice::validator` (KoSIT-Bridge) + `einvoice::mustang_bridge`
  (ZUGFeRD-PDF/A-3), beide mit Mock-Mode für Tests.
- `pdf::typst_render` (PDF/A-3b) + `pdf::klausel_check` (strenger Pre-Render-
  §19-Marker-Check).
- `archive::{store,integrity_check,audit}`: write-once, SHA-256, Tamper-Detection,
  read-only. `db::numbering`: lückenlose, parallel-sichere Nummernvergabe.
- `commands::invoices` mit `run_lock_pipeline` (validate → CII → KoSIT → klausel →
  typst → mustang → archive PDF+XML → DB-Lock → audit → backup-marker).
- Frontend: `routes/invoices/` (Liste/Detail/New/Cancel/Payment) mit
  §19-Hardline-Sperre, Live-Totals, Cash-Basis-Zahlungserfassung.
- Real auf Windows verifiziert: `RE-2026-0001.pdf` als ZUGFeRD-PDF/A-3 mit
  eingebettetem CII, KoSIT nicht-Failed, `verapdf` grün.

### block-2 — Kontakte + Seller Profile (2026-05-19)

- Migration `0002_seller_tax_number_optional.sql`: `seller_profile.tax_number`
  ist jetzt NULL-bar (Tabellen-Rebuild, weil SQLite NOT NULL nicht via ALTER
  entfernt). `EXPECTED_SCHEMA_VERSION = 2`. Onboarding ohne erteilte
  Steuernummer ist damit möglich; §14-UStG-Check wandert zu Block 3.

- `domain::contact`: Validierung (Name + Adresse Pflicht, deutsche PLZ-Form,
  DE-/EU-USt-IdNr.-Format, pragmatischer E-Mail-Check, IBAN-Längen-Check) als
  pure Funktion mit `ContactInput`/`ContactType` + `ValidationError`-Enum.
- `domain::kleinunternehmer`: `HINWEIS_TEXT` als wortgleicher §19-Pflichthinweis,
  `is_active()`, `must_show_hinweis()`, `waiver_deadline()` (01.01. des 6.
  Folgejahres nach §19 Abs. 2 UStG). `assert_no_vat()` bleibt für Block 3.
- `db::models`: `ContactRow` + `SellerProfileRow` als sqlx-`FromRow`,
  serde-camelCase für Tauri-Bridge.
- `db::repo::contacts`: CRUD + Suche + Archive/Unarchive. UUIDv7 als PK,
  kein DELETE.
- `db::repo::seller_profile`: Singleton get/upsert. §19→Regelbesteuerung-
  Wechsel setzt `waived_paragraph_19_since = today` + Audit-Log; verlangt
  explizite Bestätigung über `confirm_waive_paragraph_19`. Rückkehr zu §19
  innerhalb der 5-Jahres-Bindung wird im Backend abgelehnt.
- `db::repo::audit_log`: append-only Helper (`append`, `recent`).
- Tauri-Commands: `contacts_{list,get,create,update,archive,unarchive,search}`,
  `seller_profile_{get,upsert}`, `paragraph_19_info` — registriert in
  `lib.rs`.
- Frontend: TS-Types + typed API-Wrapper. Routes `/contacts` (Liste + Suche +
  Archive-Toggle), `/contacts/new`, `/contacts/[id]`, `/settings/seller`
  mit §19-Toggle + Modal-Dialog "Ich verstehe die 5-Jahres-Bindung".
- Integration-Tests: `tests/contacts_repo_test.rs` (CRUD + Suche + Archive
  + Validierung), `tests/seller_profile_test.rs` (Singleton + §19-Toggle +
  Audit-Log + 5-Jahres-Bindung).

### block-1 — Foundation + Phase-1-Schema (2026-05-19)

- Tauri-2 + Svelte-5 (TypeScript) + pnpm-Scaffold.
- Phase-1-DB-Schema: `app_settings`, `contacts`, `seller_profile`,
  `mail_accounts`, `invoices`, `invoice_items`, `archive_entries`,
  `attachments`, `audit_log`, `doc_number_counters`, `backup_history`,
  `archive_integrity_checks` — alle `STRICT`, Indizes auf allen Datums-
  und Foreign-Key-Spalten.
- GoBD-Triggers: `audit_log` append-only, `invoices` mit Lock-Immutability
  auf Kernfeldern, `archive_entries` mit Hash/Path/Size-Immutability.
- Schema-Version-Check beim App-Start (`EXPECTED_SCHEMA_VERSION = 1`),
  App startet nicht bei Mismatch.
- Module-Stubs für alle Phase-1- und Phase-2-Backend-Module
  (commands, db, domain, einvoice, pdf, archive, mail, backup,
  migration_export, scheduler, notify, euer, assets, depreciation,
  fiscal_year).
- Frontend-Routes: Top-Level pro Bereich + alle Settings-Sub-Routes.
- Java-Sidecar-Bundle (KoSIT 1.6.2 + XRechnung-Konfig 2026-01-31 +
  Mustang 2.23.0) im jlink-Bundle unter
  `src-tauri/binaries/klein-buch-java-x86_64-pc-windows-msvc/`,
  als `bundle.resources` registriert.
- AGPL-3.0-Header in `LICENSE`, README + CHANGELOG.
- GitHub-Actions-CI: cargo fmt + clippy + test + pnpm lint + tsc.
- Memory-Files in `~/cowork/Buchhaltung/memory/klein-buch/`.
- `inputs/`-Initial-Files: Spec-Stubs, `afa-tabellen.json` (BMF),
  `default.typ` mit §19-Klausel-Block-Marker, `invoice-de.txt` Tera-Template.

### Bekannte Einschränkungen

- Cross-OS-Bundles (macOS x86_64/aarch64, Linux x86_64) noch nicht gebaut.
  Folgen in Block 17 via GitHub-Actions-Matrix.
- Detail-Sub-Routes (`[id]/`, `new/`, `cancel/`, `send/`, `payment/`,
  `convert/`, `dispose/`, `import/`) werden in den jeweiligen Blöcken
  angelegt, in denen sie funktional benötigt werden — nicht alle in Block 1.

## Geplant

- Phase 2B (Blöcke 9–11): Kosten + Anhänge + §13b + Privatbewegungen +
  Payment-Accounts, Recurring/Scheduler, E-Rechnung-Empfang → **Tag `v0.1.0-phase2b`**.
- Phase 2C (12–14): Anlagenverzeichnis + AfA + EÜR + Steuerberater-Export.
- Phase 2D (15–17): Notifications, OAuth (Exchange Online), PDF-Templates + E2E +
  Release `v0.1.0`.
- Phase 3 (18–19): DSGVO-Auskunft + -Anonymisierung.
