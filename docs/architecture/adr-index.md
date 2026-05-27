# ADR-Index

> Vertiefung zu „ADR-Liste in `docs/adr/`" aus `../ARCHITECTURE.md`. Diese
> Datei ist die navigations-freundliche Sicht auf alle 38 Architecture
> Decision Records: Titel, Themenkreis, Status, Querverweis auf das
> Architektur-Vertiefungs-Dokument, das die Entscheidung weiterführt.

Quelle: `klein-buch/docs/adr/0001-…0038-….md`. Jeder ADR-Datei-Header
gilt; wenn diese Index-Tabelle einmal hinterherhängt, ist die ADR-Datei
selbst maßgeblich.

**Status-Konvention:** alle 38 ADRs sind im Stand v1.0-RC **akzeptiert**.
Spätere Erweiterungen werden als „Amendment" im selben ADR ergänzt
(Beispiel: ADR-0035-Amendment 2026-05-24, SQLCipher-PBKDF2 statt
Argon2id-Raw-Key).

---

## 1. Tabelle aller ADRs

| Nr. | Titel | Themenkreis | Vertiefung |
|---:|---|---|---|
| 0001 | Cross-OS-Sidecar-Bundles in Block 17 statt Block 0 | Sidecar / Build | `build-and-release.md` §2 |
| 0002 | App-Shell Tauri 2 + Rust-Backend | Plattform | `../ARCHITECTURE.md` §1 |
| 0003 | SQLite (WAL + STRICT), Integer-Cents, UUIDv7 | DB-Konvention | `data-model.md` §0 |
| 0004 | Functional Core / Imperative Shell | Architektur-Stil | `modules.md` §1 (domain/) |
| 0005 | §19-Kleinunternehmer als Hardline-Default | §19 / GoBD | `paragraph-19.md` |
| 0006 | GoBD: Immutability via DB-Trigger, Storno statt Löschung | GoBD | `gobd.md` |
| 0007 | E-Rechnung als UN/CEFACT CII; KoSIT als Validator | E-Rechnung | `modules.md` §2 (einvoice/) |
| 0008 | PDF via Typst, ZUGFeRD via Mustang im Java-Sidecar | PDF / Sidecar | `modules.md` §3 (pdf/) + `../ARCHITECTURE.md` §6 |
| 0009 | Backup als First-Class-Feature (Argon2id + AES-256-GCM) | Backup | `security.md` §3 |
| 0010 | EÜR auf Cash-Basis (§4 Abs. 3 EStG) | EÜR | `modules.md` §13 (euer/) |
| 0011 | Mail (Phase 1): SMTP via lettre, Credentials im OS-Keychain | Mail | `security.md` §4 |
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
| 0028 | Versand via Microsoft Graph (OAuth/PKCE) + append-only E-Mail-Protokoll | OAuth / Mail | `security.md` §9 + `modules.md` §8 |
| 0029 | Design-System: zentrale Tokens, wiederverwendbare Komponenten, Toast-Default | Design-System | Frontend-`src/lib/styles/tokens.css` |
| 0030 | Wählbare PDF-Vorlagen: eingebettete Unified-Built-ins + inputs-Override + Sentinel-Auflösung | PDF-Templates | `modules.md` §3 + `../ARCHITECTURE.md` §4.14 |
| 0031 | Pakete (versioniert) + Anfahrt | Pakete/Anfahrt | `modules.md` §1 (domain/package, domain/travel) + `../ARCHITECTURE.md` §4.15 |
| 0032 | DSGVO: Auskunft (Art. 15) + Anonymisierung (Art. 17) | DSGVO | `modules.md` §1 (domain/dsgvo, domain/anonymize) + `../ARCHITECTURE.md` §4.16 |
| 0033 | Wiederkehrende Ausgangsrechnungen (Abo-Rechnungen) | Abo-Rechnungen | `modules.md` §1 (domain/recurring_invoice) + `../ARCHITECTURE.md` §4.17 |
| 0034 | Backup-Ziele, Aufbewahrung + Backup-Log | Backup-Tiers | `security.md` §3 + `data-model.md` §2e (backup_log) |
| 0035 | Verschlüsselung at Rest + Passphrase = App-Login | SQLCipher | `security.md` §2 |
| 0036 | Factory Reset (vollständiges Zurücksetzen) | Factory Reset | `security.md` §6 + `gobd.md` §7 |
| 0037 | E-Rechnungs-Empfangs-Politur + Drop-Folder (PV1-A2/A5/DROP) | E-Rechnung-Empfang + Drop-Folder | `modules.md` §2 (einvoice/) + §6 (scheduler/drop_folder) |
| 0038 | Release-Bundle NSIS only (MSI verworfen wg. CalVer-Jahr vs. MAJOR-255-Cap) | Release / Build | `build-and-release.md` (G4.3) |

---

## 2. Thematische Gruppierung

Wer ein Thema verstehen will, hier die ADR-Cluster:

**Foundation / Architektur-Stil:** 0002, 0003, 0004, 0013, 0014, 0015.

**GoBD-Substanz:** 0006, 0027 (GJ-Lock), 0036 (Factory Reset als Ausnahme).
Plus auf Beleg-Ebene: 0016 (Quote-Lock), 0019 (Expense sofort gelockt),
0020 (Private Movements immutable), 0025 (AfA-Lock zum GJ-Wechsel).

**§19 und USt:** 0005 (§19-Hardline), 0019 (eingangsseitig).

**E-Rechnung:** 0007 (CII), 0008 (PDF/A-3 + Mustang), 0024 (Empfang),
0037 (Empfangs-Politur + Drop-Folder, ZUGFeRD-Profile-Whitelist, Roh-XML-
Viewer). Plus Sidecar: 0001 (Sidecar-Bundle-Block), 0008.

**Backup + Security:** 0009 (Basis), 0034 (Tiers + Log), 0035 (At-Rest-
Encryption), 0036 (Factory Reset). Plus Mail-Credentials: 0011, 0028
(OAuth-Refresh-Token-Chunking).

**EÜR + Export:** 0010 (Cash-Basis), 0022 (Zufluss/Abfluss), 0026 (Export).
Plus AfA: 0025.

**Belege:** 0012 (Nummernformat), 0016/0017 (Quote-Lifecycle + Konvertierung),
0018 (Legal-Docs), 0030 (PDF-Templates), 0031 (Pakete + Anfahrt), 0033
(Abo-Rechnungen).

**DSGVO:** 0032.

**UX/Design:** 0021, 0029.

**Scheduler/Notifications:** 0023, 0027, 0037 (Drop-Folder als sechster
Tick-Job, Inbox-only-Regeln).

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

Bei einem **echten Reversal** einer ADR (eine Entscheidung wird rückgängig
gemacht) wird eine neue ADR mit höherer Nummer eröffnet, die die alte
explizit ersetzt. Das ist bisher noch nicht passiert.

---

## 4. Lesepfade

**Für neue Maintainer:** 0002 → 0004 → 0003 → 0006 → 0005 → 0009 → 0035 →
0036. Dann nach Themen-Interesse weiter.

**Für Steuerberater-Audit:** 0006, 0005, 0010, 0019, 0022, 0025, 0026,
0027, 0032.

**Für Security-Audit:** 0009, 0011, 0013, 0028, 0034, 0035, 0036.

**Für UX-/Frontend-Arbeit:** 0021, 0029, 0030.

---

## Letzte Verifikation

Stand: 2026-05-27, ADR-Files 0001–0037. Bei einer neuen ADR diese Tabelle
am Ende verlängern und das passende Vertiefungs-Dokument verlinken.
