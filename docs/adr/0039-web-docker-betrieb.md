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
einige SQL-Filter `'localtime'` nutzen.

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
gescheiterten Update: vorheriges Image-Tag starten und das Pflicht-Backup vor der Migration
einspielen; das ältere Image öffnet eine bereits migrierte DB nicht (Schema-Check). Heute
verlangt der Restore noch exakt dieselbe Version; Umbau in W7.

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
  `TRUSTED_PROXIES` hinter einem Proxy wird das Rate-Limit je IP zum globalen (Handbuch). W6.
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
18. **Offen: Storno-Geschäftsjahr (W2).** Ein Storno im Folgejahr behält Geschäftsjahr und
    Nummernkreis der Originalrechnung (unverändertes Verhalten). Wird in W14 fachlich geprüft.
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
