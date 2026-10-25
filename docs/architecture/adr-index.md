# ADR-Index

> Vertiefung zu „ADR-Liste in `docs/adr/`" aus `../ARCHITECTURE.md`. Diese
> Datei ist die navigations-freundliche Sicht auf alle 40 Architecture
> Decision Records: Titel, Themenkreis, Status, Querverweis auf das
> Architektur-Vertiefungs-Dokument, das die Entscheidung weiterführt.

Quelle: `klein-buch/docs/adr/0001-…0040-….md`. Jeder ADR-Datei-Header
gilt; wenn diese Index-Tabelle einmal hinterherhängt, ist die ADR-Datei
selbst maßgeblich.

**Status-Konvention:** alle 40 ADRs sind **akzeptiert** (Stand 2026-10-24);
ADR 0002 und 0038 sind durch ADR 0039 (Web-/Docker-Betrieb) **abgelöst** und
bleiben als Historie stehen (Status-Zeile „Abgelöst durch ADR 0039“).
Spätere Erweiterungen werden als „Amendment" im selben ADR ergänzt
(Beispiel: ADR-0035-Amendment 2026-05-24, SQLCipher-PBKDF2 statt
Argon2id-Raw-Key; die Amendments 2026-10-23 zum Web-/Docker-Betrieb).

---

## 1. Tabelle aller ADRs

| Nr. | Titel | Themenkreis | Vertiefung |
|---:|---|---|---|
| 0001 | Cross-OS-Sidecar-Bundles in Block 17 statt Block 0 | Sidecar / Build | `build-and-release.md` §2 |
| 0002 | App-Shell Tauri 2 + Rust-Backend (abgelöst durch 0039) | Plattform | `../ARCHITECTURE.md` §1 |
| 0003 | SQLite (WAL + STRICT), Integer-Cents, UUIDv7 | DB-Konvention | `data-model.md` §0 |
| 0004 | Functional Core / Imperative Shell | Architektur-Stil | `modules.md` §1 (domain/) |
| 0005 | §19-Kleinunternehmer als Hardline-Default | §19 / GoBD | `paragraph-19.md` |
| 0006 | GoBD: Immutability via DB-Trigger, Storno statt Löschung | GoBD | `gobd.md` |
| 0007 | E-Rechnung als UN/CEFACT CII; KoSIT als Validator | E-Rechnung | `modules.md` §2 (einvoice/) |
| 0008 | PDF via Typst, ZUGFeRD via Mustang im Java-Sidecar | PDF / Sidecar | `modules.md` §3 (pdf/) + `../ARCHITECTURE.md` §6 |
| 0009 | Backup als First-Class-Feature (Argon2id + AES-256-GCM) | Backup | `security.md` §3 |
| 0010 | EÜR auf Cash-Basis (§4 Abs. 3 EStG) | EÜR | `modules.md` §13 (euer/) |
| 0011 | Mail (Phase 1): SMTP via lettre, Credentials im OS-Keychain (Amendment 2026-10-23: seit W8 in der Tabelle `secrets` der SQLCipher-DB, Migration 0031) | Mail | `security.md` §4 |
| 0012 | Doc-Number-Format `{TYP}-{YYYY}-{NNNN}` pro Geschäftsjahr | Belegnummern | `modules.md` §1 (domain/numbering) |
| 0013 | Local-First strict: keine Telemetrie, kein Auto-Update, kein Cloud-Sync | Privacy / Local-First | `../ARCHITECTURE.md` §7 |
| 0014 | Lizenz AGPL-3.0-or-later | Lizenz | `../README.md` (LICENSE) |
| 0015 | Phasen-Build (18 Blöcke / 5 Phasen), Single-Tenant strict | Build-Plan | `../ARCHITECTURE.md` §8 |
| 0016 | Angebote: eigener Belegkreis, Festschreiben = Lock → `sent` | Angebote | `modules.md` §1 (domain/quote) + `../ARCHITECTURE.md` §4.8 |
| 0017 | Angebot → Rechnung: Konvertierung nur aus `accepted`, erzeugt Draft | Angebote/Rechnungen | `modules.md` §1 + `../ARCHITECTURE.md` §4.8 |
| 0018 | Rechtsdokumente (AGB/Datenschutz): Versionierung + Angebots-Bundle | Legal-Docs | `data-model.md` §2d + `../ARCHITECTURE.md` §4.9 |
| 0019 | Kosten: Sofort-Festschreiben + Eingangsseitige USt | Kosten | `gobd.md` §2.3 + `modules.md` §1 (domain/expense) |
| 0020 | Privatbewegungen: EÜR-neutral, append-only, kein Storno | Privatbewegungen | `gobd.md` §2.6 + `modules.md` §1 |
| 0021 | UI-Feedback & Interaktions-Konventionen (Block DS, Feedback-Teil) | UX-Konventionen | Frontend-`src/lib/components/` |
| 0022 | EÜR-Einnahmen-Erfassung: Zufluss/Abfluss + Storno als negative Einnahme | EÜR-Cash-Basis | `modules.md` §13 + `../ARCHITECTURE.md` §4.11 |
| 0023 | Wiederkehrende Belege + In-App-Scheduler | Scheduler | `../ARCHITECTURE.md` §5 + `modules.md` §6 |
| 0024 | E-Rechnung-Empfang: CII+UBL-Parser, KoSIT beratend, Import als Kosten | E-Rechnung-Empfang | `modules.md` §2 + `../ARCHITECTURE.md` §4.10 |
| 0025 | Anlagenverzeichnis + AfA als Functional Core | Anlagen/AfA | `modules.md` §12 (assets/+depreciation/) |
| 0026 | EÜR-Export: ELSTER-Ausfüllhilfe, DATEV-Buchungsstapel, Steuerberater-Paket | EÜR-Export | `modules.md` §13 + `reference/datev-format.md` + `reference/elster-euer-formular-schema.md` |
| 0027 | Notifications, prüfungssicherer GJ-Abschluss, Integritäts-Cron | Notifications + GJ-Lock | `gobd.md` §2.4 + `modules.md` §6 + §11 |
| 0028 | Versand via Microsoft Graph (OAuth/PKCE; Code seit Lauf F6 entfernt) + append-only E-Mail-Protokoll | OAuth / Mail | `security.md` §9 + `modules.md` §8 |
| 0029 | Design-System: zentrale Tokens, wiederverwendbare Komponenten, Toast-Default | Design-System | Frontend-`src/lib/styles/tokens.css` |
| 0030 | Wählbare PDF-Vorlagen: eingebettete Unified-Built-ins + inputs-Override + Sentinel-Auflösung | PDF-Templates | `modules.md` §3 + `../ARCHITECTURE.md` §4.14 |
| 0031 | Pakete (versioniert) + Anfahrt | Pakete/Anfahrt | `modules.md` §1 (domain/package, domain/travel) + `../ARCHITECTURE.md` §4.15 |
| 0032 | DSGVO: Auskunft (Art. 15) + Anonymisierung (Art. 17) | DSGVO | `modules.md` §1 (domain/dsgvo, domain/anonymize) + `../ARCHITECTURE.md` §4.16 |
| 0033 | Wiederkehrende Ausgangsrechnungen (Abo-Rechnungen) | Abo-Rechnungen | `modules.md` §1 (domain/recurring_invoice) + `../ARCHITECTURE.md` §4.17 |
| 0034 | Backup-Ziele, Aufbewahrung + Backup-Log | Backup-Tiers | `security.md` §3 + `data-model.md` §2e (backup_log) |
| 0035 | Verschlüsselung at Rest + Passphrase = App-Login | SQLCipher | `security.md` §2 |
| 0036 | Factory Reset (vollständiges Zurücksetzen) | Factory Reset | `security.md` §6 + `gobd.md` §7 |
| 0037 | E-Rechnungs-Empfangs-Politur + Drop-Folder (PV1-A2/A5/DROP) | E-Rechnung-Empfang + Drop-Folder | `modules.md` §2 (einvoice/); Drop-Folder im Server entfallen (E11, §6 Hinweis) |
| 0038 | Release-Bundle NSIS only (MSI verworfen wg. CalVer-Jahr vs. MAJOR-255-Cap) (abgelöst durch 0039) | Release / Build | `build-and-release.md` (G4.3) |
| 0039 | Web-/Docker-Betrieb | Plattform / Betrieb | `security.md` |
| 0040 | Dauerbetrieb: Start- und Zeitlogik (Uhr im AppCtx, Abo-Läufe atomar, Nachhol-Grenze, Pause bei Uhrsprung und nach Restore, Nummernkreise nach Restore, Tages-Backup kalenderfest, Rotation nach Kalender-Eimern, SFTP ohne Löschung, Integritäts-Cron mit Sperre, gestaffelt nach dem Entsperren, Herunterfahren mit Zeitgrenze, Lock-Backup im Hintergrund, Off-Site „veraltet“, WAL beim Restore-Tausch, Anzeige in Europe/Berlin, Seed-`.neu` einmal je Fassung, nur Inbox, Texte und Doku des Dauerbetriebs) | Scheduler / Backup / Betrieb | `../ARCHITECTURE.md` §4.4 + §5, `modules.md` §6–7, `security.md` §3, `gobd.md` §5.4 |

---

## 2. Thematische Gruppierung

Wer ein Thema verstehen will, hier die ADR-Cluster:

**Foundation / Architektur-Stil:** 0002 (abgelöst), 0039, 0003, 0004, 0013,
0014, 0015.

**GoBD-Substanz:** 0006, 0027 (GJ-Lock), 0036 (Factory Reset als Ausnahme).
Plus auf Beleg-Ebene: 0016 (Quote-Lock), 0019 (Expense sofort gelockt),
0020 (Private Movements immutable), 0025 (AfA-Lock zum GJ-Wechsel).

**§19 und USt:** 0005 (§19-Hardline), 0019 (eingangsseitig).

**E-Rechnung:** 0007 (CII), 0008 (PDF/A-3 + Mustang), 0024 (Empfang),
0037 (Empfangs-Politur + Drop-Folder, ZUGFeRD-Profile-Whitelist, Roh-XML-
Viewer). Plus Sidecar: 0001 (Sidecar-Bundle-Block), 0008.

**Backup + Security:** 0009 (Basis), 0034 (Tiers + Log), 0035 (At-Rest-
Encryption), 0036 (Factory Reset), 0039 (Bedrohungsmodell Internetbetrieb).
Plus Mail-Credentials: 0011, 0028
(OAuth-Refresh-Token-Chunking der Desktop-Fassung; Graph-Code seit Lauf F6 entfernt).

**EÜR + Export:** 0010 (Cash-Basis), 0022 (Zufluss/Abfluss), 0026 (Export).
Plus AfA: 0025.

**Belege:** 0012 (Nummernformat), 0016/0017 (Quote-Lifecycle + Konvertierung),
0018 (Legal-Docs), 0030 (PDF-Templates), 0031 (Pakete + Anfahrt), 0033
(Abo-Rechnungen).

**DSGVO:** 0032.

**UX/Design:** 0021, 0029.

**Scheduler/Notifications:** 0023, 0027, 0037 (Drop-Folder als sechster
Tick-Job, Inbox-only-Regeln; im Server-Betrieb entfällt der Drop-Folder-Job,
Amendment 0037 / ADR 0039 E11), 0040 (Dauerbetrieb: Sperre/CAS/Transaktion,
Nachhol-Grenze, Pause, injizierte Uhr; nur noch die Inbox, Amendment 0027).

---

## 3. Amendments

ADRs können erweitert werden, ohne dass eine neue ADR-Nummer vergeben wird —
das bleibt der inhaltlichen Kontinuität zuliebe. Bekannte Amendments:

- **ADR 0034 Amendment 2026-05-25** — Off-Site-Spiegelung „immer zweifach"
  best-effort statt nur-beim-Start, kein weekly-Tag. Hintergrund: Manuel-
  Entscheidung 2026-05-25, vermeidet zusätzlich G1-HARDEN.2-Komplikationen.
- **ADR 0034 Amendment 2026-05-25 (zweites)** — `backup_log`-CHECK-Mengen
  an die reale Laufzeit angepasst (vermeidet G1-HARDEN.2).
- **ADR 0035 Amendment 2026-05-24** — DB-Key via SQLCipher-eigenes
  PBKDF2-HMAC-SHA512 (Salt im DB-Header) statt Argon2id-Raw-Key. Begründung:
  Self-Describing-Format, cross-OS-portabel. Backup-Hülle bleibt Argon2id.

**Amendments 2026-10-23 — Web-/Docker-Betrieb (ADR 0039):**

- **ADR 0001 / 0008** — Sidecar nur noch für Linux; Bezugsquellen mit SHA-256
  in `sidecar/versions.json`.
- **ADR 0009** — Backup-`FORMAT_VERSION` 2 (Branding aus `data_dir` im
  Content-ZIP; Restore nimmt Format 1 und 2). Seit Lauf F3a (Amendment
  2026-10-25) Format 3 mit dem Klartext-Kopf als AAD; Restore nimmt 1–3.
- **ADR 0011 / 0028** — Secrets in einer STRICT-Tabelle in SQLCipher statt im
  OS-Keychain; MS-Graph im Server-Betrieb ausgeblendet (nur SMTP), der Graph-Code ist
  seit Lauf F6 entfernt (Amendment 2026-10-25).
- **ADR 0013** — eingehender Listener, opt-in Versionsprüfung, keine
  Auto-Updates.
- **ADR 0014** — Verteilung als öffentliches Image (Lizenzseite, Quellarchiv
  Dritter, OCI-Labels, §13-Quelle konfigurierbar, Tags und Images nie löschen);
  vorerst keine Beiträge Dritter.
- **ADR 0023** — Scheduler 24/7 im Server, nach Neustart gesperrt bis zur
  Entsperrung im Browser; Tages-Backup im Tick; Drop-Folder-Job entfällt.
- **ADR 0034** — Off-Site Pflicht (Warnung, Festschreiben nicht blockiert);
  Verzeichnis-Ziele nur unter `/backup-targets` + SFTP; mehrere Ziele; keine
  OneDrive-/iCloud-Erkennung.
- **ADR 0035** — Entsperren der DB ≠ Anmeldung eines Clients: TOTP +
  Passphrase, Sitzungen, Re-Auth, Passphrase ändern; Bedrohungsmodell
  Netzwerk-Angreifer; Klartext-Übergangspfad entfällt.
- **ADR 0036** — Factory Reset im Server-Betrieb abgeschaltet.
- **ADR 0037** — Rechnungseingang per Upload im Browser statt Drop-Folder.

**Amendments 2026-10-24 — Dauerbetrieb (Lauf F, ADR 0040):**

- **ADR 0009 / 0034** — Tages-Backup kalenderfest, Aufbewahrung nach
  Kalender-Eimern, keine Löschung auf SFTP-Zielen, nach einem Restore bis zur
  Freigabe nur Floor; übernommene Floor-Backups als `manual`.
- **ADR 0023 / 0033** — Abo-Läufe mit Sperre, Transaktion und Nachhol-Grenze,
  Pause nach Uhrsprung und Restore (Teil F1a, dort beschrieben); Tages-Backup
  kalenderfest, kein Job am Start, Tests ohne OS-Notification-Bridge (F1c).
- **ADR 0027** — nur noch die Inbox (Trait `Notifier` entfernt), Integritäts-Cron
  mit Sperre und Backoff, Auto-AfA atomar, Erinnerungen in Europe/Berlin.
- **ADR 0003** — Schema-Prüfung beim Entsperren: eine neuere Datenbank lässt sich
  nicht entsperren, die Instanz bleibt gesperrt.
- **ADR 0039** — Verweise der überholten Einträge (E3, Nr. 13, 33, 44, 45, 54,
  65, 77, 91, 103, 120, E12 in der Oberfläche) auf ADR 0040.
- **ADR 0014** (Lauf E, ADR 0039 Nr. 122/123) — vorerst keine Code-Beiträge Dritter
  (Entscheidung statt offener Frage), Quellangebot für Bestandteile Dritter.

**Amendments 2026-10-25 — Lauf F3–F7:**

- **ADR 0003** (F5a) — Rechenregeln für Cent-Beträge: `i64`, USt je Steuergruppe aus der
  Nettosumme (BR-CO-17), ganzzahlige AfA, Domänengrenzen und `overflow-checks`; Obergrenze
  1 Mrd. € je einzeln erfasstem Betrag (F5-fix).
- **ADR 0008** (F5c) — ZUGFeRD-Profil XRECHNUNG passend zum eingebetteten XRechnung-XML.
- **ADR 0008** (F6b) — Typst 0.15: Daten über `json(bytes(…))`, Hinweis bei alten eigenen
  Vorlagen, Warnungstest der eingebauten Vorlagen.
- **ADR 0009** (F3a) — Backup-Format 3 mit dem Klartext-Kopf als AAD; Restore nimmt 1–3;
  Lock-Backups im Hintergrund.
- **ADR 0009** (F3c) — Pflicht-Backup vor einer Migration im Format der Vorversion.
- **ADR 0011** (F7a) — SMTP-E2E-Test gegen Mailpit (per Digest) statt MailHog.
- **ADR 0024** (F5c) — strenger Parser (abgeschnittenes XML), alle Gutschrift-Codes
  abgelehnt, Rundung auf Cent.
- **ADR 0025** (F5b) — aktivierte Kosten sind keine Betriebsausgabe mehr, nur die AfA.
- **ADR 0026** (F5b) — Ausfüllhilfe nach dem Vordruck des Jahres mit Kennzahl, DATEV-Geldkonto
  je Zahlungskonto, Anlagenverkauf mit Restbuchwert.
- **ADR 0028** (F6a) — Graph-Code (`mail::oauth_ms`, OAuth-Commands) entfernt.
- **ADR 0034** (F3a) — Off-Site-Status „veraltet“, Download lokaler Backups, Rotation nur
  unter den Wurzeln.
- **ADR 0034** (F6b) — Krypto-Linie mit russh 0.64, `sha2`/`hmac`/`sha1` neu mit gleichen
  Hexwerten.

Bei einem **echten Reversal** einer ADR (eine Entscheidung wird rückgängig
gemacht) wird eine neue ADR mit höherer Nummer eröffnet, die die alte
explizit ersetzt. Erstmals geschehen am 2026-10-23: ADR 0039
(Web-/Docker-Betrieb) ersetzt ADR 0002 (App-Shell Tauri) und ADR 0038
(NSIS-Bundle); beide bleiben mit Status „Abgelöst durch ADR 0039“ stehen.

---

## 4. Lesepfade

**Für neue Maintainer:** 0002 → 0039 → 0004 → 0003 → 0006 → 0005 → 0009 →
0035 → 0036. Dann nach Themen-Interesse weiter.

**Für Steuerberater-Audit:** 0006, 0005, 0010, 0019, 0022, 0025, 0026,
0027, 0032.

**Für Security-Audit:** 0039, 0009, 0011, 0013, 0028, 0034, 0035, 0036.

**Für UX-/Frontend-Arbeit:** 0021, 0029, 0030.

---

## Letzte Verifikation

Stand: 2026-10-25, ADR-Files 0001–0040 (Amendments bis Lauf F7). Bei einer neuen
ADR diese Tabelle am Ende verlängern und das passende Vertiefungs-Dokument verlinken;
bei einem neuen Amendment eine Zeile in Abschnitt 3 ergänzen.
