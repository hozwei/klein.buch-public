# Klein.Buch — Architektur (Stand Lauf 2026.11, Server-Betrieb)

Klein.Buch ist eine selbst gehostete EÜR-Buchhaltung für deutsche
§19-Kleinunternehmer: ein Docker-Image, ein Nutzer pro Instanz, bedient im
Browser, aus dem Internet erreichbar (TLS über den Reverse-Proxy des
Betreibers). Dieses Dokument ist die konsolidierte Architektur-Referenz:
Schichten, Module, Start- und Entsperr-Reihenfolge, Daten- und Kontrollflüsse,
Sidecar-Bridge, Scheduler-Modell und die nicht verhandelbaren Hard-Rules.

**Implementierungs-Stand:** Phasen 1–2D (Blöcke 0–17) + Phase 3 (Pakete/Anfahrt,
ADR 0031) + DSGVO Auskunft/Anonymisierung (ADR 0032) + Phase 4 (Abo-Rechnungen,
ADR 0033) + G1 Security/Backup-Härtung (ADRs 0034–0036) + Phase 7
(E-Rechnungs-Empfang, ADR 0037) + **Phase W** (Web-/Docker-Betrieb, ADR 0039) + **Lauf E/F** (Entscheidungen,
Funktion und Technik vor dem UI/UX-Lauf; ADR 0040) + **Lauf 2026.11**
(Oberfläche für Smartphone, Tablet und Desktop; ADR 0041, §2g):
Tauri und die Desktop-Fassung sind entfallen; ein HTTP-Server
`klein-buch-server` (axum) liefert API und SPA aus.

**Schema-Stand v32** (Migrationen 0001–0032 forward-only; 0031 = Tabelle
`secrets` für Zugangsdaten, 0032 = GoBD-Absicherungen aus Lauf F). **ADRs
0001–0041** in `docs/adr/`; ADR 0039 löst ADR 0002 (Tauri) und ADR 0038
(Desktop-Release-Bundle) ab und ändert zwölf weitere per Amendment; ADR 0040 regelt
Dauerbetrieb, Start- und Zeitlogik (Lauf F); ADR 0041 legt die UI/UX-Standards fest
(Lauf 2026.11, in Arbeit bis U7).
Betrieb: `docs/operations-guide.md`; Build und Release:
`docs/architecture/build-and-release.md`.

---

## 1. Schichten

```
┌──────────────────────────────────────────────────────────────────────┐
│  Browser — SPA (SvelteKit adapter-static, Svelte 5 Runes)            │
│  src/lib/transport.ts: fetch, Pflicht-Header, Idempotenz, Jobs       │
└───────────────────────────┬──────────────────────────────────────────┘
                            │ HTTPS (Reverse-Proxy des Betreibers) → HTTP :8080
┌───────────────────────────▼──────────────────────────────────────────┐
│  HTTP-Server — src-tauri/src/server/ (axum + tower-http)             │
│  Policy (Host/Origin, Proxys), Header, Sitzungen, TOTP, Rate-Limit,  │
│  Allowlist POST /api/cmd/{name}, Auth, Uploads, Downloads, Jobs,     │
│  /healthz, /legal/, SPA                                              │
└───────────────────────────┬──────────────────────────────────────────┘
                            │ Kern(ctx: &Arc<AppCtx>, Args) → Result<T>
┌───────────────────────────▼──────────────────────────────────────────┐
│  Command-Kerne — src-tauri/src/commands/* (Imperative Shell, dünn)   │
│  Registry: Route (Cmd/Replaced/Dropped/Blocked), auth, cost, limit   │
└───────────────────────────┬──────────────────────────────────────────┘
              ┌─────────────┴──────────────┐
   ┌──────────▼─────────┐  ┌───────────────▼──────────────────────────┐
   │  Functional Core   │  │  Imperative Shell (I/O)                  │
   │  (pure, testbar)   │  │  - db (sqlx + SQLCipher, repo/*)         │
   │  - domain::invoice │  │  - archive (write-once + SHA-256)        │
   │  - domain::contact │  │  - secrets (Zugangsdaten in der DB)      │
   │  - kleinunternehmer│  │  - mail (smtp, templates)                │
   │  - storno          │  │  - pdf::typst_render                     │
   │  - numbering       │  │  - einvoice::validator (KoSIT)           │
   │  - einvoice::gen   │  │  - einvoice::mustang_bridge              │
   │  - pdf::klausel    │  │  - backup (target/sftp/snapshot/         │
   │  - euer::aggregate │  │      rotation/encrypt/restore)           │
   │  - domain::package │  │  - uploads, downloads, seed              │
   │  - domain::travel  │  │  - migration_export, scheduler, notify   │
   │  - domain::dsgvo   │  │  - fiscal_year, assets, depreciation     │
   │  - domain::anon.   │  │                                          │
   │  - recurring_inv.  │  │                                          │
   └────────────────────┘  └───────────────┬──────────────────────────┘
                                           │ /bin/sh Launcher, Args, Temp-Dateien
                              ┌────────────▼──────────────┐
                              │  Java-Sidecar (jlink-JRE)  │
                              │  KoSIT-Validator + Mustang │
                              └───────────────────────────┘

  ┌────────────────────────────────────────────────────────────────────┐
  │ Volumes                                                            │
  │  /data    SQLCipher-DB, archive/ (0o400, SHA-256), backups/ (Floor),│
  │           branding/, auth/ (TOTP, Gerätecookie-Schlüssel), tmp/    │
  │  /inputs  Vorlagen, AfA-Tabelle (vom Menschen gepflegt, Seed)      │
  │  /backup-targets/<Name>  Off-Site-Ordner; dazu SFTP-Ziele          │
  └────────────────────────────────────────────────────────────────────┘
```

**Functional Core / Imperative Shell** (ADR 0004). Validierung, Totals-
Berechnung, XML-Generierung, Klausel-Check, AfA-Berechnung, EÜR-Aggregation,
Markdown→Typst-AST und Anonymize-Regeln sind pure Funktionen ohne I/O. Alles,
was Dateien, DB, Netzwerk oder den Sidecar berührt, lebt in der Schale. Tests
des Cores laufen ohne DB/Sidecar/Filesystem.

**Kerne statt Tauri-Commands** (ADR 0039 Nr. 19–21). Jeder Command ist ein Kern
`async fn name(ctx: &Arc<AppCtx>, args: NameArgs) -> Result<T>` im Modul
`commands::`; die Args-Structs übernehmen die camelCase-Schlüssel des
Frontends. `AppCtx` hält Pfade, den schließbaren Pool samt Passphrase
(`BackupSession`), Inbox, Upload- und Download-Ablage, Scheduler-Gate und die
Ablage eines angewendeten Restores. `commands::registry` ist die Allowlist für
`POST /api/cmd/{name}` und listet jeden Kern mit Auth-Klasse
(`public`/`session`/`reauth`/`conditional`; bedingte Re-Auth prüft der Kern,
Lauf F4a), Art (`Read`/`Write`/`Job`), Kostenklasse und Körperlimit. Die bis
Lauf F6 nur gesperrt mitgeführten Desktop-Commands (Dateimanager-Funktionen,
Lizenzpfad, MS-Graph, Vorgänger der Auth-Routen) sind samt Kern entfernt; Desktop-
Nebenwirkungen (`DesktopActions`) kennt `AppCtx` nicht mehr (ADR 0039 Nr. 138).

**Verschlüsselung at Rest** (ADR 0035). Die SQLite-Datei ist
SQLCipher-verschlüsselt; der Schlüssel wird per PBKDF2-HMAC-SHA512 (Salt im
DB-Header) aus der Passphrase abgeleitet. Die Passphrase ist zugleich
DB-Schlüssel und Wurzel der Backup-Hülle (Argon2id). Ohne Passphrase kein
Pool-Open; nach jedem Neustart ist die Instanz gesperrt, bis der Nutzer im
Browser entsperrt (E3). Die Anmeldung verlangt zusätzlich TOTP (E5).

---

## 2. Module

### 2a. Phase 1 (Rechnungen, Backup, Mail)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `domain::invoice` | Core | `validate_for_issue` (§14/§19/§33, Telefon und Fälligkeitsdatum für XRechnung), `compute_totals` (Cent, kaufm. Rundung), §14c-Schutz |
| `domain::contact` | Core | Kontakt-Validierung |
| `domain::kleinunternehmer` | Core | §19-Hinweistext (wortgleich), 5-Jahres-Bindung |
| `domain::storno` | Core | Storno-Beleg aus Original ableiten (Mengen negiert) |
| `domain::numbering` | Core | Doc-Number-Format `{TYP}-{YYYY}-{NNNN}` (ADR 0012) |
| `einvoice::generator` | Core | XRechnung als UN/CEFACT **CII** (ADR 0007), BT-22/BT-120 §19, BG-6/BT-29 |
| `pdf::klausel_check` | Core | strenger §19-Marker-Check vor Render (Rechnung + Angebot) |
| `db` + `db::repo::*` | Shell | sqlx/SQLite via SQLCipher, STRICT, Numbering, GoBD-Trigger, `begin_immediate` |
| `archive` | Shell | write-once, SHA-256, `0o400`, Re-Hash bei jedem Lesen |
| `einvoice::validator` | Shell | KoSIT-Bridge (Sidecar), Mock nur in Debug-Builds |
| `einvoice::mustang_bridge` | Shell | ZUGFeRD-PDF/A-3 (Sidecar), Mock nur in Debug-Builds |
| `pdf::typst_render` | Shell | Typst-`World`; `render_invoice` (PDF/A-3b) + `render_quote` (Plain-PDF) |
| `mail::smtp` | Shell | lettre async, TLS/STARTTLS, eigene CA, EHLO-Name (ADR 0011 + Amendment) |
| `mail::templates` | Shell | Tera-Render Subject/Body aus `inputs/mail-templates/` |
| `secrets` | Shell | Zugangsdaten (SMTP, SFTP) in der Tabelle `secrets` der verschlüsselten DB (Migration 0031, E6) |
| `backup::{target,snapshot,manifest,rotation,encrypt,restore,sftp}` | Shell | Argon2id + AES-256-GCM, Floor + mehrere Off-Site-Ziele, SFTP, Restore-Phasen, Neuverschlüsseln beim Passphrase-Wechsel (ADR 0009/0034/0035) |
| `migration_export` | Shell | offenes ZIP für Steuerberater/Migration (ohne `secrets`) |
| `commands::*` | Shell | Command-Kerne pro Domäne + `registry` |

### 2b. Phase 2A–2D (Angebote, Kosten, Recurring, E-Rechnung-Empfang, Anlagen/AfA/EÜR, Hinweise, Design, PDF-Vorlagen)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `domain::quote` | Core | Angebots-Validierung/Totals (reuse Invoice-Totals), §19-Schutz, `convert_to_invoice` (ADR 0016/0017) |
| `domain::expense` | Core | Kosten-Validierung, §13b-Reverse-Charge, brutto-in-EÜR (ADR 0019) |
| `domain::depreciation` | Core | AfA-Berechnung (linear/GWG/Computer/Privatanteil), idempotent (ADR 0025) |
| `domain::fiscal_year` | Core | GJ-Grenzen, Cash-Basis-Periodisierung; GJ immer aus dem Belegdatum (Nr. 89) |
| `euer::aggregate` | Core | EÜR-Aggregation Cash-Basis §11 EStG (ADR 0022) |
| `euer::{elster_csv, datev_csv, detail}` | Core | ELSTER-Ausfüllhilfe (Kennzahl + Zeile je Vordruck), DATEV-EXTF (SKR03/04), Einzelaufstellungen; das Steuerberater-Paket baut `commands::euer` (ADR 0026, ADR 0039 Nr. 135) |
| `einvoice::parser` | Core | XRechnung CII+UBL + ZUGFeRD-Extract (Empfang, ADR 0024), Profil-Whitelist (ADR 0037) |
| `pdf::bundle` | Core | PDF-Merge (lopdf) für das Angebots-Druck-Bundle (ADR 0018) |
| `pdf::templates` | Core+Shell | Built-in-Vorlagen + doctype-bewusster Resolver + `list_templates` (ADR 0030) |
| `notify::{store,rules}` | Shell | In-App-Inbox (Quelle der Wahrheit und einziger Kanal), Regeln, Dedup (ADR 0027); OS-Benachrichtigungen gibt es im Server nicht |
| `fiscal_year::{lock,transition,guard}` | Shell | prüfungssicherer GJ-Abschluss + `ensure_year_open`-Guard (ADR 0027) |
| `scheduler::{tick,recurring,reminders,integrity_check_cron,depreciation_year_close,recurring_invoice,update_check}` | Shell | 5-Min-Tick, nur entsperrt, Catch-up (ADR 0023/0027, §5) |
| `assets::afa_tabellen` | Shell | AfA-Tabellen-Laden aus `inputs/afa-tabellen.json` |
| `db::repo::{expenses,private_movements,payment_accounts,recurring,assets,depreciation,euer,email_log,app_settings,legal_documents,attachments,quotes}` | Shell | Repos der Phase-2-Domänen, AGB/Datenschutz-Versionierung (ADR 0018) |
| `pdf::typst_render::render_euer` | Shell | „Anlage EÜR"-PDF + Deckblatt (ADR 0026) |

### 2c. Phase 3–4 + DSGVO

| Modul | Schicht | Verantwortung |
|---|---|---|
| `domain::package` | Core | Markdown-Subset-Parser + AST→Typst (Injection-Schutz), Plaintext-Flatten für BT-154, Revisions-Validierung (ADR 0031) |
| `domain::travel` | Core | Anfahrt km × Satz → Beleg-Position, deutsches Format, kein Geocoding (ADR 0031) |
| `domain::dsgvo` | Core | Art.-15-Report-Aufbau (read-only; interne Notizen ausgelassen) (ADR 0032) |
| `domain::anonymize` | Core | Art.-17-Regel (`Anonymisiert #<hex>`, PII → NULL) (ADR 0032) |
| `domain::recurring_invoice` | Core | Abo-Vorlagen-Validierung + `AutoMode` (draft/issue/issue_send) (ADR 0033) |
| `db::repo::packages` | Shell | Paket-Revisionen append-only (`update_as_new_revision`, `rollback`), Provenienz-Snapshot (ADR 0031) |
| `db::repo::dsgvo` | Shell | Daten-Sammlung je Kontakt (`gather`) für Auskunft/Anonymisierung (ADR 0032) |
| `db::repo::recurring_invoice` | Shell | Abo-Vorlagen-CRUD (Stammdaten), `advance` per Compare-and-Swap (ADR 0033) |
| `scheduler::recurring_invoice` | Shell | Abo-Rechnungen am Stichtag materialisieren (`process_due`/`catch_up`/`run_now`), Sperre je Vorlage (ADR 0033) |
| `commands::{packages,dsgvo,recurring_invoice}` | Shell | Kerne; `recurring_invoice` erzwingt §19 serverseitig (KB-0053) |
| `pdf::typst_render::render_package_catalog` | Shell | Paket-Broschüre (kein §14-Beleg, kein Archiv) (ADR 0031) |

### 2d. G1 — Security/Backup-Härtung

| Modul | Schicht | Verantwortung |
|---|---|---|
| `db::open_pool(path, key)` | Shell | SQLCipher-Pool-Open mit `PRAGMA key`, ohne Statement-Logging (ADR 0035) |
| `db::rekey_db_file` | Shell | Neuverschlüsseln mit neuer Passphrase: `ATTACH … KEY` + `sqlcipher_export` in eine Temp-Datei, Probe-Read, atomarer Tausch (gemeinsamer Kern `reencrypt_db_file`; Passphrase-Wechsel, W6) |
| `backup::target` | Shell | Floor + Off-Site: Ordner unter `/backup-targets` (`canonicalize`) und SFTP, höchstens 10 Ziele, Status `offsite` (ADR 0034 + Amendment, Nr. 77) |
| `backup::sftp` | Shell | `russh`/`russh-sftp`, Host-Key-Pinning (SHA-256), Passwort je Ziel im `secrets`-Store, Upload unter `.<name>.part` + `rename` |
| `backup::rotation` | Shell | Kalender-Eimer (Europe/Berlin): Floor 7/3/1 Tage/Monate/Jahre (bei erfolgreichem Off-Site-Ziel, sonst 30/12/7), Ordner-Ziele je Ziel 30/12/7, SFTP nie; `manual`/`pre_restore` nie rotiert (ADR 0040) |
| `db::repo::backup_log` | Shell | Append-only `backup_log` (Migration 0026): Datum, Name, Größe, Pfad, Ziel-Typ, Status, Auslöser. **Keine Passphrase.** |
| `notify::rules::rule_backup_result` | Shell | Off-Site-bewusster `backup_overdue` + Erfolg/Fehler-Hinweis, Inbox-only (Migration 0027) |

### 2e. Phase 7 — E-Rechnungs-Empfang (ADR 0037)

| Modul | Schicht | Verantwortung |
|---|---|---|
| `einvoice::parser::check_profile_whitelist` | Core | ZUGFeRD-Profile-Whitelist beim Empfang (`en16931`/`extended`/`xrechnung`); `minimum`/`basic-wl` → `ParseError::UnsupportedProfile` (D-73) |
| `commands::expenses::expenses_receipt_xml_text` | Shell | Roh-XML-Viewer im Eingangsbeleg-Detail; liest still über `archive::store::read_and_verify_silent` (kein `archive.read`-Audit pro Klick, D-72) |

Der Drop-Folder (`domain::drop_folder`, `scheduler::drop_folder`) ist im Server
entfallen (E11, Code entfernt in W11); Eingangsrechnungen kommen per Upload
(§4.18). Migration 0029/0030 bleibt (forward-only).

### 2f. Phase W — Server-Betrieb (ADR 0039)

| Modul | Verantwortung |
|---|---|
| `server::mod` | Einstieg `main_entry` (Subcommands `serve`, `healthcheck`, `totp-reset`, `version`), Router, Zusammenbau vor dem Port-Bind |
| `server::config` | `KLEIN_BUCH_LISTEN`, `…_PUBLIC_URL`, `…_TRUSTED_PROXIES`, `…_INSECURE_HTTP`, `…_ALLOW_UNMOUNTED`; Upload- und Port-Grenzen |
| `server::policy` | öffentliche URL, vertraute Proxys, Client-IP, Host-/Origin-Prüfung (421/403), Transport-Regel für Einrichten/Anmelden |
| `server::startup` | fail-closed-Prüfungen vor dem Port-Bind, Archivpfad-Stichprobe beim Entsperren |
| `server::instance` | Instanz-Marker `/data/.klein-buch-instance` |
| `server::state` | Zustand `setup`/`locked`/`unlocked`, Sitzungen, Jobs, Lastgrenzen, Setup-Token (nur stderr) |
| `server::auth`, `server::totp`, `server::session`, `server::ratelimit`, `server::pending`, `server::passphrase_policy` | Einrichtung, Anmeldung (202 + Abholung bei langer Umstellung), Re-Auth, Sperren, Passphrase-/TOTP-Wechsel, Sitzungsübersicht; TOTP (RFC 6238) + Einmal-Codes in `/data/auth/`; Cookie `__Host-kb_session`; Wartezeiten je Gerät/IP, Obergrenze in Folge; Prüfung neuer Passphrasen |
| `server::dispatch`, `server::jobs`, `server::idempotency` | `POST /api/cmd/{name}` nach Registry; Jobs `202` + `GET /api/jobs/{id}`; Idempotenz-Schlüssel |
| `server::uploads`, `server::files` | `POST /api/upload/{art}`; `GET /files/download/{id}`, `GET /files/archive/{id}` (hash-geprüft, auditiert) |
| `server::headers`, `server::limits`, `server::http`, `server::errors`, `server::logging` | CSP mit Skript-Hashes, HSTS, Cache-Control; Semaphoren je Kostenklasse, globale Grenze, Timeouts; Annahme-Schleife mit Header-Timeout und geordnetem Herunterfahren; Fehlerabbildung (`ref` = Aufruf-Kennung); Log ohne Geheimnisse, eine Zeile je Ereignis, Zugriffsprotokoll mit `ip=`/`req=`/`sess=` |
| `server::health`, `server::legal` | `/healthz` (Loopback ohne Weiterleitungs-Header mit Zustand und `offsite`); `/legal/` ohne Sitzung, gestreamt |
| `app_ctx` | Anwendungskontext der Kerne (Nr. 19–23) |
| `config` | `Paths::from_env` (Pflicht: `KLEIN_BUCH_DATA_DIR`, `…_INPUTS_DIR`, `…_RESOURCE_DIR`, `…_WEB_DIR`; optional `…_BACKUP_TARGETS_DIR`) |
| `seed` | Seed von `/inputs` mit `.seed-manifest.json` (nie überschreiben, was der Nutzer geändert hat) |
| `uploads`, `downloads` | Ablagen mit Ticket unter `<data>/tmp`, Typprüfung am Inhalt, Platzprüfung |
| `nettest` | Port-Allowlist und Kontingent für Verbindungstests, SFTP-Probe-Speicher, Prüfung von SMTP-/SFTP-Hosts (`check_host`) |
| `build_info` | Version, Commit, Tag, Quell-URL (AGPL §13, `/api/public/info`) |
| `clock`, `limits`, `locks` | Uhr Europe/Berlin; CPU-Arbeit außerhalb der Runtime; Sperren je Beleg/Pfad/Vorlage (W1/W2) |

### 2g. Lauf 2026.11 — Oberfläche (ADR 0041)

Das Frontend (`src/`, SvelteKit 3 mit adapter-static, Svelte 5 Runes) ist eine SPA, die der
Server ausliefert; Backend-Aufrufe laufen nur über `src/lib/api.ts` → `transport.ts`. Lauf
2026.11 hat die Oberfläche für Smartphone, Tablet und Desktop überarbeitet; sie orientiert sich
an WCAG 2.2 AA und wurde mit axe-core, eigenen Messungen und manuell geprüft (Prüf-Harness
außerhalb des Repositorys). Maßgeblich sind ADR 0041 (Einzelentscheidungen ab Nr. 1) und die
Designleitlinie `architecture/design.md`; dieser Abschnitt nennt nur die Bausteine.

| Bereich | Ort | Verantwortung |
|---|---|---|
| Tokens | `src/lib/styles/tokens.css` | Palette `--c-*`, darüber Rollen (`--text*`, `--surface-*`, `--line*`, `--status-*`, `--focus-*`) und Skalen (Abstände, Typo-Rollen `--type-*`, Bedienhöhe `--control-h`, Trefferflächen `--target*`, Radien, Schatten); globale Regeln für Feldrahmen, Fokusring, Druckansicht und reduzierte Bewegung. Bausteine nutzen nur Rollen (Nr. 5–8, 60–75). |
| Bausteine | `src/lib/*.svelte` | `PageBar` (Seitentitel als h1, Hauptaktion, Nebenaktionen, Menü „Mehr“), `ActionMenu` (Menu-Button-Muster, Popover-API), `Button`, `Card`, `Table` (Karten bis 820 px, Kartenraster), `Badge` (Status mit Punkt bzw. neutrales Merkmal), `Banner`, `ToastHost`, `FormField`, `MoneyInput`/`QuantityInput` (Leseregel in `money.ts`, Ziffern-Strings und `bigint`), `ContactSelect` (Suchfeld plus natives Auswahlfeld), `DocNo`, `Toggle`, `LoadingState`, `RequiredLegend`, Dialoge über `focusTrap`, `modalStack` und `portal` |
| Layout und Container Queries | `src/routes/+layout.svelte` | Seitenleiste mit Gruppe „Sitzung“; auf Touch-Geräten und bis 900 px Breite ein Drawer (Hintergrund `inert`). `<main>` ist der Container `kb-main`: Seiten und Tabellen fragen die Inhaltsbreite ab, nicht den Viewport (Spaltenpriorität `.kb-col-opt1..3`, Formular-Raster `.kb-form-grid`; Nr. 40–43, 53). Sprunglink „Zum Inhalt“, `scroll-padding` gegen verdeckten Fokus (Nr. 10). |
| Seitentitel | `src/lib/pageTitle.svelte.ts` | `document.title` = „<Titel> – Klein.Buch“; Quellen: Anmelde-Maske, Fehlerseite, PageBar-Titel, Handbuch-Kapitel, sonst Ersatztitel je Route; gesetzt vor dem Zeichnen, damit die Routen-Ansage von SvelteKit den neuen Titel liest (Nr. 160) |
| Hilfe und F1 | `src/lib/routeHelp.ts`, `src/lib/handbook/`, `src/routes/help/` | F1 öffnet das Handbuch-Kapitel zur Route (Präfixtabelle, Test gegen die Slugs), sonst die Startseite des Handbuchs; nicht in Eingabefeldern (Nr. 161). Das Handbuch (`src-tauri/resources/handbook/*.md`) wird beim Build eingebunden (`architecture/handbook.md`). |
| Begriffe und Status | `src/lib/labels.ts`, Status-Helfer je Bereich (`routes/{invoices,quotes,expenses,assets}/*Status.ts`) | Ein Wort je Zustand auf allen Seiten, Einheiten im Klartext (Codes nach UN/ECE Rec 20 bleiben gespeichert), Re-Auth-Titel je Command (Test gegen die Registry; Nr. 73, 163, 185, 201) |
| Formate | `src/lib/format.ts`, `src/lib/date.ts`, `src/lib/money.ts` | DIN 5008 (geschütztes Leerzeichen vor „€“ und „%“, IBAN in Vierergruppen, echtes Minuszeichen), Datum TT.MM.JJJJ in Europe/Berlin, deutsche Betragseingabe „1.234,56“ ohne Float (Nr. 15–19, 206) |
| Fehlerseite, Manifest | `src/routes/+error.svelte`, `static/manifest.webmanifest` | deutsche Fehlerseite mit Weg zurück (Nr. 162); Web-App-Manifest für „Zum Startbildschirm“ ohne Service Worker und ohne Offline-Funktion, CSP unverändert (Nr. 72) |

Server-Seite: neue Lesefelder ohne Migration und der Lese-Command `assets_disposal_preview`
(Nr. 80–86, `architecture/modules.md` §14, `architecture/data-model.md` §2g); Meldungen des
Servers in der Sprache der Oberfläche, nur Texte, keine Logik (Nr. 168).

---

## 3. Start und Entsperren

Ohne Passphrase kein Pool, ohne Pool keine Migrationen, kein Scheduler, kein
DB-Command. Der Server kennt drei Zustände: **`setup`** (leeres `/data`) →
**`locked`** (kein Pool, kein Scheduler) ↔ **`unlocked`**. Browser-Sitzungen
sind davon getrennt (eine Sitzung authentisiert den Browser, die Passphrase
öffnet die DB).

**Prozessstart** (`server::main_entry`): `TZ=UTC` für den Prozess (SQLite-
Zeitstempel, Nr. 94), OpenSSL ohne `atexit` (Nr. 98), `PR_SET_DUMPABLE` 0. Dann
`serve`:

1. Logging; ein Release-Build mit gesetztem `KLEIN_BUCH_SIDECAR_MOCK` bricht ab.
2. `ServerConfig::from_env` und `Paths::from_env` — fehlende oder ungültige
   Werte brechen ab.
3. `prepare`: `index.html` für die CSP-Hashes lesen, dann `startup::run_checks`:
   Sperre `/data/.klein-buch.lock` (ein Prozess je Datenverzeichnis); `/data`
   und `/inputs` vorhanden, beschreibbar und eingebunden;
   `db::prepare_filesystem` (Seed von `/inputs`; vorgemerkter Factory Reset →
   Abbruch, E7; Reste eines Restore-Stagings aufräumen, vorgemerkten Restore
   anwenden, §4.4); Datenbank und Instanz-Marker passen zusammen; keine
   Klartext-DB; Selbsttest `0o400` im Archiv; `<data>/tmp` leeren, abgebrochene
   Backup-Teildateien entfernen (im Betrieb räumt der Sweeper stündlich Reste
   älter als 6 h, D-13).
4. Gerätecookie-Schlüssel laden, Setup-Token erzeugen, falls keine DB oder kein
   TOTP vorhanden ist (eigene Zeile auf stderr, nie im `tracing`-Log).
5. Port binden. Erst jetzt ist der Server erreichbar; `/healthz` meldet `locked`
   als gesund.

**Einrichtung** (`POST /api/auth/setup`, zweistufig, Nr. 46): Schritt 1 liefert
gegen den Setup-Token ein TOTP-Geheimnis samt QR-Code; Schritt 2 prüft Token und
TOTP-Code **vor** der Schlüsselableitung, legt dann DB (verschlüsselt),
Instanz-Marker und `totp.json` (mit zehn gehashten Einmal-Codes) an, entsperrt
und erstellt das erste Backup. Scheitert die Einrichtung vor dem Entsperren,
wird die halbe DB entfernt; scheitert nur das erste Backup, holt es der erste
Scheduler-Tick nach.

**Anmeldung = Entsperren** (`POST /api/auth/login`, eigener Task, Nr. 41/42):

1. Transport-Regel (HTTPS über vertrauten Proxy oder Loopback), Rate-Limit,
   TOTP bzw. Einmal-Code vor der KDF; falsche Faktoren antworten identisch.
2. `open_encrypted` + Probe-Read; falsche Passphrase → kein Zustand.
3. Archivpfad-Stichprobe gegen `/data/archive`; passt sie nicht → 409, wieder
   gesperrt.
4. Älteres Schema → Pflicht-Backup vor der Migration (geprüft); scheitert es,
   bleibt die Instanz gesperrt (409 `pre_migration_backup_failed`).
5. Migrationen + Schema-Check (`EXPECTED_SCHEMA_VERSION`), danach
   `activate_unlocked`: Pool und Passphrase in den Kontext, Scheduler starten,
   Audit, ggf. `backup.restore.applied`.

Ist die DB schon entsperrt, prüft die Anmeldung nur die Faktoren (Passphrase
über den Verifier in `app_settings`) und legt eine Sitzung an. Dauert die
Anmeldung länger als 25 s (Pflicht-Backup und Migration), antwortet sie mit 202
und einem Ticket-Cookie; `POST /api/auth/login/poll` liefert das Ergebnis samt
Sitzung (Lauf F4a, W14-037). **„Datenbank sperren"** schließt den Pool, löscht
die Passphrase (`zeroize`), beendet alle Sitzungen, verwirft gespeicherte
Antworten, Job-Ergebnisse und offene Anmeldungen und pausiert den Scheduler.

**Konsequenz:** Kein DB-Command läuft vor dem Entsperren; die Registry
verlangt für alles außer den öffentlichen Routen eine Sitzung, DB-Commands
zusätzlich `unlocked` (sonst 423), kritische Aktionen eine frische Re-Auth.

---

## 4. Daten- und Kontrollflüsse

### 4.1 Rechnung erstellen → festschreiben (Block 3)
UI-Form → `invoices_create_draft` → DB-Draft (Nummer allokiert, GJ aus dem
Belegdatum) → *Festschreiben* läuft als Job (`202`, `GET /api/jobs/{id}`) →
`run_lock_pipeline`: Domain-Validate → CII-XML generieren → KoSIT validate
(Sidecar) → §19-Klausel-Check → Typst-PDF/A-3b → Mustang ZUGFeRD → Archive write
(PDF + XML, SHA-256, `0o400`) → DB-Lock (`status='issued'`, `locked_at`) →
Audit-Log → Auto-Critical-Backup angefordert (Vermerk, im Scheduler kurz nach
der Antwort; Floor + Off-Site, §4.5). Sperre je Rechnung
und je Archivpfad; ein abgebrochener Request läuft als Task zu Ende, die
Wiederholung mit demselben Idempotenz-Schlüssel liefert dasselbe Ergebnis.

### 4.2 Rechnung versenden (Block 5)
Detail → *Senden* → Konto, Empfänger, Betreff, Text (Vorlage `invoice-de`) →
`mail_send_invoice` (Job) → `send_invoice_core`: Account laden → Empfänger
auflösen (Buyer-Snapshot → Kontakt) → Body rendern → ZUGFeRD-PDF aus dem Archiv
lesen **+ SHA-256-Verify** → Passwort aus `secrets` → `mail::smtp::send` →
`invoices::mark_sent` → Audit `invoice.sent` (ohne Passwort) → `email_log`
(append-only, Migration 0015). Versand nur für gelockte, nicht-stornierte
Rechnungen mit archiviertem PDF.

### 4.3 Storno (Block 3)
Storno erzeugt eine **neue** Rechnung (`is_storno_for`), durchläuft dieselbe
Lock-Pipeline, markiert das Original `canceled`. Keine Löschung (ADR 0006).

### 4.4 Restore (Block 4, W2, W7)
Zweiphasig: Die DB-Datei wird nie unter einem offenen Pool getauscht.

**Phase A (Job, Re-Auth):** Vorprüfung im Dispatcher vor der Annahme (Quelle,
Passphrase; ungültig → 400 ohne Sperre, W14-039) → Backup per Upload (`POST /api/upload/restore`) oder
aus der lokalen Historie → Manifest, Format (1–3), Schema
`30 ≤ s ≤ EXPECTED` → Entschlüsseln in place (falsche Passphrase oder
veränderter Kopf schreibt nichts) → Audit
`backup.restore.requested` → Platzprüfung → gehärtet ins Staging entpacken →
gestagte DB prüfen (Schema, Archivpfade, Migrations-Prüfsummen) → Inhalt
freigegeben, dann Pre-Restore-Backup (Lauf F3c) → zweite Platzprüfung →
Nummernkreise der laufenden DB festhalten (D-24) → Marker. Ab der Annahme lehnt der Server Änderungen mit 503 ab und pausiert den
Scheduler; nach Erfolg fährt er geordnet herunter
(Exit 0), die Container-Verwaltung startet ihn neu.

**Phase B (nächster Start, `db::prepare_filesystem`, vor dem Port-Bind):**
`restore::apply_pending` tauscht DB (samt WAL), `archive/` und `branding/` in drei Stufen
mit Zustandsdatei und Rückabwicklung; die Angaben landen in
`RESTORE_APPLIED.json`. Das erste Entsperren schreibt in **einer** Transaktion
`backup.restore.applied`, die Pause `scheduler_hold = restore` (W14-023) und das
Anheben der Nummernkreise auf die festgehaltenen Stände, wo diese höher waren
(D-24, Audit `numbering.raised_after_restore`), legt Hinweise an und übernimmt
das Pre-Restore-Backup und die nach dem eingespielten Stand entstandenen
Floor-Backups in die Historie (D-22). `/inputs` bleibt unberührt. Einzelheiten: `architecture/security.md`
§6.

### 4.5 Backup erstellen (G1, W2, W7, Lauf F3a)
`backup::create_now` (prozessweit serialisiert) → Snapshot per `VACUUM INTO`
über eine eigene Verbindung (verschlüsselt wie das Original) + `archive/` +
`branding/` (ohne versteckte Temp-Dateien) → Content-ZIP → Argon2id-Schlüssel +
AES-256-GCM in place mit dem Klartext-Kopf als AAD (Format 3; außerhalb der
Runtime, KDF-Semaphore) → `.kbk` → Floor `/data/backups` (Temp-Datei, `fsync`,
`rename`) und jedes Off-Site-Ziel (Ordner unter `/backup-targets`, SFTP) mit
Zeitlimit je Ziel, ein Puffer für alle → `backup_log` je Ziel, auch ein
gescheiterter Snapshot (**niemals** Passphrase) → Rotation je Ablage
(Kalender-Eimer; nur `.kbk` unter den Wurzeln; SFTP nie; ruht während einer
Pause) → Hinweis über `rule_backup_result`. Ein gescheitertes Ziel ist nicht
fatal (Status `offsite = failing`, Inbox; `stale` bei fehlender oder zu alter
Kopie). Nach einem Restore geht bis zur Freigabe nichts auf die Off-Site-Ziele
(D-23). **Lock-Backups** fordern die festschreibenden Commands nur an
(`backup::request_auto_backup`: Vermerk, Audit `backup.requested`, Wecksignal);
die Scheduler-Schleife legt sie nach zwei Sekunden unter der Tick-Sperre an
(ADR 0040 Nr. 50). **Download** einer `.kbk` aus Floor oder Ordner-Ziel mit
Re-Auth über ein einmaliges Ticket (`backup_download`).

### 4.6 Im Server entfallene Abläufe
- **Factory Reset** (ADR 0036): im Server abgeschaltet (E7), Code entfernt
  (W11). Ein vorgefundener Marker `FACTORY_RESET_PENDING.json` verweigert den
  Start, gelöscht wird nichts.
- **Klartext → SQLCipher-Migration** (G1-ENC.4): entfernt (E17, Nr. 57); eine
  Klartext-DB verweigert den Start.
- **Drop-Folder, OS-Benachrichtigungen, „Im Ordner zeigen", Ordnerauswahl per
  Dialog, Cloud-Ordner-Erkennung, Microsoft-Graph-Versand**: entfallen bzw.
  gesperrt (E8, E9, E11, Nr. 68, 77, 91).

### 4.7 Passphrase und zweiter Faktor ändern (W6)
Re-Auth, dann `POST /api/auth/passphrase`: warten auf laufende Commands/Jobs
(`db_gate`, höchstens 60 s) und auf den Scheduler-Tick (höchstens 10 s, sonst
503) → `db::rekey_db_file` mit der neuen
Passphrase (Probe, `quick_check`, `fsync`, Tausch) → andere Sitzungen abmelden →
Backup (`manual`) als nachverfolgter Task. Ältere Backups bleiben mit der alten
Passphrase einspielbar. TOTP und Einmal-Codes erneuert `POST
/api/auth/totp/renew` bzw. `/api/auth/recovery/renew`; bei Geräteverlust setzt
`klein-buch-server totp-reset` (Container-Shell) den zweiten Faktor zurück.

### 4.8 Angebote: Lebenszyklus + Konvertierung (Block 6/7)
Eigener Belegkreis `AN-{YYYY}-{NNNN}`. Lifecycle:
`draft → (Festschreiben = Lock → sent) → accepted | rejected → converted | canceled`.
Festschreiben setzt `locked_at`; ab da greift `trg_quotes_immutable` auf den
Kernfeldern (Angebote sind keine E-Rechnungen → kein KoSIT/Mustang). Annahme
archiviert optional den unterschriebenen Vertrag write-once (Upload-Art
`vertrag`, `attachments`, `parent_type='quote'`). Konvertierung läuft **nur aus
`accepted`**: `convert_to_invoice` (pure) → gemeinsamer Draft-Helper (setzt
`derived_from_quote_id`) → `mark_converted`. Die erzeugte Rechnung durchläuft
die normale Lock-Pipeline (§4.1). ADR 0016/0017.

### 4.9 Angebots-PDF, Rechtsdokumente, Bundle (Block 8)
`ensure_quote_pdf` rendert beim ersten Mal über `pdf::typst_render::render_quote`
(§19-Klausel-Check) ein **Plain-PDF**, archiviert es write-once
(`ArchiveKind::QuotePdf`), setzt `quotes.pdf_archive_id` (idempotent). AGB +
Datenschutz als versioniertes `legal_documents` (Upload nur PDF, write-once,
höchstens eine aktive Version pro Typ, append-only und immutable).
`prepare_quote_dispatch` bindet die aktiven Versionen append-only in
`quote_legal_documents`. Druck = `pdf::bundle::merge_pdfs` (lopdf), im Browser
als Vorschau-Dialog bzw. Download. Mail = `send_quote_core` mit **drei**
Anhängen, Audit `quote.sent`. ADR 0018.

### 4.10 Kosten + E-Rechnung-Empfang (Block 9/11)
`expenses_create` → `domain::expense`-Validierung → Beleg per Upload (Art
`beleg`, auch Kamera-Foto) write-once → sofort gelockt (ADR 0019). §19 wirkt nur
ausgangsseitig → Kosten gehen **brutto** in die EÜR. Empfang: Upload (Art
`erechnung`) → `einvoice::parser` (CII/UBL/ZUGFeRD) → KoSIT-Prüfung
**beratend** → Import als Kostenposition (`paid_date = None`) + Original
write-once archiviert (`ReceivedEinvoice`). ADR 0024.

### 4.11 Anlagen/AfA → EÜR → Export (Block 12–14)
Anlage anlegen → manueller AfA-Lauf (`domain::depreciation`, idempotent) →
`depreciation_entries`. EÜR: `euer::load_inputs` (Shell) → `euer::aggregate`
(Core, Cash-Basis §11 EStG, ADR 0022) → read-only Report. Export:
`euer_export_*` → ELSTER-Ausfüllhilfe + „Anlage EÜR"-PDF (Typst), DATEV-EXTF
(SKR03-Default), Steuerberater-ZIP — alle als Download-Ticket (E10, §4.21).
AfA-Safeguard warnt bei ungebuchter AfA. ADR 0025/0026.

### 4.12 Geschäftsjahres-Abschluss + Hinweise (Block 15)
`fiscal_year::close_year` (nur abgelaufene Jahre, nur entsperrt): AfA buchen →
Anlagen + Abschreibung sperren → EÜR-Snapshot ins Festschreibungsprotokoll
(Migration 0013, no-update/no-delete) → Audit → Auto-Backup; **unumkehrbar**.
`guard::ensure_year_open` in den Kernen (Storno bleibt erlaubt).
Scheduler-Jobs siehe §5.

### 4.13 Microsoft-Graph-Versand (Block 16) — entfernt
Der Desktop-Flow nutzte eine Loopback-Weiterleitung auf dem eigenen Rechner
(ADR 0028). Im Server gab es ihn nie (E9); seit Lauf F6 sind `mail::oauth_ms`,
die Commands `mail_oauth_*` und ihr Test entfernt (ADR 0039 Nr. 138). Neue oder
geänderte Konten mit `auth_type = oauth_microsoft` werden abgelehnt, ein
übernommenes Konto versendet nicht (Meldung ohne Verbindungsaufbau) und lässt
sich auf SMTP umstellen (Token und OAuth-Spalten werden dabei geleert) oder
löschen. Die Spalten aus Migration 0014 bleiben (forward-only). Microsoft 365
läuft über SMTP. Das append-only `email_log` bleibt; ältere Einträge mit Kanal
`graph` werden weiter angezeigt.

### 4.14 PDF-Vorlagen-Auswahl (Block 17a)
Globale Auswahl in `settings/pdf-templates` (`seller_default_template_set`,
§19-Schutz). Beim Render löst `pdf::templates::resolve_invoice_template` /
`resolve_quote_template` auf: `/inputs/pdf-templates/{name}.typ`-Override →
Built-in; Sentinel `'default'` → `seller.default_pdf_template`. Logo aus
`/data/branding` via `World::file()`. Vorschau: `pdf_template_preview` rendert
ein Muster-PDF (Download-Ablage, Vorschau-Dialog). ADR 0030.

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
erzeugt unter `<data>/tmp/dsgvo/<id>` ein ZIP (PDF + JSON + archivierte
Originale + LIESMICH), ausgeliefert als einmaliger Download; genau ein Audit,
keine Beleg-Mutation, nie die Tabelle `secrets`. **Anonymisierung (Art. 17):**
überschreibt die Kontakt-Stammdaten (`Anonymisiert #<hex>`, PII → NULL); die in
Belege eingefrorenen Buyer-Snapshots bleiben (GoBD gewinnt). Guard: keine
offenen Entwürfe; einmalig + irreversibel. ADR 0032.

### 4.17 Abo-Rechnungen / Ausgangsseite (Phase 4)
Abo-Vorlage (`recurring_invoices`, Stammdatum) → der Scheduler-Job im Tick (nur
entsperrt; die Seite löst beim Öffnen keinen Lauf aus) materialisiert am
Stichtag über **dieselbe** draft→lock-Pipeline (§4.1): Belegdatum = **heute**
(nie rückdatiert, §14 Abs. 4 Nr. 3), Leistungszeitraum als `delivery_date`.
`auto_mode` steuert je Vorlage Entwurf / Festschreiben / Festschreiben+Versand.
§19 wird beim Speichern und vor jeder Materialisierung serverseitig erzwungen.
Nummer, Entwurf, Audit und Vorrücken committen in einer Transaktion;
Festschreiben/Versand setzt ein Lauf nach einem Abbruch fort (Marker). Bis drei
fällige Termine holt der Lauf selbst nach (nur der jüngste nach `auto_mode`,
ältere als Entwurf), mehr nur nach Bestätigung („Rückstand …“). Gegen Doppel:
Sperre je Vorlage (auch Bearbeiten/Pausieren) plus Compare-and-Swap. ADR 0033,
ADR 0040.

### 4.18 Rechnungseingang per Upload (E11, ersetzt den Drop-Folder)
Eingangsrechnungen lädt der Nutzer im Browser hoch (`POST /api/upload/erechnung`,
PDF oder XML, höchstens 10 MiB, Typ am Inhalt geprüft, HTML/Skript abgelehnt).
Der Upload liegt als Ticket unter `<data>/tmp/uploads/<uuid>` (höchstens 1 h);
Schritt 1 liest ihn (Parser, KoSIT beratend), Schritt 2 archiviert dasselbe
Original und legt die Kostenposition an. Erst nach Erfolg wird der Upload
verbraucht (Nr. 69/70).

### 4.19 Roh-XML-Viewer im Eingangsbeleg-Detail (Phase 7, ADR 0037)
Eingangsbeleg → Detail-Seite → „Roh-XML anzeigen" (nur sichtbar, wenn der
Beleg eine archivierte E-Rechnung als Quelle hat). `expenses_receipt_xml_text`
lädt die XML aus dem write-once-Archiv: bei `source_format=zugferd` via
`mustang_bridge::extract_xml`; bei `xrechnung-cii`/`xrechnung-ubl` direkt als
UTF-8. Anzeige in einem `XmlViewerDialog` mit Pretty-Print, Copy-Button, Hash
und Byte-Größe. **Kein** `archive.read`-Audit pro Klick
(`read_and_verify_silent`, D-72); die Hash-Prüfung bleibt.

### 4.20 ZUGFeRD-Profile-Whitelist beim Empfang (Phase 7, ADR 0037)
Der Parser ruft `einvoice::parser::check_profile_whitelist` direkt nach dem
CII-Walk und vor dem Currency-Check auf. Akzeptiert werden Profile mit
Substring `en16931`, `extended` oder `xrechnung` im
`GuidelineSpecifiedDocumentContextParameter.ID`; `minimum` und `basic-wl`
werfen `ParseError::UnsupportedProfile(<URN>)` (D-73).

### 4.21 Downloads und Archiv-Abruf (E10, W7)
Kein Command nimmt einen Server-Pfad an. Exporte und Vorschauen entstehen in
`<data>/tmp/downloads/<uuid>/<Dateiname>`; der Kern liefert
`{downloadId, fileName, sizeBytes}`, der Browser holt die Datei über
`GET /files/download/{id}` (Sitzung). Exporte sind einmal abrufbar (die Datei
wird mit dem Abruf gelöscht) und verfallen nach 10 min (Lauf F4b, W14-014),
Vorschauen sind 10 min mehrfach abrufbar. Archivdateien
liefert `GET /files/archive/{id}` nach Hash-Prüfung (Audit `archive.read`);
inline nur eigene PDFs (Rechnung, Angebot), alles andere als `attachment` mit
CSP `sandbox`.

---

## 5. Scheduler-Modell

Ein Hintergrund-Task (`scheduler::tick::start`), Tokio-Interval **5 Minuten**
(`TICK_INTERVAL_SECS = 300`). Verpasste Ticks werden **nicht nachgefeuert**
(`MissedTickBehavior::Skip`) — der nächste reguläre Tick deckt den Rückstand
ab, weil alle Jobs ihre Fälligkeit aus persistierten Werten bestimmen. Die Zeit
kommt aus der Uhr des `AppCtx` (`clock::Clock`, in Tests `FixedClock`); Jobs
bekommen sie mit Pool, Pfaden, Sitzung und Abbruch-Anforderung über
`scheduler::JobCtx` (ADR 0040).

**Gating.** Der Scheduler startet erst **nach** dem ersten erfolgreichen
Entsperren (§3), einmal pro Prozess. Ohne Pool (gesperrt) tut ein Tick nichts;
das Log meldet den gesperrten Zustand beim Serverstart und alle 6 h.
Restore, Herunterfahren und „Datenbank sperren" pausieren ihn über ein Gate;
vorher setzen sie die kooperative Abbruch-Anforderung (`AppCtx::scheduler_stop`),
damit ein laufender Abo-Lauf nach dem aktuellen Termin und der Archiv-Vollscan
nach der aktuellen Datei aufhört. „Datenbank sperren“ wartet darauf höchstens
10 s und sperrt dann trotzdem; „Passphrase ändern“ antwortet nach dieser Frist
mit 503. Das Herunterfahren setzt die Abbruch-Anforderung schon beim Signal und
wartet dann gleichzeitig höchstens 25 s auf den Tick und 30 s auf abgekoppelte
Tasks (`stop_grace_period` 90 s, ADR 0040). Ein noch ausstehendes Lock-Backup
schreiben beide vor dem Schließen des Pools (`tick::flush_pending_backup`,
Sperren höchstens 60 s, Herunterfahren im Rest von 40 s; ADR 0040 Nr. 57). Nach einem Neustart läuft nichts,
bis jemand entsperrt (E3); der erste Tick nach dem Entsperren wartet 60 s
(`tick::UNLOCK_SETTLE`), lässt den Vollscan aus und holt Fälliges nach —
Abo-Läufe mit Grenze (bis drei Termine selbst, mehr nach Bestätigung), genau
ein Tages-Backup. Mehr als zwei Tage ohne Aktivität meldet das Entsperren als
Hinweis.

**Pause.** `app_settings.scheduler_hold` hält Abo-Rechnungen, Abo-Kosten und
Auto-AfA an (Tick und manuelle Auslöser), bis der Inhaber im Browser freigibt
(`scheduler_release_hold`, Re-Auth). Gesetzt bei unplausibler Uhr (mehr als 1 h
zurück oder mehr als 400 Tage vor dem zuletzt gesehenen Lauf) und nach einem
Restore (in der Transaktion des Restore-Audits). Backups, Integritätsprüfung und
Hinweise laufen weiter; die Backup-Rotation ruht, und nach einem Restore gehen
Backups bis zur Freigabe nur in den Floor. In derselben Transaktion rückt
`backup::after_restore` die Abo-Reihen nur vorwärts auf ihre Termine vor dem
Restore, merkt Reihen ohne bekannten Stand zur Entscheidung vor (nur solche,
für die ein Lauf etwas erzeugen würde; F3b), verwirft
Fortsetzungs- und Nachhol-Marker aus dem Backup und merkt Entwürfe vor, deren
Nummer vor dem Restore schon festgeschrieben war (ADR 0040, F1d). Die Pause des
Schedulers selbst (`AppCtx::scheduler_paused`) hebt das Entsperren auf, ohne auf
einen beim Sperren noch laufenden Tick zu warten.

**Jobs pro Tick (in dieser Reihenfolge, jeder Job isoliert):**

1. **`scheduler::recurring`** — fällige Kosten-Abos buchen (Block 10): je Abo
   unter Sperre, Beleg + Audit + Vorrücken in einer Transaktion, Fehler je Abo.
2. **`scheduler::recurring_invoice`** — fällige Abo-Rechnungen materialisieren
   (Phase 4).
3. **`scheduler::depreciation_year_close`** — Auto-AfA zur GJ-Wende, default-an
   per `app_settings.depreciation_auto_year_close` (Schalter über `fiscal_year_auto_close_set`, Stand in `fiscal_year_overview`), abschaltbar (Block 15);
   je Anlage und Jahr eine Transaktion, Fehler mit Hinweis und 6 h Backoff.
4. **Backup** — Tages-Backup (`backup::auto_daily_if_due`): genau eines je
   Berliner Kalendertag ab `backup_daily_time` (Standard 02:30; fällig ist der
   zuletzt fällige Slot, ein verpasster wird beim ersten Tick nachgeholt), nach
   einem Fehlschlag 1 h Backoff (persistiert). Es deckt die Lock-Ereignisse der
   Schritte 1 und 2 mit ab; sonst legt der Tick genau ein Lock-Backup an
   (D-19, Backup-Hardline). Ein ausgefallenes Lock-Backup (Vermerk
   `backup_pending_since`) holt der erste Tick danach nach
   (`backup::pending_backup_if_due`). Zwischen den Ticks weckt ein
   Lock-Ereignis der Commands die Schleife für sein Backup (Lauf F3a,
   `tick::BACKUP_COALESCE`), auch während der Wartezeit nach dem Entsperren.
5. **`scheduler::integrity_check_cron`** — monatlicher Archiv-Hash-Verify
   (nicht im ersten Tick nach dem Entsperren); gemeinsame Sperre mit „Jetzt
   prüfen“, 6 h Backoff nach Fehlschlag; manipuliert, fehlt und nicht lesbar
   getrennt (`archive.integrity_fail`, `_missing`, `_unreadable`).
6. **`scheduler::reminders`** — Regel-getriebene Hinweise (`notify::rules`),
   inkl. Off-Site-bewusstem `backup_overdue` (Alter in Europe/Berlin), Hinweis
   ohne Off-Site-Ziel und „Abo fällig“ für Kosten-Abos im Modus Erinnerung.
7. **`scheduler::update_check`** — opt-in Versionsprüfung (Standard aus;
   höchstens alle 24 h, nur GET, ohne Weiterleitungen, Hinweis nur als Text;
   Nr. 111). Läuft außerhalb der DB-Jobs, damit deren Tests nie ins Netz gehen.

Manuelle Trigger: Seiten, die einen Job kennen, bieten „Jetzt prüfen"
(`recurring_run_due_check`, `recurring_invoices_run_due_check`,
`archive_integrity_run`, `notifications_run_checks`). Diese Wege rufen die
Job-Funktionen direkt auf, unabhängig vom Tick, mit denselben Sperren, Grenzen
und Pausen. Seiten lösen beim Öffnen keinen Lauf und keinen Schreib-Command aus
(Vitest `no-side-effects-on-open`).

---

## 6. Sidecar-Bridge

Ein Java-Sidecar mit eigener, per `jlink` gebauter Laufzeit (Eclipse Temurin 21)
im Image unter `/opt/klein-buch/sidecar`. Er hostet zwei Kommandozeilen-Werkzeuge:

- **KoSIT-Validator** 1.6.2 mit XRechnung-Konfiguration 2026-01-31
  (XRechnung CII + UBL, ZUGFeRD-XML).
- **Mustang** 2.23.0 für PDF/A-3 + ZUGFeRD-XML-Embedding und -Extraktion.

**Warum Sidecar.** Beide Werkzeuge sind Java und in Reife/Konformität
unschlagbar. Eine Reimplementierung in Rust ist Out-of-Scope (Risiko
§14/EN-16931-Konformität). ADR 0001 (Sidecar, Amendment: nur Linux), ADR 0008
(Typst + Mustang als PDF/A-3-Pfad).

**Lifecycle.**

- **Quellen.** `src-tauri/sidecar/` enthält die Original-JARs und die entpackte
  XRechnung-Konfiguration, `versions.json` nennt Quelle und SHA-256 je
  Komponente; die CI vergleicht mit den Originalen (`image.yml`). Die Laufzeit
  baut das `Dockerfile` (Stufe `jre`). Einzelheiten:
  `architecture/build-and-release.md` §2.
- **Aufruf.** `einvoice::validator` und `einvoice::mustang_bridge` starten
  `klein-buch-java.sh` über `/bin/sh` (`tokio::process::Command`), Eingaben als
  Temp-Dateien unter `TMPDIR` (`/data/tmp`), Timeout + `kill_on_drop`, höchstens
  zwei JVM-Läufe gleichzeitig (`limits::JVM`), JVM mit `-Xmx512m`.
- **Mock.** `KLEIN_BUCH_SIDECAR_MOCK=1` überspringt den Sidecar nur in
  Debug-/Test-Builds; ein Release-Build mit gesetztem Schalter startet nicht.
- **Prüfung.** Der Smoke-Test des Images legt eine Rechnung mit echtem KoSIT und
  Mustang an; lokal laufen zusätzlich die `sidecar_real`-Tests gegen eine
  jlink-JRE.
- **Outbound.** Keiner; KoSIT und Mustang arbeiten offline.

**Trennung Core/Shell.** XML-Generierung und Klausel-Check sind Core (pur).
Validierung und PDF/A-3-Erzeugung sind Shell, weil sie den Sidecar als Effekt
brauchen. Wird der Sidecar getauscht, ändert sich nur die Shell-Schnittstelle.

---

## 7. Hard-Rules

Diese Regeln sind nicht verhandelbar. Hier eine konsolidierte Sicht; Details in
den verlinkten ADRs.

- **GoBD** (ADR 0006). Festgeschriebene Belege unveränderlich (DB-Trigger),
  Storno statt Löschung, Archiv write-once + SHA-256 + Re-Hash + `0o400`,
  Audit-Log append-only, 10 Jahre Aufbewahrung. Keine Löschung in der App, auch
  keine Totallöschung (Factory Reset im Server abgeschaltet, E7). Unter Linux
  heißt write-once Erkennung statt Verhinderung (`architecture/security.md`,
  „Write-once unter Linux").
- **§19 Kleinunternehmer** (ADR 0005). Default `is_kleinunternehmer = true`,
  USt-Felder im UI gesperrt, BT-22-Klausel Pflicht im XML + sichtbar im PDF,
  §14c-Schutz im Backend, Verzicht-Toggle mit 5-Jahres-Bindungs-Warn-Dialog.
- **Backup** (ADR 0009 + 0034 + 0035 + 0039). Passphrase bei der Einrichtung
  erzwungen; Auto-Backup bei jedem Lock (vermerkt, kurz danach im Scheduler)
  + täglich im Scheduler; Pflicht-Backup
  vor jeder Migration und vor jedem Restore; Floor + mehrere Off-Site-Ziele
  (Off-Site Pflicht, Warnung statt Blockade); `backup_log` append-only;
  Passphrase **niemals** in DB, Logs, Audit oder Backup-Log.
- **Verschlüsselung at Rest** (ADR 0035). Eine Geheimnis-Kette:
  Daten-Passwort = SQLCipher-Key (PBKDF2-HMAC-SHA512) = Backup-Schlüssel-Wurzel
  (Argon2id). Passphrase vor Pool-Open; kein Auto-Unlock (E3). Verlust =
  Totalverlust by design.
- **Anmeldung** (ADR 0039, E5). Passphrase + TOTP, Einmal-Codes, Re-Auth für
  kritische Aktionen (Verkäuferprofil bedingt), Fehlversuche nur im Log; neue
  Passphrasen gegen Sperrliste und Muster, Obergrenze von 100 Fehlversuchen in
  Folge ohne Gerätecookie, Sitzungsübersicht (Lauf F4a, Nr. 130).
- **Credentials** (ADR 0011/0028 + Amendment, E6). SMTP- und SFTP-Passwörter nur
  in der Tabelle `secrets` der verschlüsselten DB, gebunden an
  (Host, Port, Benutzer); nie in Log, Audit, Migrations- oder DSGVO-Export.
- **GJ-Abschluss** (ADR 0027). Abgeschlossene Geschäftsjahre unveränderlich
  (no-update/no-delete-Trigger), Abschluss nur für abgelaufene Jahre,
  unumkehrbar; Storno als Korrektur bleibt möglich.
- **Schema-Disziplin** (ADR 0003). STRICT-Tabellen, forward-only Migrationen
  (0001–0032, Version v32, LF-Zeilenenden), Schema-Version-Check beim
  Entsperren (neuere DB ⇒ kein Entsperren), Geld als Integer-Cents, UUIDv7-PKs.
  Restore nimmt Backups mit `30 ≤ schema ≤ EXPECTED` und migriert vorwärts.
- **`inputs/` tabu.** Vom Menschen gepflegt; der Container ergänzt das Volume
  `/inputs` aus dem Image, überschreibt aber nie, was der Nutzer geändert hat
  (Seed-Manifest, `<name>.neu`). Maschinen schreiben nach `/data`. Ausnahme:
  `inputs/pdf-templates/` darf im Quellstand direkt gepflegt werden und ist über
  alle Vorlagen inhaltlich einheitlich zu halten.
- **Local-first** (ADR 0013 + Amendment). Keine Telemetrie, kein Auto-Update,
  keine Cloud-API. Outbound nur für Versand (SMTP), Off-Site-Backup (SFTP) und
  die opt-in Versionsprüfung.
- **Zeitzone** (E12). Fachliche Daten immer Europe/Berlin (`crate::clock`,
  `src/lib/date.ts`); das Geschäftsjahr bestimmt der Server aus dem Belegdatum.
  Zeitabhängige Abläufe lesen die Uhr aus dem `AppCtx` (ADR 0040). Die DB
  speichert Zeitpunkte in UTC (`YYYY-MM-DD HH:MM:SS`); die Oberfläche zeigt sie
  über `date()`/`dateTime()` aus `src/lib/format.ts` mit Berliner Tag und
  Uhrzeit, Tage bis zur Fälligkeit über `daysUntilBerlin` — unabhängig von der
  Zeitzone des Geräts (Lauf F, D-15/D-25).

---

## 8. Phasen und Auslieferung

- **Phase 1 (Blöcke 0–5).** Setup, Foundation, Invoice-Pipeline, Backup, Mail.
- **Phase 2A–2D (6–17).** Angebote; Kosten, Recurring, E-Rechnung-Empfang;
  Anlagen, AfA, EÜR, Export; Hinweise, OAuth, Design-System, PDF-Vorlagen.
- **Phase 3 (P1–P4).** Anfahrt, Paket-Katalog, Paket-in-Beleg, Broschüre
  (ADR 0031).
- **DSGVO (Blöcke 18/19).** Auskunft Art. 15 + Anonymisierung Art. 17 (ADR 0032).
- **Phase 4 (RI-1..RI-3).** Wiederkehrende Ausgangsrechnungen (ADR 0033).
- **G1 Security/Backup-Härtung.** SQLCipher, Backup-Ziele, `backup_log`,
  Off-Site-bewusste Hinweise, Factory Reset (Desktop), Härtung (ADRs 0034–0036).
- **v1.0 (v2026.5.0, v2026.5.1).** Letzte Desktop-Releases (Windows, NSIS).
- **Phase 7.** E-Rechnungs-Empfangs-Politur (ADR 0037).
- **Phase W (W0–W14, ADR 0039).** Desktop-Fehler vorab (W1/W2), Kern-Schnitt
  (W4), HTTP-Server statt Tauri (W5), Anmeldung mit TOTP (W6), Uploads,
  Downloads, Restore, mehrere Off-Site-Ziele (W7), Zugangsdaten in der DB (W8),
  Frontend ohne Tauri (W9), responsiv (W10), Linux-Sidecar und Image (W11),
  Lizenzen, Quellen und Release-Infrastruktur (W12), Handbuch und Betriebsdoku
  (W13), Security-Review (W14). Veröffentlicht als v2026.10.0.
- **Lauf E und Lauf F.** Entscheidungen (Storno im Folgejahr u. a.) und alles
  Funktionale, Technische und GoBD-Relevante vor dem UI/UX-Lauf: Start- und
  Zeitlogik (ADR 0040), Migration 0032, Backup-Format 3 mit Lock-Backup im
  Hintergrund, Anmeldung und Härtung, Fachlogik, Tauri-Altcode entfernt,
  SvelteKit 3 / Vite 8 / Node 24 / Typst 0.15, CI und Doku. Erscheint mit
  2026.11.0 nach dem UI/UX-Lauf.
- **Lauf 2026.11 (U0–U7, ADR 0041).** Oberfläche für Smartphone, Tablet und
  Desktop: Tokens und Bausteine, deutsche Betragseingabe, Fehler am Feld, Menü
  „Mehr“, Seitentitel, F1 zum Kapitel, Begriffe der Oberfläche in Handbuch und
  Server-Meldungen, Lesefelder ohne Migration (§2g). Version 2026.11.0.

**Auslieferung** ist das signierte Image `ghcr.io/hozwei/klein-buch:<CalVer>`
aus dem öffentlichen Repository, mit SBOM und Quellarchiv je Release; betrieben
und aktualisiert wird es in der Container-Verwaltung des Betreibers.
Einzelheiten: `architecture/build-and-release.md`, `docs/operations-guide.md`,
`SECURITY.md`.

---

## 9. Vertiefende Architektur-Dokumente

Diese Datei ist die Top-Level-Übersicht. Tiefere Themen leben als eigene
Dateien in `docs/architecture/`:

| Datei | Inhalt | Stand |
|---|---|---|
| [`architecture/security.md`](architecture/security.md) | Bedrohungsmodell für den Internetbetrieb (Phase W) mit Status je Maßnahme; SQLCipher-Kette, Verifier, Backup-Hülle, Off-Site-Ziele, SFTP, `secrets`-Store, Start und Entsperren, Restore-Phasen, Integritätsprüfung, Versand, Write-once unter Linux | 2026-10-25 (Lauf F, inkl. §11 ASVS-Entscheidungen) |
| [`architecture/gobd.md`](architecture/gobd.md) | GoBD-Umsetzung: Lock-Trigger, Archiv write-once + SHA-256 + Re-Hash, Audit-Log append-only, Storno, Aufbewahrung, Tests | 2026-10-25 (Lauf F2) |
| [`architecture/build-and-release.md`](architecture/build-and-release.md) | Image-Build (`Dockerfile`), Sidecar aus geprüften Quellen, CI-Gates, Trockenlauf, Release mit Signatur/SBOM/Quellarchiv, Veröffentlichen | Phase W (W13) |
| [`architecture/paragraph-19.md`](architecture/paragraph-19.md) | §19-Logik: UI-Sperren, BT-22-Pfad in der XRechnung, PDF-Klausel-Check, Verzicht/5-Jahres-Bindung, §14c-Schutz | 2026-05-26 |
| [`architecture/adr-index.md`](architecture/adr-index.md) | ADR-Index (0001–0041) mit Querverweisen | 2026-10-27 (Lauf 2026.11, U6) |
| [`architecture/modules.md`](architecture/modules.md) | Modul-Referenz je Verzeichnis | 2026-10-24 (Phase W, inkl. `server/*` §15 und Wurzel-Module §16), Nachträge Lauf F und Lauf 2026.11 (Lesefelder, `assets_disposal_preview`) |
| [`architecture/data-model.md`](architecture/data-model.md) | Tabellen, GoBD-Trigger, Migrations-Liste, abgeleitete Lesefelder (§2g) | 2026-10-27, Schema v32 (inkl. 0032 GoBD-Absicherungen, ON DELETE; Lesefelder Lauf 2026.11) |
| [`architecture/handbook.md`](architecture/handbook.md) | Aufbau und Konventionen des eingebauten Handbuchs | — |
| [`architecture/design.md`](architecture/design.md) | Designleitlinie: Tokens, Bausteine, Status-Wörter, Muster (Listen, Detailseiten, Editoren, Bestätigungen), Responsive, deutsche Formate | 2026-10-26 (Lauf 2026.11, U4) |
| [`architecture/conventions.md`](architecture/conventions.md) | Doku-Konventionen (Sprache, Aufbau, „Letzte Verifikation") | 2026-05-26 |

**Lese-Reihenfolge.** Wer das System zum ersten Mal versteht, liest diese Datei
komplett, dann gezielt das Vertiefungs-Dokument zum eigenen Thema; für den
Betrieb `docs/operations-guide.md`. Wer ein Modul ändert, springt in den Code,
`modules.md` und das jeweilige ADR.

---

## Letzte Verifikation

Stand: 2026-10-25 (Lauf F, Block F7; Version 2026.10.0, Schema v32, ADRs
0001–0040; Zähl- und Versionsangaben nachgezogen, Inhalte von Phase W wie
W13 geprüft). Quellen: `src-tauri/src/server/` (`mod.rs`, `startup.rs`,
`auth.rs`, `policy.rs`, `health.rs`), `src-tauri/src/{app_ctx,config,seed,uploads,downloads,secrets}.rs`,
`src-tauri/src/scheduler/tick.rs`, `src-tauri/src/db/mod.rs`,
`src-tauri/src/commands/{registry,backup}.rs`, `src-tauri/src/einvoice/`,
`Dockerfile`, ADR 0039. Nachtrag 2026-10-27 (Lauf 2026.11, U6; Version 2026.11.0,
ADRs 0001–0041): Stand-Zeile, ADR-Zählung, §2g Oberfläche (gegen `src/lib/`,
`src/lib/styles/tokens.css`, `src/routes/+layout.svelte`, `pageTitle.svelte.ts`,
`routeHelp.ts`, `static/manifest.webmanifest` und ADR 0041 geprüft), §8 und §9.
