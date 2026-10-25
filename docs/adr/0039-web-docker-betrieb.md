# ADR 0039 — Web-/Docker-Betrieb

**Status:** Akzeptiert · 2026-10-23 · Phase W, Block W3.

**Löst ab:** ADR 0002 (App-Shell Tauri) und ADR 0038 (NSIS-Bundle).

**Ändert per Amendment:** ADR 0001, 0008, 0009, 0011, 0013, 0014, 0023, 0028, 0034, 0035,
0036, 0037. Umsetzung in den Blöcken W4–W14 des Arbeitsplans Phase W (Projektunterlage);
W0–W2 sind abgeschlossen.

## Kontext

Klein.Buch wurde als Windows-Desktop-App gebaut (Tauri 2, NSIS-Installer, v2026.5.x). Künftig
läuft es als **öffentliches Docker-Image** ohne Tauri, wird per Browser bedient (auch mobil),
ist **aus dem Internet erreichbar** und für **genau einen Nutzer pro Instanz** gebaut. Die
Desktop-App entfällt.

ADR 0002 hatte „reines Web + lokaler Server“ verworfen: kein echtes local-first, Port- und
Prozess-Management beim Nutzer. Beides wird heute anders bewertet:

- **Anforderung geändert.** Gebraucht wird Zugriff von mehreren Geräten, auch vom Telefon
  (Fotobeleg, Zahlung erfassen), ohne eine Desktop-DB zu synchronisieren.
- **Port und Prozess** übernimmt die Container-Verwaltung des Betreibers (Neustart-Policy,
  Healthcheck, Volumes) mit Standardwerkzeugen statt eigener Logik in der App.
- **Local-first bleibt im Kern:** Die Daten liegen auf der eigenen Instanz des Betreibers, nicht
  bei einem Anbieter; keine Telemetrie, keine Cloud-API, Outbound nur auf Nutzer-Aktion oder
  opt-in (E14). „Lokal“ heißt jetzt „eigene Instanz“ statt „eigener Laptop“.
- **Die Desktop-Verteilung hatte eigene Kosten:** Windows-only-Sidecar, unsignierter Installer,
  MSI-Versionsgrenze (ADR 0038).

Der Preis ist eine neue Angriffsfläche: Das Bedrohungsmodell wechselt vom Laptop-Dieb
(ADR 0035, Amendment „gestohlener Laptop / kopierte Datei“) zum **Netzwerk-Angreifer**.

## Entscheidung

Zielarchitektur (Kurzfassung):

- **Ein Crate, ein Binary** `klein-buch-server` (axum + tower-http). Tauri, `tauri-build`,
  Plugins, `capabilities/`, `tauri.conf.json` und `keyring` entfallen.
- **`AppCtx`** statt Tauri-`State`/`AppHandle`: Pfade aus der Umgebung, schließbarer Pool samt
  Passphrase, Inbox, `SecretStore`, Scheduler-Handle, Job-Register. Die Commands werden zu
  Kernen `api::<bereich>::<fn>(ctx, args)`.
- **Transport:** ein Endpunkt `POST /api/cmd/{name}` mit **expliziter Allowlist**; je Eintrag
  `auth` (public | session | reauth), `cost` (cheap | render | jvm | kdf | backup) und
  `body_limit`. Eigene Routen für Auth, Uploads, Downloads, Jobs, `/healthz` und `/legal/`.
- **Öffentlich (ohne Sitzung) sind genau:** `GET /api/auth/status`, `POST /api/auth/setup`,
  `POST /api/auth/login`, `GET /api/public/info`, `GET /healthz` (nach außen nur 200/503),
  `/legal/` und die SPA-Assets. Alles andere verlangt eine Sitzung, DB-Commands zusätzlich den
  Zustand `unlocked` (sonst 423), kritische Aktionen eine frische Re-Auth.
- **Zustandsautomat:** `setup` (leeres `/data`) → `locked` (kein Pool, kein Scheduler) ↔
  `unlocked`. Browser-Sitzungen sind davon getrennt.
- **Frontend:** SPA (SvelteKit `adapter-static`), vom Server ausgeliefert; `invoke` wird durch
  `fetch` ersetzt.
- **Image:** Multi-Stage, Nicht-root, read-only Root-FS, Volumes `/data`, `/inputs`, optional
  `/backup-targets`, `TZ=Europe/Berlin`, Temp-Dateien unter `/data/tmp`. TLS terminiert ein
  Reverse-Proxy des Betreibers.

Die Einzelentscheidungen E1–E17:

### E1 — Zielbild: aus dem Internet erreichbar, ein Nutzer pro Instanz

**Entscheidung.** Eine Instanz dient genau einem Nutzer (einem Unternehmen) und darf aus dem
Internet erreichbar sein. **Begründung.** Die Zielgruppe (§19-Kleinunternehmer) ist eine Person;
Benutzer, Rollen und Mandanten brächten viel Angriffsfläche ohne Nutzen. **Folge.** Kein
Mehrbenutzer, keine Mandanten, kein Actor im Audit; Single-Tenant (ADR 0015) bleibt. Weil die
Instanz öffentlich erreichbar ist, gilt Härtung auf Produkt-Niveau (Bedrohungsmodell).

### E2 — Tauri entfällt, kein Parallelbetrieb

**Entscheidung.** Die Desktop-Shell wird entfernt; es gibt keine Desktop- und Server-Variante
nebeneinander. **Begründung.** Zwei Shells hießen Feature-Split (Keychain, Opener, Drop-Folder,
Factory Reset je doppelt), doppelte Tests und weiter einen Windows-Sidecar. **Folge.** Backend
direkt auf axum; NSIS-Installer und Windows-Release-Workflow entfallen (W5, W11). Damit sind
ADR 0002 und ADR 0038 abgelöst. Der Sidecar läuft nur noch unter Linux (Amendment 0001/0008).

### E3 — Nach dem Neustart gesperrt bis zur Entsperrung

**Entscheidung.** Nach jedem Start ist die Instanz `locked`, bis der Nutzer im Browser
entsperrt. **Begründung.** Passphrase = DB-Key (ADR 0035). Ein Auto-Unlock müsste Passphrase
oder Schlüssel auf dem Host ablegen; wer das Volume stiehlt, hätte dann Klartext. Das widerspricht
der Backup-Hardline „Passphrase nie speichern“. **Folge.** Solange gesperrt: kein Pool, kein
Scheduler; `locked` gilt für den Healthcheck als gesund (keine Neustartschleife). Fällige Jobs
(Tages-Backup, Abo-Rechnungen, Integritätsprüfung) holt der Scheduler nach dem Entsperren nach
(Amendment 0023).

### E4 — Erst die Desktop-Fehler, dann der Umbau

**Entscheidung.** Fehler, die schon in v2026.5.x bestanden, wurden vor dem Umbau behoben
(W1/W2). **Begründung.** Mehrere waren Voraussetzung für den Serverbetrieb (Archiv-Schreibpfad,
Backup-Konsistenz, Transaktionen, CPU-Last in der Runtime); auf grüner Testbasis bleibt der
Schnitt W4/W5 mechanisch. **Folge.** Was dabei entschieden wurde, steht unter „Entscheidungen
während der Umsetzung“.

### E5 — Login: TOTP in der App + Passphrase

**Entscheidung.** Anmeldung mit Passphrase **und** TOTP (RFC 6238), beides in der App.
**Begründung.** Eine Passphrase allein ist ein Faktor; ein vorgeschalteter Auth-Proxy wäre
zusätzliche Infrastruktur, die die App nicht erzwingen kann (secure by default). TOTP ist
offline und braucht keinen fremden Dienst. **Folge.** Entsperren ≠ Client-Authentisierung:
die Passphrase öffnet die DB, Sitzungen authentisieren den Browser. Setup atomar mit Setup-Token
aus dem Container-Log + Passphrase (≥ 16 Zeichen) + TOTP-Bestätigung; TOTP wird **vor** der KDF
geprüft, jeder Zeitschritt nur einmal; 10 Einmal-Codes (nur SHA-256 gespeichert) und
`totp-reset` über die Container-Shell; Re-Auth für kritische Aktionen; Passphrase änderbar.
Fehlversuche nur ins Log, nicht ins `audit_log`. Details W6, Amendment 0035.

### E6 — Secrets in einer STRICT-Tabelle in SQLCipher

**Entscheidung.** SMTP- und SFTP-Zugangsdaten liegen in einer STRICT-Tabelle der verschlüsselten
DB. **Begründung.** Einen OS-Schlüsselbund gibt es im Container nicht; eine eigene Datei wäre ein
zweites Geheimnis außerhalb der DB-Verschlüsselung. **Folge.** Migration `0031` (W8), `keyring`
und `libdbus` entfallen. Zugangsdaten sind an (Host, Port, Benutzer) gebunden: ändert sich eines
davon, verfällt das gespeicherte Passwort. Nicht im Migrations- und DSGVO-Export. Amendment
0011/0028.

### E7 — Factory Reset im Server-Modus abgeschaltet

**Entscheidung.** Der Factory Reset (ADR 0036) ist im Server nicht erreichbar. **Begründung.**
Über das Netz ist eine Totallöschung die gefährlichste Einzelaktion: eine gekaperte Sitzung samt
Passphrase könnte die 10-jährige Aufbewahrung vernichten. Ein Neustart ohne Daten ist im
Container einfacher und nachvollziehbarer: Der Betreiber legt eine neue Instanz mit leerem Volume
an. **Folge.** Commands in der Allowlist gesperrt (W5), Code gelöscht (W11). Die Verantwortung
für das Löschen eines Volumes liegt beim Betreiber (Handbuch). Amendment 0036.

### E8 — Off-Site-Backup ist Pflicht: Verzeichnis-Ziel + SFTP

**Entscheidung.** Off-Site-Ziele sind (a) ein Verzeichnis-Ziel unter `/backup-targets`, das der
Betreiber einbindet, z. B. den Sync-Client eines Cloud-Speichers, und (b) SFTP, z. B. ein zweites
Gerät im LAN oder ein Anbieter mit SFTP-Zugang. Mehrere Ziele gleichzeitig. **Begründung.** Die
lokalen Backups liegen auf demselben Volume wie die Daten; ein Hostausfall wäre Totalverlust.
Keine Cloud-API im Code (local-first): Sync-Client oder SFTP decken jeden Anbieter ab.
**Folge.** Verzeichnis-Ziele nur unter den Mount-Wurzeln `/backup-targets/…` (`canonicalize`);
jedes Backup auf jedes Ziel, Fehler je Ziel im `backup_log`; Status `offsite` (missing | failing |
ok) als Inbox-Warnung und Banner, das Festschreiben blockiert er nicht. OneDrive-/iCloud-Erkennung
entfällt. Amendment 0034.

### E9 — Microsoft 365: vorerst nur SMTP

**Entscheidung.** Versand nur über SMTP; der Microsoft-Graph-Versand (ADR 0028) ist im Server
ausgeblendet. **Begründung.** Der Desktop-Flow nutzte eine Loopback-Weiterleitung auf dem
eigenen Rechner. Im Server bräuchte es eine öffentliche Redirect-URI je Instanz und eine
App-Registrierung je Betreiber oder einen Device-Code-Flow, jeweils mit eigener Bedrohungsanalyse.
**Folge.** `mail_oauth_*` gesperrt (W5); Web-, Device-Code- und Paste-Flow nicht in Phase W.
Das append-only `email_log` bleibt. Amendment 0028.

### E10 — Exporte nur als Browser-Download

**Entscheidung.** Kein Command nimmt einen Server-Pfad vom Client an. **Begründung.**
Client-Pfade wären Pfad-Traversal bzw. beliebiges Schreiben auf dem Server; der Nutzer sitzt am
Browser, nicht am Server. **Folge.** Die sieben Exporte (EÜR, DATEV, ELSTER, Steuerberater-ZIP,
Detail-ZIP, Migrations-Export, DSGVO-Auskunft) entstehen unter `<data_dir>/tmp`, werden gestreamt
und nach Auslieferung bzw. nach 1 h gelöscht. Ein Test über die Registry prüft, dass kein
Args-Struct ein Pfadfeld hat (W7).

### E11 — Rechnungseingang per Upload im Browser

**Entscheidung.** Eingangsrechnungen kommen per Upload; der Drop-Folder (ADR 0037) entfällt im
Server. **Begründung.** Ein überwachter Ordner bräuchte ein weiteres Volume und Polling, halb
geschriebene Dateien wären möglich. Ein Upload ist einer Sitzung zugeordnet, größenbegrenzt und
typgeprüft. **Folge.** `POST /api/upload/{art}` (multipart, gestreamt, Magic-Bytes-Allowlist,
Limits je Art, W7); Drop-Folder gesperrt (W5), Code gelöscht (W11). Amendment 0037.

### E12 — Zeitzone fest Europe/Berlin

**Entscheidung.** `TZ=Europe/Berlin` im Image, zentrale Uhr mit `chrono-tz` im Backend,
Datumshelfer mit `Intl.DateTimeFormat(…, { timeZone: 'Europe/Berlin' })` im Frontend.
**Begründung.** Belegdatum und Geschäftsjahr bestimmen Nummernkreis und EÜR. Container laufen
meist in UTC, Browser in beliebigen Zeitzonen; vor dem Fix bekam ein Beleg zwischen 0 und 1–2 Uhr
das Vortagsdatum, in der Neujahrsnacht eine Nummer aus dem Vorjahreskreis. **Folge.** Umgesetzt
in W1 (`src-tauri/src/clock.rs`, `src/lib/date.ts`); das Geschäftsjahr bestimmt der Server aus
dem Belegdatum, ein abweichendes Client-GJ wird abgelehnt. `TZ` im Image bleibt Pflicht, weil
einige SQL-Filter `'localtime'` nutzen. *(Korrektur W9, 24.10.2026: Der Server setzt für seinen
Prozess `TZ=UTC`, die Filter rechnen ohne `'localtime'` — Nr. 94; seit W9 ignoriert der Server
ein Client-GJ — Nr. 89.)*

### E13 — Image aus GitHub Actions, öffentlich

**Entscheidung.** Das Image wird in GitHub Actions aus dem öffentlichen Repo gebaut und
öffentlich bereitgestellt. **Begründung.** AGPL-3.0 (ADR 0014): Wer Klein.Buch als Netzdienst
anbietet, muss die Quelle anbieten (§13); ein Build aus dem öffentlichen Repo ist nachprüfbar.
**Folge.** AGPL-Volltext in `LICENSE`, Lizenzseite `/legal/` (Rust, JS-Bundle, Java-Inventar,
Debian-Pakete), Quellen Dritter als Release-Asset, §13-Quelllink auf Login und „Über“
(für Forks per `KLEIN_BUCH_SOURCE_URL` überschreibbar); Signatur, SBOM, Scan, gepinnte Actions
und Images (W11, W12); Handbuch, `SECURITY.md`, `CONTRIBUTING.md` (W13). Betrieben und
aktualisiert wird das Image in der Container-Verwaltung des Betreibers. Amendment 0014.

### E14 — Updates: Hinweis in der App, Update in der Container-Verwaltung

**Entscheidung.** Die App meldet neue Versionen (opt-in) und sichert vor jeder Migration; das
Update selbst löst der Betreiber in seiner Container-Verwaltung aus. **Begründung — warum kein
Update-Knopf in der App:** Damit der Container sein eigenes Image tauschen könnte, bräuchte er
`docker.sock`, also praktisch root auf dem Host. Bei einem Dienst, der aus dem Internet
erreichbar ist, ist das ausgeschlossen. Die Versionsprüfung ist opt-in, weil ADR 0013 Outbound
nur auf ausdrückliche Entscheidung des Nutzers erlaubt. **Folge.** Abfrage der GitHub-Releases nur
nach Opt-in, keine weiteren Daten gesendet; Name und Link landen als Text (nie HTML) in der Inbox
(W12). Keine Auto-Updates. Pflicht-Backup vor Migration siehe „Standardentscheidungen“.
Amendment 0013.

### E15 — Feste Versions-Tags, Restore älterer Backups

**Entscheidung.** Images tragen nur volle Versionen (`:<CalVer>`), kein `:latest`, kein gleitender
Minor-Tag; Images und Tags `v*` werden nie gelöscht oder verschoben. Der Restore akzeptiert
Backups mit `30 ≤ schema_version ≤ EXPECTED_SCHEMA_VERSION` und migriert danach vorwärts.
**Begründung.** Mit `:latest` würde jeder Pull die DB unbemerkt und unumkehrbar migrieren, und der
Rückfall hätte keinen Namen. Nach einem Update müssen Backups der Vorversionen einspielbar
bleiben; die Vorwärtsmigration ist derselbe Weg wie beim Update. Die **Untergrenze 30** folgt aus
E17: Es gibt keine produktiven Bestände mit älterem Schema, der Serverbetrieb beginnt mit Schema
30. Ältere Stände zu unterstützen hieße, Pfade (Klartext-DB, CRLF-Migrationen, Backup ohne
Branding) für Daten zu testen, die es nicht gibt. **Folge.** Backups einer **neueren** Version
lehnt der Restore weiter ab (Down-Migration-Schutz, Schema-Hardline). Rückfall nach einem
gescheiterten Update (korrigiert 24.10.2026, W13, Nr. 115): Im bisherigen `/data` geht das nicht —
das ältere Image entsperrt eine bereits migrierte DB nicht (Schema-Check), ohne Sitzung gibt es
keinen Restore, und das neue Image migriert ein eingespieltes älteres Backup wieder vorwärts.
Der Weg ist das vorherige Image-Tag mit einem neuen, leeren `/data` (Einrichtung) und dort der
Restore des Pflicht-Backups vor der Migration (Disaster-Recovery-Weg, W7). Der Restore älterer
Backups mit Vorwärtsmigration ist seit W7 umgesetzt.

### E16 — Voll responsiv

**Entscheidung.** Die Oberfläche ist auf Telefon, Tablet und Desktop bedienbar. **Begründung.**
Eine aus dem Internet erreichbare Instanz wird unterwegs genutzt (Fotobeleg, Zahlung, Hinweise).
**Folge.** PRD NG17 „Mobile“ wird revidiert (PRD-Revision 8). Kernabläufe (Login/TOTP,
Übersicht, Rechnung anlegen bis PDF, Zahlung, Ausgabe mit Fotobeleg) mobil voll nutzbar, alle
übrigen Seiten ohne horizontales Scrollen; Prüfung bei 390×844, 768×1024 und 1280×800 (W10).

### E17 — Kein Datenumzug von der Desktop-App

**Entscheidung.** Es gibt keinen Umzug Desktop → Container; der Container startet leer. Beim
Hersteller wurden mit der Desktop-App nie echte Belege festgeschrieben, ein Desktop-Bestand
muss also nicht übernommen oder aufbewahrt werden. **Begründung.** Ein Umzug müsste absolute
Archivpfade umschreiben und `inputs/`, Branding und Secrets übernehmen: aufwendig und
fehleranfällig für ein GoBD-Archiv, ohne Daten, die das rechtfertigen. **Folge.** Der
Klartext-Übergangspfad (Adoption einer unverschlüsselten DB) wird entfernt (W6); daraus folgt die
Restore-Untergrenze 30 (E15). Wer die veröffentlichte Desktop-Version produktiv genutzt hat,
bewahrt den Bestand mit dieser Version auf; ein Importpfad ist nicht Teil von Phase W.

## Bedrohungsmodell

Angreifer ist nicht mehr der Laptop-Dieb, sondern wer die Instanz über das Netz erreicht.
Schutzgut: Steuer- und Kundendaten, die Unveränderlichkeit festgeschriebener Belege (GoBD) und
die Verfügbarkeit über 10 Jahre.

| Bedrohung | Gegenmaßnahme | Block |
|---|---|---|
| **Angreifer aus dem Internet ohne Zugang** | Explizite Allowlist (alles andere 404), öffentlich nur die in „Entscheidung“ genannten Routen; Sitzung für alles andere, DB-Commands nur `unlocked`. Fehlerabbildung ohne interne Details (500 nur mit `ref`). Setup nur mit Setup-Token aus dem Container-Log. Rate-Limit je Client-IP mit wachsender Wartezeit, ohne dauerhafte Sperre; Fehlversuche als Logzeile (fail2ban-tauglich). Body-Limits, Header-Read-Timeout, Sicherheits-Header (CSP mit Skript-Hashes, HSTS, `nosniff`, `frame-ancestors 'none'`). Start fail-closed (Mounts, Instanz-Marker, Klartext-DB, Archiv-Selbsttest). | W5, W6 |
| **Gestohlene Passphrase oder gestohlenes TOTP** (nur ein Faktor) | Login verlangt beide; TOTP vor der KDF, jeder Zeitschritt nur einmal (auch über Neustarts), konstantzeitiger Vergleich; identische Antwort bei falschem TOTP und falscher Passphrase. Einmal-Codes ersetzen nur den TOTP-Faktor. Re-Auth (beide Faktoren, 5 min) vor Restore, Backup-Zielen, Mailkonten, Steuerberater-, Detail-, Migrations- und DSGVO-Export, TOTP- und Passphrase-Wechsel. Cookie `__Host-kb_session` (HttpOnly, Secure, SameSite=Strict), neue ID bei jedem Login, Idle- und Absolut-Timeout, „Datenbank sperren“ beendet alle Sitzungen. | W6 |
| **Bösartige Uploads** (XRechnung/ZUGFeRD mit XXE, Zip-Bombe, HTML/SVG) | Limits je Art (Belege 20 MiB, E-Rechnung 10 MiB, Logo/Unterschrift 2 MiB, Restore 4 GiB), Typ per Magic Bytes gegen Allowlist, `mime_type` aus der Prüfung; SVG nur für Logo/Unterschrift. Der Rust-Parser (quick-xml) löst keine externen Entitäten auf; KoSIT und Mustang werden mit XXE-Fixtures geprüft (W14). Restore-ZIP: `enclosed_name()`, Namens-Allowlist, höchstens 200 000 Einträge, Lesemenge begrenzt, Abbruch über 200:1, keine Symlinks. Fremde Dateien nur als `attachment` mit `Content-Security-Policy: sandbox`; `inline` nur für selbst erzeugte PDFs. Freier Platz wird vorher geprüft. | W7 |
| **CSRF/XSS über den Browser** | SameSite=Strict, Pflicht-Header `X-KleinBuch-Request`, Origin- und Host-Prüfung; kein CORS. CSP ohne `'unsafe-inline'` für Skripte, `object-src 'none'`, `base-uri 'none'`. Svelte escaped standardmäßig; Update-Hinweis und Dateinamen nur als Text; `Content-Disposition` nach RFC 6266 bereinigt. Paket-Markup geht AST-only mit Escaping nach Typst (ADR 0031). | W5, W6, W7 |
| **DoS über teure Endpunkte** (KDF, JVM, Render, Upload) | Semaphoren je `cost` (render 2, jvm 2, kdf 1, backup/export 1), Job-Warteschlange höchstens 8 (sonst 503 + `Retry-After`), je Sitzung höchstens 30 render-/jvm-Aufrufe pro Minute (sonst 429), `ConcurrencyLimitLayer`, Timeouts. Ohne gültiges TOTP läuft keine KDF; die KDF-Semaphore wartet höchstens 10 s. Uploads gestreamt, mit Limit und Platzprüfung; JVM mit `-Xmx`, Container mit `mem_limit` und `pids_limit`. | W5, W6, W7, W11 |
| **Fehlkonfiguration durch den Betreiber** (Proxy, HTTP) | `KLEIN_BUCH_PUBLIC_URL` Pflicht, falscher Host → 421. `KLEIN_BUCH_TRUSTED_PROXIES` leer per Default: Weiterleitungs-Header werden nur von dort ausgewertet. Login und Setup nur über Loopback, einen vertrauten Proxy mit HTTPS oder mit `KLEIN_BUCH_INSECURE_HTTP=1` (nur Tests, sichtbare Warnung). Fail-closed-Start, gehärtetes Compose-Beispiel, Handbuch zu Proxy, TLS und Volumes. | W5, W6, W11, W13 |
| **Diebstahl von Volume oder Backup** | DB mit SQLCipher verschlüsselt (ADR 0035), Backups zusätzlich in der Argon2id/AES-256-GCM-Hülle (ADR 0009); Secrets in der verschlüsselten DB. Die Passphrase wird nie gespeichert, nie geloggt (auch nicht über das sqlx-Statement-Logging); Core-Dumps aus (`PR_SET_DUMPABLE` 0, `ulimit core 0`). Das TOTP-Geheimnis liegt außerhalb der DB (`/data/auth/`, 0600) und nützt ohne Passphrase nichts. Restschutz gegen Offline-Raten ist die Länge der Passphrase (≥ 16). | W5, W6, W8, W11 |
| **Codeausführung im Container** (Lücke in einer Abhängigkeit) | Nicht-root (UID 65532), read-only Root-FS, `cap_drop: ALL`, `no-new-privileges`, kein `docker.sock` (E14), kein Root-Entrypoint. | W11 |
| **Manipulierte Lieferkette** | Build nur aus dem öffentlichen Repo; Signatur (cosign keyless), SBOM, Trivy-Gate, `cargo deny`/`cargo audit`/`pnpm audit`, gitleaks; Actions per Commit-SHA, Basis-Images und JARs per Digest bzw. SHA-256. | W11, W12 |

**Nicht abgedeckt:**

- Ein kompromittierter Host oder eine kompromittierte Container-Runtime, Root auf dem Host:
  Wer dort Code ausführt, liest die Passphrase aus dem Prozess-Speicher einer entsperrten Instanz.
- Ein kompromittierter Browser oder ein kompromittiertes Gerät des Nutzers: Malware oder
  Erweiterungen handeln innerhalb einer gültigen Sitzung wie der Nutzer; Re-Auth schützt nur die
  kritischen Aktionen und nur, solange die Faktoren nicht mitgelesen werden.
- Volumetrische DoS-Angriffe auf Netzebene (Sache von Proxy und Netz des Betreibers).
- Verlust der Passphrase: Totalverlust by design (ADR 0035).

## Standardentscheidungen

- **Timeouts:** Sitzung Idle 30 min, absolut 12 h; Re-Auth gilt 5 min.
- **Basis-Image:** Laufzeit `debian:trixie-slim@sha256:<Digest>`, Builder auf derselben
  Debian-Version, JRE per jlink aus `eclipse-temurin:21` (Digest). **Kein distroless in Phase W:**
  Der Sidecar-Launcher braucht `/bin/sh`, die Lizenzpflichten (glibc, libgcc, OpenJDK) wären
  dieselben, und ohne `dpkg-query` fehlt die Paketliste für das Quellangebot.
- **Registry:** `ghcr.io/<owner>/klein-buch` (Besitzer des öffentlichen Repos), gebaut nur aus
  dem öffentlichen Repo, nur `linux/amd64`, Tags `:<CalVer>`, **kein `:latest`** (E15).
- **Off-Site:** mehrere Ziele gleichzeitig (Verzeichnis-Ziele und SFTP), Status je Ziel (E8).
- **Seed-Manifest bei Updates:** `/inputs` ist ein eigenes, vom Menschen gepflegtes Volume
  (`inputs/`-Hardline). `/inputs/.seed-manifest.json` hält je Datei den SHA-256 der zuletzt
  geseedeten Fassung und die Image-Version. Beim Start: (1) Datei fehlt → kopieren;
  (2) unverändert (Hash = Manifest) und das Image bringt eine neue Fassung → ersetzen +
  Inbox-Hinweis; (3) vom Nutzer geändert → nie anfassen, neue Fassung als `<name>.neu` +
  Inbox-Hinweis. Begründung: Ohne Manifest bliebe nur „nie aktualisieren“ (veraltete AfA-Tabellen
  und Vorlagen) oder „überschreiben“ (bricht die Hardline). Umsetzung W11.
- **HTTP-Policy:** kein TLS im Prozess. `KLEIN_BUCH_PUBLIC_URL` ist Pflicht; Host und `Origin`
  müssen passen (sonst 421/403, Ausnahme `/healthz` über Loopback). `KLEIN_BUCH_TRUSTED_PROXIES`
  (CIDR, Default leer): `X-Forwarded-*` nur von dort, Client-IP = von rechts erster Eintrag
  außerhalb der Liste. Login und Setup nur über Loopback, über einen vertrauten Proxy mit
  `X-Forwarded-Proto: https` oder mit `KLEIN_BUCH_INSECURE_HTTP=1` (nur Tests und CI). Ohne
  `TRUSTED_PROXIES` hinter einem Proxy lehnt der Server Einrichtung und Anmeldung ab (keine
  HTTPS-Angabe von einem vertrauten Proxy); schickt ein eingetragener Proxy kein
  `X-Forwarded-For`, wird das Rate-Limit je IP zum globalen (Handbuch; korrigiert 24.10.2026,
  W13). W6.
- **Pflicht-Backup vor jeder Migration** (Entsperren mit älterem Schema und Restore eines älteren
  Backups): Backup erzeugen und prüfen, erst dann `MIGRATOR.run`. Auslöser `manual`, weil der
  CHECK in `backup_log`/`backup_history` nur `manual`, `auto_daily`, `auto_critical` und
  `pre_restore` erlaubt und die Zeile noch auf dem alten Schema entsteht; `manual` wird nie
  automatisch rotiert. Nach erfolgreicher Migration Audit `backup.pre_migration` {from, to,
  backup_id}. Scheitert das Backup, wird nicht migriert, die Instanz bleibt `locked`. W7, vor der
  ersten neuen Migration `0031` (W8).
- **Restore-Untergrenze:** Schema 30 (E15, E17).
- **Migrationen mit LF:** sqlx prüft eine Prüfsumme (SHA-384) der Rohbytes jeder Migration;
  CRLF und LF ergäben verschiedene Prüfsummen und damit `VersionMismatch` beim Restore. Alle
  Migrationen haben LF, ein Test weist jedes CR ab (W1).
- **Versionsangabe vor dem Login bewusst akzeptiert:** `GET /api/public/info` liefert ohne
  Sitzung {version, commit, sourceUrl, licenseSpdx} (mit Rate-Limit), weil AGPL §13 den Nutzern
  eines Netzdienstes das Quellangebot der laufenden Version schuldet. Die Version hilft einem
  Angreifer, bekannte Lücken zu wählen; das Image ist ohnehin öffentlich, Schutz ist die
  Update-Disziplin (`SECURITY.md` nennt unterstützte Versionen).
- **Beiträge Dritter** werden vorerst nicht angenommen, bis der Rechteinhaber über eine CLA bzw.
  Rechteübertragung entschieden hat, damit Dual-Licensing offen bleibt (`CONTRIBUTING.md`,
  Amendment 0014).
- **Quellangebot** für die Komponenten Dritter als Release-Asset
  (`klein-buch-<Version>-quellen-dritter.tar.zst`; ohne Quellarchiv kein Push des Images). Ob
  zusätzlich ein schriftliches Angebot nötig ist, bestätigt der Rechteinhaber vor dem ersten
  öffentlichen Image.

## Write-once unter Linux

Das Archiv setzt jede Datei nach dem Schreiben auf `0o400` (write-once, ADR 0006). Unter Linux verhindert
das **kein** Löschen oder Umbenennen durch dieselbe UID: Beides hängt am Schreibrecht des
Verzeichnisses, das der Server für neue Belege braucht. `0o400` schützt nur vor versehentlichem
Überschreiben des Inhalts. Der eigentliche Schutz ist:

- SHA-256 je Datei beim Schreiben, Re-Hash bei jedem Lesen und Ausliefern (W7);
- der monatliche Integritäts-Cron (ADR 0027), der fehlende, manipulierte und nicht lesbare
  Dateien unterscheidet;
- verschlüsselte Off-Site-Backups (E8) mit Aufbewahrung je Ziel;
- Snapshots des Dateisystems beim Betreiber (empfohlen im Handbuch).

Archiv und DB gehören auf ein lokales Dateisystem mit Hardlinks und POSIX-Rechten (kein
NFS/SMB/exFAT, Handbuch W13); der Start prüft per Selbsttest, ob `0o400` im Archiv greift,
und verweigert sonst den Dienst (W5).

## Entscheidungen während der Umsetzung

Nachgetragen aus W0–W2 und fortlaufend ab W4, je mit Begründung.

1. **XRechnung BG-6/BT-29 (W1).** Ohne USt-IdNr. wird die Steuernummer als BT-29 ausgegeben —
   eine vorhandene Kennung, nichts erfunden; sonst lehnt KoSIT jede Rechnung eines
   §19-Verkäufers ab (BR-CO-26). BG-6 „Seller Contact“ kommt aus den Stammdaten (BT-41 =
   gepflegter Inhaber-Name, sonst Firmenname; BT-42 Telefon; BT-43 E-Mail).
2. **Telefon ist Pflicht beim Festschreiben (W1)** statt eines Platzhalters: keine erfundenen
   Pflichtangaben (BR-DE-6).
3. **Auch eine Kleinbetragsrechnung braucht Steuernummer oder USt-IdNr. (W1).** Jede Rechnung
   bekommt ein XRechnung-XML, das ohne Kennung an KoSIT scheitert; das ist strenger als
   §33 UStDV.
4. **KoSIT-Ablehnungen nennen die Regeln (W1):** Der Report wird auch bei Exit ≠ 0 gelesen, die
   Meldung nennt die ersten Regel-IDs statt nur „exit-code 1“.
5. **Mock-Policy fail-closed (W1).** Ein Release-Build verweigert den Start bei **jedem**
   nicht-leeren `KLEIN_BUCH_SIDECAR_MOCK`, nicht nur bei `1`.
6. **Backup-Format 2 ohne `inputs/branding` (W1).** Das Content-ZIP sichert `branding/` aus dem
   Datenverzeichnis (Logo, Unterschrift); `/inputs` wird eigenes, vom Menschen gepflegtes Volume
   und vom Betreiber selbst gesichert. Restore nimmt Format 1 und 2; Format 2 stellt das Branding
   exakt her, bei Format 1 bleibt es unverändert und der Restore-Bericht sagt das.
7. **Typst-Escaping (W1).** Jedes ASCII-Satzzeichen wird maskiert; `"` und `'` erscheinen im PDF
   als gerade Anführungszeichen, damit der Text exakt BT-154 entspricht.
8. **Abo-Rechnungen ohne Doppel (W1):** Sperre je Vorlage im Prozess (Server = ein Prozess) plus
   Compare-and-Swap in der DB als zweite Linie.
9. **Verwaiste Archivdateien werden beiseitegelegt, nicht gelöscht (W2).** Eine Datei am Zielpfad
   ohne DB-Eintrag (Rest eines abgebrochenen Laufs) wird nach `.<name>.orphan-<uuid>` verschoben;
   alter Hash und neuer Name stehen im Audit. Sie war nie Teil der Aufbewahrung, bleibt aber
   erhalten, falls die Erkennung je irrt. Sperren je Archivpfad und je Rechnung verhindern
   parallele Schreiber.
10. **Wiederholung nach Abbruch erzeugt eine neue Dateiversion (W2)** (`RE-…-2.pdf`), statt die
    Nummer dauerhaft an „existiert mit anderem Hash“ zu blockieren; registrierte Dateien werden
    nie überschrieben.
11. **„Nicht lesbar“ ≠ manipuliert ≠ fehlt (W2).** Rechteprobleme zählen nicht als Manipulation,
    bekommen aber einen eigenen Hinweis; DB-Fehler brechen die Prüfung ab, statt Befunde zu
    erzeugen.
12. **Restore-Tausch nie mit Löschen in unklarer Lage (W2).** Zustandsdatei (moving → moved →
    copied), Rückabwicklung in jeder Stufe; ohne Zustand wird nichts gelöscht, sondern als
    `.verworfen-<uuid>` beiseitegelegt. Der Restore-Marker entsteht erst nach dem Audit; solange ein Restore offen ist,
    gibt es kein Einrichten und kein Entsperren. Das Archiv ist danach wieder `0o400`.
13. **Backup-Konsistenz (W2):** Snapshot per `VACUUM INTO` über eine eigene Verbindung,
    verschlüsselt wie das Original; Backups serialisiert, Dateinamen mit Millisekunden und
    Kurz-UUID; Off-Site per Temp-Datei, fsync und rename. Das Tages-Backup läuft im
    Scheduler-Tick (nach einem Fehlschlag 1 h Pause); das Entsperren löst es nicht mehr
    zusätzlich aus.
14. **Semaphoren auf Kern-Ebene (W2):** RENDER 2, JVM 2, KDF 1 schon vor dem Server-Umbau, damit
    die HTTP-Schicht dieselben Grenzen nutzt; Typst, Argon2id und ZIP laufen außerhalb der
    async-Runtime. Sidecar-Temp folgt `TMPDIR` (im Image `/data/tmp`).
15. **Schreib-Transaktionen mit `BEGIN IMMEDIATE`, `busy_timeout` 15 s (W2):** Festschreiben und
    Zahlung derselben Rechnung parallel enden nicht in `SQLITE_BUSY`.
16. **Kein Geheimnis im sqlx-Log (W2):** Statement- und Slow-Statement-Logging für verschlüsselte
    Verbindungen aus, weil `PRAGMA key` die Passphrase enthält.
17. **CI-Runner fest `ubuntu-24.04` mit apt-Paket-Cache (W0).** `ubuntu-latest` wechselte die
    Version, die Paketspiegel lieferten zeitweise nur mit Timeouts; fester Runner, Paket-Cache,
    Spiegel-Fallback und Zeitlimit machen die CI reproduzierbar. Der apt-Schritt entfällt mit
    Tauri (W5).
18. **Storno-Geschäftsjahr (W2; entschieden in Lauf E, siehe Nr. 122).** Bis v2026.10.0 behielt ein
    Storno im Folgejahr Geschäftsjahr und Nummernkreis der Originalrechnung.
19. **Command-Kerne bleiben im Modul `commands::` (W4).** Der Plan nennt `api::<bereich>::<fn>`;
    die 23 Dateien zu verschieben brächte nichts, der HTTP-Namensraum `/api/cmd/{name}` ist davon
    unabhängig. Jeder Kern ist `async fn name(ctx: &Arc<AppCtx>, args: NameArgs) -> Result<T>`;
    die Args-Structs übernehmen die camelCase-Schlüssel der bisherigen Aufrufe, damit das
    Frontend unverändert bleibt. Eine Registry (`commands::registry`) listet alle Kerne mit den
    Metadaten `auth`, `cost` und `body_limit`.
20. **`&Arc<AppCtx>` statt `&AppCtx` (W4):** Entsperren startet den Scheduler, der einen eigenen
    Verweis auf den Kontext braucht.
21. **Desktop-Nebenwirkungen über einen Trait (W4):** Datei öffnen, im Ordner zeigen, Browser
    öffnen und Neustart laufen über `DesktopActions`; die Tauri-Hülle setzt sie um, der Server
    hat keine. Die betroffenen Commands entfallen im Server oder liefern dort Bytes (W5/W7).
22. **`SecretStore` zunächst nur als Schnittstelle (W4):** Die Keychain-Fassung hängt im Kontext;
    Mail und SFTP stellt erst W8 um, zusammen mit der Tabelle in der verschlüsselten DB.
23. **`Paths` aus der Umgebung (W4):** `KLEIN_BUCH_DATA_DIR`, `KLEIN_BUCH_INPUTS_DIR`,
    `KLEIN_BUCH_RESOURCE_DIR`, `KLEIN_BUCH_WEB_DIR` sind Pflicht, ohne Sonderwege für
    Debug-Builds; `/inputs` und `/data` dürfen nicht ineinander liegen. Das Ergänzen von
    `/inputs` aus `RESOURCE_DIR/inputs` läuft bei jedem Start und überschreibt nie.

24. **Allowlist über die Registry (W5).** Jeder Kern hat einen Weg (`Cmd`, `Replaced`, `Dropped`,
    `Blocked`); über `POST /api/cmd/{name}` erreichbar ist nur `Cmd` (160 von 183). Die Kerne der
    übrigen 23 bleiben vorerst im Code und werden in dem Block entfernt bzw. umgebaut, der sie
    ersetzt (Upload/Download W7, Frontend-Reste W9, Reset und Drop-Folder W11). So bleibt jede
    Entscheidung in einer Tabelle prüfbar (`commands/registry.rs`, Test über alle Namen).
25. **Re-Auth-Klasse schon in W5 gesetzt (W5).** Die 13 Commands aus Plan W6 tragen `Reauth`; bis
    W6 die Frische prüft, gilt für sie die Sitzung.
26. **Pflicht-Header schon in W5 erzwungen (W5).** `X-KleinBuch-Request` fehlt → 403 auf
    `/api/cmd/*`, `/api/auth/setup` und `/api/auth/login`. W6 ergänzt Origin/Host.
27. **Archivpfad-Stichprobe beim Entsperren statt vor dem Port-Bind (W5, Plankorrektur).** Die DB
    ist vor der Passphrase verschlüsselt; `archive_entries` lässt sich erst nach dem Entsperren
    lesen. Passt die Stichprobe (erste und letzte drei Einträge) nicht zu `archive_dir`, wird
    sofort wieder gesperrt; der Login antwortet 409 „Entsperren verweigert“, Details im Log.
28. **Keine Neueinrichtung über einem Archiv (W5).** Weder DB noch Marker, aber Dateien in
    `archive/` → Start verweigert (zusätzlich zu „DB ohne Marker“/„Marker ohne DB“).
29. **Selbsttest verweigert auch root (W5).** Lässt sich eine Datei mit 0400 trotzdem zum
    Schreiben öffnen (root, ACL), startet der Server nicht: der Write-once-Schutz der Belege
    (GoBD) hinge sonst allein am Code.
30. **Keep-Alive faktisch 10 s (W5, Plankorrektur).** hyper misst den Header-Read-Timeout auch in
    der Leerlaufzeit zwischen zwei Anfragen einer Keep-Alive-Verbindung; eine ruhende Verbindung
    schließt nach 10 s statt der im Plan genannten 60 s. Strenger, für Browser und Proxy unschädlich.
31. **Timeout und Zulassung (W5).** Der 30-s-Timeout auf `/api/*` antwortet 503 `timeout` mit
    `Retry-After` (statt 408); die Arbeit läuft als eigener Task weiter, eine Wiederholung mit
    demselben Idempotenz-Schlüssel liefert ihr Ergebnis. Zulassung je Kostenklasse über eigene
    Semaphoren (render 2, jvm 2, kdf 1, backup/export 1), getrennt von denen aus W2, damit ein
    Aufruf nie auf seine eigene Genehmigung wartet; ein direkter Aufruf wartet höchstens 5 s, sonst
    503. Globale Grenze 64 Anfragen ohne Warteschlange (503 statt Stau).
32. **Idempotenz (W5).** Der Server akzeptiert Aufrufe ohne Schlüssel; das Frontend schickt immer
    einen. Gespeichert werden 2xx/4xx-Antworten für 1 h (höchstens 2048), 5xx nicht (die
    Wiederholung führt neu aus); gleicher Schlüssel mit anderem Inhalt → 422.
33. **Jobs (W5).** Die Plan-Liste plus `recurring_invoices_run_now` und
    `recurring_invoices_run_due_check`, weil beide Rechnungen festschreiben (13 Commands).
    Ergebnisse bleiben 1 h abrufbar. Nach einem erfolgreichen Restore lehnt der Server Änderungen
    ab, wartet 3 s (Ergebnis abholbar) und fährt geordnet herunter (Exit 0); der Neustart durch die
    Container-Verwaltung wendet den Restore vor dem Port-Bind an.
34. **Fehlerabbildung (W5).** Domain und Mail 422, Backup 409, Crypto 400, neue Variante
    `BadRequest` (Argumente passen nicht zum Args-Struct) 400, Locked 423, Schema-Mismatch 409,
    Trigger-RAISE 409 mit Text, Sidecar 503 mit festem Text und `ref`, alles Übrige 500 nur mit
    `ref`. Vor dem Login nur generische Meldungen; einzige Ausnahme ist der Hinweis auf die
    Mindestlänge der Passphrase bei der Einrichtung.
35. **Kein `panic = "abort"` im Release-Profil (W5).** Eine Panik beendet nur die Anfrage bzw. den
    Job, nicht den Prozess mit entsperrter DB.
36. **Adresse `KLEIN_BUCH_LISTEN` (W5),** Standard `0.0.0.0:8080`; der Subcommand `healthcheck`
    fragt denselben Port über `127.0.0.1` ab.
37. **Übergangs-Body-Limit (W5).** 256 KiB für `/api/cmd/*`; die 7 Commands, die bis W7 Dateien
    noch als JSON-Zahlenfeld schicken, bekommen übergangsweise 32 MiB.
38. **CI ohne Tauri-Pakete (W5).** Der apt-Schritt installiert nur noch `libdbus-1-dev` (für den
    secret-service-Zweig von `keyring`, entfällt mit W8) und `liblzma-dev` (`zip` → `lzma-sys`
    linkt die System-Bibliothek; sie kam bisher über die WebKit-Pakete mit, der erste Lauf ohne
    sie scheiterte beim Linken). Der Builder im Dockerfile (W11) braucht dasselbe.
39. **Sitzungen nur im RAM (W5).** Ein Neustart meldet alle Sitzungen ab (die DB ist danach ohnehin
    gesperrt, E3). Der Cookie heißt schon jetzt `__Host-kb_session` (Secure, HttpOnly,
    SameSite=Strict); Browser behandeln `localhost`/`127.0.0.1` als sicheren Kontext.

40. **Vorgemerkter Factory Reset verweigert den Start (W5, Gegenprüfung).** Der Server wendet
    `FACTORY_RESET_PENDING.json` nie an; liegt die Datei vor, bricht der Start ab und nichts wird
    gelöscht (E7). Vorher hätte jeder Neustart `/data` geleert, und bis W7 ließe sich die Datei über
    einen Export mit Client-Pfad anlegen.
41. **Entsperren in zwei Schritten (W5, Gegenprüfung).** Öffnen und Probe-Read, dann die
    Archivpfad-Stichprobe, erst danach Migrationen und Aktivieren (Pool, Passphrase, Scheduler,
    Audit). Eine verweigerte Stichprobe hinterlässt keinen Zustand.
42. **Einrichten und Anmelden als eigener Task (W5, Gegenprüfung).** Ein abgebrochener Request
    (Client, Timeout) unterbricht den Zustandswechsel nicht. Scheitert die Einrichtung vor dem
    Entsperren, wird die gerade angelegte DB entfernt (sie enthält noch nichts; bliebe sie, könnte
    der erste Login sie mit beliebiger Passphrase übernehmen). Scheitert nur das Erst-Backup, gilt
    die Instanz als eingerichtet; der erste Scheduler-Tick holt es nach.
43. **Ein Prozess je Datenverzeichnis (W5, Gegenprüfung).** Exklusive Sperre auf
    `/data/.klein-buch.lock` vor allen Schritten, die das Verzeichnis verändern (Restore-Tausch,
    `tmp/` leeren).
44. **Restore sperrt Änderungen ab der Annahme (W5, Gegenprüfung).** Der Restore-Job pausiert den
    Scheduler; Änderungen werden bis zum Neustart mit 503 abgelehnt (sie gingen beim Tausch
    verloren). Scheitert der Restore, werden Sperre und Pause aufgehoben. Den vollständigen Umbau
    (Upload, Staging, Phase A) bringt W7.
45. **Herunterfahren wartet auf alle abgekoppelten Tasks (W5, Gegenprüfung)** (`TaskTracker`:
    Commands, Jobs, Anmeldungen; höchstens 10 min), danach Scheduler-Tick und Pool. Die
    Stop-Frist der Container-Verwaltung muss dazu passen (Compose-Beispiel W11, Handbuch W13).

46. **Einrichtung zweistufig über dieselbe Route (W6).** Der Browser braucht das TOTP-Geheimnis,
    bevor er den Bestätigungscode schicken kann. `POST /api/auth/setup` mit `{setupToken,
    step:"totp"}` liefert Geheimnis und QR-Code (ausstehend, an den Token gebunden);
    `{setupToken, passphrase, totpCode}` richtet atomar ein — Token und Code werden geprüft,
    **bevor** die DB entsteht. Das Inventar der öffentlichen Routen (Plan §3) bleibt unverändert.
47. **Setup-Token nur auf stderr (W6).** Der Token steht als eigene Zeile im Container-Log
    (stderr), nie im `tracing`-Log; der Log-Test prüft `tracing` auf Token, TOTP, Passphrase und
    Sitzungs-ID. Ein Token entsteht beim Start, wenn keine DB oder keine `totp.json` vorhanden ist,
    bzw. beim ersten Login nach `totp-reset`.
48. **Bestätigungscode gilt als verbraucht (W6).** Beim Einrichten wird `last_used_step` auf den
    aktuellen Schritt + 1 gesetzt; der Code der Einrichtung lässt sich nicht für den Login
    wiederverwenden.
49. **Rate-Limit-Schlüssel (W6):** gültiges Gerätecookie, sonst Client-IP. Auch Einrichtung (Token)
    und Re-Auth zählen. Eine Wartezeit antwortet 429 mit `Retry-After`; die Antworten auf falsches
    TOTP und falsche Passphrase bleiben identisch (401 „Anmeldung fehlgeschlagen.“). Das Gerätecookie
    entsteht nach erfolgreicher Einrichtung bzw. Anmeldung.
50. **Re-Auth akzeptiert auch einen Einmal-Code (W6)** — gleiche Faktoren wie der Login.
51. **Origin-Regel (W6):** Bei ändernden Methoden muss ein vorhandener `Origin` dem öffentlichen
    Origin entsprechen; fehlt er (Nicht-Browser-Clients), entscheidet der Pflicht-Header
    `X-KleinBuch-Request`, der im Browser einen CORS-Preflight erzwingt (kein CorsLayer → scheitert).
52. **Effektiver Host (W6):** `X-Forwarded-Host` zählt nur von vertrauten Proxys, sonst der
    `Host`-Header; geprüft wird gegen `KLEIN_BUCH_PUBLIC_URL` (421).
53. **TOTP-Uhr = Systemzeit (W6).** Authenticator-Apps rechnen mit der Wanduhr; eine monotone Uhr
    liefe nach einem Zeitsprung des Hosts auseinander. Tests verschieben sie über eine
    Test-Schnittstelle; Sitzungs-Timeouts laufen über `tokio::time` (pausierbar).
54. **„Datenbank sperren“ und „Passphrase ändern“ warten höchstens 60 s (W6)** über eine
    Lese-/Schreibsperre auf laufende Commands und Jobs (`db_gate`), sonst 503; neue Aufrufe
    warten solange.
55. **Passphrase-Wechsel (W6):** gleiche Passphrase → 422; scheitert das Neuverschlüsseln vor dem
    Tausch, gilt die alte Passphrase weiter und der Pool wird mit ihr wieder geöffnet. Danach ein
    Backup (`manual`); ältere Backups bleiben mit der alten Passphrase einspielbar.
56. **Weitere Auth-Routen nur mit Sitzung (W6):** `logout`, `lock`, `reauth`, `security`; mit
    Sitzung + Re-Auth `totp/renew` (zweistufig wie die Einrichtung), `recovery/renew`,
    `passphrase`.
57. **Klartext-Übergangspfad entfernt (W6, E17):** Adoption in der Einrichtung, Klartext-Zweig beim
    Entsperren und `migrate_plaintext_to_encrypted` samt Test; `encrypt_db_file_in_place` bleibt
    als gemeinsamer Kern des Neuverschlüsselns.
58. **„Fehlversuche seit der letzten Anmeldung“ nur im RAM (W6);** ein Neustart setzt den Zähler
    zurück. Fehlversuche stehen dauerhaft nur im Log (`WARN auth.login.failed ip=… reason=…`).
59. **Rate-Limit gegen Adress-Rotation (W6, Gegenprüfung).** IPv6-Clients zählen je /64. Dazu ein
    globales Budget für alle Clients **ohne** gültiges Gerätecookie (ab 50 Fehlversuchen dieselbe
    Wartezeit-Staffel): wer die Passphrase kennt, kann den TOTP-Faktor auch mit vielen Adressen
    nicht durchprobieren; der eigene Client mit Gerätecookie bleibt frei. Die Tabelle ist begrenzt
    (10 000 Einträge, älteste fallen heraus). Die Wartezeit wird nach dem Warten auf die
    Zustandssperre erneut geprüft (parallele Versuche bekommen keine Zusatzanläufe).
60. **Passphrase-Wechsel ohne Zeitfalle (W6, Gegenprüfung).** Das Backup mit der neuen Passphrase
    läuft nach der Antwort als nachverfolgter Task; der Transport wiederholt Auth-Aufrufe nicht
    (weder nach Netzfehlern noch nach 503). Im Fehlerfall entscheidet ein Probe-Öffnen, welche
    Passphrase gilt; lässt sich der Verifier nicht schreiben, wird gesperrt (Entsperren prüft dann
    über SQLCipher, also nur mit der neuen). Läuft ein Restore, wird der Wechsel abgelehnt. Die neue
    Datei wird vor dem Tausch per `quick_check` geprüft und mit fsync geschrieben.
61. **Loopback nur ohne Weiterleitungs-Header (W6, Gegenprüfung).** Ein Reverse-Proxy auf demselben
    Host erscheint als Loopback; mit `X-Forwarded-*`/`Forwarded` gilt die Anfrage nicht als lokal
    (sonst wäre Login über Klartext-HTTP aus dem Netz möglich). Ein unlesbarer `X-Forwarded-For`-
    Eintrag bricht die Kette: dann zählt der vertraute Proxy selbst als Client.
62. **Transport-Regel auch für Re-Auth, Passphrase und Erneuerungen (W6, Gegenprüfung);** das
    Frontend schickt Passphrase und Codes nur in einem sicheren Kontext (HTTPS oder localhost).
63. **HSTS ohne `includeSubDomains` (W6, Gegenprüfung)** — die Instanz kennt die Subdomains des
    Betreibers nicht. `KLEIN_BUCH_TRUSTED_PROXIES` mit `/0` erzeugt eine Warnung im Log.
64. **Setup-Token nach `totp-reset` beim Statusabruf (W6, Gegenprüfung),** nicht erst beim Login —
    die Oberfläche zeigt sofort die Neueinrichtung. Der Healthcheck-Subcommand schickt den Host der
    öffentlichen URL (fest gebundene IP).
65. **Downloads über eine Ablage mit Ticket (W7, E10).** Exporte und Vorschauen entstehen in
    `<data>/tmp/downloads/<uuid>/<Dateiname>`; der Command liefert `{downloadId, fileName,
    sizeBytes}`, der Browser holt die Datei über `GET /files/download/{id}` (Sitzung). Exporte sind
    einmalig abrufbar (danach gelöscht, auch bei abgebrochenem Abruf) und verfallen nach 1 h,
    Vorschauen sind 10 min mehrfach abrufbar (die PDF-Ansicht lädt nach). Die Ablage lebt im RAM;
    Reste nach einem Neustart räumt die `tmp`-Bereinigung beim Start.
66. **Inline nur für eigene PDFs (W7).** `/files/archive/{id}` liefert `issued_invoice_pdf` und
    `issued_quote_pdf` inline (CSP `frame-ancestors 'self'`), Vorschauen aus der Download-Ablage
    ebenso; alle anderen Quellen (Belege, Anhänge, Rechtsdokumente, empfangene E-Rechnungen, XML)
    als `attachment` mit CSP `sandbox`. Der Inhaltstyp kommt nur aus einer Allowlist
    (`application/octet-stream` sonst), der Pfad wird per `canonicalize` gegen `archive_dir`
    geprüft (Fehler ohne Pfad in der Antwort), der Abruf ist hash-geprüft und auditiert.
67. **PDF-Vorschau als Dialog mit iframe (W7)** statt neuem Fenster: ein `window.open` nach einem
    `await` würde vom Popup-Blocker abgefangen. Der Dialog bietet „In neuem Tab öffnen“ und
    „Herunterladen“ als echte Links. Anhänge und Beleg-Originale werden heruntergeladen.
68. **Explorer-Funktionen ersatzlos (W7).** „Im Ordner zeigen“, „Ordner öffnen“, „Pfad kopieren“
    entfallen in der Oberfläche (Rechnung, Kosten, Angebot, EÜR-Export, Backup-Seite); die
    Commands bleiben bis W11 in der Registry als `Dropped`/`Replaced`.
69. **Uploads: Ablage mit Ticket, Verbrauch erst nach Erfolg (W7).** `POST /api/upload/{art}`
    schreibt nach `<data>/tmp/uploads/<uuid>/datei` und liefert `{uploadId, fileName, mimeType,
    sizeBytes}`. Commands lesen den Upload und geben ihn erst nach Erfolg frei; ein abgelehnter
    Versuch (z. B. Validierungsfehler im Formular) kann dieselbe Datei erneut verwenden. Das
    Einlesen einer E-Rechnung verbraucht den Upload nicht (Schritt 2 archiviert dasselbe Original).
    Höchstens 100 offene Uploads, Ablauf nach 1 h. Die Upload-Route liegt außerhalb des
    Gesamt-Timeouts von `/api/*`; stattdessen 60 s Leerlauf-Timeout je Datenblock.
70. **Typprüfung am Inhalt (W7).** PDF, PNG, JPEG, WebP, GIF, TIFF per Magic Bytes; XML nur als
    gültiges UTF-8 mit Wurzelelement, HTML/XHTML (auch eingebettet per Namensraum) wird abgelehnt;
    SVG nur für Logo und Unterschrift; die Backup-Art nur mit dem Backup-Header. Erlaubt je Art:
    Beleg/Anhang/Vertrag alle Dokumenttypen inkl. XML, Rechtsdokument nur PDF, E-Rechnung PDF oder
    XML, Logo/Unterschrift PNG/JPEG/WebP/GIF/SVG. Der MIME-Typ im Archiv kommt aus dieser Prüfung;
    Logo und Unterschrift bekommen die Endung des erkannten Typs. XML in falscher Kodierung bekommt
    die genaue Meldung aus R3-006 schon beim Upload.
71. **Platzprüfung vor dem Upload (W7).** Abgelehnt mit 507, wenn der freie Platz unter
    `KLEIN_BUCH_MIN_FREE_BYTES` (Standard 2 GiB) plus der angekündigten Größe (höchstens dem
    Limit der Art) liegt; ungültige Werte der beiden Upload-Variablen brechen den Start ab.
    `statvfs` über `rustix` (kein `unsafe`).
72. **Restore-Reihenfolge (W7).** Manifest → Format → Schema `30 ≤ s ≤ EXPECTED` → Entschlüsseln
    (Passphrase-Prüfung) → Pre-Restore-Backup → Platzprüfung (entpackte Größe laut ZIP-Verzeichnis
    ≤ `KLEIN_BUCH_MAX_RESTORE_BYTES` und ≤ frei − `KLEIN_BUCH_MIN_FREE_BYTES`) → Entpacken →
    gestagte DB mit der Backup-Passphrase öffnen, Schema-Version = Manifest. Eine falsche Passphrase
    schreibt nichts (auch kein Pre-Restore-Backup) und lässt den Upload für den nächsten Versuch
    liegen; jeder andere Fehler verwirft Upload und Staging. Eine unverschlüsselte DB im Backup wird
    abgelehnt (sie würde den Start sperren). Quelle in Audit, Marker und Bericht ohne Serverpfad
    („Upload <Datei>“ bzw. „Backup vom <Zeit>“); aus der Historie nur Dateien im lokalen
    Backup-Ordner. Die Datei liegt beim Entschlüsseln ganz im Speicher (AES-GCM über den ganzen
    Inhalt): Speicherbedarf rund das Dreifache der Backup-Größe (Datei, Ciphertext-Kopie,
    Klartext; Betrieb W13; seit Lauf F3a in place, etwa einmal, Nr. 127 (2)). Gegenprüfung: Phase A prüft zusätzlich die Archivpfade der gestagten
    DB gegen `archive_dir` und die Migrations-Prüfsummen gegen diese Version (sonst wäre die
    Instanz nach dem Tausch dauerhaft gesperrt) und verlangt das Doppelte der entpackten Größe als
    freien Platz (der Tausch kopiert das Staging). Ein zweiter Restore während des ersten wird mit
    503 abgelehnt; ein schon vorgemerkter Restore sperrt weitere. Der Restore-Job wartet
    exklusiv auf laufende Commands/Jobs (`db_gate.write`), deren Änderungen damit im
    Pre-Restore-Backup stehen; wartende Schreib-Commands brechen danach mit 503 ab.
73. **Gehärtetes Entpacken (W7).** Nur normale Pfadbestandteile (`enclosed_name()` lässt `a/../b`
    zu — das unterliefe die Allowlist), Allowlist `klein-buch.sqlite`, `archive/**`, `branding/**`,
    im Format 1 zusätzlich `inputs/branding/**` (übersprungen); keine Symlinks, keine doppelten Dateien (`create_new`), höchstens
    200 000 Einträge, je Eintrag höchstens 200 × gepackte Größe + 1 MiB Sockel (kleine, sehr gut
    komprimierbare Dateien), gesamt höchstens das Budget aus Nr. 72 — gezählt werden die
    tatsächlich gelesenen Bytes (`take`), nicht die Angaben im ZIP.
74. **Pflicht-Backup vor Migration mit echter Schema-Version (W7).** Backups tragen die
    Schema-Version der gesicherten Datenbank (Manifest und `backup_history`), nicht mehr die
    einkompilierte — sonst würde das Backup einer v30-DB vor der Migration als v31 gelten und beim
    Restore nicht migriert. Beim Entsperren mit älterem Schema: Backup `manual`, danach vollständig
    gelesen und entschlüsselt; scheitert das, wird nicht migriert (409
    `pre_migration_backup_failed`, Log, Inbox, Instanz bleibt gesperrt). Nach der Migration Audit
    `backup.pre_migration {from, to, backup_id}`. Ein Restore eines älteren Backups läuft beim
    nächsten Entsperren über denselben Weg. Der Fall „Schema 30 → 31“ wird erst mit Migration 0031
    (W8) echt; bis dahin prüfen die Tests den Weg mit einer um eine Version älteren DB.
75. **Schreibsperre während des Restores bleibt 503 `restarting` (W7, Plan korrigiert).** 423
    bedeutet im Transport „Datenbank gesperrt → Entsperren-Maske“; während Phase A ist Warten und
    Wiederholen richtig (Nr. 44).
76. **Vorlagen nach dem Restore (W7).** Nach dem Anwenden prüft der erste Entsperr-Lauf die
    Standard-Vorlage im Verkäuferprofil, offene Rechnungs- und Angebotsentwürfe und aktive
    Abo-Rechnungen gegen `/inputs/pdf-templates` (+ eingebaute) und legt bei fehlenden eine
    Inbox-Warnung mit den Namen an (`/inputs` ist nicht Teil des Backups).
77. **Mehrere Off-Site-Ziele (W7).** `app_settings.backup_targets` = `[{id, label, kind, …}]`
    (höchstens 10), Lesefallback `backup_target`/`backup_target_path` (ein Verzeichnis außerhalb der
    Wurzel wird verworfen). Verzeichnis-Ziele werden relativ zur Wurzel `/backup-targets`
    (`KLEIN_BUCH_BACKUP_TARGETS_DIR`, neues Feld `Paths::backup_targets_dir`) gespeichert und bei
    jedem Schreiben per `canonicalize` geprüft; fehlende Unterordner werden angelegt, aber nie
    außerhalb (jeder vorhandene Vorfahr wird geprüft). Zur Auswahl listet die Oberfläche die
    Unterordner der Wurzel (eingebundene Volumes). Jedes Backup geht auf jedes Ziel; Fehler je Ziel
    in `backup_log` (Label = Ziel-Bezeichnung), Audit, Inbox-Warnung (nicht abschaltbar, je Ziel und
    Tag) und `app_settings.backup_offsite_results`. Status `offsite` = `missing` (kein Ziel) |
    `failing` (ein Ziel scheiterte zuletzt) | `ok` (seit Lauf F3a auch `stale`, ADR 0040 Nr. 52); im Loopback-`/healthz`, in den
    Backup-Einstellungen und als Aufgabe in der Übersicht. Ohne Ziel wöchentlich ein Inbox-Hinweis.
    Rotation je Ziel (Gruppe = enthaltender Ordner bzw. SFTP-Ordner). Neue Commands
    `backup_target_add_directory`, `backup_target_add_sftp`, `backup_target_remove` (Re-Auth);
    `backup_set_target`/`backup_set_sftp_target` sind ersetzt (nicht mehr freigegeben, Code entfällt
    in W11); `detect_cloud_targets`/`default_suggestion` und die Ordnerauswahl per Tauri-Dialog
    entfallen.
78. **SFTP je Ziel (W7).** Passwort je Ziel-ID im Schlüsselspeicher (Rückfall auf den alten
    Einzel-Eintrag; ab W8 im `SecretStore` der DB); Upload unter `.<name>.part`, dann `rename`
    (kein halbes `.kbk` unter dem Endnamen). Ein nicht erreichbares SFTP-Ziel dient im Test als
    „Mock“ für den Ausfall eines Ziels.
79. **Restore-Audit dauerhaft (W7, Gegenprüfung).** `backup.restore.requested` entsteht nach der
    Passphrase-Prüfung und VOR dem Pre-Restore-Backup, steht also in genau dieser Sicherung des
    ersetzten Stands (die alte DB wird beim Tausch verworfen). Der Tausch legt die Angaben in
    `RESTORE_APPLIED.json` ab (statt nur im RAM); das erste Entsperren schreibt
    `backup.restore.applied` (mit Pre-Restore-Backup und Schema) in die wiederhergestellte DB,
    übernimmt das Pre-Restore-Backup in `backup_history` (Restore aus der Liste möglich) und löscht
    die Datei erst danach — auch nach weiteren Neustarts vor dem Entsperren. Pre-Restore-Backups
    gehen jetzt auch an die Off-Site-Ziele (nie rotiert). Reste eines abgebrochenen Phase-A-Laufs
    (Staging ohne Marker/Tausch-Zustand) räumt der Start auf.
80. **Desktop-Backups (W7, Gegenprüfung, Korrektur von Nr. 73).** Desktop-Backups enthalten
    Windows-Archivpfade und werden deshalb in Phase A abgelehnt, sobald sie festgeschriebene Belege
    enthalten (E17: kein Datenumzug aus der Desktop-Fassung). Format 1 bleibt nur für Backups ohne
    Archiveinträge bzw. mit passenden Pfaden zulässig. Disaster Recovery setzt denselben
    Datenpfad voraus (im Container immer `/data`).
81. **Logo-Dateiname nie vom Client (W7, Gegenprüfung, S1).** `seller_profile_upsert` übernahm
    `logoFilename` ungeprüft; `seller_logo_data`/`seller_logo_clear` bauten daraus einen Pfad —
    Lesen (z. B. `/data/auth/totp.json`) und Löschen beliebiger Dateien mit einer Sitzung. Jetzt
    wird das Feld ignoriert (nur `seller_logo_set`/`_clear` setzen es), und Branding-Namen müssen
    `logo|signature.<erlaubte Endung>` sein.
82. **Uploads und Ablagen (W7, Gegenprüfung).** Der ganze Multipart-Körper ist auf Limit + 64 KiB
    begrenzt (`http_body_util::Limited`, sonst puffert multer Präambel/Part-Header unbegrenzt);
    genau ein Feld `file`. Downloads und Uploads werden minütlich aufgeräumt (auch unregistrierte
    Reste abgebrochener Vorgänge nach Ablauf), beim Sperren der Datenbank ganz geleert; die
    DSGVO-Auskunft entsteht in `<data>/tmp/dsgvo/<id>` und wird bei Fehlern sofort gelöscht. HEAD
    auf `/files/*` → 405 (verbrauchte sonst einmalige Exporte bzw. schrieb `archive.read`). XML
    ohne `<script`/`<p:script`, ohne Wurzel html/body/script und ohne Zeichenreferenzen in
    Namensraum-Angaben; eingebettetes SVG gilt als SVG. Der Migrations-Export schreibt den
    Dateinamen statt des Serverpfads ins Audit.
83. **Ziele (W7, Gegenprüfung).** Die erste Ebene unter `/backup-targets` (das Volume) wird nie
    angelegt; beim Hinzufügen Normalisierung, Dublettenprüfung am aufgelösten Pfad und ein
    Schreibtest. Je Ziel Gesamt-Timeout (60 s + 1 s je MiB), Verzeichnis-Schreiben außerhalb der
    Runtime. Die kurze Floor-Aufbewahrung gilt nur, wenn mindestens ein Ziel zuletzt erfolgreich
    war. Der alte SFTP-Schlüsselbund-Eintrag dient nur noch dem übernommenen Altziel `sftp`.
    `KLEIN_BUCH_BACKUP_TARGETS_DIR` darf nicht `/` sein und nicht mit Daten/Vorlagen
    ineinanderliegen. `backup.pre_migration` wird vor der Migration in die alte DB geschrieben.
    Angebot annehmen prüft den Status vor dem Archivieren des Vertrags. Nicht übernommen (S3,
    Queue): WAL/SHM beim Tausch mitsichern, laufende Uploads bei `MAX_PENDING`/Platz reservieren,
    Angebots-Bundle mit hochgeladenen Rechtsdokumenten weiter inline (nur eigene Uploads des
    Inhabers), Status `offsite` veraltet nicht (die Regel `backup_overdue` deckt veraltete Kopien
    ab).
84. **`SecretStore` (W8).** Migration `0031`: Tabelle `secrets (key, value, updated_at)` STRICT,
    Schlüssel `smtp:<Konto-ID>`, `oauth:<Konto-ID>`, `sftp:<Ziel-ID>`; Zugriff nur über
    `crate::secrets` bei offenem Pool. Die Tabelle fehlt im Migrations-Export (`EXCLUDED_TABLES`);
    die DSGVO-Auskunft ist kontaktbezogen und liest sie nie. In Backups ist sie enthalten (ganze
    verschlüsselte DB, ein Restore bringt die Zugangsdaten mit). `keyring` und damit `libdbus`
    entfallen (`mail/keyring.rs` gelöscht, CI ohne `libdbus-1-dev`). Kein Übernehmen alter
    Schlüsselbund-Einträge (E17): Das Altziel `sftp` aus der Desktop-Fassung und Desktop-Mailkonten
    haben im Server kein Passwort und melden das beim Versand bzw. Test; die Spalte
    `mail_accounts.keychain_service_id` bleibt nur als Markierung „Passwort-Konto“ (kein
    Schema-Umbau).
85. **Bindung an (Host, Port, Benutzer) (W8).** `mail_account_update` vergleicht Host, Port und
    Benutzer (getrimmt) mit dem gespeicherten Konto. Ändert sich eines, löscht der Server das
    Passwort; mit Benutzer ist dann ein neues Pflicht, sonst 422 ohne Änderung. SFTP-Ziele lassen
    sich nicht ändern (nur entfernen und neu anlegen, Nr. 77), `backup_set_sftp_target` ist ersetzt
    — die Bindung ergibt sich dort aus der Ziel-ID. Gespeicherte Konten und Ziele testen nur
    `mail_account_test_saved(id)` und `backup_test_sftp_saved(id)` (Re-Auth, beide neu in der
    Allowlist, 188 Commands); die Ad-hoc-Tests verlangen ein eingegebenes Passwort, sobald ein
    Benutzer angegeben ist, und nehmen keine ID an — ein gespeichertes Passwort erreicht nie einen
    frei gewählten Host. `backup_test_sftp_saved` prüft zusätzlich den gepinnten Fingerprint.
86. **Verbindungstests (W8, Abweichung vom Plan).** Kontingent 10 je Minute **instanzweit** statt
    je Sitzung (eine Instanz = ein Mandant, mehrere Sitzungen desselben Inhabers sollen das Limit
    nicht vervielfachen). Ebenso merkt sich der Server die SFTP-Probe je (Host, Port) instanzweit
    für 10 Minuten statt je Sitzung; `backup_target_add_sftp` übernimmt nur genau diesen
    Fingerprint (Host in Kleinbuchstaben, sonst Fehler „Verbindungstest erneut“). Ports aus
    `KLEIN_BUCH_ALLOWED_TEST_PORTS` werden beim Start gelesen (ungültig → Start abgebrochen) und
    ersetzen die Standardlisten beider Arten. Port-Prüfung, Klartext-Prüfung und Passwort-Pflicht
    laufen vor dem Kontingent und vor jedem Verbindungsaufbau. Gescheiterte Verbindungen melden
    einheitlich „Verbindung zu Host:Port fehlgeschlagen (Host, Port, Zugangsdaten und Firewall
    prüfen).“; die Einzelheiten stehen nur im Debug-Log. Das gilt auch für die Testmail
    (`mail_send_test`, Port-Prüfung + Kontingent).
87. **SMTP im Server (W8).** `use_tls = false` nur ohne Benutzer und Passwort (anlegen, ändern —
    auch mit bereits gespeichertem Passwort — und testen); der Transport lehnt Klartext mit
    Zugangsdaten zusätzlich selbst ab. `KLEIN_BUCH_SMTP_HELO_NAME` setzt den EHLO-Namen (Standard
    lettre: Hostname des Containers), `KLEIN_BUCH_SMTP_CA_FILE` eine eigene CA (PEM-Datei im
    Volume, ergänzt die eingebauten Wurzeln); eine Option „Zertifikat ignorieren“ gibt es nicht.
    Neue oder geänderte Konten mit `auth_type = oauth_microsoft` lehnt der Server ab (E9); ein
    übernommenes Microsoft-Konto lässt sich auf SMTP umstellen (der Token wird dabei gelöscht) oder
    löschen. `mail_oauth_*` sind gesperrt (404) und aus `api.ts` entfernt. Der Code (`mail/oauth_ms.rs`,
    die Commands, `tests/oauth_ms_test.rs`) ist noch vorhanden — die in W11 geplante Entfernung
    nannte der Plan nicht als Löschliste (Stand 24.10.2026, W13; S3).
88. **SFTP-Testserver (W8).** Für den Done-Check „SFTP-Ziel mit Fingerprint“ startet
    `tests/sftp_mock` einen russh-SFTP-Server auf `127.0.0.1` mit einem im Test erzeugten
    Ed25519-Schlüssel (keine Fixture-Datei) und Passwort-Login; er kann nur, was der Upload braucht.
    CI braucht dafür keinen zusätzlichen Dienst. Der SMTP-E2E gegen MailHog (`tests/e2e_test.rs`,
    CI-Dienst) bleibt; lokal überspringt er sich.
89. **Geschäftsjahr nie vom Client (W9).** Das Feld `fiscalYear` fällt aus den Args von
    `invoices_create_draft`, `quotes_create_draft`, `expenses_create`,
    `expenses_create_from_einvoice` und `private_movements_create` sowie aus `api.ts`; der Server
    nimmt `fiscal_year_for(Belegdatum)`. Ein trotzdem mitgeschicktes `fiscalYear` wird ignoriert
    (unbekannte Felder verwirft serde), nicht mehr abgelehnt — es kann den Nummernkreis nicht
    mehr beeinflussen. `resolve_document_fiscal_year` entfällt. Abo-Rechnungen gehen denselben
    Weg. Die Jahresauswahl der Berichte (`euer_*`, `depreciation_*`, Listenfilter) bleibt.
90. **Über-Dialog (W9).** Normale Links statt Opener-Plugin; die Quell-URL kommt aus
    `/api/public/info` (`KLEIN_BUCH_SOURCE_URL`, AGPL §13), Lizenztext über gnu.org, die
    vollständige Liste der Drittanbieter-Lizenzen verweist auf `/legal/third-party-licenses.html`,
    das der Server ab W12 ohne Sitzung ausliefert (bis dahin 404). Drittanbieter-Liste „axum /
    tokio“ statt „Tauri 2“; die vollständige Liste entsteht in W12.
91. **Entfallene Oberflächen (W9).** Routen `settings/reset` (E7) und `settings/drop-folder`
    (E11) samt Einträgen gelöscht; die zugehörigen `api.ts`-Funktionen (`factory_reset*`,
    `drop_folder_*`, `third_party_licenses_path`) ebenso — die Commands sind seit W5 gesperrt,
    ihr Rust-Code entfällt in W11. Der Kanal „System-Mitteilung“ der Erinnerungen ist aus der
    Oberfläche entfernt; die Spalte `notification_rules.deliver_os_native` bleibt (kein
    Schema-Umbau), zugestellt wird nur in die Inbox (seit W4). `@tauri-apps/*` und das Skript
    `tauri` sind aus `package.json` entfernt.
92. **Ohne Secure Context (W9).** Ohne Secure Context (HTTP außerhalb von Loopback, etwa mit
    `KLEIN_BUCH_INSECURE_HTTP=1` in Tests) fehlen `crypto.randomUUID` und `navigator.clipboard`: `src/lib/browser.ts` liefert `randomId()` (Rückfall
    `getRandomValues`) und `copyText()` (Rückfall verstecktes Textfeld + `execCommand`). Die
    Anmeldung selbst verlangt weiter HTTPS bzw. Loopback (W6).
93. **Entwicklung (W9).** `pnpm dev` leitet `/api`, `/files`, `/healthz` und `/legal` an den
    lokalen Server (`KLEIN_BUCH_DEV_API`, Standard `http://127.0.0.1:8080`); der Server läuft
    dafür mit `KLEIN_BUCH_PUBLIC_URL=http://127.0.0.1:5173`. Der Build-Abschnitt im README ist
    darauf umgestellt; Schnellstart und Betrieb folgen in W13.
94. **Zeitstempel in UTC unabhängig von `TZ` (W9-Fund, S1).** SQLite wendet den Modifikator
    `'utc'` auch auf `'now'` an, als wäre das Ortszeit: mit `TZ=Europe/Berlin` (E12) liefert
    `datetime('now','utc')` UTC minus 1–2 h (geprüft mit der eingebauten SQLCipher 3.45.3).
    Das betrifft die Standardwerte und Trigger aller Tabellen (149 Stellen, u. a.
    `audit_log.timestamp_utc`, `backup_log`/`email_log.created_at`, `created_at`/`updated_at`);
    in CI (UTC) fiel es nicht auf, aufgedeckt hat es `contacts_repo_test` in der Kopie (TZ
    Europe/Berlin) kurz nach Mitternacht. Die Desktop-Fassung dürfte unter Windows ebenso
    betroffen gewesen sein (SQLite nutzt dort die Zeitzone des Systems; nicht nachgeprüft).
    Migrationen lassen sich nicht ändern (Prüfsummen), ein Tabellen-Neubau aller Belegtabellen
    samt GoBD-Triggern wäre unverhältnismäßig; `SQLITE_OMIT_LOCALTIME` scheidet aus (dann
    liefert `'utc'` NULL). Deshalb: `server::main_entry` setzt `TZ=UTC` vor jedem weiteren
    Thread; die vier Datumsfilter der Protokolle vergleichen gegen Berliner Tagesgrenzen in UTC
    (`clock::berlin_day_start_db_utc`) statt `date(…, 'localtime')`. Fachliche Daten rechneten
    schon vorher mit `crate::clock` (chrono-tz). Der Java-Sidecar erbt `TZ=UTC` (Daten bekommt er
    explizit). Bestehende Zeitstempel werden nicht korrigiert (GoBD, E17). Test:
    `server_binary_test` startet das Binary mit `TZ=Europe/Berlin` und prüft
    `audit_log.timestamp_utc` gegen die UTC-Uhr; `kb-gate` und README: Tests mit `TZ=UTC` wie CI.
95. **Responsiv (W10).** Bruchstellen 900 px (Navigation als Drawer mit Menüknopf in einer
    klebenden Leiste, Dokument- statt `main`-Scroll, schmalere Ränder, Eingaben 16 px gegen den
    iOS-Zoom), 820 px (Tabellen als Karten; Handy und Tablet hochkant) und 600 px (einspaltige Formulare, PageBar nicht
    klebend, Toasts unten, PDF-Dialog vollflächig). Zentrale Bausteine in `tokens.css`:
    `table.cards` mit `data-label` je Zelle (Label als float, Inhalt rechtsbündig im Textfluss,
    `td.cards-full` für lange Texte/Beschreibungs-Eingaben), Tabellen außerhalb von Karten
    scrollen in sich, Labels dürfen schrumpfen (lange Auswahltexte). Touch-Ziele: primäre Buttons
    überall ≥ 44 × 44 px, auf Touch-Geräten und unter 900 px alle Aktions-Buttons ≥ 44 px hoch.
    Angewendet auf 38 Seiten (Workflow mit 4 Agenten, getrennte Dateigruppen, keine Builds;
    Prüfung, Messung und Korrekturen danach zentral).
96. **PDF und Fotobeleg mobil (W10).** Mobile Browser zeigen PDFs im iframe nicht (Android) oder
    nur die erste Seite (iOS): unter 600 px zeigt der PDF-Dialog statt des iframes „PDF öffnen“
    (Ansicht des Browsers) und „Herunterladen“. Die Kostenerfassung hat neben der Dateiauswahl
    „Beleg fotografieren“ (`accept="image/*" capture="environment"`); iOS liefert dabei JPEG,
    das der Upload (Art `beleg`) annimmt. `viewport-fit=cover`, `theme-color`, `lang="de"`.
97. **Messung (W10).** Playwright über alle 54 Routen bei 390×844, 768×1024 und 1280×800 mit
    Testdaten: Dokument und `main` ohne seitlichen Überlauf, keine überstehenden Elemente
    außerhalb eigener Scroll-Container, primäre Buttons ≥ 44 px, keine Konsolenfehler,
    Screenshots unter `/data/test/Buchhaltung/arbeit/screens/w10`; zusätzlich als Information
    die Bereiche, die in sich seitlich scrollen. Kernabläufe als E2E bei 390×844 mit Touch
    (`isMobile`). Echte Geräte prüft Manuel nach W14.
98. **OpenSSL ohne `atexit` (W10, CI-Fund).** Ein Testbinary stürzte nach allen grünen Tests
    beim Beenden ab („double free or corruption“, lokal nicht reproduziert). Wahrscheinliche
    Ursache: Das vendored OpenSSL von SQLCipher räumt
    sich per `atexit` auf, während sqlx-Threads noch Verbindungen schließen. `db::init_crypto`
    (`openssl_sys::init()`, `OPENSSL_INIT_NO_ATEXIT`) läuft vor dem ersten SQLCipher-Zugriff
    (`open_pool`, Snapshot, Serverstart); `openssl-sys` ist dafür direkte Abhängigkeit (dieselbe
    Version wie über `libsqlite3-sys`). Die Testhilfe setzt `KLEIN_BUCH_SIDECAR_MOCK` nur noch
    einmal je Prozess.
99. **Sidecar aus geprüften Quellen (W11).** `src-tauri/sidecar/` enthält `lib/` (KoSIT-Validator
    1.6.2 = `validator-1.6.2-standalone.jar` von GitHub, Mustang 2.23.0 = `Mustang-CLI-2.23.0.jar`
    von Maven Central — beide bytegleich mit den bisher gebündelten JARs), `xrechnung-config/`
    (Release-ZIP 2026-01-31 entpackt; der bisherige Stand war bis auf CRLF-Zeilenenden gleich),
    `versions.json` (Schema 2: je Komponente Quelle + SHA-256, jlink-Module ohne
    `jdk.crypto.mscapi`, JDK-Hersteller/Version/Image-Digest, Basis-Images, Angaben der
    Windows-Releases unter `desktop_releases_2026_5`) und `klein-buch-java.sh`. Der Image-Workflow
    lädt die Originale und vergleicht SHA-256 bzw. den entpackten Baum. Windows-JRE, `.bat` und
    `scripts/build-sidecar.ps1` sind entfernt. `.gitattributes` schließt `sidecar/lib` und
    `sidecar/xrechnung-config` von der Zeilenend-Umwandlung aus (der Veröffentlichungs-Klon nutzt
    `core.autocrlf=input`; vier Dateien des ZIPs haben CRLF — der erste Image-Lauf meldete
    deshalb Abweichungen).
100. **Launcher (W11).** `klein-buch-java.sh` (POSIX-sh, LF) reicht das Subcommand-Wort nicht an das
    JAR weiter; Java aus `sidecar/jre`, sonst `JAVA_HOME`, `KLEIN_BUCH_JAVA` (Tests) oder `java`;
    JVM `-XX:-UsePerfData -Xmx512m -Djava.io.tmpdir=$TMPDIR -Djava.awt.headless=true`. Der Server
    startet ihn über `/bin/sh <launcher>` — Nextcloud und das Veröffentlichen übertragen kein
    Ausführbar-Bit (im Image zusätzlich `chmod 0755`).
101. **XRechnung-Fund (W11, S1).** Mit dem echten KoSIT-Validator (`sidecar_real`-Tests gegen eine
    lokal per `jlink` gebaute JRE) lehnte KoSIT jede §19-Rechnung als Schemafehler ab: In
    `IncludedNote` stand `SubjectCode` (R3-001) vor `Content`, das CII-Schema verlangt
    `Content*, SubjectCode?`. Das Festschreiben bricht bei `failed` ab — im Image wäre keine
    Rechnung ausstellbar gewesen (bis W10 liefen alle Tests mit dem Sidecar-Mock). Behoben im
    Generator, `r3_review_test` nachgezogen; die Desktop-Releases v2026.5.x enthalten dieselbe
    Reihenfolge (wie sich das dort auswirkte, ist nicht nachgeprüft).
102. **Image (W11).** `klein-buch/Dockerfile`: Stufen Node (`pnpm install --frozen-lockfile`,
    Build der SPA samt Handbuch) → Rust (`rust-toolchain.toml` 1.99.1, `cargo auditable build
    --release --locked`, cargo-auditable 0.7.7, Commit als spätes `ARG`) → JRE (`jlink` aus
    `eclipse-temurin:21.0.12_8-jdk`) → `debian:trixie-slim` mit `tzdata`, `fontconfig`,
    `fonts-dejavu-core`, `libfreetype6`; alle Basis-Images mit Digest. `/opt/klein-buch`
    enthält `web/`, `inputs/` (Seed-Quelle aus `klein-buch/inputs`), `sidecar/` (+ `jre/`) und
    `legal/`; `KLEIN_BUCH_*`-Pfade, `TZ=Europe/Berlin`, `HOME=/tmp`, `TMPDIR=/data/tmp` als
    `ENV`; kein `VOLUME`, `USER 65532:65532`, `HEALTHCHECK` über den Subcommand. Der Server setzt
    beim Start `PR_SET_DUMPABLE=0`. `compose.example.yaml` mit `read_only`, `tmpfs /tmp`,
    `cap_drop: [ALL]`, `no-new-privileges`, `ulimits.core: 0`, `pids_limit`, `mem_limit`,
    `stop_grace_period`, Log-Rotation.
103. **Seed-Manifest (W11).** `/inputs/.seed-manifest.json` (Pfad → SHA-256 der zuletzt
    geseedeten Fassung, Image-Version, offene Hinweise): fehlt eine Datei → kopieren;
    unverändert und das Image bringt eine neue Fassung → ersetzen; vom Nutzer geändert (auch
    ohne Manifest-Eintrag, wenn abweichend) → nie anfassen, neue Fassung als `<name>.neu`. Die
    Hinweise wandern beim nächsten Entsperren in die Inbox (die DB ist beim Start zu). Dateien
    werden über Temp-Datei + `rename` geschrieben.
104. **Image-CI (W11).** `.github/workflows/image.yml` auf `phase-w`: nur `contents: read`, kein
    Push, Build mit buildx-GHA-Cache und `load: true`; Smoke im selben Job: Größe < 400 MB,
    Mock-Schalter verweigert den Start, Container `--read-only --tmpfs /tmp --cap-drop ALL
    --security-opt no-new-privileges --user 568:568` mit Bind-Mounts, Healthcheck,
    `id -u`, `date` in Europe/Berlin, Schreibversuch auf `/`, `CapEff` 0, Seed-Manifest; dann
    über HTTP (`.github/scripts/image_smoke.py`): Einrichtung mit Setup-Token + TOTP,
    Festschreiben mit echtem KoSIT + Mustang (PDF/A-3), ZUGFeRD-PDF im Rechnungseingang
    extrahiert und gültig, manipulierte XRechnung abgelehnt. Abweichung vom Plan: Die
    `sidecar_real`-Tests laufen lokal gegen die jlink-JRE; im Image-Job decken die HTTP-Schritte
    dieselben Wege ab (im Image gibt es keine Rust-Toolchain).
105. **Altbestand (W11).** Entfernt: Factory-Reset-Code (`commands/`, `backup/`,
    `domain/factory_reset.rs`; der Start verweigert einen gefundenen Reset-Marker weiterhin),
    Drop-Folder (`scheduler/`, `domain/drop_folder.rs`, Commands, `drop_folder_sync_test.rs`;
    Migration 0029 bleibt), `src-tauri/resources/inputs/`, `src-tauri/gen/`,
    `.github/workflows/release.yml`, `config::sidecar_target_triple`. Die Registry hat 183
    Commands.
106. **BR-CO-25 (W11-Fund im Image-Smoke).** KoSIT verlangt bei offenem Betrag ein
    Fälligkeitsdatum (BT-9) oder Zahlungsbedingungen (BT-20). `validate_for_issue` lehnt eine
    positive Rechnung ohne Fälligkeitsdatum jetzt vorab mit klarer Meldung ab
    (`XRechnungDueDateMissing`), wie W1.11 bei der Telefonnummer; Zahlungsbedingungen werden
    nicht erfunden. Die Oberfläche belegt das Fälligkeitsdatum ohnehin vor, Abo-Rechnungen
    setzen es aus dem Zahlungsziel, Storno-Belege sind ausgenommen.
107. **Java-Inventar und -Hinweise (W12).** `src-tauri/sidecar/THIRD-PARTY-JAVA.md` (von Hand
    gepflegt) führt jede `pom.properties`-Koordinate beider JARs (94 eindeutige) und die
    Komponenten ohne `pom.properties` (Saxon-HE 12.8/12.9, xmlresolver, Rhino 1.7.13, dom4j
    2.1.5, stax-utils, xml-apis-ext, picocli 4.7.7) mit SPDX und Quell-Link; Herkunft geprüft,
    indem jede `.class` gegen das Original-Artefakt verglichen wurde. `sidecar/notices/` (59
    Dateien) enthält die NOTICE-/LICENSE-Texte aus den Original-Artefakten bzw. Repo-Tags,
    `sidecar/copyleft-sources.txt` die Quellen der MPL-/EPL-/EUPL-/GPL-Teile fürs Quellarchiv.
    CI prüft mit `legal/check_java_inventory.py`. Funde über den Plan hinaus: `mustang.jar`
    enthält GNU FreeSans (GPL-3.0-or-later mit Font-Ausnahme; SFD-Quelle im Quellarchiv) und
    eine zweite CEN-Schematron-Fassung (1.3.12, EUPL-1.2, ebenfalls im Quellarchiv); die
    Factur-X-/ZUGFeRD-Schemata tragen keine Lizenzangabe (Inventar: NOASSERTION, verbreitet als
    Teil von Mustang); `verapdf-xmp-core-jakarta` wird wegen der Quelltextköpfe als MPL-Teil
    behandelt.
108. **Lizenzseite im Image-Build (W12).** Dockerfile-Stufen `legal-tools` (cargo-about 0.9.2
    mit `--features cli`, parallel zum Server-Build), `legal` (auf der Server-Stufe: `cargo
    metadata --filter-platform x86_64-unknown-linux-gnu`, `cargo about generate --fail`,
    `legal/build_legal.py licenses`) und `legal-sources` (`build_legal.py sources` mit der
    `dpkg-query`-Liste der Laufzeit-Basis `runtime-base`, nach der kein `apt-get install` mehr
    folgt). Ergebnis in `/opt/klein-buch/legal` und `/usr/share/doc/klein-buch`:
    `third-party-licenses.html` (Rust, mitgelieferte NOTICE-Dateien inkl. SQLCipher/Zetetic und
    OpenSSL, eingebettete Schriften, CSL-Stile von hayagriva mit CC BY-SA 3.0 — vorsichtshalber
    als mitgeliefert gezählt —, Java, JS, weitere Hinweise), `LICENSE.txt` (+ `LICENSE`),
    `legal.css`, `quellen.html`, `SOURCES.md`, `debian-pakete.tsv`. Der Build scheitert, wenn ein
    Pflichtteil oder eine der Zeichenfolgen „SIL OPEN FONT LICENSE“, „GUST Font License“,
    „Zetetic“, „BY-SA 3.0“ fehlt oder „Dev-Build-Platzhalter“ vorkommt. Die JS-Liste entsteht im
    Vite-Build (eigenes Plugin ohne neue Abhängigkeit) aus den Modulen des Client-Bundles; der
    Build bricht ab, wenn einem Paket Lizenzfeld oder -text fehlt. Die `legal`-Stufe braucht Netz,
    weil cargo-about den Graphen für alle Plattformen auflöst. Basis-Images stehen mit Digest direkt
    in den `FROM`-Zeilen (Dependabot erkennt `FROM ${ARG}` nicht); `legal/check_base_images.py`
    prüft in CI, dass sie zu `versions.json` passen (daraus speisen sich `quellen.html`,
    `SOURCES.md` und das Quellarchiv). Die Laufzeit-Basis spielt beim Build `apt-get upgrade` ein,
    damit Debian-Sicherheitsupdates seit dem Digest nicht als offene Funde im Scan stehen.
109. **`/legal/` und Rate-Limit öffentlicher Routen (W12).** Ohne Sitzung, nur Dateien direkt in
    `<RESOURCE_DIR>/legal` (Name `[A-Za-z0-9._-]`, kein führender Punkt, Endungen html/css/txt/tsv/
    json, `canonicalize` + Elternordner-Prüfung, höchstens 32 MiB), keine Verzeichnislisten,
    `/legal` → 308 auf `/legal/`. Eigene Header-Klasse: CSP ohne Skripte (`default-src 'none';
    style-src 'self'; img-src 'self'`), `no-cache`. Neu: Rate-Limit 120 Anfragen je 60 s und
    Client-IP (IPv6 je /64) gemeinsam für `/api/public/info` und `/legal/*`, darüber 429 mit
    `Retry-After`.
110. **AGPL §13 (W12).** Build-Werte `KLEINBUCH_SOURCE_URL`, `KLEINBUCH_BUILD_COMMIT`,
    `KLEINBUCH_BUILD_TAG` (spätes `ARG`/`ENV` vor `cargo build`; `KLEIN_BUCH_COMMIT` entfällt),
    Laufzeit-Override `KLEIN_BUCH_SOURCE_URL` (nur http(s) ohne Benutzerangaben/Query, sonst
    Warnung und ignoriert). `GET /api/public/info` liefert `{version, commit, tag, sourceUrl,
    sourceLink, licenseSpdx}`; `sourceLink` = `<url>/releases/tag/<tag>` bzw.
    `<url>/tree/<commit>`, ohne Commit `null` und die Oberfläche zeigt „Entwicklungs-Build,
    Quellstand unbekannt“ — nie `main`. Anzeige auf Anmeldung/Einrichtung (`SourceNotice`) und im
    Über-Dialog; `app_info` nutzt dieselbe Quelle, die festen URLs sind entfernt. Der Über-Dialog
    verweist für Lizenzen, NOTICE-Dateien und Quellen auf `/legal/`.
111. **Opt-in Versionsprüfung (W12, E14).** `app_settings.update_check_enabled` (Standard aus);
    der Scheduler-Tick fragt nur entsperrt und höchstens alle 24 h (Zeitstempel vor der Abfrage,
    also auch nach Fehlschlag) `…/releases/latest` ab — aus `KLEIN_BUCH_UPDATE_URL`, sonst aus der
    Quell-URL, sonst `hozwei/klein.buch-public`. Nur GET mit festem User-Agent
    „Klein.Buch-Versionspruefung“, ohne Weiterleitungen, Antwort ≤ 512 KiB; Entwürfe und
    Vorabversionen zählen nicht. Eine neuere CalVer ergibt einen Inbox-Eintrag mit Name und Link
    nur als Text (Steuer-/Bidi-Zeichen entfernt, kein `action_url`), je Tag einmal. Schalter unter
    Einstellungen → Sicherheit & Anmeldung; Commands `update_check_get`/`update_check_set`
    (Registry 185, `api.ts` 167). Test: ohne Opt-in keine Verbindung.
112. **Release-Infrastruktur (W12).** `.github/workflows/release-image.yml`: Trockenlauf auf
    `phase-w` (nur `contents: read`): OCI-Layout mit SBOM- und Provenance-Attestierung, per skopeo
    in Docker geladen (Schicht-IDs verglichen), `.github/scripts/image_smoke.sh` (alle
    Image-Prüfungen aus W11 plus Lizenz-/Quelldateien, `/legal/` und `/api/public/info` mit dem
    Build-Commit), syft-SBOM, trivy (CRITICAL/HIGH mit Fix, Ausnahmen in `.trivyignore` mit
    Ablaufdatum), Quellarchiv (`.github/scripts/build_sources_archive.sh`: OpenJDK-Quellen des
    exakten Temurin-Builds + jlink-Befehl, Debian-Quellpakete über snapshot.debian.org,
    `-sources.jar`/Tag-Archive aus `copyleft-sources.txt`) als Artefakt. Release-Lauf nur auf
    `v*`-Tags (Tag = Version in `Cargo.toml`): ohne Cache bauen → Smoke → SBOM → Scan →
    Quellarchiv → Push mit `--preserve-digests` (ohne Quellarchiv kein Push) → cosign keyless +
    verify → Release-Entwurf mit Digest, Quellarchiv, SBOM. Abweichung von W11: `image.yml` prüft
    nur noch die Sidecar-Quellen; Build und Smoke des Images laufen im Trockenlauf (sonst zwei
    Image-Builds je Push). Der Release-Job baut nur, wenn der Tag-Commit auf `main` liegt;
    `Veroeffentlichen.ps1` setzt Tags nur mit `-Branch main`.
113. **Gates und Pins (W12).** Neue CI-Jobs (nur `contents: read`): cargo-deny 0.20.2
    (`advisories licenses`, `deny.toml` mit derselben Allowlist wie `about.toml`), cargo-audit
    0.22.2 (ganzes `Cargo.lock`, dieselben Ausnahmen), `pnpm audit --prod --audit-level high`,
    gitleaks 8.30.1, Java-Inventar. Alle Actions per Commit-SHA, Werkzeuge per SHA-256.
    `.github/dependabot.yml` (cargo, npm, Actions, Docker; wöchentlich, gruppiert) — die PRs sind
    nur Hinweise, übernommen wird im Entwicklungsordner. Der RFC-6238-Testvektor in `totp.rs` ist
    für gitleaks markiert.
114. **Abhängigkeiten wegen RustSec (W12).** Für grüne Gates aktualisiert: russh 0.60 → 0.64
    (crypto-bigint-Timing; API: `PublicKeyOrCertificate`, `ChannelOpenHandle`), quick-xml 0.36 →
    0.42 (DoS beim Parsen; Entity-Referenzen kommen jetzt als eigene Events — der
    E-Rechnungs-Parser sammelt den Text je Element und ordnet ihn am Element-Ende zu, neuer Test),
    lopdf 0.40 → 0.46 (Stack-Überlauf), plist 1.10.3, rustls 0.23.46, rustls-webpki 0.103.16,
    webpki-roots 1.0.11, quinn-proto 0.11.19, tokio 1.54.1, anyhow 1.0.104 u. a. Begründete
    Ausnahmen: rsa/Marvin (kein Fix), drei zip-Meldungen zu `extract` (nicht genutzt, Fix erst in
    zip 9), quick-xml 0.36 über typst 0.13 → hayagriva → citationberg (nur eingebettete bzw. vom
    Betreiber geladene CSL-Daten; weg mit typst ≥ 0.14). `zip` nur noch mit `deflate` + `time`:
    zstd, bzip2 und xz samt statisch gelinktem C-Code entfallen (sonst fehlten deren
    BSD-Hinweise auf der Lizenzseite), fremde ZIPs mit anderen Verfahren werden abgelehnt.
    trivy meldet 11 HIGH-Funde im unveränderten Mustang-Uber-JAR (jackson-core/-databind 2.17.3,
    assertj-core 2.9.0, veraPDF 1.26.5). Ein Class-Load-Trace der JVM über `combine`, `extract`
    und den KoSIT-Validator lädt keine Klasse dieser Bibliotheken — begründete Ausnahmen in
    `.trivyignore` (Ablauf 2027-04-30). Mustang 2.26 behebt nur einen Teil (Jackson 2.22.2); ein
    Update folgt, sobald es alle Funde behebt.
115. **Handbuch und Betriebsdoku (W13).** Neue Handbuch-Kategorie `betrieb` (Installation mit
    Docker Compose, Reverse-Proxy und TLS, Volumes und Rechte, TOTP, Updates und Rückfall,
    Notfall-Wiederherstellung, Speicher, Fehlersuche im Container inkl. fail2ban-Filter auf die
    Zeilen `auth.(login|setup|reauth).failed ip=… reason=…`, Lizenz und Quellcode). Entfallen:
    `factory-reset`, `troubleshooting-oauth`, `troubleshooting-smartscreen`; `eingangsrechnungen-via-ordner`
    wird zu `rechnungseingang-per-upload`. Zwei Korrekturen an diesem ADR (Rückfall in E15,
    Folgen fehlender `TRUSTED_PROXIES` unter „Standardentscheidungen“) nach Abgleich mit dem Code.
    Ein falscher TOTP-Code bei der Einrichtung schreibt jetzt ebenfalls eine
    `auth.setup.failed`-Zeile (vorher nur Token und Passphrase). `SECURITY.md` und `CONTRIBUTING.md` neu,
    `docs/release-signing-guide.md` entfernt; `compose.example.yaml` wird in CI mit
    `docker compose config` geprüft.
116. **Security-Review W14.** Vier Prüfer je ASVS-4.0.3-L2-Kapitelgruppe (V2/V3, V4/V5/V12/V13,
    V7/V8/V14, Geschäftslogik), S0/S1 gegengeprüft; Befunde W14-NNN und Checkliste in
    `Buchhaltung/docs/REVIEW-PHASE-W.md` (nicht veröffentlicht). Kein S0; drei S1 behoben (Nr. 117,
    118, 119). Verschoben mit Begründung in den Folgelauf 2026.11 (U5): Kosten-Abos ohne Sperre/CAS
    (Doppelbuchung nur bei manuellem Auslöser parallel zum Tick), Pause nach Restore, Absturzfestigkeit
    der Abo-Läufe, Nachholen ohne Bestätigung, Sperren ohne Zeitgrenze. Offen bleibt die bewusste
    Frage „Storno im Folgejahr behält GJ/Nummernkreis“ (Freigabe).
117. **Anmeldung ohne Aussperrung (W14).** Ein gültiges Gerätecookie wird bei jeder erfolgreichen
    Einrichtung, Anmeldung und Re-Auth mit gleichem Wert und frischer Laufzeit neu gesetzt (gleitend
    statt fest 1 Jahr). Ein Login mit Einmal-Code (ohne TOTP-Feld) zählt nur unter dem eigenen
    Schlüssel, nicht im globalen Budget — ein Angreifer mit vielen Adressen kann das Budget sonst
    dauerhaft ausschöpfen und den Inhaber auf jedem neuen Gerät aussperren; die Passphrase bleibt
    Pflicht, Raten der Einmal-Codes ist aussichtslos.
118. **Langsame Körper (W14).** Kleine JSON-Körper (Commands, Anmeldung) werden höchstens 10 s
    gelesen (sonst 408 `body_timeout`); höchstens 16 gleichzeitige POST-Anfragen je Client-Adresse
    (IPv6 je /64), GETs ausgenommen (SPA hinter Proxys ohne `TRUSTED_PROXIES`). Uploads streamen und
    verlangen eine Sitzung.
119. **Storno-Datum (W14, GoBD).** Das Storno-Datum muss zwischen Rechnungsdatum und heute liegen und
    in einem offenen Geschäftsjahr — sonst änderte sich die EÜR eines festgeschriebenen Jahres
    nachträglich (wie beim Kosten-Storno R2-003 und bei Zahlungen); die Oberfläche begrenzt das
    Datumsfeld entsprechend.
120. **Sitzungen und Re-Auth (W14).** Die Re-Auth rotiert die Sitzungs-ID (neue ID mit frischer
    Re-Auth; die alte verliert das Re-Auth-Recht und gilt noch 10 s für laufende Anfragen) — ein
    gestohlenes Cookie endet mit der Bestätigung des Inhabers. TOTP- und Einmal-Code-Erneuerung beenden
    alle anderen Sitzungen. Re-Auth-pflichtig zusätzlich: `seller_profile_upsert` (IBAN/BIC, §19-Status),
    `payment_accounts_create`/`_update`, `contacts_anonymize`, `fiscal_year_close` (21 Commands); eine
    Anmeldung zählt bewusst nicht als Re-Auth. Versand bleibt ohne Re-Auth (täglicher Gebrauch,
    Mail-Protokoll).
121. **Weitere Härtung (W14).** Vorlagennamen sind einzelne Dateinamen (kein Pfadtrenner, kein
    führender Punkt) — sonst lief eine hochgeladene Datei als Typst-Vorlage. `Cache-Control: immutable`
    nur für erfolgreiche Antworten (eine 503 unter `/_app/immutable/` hätte den Browser dauerhaft
    gebrochen). Sidecar-Launcher mit `ulimit -f` (64 MiB) und Größenprüfung des extrahierten XML
    (10 MiB) gegen Dekompressionsbomben in fremden ZUGFeRD-PDFs; lopdf lädt hochgeladene PDFs mit
    Dekompressionsgrenze. Angriffstests: XXE (Datei/Netz) gegen den Rust-Parser, KoSIT und
    Mustang-Extract (lokaler Zähl-Listener, 0 Abrufe), 512-MiB-Bombe, Log-Scan inkl. TOTP-Geheimnis und
    SMTP-Passwort. CI: `security.yml` (Härtung per `docker inspect`, Abnahme-Ablauf, OWASP ZAP Baseline +
    API-Scan über alle Commands mit Sitzung, Gate mit `.github/zap-ausnahmen.tsv`); `image_smoke.py`
    mit Abnahme-Ablauf (Zahlung, Backup, Restore samt Neustart und Anmeldung, Export) — Container daher
    mit `--restart unless-stopped`. ZAP-Gate: Ausnahmen je alertRef oder Plugin-ID mit Begründung;
    einzige Ausnahme `10055-6` (CSP `style-src 'unsafe-inline'`, nötig für Svelte-Stilattribute;
    `script-src` bleibt ohne `unsafe-inline`). Die 30-s-Hänger im lokalen Gate: in W14 ein Testfehler
    (ein neuer Test meldete sich ohne die KDF-Serialisierung der Testhilfen an, behoben), sonst ein
    Artefakt des überbuchten Test-Hosts (PBKDF2 je neuer Verbindung, viele parallele Testserver; in CI
    nie): kein automatischer Wiederholversuch im Produktcode.

122. **Storno-Geschäftsjahr und Bezug aufs Original (Lauf E, 2026-10-24; löst Nr. 18 ab).** Ein
    Storno-Beleg bekommt Geschäftsjahr und Nummer aus seinem **Storno-Datum** (`ST-2026-0001`
    vom 15.01.2026 zu `RE-2025-0042`), nicht mehr aus dem Original. Grundlage (Rechtsprüfung mit
    Gegenprüfung, `Buchhaltung/docs/ENTSCHEIDUNGEN-UMSETZUNG-PLAN.md` §3): GoBD Rz. 51 („Zwingend
    ist die Zuordnung zum jeweiligen Geschäftsjahr“, in dem der Vorgang anfällt), §4a Abs. 1 S. 2
    Nr. 3 EStG, §11 EStG, §17 Abs. 1 S. 7 UStG (bei Verzicht auf §19), §147 Abs. 4 AO; den
    Nummernkreis regelt das Gesetz nicht (UStAE 14.5 Abs. 10/11, §34a UStDV ohne Nummernpflicht
    bei §19). Ein abgeschlossenes Vorjahr bekommt damit keine neuen Belege. Der Bezug aufs
    Original (§31 Abs. 5 und §34a S. 2 UStDV, GoBD Rz. 64) steht im XML als **BT-25 = Nummer**
    und **BT-26 = Rechnungsdatum** der stornierten Rechnung — bisher stand in BT-25 die interne
    ID (`is_storno_for`), korrigiert — und auf dem PDF als „Storno zur Rechnung <Nr> vom <Datum>“
    unter der Überschrift „Stornorechnung“, mit „Stornobetrag“ und ohne Überweisungshinweis
    (alle vier Rechnungsvorlagen; eigene Vorlagen lesen `data.invoice.storno_of` über
    `.at(…, default: none)` und rendern unverändert weiter). Die EÜR rechnete schon nach Datum
    (§11 EStG) und bleibt unverändert. Bestehende Stornos bleiben, wie sie sind (festgeschrieben).
    Beim Prüfen mit dem echten KoSIT-Validator fiel auf, dass jedes Storno an **BR-27** scheiterte
    (negativer Einzelpreis BT-146; bis W10 lief alles mit dem Sidecar-Mock, die Abnahme in W14 hat
    nicht storniert). Der Generator schreibt bei negativem Preis das Vorzeichen in die Menge
    (BT-129) und den Betrag des Preises; `image_smoke.py` storniert seither mit dem echten
    Validator (Release-Trockenlauf, Security-Scan). Nach der Gegenprüfung: Storno-Entwürfe entstehen
    nur über `invoices_cancel` (`invoices_create_draft` lehnt `is_storno_for` ab),
    `invoices_lock_and_issue` schreibt keinen Storno-Entwurf fest (sonst Storno ohne Original und
    doppelte Erstattung in der EÜR), `invoices_cancel` verwendet einen liegengebliebenen Entwurf
    desselben Jahres wieder; `create_invoice_draft_from_input` prüft `ensure_year_open` vor der
    Nummernvergabe. Storno-Mails nutzen eine eigene Vorlage (`storno-de`, eingebettet).
123. **Quellangebot der Komponenten Dritter (Lauf E, 2026-10-24).** Das Quellarchiv am Release
    genügt den Lizenzen (GPLv2 §3 letzter Absatz und LGPL-2.1 §4 Abs. 2 „from the same place“ —
    nach FSF-FAQ genügt ein Verweis neben dem Programm; GPLv3 §6(d) anderer Server mit klarem
    Hinweis; MPL-2.0 §3.2, EPL-2.0 §3.1, EUPL-1.2 Art. 5); ein schriftliches Angebot nach GPLv2
    §3(b) gibt es nicht — es wäre bei GPLv3 §6(b) nur für physische Produkte zulässig und bei
    LGPL-2.1 §4 nicht vorgesehen. Gehärtet: (1) Der Release-Job veröffentlicht das Release mit
    Quellarchiv, SBOM und Digest **vor** dem Image-Push (bei v2026.10.0 war das Image rund
    25 Minuten öffentlich, während das Archiv noch im Entwurf lag); Wiederholungen laden nur
    fehlende Assets hoch, ersetzen nie. (2) Hinweis dort, wo das Image geladen wird: README
    (erscheint auf der GHCR-Paketseite) und Label `de.klein-buch.source-archive` mit der direkten
    Archiv-Adresse. (3) Aufbewahrung: Release-Assets bleiben, solange ein zugehöriges Image
    abrufbar ist; das Repository bleibt öffentlich (`build-and-release.md`,
    `tools/veroeffentlichen/LIESMICH.md`, Handbuch „Lizenz und Quellcode“). Wiederholungen: Weicht
    der Digest eines bestehenden Releases vom neuen Build ab, wird nur ersetzt, wenn zum alten
    Digest nie ein Image in GHCR lag; sonst Abbruch. Registry-Abfragen unterscheiden „nicht
    vorhanden“ von Fehlern; der Release-Checkout hält kein Schreib-Token für git.
124. **Migration 0032: GoBD- und Schema-Absicherungen (Lauf F, Block F2a, 2026-10-24).** Die
    einzige Migration von Lauf F verschärft nur, kein Trigger wird gelockert, kein
    festgeschriebener Beleg geändert (Schema 32, nächste freie Migration 0033). Entschieden:
    (1) **Höchstens ein festgeschriebener Storno je Original** als Partial Unique Index
    `ux_invoices_one_locked_storno` (`WHERE is_storno_for IS NOT NULL AND locked_at IS NOT
    NULL`); offene Storno-Entwürfe bleiben DB-seitig mehrfach möglich (`invoices_cancel`
    verwendet einen offenen wieder, verworfene nicht). Gibt es im Bestand schon zwei
    festgeschriebene Stornos zu einem Original, **bricht die Migration ab** (benannter CHECK auf
    einer temporären Tabelle, Meldung „Migration 0032 abgebrochen: …“): bereinigen ließe sich das
    nur durch Ändern festgeschriebener Belege. Die Transaktion rollt zurück (seit F2c lehnt schon die
    Vorabprüfung vor dem Pflicht-Backup ab, Nr. 126 (1)), das Entsperren antwortet 409
    `db_integrity_failed`, das Handbuch
    („Fehlersuche im Container“) nennt Rückfall auf das vorige Image bzw. Restore. Die
    Index-Bedingung wurde nicht aufgeweicht, weil eine Doppel-Erstattung in der EÜR genau der
    Fehler ist, den der Index verhindern soll. (2) **Integritäts-Historie:** `files_missing`,
    `files_unreadable` und die ID-Listen als JSON (per `serde_json`) sind **nullbar** statt
    `NOT NULL DEFAULT 0` — für Läufe vor 0032 sind die Werte nicht erfasst, eine 0 behauptete
    „nichts fehlte“. (3) **Entwurf verwerfen (W14-032)** über `voided_at` + `void_reason` statt
    eines neuen Status: ein neuer Wert im `status`-CHECK hieße Tabellenumbau von `invoices` mit
    allen Triggern und Fremdschlüsseln; `status` bleibt `draft`. `invoices_void_draft` mit
    Re-Auth (unumkehrbar, die Nummer bleibt verbraucht) und Begründung ≥ 10 Zeichen (wie
    `numbering_raise`), Audit `invoice.draft_voided` in derselben Transaktion, die auch einen
    Abo-Fortsetzungs-Marker und eine Restore-Vormerkung auf den Entwurf entfernt. Kein DELETE:
    Der verworfene Entwurf dokumentiert die Nummer (GoBD Rz. 58 ff., Lücke zulässig, wenn
    dokumentiert; §14 Abs. 4 Nr. 4 UStG verbietet nur die Doppelvergabe); die Anonymisierung des
    Kontakts blockiert er nicht mehr. **Empfängerdaten (Amendment F2c, 2026-10-25; ersetzt „bleibt
    mit seinem Empfänger-Stand wie die übrigen Unterlagen aufbewahrt“):** Ein nie ausgestellter,
    verworfener Entwurf ist weder Buchungsbeleg noch empfangener oder abgesandter Handelsbrief
    (§147 Abs. 1 AO) noch eine ausgestellte Rechnung (§14b UStG); Art. 17 Abs. 3 lit. b DSGVO trägt
    seine Aufbewahrung deshalb nicht. Für die Dokumentation der Nummernlücke genügen Nummer,
    Geschäftsjahr, Datum, Betrag, Positionen, Zeitpunkt und Grund des Verwerfens. Darum entfernt die
    Anonymisierung des Kontakts (`contacts::anonymize_with_pauses`, eine Transaktion samt Audit
    `contact.anonymize` mit den betroffenen Nummern) den Empfänger-Snapshot: `buyer_name` wird der
    Platzhalter des anonymisierten Kontakts, `buyer_street`, `buyer_postal_code`, `buyer_city`,
    `buyer_vat_id`, `buyer_email` und die interne `notes` werden `NULL`; das Land bleibt (nicht
    identifizierend), ebenso `void_reason` (Handbuch: keine Namen in den Grund). Dafür hat
    `trg_invoices_voided_final` die **einzige, eng begrenzte Ausnahme**: zulässig ist nur genau
    diese Änderung, nur wenn `contacts.anonymized_at` gesetzt ist und `buyer_name` dem Platzhalter
    des Kontakts entspricht; jede andere Spalte ist im Trigger einzeln als unverändert geprüft
    (auch `updated_at`), ein Test vergleicht die Liste mit `PRAGMA table_info(invoices)`, damit eine
    künftige Spalte nicht stillschweigend änderbar wird. Kein Trigger wird dadurch für
    festgeschriebene Belege gelockert. Gleich behandelt werden zurückgezogene, nie festgeschriebene
    Angebote (ohne Trigger-Änderung, `trg_quotes_canceled_final` schützt Status und Rückzug).
    Positionen bleiben: Sie dokumentieren, wofür die Nummer verbraucht wurde. Verwerfen geht auch in einem
    abgeschlossenen Geschäftsjahr (ein Entwurf ist nicht Teil der EÜR). Danach ist der Entwurf
    unveränderlich und nicht festschreibbar (`trg_invoices_voided_final`,
    `trg_invoices_void_draft_only`, `trg_invoices_void_not_on_insert`). **Angebote** bekommen keine
    eigene Spalte: Das bestehende „Zurückziehen“ (`status = 'canceled'` mit Zeitpunkt, Grund,
    Audit) verwirft einen Angebots-Entwurf bereits; zurückgezogen ist jetzt endgültig
    (`trg_quotes_canceled_final`), ohne Re-Auth (ein Angebot ist kein Buchungsbeleg). (4)
    **Positionen** festgeschriebener oder verworfener Rechnungen und festgeschriebener oder
    zurückgezogener Angebote: INSERT, UPDATE (auch Verschieben in einen solchen Beleg) und DELETE
    scheitern (KB-0002). (5) **`archive_entries` ohne DELETE** (KB-0004; kein App-Pfad löscht).
    (6) **Storno endgültig:** `trg_invoices_canceled_final` hält `status`, `canceled_at`,
    `canceled_by_storno_id`, `cancel_reason` nach dem ersten Setzen fest; die nur noch von Tests
    genutzte Repo-Funktion `mark_canceled` ohne Guard ist entfernt (R1-S2-MARKCANCELED). (7)
    **Anlagen-Abgang endgültig** (`trg_assets_disposal_final`, `dispose` mit `AND disposed = 0`);
    Buchwert-Fortschreibung bleibt offen. (8) **Ein Standard-Zahlungskonto** als
    `uq_payment_accounts_default` mit `WHERE is_default = 1` — strenger als der Vorschlag
    `AND active = 1`, weil `ensure_one_default` ein inaktives Konto ohnehin nie Standard sein
    lässt und zuerst räumt, dann befördert. Bestand: inaktive Standardkonten und von mehreren
    aktiven alle außer dem ältesten (wie `ensure_one_default`) verlieren den Standard, je Konto
    Audit `payment_account.default_cleared` (Stammdaten, nicht festgeschrieben). (9)
    **Fremdschlüssel-Prüfung bei jedem Entsperren** (`PRAGMA foreign_key_check` nach den
    Migrationen, R1-012), nicht nur nach einer angewendeten Migration: billig und erkennt auch
    eine Bearbeitung von außen. **Amendment F2c:** stehen Migrationen an, prüft
    `db::precheck_before_migration` (Fremdschlüssel und doppelte festgeschriebene Stornos)
    zusätzlich **vor** dem Pflicht-Backup und vor der ersten Migration — sonst sperrte ein Altfehler
    die Instanz auf dem neuen Schema aus, auf dem das vorige Image sie nicht mehr öffnet
    (Nr. 126 (1)). Ein Treffer blockiert das Entsperren (409
    `db_integrity_failed`), Daten werden nicht angefasst; das Log nennt Tabelle, rowid und
    Zieltabelle, keine Inhalte. Dieselbe Antwort gilt für jede künftige Vorab-Prüfung einer
    Migration nach dem Muster „CHECK constraint failed: Migration 00NN abgebrochen: …“. (10)
    **Seed `rule_recurring_due`** (ADR 0040 Nr. 8) mit `deliver_os_native = 0`; der Code verlangt
    die Regel-Zeile jetzt wie alle anderen Regeln. (11) **Phase A eines Restores** prüft
    beides schon vor dem Tausch (Fremdschlüssel; doppelte festgeschriebene Stornos über
    `db::duplicate_locked_stornos`, dieselbe Bedingung wie in der Migration) und lehnt das
    Backup ab — wie bei den Archivpfaden (Gegenprüfung W7) bliebe die Instanz sonst nach dem
    Tausch dauerhaft gesperrt. Tests: `tests/migration_0032_test.rs`,
    `tests/server_auth_test.rs` (`foreign_key_violation_refuses_unlock`),
    `tests/server_restore_test.rs` (`schema_31_backup_is_restored_and_migrated_forward`,
    `backups_the_unlock_would_refuse_are_rejected_in_phase_a`),
    `scheduler::recurring_invoice::pipeline_tests::pending_marker_on_voided_draft_is_dropped`.
125. **Audit, Exporte, Jahresabschluss, Archiv, DSGVO (Lauf F, Block F2b, 2026-10-25; keine
    Migration).** (1) **Audit-Details nur als JSON-Wert** (W14-010, R2-S2-AUDITJSON):
    `audit_log::append`/`append_in` und `archive::audit::archive_event` nehmen
    `Option<serde_json::Value>` und serialisieren selbst; die neun `escape`-Kopien und alle
    `format!`-JSON-Stellen sind entfernt. Die Signatur macht handgebautes JSON unmöglich (Typfehler
    statt Review-Befund). (2) **Akteur** (R1-S3-ACTOR) statt „bewusst schließen“: der Aufwand ist
    gering und die Verfahrensdoku gewinnt die Unterscheidung Handlung/Automatik. Task-lokaler Wert
    (`audit_log::with_actor`): `owner` setzt der Dispatcher für Commands und Jobs und die Anmeldung
    für ihre erfolgreichen Schritte, `scheduler` der Tick, sonst `system`. Ein eigener
    `tokio::spawn` erbt den Wert nicht (fällt auf `system`) — bewusst, solche Tasks sind
    Automatik; Bestandseinträge bleiben `system`. (3) **Jeder Export auditiert** (R2-016, ASVS
    8.3.5): `euer.pdf_export`, `euer.datev_export`, `euer.elster_export`, `euer.detail_export`
    neben dem bestehenden `euer.stb_export`; Details Geschäftsjahr, Dateiname, Zählwerte,
    Snapshot-Status, keine Inhalte, Pflicht (`?`: ohne Audit kein Download). DSGVO- und
    Migrations-Export waren schon auditiert. (4) **Snapshot-Abgleich abgeschlossener Jahre**
    (KB-0031, Variante a): weiter aus den Einzelbuchungen rechnen und gegen `fiscal_year_locks`
    prüfen, nicht aus dem Snapshot rechnen (er hat nur Summen; Einzelaufstellung, DATEV-Stapel
    und Belege entstehen nur aus den Buchungen, und Summen ohne passende Einzelposten wären nicht
    nachvollziehbar). Anzeige (`EuerPackage.snapshotCheck`, `FiscalYearStatus.snapshotMatches`)
    schreibt nichts; jeder Export schreibt bei Abweichung `euer.snapshot_mismatch` (beide
    Eckwerte, Felder, Version des Abschlusses), einen dringenden Hinweis einmal je Jahr und im
    Steuerberater-Paket einen LIESMICH-Block. Nichts wird blockiert (`gobd.md` §6.1). (5)
    **Keine automatische Löschung nach Fristablauf** (ASVS 8.3.8, entschieden): §147 Abs. 3 Satz 5
    AO verlängert die Frist, solange Unterlagen für offene Festsetzungen gebraucht werden — das
    kann die App nicht beurteilen. Nach Fristablauf entscheidet der Inhaber (Anonymisierung,
    Stilllegung der Instanz); Handbuch „Aufbewahrung 10 Jahre“, `gobd.md` §6. **Korrektur F2c:**
    Die Anonymisierung entfernt nur Stammdaten und die Snapshots nie ausgestellter Belege
    (Nr. 124 (3)); Belege mit Empfänger-Snapshot, Archivdateien, Versandprotokoll und Audit bleiben
    bis zur Stilllegung (Nr. 126 (9)). (6) **Archiv**
    (W2-S2-4, KB-0040): ohne Hardlinks `renameat2(RENAME_NOREPLACE)` (rustix), bei
    `EINVAL`/`ENOSYS`/`EOPNOTSUPP` Prüfung unmittelbar vorher und `rename` unter der Pfad-Sperre;
    die Temp-Datei wird nach `fsync` zurückgelesen und gegen den Hash des Puffers geprüft, bevor
    sie sichtbar wird (Restrisiko Seiten-Cache, abgedeckt durch Re-Hash und Integritätslauf). (7)
    **Archivpfad-Stichprobe beim Entsperren nur Präfix** (W5-ARCHIVE-PREFIX, entschieden): eine
    fehlende Datei sperrt den Zugang nicht — GoBD verlangt Erkennung und Wiederherstellbarkeit,
    keine Aussperrung vom übrigen Bestand; die Integritätsprüfung meldet „fehlt“ und historisiert
    es seit Migration 0032 (Nr. 124 (2)). (8) **Löschen ohne `exists()` vorab** (LE-CI-FLAKE):
    `db::remove_file_if_exists`, `backup::restore::remove_if_exists`, `remove_path` und
    `branding::clear_logo` löschen direkt und werten nur `NotFound` als Erfolg — SQLite entfernt
    `-wal`/`-shm` beim Schließen der letzten Verbindung selbst, das Schließen kann nachlaufen
    (Ursache des CI-Ausreißers; betraf Bestands-Migration, Passphrase-Wechsel und Restore-Tausch).
    (9) **Nummernkreise** (KB-0005): Doku begründet die Garantie mit `UPDATE … RETURNING` in einer
    Schreib-Transaktion und dem Single-Writer-Modell, Test mit Datei-DB, WAL und mehreren
    Verbindungen. (10) **DSGVO-Auskunft** (KB-0049, R2-S2-DSGVO, R1-S2-NPLUS1): feste Zahl von
    Abfragen (Positionen, Anhänge und Archiv-Metadaten je eine Abfrage, ID-Listen als ein
    `json_each`-Parameter); Versandprotokoll über Beleg-IDs, den Kontakt (Katalogversand) und die
    Adressmenge aus aktueller Adresse und den `buyer_email`-Snapshots seiner Rechnungen und
    Angebote, verglichen nach `trim` und ASCII-Kleinschreibung (dieselbe Faltung wie SQLites
    `lower()`); Audit-Bezüge nur bei passendem Paar `entity_type`/`entity_id`
    (`contact`/`invoice`/`quote`/`expense`). `euer::income_detail` ist kein N+1 (eine Abfrage mit
    korreliertem Unterselect über `uq_invoice_items_position`). (11) **Katalogversand im
    `email_log`** mit `related_id` = Kontakt (bisher leer), damit die Auskunft ihn auch nach einem
    Adresswechsel findet. (12) **Audit bei Stammdaten ohne Buchungsbezug bleibt best-effort**
    (Fund F2b): Paket-, Kategorie- und Katalog-Audits sowie die Reisekosten-Einstellung schreiben
    ihr Audit nach dem Commit und werten einen Fehlschlag nur als Log-Warnung — sie sind keine
    Buchungsdaten (Paket-Revisionen sind selbst append-only), und eine bereits gespeicherte Änderung
    darf nicht als gescheitert gemeldet werden. Buchungen, Abschluss, Exporte und
    Archivvorgänge auditieren als Pflicht bzw. in derselben Transaktion. Tests: `tests/audit_export_test.rs`, `tests/fiscal_year_guard_test.rs`,
    `tests/dsgvo_export_test.rs`
    (`gather_matches_old_addresses_case_insensitively_and_filters_audit_by_type`),
    `db::repo::audit_log::tests`, `archive::store::tests::{rename_no_clobber_never_overwrites,
    store_audit_details_are_valid_json_for_odd_names}`,
    `db::tests::remove_file_if_exists_tolerates_vanished_files`,
    `db::numbering::tests::parallel_allocations_over_several_connections_stay_gapless`,
    `fiscal_year::snapshot::tests`, `scheduler::tick::tests::tick_creates_daily_backup_without_unlock_event`
    (Akteur `scheduler`). Sperre Abschluss ↔ Buchung: ADR 0040 Nr. 48.
126. **Nachprüfung F2 (Lauf F, Block F2c, 2026-10-25; Korrekturen direkt in der unveröffentlichten
    Migration 0032).** (1) **Fremdschlüssel vor der Umstellung (S1):** Bisher lief
    `PRAGMA foreign_key_check` erst nach dem Commit der Migrationen. Ein Altfehler hätte die Instanz
    auf Schema 32 gesperrt: das vorige Image lehnt die neuere DB ab (Down-Migration-Schutz), ein
    Restore braucht eine Sitzung, und die Meldung „nichts verändert“ stimmte nicht. Jetzt prüft
    `db::precheck_before_migration` vor dem Pflicht-Backup (kein Backup-Ballast je Fehlversuch) und
    vor der ersten Migration Fremdschlüssel und, für Schema < 32, doppelte festgeschriebene Stornos
    (in Rust, also auch vor 0031 bei einem Stand 30; der benannte CHECK in 0032 bleibt zweite Linie).
    Ein Treffer lehnt mit 409 `db_integrity_failed` ab, die DB steht unverändert auf dem alten
    Schema, das vorige Image öffnet sie. Die Prüfung nach den Migrationen bleibt **blockierend**
    (R1-012), unterscheidet aber die Meldung: ohne Umstellung „Daten nicht angefasst“; nach einer
    gespeicherten Umstellung (nur noch durch einen Fehler der Migration selbst möglich) sagt sie
    das (`db::INTEGRITY_AFTER_MIGRATION`) und nennt den Rückweg ohne Sitzung: neue Instanz mit dem
    vorigen Image, das Pflicht-Backup davor einspielen (Handbuch „Fehlersuche im Container“).
    Abgewogen gegen „nach der Umstellung nur warnen“: das ließe mit verwaisten Verweisen arbeiten;
    der Fall ist nach der Vorabprüfung ein Programmfehler, für den der dokumentierte Rückweg reicht.
    `check_foreign_keys` ist ersetzt durch `foreign_key_violations` (nur Befunde) und
    `log_foreign_key_violations` (mit Kontext); Restore Phase A meldet als „Restore-Prüfung eines
    Backups“ statt „beim Entsperren“. (2) **Anonymisierung verworfener Entwürfe:** Nr. 124 (3)
    (Amendment). Zusätzlich verlieren die Abo-Vorlagen des Kontakts Bezeichnung (Platzhalter
    „Abo-Vorlage (Kontakt anonymisiert)“) und Notiz; ihre Positionen bleiben wie Rechnungspositionen.
    Kosten-Abos, bei denen der Kontakt Lieferant ist, werden nur pausiert (sie beschreiben die
    eingekaufte Leistung). Das Audit `contact.anonymize` steht jetzt in derselben Transaktion.
    (3) **Umwandlung zurücknehmen:** Verwirft der Inhaber den aus einem Angebot entstandenen
    Entwurf, setzt `void_draft_core` das Angebot in derselben Transaktion auf `accepted` zurück
    (`converted_at`/`converted_invoice_id` leer, Audit `quote.conversion_reverted`); sonst bliebe
    ein angenommenes Angebot auf einen verworfenen Beleg zeigend für immer blockiert (Entwürfe sind
    nicht bearbeitbar, die Korrektur ist „verwerfen, neu anlegen“). Abgelehnt: das Verwerfen solcher
    Entwürfe zu verbieten (dann gäbe es keinen Korrekturweg). Ein Angebot ist kein Buchungsbeleg;
    der Statuswechsel ist dokumentiert, der verworfene Entwurf behält `derived_from_quote_id`.
    Neu `trg_quotes_converted_final` (verschärft): umgewandelt bleibt umgewandelt, Rücknahme nur auf
    `accepted` ohne Bezug und nur, wenn der Bezugs-Entwurf verworfen ist. (4) **Angebot
    zurückziehen:** Grund Pflicht (mindestens 10 Zeichen, wie beim Verwerfen einer Rechnung — das
    Zurückziehen verwirft einen Angebots-Entwurf endgültig), Statuswechsel, Audit `quote.cancel` und
    das Erledigen einer Restore-Vormerkung in einer `BEGIN IMMEDIATE`-Transaktion, Status-Bedingung im
    `UPDATE` (`status NOT IN ('canceled','converted')`) und eine Sperre je Angebot
    (`locks::QUOTES`), die auch die Umwandlung nimmt; ein zweites Zurückziehen meldet „bereits
    zurückgezogen“ statt eines rohen Trigger-Fehlers. (5) **DSGVO-Auskunft vollständig:** Abo-
    Rechnungs-Vorlagen samt Positionen, Kosten-Abos und Anlagen mit dem Kontakt als Lieferant und
    deren Audit-Bezüge (`recurring_invoice`, `recurring`, `asset`, je Paar Typ/ID) sowie ihre
    Anhänge; ohne interne Notizen. Ein verworfener Entwurf erscheint mit Status `voided`,
    `voidedAt` und `voidReason` statt als `draft`; die PDF-Vorlage zeigt Status deutsch.
    (6) **Veräußerte Anlage:** `reset_asset` und `assets_update` lehnen `disposed = 1` ab (das
    Bearbeiten auch im `UPDATE` per `AND disposed = 0 AND locked_at IS NULL`);
    `trg_assets_disposal_final` hält zusätzlich Nummer, Bezeichnung, Anschaffung, AfA-Grundlage,
    Verknüpfungen und Restbuchwert fest; `trg_depreciation_entries_no_delete_disposed` verbietet das
    Löschen ihrer AfA-Buchungen (sonst fehlte die AfA des Abgangsjahres endgültig, der AfA-Lauf bucht
    veräußerte Anlagen nicht). Der Jahresabschluss schreibt alle bis zum Jahr angeschafften Anlagen
    fest, auch veräußerte — im Jahr oder später veräußerte Anlagen stehen im Anlageverzeichnis des
    Jahres. **Neuer Fund:** `assets_dispose` holte fehlende Vorjahres-AfA nicht nach; danach blieb sie
    für immer aus, und die anteilige AfA des Abgangsjahres rechnete vom zu hohen Restbuchwert. Jetzt
    bucht es vorher `accrue_asset_through(…, Abgangsjahr − 1)` (abgeschlossene Jahre ausgenommen).
    (7) **Verworfene Nummern in der Steuerberater-Übergabe:** `nicht-festgeschriebene-nummern.csv`
    (Nummer, Art, Belegdatum, Status verworfen/offen, Zeitpunkt in Europe/Berlin, Grund, Betrag) im
    Paket und im Manifest; die `LIESMICH.txt` nennt die Anzahl — sonst sähe der Prüfer eine
    unerklärte Lücke (GoBD Rz. 58 ff.). Offene Entwürfe stehen mit drin, weil auch sie Lücken in der
    Folge der beigelegten Belege erklären. (8) **Halbes Storno-Paar aus dem Altbestand:** Gibt es zu
    einem nicht stornierten Original schon einen festgeschriebenen Storno (Desktop-Zeit vor
    R1-003), vervollständigt `invoices_cancel` das Paar in einer Transaktion (Original `canceled`
    mit Verweis und Festschreibe-Zeitpunkt des Stornos, Audit `invoice.cancel_pair_completed`)
    statt einen zweiten Storno samt verwaister Archivdateien zu erzeugen und am Index zu scheitern.
    Die EÜR ändert sich nicht (Erstattung über den festgeschriebenen Storno, Einnahme am
    Zahlungsdatum). (9) **ASVS 8.3.8 korrigiert:** Die Anonymisierung entfernt nach Fristablauf
    **nicht** alle Personendaten: Empfänger-Snapshots festgeschriebener Belege, deren PDF/XML im
    write-once-Archiv, `email_log.to_email` und Audit-Details bleiben bis zur Stilllegung der Instanz
    (`gobd.md` §6, Handbuch „Aufbewahrung 10 Jahre“). Eine „Beleg-Pseudonymisierung nach
    Fristablauf“ (gezielte, protokollierte Ausnahme für Jahre jenseits der verlängerten Frist) ist
    eine Post-v1.0-Aufgabe; bis dahin bleibt die Entscheidung „keine automatische Löschung“ mit dieser
    ehrlichen Grenze. (10) **Snapshot-Abweichung in Einzel-Exporten:** EÜR-PDF mit Hinweiskasten
    (festgeschrieben/heute), ELSTER-Ausfüllhilfe mit Kommentarzeile `# ACHTUNG: …` unter dem Titel,
    Einzelaufstellung mit `HINWEIS-ABWEICHUNG-JAHRESABSCHLUSS.txt`; im Steuerberater-Paket gelten PDF
    und ELSTER-Datei ebenso. Der DATEV-Buchungsstapel bleibt unverändert (EXTF erlaubt keine
    Zusatzzeilen, ein Begleitfile zur Einzel-CSV gibt es nicht); Audit, Hinweis und
    `snapshotCheck` in der Antwort tragen die Abweichung, die Anzeige im Export-Dialog gestaltet der
    UI/UX-Lauf. (11) `server::auth::remove_partial_db` löscht ohne `exists()` vorab
    (`db::remove_file_if_exists`, Nr. 125 (8)). Tests: `tests/migration_0032_test.rs`
    (`foreign_key_violation_before_migration_keeps_old_schema`,
    `integrity_message_after_migration_is_honest`, `voided_final_trigger_lists_every_invoice_column`,
    `voided_draft_snapshot_changes_only_by_anonymization`,
    `voided_drafts_no_longer_block_anonymization`,
    `voiding_converted_draft_reverts_quote_and_allows_new_conversion`,
    `quote_cancel_needs_reason_and_reports_repeat_cleanly`,
    `legacy_half_storno_pair_is_completed_without_second_storno`,
    `disposed_asset_keeps_depreciation_and_master_data`,
    `dispose_catches_up_missing_prior_year_depreciation`), `tests/server_restore_test.rs`
    (`schema_30_backup_is_restored_and_migrated_forward`,
    `foreign_key_violation_before_migration_refuses_unlock_without_changes`),
    `tests/dsgvo_export_test.rs` (`export_includes_templates_subscriptions_assets_and_voided_drafts`),
    `tests/audit_export_test.rs`, `tests/fiscal_year_lock_test.rs`
    (`close_year_locks_disposed_assets_too`), `fiscal_year::snapshot::tests::pdf_block_only_on_mismatch`,
    `euer::detail::tests::unissued_numbers_csv_lists_voided_and_open_drafts`,
    `euer::elster_csv::tests::notes_are_comment_lines_after_the_title`,
    `server::auth::tests::remove_db_files_ignores_vanished_companions`.
127. **Backup, Restore, Krypto (Lauf F, Block F3a, 2026-10-25; keine Migration; Zeitlogik in
    ADR 0040 Nr. 50–53).** (1) **Backup-Format 3 — Klartext-Kopf als AAD (W14-020):** Der Kopf
    (Magic-Zeile und Manifest-JSON samt Zeilenumbrüchen, alle Bytes vor dem Ciphertext) ist als
    *Associated Data* an AES-256-GCM gebunden. Schema-Version, Zeitpunkt, KDF-Angaben, Salt,
    Nonce und Inhalts-Hash sind damit nach dem Entschlüsseln authentisch; ein veränderter Kopf
    scheitert wie eine falsche Passphrase (vor jedem Schreiben, kein Pre-Restore-Backup). Das
    Herabsetzen auf Format 2 hilft nicht: Format 2 entschlüsselt ohne AAD, das Tag passt nicht.
    Formate 1 und 2 (ohne AAD) bleiben lesbar (`MIN_SUPPORTED_FORMAT_VERSION` 1); Magic bleibt
    `KLEIN-BUCH-BACKUP-1`, damit ältere Versionen ein Format-3-Backup mit „Format 3 wird nicht
    unterstützt“ statt „ungültige Datei“ ablehnen. Inhalt wie Format 2. Vor dem Entschlüsseln
    bleibt der Kopf eine ungeprüfte Anzeige (Vorschau ohne Passphrase); die Schema-Vorprüfung
    nutzt ihn weiter, die gestagte DB wird danach ohnehin gegen das Manifest geprüft. Die KDF-
    Parameter im Kopf bleiben Information (abgeleitet wird mit den festen Konstanten; ein
    Kopf mit fremden Parametern kann keinen teuren KDF-Lauf erzwingen). (2) **Speicher
    (W7-RESTORE-RAM):** Entschlüsseln in place (`restore::decrypt_backup_owned`: Kopf aus dem
    Dateipuffer schieben, `decrypt_in_place_detached`) und Verschlüsseln in place
    (`seal_backup_at`: Kopf vor den ZIP-Puffer, Inhalt verschlüsseln, Tag anhängen); Floor und
    Ziele schreiben denselben Puffer (`target::write_backup_shared`, keine Kopie je Ziel).
    Restore und Prüf-Lesen brauchen etwa einmal die Backup-Größe (bisher dreimal), das Erstellen
    etwa ein- bis zweimal (ZIP-Aufbau); `compose.example.yaml` nennt „512 MiB + 2 × größtes
    Backup“. Abgewogen gegen Streaming-AEAD (STREAM/chunked): neues Containerformat ohne
    Bibliotheksstandard im Bestand, für Archive einer Einzelfirma nicht nötig. (3) **Download
    lokaler Backups (W13-BACKUP-DOWNLOAD):** `backup_download {backupId}` (Re-Auth — die `.kbk`
    verlässt den Server und erlaubt einen Offline-Angriff auf die Passphrase) legt die
    verschlüsselte Datei per Hardlink (sonst Kopie) in den Download-Ordner und liefert ein
    einmaliges Ticket wie die Exporte (eine Stunde, seit Lauf F4b 10 min — Nr. 131; gelöscht beim Sperren); Audit
    `backup.downloaded` vor der Freigabe. Erlaubt sind der lokale Backup-Ordner und
    Verzeichnis-Ziele unter `/backup-targets`, nicht SFTP. Das Herunterladen löscht nur den Link.
    Schaltfläche schlicht in der Backup-Liste, Gestaltung im UI/UX-Lauf. (4) **Wurzelprüfung
    (W14-011):** `rotation::resolve_local_backup` löst `target_path` per `canonicalize` auf und
    verlangt eine reguläre `.kbk` unter einer erlaubten (ebenfalls aufgelösten) Wurzel. Die
    Rotation löscht nur solche Dateien (Floor, `/backup-targets`); eine Zeile mit anderem Pfad
    (`..`, Symlink nach draußen, fremde Endung) verliert nur ihren Eintrag (Audit
    `backup.rotation.prune` mit `file: refused`). Restore aus der Historie (nur Floor) und
    Download nutzen dieselbe Prüfung. (5) **Restore-Vorprüfung vor der Sperre (W14-039):**
    `CommandSpec::precheck` läuft im Dispatcher nach dem Idempotenz-Beginn und vor
    `begin_restore`: Argumente lesbar, genau eine Quelle, Upload bzw. einspielbarer
    Historien-Eintrag vorhanden, Passphrase nicht leer — sonst 400 (bzw. 423 gesperrt), ohne
    Sperre der Änderungen, ohne Pre-Restore-Backup; eine Wiederholung mit demselben Schlüssel
    bekommt dieselbe Antwort. (6) **Snapshot-Fehler im Protokoll (W2-SNAP-LOG):** Snapshot,
    Schema-Lesen und Verschlüsselung haben in `create_with` eine gemeinsame Fehlerbehandlung —
    Zeile `failed` im `backup_log` (Ziel `local`, geplanter Dateiname und Pfad, Detail
    „Snapshot/Verschlüsselung: …“) und Hinweis über `rule_backup_result` (einmal je Tag), wie ein
    gescheiterter Floor. Das Detail nennt den Fehlertext samt Pfaden: `backup_log.full_path` ist
    laut Backup-Hardline ohnehin der volle Pfad, und das Protokoll sieht nur der Inhaber.
    (7) **Temp-Dateien im Snapshot (W2-SNAP-TMP):** versteckte `.<name>.<uuid>.tmp`
    (`archive::store`) und `.<name>.<uuid>.part` (Backup-Schreiben) und symbolische Links kommen
    nicht ins Backup; verschwindet eine Datei zwischen Auflisten und Lesen, wird sie
    übersprungen (gelesen wird vor dem ZIP-Eintrag). (8) **Mutex-Poisoning (KB-0034):**
    `BackupSession` und der Host-Key-Speicher des SFTP-Handshakes nutzen
    `unwrap_or_else(PoisonError::into_inner)` statt `expect`. Tests:
    `backup::encrypt::tests::in_place_matches_classic_and_binds_aad`,
    `backup::manifest::tests::parse_header_offset_points_to_ciphertext`,
    `backup::restore::tests::{format3_header_is_bound_to_ciphertext,
    restore_of_format_v2_backup_without_aad, restore_of_format_v1_backup_keeps_branding}`,
    `tests/server_restore_test.rs` (`schema_30_backup_is_restored_and_migrated_forward` mit einem
    Format-2-Backup, `backup_download_needs_reauth_and_serves_only_local_kbk`,
    `restore_from_history_only_local`), `tests/backup_restore_test.rs`
    (`rotation_run_deletes_files_only_under_the_roots`), `tests/backup_schedule_test.rs`
    (`snapshot_failure_is_logged_and_notified`), `backup::snapshot::tests::{snapshot_skips_hidden_temp_files,
    vanished_file_between_listing_and_reading_is_skipped}`,
    `backup::tests::backup_session_survives_poisoned_mutex`,
    `backup::sftp::tests::poisoned_seen_mutex_does_not_panic`. **Berichtigt in Nr. 129 (F3c):**
    (1) das Pflicht-Backup vor einer Migration hat das Format der Vorversion (Rückfall), (2) der
    Restore gibt den Inhalt vor dem Pre-Restore-Backup frei (Spitze nicht 1 ×, sondern das Größere
    von Restore und Pre-Restore-Backup), (3) Download aus einem anderen Dateisystem ohne Kopie,
    (7) übersprungene Links und verschwundene Archivdateien werden gemeldet.
128. **Nachzügler F2 (Lauf F, Block F3b, 2026-10-25; keine Migration; Restore-Hinweise in
    ADR 0040 Nr. 54–56).** (1) **Anlagen-Abgang (S2):** `assets_dispose` läuft jetzt in einer
    `BEGIN IMMEDIATE`-Transaktion (Vorjahres-AfA nachholen, anteilige AfA, Abgang, Audit — zusammen
    oder gar nicht; Uhr aus dem `AppCtx`). Ein Abgangsdatum vor der Anschaffung lehnt ab (die
    anteilige AfA wäre ein „Datenfehler“ ohne Buchung, der ganze Restbuchwert ginge als Verlust in
    ein Jahr vor der Anschaffung, und die Abgangsfelder sind danach fest). Ist ein Jahr nach dem
    Abgangsjahr abgeschlossen, das die Anlage führt (Anschaffung bis dahin), oder ist eine AfA ab
    dem Abgangsjahr festgeschrieben, lehnt der Abgang ab und nennt das erste mögliche Jahr — der
    Abschluss hat die Anlage im Bestand und mit ihrer AfA festgeschrieben, eine frühere
    Veräußerung ist eine Berichtigungsfrage für die Steuerberatung. Offene AfA-Buchungen ab dem
    Abgangsjahr entfernt
    er in derselben Transaktion (Audit `depreciation.reset_for_disposal` mit Jahr, Betrag, Monaten
    und wiederhergestelltem Restbuchwert) und bucht das Abgangsjahr anteilig bis zum Abgangsmonat
    neu (R 7.4 Abs. 8 EStR). Damit ersetzt eine volle Jahres-AfA im Abgangsjahr nicht mehr die
    anteilige (bisher „regulärer Lauf vorrangig“, R2-021); nach dem Abgang wäre sie wegen F2c nicht
    mehr korrigierbar. AfA und Restbuchwert ergeben zusammen die Anschaffungskosten. Abgewogen gegen
    „ablehnen mit Hinweis aufs Zurücksetzen“: Das Zurücksetzen löscht alle offenen Jahre und
    verlangt einen zweiten Schritt, ohne dass ein anderes Ergebnis möglich wäre — AfA nach dem
    Abgang ist immer falsch. `accrue_yearly::accrue_asset_through_in` und `assets::dispose_in`
    arbeiten in der Transaktion des Aufrufers. (2) **Meldung nach einer gespeicherten Umstellung
    (S3, zu Nr. 126 (1)):** Scheitert die Prüfung nach der Migration, steht bei weiteren Versuchen
    keine Migration mehr an; die Meldung sagte dann „nicht angefasst“ und schickte zum vorigen
    Image, das Schema 32 ablehnt. `db::unfinished_migration_from` erkennt das jüngste
    `backup.pre_migration` auf das aktuelle Schema ohne `backup.unlock.ok` danach; die Meldung
    bleibt „nach der Umstellung von Schema N“, bis ein Entsperren gelingt (Handbuch ergänzt).
    (3) **Angebot festschreiben (S3):** `quotes_issue` nimmt `locks::QUOTES` vor dem Lesen des
    Status (ebenso `quotes_accept` und `quotes_reject`); Kontakt frisch lesen, Status prüfen,
    Empfänger-Snapshot, Festschreiben und Audit `quote.issue` laufen in einer Schreib-Transaktion.
    `quotes::set_buyer_snapshot` und `issue` verlangen `status = 'draft'` im `UPDATE` und melden
    „nicht (mehr) im Entwurf“. Ein zurückgezogener Entwurf eines inzwischen anonymisierten Kontakts
    bekommt so die Personendaten nicht zurück; ein anonymisierter Kontakt bekommt kein
    festgeschriebenes Angebot. `trg_quotes_canceled_final` bleibt unverändert (Erweiterung hieße
    Migration 0033; die App-Pfade sind dicht, die Anonymisierung ist dort die einzige erlaubte
    Änderung). (4) **Entwurf verwerfen in der Oberfläche:** Die Detail-Ansicht eines
    Rechnungs-Entwurfs hat eine schlichte Aktion „Verwerfen“ mit Pflicht-Grund (Re-Auth über den
    Transport), ein verworfener Entwurf zeigt Zeitpunkt und Grund und keine Aktionen mehr; das
    Handbuch nennt „Daten-Passwort und App-Code“ statt „Passwort“. Gestaltung im UI/UX-Lauf.
    (5) **Anonymisierung von Kosten-Abos (S3, Amendment zu Nr. 126 (2)):** Kosten-Abos mit dem
    Kontakt als Lieferant sind Stammdaten wie Abo-Vorlagen; Art. 17 Abs. 3 lit. b DSGVO trägt ihre
    Bezeichnung nicht. Sie werden weiter pausiert und verlieren jetzt die Bezeichnung (Platzhalter
    „Kosten-Abo (Lieferant anonymisiert)“); die Buchungsvorlage ersetzt der Platzhalter nur, wenn
    sie den Namen nennt (`domain::anonymize::mentions_name`: ganzer Name oder Namensteil ab vier
    Zeichen, ohne „GmbH“, Unicode ohne Groß-/Kleinschreibung) — sonst beschreibt sie die
    eingekaufte Leistung wie die Positionen einer Vorlage. Gebuchte Kosten sind Belege und bleiben.
    Audit `contact.anonymize` mit `scrubbedRecurringSubscriptions` und
    `scrubbedSubscriptionTemplates`. Tests: `tests/migration_0032_test.rs`
    (`dispose_removes_later_depreciation_and_books_pro_rata`,
    `dispose_refuses_locked_later_depreciation_and_date_before_purchase`,
    `integrity_message_after_migration_survives_retries`,
    `quote_issue_refreshes_buyer_only_on_drafts`,
    `anonymize_scrubs_subscriptions_of_the_supplier`),
    `domain::anonymize::tests::mentions_name_finds_full_name_and_parts`,
    `src/lib/voidDraft.test.ts`.
129. **Nachbesserung F3 (Lauf F, Block F3c, 2026-10-25; keine Migration; Zeitlogik in ADR 0040
    Nr. 57–59; berichtigt Nr. 127 (1)–(3) und (7)).** (1) **Pflicht-Backup vor einer Migration
    im Format der Vorversion (S1):** Seit Nr. 127 (1) schrieb jedes Backup Format 3; v2026.10.0
    liest nur Format 1–2. Beim ersten Update auf 2026.11 hätte der dokumentierte Rückweg
    (Handbuch „Updates und Rückfall“, operations-guide §8: neues `/data`, voriges Tag, das
    Backup von vor der Umstellung hochladen) mit „Backup-Format 3 wird nicht unterstützt“
    geendet — übrig gewesen wäre nur das letzte Backup von 2026.10, ohne alles danach.
    `migrate_with_backup` schreibt das Pflicht-Backup jetzt im höchsten Format, das die Version
    mit dem Schema-Stand `from` einspielt (`manifest::readable_format_for_schema`, Tabelle
    `READABLE_FORMAT_BY_SCHEMA`: Schema ≤ 31 → Format 2; darüber das aktuelle Format).
    Format 2 hat denselben Inhalt, nur ohne AAD — genau die Leselogik bis v2026.10
    (`encrypt::decrypt` über alles nach dem Kopf). `seal_backup_at`/`CreateOptions` tragen das
    Format; geschrieben werden nur 2 bis `FORMAT_VERSION` (Format 1 hatte `inputs/branding`).
    `verify_backup_file` bleibt unverändert und liest 1–3. Sicherheit: Die AAD-Bindung von
    Format 3 bleibt für alle übrigen Backups; ein Format-2-Kopf ist damit wieder legitim, aber
    nur für ohne AAD verschlüsselte Dateien — einen Format-3-Kopf auf 2 herabzusetzen scheitert
    weiter am Tag. Das Pflicht-Backup vor der Migration kennt den Kopf ohnehin nur als Anzeige,
    bis die gestagte DB gegen ihn geprüft wird. Wer `FORMAT_VERSION` erhöht, ergänzt die Tabelle
    um (Schema der Vorversion, deren Format); ein Test hält das fest. Abgewogen: (a) Format 3 in
    einem Patch-Release von 2026.10 lesbar machen — es gibt keine Zwischen-Releases, und wer
    zurückfällt, nimmt die zuletzt laufende Version; (b) Format 2 für alle Backups bis zu einem
    späteren Release — gibt die Kopf-Bindung ohne Not für jedes Backup auf. Zusätzlich empfehlen
    Handbuch und operations-guide, unmittelbar vor einem Update mit der alten Version „Jetzt
    sichern“; spätere Backups der neuen Version liest die alte unter Umständen nicht.
    (2) **Restore: Inhalt vor dem Pre-Restore-Backup freigeben (S2/S3):** Phase A hielt den
    entschlüsselten Inhalt (≈ 1 × eingespieltes Backup), während das Pre-Restore-Backup die
    laufende Instanz packte und verschlüsselte (1–2 × deren Backup) — Spitze bis etwa 3 ×,
    dokumentiert war 1 ×. Neue Reihenfolge: entschlüsseln → Audit `backup.restore.requested` (wie
    bisher vor dem Pre-Restore-Backup, steht also in ihm) → Platz vorab prüfen → gehärtet ins
    Staging entpacken und die gestagte DB prüfen (berührt keine Live-Daten; das Staging wird bei
    einem Fehler entfernt; der Inhalt ist danach freigegeben) → Pre-Restore-Backup → zweite
    Platzprüfung (Pre-Restore-Backup liegt jetzt in `/data`, der Tausch braucht die entpackte
    Größe noch einmal) → Nummernkreise → Marker. Spitze = das Größere von beidem; die Faustregel
    „512 MiB + 2 × größtes Backup“ deckt damit Erstellen und Restore, wie Compose, operations-guide
    §9/§10, Handbuch „Speicherbedarf“ und „Notfall …“ jetzt sagen. Ein in der Prüfung der
    gestagten DB abgelehntes Backup erzeugt kein Pre-Restore-Backup mehr (der Audit des Versuchs
    bleibt). (3) **Download aus einem Verzeichnis-Ziel ohne Kopie (S3, zu Nr. 127 (3)):** Gelingt
    kein Hardlink (Ziel auf einem anderen Dateisystem), wurde die `.kbk` ohne Platzprüfung nach
    `<data>/tmp/downloads` kopiert und lag bis zu einer Stunde, bei Neuversuchen mehrfach. Jetzt
    registriert `DownloadStore::register_borrowed` die aufgelöste Quelle direkt (Eintrag
    `owned = false`): Abruf, Ablauf und „Datenbank sperren“ entfernen nur den Eintrag, nie die
    Datei; ausgeliefert wird von ihrem Platz. Der Hardlink für Floor und Ziele im selben
    Dateisystem bleibt (eine Rotation in der Zwischenzeit trifft das Ticket dann nicht). Audit
    `backup.downloaded` mit `linked`. (4) **Unvollständiges Backup sichtbar (S3, zu Nr. 127 (7)):**
    Symbolische Links im Archiv bzw. Branding werden weiter nicht verfolgt (das Archiv enthält nur
    echte Dateien; ein Link zeigt auf Daten außerhalb der Instanz, Restore schriebe sie als Dateien
    zurück), und eine Archivdatei, die zwischen Auflisten und Lesen verschwindet, wird
    übersprungen — beides aber nicht mehr still: `snapshot::Content::skipped` sammelt sie, die
    `ok`-Zeile im `backup_log` nennt sie im Detail („Unvollständig: N Einträge nicht gesichert —
    archive/2024 (symbolischer Link) …“), die Inbox meldet „Backup unvollständig“ über
    `rule_backup_result` (einmal je Tag), das Audit `backup.created` zählt sie (`skipped`). Nur
    Temp-Dateien laufender Schreibvorgänge und ein ersetztes Logo fallen still weg. Abgewogen
    gegen „Backup scheitern lassen“: ohne jedes neue Backup wären auch Datenbank und übriges
    Archiv ungesichert; ein vollständiges Backup bis auf benannte Einträge plus täglicher Hinweis
    ist die sicherere Wahl (GoBD: Lücke fällt auf, statt erst beim Restore). (5) **Test-Profil
    (Ausreißer, Fund (d)):** `[profile.dev.package."*"] opt-level = 2` in `Cargo.toml`
    (Test-Profil erbt; eigener Crate opt-level 0; Release unberührt). Argon2id (64 MiB), SHA-256,
    AES-GCM, Deflate, der C-Teil von SQLite/SQLCipher (über `OPT_LEVEL`) und Typst liefen in
    Tests unoptimiert; Einrichten, Anmelden und Backups überschritten unter Volllast die
    30-s-Grenze (503 „timeout“). Kosten: einmal längeres Übersetzen der Abhängigkeiten (CI-Cache
    `Swatinem/rust-cache` hängt an `Cargo.toml`/`Cargo.lock`, danach wiederverwendet; `clippy`
    übersetzt ohnehin ohne Code-Erzeugung). Gemessen (Kopie, 25.10.2026): `server_auth_test` allein
    136–143 s → 51 s, `server_restore_test` 289 s → 67 s, Test-Schritt des vollen Gates
    1006–1182 s (mit Ausreißern) → 305–354 s ohne Ausreißer. Ein Lauf mit drei Fehlern in
    Datei-SQLite-Unit-Tests (`PoolTimedOut`, `database is locked`; bekannt seit F2b) fiel in eine
    I/O-Spitze des Hosts (PSI `full avg60` 36 %, `backup_log_test` 49 s statt 2 s) und ist
    nicht Folge des Profils. Tests: `backup::manifest::tests::rollback_formats_cover_current_format`,
    `backup::restore::tests::format2_for_rollback_decrypts_without_aad`,
    `tests/server_restore_test.rs` (`pre_migration_backup_on_unlock` mit der Leselogik von
    v2026.10.0, `foreign_archive_paths_are_rejected` ohne Pre-Restore-Backup,
    `backup_download_from_other_filesystem_needs_no_copy`),
    `downloads::tests::borrowed_file_is_never_deleted`,
    `backup::snapshot::tests::{symlinks_are_reported_not_followed,
    vanished_file_between_listing_and_reading_is_skipped}`,
    `tests/backup_schedule_test.rs` (`incomplete_backup_is_logged_and_notified`).
130. **Anmeldung, Sitzung, Geheimnisse im Speicher (Lauf F, Block F4a, 2026-10-25; keine
    Migration; ergänzt Nr. 117 und 120).** (1) **Obergrenze in Folge (W14-004, NIST SP
    800-63B §5.2.2):** Fehlversuche ohne Gerätecookie bei Anmeldung und Re-Auth mit TOTP
    (nicht Einmal-Code-Logins, nicht die Einrichtung) zählen in Folge; nach 100 ohne
    zwischenzeitliche erfolgreiche Anmeldung lehnt der Server Anmeldung und Re-Auth mit
    TOTP ohne Gerätecookie ab (429 `totp_locked`, keine KDF), unabhängig von der Zeit.
    Aufgehoben wird die Sperre nur durch eine Anmeldung mit Gerätecookie oder Einmal-Code
    bzw. die abgeschlossene Einrichtung (`totp-reset`, Container-Zugriff). Zähler in
    `/data/auth/totp-lock.json` (0600, Temp + rename; übersteht Neustarts; eine unlesbare
    Datei beginnt bei 0 statt auszusperren); WARN `auth.totp_locked`, Inbox-Hinweise bei
    Sperre (wenn entsperrt) und Aufhebung. Abgewogen: Sperre auch für Gerätecookies (würde
    jedem Angreifer die Aussperrung des Inhabers erlauben) und ein Ablauf nach Zeit (NIST
    verlangt eine Grenze aufeinanderfolgender Versuche; die Wartezeit-Staffel bleibt
    zusätzlich). (2) **Sitzungsübersicht (W14-005, ASVS V3.3.4):** `GET /api/auth/sessions`
    (Anmeldezeit und letzte Nutzung aus der Uhr des Servers, gekürzte Kennung = 4 Byte des
    SHA-256, „diese Sitzung“; keine IP, kein Gerätename — nicht gespeichert) und
    `POST /api/auth/sessions/others/revoke` (Re-Auth, Audit `auth.sessions_revoked` mit
    Anzahl); schlichter Block in Einstellungen → Sicherheit. (3) **Bedingte Re-Auth
    (W14-038; berichtigt Nr. 120 für `seller_profile_upsert`):** neue Art `Auth::Conditional`;
    der Dispatcher gibt dem Kern per Task-Local (`commands::reauth`) mit, ob die Sitzung
    frisch bestätigt ist — Re-Auth der letzten 5 min **oder** Anmeldung bzw. Einrichtung der
    letzten 5 min (nur für bedingte Commands; für alle übrigen bleibt „eine Anmeldung ist
    keine Re-Auth“). Der Kern vergleicht unter `locks::SELLER_PROFILE` mit dem gespeicherten
    Profil: IBAN, BIC (ohne Leerraum, Groß-/Kleinschreibung egal) und §19-Status samt
    Verzicht verlangen die Bestätigung (`Error::ReauthRequired` → 403 `reauth_required`),
    Adresse, Steuernummer usw. nicht. Außerhalb des Dispatchers gilt fail-closed. 401/403
    werden nicht als Idempotenz-Ergebnis gespeichert (die Wiederholung nach dem Dialog führt
    aus). (4) **Re-Auth erweitert (W14-028, ASVS V11.1.4):** `euer_export_datev`,
    `euer_export_elster`, `euer_export_pdf` (Buchungstexte, Kundennamen, Beträge im
    Klartext) und `payment_accounts_set_active` (welche Bankverbindung auf Rechnungen steht)
    — damit 29 Commands mit Re-Auth und einer bedingt. (5) **Idempotenz und Speicher
    (W14-033, W14-015, R4-S2-ZEROIZE):** Idempotenz-Schlüssel gelten je Anmeldung
    (Folgenummer der Sitzung, übersteht die Rotation durch Re-Auth — keine Übertragung
    nötig). „Datenbank sperren“ leert Idempotenz-Speicher, Ergebnisse beendeter Jobs,
    offene Anmelde-Ergebnisse und ein ausstehendes TOTP-Geheimnis. `BackupSession` hält die
    Passphrase als `Zeroizing`, `get` liefert `Zeroizing`; `secrets::get` ebenso; die
    Auth-Routen nehmen Passphrasen ohne Kopie aus dem Körper; `SmtpConfig.password` und das
    SFTP-Passwort sind `Zeroizing`. Bewusst akzeptierte Reste und
    `cipher_memory_security` = aus: security.md „Passphrase im Speicher“. (6) **Lange
    Anmeldung (W14-037):** Anmeldung und Einrichtung warten höchstens 25 s
    (`pending::LOGIN_SYNC_WAIT`), sonst `202 {"state":"pending"}` mit Ticket-Cookie
    `__Host-kb_login` (256 bit, nur SHA-256 gespeichert, höchstens 8 offen, fertige
    Ergebnisse 15 min abholbar); der Ablauf läuft zu Ende; `POST /api/auth/login/poll`
    liefert genau einmal das Ergebnis, die Sitzung entsteht erst dort; unbekannt → 410
    `login_expired`. Frontend, Image-Smoke-Test und Testhilfen folgen der 202. Abgewogen:
    eine längere Zeitgrenze für die Route (scheitert am Proxy-Timeout, nginx-Standard 60 s)
    und Abfragen über `/api/auth/status` (kein Fehlerkanal, müsste die Sitzung ausliefern).
    (7) **Neue Passphrasen (ASVS V2.1.7, V2.1.2):** `server::passphrase_policy` —
    eingebettete, eigens zusammengestellte Sperrliste (normalisiert, rund 250 Einträge:
    bekannte Sätze, Beispiele aus Anleitungen und Comics, häufige lange Passwörter,
    Anwendungsbegriffe), triviale Muster (≤ 3 verschiedene Zeichen, Wiederholung, ≥ 80 %
    Tastatur-/Alphabet-/Zahlenfolgen), Rest nach Abzug von Anwendungs-, Firmen- und
    E-Mail-Namen ≥ 8 Zeichen, höchstens 1024 Zeichen; 422 `passphrase_common` bzw.
    `passphrase_too_long`. Gilt für Einrichtung (vorab schon beim Anfordern des QR-Codes,
    verbindlich im letzten Schritt) und Wechsel; bestehende Passphrasen bleiben gültig.
    Das Beispiel der Anmelde-Maske steht auf der Liste und wird nicht mehr angezeigt; die
    Test-Passphrase der Server-Tests wurde ersetzt. Abgewogen: Online-Abfrage geleakter
    Passwörter (k-Anonymität) widerspricht local-first; eine große fremde Liste brächte
    Lizenz- und Größenfragen bei wenig Mehrwert gegenüber Mindestlänge + Mustern.
    (8) **TOTP-Replay (ASVS V2.8.5):** `Check::Replay`, Log `reason=totp_replay`, Antwort
    unverändert. (9) **Geschlossen mit Begründung** (security.md §11): V2.1.6 (gleichwertig
    über Re-Auth mit Rotation), V2.2.3/V2.5.5 (kein Kanal außerhalb der Instanz), V2.3.1
    (Setup-Token bis zum Neustart), V2.3.2 (kein WebAuthn), V2.6.2/V2.6.3 (Einmal-Codes
    neben dem TOTP-Geheimnis), V2.8.2 (TOTP-Geheimnis 0600 vor dem DB-Open nötig).
    (10) **Frontend:** Passphrasen nur im Zustand der jeweiligen Maske, geleert nach Absenden
    bzw. Abbrechen (Restore, Passphrase-Wechsel, Einrichtung bei Zustandswechsel; Anmeldung
    und Re-Auth schon zuvor), nie in Stores, URL oder Storage (Teil von R5-S2-UI).
    `tasks:W6-ACTIVATE-ORDER` ist mit D-21 (Lauf F1) erledigt. Tests:
    `tests/server_auth_test.rs` (`seller_profile_reauth_matrix`,
    `sessions_overview_and_revoke_others`, `idempotency_key_is_bound_to_the_login`,
    `lock_clears_stored_results_and_pending_logins`,
    `totp_streak_lock_spares_recovery_codes_and_known_devices`,
    `new_passphrases_are_checked_against_blocklist_patterns_and_profile`,
    `slow_login_answers_202_and_poll_delivers_session_once`,
    `reauth_commands_require_fresh_reauth`), `tests/server_restore_test.rs`
    (`slow_unlock_with_migration_answers_202_and_poll_delivers_session`),
    `tests/server_log_test.rs` (`reason=totp_replay`), `tests/server_contract_test.rs`,
    Unit-Tests in `server::{session, idempotency, jobs, ratelimit, pending, totp,
    passphrase_policy, errors}`, `commands::reauth`, `db::repo::seller_profile`;
    `src/lib/transport.test.ts`.
131. **HTTP-Server, Eingaben, Exporte, Sidecar (Lauf F, Block F4b, 2026-10-25; keine
    Migration; ergänzt Nr. 63 und die Annahme-Schleife aus W5.8).** (1) **Verbindungen
    (W12-LEGAL-TIMEOUTS):** höchstens 256 offene Verbindungen; die Annahme-Schleife holt
    erst einen freien Platz und nimmt dann an — bei voller Grenze warten neue Verbindungen
    im Backlog des Kernels, statt angenommen und ruhend gehalten zu werden. Jede Verbindung
    trägt einen Schreib-Leerlauf-Timeout von 30 s (`http::WriteIdleTimeout`: kommt ein
    Schreibvorgang so lange nicht voran, endet die Verbindung). Ein Lese-Timeout auf
    Verbindungsebene entfällt bewusst: Header (10 s, auch Keep-Alive-Leerlauf), JSON-Körper
    (10 s) und Uploads (Leerlauf je Datenblock) sind schon begrenzt, und ein Lese-Timeout
    träfe lange laufende Anfragen. `/legal/` streamt die Datei (`ReaderStream`, mit
    `Content-Length`) statt sie je Anfrage zu lesen; ein Cache entfällt (Dateien bis 32 MB,
    selten abgerufen). Abgewogen: `tokio-io-timeout` (neue Abhängigkeit für 60 Zeilen) und
    eine Mindest-Übertragungsrate (würde langsame Mobilverbindungen treffen). (2) **Panik
    (ASVS 7.4.3):** `tower_http::catch_panic::CatchPanicLayer` (Feature `catch-panic`)
    zwischen globaler Grenze und Policy — die 500 nach einer Panik bekommt Policy-Angaben
    (HSTS), Sicherheits-Header und eine Protokollzeile; Antwort im `ApiError`-Format mit
    `ref`, ERROR-Log nur mit der Referenz. Der Router-Aufbau ist in `routes` und `layered`
    geteilt (Test mit einer Route, die panict). (3) **Inhaltstyp (ASVS V13.1.5/V13.2.5):**
    `dispatch::read_body` verlangt bei nicht leerem Körper `application/json` (Parameter
    erlaubt, Groß-/Kleinschreibung egal), sonst 415 `unsupported_media_type`; gilt für
    `/api/cmd/*` und alle Auth-Routen; leere Körper ohne Inhaltstyp bleiben erlaubt. Alle
    Aufrufer senden den Typ bereits (Frontend `transport.ts`, Image-Smoke-Test, ZAP über die
    OpenAPI aus der Registry). (4) **`/healthz` (W14-006):** „lokal“ entscheidet die
    Policy-Middleware (`ClientInfo.loopback`: Loopback-Gegenstelle **ohne**
    Weiterleitungs-Header) statt der rohen Gegenstelle — ein Reverse-Proxy auf demselben
    Host bekommt nur den Status. Der Subcommand `healthcheck` sendet keine
    Weiterleitungs-Header und sieht weiter Zustand und Off-Site-Status. (5) **Art je
    Command (W14-012):** `registry::classified_kind` stuft jeden der 196 Commands
    ausdrücklich als Job (13), Read (75) oder Write (108) ein — dieselbe Einteilung wie
    vorher, aber ohne Ableitung aus Namensendungen (`_list`, `_get`, `_check` …). Ein
    neuer Command ohne Einstufung ergibt `None`; der Test
    `server_contract_test::every_command_has_an_explicit_kind` scheitert dann, im Betrieb
    gilt er vorsichtshalber als Write (abgekoppelt + Idempotenz). (6) **Klartext-Exporte
    (W14-014):** `EXPORT_TTL` 10 min statt 1 h; die Datei verschwindet mit dem ersten Abruf
    (Entnahme beim Abruf, Löschen am Ende des Streams, auch bei Abbruch — strenger als
    „nach vollständigem Download“, dafür ohne Fenster für einen zweiten Abruf). Nicht
    registrierte Download-Ordner (Kern schreibt noch) räumt der Sweeper erst nach
    `ORPHAN_TTL` = 1 h ab, damit ein großer Migrations- oder DSGVO-Export nicht mitten im
    Schreiben verschwindet. Eine Verschlüsselung mit flüchtigem Schlüssel entfällt bei
    dieser Verweildauer. (7) **Zeichensatz (ASVS 14.4.1):** Textantworten ohne
    Zeichensatz (`text/*` aus `ServeDir`, JavaScript, SVG) bekommen in der Header-Middleware
    `charset=utf-8`. (8) **Sidecar-Health-Check (R3-014, deckt R3-015 unter Linux ab):**
    direkt nach dem Port-Bind im Hintergrund (`einvoice::sidecar_health`): Launcher, beide
    JARs, XRechnung-Konfiguration, `java -version` mit derselben Java-Auswahl wie
    `klein-buch-java.sh` (`KLEIN_BUCH_JAVA`, `jre/bin/java`, `JAVA_HOME`, `PATH`; 30 s).
    Ergebnis INFO bzw. ERROR im Log, bei Problemen ein dringender Inbox-Hinweis über die
    neue Ablage `notify::deferred` (Hinweise aus dem gesperrten Zustand, beim Entsperren
    bzw. nach der Einrichtung in die Inbox; `dedup_key` je Version und Berliner Tag). Kein
    Abbruch — alles außer E-Rechnungen funktioniert, und der Betreiber soll die Instanz
    erreichen. Mit Sidecar-Mock (nur Debug/Test) entfällt die Prüfung. Ein Feld in
    `/healthz` entfällt (Details gibt es dort nur über Loopback, das Log reicht).
    (9) **Geschlossen mit Begründung** (security.md §11): V5.1.1 (ein Parser), V11.1.3
    (Entwürfe nur mit Sitzung, Verwerfen dokumentiert), V12.2.1 (Präfix-Prüfung reicht bei
    Auslieferung als `attachment` + `sandbox`), V12.4.2 (kein ClamAV im Image), V13.2.2
    (Lese-Timeouts vorhanden; kein `deny_unknown_fields`), 14.4.2 (CSP + `nosniff` auf der
    API), 14.4.5 (HSTS ohne `includeSubDomains`, Nr. 63; Empfehlung für den Proxy im
    Handbuch). **`tasks:W7-UPLOAD-RESERVE`** bewusst geschlossen: Die Platzprüfung
    reserviert nichts für parallel laufende Uploads; im Einbenutzerbetrieb mit 20 MiB je
    Upload und 2 GiB Reserve ist ein Überbuchen folgenlos (der Upload scheitert sauber),
    als bekannte Grenze im Betriebshandbuch. Tests: `tests/server_hardening_test.rs`
    (`json_body_requires_json_content_type`, `healthz_details_only_for_direct_loopback`,
    `panic_in_handler_returns_500_with_reference`, `legal_file_is_streamed_with_length`,
    `slow_reader_is_disconnected_after_write_idle_timeout`,
    `connection_limit_defers_accept`, `sidecar_problem_reaches_the_inbox_on_unlock`),
    `tests/server_contract_test.rs` (`every_command_has_an_explicit_kind`,
    Header-Schnappschuss mit `charset`), Unit-Tests in `server::headers`, `downloads`,
    `einvoice::sidecar_health`, `notify::deferred`.

132. **Logs, Fehlermeldungen, Geheimnis-Tabelle, Mustang (Lauf F4c, 2026-10-25; keine
    Migration, keine neue Abhängigkeit; ergänzt Nr. 16 und Plan W5.9).** (1) **Serverpfade
    (W5-ERR-PATHS):** Fachliche Fehler, die an den Client gehen (`Domain`, `Backup`,
    `Mail`), nennen keinen absoluten Serverpfad mehr: Archiv-Meldungen
    (`archive::store::path_error`: Konflikt mit anderem Hash, parallel angelegt,
    Rücklese-Prüfung, Nicht-UTF-8) nur den Dateinamen, DB-Snapshot, Archivpfad-Prüfung
    beim Restore und die CA-Datei von `KLEIN_BUCH_SMTP_CA_FILE` nur die Art; der volle
    Pfad steht als WARN im Log. Bewusst sichtbar bleiben die Wurzel der Verzeichnis-Ziele
    (`/backup-targets`, Teil der Bedienung und der API) und der volle Pfad im
    `backup_log` (Backup-Hardline). `Config`-Fehler mit Pfaden sind ohnehin 500 mit `ref`.
    (2) **Log-Tests (W5-LOG-TESTS):** Unit-Test, dass `logging::filter` auch bei
    `RUST_LOG=sqlx::query=trace` bzw. `trace` kein Statement-Ereignis durchlässt; der
    Log-Test unter TRACE deckt zusätzlich Backup-Snapshot und Passphrase-Wechsel ab.
    (3) **Mailadressen (W14-016):** Der gescheiterte Auto-Versand einer Abo-Rechnung
    loggt auf WARN nur Nummer und Fehlerart (`Error::kind`, neu), den Volltext auf DEBUG;
    Hinweis und `email_log` behalten ihn (der Inhaber braucht ihn zum Handeln).
    (4) **Log-Injektion (W14-017):** `nettest::check_host` erlaubt für SMTP- und
    SFTP-Hosts nur Hostnamen (Labels aus Buchstaben inkl. Umlauten, Ziffern, `-`, `_`,
    je ≤ 63 Zeichen, gesamt ≤ 253, optional Punkt am Ende) oder IP-Adressen ohne
    Klammern — geprüft beim Anlegen/Ändern (Host getrimmt gespeichert, leer → `None`),
    beim Ad-hoc-Test und vor jeder Verbindung (`smtp::build_transport`, `sftp::open`),
    damit auch Einträge älterer Stände abgewiesen werden; die Meldung wiederholt den Wert
    nicht. Host-Felder im Log per Debug-Escaping (`?host`). Dazu global der Log-Writer
    `logging::OneLine`: `tracing_subscriber::fmt` schreibt ein Ereignis mit einem
    `write_all`, der Writer escaped darin Zeilenumbrüche (`\n`, `\r`) und übrige
    Steuerzeichen (`\u{..}`) außer Tab und behält den abschließenden Umbruch — kein Wert
    aus Eingaben oder Gegenstellen erzeugt eine zweite Zeile. Der fail2ban-Filter im
    Handbuch und im Betriebshandbuch ist auf Zeilenanfang, Ziel `klein_buch_lib::auth`
    direkt hinter dem Zeitstempel und Zeilenende verankert und passt auf `docker logs`
    und `json-file` (mit und ohne von fail2ban entfernten Zeitstempel). Abgewogen: ein
    JSON-Log-Format (bricht bestehende Filter und Lesbarkeit im Terminal) und ein
    Escaping je Aufrufstelle (lückenanfällig). (5) **Zugriffsprotokoll (W14-018):** je
    Anfrage `ip=` (Client-IP nach der Proxy-Regel; die Policy-Middleware hängt sie als
    `ClientIp` an jede Antwort, auch an 421/403), `req=` (UUIDv4 je Anfrage als
    Task-Local; `ApiError::internal` und Sidecar-Fehler nehmen sie als `ref`, abgekoppelte
    Kerne, Jobs und Anmelde-Tasks erben sie über `logging::with_request_id`) und `sess=`
    (erste 4 Byte des SHA-256 des Sitzungs-Cookies, wie `SessionId::short` in der
    Sitzungsübersicht; ohne Prüfung und ohne Verlängern der Sitzung). Kein eigener
    Header `X-Request-Id` (die `ref` im Fehlerkörper genügt). Datenschutz: Die IP ist
    personenbezogen; Rechtsgrundlage berechtigtes Interesse an der Sicherheit (Art. 6
    Abs. 1 lit. f DSGVO), Aufbewahrung über die Log-Rotation des Betreibers (Beispiel
    5 × 10 MB). Dazu `auth.reauth.ok ip=…` (ASVS 7.2.1). (6) **`tasks:W8-SECRETS-ORPHANS`
    bewusst geschlossen:** kein Aufräumen verwaister `secrets`-Einträge. Kein Pfad der
    Anwendung erzeugt sie (Löschen eines Kontos bzw. Ziels nimmt den Eintrag mit; ein
    Rest entsteht nur bei Absturz zwischen zwei Schritten oder durch DB-Eingriffe), ein
    Rest ist verschlüsselt, über keine API lesbar und an eine nie wiederverwendete
    UUIDv7 gebunden; ein automatisches Löschen „ohne Bezug“ könnte bei einem Lesefehler
    der Ziel-Liste gültige Passwörter löschen. (7) **Mustang (`tasks:W12-MUSTANG`):**
    Maven Central am 25.10.2026 geprüft — neueste Fassung 2.26.0, `core-2.26.0.pom`
    setzt Jackson 2.22.2 (< 2.22.3). Kein Update; die befristeten Ausnahmen in
    `.trivyignore` (Ablauf 30.04.2027, Nr. 114) bleiben, nächste Prüfung bis 01.03.2027.
    Tests: `server::logging::tests` (`filter_keeps_sqlx_statements_off_whatever_rust_log_says`,
    `one_line_per_event`, `request_id_reaches_detached_tasks`),
    `nettest::tests::host_validation`,
    `archive::store::tests::store_rejects_different_bytes_for_same_path` (ohne
    Serverpfad), `tests/server_log_test.rs` (Snapshot, Passphrase-Wechsel, abgelehnter
    Host, `ip=`/`req=`/`sess=`, `ref` = `req`), `tests/recurring_scheduler_test.rs`
    (`auto_send_failure_keeps_recipient_out_of_warn_log` mit SMTP-Gegenstelle auf
    127.0.0.1), `tests/server_restore_test.rs` (keine Serverpfade in abgelehnten
    Restores).
133. **Nachbesserung F4 (Lauf F4d, 2026-10-25; keine Migration, keine neue Abhängigkeit;
    ergänzt Nr. 127, 130 und 131).** (1) **TOTP-Sperre (W14-004, Befund S3):** Die
    Obergrenze von 100 Fehlversuchen in Folge zählt weiter auch falsche App-Codes ohne
    jede Kenntnis der Passphrase (der zweite Faktor wird vor der KDF geprüft). Nur
    Fehlversuche nach gültigem App-Code zu zählen, würde das Durchprobieren des zweiten
    Faktors nicht mehr begrenzen (NIST SP 800-63B §5.2.2) und ist damit eine
    Abschwächung; die Sperre bleibt also von jedem auslösbar, der die Anmeldeseite
    erreicht, und kostet den Inhaber auf neuen Geräten Einmal-Codes (bekannte Geräte
    bleiben frei) — als bekannte Grenze in security.md §11 (V2.2.1) festgehalten.
    Richtiggestellt sind Hinweis und Doku: Die Sperre zeigt massenhaftes Durchprobieren,
    das Daten-Passwort ist damit nicht verraten; empfohlen werden Prüfung der Sitzungen
    und der restlichen Einmal-Codes. Getrennt davon zählt der Server den aussagekräftigen
    Fall „falsche Passphrase nach gültigem App-Code bzw. Einmal-Code“ (Anmeldung und
    Re-Auth, mit und ohne Gerätecookie): ab drei in Folge ohne erfolgreiche Anmeldung
    dazwischen einmal je Folge WARN `auth.factor_suspect` und dringender Inbox-Hinweis
    „Gültiger App-Code mit falschem Daten-Passwort“ mit der Empfehlung, den App-Code neu
    einzurichten. Drei statt eins, damit ein Tippfehler des Inhabers keinen Alarm
    auslöst. Der Zähler liegt im Speicher (`RateLimiter::factor_suspect_*`); bei
    gesperrter Instanz holt die nächste erfolgreiche Anmeldung den Hinweis nach
    (`dedup_key` mit der Kennung der Folge, kein Doppel). (2) **Verbindungen je
    Gegenstelle (Befund S3 zu Nr. 131 (1)):** zusätzlich höchstens 32 offene
    Verbindungen je Gegenstelle (IPv4-Adresse, IPv4-mapped kanonisiert, IPv6 je /64 wie
    das Rate-Limit); weitere werden angenommen und sofort geschlossen (WARN höchstens
    einmal je Minute), sie belegen keinen globalen Platz. Vertraute Proxys
    (`TRUSTED_PROXIES`) zählen nur gegen die globale Grenze, weil hinter ihnen alle
    Clients eine Peer-Adresse teilen. Bekannte Grenze: Ein Proxy ohne Antwort-Pufferung
    reicht langsame Leser durch; dort helfen Pufferung bzw. Grenzen je Client im Proxy.
    Abgewogen gegen eine absolute Schreibfrist für Antworten ohne Sitzung: träfe
    legitime große Downloads über langsame Leitungen und hätte die globale Grenze
    weiter einer einzelnen Gegenstelle überlassen. (3) **Restore-Tausch (CI von
    `23ede89`):** `apply_pending` und `recover_pending_rollback` verschieben die Paare
    über `rename_if_exists` — eine Quelle, die zwischen Prüfung und `rename` verschwindet
    (das `-wal`, das SQLite beim Schließen der letzten Verbindung nachlaufend löscht),
    gilt als „nicht vorhanden“ statt als Fehler und zählt nicht als verschoben;
    `NotFound` bei noch vorhandener Quelle (fehlender Zielordner) bleibt ein Fehler.
    Test-Hook `InjectAt::Vanish`. (4) **Hintergrund-Backups in Tests:**
    `tests/server_support::settle_lock_backups` wartet auf die Tick-Sperre und legt ein
    noch vermerktes Lock-Backup unter ihr an (wie der geweckte Lauf, Akteur
    `scheduler`), statt mit `sleep` auf ADR 0040 Nr. 50/51 zu hoffen; Tests, die nach
    Lock-Ereignissen Backups zählen, rufen ihn auf. (5) **Debug ohne Geheimnisse
    (Hinweis F4a):** Argument-Structs mit Passphrase, Passwort, Token oder
    Autorisierungs-Code leiten `Debug` nicht ab, sondern nutzen `redact::redacted_debug!`
    (geheime Felder als `***` bzw. `Some(***)`): `BackupSetupPassphraseArgs`,
    `BackupUnlockArgs`, `BackupRestoreApplyArgs`, `BackupTestSftpArgs`,
    `BackupSetSftpTargetArgs`, `BackupTargetAddSftpArgs`, `TestConnectionArgs`,
    `MailAccountCreateArgs`, `MailAccountUpdateArgs` sowie `oauth_ms::{Callback,
    TokenResponse, TokenBundle}`. `AuthBody`, `SetupState`, `BackupSession` und
    `SessionStore` haben ohnehin kein `Debug`. Ein eigener `Secret<String>`-Typ wurde
    abgewogen: Er änderte die Schnittstelle aller Commands, ohne mehr zu schützen;
    Zeroize bleibt wie bisher an den Verarbeitungsstellen (Nr. 130). (6) **Doku:** Der
    Download-Link eines Backups gilt einmal und höchstens 10 min (seit Nr. 131 wie die
    Exporte); Betriebshandbuch, Handbuch und security.md angeglichen.
    Tests: `tests/server_auth_test.rs` (`wrong_passphrase_after_valid_totp_warns_once_per_streak`,
    Hinweistext in `totp_streak_lock_spares_recovery_codes_and_known_devices`),
    `tests/server_hardening_test.rs` (`per_peer_limit_closes_excess_connections`),
    `backup::restore::tests` (`wal_vanishing_between_check_and_rename_does_not_abort_swap`,
    `rename_if_exists_distinguishes_missing_source_from_missing_target_dir`),
    `tests/server_restore_test.rs`
    (`older_backup_restore_keeps_numbers_pauses_and_mirrors_only_after_release` mit
    `settle_lock_backups`), `redact::tests::secret_fields_never_appear_in_debug_output`,
    `commands::backup::tests::debug_output_of_secret_args_is_redacted`,
    `commands::mail::tests::debug_output_of_password_args_is_redacted`.

## Konsequenzen

- Klein.Buch ist von jedem Gerät mit Browser nutzbar, auch mobil; eine Instanz, ein Datenbestand,
  kein Sync zwischen Geräten.
- Eine Code-Basis ohne Feature-Split; Linux-Sidecar statt Windows-JRE; Release als signiertes
  Image statt unsigniertem Installer.
- Die Angriffsfläche wächst: Auth mit zwei Faktoren, Sitzungen, Header, Limits und Uploads werden
  Produktbestandteil und brauchen ein Security-Review vor dem ersten öffentlichen Image (W14).
- Der Betreiber trägt neue Verantwortung: Reverse-Proxy mit TLS, Volumes auf geeignetem
  Dateisystem, Off-Site-Ziel, Sicherung von `/inputs`, Updates in seiner Container-Verwaltung,
  Entsperren nach jedem Neustart.
- Nach einem Neustart laufen keine Jobs, bis jemand entsperrt (E3).
- Desktop-Funktionen entfallen: Factory Reset, Drop-Folder, Microsoft-Graph-Versand,
  Cloud-Ordner-Erkennung, „Im Ordner zeigen“.
- GoBD- und §19-Hardlines bleiben unberührt: Die HTTP-Handler rufen nur die Kerne, die DB-Trigger
  erzwingen Unveränderlichkeit und append-only wie bisher.

## Alternativen

| Option | Contra |
|---|---|
| Desktop-App weiterführen | Kein Zugriff von mehreren Geräten und mobil; Windows-only-Sidecar und unsignierter Installer bleiben |
| Parallelbetrieb Desktop + Server | Feature-Split (Keychain, Opener, Drop-Folder, Reset), doppelte Tests und Releases, zwei Bedrohungsmodelle |
| Vorgeschalteter Auth-Proxy statt TOTP in der App | Zusätzliche Infrastruktur beim Betreiber; die App kann ihn nicht erzwingen — falsch konfiguriert stünde die Instanz mit einem Faktor im Netz (nicht secure by default) |
| Auto-Update per `docker.sock` | Socket = praktisch root auf dem Host; bei einem Internetdienst ausgeschlossen. Auto-Updates widersprechen zudem ADR 0013 |

## Referenzen

ADR 0002, 0038 (abgelöst); ADR 0006 (GoBD), 0009 (Backup-Krypto), 0013 (Local-First),
0014 (AGPL), 0015 (Single-Tenant), 0027 (Integritäts-Cron), 0034 (Backup-Ziele), 0035
(SQLCipher, Passphrase), 0036 (Factory Reset), 0037 (Drop-Folder). Code aus W1/W2:
`src-tauri/src/clock.rs`, `src-tauri/src/limits.rs`, `src-tauri/src/locks.rs`,
`src-tauri/src/backup/restore.rs`, `src-tauri/src/archive/store.rs`, `src/lib/date.ts`.

## Amendment 2026-10-24 — Dauerbetrieb: Start- und Zeitlogik (Lauf F, ADR 0040)

**Status:** Akzeptiert · Lauf F (Block F1). Die Start- und Zeitlogik des
Server-Betriebs ist in **ADR 0040** entschieden; dort stehen Zielmodell,
Entscheidungen und Begründungen. Die folgenden Einträge oben gelten nur noch mit
diesen Änderungen (die Historie bleibt unverändert stehen):

- **E3 / Konsequenzen „holt der Scheduler nach dem Entsperren nach“:** Nachholen
  begrenzt (bis drei Termine selbst, mehr nur nach Bestätigung, nachgeholte
  Rechnungen nie automatisch versendet), erster Tick 60 s nach dem Entsperren
  ohne Vollscan, ein gesammeltes Lock-Backup je Tick; der gesperrte Zustand steht
  beim Serverstart und alle 6 h im Log (ADR 0040 Nr. 4, 18).
- **Nr. 33 (Jobs) und Nr. 44 (Restore):** Nach dem Einspielen eines Backups
  pausieren Abo-Läufe, Auto-AfA, Versand und Off-Site-Spiegelung bis zur
  Freigabe im Browser; Nummernkreise werden auf den Stand vor dem Restore
  angehoben, Floor-Backups nach dem eingespielten Stand kommen in die Historie
  (ADR 0040 Nr. 11–14).
- **Nr. 45 (Herunterfahren bis 10 min):** ersetzt durch sofortige
  Abbruch-Anforderung an den Scheduler, höchstens 30 s für Verbindungen, dann
  gleichzeitig höchstens 25 s Tick und 30 s Jobs, 5 s Pool;
  `stop_grace_period` 90 s (ADR 0040 Nr. 19).
- **Nr. 54 (Sperren und Passphrase-Wechsel warten bis 60 s):** Der Scheduler-Tick
  bekommt höchstens 10 s; „Datenbank sperren“ beendet die Sitzungen sofort und
  sperrt danach auch bei laufendem Tick (ADR 0040 Nr. 9). Die 60 s für
  laufende Commands und Jobs (`db_gate`) bleiben.
- **Nr. 65 (`tmp`-Reste nur beim Serverstart):** Reste in `data/tmp` und
  abgebrochene Backup-Teildateien räumt zusätzlich ein stündlicher Sweeper
  (älter als 6 h, ADR 0040 Nr. 19, Umsetzung 24).
- **Nr. 77 (Rotation je SFTP-Ordner):** Auf SFTP-Zielen löscht Klein.Buch
  nichts mehr; Ordner-Ziele und Floor rotieren nach Kalender-Eimern (ADR 0040
  Nr. 16, Amendment ADR 0034).
- **Nr. 13 (Tages-Backup im Tick, W2):** kalenderfest
  einmal je Berliner Tag ab einer einstellbaren Uhrzeit (Standard 02:30) statt
  24 h nach dem letzten Backup (ADR 0040 Nr. 15).
- **Nr. 91 (Kanal „System-Mitteilung“):** Der zuletzt tote Trait `Notifier`
  (`InboxOnly`) ist entfernt; `notify::emit` schreibt nur in die Inbox (Amendment
  ADR 0027).
- **Nr. 103 (Seed-Manifest):** Die neue Fassung einer vom Nutzer geänderten
  Datei wird je Fassung einmal als `<name>.neu` angeboten (`offered` im
  Manifest); eine gelöschte `.neu`-Datei kommt erst mit einer weiteren
  Image-Fassung wieder. Hinweise tragen einen `dedup_key` (Lauf F, D-10).
- **Nr. 120 (Re-Auth, 21 Commands):** Mit `scheduler_release_hold` und
  `numbering_raise` sind es 23, mit `numbering_restored_draft_confirm` (F1d,
  ADR 0040 Entscheidung 35) 24, mit `invoices_void_draft` (F2a, Nr. 124) 25, mit
  `backup_download` (F3a, Nr. 127) 26; Lauf F4a (Nr. 130): dazu DATEV-, ELSTER-,
  PDF-Export und `payment_accounts_set_active`, `seller_profile_upsert` nur noch
  bedingt — 29 mit Re-Auth und einer bedingt.
- **E12 (Zeitzone) in der Oberfläche:** DB-Zeitstempel (UTC) zeigt die
  Oberfläche über `date()`/`dateTime()` (`src/lib/format.ts`) in Europe/Berlin,
  Tage bis zur Fälligkeit über `daysUntilBerlin` (`src/lib/date.ts`) — nicht
  mehr in der Zeitzone des Geräts (Lauf F, D-15/D-25).
- **PRD:** Revision 9 „Dauerbetrieb (Lauf F)“ löst die PRD-Stellen mit
  Start-Logik der Desktop-Fassung ab.
