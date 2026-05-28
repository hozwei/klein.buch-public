# Klein.Buch

**Lokale EÜR-Buchhaltung für deutsche §19-Kleinunternehmer.**
Offline. Ohne Cloud. AGPL-3.0.

Klein.Buch deckt den vollständigen Beleg-Lebenszyklus eines deutschen
Kleinunternehmers ab — Angebot, Rechnung (XRechnung + ZUGFeRD-PDF/A-3),
Eingangsrechnung per Datei-Ordner, Kosten, Anlagen mit linearer AfA,
Einnahmen-Überschuss-Rechnung und Steuerberater-/ELSTER-Export — und
legt dabei besonderen Wert auf **GoBD-Konformität** und den **§19-Schutz**
(kein versehentlicher Umsatzsteuer-Ausweis).

Daten liegen ausschließlich auf Deinem Rechner. Es gibt keinen Server,
kein Konto, keine Telemetrie, kein Auto-Update. Backups verschlüsselt
Klein.Buch mit Deiner eigenen Passphrase.

> **Klein.Buch ist ein Werkzeug, kein Steuerberater.** Pflichtangaben-,
> EÜR- und Export-Logik sind sorgfältig umgesetzt und getestet, ersetzen
> aber keine steuerliche Beratung. Vor dem Echteinsatz mit einem
> Steuerberater abgleichen.

---

## Für wen

- **Kleinunternehmer nach §19 UStG** (Vorjahres-Umsatz ≤ 25.000 €,
  laufendes Jahr ≤ 100.000 €).
- Einzelunternehmer, Freiberufler, Kleingewerbe mit EÜR-Pflicht
  (§4 Abs. 3 EStG).
- Wer seine Buchhaltung lokal halten will und kein Cloud-SaaS möchte.
- Wer XRechnungen empfangen und ausstellen muss (E-Rechnungs-Pflicht
  seit 2025).

Nicht gedacht für: bilanzierungspflichtige Unternehmen, Vereine mit
Spenden-Workflow, regelmäßige Auslands-B2B-Geschäfte mit Reverse-Charge
über §13b hinaus.

---

## Features

**Stammdaten** — Verkäuferprofil mit §19-Schalter (inklusive
5-Jahres-Bindungs-Warnung beim Verzicht), Kunden- und Lieferanten-
Verwaltung, optionales Firmenlogo, anpassbare Kilometer-Sätze für die
Anfahrt-Abrechnung.

**Angebote** — anlegen, festschreiben, annehmen, in Rechnungen wandeln.
Versand als PDF inklusive verknüpfter AGB- und Datenschutz-Version
(zentral und unveränderlich versioniert).

**Rechnungen** — XRechnung (UN/CEFACT CII) und ZUGFeRD-PDF/A-3 in einem
Schritt, §19-Klausel als Pflichtangabe sowohl auf dem PDF als auch in
BT-22 der XML, KoSIT-validiert beim Ausstellen, Kleinbetragsrechnung
nach §33 UStDV, Storno als eigenständiger Gegen-Beleg, Cash-Basis-
Zahlungen mit Teilzahlung über Jahresgrenzen hinweg.

**Pakete & Anfahrt** — wiederverwendbare Leistungs-Pakete mit
Markdown-formatierter Beschreibung, Versionierung mit eingefrorenen
Beleg-Kopien (keine nachträgliche Verfälschung), Kilometer-basierte
Anfahrt-Positionen, optionaler Paket-Katalog als Broschüre.

**Abo-Rechnungen** — wiederkehrende Ausgangsrechnungen mit drei
Automatik-Stufen (nur Entwurf / festschreiben / festschreiben und
versenden), 5-Minuten-Scheduler mit Catch-up nach App-Pause.

**Eingangs-Rechnungen** — XRechnung (CII + UBL) und ZUGFeRD lesen,
KoSIT-prüfen (beratend), Original GoBD-konform archivieren, als
Kosten-Position übernehmen. Optional über einen überwachten Ordner
("Rechnungs-Eingang"), der auch Cloud-synchron befüllbar ist
(OneDrive, iCloud, Dropbox, Nextcloud). Anzeige der ursprünglichen
Roh-XML einer empfangenen Rechnung über einen Detail-Dialog.

**Profile-Whitelist** — nur die seit 2025 als gültige E-Rechnung
zugelassenen ZUGFeRD-Profile (`EN16931`, `EXTENDED`, `XRECHNUNG`).
`MINIMUM` und `BASIC-WL` werden mit klarer Meldung abgelehnt.

**Kosten** — geschäftliche Ausgaben mit Beleg-Anhang als PDF oder Bild,
DATEV-Kontenrahmen-Mapping (SKR03 als Standard), Privatbewegungen
(EÜR-neutral), Storno-Korrektur, wiederkehrende Vorlagen mit
Fälligkeits-Erinnerung.

**Anlagen & AfA** — Anlagenverzeichnis, lineare Abschreibung, amtliche
AfA-Tabelle als pflegbares JSON, Ausbuchung beim Verkauf oder
Verschrotten mit Buch-Wert-Auflösung.

**EÜR & Export** — Cash-Basis-Aggregation nach §4 Abs. 3 / §11 EStG,
ELSTER-Ausfüllhilfe mit Zeilen-genauen Werten und eigener "Anlage
EÜR"-PDF, DATEV-Buchungsstapel (EXTF, SKR03/SKR04), kompletter
Steuerberater-Export als ZIP mit Hash-Verifikation.

**Geschäftsjahres-Abschluss** — prüfungssicherer Jahres-Lock nach §146
AO, automatischer AfA-Lauf zum Jahreswechsel (abschaltbar), Festschreibungs-
Protokoll, Auto-Backup mit Auslöser-Markierung.

**Versand** — SMTP (Passwort im OS-Schlüsselbund) oder Microsoft 365
Exchange Online via Graph API mit OAuth 2.0 + PKCE (Refresh-Token
ebenfalls im Schlüsselbund). Append-only-Versandprotokoll mit
Provider-Antwort.

**Hinweise** — In-App-Liste plus optionale Windows-Toast-Benachrichtigungen
für fällige Backups, überfällige Off-Site-Sicherungen, eingegangene
E-Rechnungen, Mail-Versand-Fehler, abgelaufene OAuth-Token und
anstehende AfA-Läufe. Pro Regel ein- und ausschaltbar.

**Sicherheit & Nachvollziehbarkeit** — SQLCipher-Datenbank-
Verschlüsselung mit Passphrase = App-Login = DB-Key, write-once
Beleg-Archiv mit SHA-256-Integritätsprüfung, append-only Audit-Log,
verschlüsselte Backups (Argon2id-Schlüssel-Ableitung + AES-256-GCM
Hülle) mit Pre-Restore-Pflicht-Sicherung, Factory Reset mit
Export-Pflicht oder Aufbewahrungs-Quittung.

**Backup-Ziele** — lokale Verzeichnisse, USB-Sticks, Netzlaufwerke,
Cloud-Sync-Ordner (OneDrive, iCloud, Dropbox, Nextcloud) und SFTP
(mit Host-Key-Verifikation). Auto-Detect typischer Cloud-Pfade,
Retention pro Ziel (Tage/Wochen/Monate), Off-Site-Markierung,
Backup-Protokoll als append-only Liste.

**PDF-Layouts** — vier mitgelieferte Vorlagen (Standard, Modern,
Klassisch, Minimal), alle §19-konform mit Pflicht-Klausel und
Pflichtangaben, mit Live-Vorschau.

**Eingebautes Handbuch** — vollständiges Benutzer-Handbuch in der App
(F1-Taste oder Hauptmenü → Hilfe). 43 Kapitel in sechs Kategorien
(Erste Schritte, Bedienen, Recht und Steuern, FAQ, Troubleshooting,
Glossar) mit Volltext-Suche. Plain-Language, ohne Fachjargon-Wüsten.

---

## Status

**v1.0-Release in Vorbereitung** (Windows-only, macOS-Support
verschoben auf späteren Release).

Funktional vollständig:
- Phasen 1 (Walking Skeleton), 2A (Angebote), 2B (Kosten + Recurring
  + E-Rechnung-Empfang), 2C (Anlagen + AfA + EÜR), 2D (Notifications
  + OAuth + Polish), 3 (Pakete + Anfahrt), 4 (Abo-Rechnungen)
- G-Säulen (Backup-Härtung, Tech- und User-Doku, UX-Politur)
- PV1-Phase (ZUGFeRD-Profil-Whitelist, Roh-XML-Viewer, Rechnungs-
  Eingangs-Ordner, UI-Sprach-Sweep)
- R-Reviews 1–6 (Layer-weiser Quality-Audit, ~600 Findings, alle
  S1-Fixes umgesetzt)

Offen vor v1.0-Tag: finale Release-Validierung, Code-Signing-Setup,
GitHub-Release-Workflow.

Aktuelle App-Version: `2026.5.1` (CalVer). Schema-Stand: Migration
0030. ADR-Stand: 0001–0037.

---

## Stack

| Bereich | Technologie |
|---|---|
| Shell | Tauri 2 |
| Backend | Rust (Edition 2021) |
| Frontend | Svelte 5 (Runes) + TypeScript |
| Datenbank | SQLite mit WAL + STRICT-Tables, SQLCipher-verschlüsselt |
| PDF-Erzeugung | Typst (Templates in `inputs/pdf-templates/`) |
| E-Rechnung | Mustang (ZUGFeRD/PDF-A-3) + KoSIT-Validator, gebündelt in einem jlink-JRE-Sidecar |
| Mail-Versand | lettre (SMTP) + Microsoft Graph (OAuth 2.0 + PKCE) |
| Crypto | Argon2id + AES-256-GCM für Backups, SQLCipher für die DB |
| Tests | cargo test (Unit + Integration), Front-Matter-Verify für das Handbuch |
| CI | GitHub Actions |

Architektur: **Functional Core / Imperative Shell**. Pure Funktionen
in `domain/`, `einvoice/{parser,generator}`, `depreciation::compute`,
`euer::aggregate`, `pdf::klausel_check`. I/O in `commands/`, `db/`,
`archive/`, `mail/`, `scheduler/`, `backup/`, `migration_export/`.

---

## Build aus dem Quellcode

**Voraussetzungen:**
- Rust ≥ 1.78
- Node ≥ 20, pnpm ≥ 9
- Tauri-CLI 2.x
- JDK 21 (für den jlink-Sidecar-Build)
- Windows: MSVC Build Tools 2022, Windows SDK

```bash
# Frontend-Abhängigkeiten installieren
pnpm install

# Java-Sidecar einmalig bauen (KoSIT-Validator + Mustang in einer jlink-JRE)
pwsh ../scripts/build-sidecar.ps1

# Dev-Mode mit Hot-Reload
pnpm tauri dev

# Production-Bundle (NSIS-Installer + MSI)
pnpm tauri build

# Tests
cargo test --manifest-path src-tauri/Cargo.toml
pnpm check                  # svelte-check (TypeScript)
pnpm lint
```

Der Sidecar liegt nach dem Build unter
`src-tauri/binaries/klein-buch-java-{target_triple}/`. Im Repo
ausgeliefert ist nur das Windows-Triple; andere Triples entstehen
über GitHub Actions in einer Matrix.

---

## Repository-Struktur

```
klein-buch/
├── src/                    Svelte-5-Frontend
│   ├── routes/             SvelteKit-Routen (Rechnungen, Angebote, Kosten,
│   │                       Pakete, Anlagen, EÜR, GJ, Einstellungen, Hilfe)
│   └── lib/                API-Bridge, Stores, Types, Design-Tokens,
│                           wiederverwendbare Komponenten
├── src-tauri/
│   ├── src/                Rust-Backend (Functional Core / Imperative Shell)
│   ├── migrations/         Forward-only SQL-Migrationen (0001–0030)
│   ├── tests/              Unit- und Integrationstests
│   ├── resources/handbook/ Benutzer-Handbuch (43 Markdown-Kapitel)
│   └── binaries/           jlink-JRE-Sidecar (KoSIT + Mustang)
├── docs/                   ARCHITECTURE, ADRs (0001–0037), Guides
├── inputs/                 Menschen-gepflegt: PDF-Templates, AfA-Tabelle,
│                           Beispiel-Daten, Logo
└── data/                   Maschinen-generiert: DB, Beleg-Archiv,
                            Backups, Vorschau-Renderings (gitignored)
```

`inputs/` ist tabu für automatische Schreib-Operationen (Manuel-
gepflegte Domain-Inputs). PDF-Templates dürfen direkt editiert werden,
müssen aber inhaltlich konsistent gehalten werden.

---

## GoBD- und §19-Hardline

Diese Punkte sind nicht abschaltbar und im Code (DB-Trigger,
Functional-Core-Guards, UI-State) erzwungen:

- Festgeschriebene Belege (Rechnungen, Stornos, Angebote, Kosten,
  AfA-Buchungen, abgeschlossene Geschäftsjahre) sind unveränderlich.
- Storno ersetzt Löschung. Ein neuer Storno-Beleg im Format
  `ST-{YYYY}-{NNNN}` neutralisiert die fehlerhafte Rechnung.
- Beleg-Archiv ist write-once mit SHA-256-Verifikation bei jedem
  Lese-Zugriff. Tamper- und Waisen-Erkennung.
- Audit-Log ist append-only (DB-Trigger gegen Update und Delete).
- Aufbewahrung 10 Jahre. Es gibt absichtlich kein Löschen-UI für
  einzelne Belege.
- Die einzige Total-Löschung ist der Factory Reset. Er verlangt
  vorher entweder einen vollständigen Steuerberater-Export oder
  eine getippte Aufbewahrungs-Quittung. Off-Site-Backups bleiben
  bestehen.
- §19-Default: alle Umsatzsteuer-Felder UI-gesperrt, `tax_amount = 0`,
  Items mit `tax_category_code = 'E'`, die Klausel "Gemäß §19 UStG
  wird keine Umsatzsteuer ausgewiesen." steht sichtbar auf jedem PDF
  und in BT-22 der XRechnung.
- Verzicht auf §19 ist möglich, aber an einen 5-Jahres-Bindungs-
  Warn-Dialog gekoppelt (gesetzliche Folge, keine Klein.Buch-Regel).
- §14c-Schutz: bei `is_kleinunternehmer = true` verhindert die UI
  jeden versehentlichen Umsatzsteuer-Ausweis.

---

## Sicherheits-Modell

- **Passphrase = App-Login = DB-Key = Backup-Key.** Eine Passphrase-
  Kette schützt alle drei Ebenen. SQLCipher leitet den DB-Key aus
  der Passphrase ab; die Backup-Hülle nutzt Argon2id. Ohne
  Passphrase: kein DB-Open, kein App-Zugriff, kein lesbares Backup.
- **Kein Recovery-Backdoor.** Passphrase-Verlust = Totalverlust
  per Design. Notiere die Passphrase außerhalb der App in einem
  Passwort-Manager.
- **Schlüsselbund statt Klartext.** SMTP-Passwörter, OAuth-Refresh-
  Tokens, SFTP-Passwörter liegen ausschließlich im OS-Schlüsselbund,
  nie in der Datenbank.
- **Local-First strict.** Keine Telemetrie, kein Auto-Update, kein
  Cloud-Sync. Outbound-Verbindungen nur bei explizitem User-Trigger
  (Mail-Versand) oder gegen den lokalen Sidecar (KoSIT-Validator,
  Mustang-Generator).

---

## Mitwirken

Issues und Pull-Requests willkommen. Vor einem PR:

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test`
- `pnpm check`, `pnpm lint`
- Bei Datenbank-Änderungen: Migration als Forward-only-SQL-File mit
  Schema-Versions-Bump in `app_settings`, ergänzt durch einen Test,
  der die neue Version gegen `EXPECTED_SCHEMA_VERSION` prüft.
- Bei Hardline-relevanten Änderungen (GoBD, §19, Backup): vorher
  Diskussion im Issue oder ADR.

Architektur-Entscheidungen liegen als ADRs in `docs/adr/`. Größere
neue Features starten mit einem PRD-Vorschlag.

Persönliche Daten oder echte Kunden-Daten gehören nicht in Issues
oder Tests. Beim Melden eines Fehlers bitte nur die Fehler-Meldung
und die App-Version anhängen.

---

## Dokumentation

- **Benutzer-Handbuch** — direkt in der App (F1) oder im Repo unter
  `src-tauri/resources/handbook/`. Plain-Language, 43 Kapitel,
  Volltext-Suche, Glossar.
- **Architektur-Übersicht** — `docs/ARCHITECTURE.md` für den
  Top-Level-Blick, `docs/architecture/` für Vertiefung je Thema.
- **Architecture Decision Records** — `docs/adr/` (0001–0037).
- **Release-Guide** — `docs/RELEASE-1.0-GUIDE.md` (G-Säulen,
  R-Review-Säule, Verfahren für den v1.0-Tag).
- **QA-Handbuch** — `docs/QA-HANDBUCH.md` und `docs/REVIEW-LOG.md`.

---

## Lizenz

**AGPL-3.0-or-later.** Siehe `LICENSE`.

Bei Eigen-Hosting einer veränderten Version (zum Beispiel als
Webdienst) gilt §13 AGPL: der vollständige Quelltext muss
gleichermaßen verfügbar gemacht werden. Für rein persönliche
Nutzung ohne öffentliche Bereitstellung gibt es keine zusätzliche
Pflicht.

---

## Wer steht dahinter

Klein.Buch wird von Manuel Schmid (Wildbach Computerhilfe,
Landshut) entwickelt. Kontakt für Geschäftliches:
[schmidm@wildbach-computerhilfe.de](mailto:schmidm@wildbach-computerhilfe.de).
Für Bug-Reports und Feature-Wünsche bitte den GitHub-Issue-Tracker
dieses Repos verwenden.
