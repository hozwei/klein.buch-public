# Klein.Buch

**Selbst gehostete EÜR-Buchhaltung für deutsche §19-Kleinunternehmer.**
Im Browser, auf der eigenen Instanz, auch vom Telefon. AGPL-3.0.

Klein.Buch deckt den vollständigen Beleg-Lebenszyklus eines deutschen
Kleinunternehmers ab — Angebot, Rechnung (XRechnung + ZUGFeRD-PDF/A-3),
Eingangsrechnung per Upload, Kosten, Anlagen mit linearer AfA,
Einnahmen-Überschuss-Rechnung und Steuerberater-/ELSTER-Export — und legt dabei
besonderen Wert auf **GoBD-Konformität** und den **§19-Schutz** (kein
versehentlicher Umsatzsteuer-Ausweis).

Klein.Buch läuft als Docker-Image auf einem eigenen Server; bedient wird es im
Browser. Eine Instanz gehört genau einem Nutzer. Die Daten liegen auf der Instanz
des Betreibers, nicht bei einem Anbieter: keine Telemetrie, keine Cloud-API, kein
Auto-Update. Datenbank und Backups sind mit der eigenen Passphrase verschlüsselt,
die Anmeldung verlangt zusätzlich einen zweiten Faktor (TOTP).

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
- Wer seine Buchhaltung auf eigener Hardware bzw. einem eigenen Server
  betreiben will und kein Cloud-SaaS möchte.
- Wer XRechnungen empfangen und ausstellen muss (E-Rechnungs-Pflicht
  seit 2025).

Nicht gedacht für: bilanzierungspflichtige Unternehmen, Vereine mit
Spenden-Workflow, regelmäßige Auslands-B2B-Geschäfte mit Reverse-Charge
über §13b hinaus, mehrere Nutzer oder Mandanten auf einer Instanz.

Für den Betrieb braucht es einen Linux-Server (x86-64) mit Docker und einen
Reverse-Proxy mit TLS. Wer das nicht selbst betreiben will, braucht jemanden,
der es tut.

---

## Schnellstart mit Docker Compose

Voraussetzungen: Linux-Host `linux/amd64` mit Docker und Docker Compose, ein
eigener Hostname (Klein.Buch läuft nur an der Wurzel eines Hosts, z. B.
`https://buch.example.org/`, nicht unter einem Pfad), ein Reverse-Proxy mit TLS
für diesen Hostname, lokale Datenträger für Datenbank und Archiv (kein NFS/SMB).

1. **Verzeichnisse anlegen.** Sie müssen der UID gehören, unter der der Container
   läuft (im Beispiel `1000:1000`):

   ```sh
   sudo install -d -o 1000 -g 1000 /srv/klein-buch/data /srv/klein-buch/inputs
   ```

2. **Compose-Datei anlegen.** `compose.example.yaml` als `compose.yaml` kopieren
   und anpassen:
   - `image:` auf eine **feste Version** setzen,
     `ghcr.io/hozwei/klein-buch:<Version>` (Versionen auf der
     [Releases-Seite](https://github.com/hozwei/klein.buch-public/releases);
     es gibt kein `:latest`); zu jeder Version liegt dort auch der
     Quelltext aller Komponenten Dritter (GPL, LGPL, MPL, EPL, EUPL u. a.) als
     `klein-buch-<Version>-quellen-dritter.tar.zst`;
   - `KLEIN_BUCH_PUBLIC_URL` auf die öffentliche Adresse;
   - `KLEIN_BUCH_TRUSTED_PROXIES` auf die Adresse, unter der der Reverse-Proxy
     beim Container ankommt (`docs/operations-guide.md` §6);
   - Volumes und `user:` an den eigenen Host anpassen; für Off-Site-Backups je
     Ziel ein Ordner unter `/backup-targets/…`.

3. **Signatur prüfen** (empfohlen, `SECURITY.md`):

   ```sh
   cosign verify ghcr.io/hozwei/klein-buch:<Version> --certificate-identity-regexp '^https://github\.com/hozwei/klein\.buch-public/\.github/workflows/release-image\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$' --certificate-oidc-issuer https://token.actions.githubusercontent.com
   ```

4. **Starten und den Setup-Token aus dem Log holen:**

   ```sh
   docker compose up -d
   docker compose logs klein-buch | grep Setup-Token
   ```

   Der Token steht nur im Container-Log und gilt bis zum nächsten Neustart.

5. **Im Browser einrichten:** die öffentliche Adresse öffnen, Setup-Token
   eingeben, ein Daten-Passwort (Passphrase, mindestens 16 Zeichen) festlegen,
   den QR-Code mit einer Authenticator-App scannen und den Code bestätigen. Die
   zehn Einmal-Codes sicher aufbewahren. Das Daten-Passwort ist zugleich der
   Schlüssel der Datenbank und der Backups: **Ohne es sind die Daten verloren**,
   es gibt keinen Wiederherstellungsweg.

6. **Off-Site-Backup einrichten:** Einstellungen → Backups → Externe Ziele
   (Ordner-Ziel unter `/backup-targets` oder SFTP-Ziel). Ohne Ziel liegen alle
   Backups auf demselben Volume wie die Daten.

Nach jedem Neustart des Containers ist die Instanz **gesperrt**, bis sich jemand
im Browser anmeldet; erst dann laufen Tages-Backup, Abo-Rechnungen und die
übrigen geplanten Aufgaben (ADR 0039, E3).

Ausführlich: Handbuch-Kapitel `installation-docker`, `reverse-proxy-und-tls` und
`volumes-und-rechte` (unter `src-tauri/resources/handbook/`, in der App unter
Hilfe) und das Betriebshandbuch `docs/operations-guide.md`.

---

## Features

**Im Browser, auch mobil** — eine Instanz, ein Datenbestand, erreichbar von
jedem Gerät mit Browser; Kernabläufe (Anmelden, Rechnung bis PDF, Zahlung,
Ausgabe mit Fotobeleg) auf dem Telefon bedienbar.

**Stammdaten** — Firmendaten mit §19-Schalter (inklusive
5-Jahres-Bindungs-Warnung beim Verzicht), Kunden- und Lieferanten-Verwaltung,
optionales Firmenlogo, anpassbare Kilometer-Sätze für die Anfahrt-Abrechnung.

**Angebote** — anlegen, festschreiben, annehmen, in Rechnungen wandeln.
Versand als PDF inklusive verknüpfter AGB- und Datenschutz-Version (zentral und
unveränderlich versioniert).

**Rechnungen** — XRechnung (UN/CEFACT CII) und ZUGFeRD-PDF/A-3 in einem
Schritt, §19-Klausel als Pflichtangabe sowohl auf dem PDF als auch in BT-22 der
XML, KoSIT-validiert beim Ausstellen, Storno als eigenständiger Gegen-Beleg,
Cash-Basis-Zahlungen mit Teilzahlung über Jahresgrenzen hinweg.

**Pakete & Anfahrt** — wiederverwendbare Leistungs-Pakete mit
Markdown-formatierter Beschreibung, Versionierung mit eingefrorenen
Beleg-Kopien (keine nachträgliche Verfälschung), Kilometer-basierte
Anfahrt-Positionen, optionaler Paket-Katalog als Broschüre.

**Abo-Rechnungen** — wiederkehrende Ausgangsrechnungen mit drei
Automatik-Stufen (nur Entwurf / festschreiben / festschreiben und versenden);
der Scheduler prüft alle fünf Minuten und holt verpasste Perioden nach.

**Eingangs-Rechnungen** — XRechnung (CII + UBL) und ZUGFeRD per Upload im
Browser lesen, KoSIT-prüfen (beratend), Original GoBD-konform archivieren, als
Kosten-Position übernehmen; Anzeige der Roh-XML. Nur die seit 2025 als
E-Rechnung zulässigen ZUGFeRD-Profile (`EN16931`, `EXTENDED`, `XRECHNUNG`);
`MINIMUM` und `BASIC-WL` werden mit klarer Meldung abgelehnt.

**Kosten** — geschäftliche Ausgaben mit Beleg als PDF oder Bild (auch direkt mit
der Kamera), DATEV-Kontenrahmen-Mapping (SKR03 als Standard), Privatbewegungen
(EÜR-neutral), Storno-Korrektur, wiederkehrende Vorlagen.

**Anlagen & AfA** — Anlagenverzeichnis, lineare Abschreibung, amtliche
AfA-Tabelle als pflegbares JSON, Ausbuchung beim Verkauf oder Verschrotten mit
Buchwert-Auflösung.

**EÜR & Export** — Cash-Basis-Aggregation nach §4 Abs. 3 / §11 EStG,
ELSTER-Ausfüllhilfe mit zeilengenauen Werten und eigener „Anlage EÜR“-PDF,
DATEV-Buchungsstapel (EXTF, SKR03/SKR04), Steuerberater-Paket als ZIP mit
Hash-Verifikation — alles als Browser-Download.

**Geschäftsjahres-Abschluss** — prüfungssicherer Jahres-Lock nach §146 AO,
automatischer AfA-Lauf zum Jahreswechsel (abschaltbar), Festschreibungs-
Protokoll, Auto-Backup.

**Versand** — SMTP mit TLS/STARTTLS (auch Microsoft 365 über SMTP);
Zugangsdaten verschlüsselt in der Datenbank. Append-only-Versandprotokoll mit
Provider-Antwort.

**Hinweise** — Inbox in der App für fällige Backups, fehlende oder gescheiterte
Off-Site-Sicherungen, Mail-Fehler, anstehende AfA-Läufe und (opt-in) neue
Versionen. Pro Regel ein- und ausschaltbar.

**Sicherheit & Nachvollziehbarkeit** — Anmeldung mit Passphrase und TOTP plus
Einmal-Codes, Re-Auth vor kritischen Aktionen, SQLCipher-verschlüsselte
Datenbank (Passphrase = Datenbank-Schlüssel = Backup-Schlüssel), write-once
Beleg-Archiv mit SHA-256-Prüfung bei jedem Abruf, append-only Audit-Log,
verschlüsselte Backups (Argon2id + AES-256-GCM) mit Pflicht-Sicherung vor jedem
Restore und vor jeder Datenbank-Umstellung.

**Backup-Ziele** — lokaler Floor im Daten-Volume plus beliebig viele Off-Site-Ziele
gleichzeitig: Ordner unter `/backup-targets` (z. B. NAS-Freigabe oder der Ordner
eines Sync-Clients) und SFTP mit Host-Key-Prüfung. Aufbewahrung je Ziel, Status
je Ziel, Backup-Protokoll als append-only Liste.

**PDF-Layouts** — vier mitgelieferte Vorlagen (Standard, Modern, Klassisch,
Minimal), alle §19-konform mit Pflicht-Klausel und Pflichtangaben, mit Vorschau;
eigene Vorlagen im Volume `/inputs`.

**Eingebautes Handbuch** — Benutzer- und Betriebshandbuch in der App (F1 oder
Menü → Hilfe) mit Volltext-Suche und Glossar.

---

## Betrieb

- **Betriebshandbuch:** `docs/operations-guide.md` — Umgebungsvariablen, Volumes,
  Reverse-Proxy, Backups, Updates, Restore, Logs, Ressourcen.
- **Handbuch-Kapitel für Betreiber** (`src-tauri/resources/handbook/<Kapitel>.md`):
  `installation-docker`, `reverse-proxy-und-tls`, `volumes-und-rechte`,
  `totp-einrichten-und-wiederherstellen`, `updates-und-rueckfall`,
  `disaster-recovery`, `speicherbedarf`, `fehlersuche-im-container`,
  `lizenz-und-quellcode`.
- **Updates** löst der Betreiber in seiner Container-Verwaltung aus (Tag ändern,
  neu starten); vor jeder Datenbank-Umstellung legt Klein.Buch selbst ein
  geprüftes Backup an.

Die Windows-Desktop-Releases v2026.5.0 und v2026.5.1 sind die letzten
Desktop-Versionen. Einen Datenumzug von der Desktop-App in die Docker-Fassung gibt
es nicht (ADR 0039, E17).

---

## Stack

| Bereich | Technologie |
|---|---|
| Server | Rust (axum + tower-http), ein Binary `klein-buch-server` |
| Frontend | Svelte 5 (Runes) + TypeScript, SvelteKit als SPA, vom Server ausgeliefert |
| Datenbank | SQLite mit WAL + STRICT-Tables, SQLCipher-verschlüsselt |
| PDF-Erzeugung | Typst (Vorlagen in `inputs/pdf-templates/`) |
| E-Rechnung | Mustang (ZUGFeRD/PDF-A-3) + KoSIT-Validator, Java-Sidecar mit jlink-Laufzeit |
| Mail-Versand | lettre (SMTP) |
| Crypto | SQLCipher für die DB, Argon2id + AES-256-GCM für Backups, TOTP (RFC 6238) |
| Image | Debian trixie-slim, Nicht-root, read-only Root-FS, `linux/amd64` |
| Tests | cargo test (Unit + Integration), vitest, Smoke-Test des Images |
| CI | GitHub Actions |

Architektur: **Functional Core / Imperative Shell**. Pure Funktionen in
`domain/`, `einvoice/{parser,generator}`, `depreciation::compute`,
`euer::aggregate`, `pdf::klausel_check`. I/O in `commands/`, `db/`, `archive/`,
`mail/`, `scheduler/`, `backup/`, `migration_export/`; HTTP in `server/`.

---

## Entwicklung

**Voraussetzungen:**
- Rust in der Version aus `src-tauri/rust-toolchain.toml`
- Node ≥ 22, pnpm (Version aus `packageManager` in `package.json`)
- für das Image: Docker mit BuildKit

```bash
# Frontend-Abhängigkeiten installieren und einmal bauen
# (der Server liest build/index.html für die CSP-Hashes)
pnpm install
pnpm build

# Entwicklung: Server auf 127.0.0.1:8080, Vite-Dev-Server auf 127.0.0.1:5173
# (leitet /api, /files, /healthz und /legal an den Server weiter).
# RESOURCE_DIR=$PWD: ./inputs ist die Seed-Quelle, kopiert wird ins
# Entwicklungs-Volume unter /tmp; ./inputs selbst bleibt unberührt.
KLEIN_BUCH_DATA_DIR=/tmp/klein-buch-dev/data \
KLEIN_BUCH_INPUTS_DIR=/tmp/klein-buch-dev/inputs \
KLEIN_BUCH_RESOURCE_DIR=$PWD KLEIN_BUCH_WEB_DIR=$PWD/build \
KLEIN_BUCH_ALLOW_UNMOUNTED=1 KLEIN_BUCH_LISTEN=127.0.0.1:8080 \
KLEIN_BUCH_PUBLIC_URL=http://127.0.0.1:5173 KLEIN_BUCH_INSECURE_HTTP=1 \
KLEIN_BUCH_SIDECAR_MOCK=1 \
  cargo run --manifest-path src-tauri/Cargo.toml --bin klein-buch-server
pnpm dev

# Tests (in UTC wie die CI; der Server setzt TZ=UTC selbst)
TZ=UTC cargo test --manifest-path src-tauri/Cargo.toml
pnpm check                  # svelte-check (TypeScript)
pnpm test                   # vitest

# Image bauen (Build-Kontext ist dieser Ordner)
docker build -t klein-buch:dev .
```

`KLEIN_BUCH_SIDECAR_MOCK=1` ersetzt Mustang/KoSIT durch einen Mock (nur in
Debug-Builds; ein Release-Build mit gesetztem Schalter startet nicht).
`KLEIN_BUCH_ALLOW_UNMOUNTED=1` (keine Volume-Prüfung, fehlende Verzeichnisse
werden angelegt) und `KLEIN_BUCH_INSECURE_HTTP=1` sind nur für lokale Tests
gedacht. Den Java-Sidecar (`src-tauri/sidecar/`: KoSIT-Validator,
Mustang, XRechnung-Konfiguration, Launcher) baut das `Dockerfile` mit einer
jlink-Laufzeit zusammen. Build, CI und Release: `docs/architecture/build-and-release.md`.

---

## Repository-Struktur

```
├── Dockerfile              Image-Build (SPA → Server → jlink-JRE → Lizenzseite → Laufzeit)
├── compose.example.yaml    Beispiel für Docker Compose (gehärtet)
├── src/                    Svelte-5-Frontend (SPA)
│   ├── routes/             SvelteKit-Routen (Rechnungen, Angebote, Kosten,
│   │                       Pakete, Anlagen, EÜR, GJ, Einstellungen, Hilfe)
│   └── lib/                Transport (fetch), API-Funktionen, Stores, Types,
│                           Design-Tokens, Komponenten
├── src-tauri/              Rust-Crate (Name aus der Desktop-Zeit beibehalten)
│   ├── src/                Backend (Functional Core / Imperative Shell), server/
│   ├── migrations/         Forward-only SQL-Migrationen (0001–0031)
│   ├── tests/              Integrationstests
│   ├── resources/handbook/ Benutzer- und Betriebshandbuch (Markdown)
│   └── sidecar/            Java-Sidecar: JARs, XRechnung-Konfiguration,
│                           Launcher, versions.json, Lizenzinventar
├── legal/                  Erzeugung und Prüfung der Lizenzseite /legal/
├── docs/                   ARCHITECTURE, ADRs (0001–0039), Betriebshandbuch
├── inputs/                 Menschen-gepflegt: PDF-Vorlagen, AfA-Tabelle,
│                           Beispiel-Daten (Seed für das Volume /inputs)
└── .github/                CI, Image- und Release-Workflows
```

`inputs/` ist tabu für automatische Schreib-Operationen: Der Container kopiert
die mitgelieferten Dateien beim Start ins Volume `/inputs` und überschreibt dort
nie, was der Nutzer geändert hat (neue Fassungen landen als `<name>.neu`).

---

## GoBD- und §19-Hardline

Diese Punkte sind nicht abschaltbar und im Code (DB-Trigger,
Functional-Core-Guards, UI-State) erzwungen:

- Festgeschriebene Belege (Rechnungen, Stornos, Angebote, Kosten,
  AfA-Buchungen, abgeschlossene Geschäftsjahre) sind unveränderlich.
- Storno ersetzt Löschung. Ein neuer Storno-Beleg im Format
  `ST-{YYYY}-{NNNN}` neutralisiert die fehlerhafte Rechnung.
- Beleg-Archiv ist write-once mit SHA-256-Verifikation bei jedem
  Lese-Zugriff und monatlicher Integritätsprüfung.
- Audit-Log ist append-only (DB-Trigger gegen Update und Delete).
- Aufbewahrung 10 Jahre. Es gibt absichtlich kein Löschen-UI für
  einzelne Belege und keine Totallöschung in der App (der Factory Reset der
  Desktop-Fassung entfällt, ADR 0039, E7). Ein Volume zu löschen liegt in der
  Verantwortung des Betreibers.
- §19-Default: alle Umsatzsteuer-Felder UI-gesperrt, `tax_amount = 0`,
  Items mit `tax_category_code = 'E'`, die Klausel „Gemäß §19 UStG
  wird keine Umsatzsteuer ausgewiesen." steht sichtbar auf jedem PDF
  und in BT-22 der XRechnung.
- Verzicht auf §19 ist möglich, aber an einen 5-Jahres-Bindungs-
  Warn-Dialog gekoppelt (gesetzliche Folge, keine Klein.Buch-Regel).
- §14c-Schutz: bei `is_kleinunternehmer = true` verhindert die UI
  jeden versehentlichen Umsatzsteuer-Ausweis.

---

## Sicherheits-Modell

- **Passphrase = Datenbank-Schlüssel = Backup-Schlüssel.** SQLCipher leitet den
  DB-Schlüssel aus der Passphrase ab, die Backup-Hülle nutzt Argon2id. Die
  Passphrase wird nie gespeichert; nach jedem Neustart ist die Instanz gesperrt,
  bis sich der Nutzer anmeldet.
- **Zwei Faktoren.** Anmeldung mit Passphrase und TOTP (plus zehn Einmal-Codes),
  Re-Auth vor Restore, Backup-Zielen, Mailkonten, vollständigen Exporten und dem
  Wechsel von Passphrase oder TOTP.
- **Kein Recovery-Backdoor.** Passphrase-Verlust = Totalverlust per Design.
- **Zugangsdaten verschlüsselt.** SMTP- und SFTP-Passwörter liegen in der
  verschlüsselten Datenbank, nie in Logs, Audit oder Exporten.
- **Gehärteter Betrieb.** Explizite Allowlist der API, Sitzungs-Cookie
  `__Host-…` (HttpOnly, Secure, SameSite=Strict), CSP, Rate-Limits; der Container
  läuft ohne Root mit read-only Root-FS und ohne Capabilities.
- **Local-First.** Keine Telemetrie, kein Auto-Update, keine Cloud-API.
  Verbindungen nach außen nur für Versand (SMTP), Off-Site-Backup (SFTP) und die
  opt-in Versionsprüfung.

Bedrohungsmodell: `docs/architecture/security.md`. Sicherheitslücken bitte privat
melden: `SECURITY.md`.

---

## Mitwirken

Beiträge Dritter werden vorerst nicht angenommen; Fehler und Wünsche gerne als
Issue, Forks unter der AGPL willkommen. Einzelheiten: `CONTRIBUTING.md`.

---

## Dokumentation

- **Handbuch** — in der App (F1) oder unter `src-tauri/resources/handbook/`:
  Bedienung, Recht und Steuern, Betrieb, Fehlersuche, Glossar.
- **Betriebshandbuch** — `docs/operations-guide.md`.
- **Architektur** — `docs/ARCHITECTURE.md` für den Überblick,
  `docs/architecture/` für Vertiefungen (Sicherheit, GoBD, §19, Datenmodell,
  Build und Release).
- **Architecture Decision Records** — `docs/adr/` (0001–0039); der Umbau zur
  Docker-Fassung steht in ADR 0039.
- **Änderungen** — `CHANGELOG.md`.

---

## Lizenz

**AGPL-3.0-or-later.** `LICENSE` enthält den vollständigen Lizenztext der GNU
Affero General Public License, Version 3.

Klein.Buch
Copyright (C) 2026  Manuel Schmid <schmidm@wildbach-computerhilfe.de>

Urheberhinweis und Kurzfassung der Lizenzbedingungen stehen in `COPYRIGHT`,
Komponenten Dritter in `THIRD-PARTY-NOTICES.md`. Die laufende Instanz zeigt unter
`/legal/` (ohne Anmeldung) die Lizenzen und Hinweise aller eingebauten
Komponenten und die Quellangaben; der Quelltext der Komponenten Dritter liegt je
Release als Asset bei.

Wer eine veränderte Fassung als Netzdienst betreibt, muss ihren Nutzern den
Quelltext dieser Fassung anbieten (AGPL-3.0 §13). Klein.Buch verlinkt den
laufenden Quellstand auf der Anmeldeseite und im Über-Dialog; Forks setzen dafür
`KLEIN_BUCH_SOURCE_URL` (siehe `CONTRIBUTING.md`).

---

## Wer steht dahinter

Klein.Buch wird von Manuel Schmid (Wildbach Computerhilfe,
Landshut) entwickelt. Kontakt für Geschäftliches:
[schmidm@wildbach-computerhilfe.de](mailto:schmidm@wildbach-computerhilfe.de).
Für Bug-Reports und Feature-Wünsche bitte den GitHub-Issue-Tracker
dieses Repos verwenden.
