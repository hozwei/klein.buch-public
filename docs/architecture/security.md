# Sicherheits-Modell

> Vertiefung zu „Backup" und „Verschlüsselung at Rest" in
> `../ARCHITECTURE.md` §7. Wie die Geheimnis-Kette gebaut ist, wo welche
> Geheimnisse leben, wie Start, Entsperren und Restore race-frei zueinander
> passen — und gegen wen sich Klein.Buch im Internetbetrieb (Phase W) schützen
> muss.

ADR-Basis: 0035 (Verschlüsselung at Rest), 0034 (Backup-Ziele/Retention/Log),
0036 (Factory Reset; im Server abgeschaltet und entfernt, E7), 0009 (Backup als
First-Class), 0011 + 0028 (Mail-Credentials; seit W8 im SecretStore der DB),
0039 (Web-/Docker-Betrieb).

**Stand des Codes.** Beschrieben ist der Server-Betrieb nach Phase W bis
einschließlich W12: Tauri und die Desktop-Shell sind entfernt, Klein.Buch läuft
als HTTP-Server im Docker-Image. Die Status-Spalte der Tabellen nennt je
Maßnahme den Block, in dem sie umgesetzt wurde. Abläufe der Desktop-Fassung
(Factory Reset, Klartext-Migration, Schlüsselbund) stehen nur noch als
Herkunft da und sind als entfallen gekennzeichnet. Betriebsaufgaben des
Betreibers: `../operations-guide.md`.

---

## Threat Model für den Internetbetrieb (Phase W, ADR 0039)

Mit Phase W wird Klein.Buch ein selbst gehosteter Dienst im Docker-Image: **ein
Nutzer pro Instanz**, bedient im Browser, aus dem Internet erreichbar; TLS
terminiert der Reverse-Proxy des Betreibers. Das bisherige Bedrohungsmodell
(ADR 0035: gestohlenes Gerät, kopierte Datei) bleibt gültig, ist aber nicht mehr
das maßgebliche. Maßgeblich ist jetzt ein Angreifer im Netz.

Status in den Tabellen: **umgesetzt (Wn)** = im Code seit Block Wn von
Phase W; **umgesetzt** = schon vor Phase W vorhanden; **dokumentiert** =
Betriebsaufgabe bzw. Grenze, beschrieben im Betriebshandbuch; **bewusst
akzeptiert** = Restrisiko.

### Schutzgüter

- **Belege und Archiv** — Integrität über 10 Jahre (GoBD, §147 AO).
- **Steuer- und Kundendaten** — Vertraulichkeit (DSGVO).
- **Die Passphrase** — ein Geheimnis für DB, Backups und Anmeldung (§1).
- **Zugangsdaten** zu SMTP- und SFTP-Servern.
- **Verfügbarkeit** von Festschreiben, Versand und Backups.

### Akteure

| # | Akteur | Fähigkeiten | Annahme |
|---|---|---|---|
| A1 | Anonymer Angreifer im Internet | erreicht alle öffentlichen Routen; Scans, Passwort-Raten, präparierte Requests | Hauptgegner |
| A2 | Angreifer mit fremder Sitzung | gestohlenes Cookie oder offenes Browser-Fenster des Nutzers; kennt weder Passphrase noch TOTP | wird durch Re-Auth begrenzt |
| A3 | Bösartige Website im Browser des Nutzers | CSRF, Clickjacking, Skript über eine ausgelieferte fremde Datei | wird durch Header und Origin-Prüfung begrenzt |
| A4 | Netzwerk-Angreifer auf dem Weg zu externen Zielen | Man-in-the-Middle zu SMTP, SFTP, Versionsabfrage | TLS bzw. Host-Key-Pinning |
| A5 | Lieferant einer Datei | präparierte PDF, XML oder ZIP, die der Nutzer hochlädt oder einspielt | Parser laufen begrenzt |
| A6 | Wer an Volumes oder Backups kommt | Dateizugriff ohne Passphrase: Backup-Datenträger, Off-Site-Anbieter, fehlkonfigurierter Host | Verschlüsselung und Erkennung |
| A7 | Lieferkette | kompromittierte Crate, npm-Paket, JAR, Basis-Image oder GitHub-Action | Pins, Scans, Signatur |

**Vertraut bzw. nicht abgedeckt:** root auf dem Host des Betreibers (kann nach
dem Entsperren die Passphrase aus dem Prozessspeicher lesen und jede Datei
ändern) und der Nutzer selbst. Mehrere Nutzer, Rollen und Mandanten gibt es
nicht (E1).

### Angriffsflächen und Gegenmaßnahmen

**HTTP-API** (`POST /api/cmd/{name}`, Auth-Routen, `/healthz`, `/legal/`, SPA)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Aufruf beliebiger Commands, auch vor dem Entsperren | explizite Allowlist (alles andere 404), Sitzung Pflicht, DB-Commands nur im Zustand `unlocked` (sonst 423); die Art jedes Commands (Lesen, Ändern, Job) steht ausdrücklich in der Registry, nicht abgeleitet aus dem Namen — ein Registry-Test lehnt nicht eingestufte Commands ab | umgesetzt (W5; Lauf F4b: W14-012) |
| Übernahme einer frischen Instanz (wer zuerst kommt, setzt die Passphrase) | Setup-Token aus dem Container-Log, Setup atomar mit Passphrase + TOTP | umgesetzt (W6) |
| Passwort-Raten | Passphrase ≥ 16 Zeichen; **neue** Passphrasen (Einrichtung, Wechsel) zusätzlich gegen eine eingebettete Sperrliste, triviale Muster (Wiederholung, Tastatur-/Zahlenfolgen) und Firmenname/E-Mail aus dem Profil geprüft (422 `passphrase_common`), höchstens 1024 Zeichen; bestehende bleiben gültig | umgesetzt (Lauf F4a, ASVS V2.1.7/V2.1.2) |
| | TOTP vor der KDF; Wartezeit je Client-IP ab 5 Fehlversuchen (1 min, verdoppelnd bis 1 h, kein Dauer-Lockout); Gerätecookie gegen fremd ausgelöste Wartezeiten | umgesetzt (W6; IPv6 je /64, globales Budget ohne Gerätecookie; W14: Gerätecookie gleitend erneuert, Einmal-Code-Login nicht im globalen Budget) |
| | Obergrenze in Folge: nach 100 Fehlversuchen ohne Gerätecookie (Anmeldung, Re-Auth) ohne zwischenzeitliche Anmeldung keine Anmeldung mit TOTP ohne Gerätecookie mehr (429 `totp_locked`), bis eine Anmeldung von einem bekannten Gerät oder mit Einmal-Code gelingt bzw. die Neueinrichtung (`totp-reset`) abgeschlossen ist; Zähler in `/data/auth/totp-lock.json` (übersteht Neustarts), WARN `auth.totp_locked` und Inbox-Hinweis | umgesetzt (Lauf F4a, W14-004, NIST SP 800-63B §5.2.2) |
| | Falsche Passphrase nach **gültigem** App-Code bzw. Einmal-Code: drei in Folge ohne erfolgreiche Anmeldung bzw. Re-Auth dazwischen → WARN `auth.factor_suspect` und dringender Inbox-Hinweis „Gültiger App-Code mit falschem Daten-Passwort“ (Empfehlung: App-Code neu einrichten); bei gesperrter Instanz nach der nächsten erfolgreichen Anmeldung, je Folge ein Hinweis; die Obergrenze in Folge ist dagegen ohne Kenntnis der Passphrase auslösbar (§11, V2.2.1) | umgesetzt (Lauf F4d) |
| Anmeldung länger als die Antwortzeit (erstes Entsperren nach einem Update: Pflicht-Backup und Umstellung der Datenbank) | nach 25 s `202 {"state":"pending"}` mit Ticket-Cookie `__Host-kb_login` (256 bit, nur SHA-256 gespeichert, HttpOnly/Secure/SameSite=Strict); der Ablauf läuft zu Ende, `POST /api/auth/login/poll` liefert genau einmal das Ergebnis (Sitzung entsteht erst dort), unbekannt → 410; „Datenbank sperren“ verwirft offene Ergebnisse | umgesetzt (Lauf F4a, W14-037) |
| Überlast durch langsame Körper | Körper kleiner Anfragen höchstens 10 s, höchstens 16 gleichzeitige POSTs je Client-Adresse | umgesetzt (W14) |
| Fremde Inhaltstypen (Formular-Posts, Parser-Verwechslung) | nicht leerer Körper auf `/api/cmd/*` und `/api/auth/*` nur als `application/json` (Parameter wie `charset` erlaubt), sonst 415; leere Körper (Abmelden, Sperren) ohne Inhaltstyp | umgesetzt (Lauf F4b, ASVS V13.1.5/V13.2.5) |
| | höchstens eine Passphrase-Ableitung gleichzeitig (Semaphore `KDF` = 1) | umgesetzt (W2) |
| Sitzungsdiebstahl | Cookie `__Host-…`, HttpOnly, Secure, SameSite=Strict; ID 256 bit, serverseitig nur als SHA-256; Idle 30 min, absolut 12 h; Re-Auth (5 min) für Restore, Ziele, Mailkonten, Exporte (Steuerberater-, Detail-, DATEV-, ELSTER-, PDF-, Migrations-Export, DSGVO-Auskunft), Backup-Download, Passphrase-Wechsel, Zahlungskonten (Anlegen, Ändern, Aktivieren/Deaktivieren), Anonymisieren, GJ-Abschluss, Freigabe angehaltener Abo-Läufe (ADR 0040), Anheben von Nummernkreisen, Freigabe vorgemerkter Entwürfe, Verwerfen von Rechnungs-Entwürfen (Lauf F); Verkäuferprofil nur bei Änderung von IBAN, BIC oder §19-Status sowie Rechnungs- und Angebotsversand an einen Empfänger, der von der hinterlegten Adresse abweicht (Lauf F5c, W14-009), dort zählt auch eine Anmeldung der letzten 5 min (bedingte Re-Auth im Kern); Re-Auth rotiert die Sitzungs-ID; TOTP-/Einmal-Code-Erneuerung und Passphrase-Wechsel beenden andere Sitzungen; Sitzungsübersicht und „andere abmelden“ (Re-Auth, Audit `auth.sessions_revoked`); Idempotenz-Schlüssel gelten nur innerhalb einer Anmeldung | umgesetzt (W6; W14 Rotation und erweiterte Liste; Lauf F4a: W14-005, W14-028, W14-033, W14-038) |
| CSRF, Clickjacking | Pflicht-Header `X-KleinBuch-Request` + Origin/Host-Prüfung; CSP mit Script-Hashes und `frame-ancestors 'none'`; kein CORS | umgesetzt (W5/W6) |
| Gefälschte Proxy-Header | `KLEIN_BUCH_PUBLIC_URL` Pflicht; `X-Forwarded-*` nur von `KLEIN_BUCH_TRUSTED_PROXIES`; Login nur über HTTPS-Proxy oder Loopback | umgesetzt (W6) |
| Überlast durch teure Arbeit | Typst, Argon2id und ZIP außerhalb der async-Runtime; Semaphoren `RENDER` 2, `JVM` 2, `KDF` 1 (`limits.rs`) | umgesetzt (W2); seit W5 laufen Kerne als eigene Tasks — ein Abbruch des Requests bricht sie nicht mehr ab |
| | Header-Read-Timeout 10 s, Request-Timeouts, `ConcurrencyLimitLayer(64)`, Job-Warteschlange ≤ 8, ≤ 30 Render-/JVM-Aufrufe je Sitzung und Minute, Body-Limits je Route | umgesetzt (W5; globale Grenze 64 ohne Warteschlange) |
| Langsame oder ruhende Gegenstellen halten Verbindungen | höchstens 256 offene Verbindungen (darüber nimmt der Server erst nach dem Ende einer Verbindung wieder an, die übrigen warten im Backlog des Kernels); Schreib-Leerlauf 30 s (eine Gegenstelle, die nicht mehr liest, verliert die Verbindung); Lesen begrenzt über Header-Timeout 10 s (auch Keep-Alive-Leerlauf), Körper 10 s, Uploads je Datenblock; `/legal/` gestreamt statt je Anfrage in den Speicher gelesen ; je Gegenstelle (IPv4-Adresse bzw. IPv6-/64) höchstens 32 davon, weitere werden angenommen und sofort geschlossen — vertraute Proxys (`TRUSTED_PROXIES`) zählen nur gegen die globale Grenze. Bekannte Grenze: Hinter einem Reverse-Proxy, der Antworten nicht puffert (z. B. Caddy in der Grundeinstellung), teilen sich alle Clients die Plätze des Proxys; ein langsamer Leser kann dort weiter Verbindungen des Proxys binden — Gegenmittel beim Betreiber: Antwort-Pufferung bzw. Grenzen je Client im Proxy | umgesetzt (Lauf F4b, W12-LEGAL-TIMEOUTS; Lauf F4d: Grenze je Gegenstelle) |
| Interna in Fehlermeldungen | Fehlerabbildung je Variante; sonst 500 mit `ref`, Volltext nur im Log | umgesetzt (W5) |
| Serverpfade in fachlichen Fehlermeldungen | Archiv-, Snapshot-, Restore- und CA-Datei-Meldungen nennen nur den Dateinamen bzw. die Art; der volle Pfad steht nur im Log. Bewusst sichtbar bleibt die Wurzel der Verzeichnis-Ziele (`/backup-targets`, Teil der Bedienung und der API) sowie der volle Pfad im `backup_log` (Backup-Hardline) | umgesetzt (Lauf F4c, W5-ERR-PATHS) |
| Panik in Handler oder Middleware bricht die Verbindung ab | `CatchPanicLayer` innerhalb von Policy, Sicherheits-Headern und Zugriffsprotokoll: 500 im API-Format mit `ref`, ERROR-Log nur mit der Referenz (ohne Panik-Text und Anfrage-Inhalt); Kerne und Jobs laufen ohnehin als eigene Tasks (Panik → 500 mit `ref`) | umgesetzt (Lauf F4b, ASVS 7.4.3) |
| Zustand der Instanz über `/healthz` | von außen nur 200/503 ohne Körper; Zustand und Off-Site-Status nur über Loopback **ohne** Weiterleitungs-Header — ein Reverse-Proxy auf demselben Host (Gegenstelle 127.0.0.1 mit `X-Forwarded-*`) bekommt nur den Status | umgesetzt (W5; Lauf F4b: W14-006) |
| Versionsangabe vor dem Login | `GET /api/public/info` (Version, Commit, Tag, Quell-URL) und `/legal/` sind wegen AGPL §13 öffentlich | bewusst akzeptiert (ADR 0039) |
| Überlast über die öffentlichen Routen | 120 Anfragen je Minute und Client-IP (IPv6 je /64) für `/api/public/info` und `/legal/*`, dann 429; `/legal/` liefert nur Dateien direkt im Ordner mit fester Endungsliste, CSP ohne Skripte | umgesetzt (W12) |

**Uploads** (Belege, Anhänge, E-Rechnungen, Rechtsdokumente, Logo, Unterschrift)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Pfad-Tricks im Dateinamen | `archive::store` lehnt Namen mit `/`, `\`, `:` ab | umgesetzt |
| Übergroße oder falsch deklarierte Dateien | Multipart gestreamt, Limits je Art (20 / 10 / 2 MiB), Typ per Magic Bytes gegen Allowlist, `mime_type` aus dieser Prüfung, freier Platz vor dem Archivieren | umgesetzt (W7): `POST /api/upload/{art}`, 413/415/507 |
| Skript in einer ausgelieferten fremden Datei | fremde Dateien nur als `attachment` mit `Content-Security-Policy: sandbox`; `inline` nur für selbst erzeugte PDFs; `nosniff` | umgesetzt (W7): `/files/archive`, `/files/download` |
| Auslieferung manipulierter oder fremder Archivdateien | Download nur über `read_and_verify` (Hash vor dem ersten Byte), `canonicalize` gegen `archive_dir`, Audit `archive.read` | umgesetzt (W7) |

**Restore-Upload** (`.kbk`)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Manipulierte Backup-Datei | GCM-Tag + SHA-256 des Content-ZIP (§3.2) | umgesetzt |
| Pfad-Traversal im Content-ZIP | Prüfung je Pfadkomponente (`..`, Wurzel, Laufwerkspräfix → Abbruch) | umgesetzt (W2) |
| Zip-Bombe, fremde Einträge, Symlinks | Allowlist `klein-buch.sqlite`, `archive/**`, `branding/**`; ≤ 200 000 Einträge; gelesene Bytes begrenzt; Abbruch über 200:1; Upload-Limit `KLEIN_BUCH_MAX_RESTORE_BYTES` (4 GiB) | umgesetzt (W7), dazu nur normale Pfadbestandteile |
| Restore durch eine fremde Sitzung | Re-Auth; Hinweis „danach gilt die Passphrase dieses Backups" | umgesetzt (W6/W7) |
| Schema-Konflikt | `30 ≤ schema ≤ EXPECTED` mit Pflicht-Backup und Vorwärtsmigration beim nächsten Entsperren; neuere Backups abgelehnt (Down-Migration-Schutz) | umgesetzt (W7); echte Vorwärtsmigration 30 → 31 seit W8 |

**Sidecar-Eingaben** (KoSIT, Mustang in der JVM)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Hängende oder ausufernde Prüfung präparierter XML/PDF | eigener Prozess, Timeout + `kill_on_drop` | umgesetzt |
| | höchstens 2 JVM-Starts gleichzeitig (`limits::JVM`) | umgesetzt (W2) |
| Mock-Modus im Produktivbetrieb | Release-Build mit gesetztem `KLEIN_BUCH_SIDECAR_MOCK` startet nicht | umgesetzt (W1) |
| Fehlender oder kaputter Sidecar fällt erst beim Festschreiben auf | Health-Check direkt nach dem Start im Hintergrund (Launcher, JARs, XRechnung-Konfiguration, `java -version` mit derselben Java-Auswahl wie der Launcher): ERROR im Container-Log und Inbox-Hinweis beim nächsten Entsperren, kein Abbruch | umgesetzt (Lauf F4b, R3-014) |
| Manipulierte Java-Komponenten, zu viel Speicher | Linux-Sidecar mit SHA-256 der Original-JARs in `sidecar/versions.json` (CI vergleicht mit den Quellen), `-Xmx512m`, Temp unter `/data/tmp` | umgesetzt (W11) |

Fehler in den Parsern der Drittkomponenten bleiben ein Restrisiko; begrenzt wird
es durch Prozessgrenze, Speicherlimit und den Nicht-root-Container. Typst, `lopdf`
und `quick-xml` laufen im Server-Prozess (Entscheidung ASVS 14.2.6 in §11).

**Logs**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Passphrase im Statement-Log (`PRAGMA key`, `ATTACH … KEY`) | Statement- und Slow-Statement-Logging auf allen verschlüsselten Verbindungen aus (`db::open_pool`, Snapshot, Verschlüsselungs-Migration) | umgesetzt (W2) |
| Geheimnisse über andere Wege | `sqlx::query=off` fest im Filter (auch gegen `RUST_LOG=sqlx::query=trace`, Unit-Test), Default `info`, Request-Log ohne Query, Header und Bodies, Empfänger und Dateinamen nur auf DEBUG; Log-Test unter TRACE auf Passphrase (auch Backup-Snapshot und Passphrase-Wechsel), TOTP, Setup-Token, Session-ID | umgesetzt (W5/W6; Lauf F4c W5-LOG-TESTS) |
| Kunden-Mailadressen im Log | Der Fehlertext eines gescheiterten Versands (Antwort des Mailservers) nennt oft den Empfänger: WARN-Zeilen tragen nur Rechnungsnummer und Fehlerart (`Error::kind`), der Volltext steht auf DEBUG, im Inbox-Hinweis und im `email_log` | umgesetzt (Lauf F4c, W14-016) |
| Log-Injektion (gefälschte Zeilen, etwa `auth.login.failed` für fail2ban) | Jedes Ereignis ist genau eine Zeile: der Log-Writer (`logging::OneLine`) escaped Zeilenumbrüche und Steuerzeichen innerhalb eines Ereignisses; SMTP- und SFTP-Hosts sind beim Anlegen, Ändern, Testen und vor jeder Verbindung auf Hostname bzw. IP-Adresse geprüft (`nettest::check_host`); Nutzerwerte in Feldern per Debug-Escaping (`?host`); der fail2ban-Filter im Handbuch ist auf Zeilenanfang, Ziel `klein_buch_lib::auth` und Zeilenende verankert | umgesetzt (Lauf F4c, W14-017) |
| Vorfälle nicht nachvollziehbar (Zugriffsprotokoll ohne Client und Sitzung) | Zugriffsprotokoll mit `ip=` (Client-IP nach der Proxy-Regel, auch bei 421/403), `req=` (Kennung der Anfrage = `ref` interner Fehler, auch aus abgekoppelten Kernen und Jobs) und `sess=` (erste 4 Byte des SHA-256 des Sitzungs-Cookies wie in der Sitzungsübersicht); `auth.reauth.ok ip=…`. Datenschutz: IP im Log aus berechtigtem Interesse an der Sicherheit (Art. 6 Abs. 1 lit. f DSGVO), Aufbewahrung über die Log-Rotation des Containers (Beispiel 5 × 10 MB) | umgesetzt (Lauf F4c, W14-018) |
| Personenbezogene Daten im Audit | Fehlversuche nur als Logzeile `WARN auth.login.failed ip=… reason=…` (fail2ban-tauglich; `reason` = `totp`, `totp_replay`, `passphrase`, `rate_limited`, `totp_locked`), nicht ins append-only `audit_log` | umgesetzt (W6; `totp_replay`/`totp_locked` Lauf F4a) |

**Volumes** (`/data`, `/inputs`, optional `/backup-targets`)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Lesen der Datenbank | SQLCipher (§2) | umgesetzt |
| Lesen der Belege | **Archivdateien (PDF/XML), Logo und Unterschrift liegen unverschlüsselt im Volume.** Schutz ist Sache des Betreibers (Host-Zugriff, Volume-Verschlüsselung). | bewusst akzeptiert |
| Löschen oder Ersetzen von Archivdateien | Erkennung statt Verhinderung, siehe „Write-once unter Linux" | umgesetzt (W2), Doku W3 |
| Temp-Dateien im RAM-tmpfs oder in fremden Verzeichnissen | DB-Snapshot unter `<data_dir>/tmp` | umgesetzt (W2) |
| | alle Temp-Dateien unter `/data/tmp`: Uploads, Downloads, Exporte, DSGVO-Auskunft (W7), Java-Sidecar über `TMPDIR=/data/tmp` im Image (W11); alle Reste beim Serverstart, Up-/Downloads minütlich, übrige Reste und Backup-Teildateien stündlich ab 6 h Alter (Lauf F, D-13) | umgesetzt (W7/W11, Lauf F) |
| Klartext-Exporte im persistenten Volume (Snapshots von `/data`) | Exporte (EÜR, DATEV, ELSTER, Steuerberater, Detail, Migration, DSGVO-Auskunft) liegen höchstens 10 min unter `/data/tmp/downloads`; der erste Abruf entnimmt sie, die Datei wird mit dem Ende der Übertragung gelöscht (auch bei Abbruch — dann neu erzeugen); „Datenbank sperren“ löscht alle; Ordner, in die ein Export noch geschrieben wird, räumt der Sweeper erst nach 1 h ab. Eine Verschlüsselung mit flüchtigem Schlüssel wäre bei dieser Verweildauer ohne nennenswerten Gewinn | umgesetzt (W7; Lauf F4b: W14-014) |
| Start auf falschem oder leerem Volume, Klartext-DB | fail-closed vor dem Port-Bind: Mountpoints, Instanz-Marker, `chmod 0400`-Selbsttest im Archiv, Stichprobe der Archivpfade, Klartext-DB → Start verweigert | umgesetzt (W5; Stichprobe beim Entsperren, die DB ist vorher verschlüsselt) |
| Rechteausweitung im Container | `USER 65532`, read-only Root-FS, `cap_drop: [ALL]`, `no-new-privileges`, Core-Dumps aus, `PR_SET_DUMPABLE` 0 | umgesetzt (W11; Image-Smoke prüft UID, read-only Root-FS und `CapEff` 0) |

**Backups**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Lesen eines Backups | Argon2id + AES-256-GCM (§3.2); die DB im Content-ZIP ist zusätzlich SQLCipher-verschlüsselt | umgesetzt (W2) |
| Inkonsistenter oder überschriebener Stand | Snapshot per `VACUUM INTO`, Backups serialisiert, Dateinamen mit Millisekunden + Kurz-UUID | umgesetzt (W2) |
| Migration ohne Rückweg | Pflicht-Backup vor jeder Migration (Entsperren mit älterem Schema, Restore eines älteren Backups) | umgesetzt (W7) |
| Speicher | das Backup wird im RAM gebaut und in place verschlüsselt (etwa 1–2× Backup-Größe), der Restore entschlüsselt in place (etwa 1× Backup-Größe) und legt das Pre-Restore-Backup erst nach dem Entpacken an (nacheinander, Lauf F3c); `mem_limit` im Compose-Beispiel | umgesetzt (Lauf F3a/F3c, `../operations-guide.md` §10) |
| Veränderter Klartext-Kopf | Format 3: Kopf als AAD an AES-GCM gebunden; Formate 1/2 weiter lesbar | umgesetzt (Lauf F3a) |
| Antwort wartet auf das Backup | Lock-Backups vermerkt und im Scheduler angelegt (ADR 0040 Nr. 50) | umgesetzt (Lauf F3a) |

**Off-Site-Ziele**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Halbe Datei unter dem endgültigen Namen (Verzeichnis-Ziel) | Temp-Datei + `fsync` + `rename` + Verzeichnis-`fsync` | umgesetzt (W2) |
| Halbe Datei auf SFTP | Upload unter `.<name>.part`, dann `rename` | umgesetzt (W7) |
| Schreiben an beliebige Pfade | Verzeichnis-Ziele nur unter `/backup-targets` + `canonicalize`; mehrere Ziele; Status `offsite` = missing / failing / stale / ok | umgesetzt (W7, `stale` Lauf F3a) |
| Löschen oder Ausliefern beliebiger Pfade aus `backup_history` | Rotation, Restore aus der Historie und Download nur für reguläre `.kbk` unter Floor bzw. `/backup-targets` (aufgelöst per `canonicalize`) | umgesetzt (Lauf F3a, W14-011) |
| MITM zum SFTP-Server | Host-Key-Pinning (SHA-256, TOFU) | umgesetzt |
| SSRF und Abgreifen gespeicherter Zugangsdaten über Test-Verbindungen | Zugangsdaten an (Host, Port, Benutzer) gebunden; Tests gespeicherter Ziele nur per ID; Ad-hoc-Tests nur mit eingegebenem Passwort; Port-Allowlist; ≤ 10 Tests je Minute (instanzweit); einheitliche Fehlermeldung; Re-Auth | umgesetzt (W8) |
| Fremder Fingerprint beim Anlegen | `backup_target_add_sftp` übernimmt nur den Fingerprint der letzten Probe für (Host, Port), höchstens 10 min alt | umgesetzt (W8) |

**Lieferkette des Images**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Kompromittierte Abhängigkeit | `Cargo.lock` + `pnpm-lock.yaml`; Build mit `--locked` bzw. `--frozen-lockfile`, `cargo auditable` | umgesetzt (W11) |
| | `cargo deny`, `cargo audit`, `pnpm audit`, gitleaks, `trivy`-Gate, SBOM; Actions per Commit-SHA, Basis-Images per Digest; Dependabot | umgesetzt (W12; Ausnahmen begründet in `deny.toml`/`.trivyignore`) |
| Untergeschobenes Image | Build nur aus dem öffentlichen Repo, cosign-Signatur (keyless), feste Tags ohne `:latest`, Tags und Images nie löschen | umgesetzt (W12, `release-image.yml`; erster Release-Lauf nach der Freigabe) |
| Selbst-Update als Einfallstor | kein Update-Knopf in der App (bräuchte `docker.sock`, also praktisch root auf dem Host); opt-in Versionsprüfung ohne weitere Daten | umgesetzt (W12: Standard aus, fester User-Agent, keine Weiterleitungen, Hinweis nur als Text) |

**Passphrase im Speicher.** Nach dem Entsperren liegt die Passphrase im
Prozessspeicher (`BackupSession`, als `Zeroizing<String>`). Die KDF-Pfade
arbeiten mit `Zeroizing`-Kopien (umgesetzt). „Datenbank sperren" (Pool zu,
Scheduler pausiert, Passphrase überschrieben, alle Sitzungen ungültig) ist
umgesetzt (W6). Die Sitzungen enden sofort; auf einen laufenden Scheduler-Tick
wartet das Sperren höchstens 10 s und sperrt dann trotzdem (Lauf F, ADR 0040
Nr. 9). Nach einem Neustart bleibt die Instanz gesperrt, bis der Nutzer im
Browser entsperrt; ein Auto-Unlock ist ausgeschlossen (E3).

Seit Lauf F4a (W14-015, R4-S2-ZEROIZE):

- `BackupSession::get` und `secrets::get` liefern `Zeroizing<String>`; die
  Aufrufer reichen sie nur als `&str` weiter. Die Auth-Routen nehmen Passphrase
  und neue Passphrase ohne Kopie aus dem Körper (`take_passphrase`) und geben
  sie ohne Kopie an Entsperren bzw. Einrichtung weiter; Passphrase-Wechsel,
  Restore, SFTP- und SMTP-Tests halten ihre Werte als `Zeroizing`.
- „Datenbank sperren“ leert zusätzlich gespeicherte Idempotenz-Antworten,
  Ergebnisse beendeter Jobs (z. B. Download-Tickets), offene Anmelde-Ergebnisse
  (W14-037) und ein ausstehendes TOTP-Geheimnis einer Neueinrichtung.
- **Unvermeidbare Reste (bewusst akzeptiert):** die Pool-Optionen von sqlx
  halten `PRAGMA key` für neue Verbindungen, solange der Pool offen ist;
  SQLCipher hält den abgeleiteten Schlüssel je Verbindung; Empfangspuffer von
  hyper/axum, der JSON-Wert im Dispatcher und Antwortkörper werden nicht
  überschrieben; lettre (SMTP) und russh (SFTP) halten Zugangsdaten als eigene
  `String`s für die Dauer der Verbindung. `PRAGMA cipher_memory_security`
  bleibt aus (SQLCipher-Standard seit 4.5): Es überschreibt und sperrt jede
  Speicherzuteilung von SQLite, kostet spürbar Leistung und schützt nur
  freigegebenen Speicher — der Schlüssel muss bei offener Datenbank ohnehin im
  Speicher liegen. Gegen das Auslesen schützen Nicht-root-Container,
  `PR_SET_DUMPABLE` 0, Core-Dumps aus und `no-new-privileges`; root auf dem
  Host ist vertraut (siehe Akteure).

### Restrisiken (bewusst akzeptiert)

- Wer den Host oder Code als Server-Benutzer kontrolliert, kann nach dem
  Entsperren alles: Passphrase lesen, Dateien löschen oder ersetzen. Klein.Buch
  erkennt Änderungen am Archiv, verhindert sie aber nicht.
- Archivdateien sind im Volume lesbar (siehe oben).
- Ohne korrekt gesetztes `KLEIN_BUCH_TRUSTED_PROXIES` sieht der Server nur die
  Adresse des Proxys; die Wartezeit je IP wirkt dann global.
- Wer Passphrase **und** zweiten Faktor hat, ist der Inhaber. Es gibt kein
  Rollenmodell.
- Betreff und Text einer Rechnungs- oder Angebotsmail sind ohne Re-Auth frei
  (W14-009): Wer eine Sitzung übernimmt, kann an die **hinterlegte** Adresse eines
  Kunden einen geänderten Text schicken (etwa eine falsche Bankverbindung) bzw.
  eine Kontaktadresse ändern und dann versenden. Das angehängte PDF trägt die
  echte Bankverbindung (Änderung nur mit Re-Auth), jeder Versand steht mit
  Empfänger und Betreff im `email_log` und im Audit; Sitzungen enden nach 30 min
  Leerlauf. Ein Re-Auth für jeden geänderten Text kostete im Alltag mehr, als er
  gegen dieses Szenario brächte (ADR 0039 Nr. 136).

---

## 1. Eine Geheimnis-Kette

Klein.Buch hat **eine** Passphrase. Sie ist gleichzeitig:

1. **Anmeldung.** Ohne sie wird kein Pool geöffnet. Im Server-Betrieb kommt ein
   zweiter Faktor (TOTP) dazu, und Entsperren der Datenbank und Anmelden einer
   Browser-Sitzung sind getrennt (umgesetzt W6, Amendment ADR 0035).
2. **SQLCipher-Key-Source.** Aus ihr leitet SQLCipher per PBKDF2-HMAC-SHA512
   (Salt im DB-Header) den Page-Encryption-Key ab.
3. **Backup-Schlüssel-Wurzel.** Die Backup-Hülle wird mit einem
   Argon2id-abgeleiteten Key (m = 64 MiB, t = 3, p = 4) aus derselben Passphrase
   gebildet.

**Konsequenz.** Wer die Passphrase hat, hat die DB *und* die Backups. Wer sie
**verliert**, hat *nichts*. Es gibt **keinen Recovery-Backdoor**. Das ist
Absicht (ADR 0035): wer Belege jahrzehntelang aufbewahren will, muss seine
Passphrase ebenso lange sicher verwahren — typischerweise in einem Passwort-
Manager. Mindestlänge: 16 Zeichen, geprüft im Frontend und im Backend.

**Warum zwei verschiedene KDFs?**

- **SQLCipher's eigene PBKDF2-HMAC-SHA512** ist im DB-Format eingebaut, mit
  Salt im DB-Header. Das macht die DB-Datei selbstbeschreibend (jede DB-Datei
  enthält ihr eigenes Salt) und cross-OS-portabel. Argon2id-Raw-Key als
  SQLCipher-Schlüssel haben wir bewusst nicht genommen — der Vorteil wäre durch
  den Verlust der Selbstbeschreibung zunichte gemacht (ADR-0035-Amendment
  2026-05-24).
- **Backup-Hülle mit Argon2id**, weil dort kein Self-Describing-Format
  gefordert ist und Argon2id für Brute-Force-Resistenz aktuell die beste
  Wahl ist. Salt + Argon2id-Parameter stehen im Manifest jedes Backups.

---

## 2. Die SQLCipher-Kette im Detail

### 2.1 Build-Setup

`klein-buch/src-tauri/Cargo.toml`:

```toml
libsqlite3-sys = { version = "=0.30.1",
    features = ["bundled-sqlcipher-vendored-openssl"] }
```

Der Exakt-Pin auf `=0.30.1` zwingt Cargos Feature-Unification dazu, im
gesamten Dependency-Graph (inkl. `sqlx-sqlite`) die SQLCipher-fähige Lib zu
nutzen statt der Vanilla-`bundled`. `vendored-openssl` baut OpenSSL mit,
damit das Produkt self-contained ist (kein System-OpenSSL nötig — wichtig
für Auslieferung).

Build-Werkzeuge: **Perl** und **make** (im Image-Build installiert die Stufe
`server` beide, `build-and-release.md` §3). Falls der Exakt-Pin durch ein
sqlx-Update bricht, muss `cargo tree -i libsqlite3-sys` genau **eine** Version
zeigen, sonst greift die Unification nicht. `db::init_crypto` initialisiert das
mitgebaute OpenSSL ohne `atexit`-Aufräumen (ADR 0039 Nr. 98).

### 2.2 Pool-Open

```rust
db::open_pool(path, key)
    → SqliteConnectOptions::from_str(url)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(Wal)
        .busy_timeout(15 s)
        .log_statements(Off)
        .log_slow_statements(Off, …)
        .pragma("key", sqlcipher_key_literal(passphrase));  // nur mit key
```

Der `PRAGMA key`-Befehl wird beim Verbindungsaufbau vor jedem anderen Statement
ausgeführt; die Passphrase steht als SQL-Literal darin (einfache
Anführungszeichen verdoppelt). Ohne Passphrase ist die Datei eine Zufallszahl.
Mit falscher Passphrase liefert die erste Abfrage einen `file is not a
database`-Fehler — sauber abfangbar.

**Kein Statement-Logging** (W2): sqlx protokolliert jedes Statement auf DEBUG
und langsame Statements (die erste PBKDF2-Ableitung dauert spürbar) auf WARN —
samt Passphrase im `PRAGMA key`. Deshalb ist beides auf allen Verbindungen mit
Schlüssel abgeschaltet: Pool, Snapshot-Verbindung, Verschlüsselungs-Migration
(`ATTACH … KEY`).

Schreib-Transaktionen, die vorher lesen (Festschreiben, Zahlung,
Nummernkreis), beginnen mit `BEGIN IMMEDIATE` (`db::begin_immediate`, W2): die
Schreibsperre wird am Anfang geholt und bis zum `busy_timeout` von 15 s
abgewartet, statt unter WAL beim Hochstufen sofort mit `SQLITE_BUSY` zu
scheitern.

### 2.3 Prüfen der Passphrase (Verifier)

Klein.Buch speichert weder die Passphrase noch einen Passwort-Hash. Es gibt zwei
Prüfwege:

1. **Verschlüsselte DB (Normalfall).** SQLCipher selbst ist der Verifier.
   Entsperren = `db::open_pool(path, Some(passphrase))` + `db::probe_readable`
   (`SELECT count(*) FROM sqlite_master`). Schlägt das fehl, war die Passphrase
   falsch; der Pool wird verworfen, am Zustand ändert sich nichts.
2. **Verschlüsselter Verifier in `app_settings`.** Beim Einrichten
   (`backup::setup_passphrase`) wird aus der Passphrase per Argon2id mit
   zufälligem Salt (16 Byte) ein Schlüssel abgeleitet und damit die feste
   Konstante `KLEIN-BUCH-PASSPHRASE-OK` per AES-256-GCM (zufällige Nonce,
   12 Byte) verschlüsselt. Gespeichert werden vier Schlüssel:

   | Schlüssel | Inhalt |
   |---|---|
   | `backup_passphrase_set` | `"true"`, sobald eingerichtet |
   | `backup_verifier_salt_hex` | Argon2id-Salt (hex) |
   | `backup_verifier_nonce_hex` | AES-GCM-Nonce (hex) |
   | `backup_verifier_ct_hex` | Ciphertext + GCM-Tag (hex) |

   `backup::verify_passphrase` leitet den Schlüssel neu ab und entschlüsselt;
   scheitert das GCM-Tag oder weicht der Klartext ab, ist die Passphrase
   falsch. Genutzt wird das bei bereits offener DB, ohne einen zweiten Pool zu
   öffnen: Anmeldung einer weiteren Sitzung, Re-Auth und Neueinrichtung des
   zweiten Faktors nach `totp-reset` (umgesetzt W6). Die früheren Nutzer
   (Bestätigung vor dem Factory Reset, Klartext-Übergangspfad) sind entfallen.

**Vorteil:** der Verifier liegt selbst in der verschlüsselten DB; wer ihn aus
einer kopierten DB-Datei holen will, braucht schon die Passphrase. Es gibt also
kein offline ratbares Geheimnis außerhalb der DB, und jeder Rateversuch gegen
den Verifier kostet eine volle Argon2id-Ableitung.

---

## 3. Die Backup-Hülle

### 3.1 Snapshot

`backup::snapshot::build_content_zip(paths, key)` baut den unverschlüsselten
Content-ZIP, der danach in die Hülle kommt.

**DB-Snapshot (W2).** `VACUUM INTO '<data_dir>/tmp/snapshot-<uuid>.db'` über
eine **eigene** Verbindung, nicht über den Pool (`VACUUM INTO` darf nicht in
einer Transaktion laufen). SQLite liest dabei einen konsistenten Stand: alle vor
dem Start committeten Transaktionen, nichts Halbes, auch bei parallelem
Schreiber (WAL). Danach wird die Datei gelesen und gelöscht.

**Verschlüsselt wie das Original.** Die Verbindung öffnet mit der Passphrase.
Liefert SQLCipher trotzdem eine unverschlüsselte Kopie (erkannt am
Magic-Header `SQLite format 3`), wird sie verworfen und per `ATTACH … KEY` +
`sqlcipher_export` verschlüsselt neu erzeugt; ist sie danach immer noch
unverschlüsselt, bricht das Backup ab. Ein Content-ZIP mit Klartext-DB entsteht
nie.

**Temp-Dateien** liegen unter `<data_dir>/tmp`, nie unter `/tmp` (im Container
ein RAM-tmpfs).

**Was nicht ins Backup kommt (Lauf F3a):** versteckte Temp-Dateien laufender
Schreibvorgänge im Archiv (`.<name>.<uuid>.tmp`, `.<name>.<uuid>.part`) und
symbolische Links; eine Datei, die zwischen Auflisten und Lesen verschwindet,
wird übersprungen.

**Inhalt (Backup-Format 2, W1; Format 3 mit gleichem Inhalt, Lauf F3a):**

```text
klein-buch.sqlite        ← SQLCipher-Snapshot
archive/<jahr>/<...>     ← write-once GoBD-Archiv (PDF/XML/Belege)
branding/<...>           ← Logo + Unterschrift aus <data_dir>/branding
```

Format 1 enthielt statt `branding/` den Ordner `inputs/branding`. Der Restore
nimmt Format 1, 2 und 3 an; bei Format 1 bleiben Logo und Unterschrift unverändert,
und der Restore-Bericht sagt das. `inputs/` (Vorlagen, AfA-Tabellen) wird nicht
gesichert; im Server-Betrieb ist `/inputs` ein eigenes Volume mit eigener
Sicherung beim Betreiber.

Das Packen (Lesen + Deflate) läuft in `spawn_blocking`.

### 3.2 Hülle

`backup::create_now(pool, paths, passphrase, trigger)`:

1. Content-ZIP (§3.1) + SHA-256.
2. Zufälliges Salt (16 Byte) und zufällige Nonce (12 Byte) pro Backup (OS-RNG).
3. `key = argon2id(passphrase, salt, m=64 MiB, t=3, p=4)` → 32 Byte. Ableitung
   und Verschlüsselung laufen außerhalb der async-Runtime unter der
   `KDF`-Semaphore.
4. Kopf = Zeile 1 `KLEIN-BUCH-BACKUP-1`, Zeile 2 Manifest-JSON.
5. `ciphertext = aes_256_gcm(key, nonce, content_zip, aad = kopf)` inkl.
   16-Byte-Tag (Format 3, Lauf F3a; Format 1/2 ohne AAD). Verschlüsselt wird
   im Puffer des Content-ZIP (Kopf davor, Tag dahinter) — keine weitere Kopie.
6. Datei = Kopf, danach der Ciphertext. Floor und Ziele schreiben denselben
   Puffer.

Das **Manifest** ist Klartext, damit der Restore-Wizard ohne Passphrase anzeigen
kann, was er einspielen würde: `formatVersion` (3), `schemaVersion`,
`appVersion`, `createdAt`, KDF-Parameter, Salt, Nonce, SHA-256 und Größe des
entschlüsselten Content-ZIP. Der Inhalt ist verschlüsselt und über das GCM-Tag
gegen Veränderung geschützt; nach dem Entschlüsseln wird zusätzlich der
Content-SHA-256 verglichen. Seit Format 3 ist der Kopf als AAD Teil der
GCM-Authentisierung: ein veränderter Kopf — auch das Herabsetzen auf Format 2 —
scheitert wie eine falsche Passphrase. Vor dem Entschlüsseln bleibt er eine
ungeprüfte Anzeige; maßgeblich für die Schema-Version ist nach dem Einspielen
der Schema-Check der DB selbst. Entschlüsselt wird ebenfalls im Dateipuffer
(Restore etwa einmal die Backup-Größe im Speicher; das Pre-Restore-Backup folgt
erst nach dem Entpacken, Lauf F3c). Das Pflicht-Backup vor einer Migration
entsteht im Format, das die Vorversion liest (für Schema ≤ 31 Format 2 ohne AAD,
damit der Rückfall auf 2026.10 gelingt; ADR 0039 Nr. 129). Ein Format-2-Kopf
ist damit legitim, aber nur für Dateien, die ohne AAD verschlüsselt wurden; einen
Format-3-Kopf auf 2 herabzusetzen scheitert weiter am Tag.

**Ablauf drumherum (W2):**

- Backups laufen **nacheinander** (prozessweiter Mutex in `create_now`):
  Auto-Backup nach dem Festschreiben, Tages-Backup im Scheduler-Tick, manuelles
  Backup, Pre-Restore-Backup — nie zwei gleichzeitig.
- Das **Auto-Backup nach dem Festschreiben** hält die Antwort nicht auf
  (Lauf F3a): Der Command vermerkt es (`backup_pending_since`, Zähler
  `backup_pending_seq`, Audit `backup.requested`) und weckt den Scheduler, der
  es nach zwei Sekunden unter der Tick-Sperre anlegt; der Vermerk übersteht
  Sperren und Neustart (ADR 0040 Nr. 50/51).
- Dateiname `klein-buch-<JJJJMMTT-hhmmssmmm>-<8 Hex>.kbk` (Europe/Berlin,
  Millisekunden + Kurz-UUID): zwei Backups in derselben Sekunde ergeben zwei
  Dateien.
- Jeder Schreibversuch auf ein Ziel landet im append-only `backup_log` (Erfolg
  und Fehlschlag, je Ziel) — nie mit Passphrase. Fehler **vor** dem Schreiben
  (Snapshot, Verschlüsselung) stehen seit Lauf F3a ebenfalls dort (`local`,
  `failed`, geplanter Dateiname) und erzeugen einen Hinweis (einmal je Tag).

### 3.3 Ziele

- **Floor** — immer lokal unter `<data_dir>/backups`, nicht abschaltbar.
  Schlägt er fehl, ist das Backup fehlgeschlagen.
- **Off-Site** (W7) — beliebig viele Ziele (`app_settings.backup_targets`,
  höchstens 10): Verzeichnisse unter `/backup-targets/…` (eingebundene Volumes,
  relativ gespeichert, beim Schreiben per `canonicalize` geprüft; das Volume
  selbst wird nie angelegt) und SFTP. Jedes Backup — auch das Pre-Restore-Backup —
  geht an jedes Ziel, je Ziel mit Gesamt-Timeout. Ein nicht erreichbares Ziel
  ist **nicht** fatal: `backup_log` mit `status='failed'` und Ziel-Bezeichnung,
  Audit `backup.mirror.failed`, Inbox-Hinweis; die übrigen Ziele und der Floor
  sind gesichert, ein festgeschriebener Beleg wird nie zurückgerollt. Status
  `offsite` = missing / failing / stale / ok (Übersicht, Einstellungen,
  Loopback-`/healthz`; `stale` = ein Ziel ohne Kopie oder mit einer älter als
  die Frist der Regel „Backup überfällig“, Standard 7 Tage); ohne Ziel
  wöchentlich ein Inbox-Hinweis.
- **Download** (Lauf F3a): Backups aus Floor und Verzeichnis-Zielen lassen sich
  mit Re-Auth als verschlüsselte `.kbk` über ein einmaliges Download-Ticket
  herunterladen (Hardlink in den Download-Ordner; ein Ziel auf einem anderen
  Dateisystem direkt von seinem Platz, nie gelöscht, keine Kopie unter `/data/tmp`
  — Lauf F3c; Audit `backup.downloaded`). Das Ticket gilt einmal und höchstens
  10 min (wie die Exporte, Lauf F4b); ein abgebrochener Download wird neu angefordert.
- **Schreiben ins Verzeichnis** (Floor und Off-Site, W2): Temp-Datei
  `.<name>.<uuid>.part` im Zielordner, `fsync`, `rename`, Verzeichnis-`fsync`.
  Ein Abbruch hinterlässt nie eine halbe `.kbk` unter dem endgültigen Namen.
- **Aufbewahrung:** nach Kalender-Eimern (§3.5): Floor 7 Tage / 3 Monate /
  1 Jahr, solange ein Off-Site-Ziel zuletzt erfolgreich war, sonst wie
  Off-Site 30 / 12 / 7; Ordner-Ziele je Ziel rotiert, **SFTP nie** (ADR 0040).
- **Tages-Backup** (W2, kalenderfest seit Lauf F): genau eines je Berliner
  Kalendertag ab der eingestellten Uhrzeit (`app_settings.backup_daily_time`,
  Standard 02:30), beim ersten Tick danach; gemerkt in
  `backup_daily_last_on`. Nach einem Fehlschlag frühestens nach einer Stunde
  erneut (persistiert, `backup_daily_last_failure_at`).
- **Nach einem Restore** (D-23): bis zur Freigabe nur Floor, keine Spiegelung
  (Audit `backup.mirror.skipped`) — eine Probe-Instanz schreibt keinen älteren
  Datenstand mit neuerem Zeitstempel neben die echten Backups. Während jeder
  Pause der automatischen Abläufe ruht die Rotation.

**Kein** proprietäres Cloud-API; Cloud-Speicher nur über einen Ordner, den der
Betreiber als Volume unter `/backup-targets` einbindet. Die Auto-Erkennung von
OneDrive/iCloud ist entfallen (W7).

### 3.4 SFTP

`backup::sftp` benutzt `russh` 0.65/`russh-sftp`. Nur **Passwort-Auth** (kein
SSH-Key-Auth — vereinfacht das Onboarding für Nicht-Techniker); **kein
SSH-Agent** (kein Agent-Client; einen vom Server geöffneten Agent-Kanal
`auth-agent@openssh.com` lehnt der Client-Handler ausdrücklich ab; CVE-2026-116794 in
russh 0.64 betrifft nur den Agent-Pfad, behoben mit 0.65, ADR 0042 V3c). Die
RSA-Crate dient nur zum Prüfen von RSA-Host-Key-Signaturen (öffentlicher
Schlüssel); der Marvin-Angriff (RUSTSEC-2023-0071) braucht Privatschlüssel-
Operationen und ist nicht erreichbar. **Host-Key-
Pinning** über SHA-256 (Trust-On-First-Use): der Verbindungstest (`probe`)
liefert den Fingerprint (`SHA256:…`), der Nutzer bestätigt ihn, und er wird im
Ziel gespeichert (`app_settings.backup_targets`, Feld `host_fingerprint`). Ein
Upload ohne passenden Pin wird abgelehnt (Man-in-the-Middle-Schutz); ein
abweichender Host-Key bricht schon im Handshake ab, vor der Passwort-Anmeldung
(Test `sftp_upload_rejects_foreign_host_key_before_auth`, ADR 0042 V3c).
Verbindungsaufbau mit 20 s Timeout.

Das Passwort liegt je Ziel-ID im **SecretStore** der verschlüsselten DB (§4,
seit W8), nie im Ziel-JSON, in Logs oder im Audit.

Der Upload schreibt unter `.<name>.part` und benennt danach um (W7); ein
abgebrochener Upload hinterlässt keine `.kbk` unter dem endgültigen Namen.

**Server-Betrieb (W8):** Ein SFTP-Ziel lässt sich nicht ändern, nur entfernen
und neu anlegen — Zugangsdaten bleiben so an (Host, Port, Benutzer) gebunden.
`backup_test_sftp` verlangt das Passwort und merkt sich den gesehenen
Fingerprint je (Host, Port) für 10 Minuten (instanzweit, ADR 0039 Nr. 86);
`backup_target_add_sftp` übernimmt nur genau diesen. Gespeicherte Ziele testet
`backup_test_sftp_saved(id)` (prüft auch den Pin). Port-Allowlist (22, 2222 bzw.
`KLEIN_BUCH_ALLOWED_TEST_PORTS`), höchstens 10 Tests je Minute, alles mit
Re-Auth.

### 3.5 Rotation

`backup::rotation::run(pool, backups_dir, targets_root)` behält je Ablage
(Floor bzw. Ordner-Ziel, zugeordnet über den `target_path`) das jüngste
automatische Backup je Berliner Kalendertag, Monat und Jahr für die jüngsten
`keep_*` Eimer mit Backup (Lauf F, D-05) — unabhängig von der Zahl der
Lock-Backups eines Tages und vom Datum des Erstellens. `manual`- und
`pre_restore`-Backups (auch nach einem Restore übernommene) werden **nie**
automatisch gelöscht; SFTP-Einträge plant die Rotation nie ein (D-06: keine
Löschung durch Klein.Buch, Historie und Audit bleiben wahr). Während einer
Pause der automatischen Abläufe ruft `create_with` die Rotation nicht auf. **Invariante** (G1-HARDEN.5): das **global neueste** Backup wird
**nie** gelöscht, auch wenn keep-Werte auf 0 stehen würden. Eine garantierte
Mindestzahl bleibt immer erhalten.

**Nur unter den Wurzeln** (Lauf F3a, W14-011): Gelöscht wird nur, was
`rotation::resolve_local_backup` aufgelöst (`canonicalize`) als reguläre
`.kbk` unter dem Floor oder `/backup-targets` findet. Eine Historienzeile, die
per `..`, Symlink oder fremder Endung woandershin zeigt, verliert nur ihren
Eintrag (Audit `backup.rotation.prune` mit `file: refused`); die Datei bleibt.

Test: die Unit-Tests in `src/backup/rotation.rs` (Abschnitt G1-HARDEN.5)
prüfen diese Invariante explizit; `tests/backup_restore_test.rs`
(`rotation_run_deletes_files_only_under_the_roots`) prüft `run` mit DB und
Dateien (KB-0035).

---

## 4. SecretStore (Zugangsdaten in SQLCipher, seit W8)

Zugangsdaten liegen in der Tabelle `secrets` der verschlüsselten DB (Migration
`0031`, STRICT; E6, Amendment ADR 0011/0028). Den OS-Schlüsselbund der
Desktop-Fassung (`keyring`, `libdbus`) gibt es nicht mehr.

| Schlüssel | Inhalt |
|---|---|
| `smtp:{mail_account_id}` | SMTP-Passwort |
| `oauth:{mail_account_id}` | OAuth-Refresh-Token (nur Desktop-Bestand; im Server ohne Funktion, E9; entfernt beim Umstellen auf SMTP oder Löschen des Kontos) |
| `sftp:{target_id}` | SFTP-Passwort je Ziel |

**Disziplin.**

- Lesbar nur bei offener DB (Passphrase); Teil jedes verschlüsselten Backups,
  nicht des Migrations-Exports (`EXCLUDED_TABLES`) und nie der DSGVO-Auskunft.
- Werte stehen nie in Log, Audit, Antworten oder im Konto-/Ziel-JSON;
  `mail_accounts.keychain_service_id` ist nur noch eine Markierung.
- Bindung an (Host, Port, Benutzer): `mail_account_update` löscht das
  Passwort, wenn sich eins davon ändert; mit Benutzer ist ein neues Pflicht
  (sonst 422 ohne Änderung). SFTP-Ziele sind unveränderlich (§3.4).
- Löschen eines Kontos bzw. Ziels löscht seine Einträge.
- **Verwaiste Einträge** (Schlüssel ohne Konto bzw. Ziel) räumt die Anwendung
  bewusst nicht auf (Lauf F4c, `tasks:W8-SECRETS-ORPHANS`): Kein Pfad der
  Anwendung erzeugt sie (Löschen nimmt den Eintrag mit; ein Rest entsteht nur
  bei einem Absturz genau zwischen zwei Schritten oder durch direkte
  DB-Eingriffe). Ein Rest ist verschlüsselt, über keine API lesbar und an eine
  nie wiederverwendete UUIDv7 gebunden. Ein automatisches Löschen „ohne Bezug“
  wäre riskanter: Ein Lesefehler der Ziel-Liste würde dann gültige Passwörter
  löschen (ADR 0039 Nr. 132).
- App-/Daten-Passphrase liegt **nicht** hier — sie kommt vom Benutzer.
- Kein Übernehmen alter Schlüsselbund-Einträge aus der Desktop-Fassung (E17).
- Gelesene Werte kommen als `Zeroizing<String>` (`secrets::get`, Lauf F4a) und
  werden beim Drop überschrieben; die Kopien in lettre bzw. russh für die Dauer
  einer Verbindung sind ein bewusst akzeptierter Rest (siehe „Passphrase im
  Speicher“).

Test: `tests/server_secrets_test.rs` (Klartext weder in DB-Datei, Audit,
Migrations- noch DSGVO-ZIP; Bindung; Port-Allowlist; Kontingent; SFTP gegen
einen Testserver auf `127.0.0.1` mit Fingerprint).

---

## 5. Bootstrap-Reihenfolge

Aus `../ARCHITECTURE.md` §3 — hier mit Sicherheits-Blickwinkel. Zustandsautomat
`setup` (leeres `/data`) → `locked` (kein Pool, kein Scheduler) ↔ `unlocked`;
Browser-Sitzungen sind davon getrennt (umgesetzt W5/W6).

1. **Vor dem Port-Bind** (`server::prepare`, synchron, fail-closed): Sperre
   `/data/.klein-buch.lock`; Volumes vorhanden, beschreibbar, eingebunden;
   `db::prepare_filesystem` — legt `data_dir` an, ergänzt `/inputs` aus dem
   Image (Seed-Manifest, nie überschreiben), verweigert einen
   Factory-Reset-Marker `FACTORY_RESET_PENDING.json` (E7, nichts wird gelöscht),
   räumt Reste eines abgebrochenen Restore-Stagings auf, heilt einen
   unterbrochenen Tausch und wendet einen vorgemerkten Restore an (§6).
   `archive/` und `backups/` entstehen erst **danach**: ein leeres neues
   `archive/` neben `archive.rollback` sah für die Wiederherstellung früher wie
   „Tausch fertig" aus. Dann: DB und Instanz-Marker passen zusammen, keine
   Klartext-DB, Selbsttest `0o400` im Archiv, `<data>/tmp` leeren. **Kein**
   Pool-Open.
2. **Einrichtung** (Zustand `setup`): Setup-Token (nur stderr) und TOTP-Code
   werden geprüft, **bevor** die DB entsteht; dann DB verschlüsselt anlegen,
   Verifier (§2.3) schreiben, Instanz-Marker und `totp.json`, erstes Backup.
3. **Anmelden = Entsperren** (Zustand `locked`): TOTP vor der KDF, Pool mit der
   Passphrase öffnen + Probe-Read, Archivpfad-Stichprobe, bei älterem Schema
   Fremdschlüssel- und Storno-Vorabprüfung vor der Umstellung (Lauf F2c),
   Pflicht-Backup vor der Migration, Migrationen + Schema-Check +
   Fremdschlüssel-Prüfung (R1-012; Treffer → 409 `db_integrity_failed`), dann
   `activate_unlocked`: Passphrase in die `BackupSession` (Prozessspeicher),
   zuerst das Pflicht-Audit (scheitert es, bleibt die Instanz gesperrt, D-21),
   dann Passphrase in die `BackupSession` (Prozessspeicher), Pool in den
   Kontext, Scheduler-Start; der erste Tick folgt frühestens 60 s später
   (D-19) und legt das fällige Tages-Backup an. Ein ausstehender Restore wird
   davor in `run_migrations` abgeschlossen (Audit, Pause `restore`,
   Nummernkreise, Historie).

**Solange ein Restore offen ist** — Marker, Zustandsdatei
`RESTORE_SWAP_STATE.json` oder `*.rollback`-Reste vorhanden — lehnen
Einrichtung und Entsperren ab. Sonst entstünde neben der beiseitegelegten DB
eine neue.

**Was diese Reihenfolge schützt:** ohne Schritt 2 oder 3 gibt es keinen Pool.
DB-Commands antworten dann 423 („gesperrt"), die Oberfläche zeigt die
Anmeldung. Nach jedem Neustart ist die Instanz gesperrt, bis der Nutzer
entsperrt (E3); einen Auto-Unlock gibt es nicht.

---

## 6. Race-Freiheit durch Zwei-Phasen-Pattern

Der **Restore** ersetzt DB-Datei, Archiv und Branding. Solange der Prozess
läuft, ist der Pool offen — die Datei lässt sich nicht einfach ersetzen
(offene Verbindungen, WAL-Zustand). Lösung: zwei Phasen, verbunden über eine
Marker-Datei im Dateisystem (nicht in der DB, die ja ausgetauscht wird). Der
Factory Reset der Desktop-Fassung nutzte dasselbe Muster; im Server ist er
abgeschaltet und entfernt (E7).

### Phase A (Live)

- **Restore** (`backup::restore::apply`, W7; Quelle Upload oder lokales Backup
  aus der Historie). Format prüfen (1–3), Schema `30 ≤ s ≤ EXPECTED`, Backup
  entschlüsseln + Content-Hash prüfen (unter der `KDF`-Semaphore; falsche
  Passphrase schreibt nichts), Pflicht-Audit `backup.restore.requested`, Platz
  prüfen, gehärtet ins Staging `restore-staging/` entpacken (Allowlist, nur
  normale Pfadbestandteile, Größen- und Verhältnisgrenzen, keine Symlinks),
  gestagte DB mit der Backup-Passphrase öffnen (Schema, Archivpfade,
  Migrations-Prüfsummen, Fremdschlüssel, höchstens ein festgeschriebener Storno
  je Original — was das Entsperren nach dem Tausch verweigern würde, lehnt
  schon Phase A ab, Lauf F2a), dann — der entschlüsselte Inhalt ist freigegeben
  (Lauf F3c) — **Pre-Restore-Backup** mit der *aktuellen* Passphrase (enthält den
  Audit), Platz erneut prüfen, Marker per `rename`. Der Tausch beim Neustart legt
  `RESTORE_APPLIED.json` an; das erste Entsperren schreibt
  `backup.restore.applied` und trägt das Pre-Restore-Backup in die Historie ein.

Der Restore-Job wartet exklusiv auf laufende Commands und Jobs (`db_gate`),
pausiert den Scheduler und lehnt ab der Annahme Änderungen mit 503 ab (sie gingen
beim Tausch verloren); ein zweiter Restore wird abgelehnt. In Phase A wird der
Pool nicht geschlossen. Nach Erfolg beendet sich der Prozess geordnet (Exit 0,
nach dem Abwarten laufender Aufgaben), und die Container-Verwaltung startet ihn
neu (umgesetzt W5/W7; die Restart-Policy muss auch nach Exit 0 neu starten,
`../operations-guide.md` §3). Scheitert Phase A, werden Sperre und Pause
aufgehoben, und Upload bzw. Staging werden verworfen (bei falscher Passphrase
bleibt der Upload für den nächsten Versuch liegen).

### Phase B (nächster Start, in `prepare_filesystem`, vor dem Pool-Open)

**Restore** (`restore::apply_pending`) tauscht DB, `archive/` und ab Format 2
`branding/` in drei Stufen. Den Fortschritt hält die Zustandsdatei
`RESTORE_SWAP_STATE.json` fest (W2), jeweils atomar geschrieben (Temp-Datei,
`fsync`, `rename`, Verzeichnis-`fsync`):

1. Zustand `moving`. Das `-shm` der alten DB (nur Index) entfernen; die
   Originale per `rename` beiseitelegen (`klein-buch.sqlite.rollback`,
   `archive.rollback`, `branding.rollback`) und das WAL der alten DB mit ihnen
   (`klein-buch.sqlite.rollback-wal`, Lauf F3a): nach einem harten Abbruch kann
   es festgeschriebene, noch nicht übernommene Transaktionen enthalten, die ein
   Rückfall sonst verlöre. Scheitert ein Schritt, werden die schon verschobenen
   sofort zurückgeholt.
2. Zustand `moved`. Kopie aus dem Staging, jede Datei mit `fsync`; danach das
   Archiv wieder schreibgeschützt (`0o400`, dieselbe Funktion wie beim
   Archivieren, W2). Scheitert die Kopie (auch der Schreibschutz), werden die
   Teilkopien entfernt und die Originale zurückgeholt; gelingt das nicht
   vollständig, bleibt der Zustand `moved` für den nächsten Start.
3. Zustand `copied`. `.rollback`-Fassungen, Marker, Staging und Zustandsdatei
   löschen.

**Wiederherstellung nach einem Abbruch** (`restore::recover_pending_rollback`,
am Anfang von Phase B, also vor jedem Pool-Open und bevor `archive/` angelegt
wird):

| Zustand | Bedeutung | Aktion |
|---|---|---|
| `copied` | Kopie war vollständig | nur die `.rollback`-Fassungen löschen |
| `moving` / `moved` | Tausch unvollständig | Teilkopien entfernen, Originale aus `.rollback` zurück; der Marker bleibt, der Tausch beginnt neu |
| keine Zustandsdatei, aber `.rollback` vorhanden (älterer Stand) | unklar | **nichts löschen**: fehlt das Original, kommt die `.rollback`-Fassung zurück; sind beide da, wird die `.rollback`-Fassung als `.verworfen-<uuid>` beiseitegelegt |

Die Angaben zum angewendeten Restore stehen in `RESTORE_APPLIED.json` (übersteht
weitere Neustarts). Das erste Entsperren schreibt daraus den Audit
`backup.restore.applied` in die wiederhergestellte DB und löscht die Datei erst
danach; scheitert der Audit, schlägt das Entsperren fehl, und der Eintrag bleibt
für den nächsten Versuch vorgemerkt.

**Factory Reset:** Ein vorgefundener Marker `FACTORY_RESET_PENDING.json` wird nie
angewendet; der Start bricht ab, nichts wird gelöscht (E7, Amendment ADR 0036;
Code seit W11 entfernt).

**Tests:** Unit-Tests in `src/backup/restore.rs` (Tausch, Crash in Stufe 2,
Wiederherstellung ohne Zustandsdatei löscht nie); `tests/server_restore_test.rs`
(Disaster Recovery mit anderer Passphrase, abgelehnte Backups hinterlassen
nichts, Restore aus der Historie nur lokal, Pflicht-Backup vor Migration,
Backup eine Schema-Version älter mit Vorwärtsmigration, fremde Archivpfade,
Backups mit verletzten Fremdschlüsseln, Audit über Neustarts); `tests/server_jobs_test.rs`
(`restart_after_restore_blocks_writes_and_shuts_down`); nach Backup → Restore
hat keine Datei unter `archive/` ein Schreibbit.

---

## 7. Neuverschlüsseln (Passphrase-Wechsel) und entfallene Klartext-Migration

**Klartext-DB.** Eine unverschlüsselte Datenbank (erkannt an
`db::db_file_is_plaintext`: die Datei beginnt mit `SQLite format 3\0`; SQLCipher
verschlüsselt auch den Header) lässt den Server nicht starten (umgesetzt W5);
ein Restore lehnt ein Backup mit Klartext-DB ab (W7). Den Übergangspfad der
Desktop-Fassung (`migrate_plaintext_to_encrypted`, Adoption einer Klartext-DB in
der Einrichtung) gibt es nicht mehr (umgesetzt W6, E17: kein Datenumzug aus der
Desktop-Fassung).

**Passphrase-Wechsel** (`POST /api/auth/passphrase`, Re-Auth, W6;
`db::rekey_db_file`, gemeinsamer Kern `reencrypt_db_file`):

1. Gleiche Passphrase → 422. Warten auf laufende Commands und Jobs
   (`db_gate`, höchstens 60 s, sonst 503); läuft ein Restore, wird abgelehnt.
2. Pool schließen; eigene Einzelverbindung auf die DB (Statement-Logging aus),
   `ATTACH DATABASE '<temp>' AS encrypted KEY …`,
   `SELECT sqlcipher_export('encrypted')`, `DETACH`. Kein `PRAGMA rekey` (WAL,
   offene Verbindungen).
3. **Probe-Lesen** und `quick_check` der neuen Datei mit der neuen Passphrase,
   `fsync`; erst danach WAL/SHM entfernen und **atomarer `rename`** auf den
   DB-Pfad. Scheitert etwas vor dem Tausch, gilt die alte Passphrase weiter und
   der Pool wird mit ihr wieder geöffnet; im Fehlerfall entscheidet ein
   Probe-Öffnen, welche Passphrase gilt.
4. Verifier (§2.3) neu schreiben, andere Sitzungen abmelden; danach ein Backup
   (`manual`) als nachverfolgter Task. Ältere Backups bleiben mit der alten
   Passphrase einspielbar.

Neben der DB bleibt keine Kopie mit der alten Passphrase zurück. Tests:
`tests/server_auth_test.rs` (Wechsel mit Datenerhalt und Verifier).

---

## 8. Integritätsprüfung: manipuliert, fehlt, nicht lesbar

`scheduler::integrity_check_cron` prüft höchstens einmal je Kalendermonat
(Monat in Europe/Berlin; nur abgeschlossene Läufe zählen, ein abgebrochener
Scan markiert den Monat nicht als erledigt). Manuell: `archive_integrity_run`.
Je Eintrag in `archive_entries` klassifiziert
`archive::integrity_check::verify_one` genau einen Befund mit genau einem
Audit-Eintrag:

| Befund | Erkennung | Audit |
|---|---|---|
| **in Ordnung** | Datei lesbar, Größe und SHA-256 stimmen | `archive.integrity_pass` |
| **manipuliert** | Datei lesbar, Größe oder SHA-256 weicht ab | `archive.integrity_fail` |
| **fehlt** (G1-HARDEN.4) | `metadata` liefert `NotFound` | `archive.integrity_missing` |
| **nicht lesbar** (W2) | Datei oder Ordner vorhanden, aber Lese- bzw. Rechtefehler (z. B. `PermissionDenied`) | `archive.integrity_unreadable` |

Ein DB-Fehler bricht den Scan ab, statt als Manipulation zu zählen.

**Warum getrennt?** Eine fehlende Datei kann auch eine schiefgegangene Sync,
ein gelöschtes externes Volume oder ein verschobenes `archive/` sein. Eine nicht
lesbare Datei ist meist ein Rechteproblem — im Container etwa eine falsche UID
am Volume; ohne die Trennung wäre dann das ganze Archiv „manipuliert". Beides
ist ein Problem, weil der Beleg nicht mehr lesbar ist, aber keines heißt „jemand
hat die Datei verändert".

**Ergebnis.** `archive_integrity_checks` hält das Aggregat: `files_failed` =
manipuliert + fehlt (ehrliche Kopfzahl, IDs in `failed_archive_ids_json`). Nicht
lesbare Dateien zählen **nicht** in `files_failed`, sondern gesondert im Ergebnis
des Laufs und im Hinweis; die Historie in `archive_integrity_checks` speichert sie
noch nicht (braucht eine Spalte, also eine Migration). Hinweise
(auch nach einem manuellen Lauf): „Archiv-Integrität gestört" (dringend; nennt
verändert und fehlend getrennt) und „Archiv-Dateien nicht lesbar" (Warnung).

Der fachliche Lesepfad `archive::store::read_and_verify` prüft ebenfalls Größe
und Hash vor der Rückgabe der Bytes und schreibt bei Erfolg `archive.read`.

---

## 9. Versand-Sicherheit

- **SMTP** läuft über `lettre` async, standardmäßig mit **TLS** (Port 465)
  oder **STARTTLS**. Ohne TLS nur ohne Benutzer und Passwort (anlegen, ändern,
  testen; der Transport prüft es zusätzlich, W8). Eigene CA nur als PEM-Datei
  (`KLEIN_BUCH_SMTP_CA_FILE`, ergänzt die eingebauten Wurzeln), keine Option
  „Zertifikat ignorieren“; EHLO-Name über `KLEIN_BUCH_SMTP_HELO_NAME`. Passwort
  aus dem SecretStore (§4), Audit-Eintrag `mail.sent` ohne Passwort.
- **Verbindungstests** (Verbindung testen, Testmail): nur freigegebene Ports
  (SMTP 25/465/587/2525), höchstens 10 je Minute, einheitliche Fehlermeldung,
  Re-Auth; gespeicherte Konten nur per ID (`mail_account_test_saved`).
- **Gespeicherte Ports** (Lauf F5c, ASVS V12.6.1): Anlegen eines Kontos und jeder
  Wechsel von Server oder Port verlangen einen freigegebenen Port wie die Tests
  (`KLEIN_BUCH_ALLOWED_TEST_PORTS`); ein unveränderter Bestandsport bleibt
  nutzbar. Private und Loopback-Adressen sind erlaubt (Relay im LAN oder auf dem
  Docker-Host). Eine CA-Datei ohne PEM-Zertifikat wird mit klarer Meldung
  abgelehnt; der STARTTLS-Handshake mit eigener CA ist getestet
  (`smtp_tls_ca_test`).
- **Empfänger** (Lauf F5c, W14-009): Rechnungs- und Angebotsversand an eine
  andere als die hinterlegte Adresse (Rechnung: Käufer-Snapshot, sonst Kontakt;
  Angebot: Kontakt; Groß-/Kleinschreibung egal) verlangt eine frische Bestätigung
  — Vorprüfung vor der Annahme des Jobs (403 `reauth_required`) und erneut im
  Kern. Standardversand, geänderter Betreff und Text bleiben frei (Restrisiken
  oben); der Abo-Versand nutzt immer die hinterlegte Adresse.
- **Hosts** von SMTP-Konten und SFTP-Zielen: nur Hostname (Buchstaben, Ziffern,
  `-`, `_`, Punkt) oder IP-Adresse, ohne Leerzeichen, Steuerzeichen, Port oder
  Protokoll — geprüft beim Anlegen, Ändern, Testen und vor jeder Verbindung
  (auch für Einträge aus älteren Ständen; Lauf F4c, W14-017).
- **OAuth/Graph** (Desktop-Fassung: PKCE-S256, Loopback-Capture des Redirects)
  gibt es im Server nicht (E9): Commands gesperrt ab W5, aus der Oberfläche
  entfernt und neue Microsoft-Konten abgelehnt ab W8, Code samt Commands seit
  Lauf F6 entfernt (ASVS 14.2.2, ADR 0039 Nr. 138). Ein übernommenes
  Microsoft-Konto versendet nicht (Meldung vor jedem Verbindungsaufbau); beim
  Umstellen auf SMTP oder Löschen entfernt der Server dessen Token.
- Jeder Versand-Versuch (erfolgreich oder nicht) landet im append-only
  `email_log`. Provider-Antwort wird kanonisiert (Header-Stripping) und
  gespeichert — kein OAuth-Token darin. Scheitert der Protokolleintrag, hält ein
  Audit-Eintrag `mail.log_failed` den Versuch fest (Empfänger, Betreff, Beleg),
  die Inbox bekommt den Hinweis „Versandprotokoll unvollständig“ und der Aufrufer
  eine Warnung (Lauf F5c, R4-S2-EMAILLOG).

---

## 10. Was nicht im Scope ist

- **Multi-User / Rollen.** Klein.Buch ist Single-User, auch im Server-Betrieb
  (E1): eine Instanz = ein Nutzer = ein Unternehmen. Wer die Passphrase (und
  im Server den zweiten Faktor) hat, ist der Inhaber. Wer mehreren Personen
  Zugriff geben will, betreibt getrennte Instanzen.
- **SaaS / Mandantenbetrieb.** Selbst gehosteter Betrieb einer eigenen Instanz
  ist mit Phase W vorgesehen (ADR 0039). Ein gehosteter Dienst für Dritte
  bräuchte Mandantentrennung, DSGVO-AVV und ein Betriebskonzept — das ist eine
  eigene Produkt-Linie.
- **Anti-Forensik / Plausible Deniability.** Wer eine verschlüsselte DB
  hat, hat eine. Klein.Buch behauptet nicht, dass die *Existenz* der DB
  verborgen wäre.
- **Hardware-Token.** Yubikey/Smartcard/WebAuthn sind nicht vorgesehen. Im
  Server-Betrieb ist TOTP der zweite Faktor (umgesetzt W6); die Passphrase
  gehört in einen guten Passwort-Manager. Begründung: §11 (ASVS V2.3.2).
- **Physische Unveränderbarkeit des Archivs (WORM).** Siehe „Write-once unter
  Linux".

---

## 11. ASVS 4.0.3 Level 2 — Entscheidungen zu offenen und teilweise erfüllten Punkten (Lauf F)

Die vollständige Checkliste des Security-Reviews W14 liegt im nicht
veröffentlichten Projektordner (`docs/REVIEW-PHASE-W.md`). Hier stehen die dort
als „offen“ oder „teilweise“ geführten Punkte mit ihrer Erledigung bzw.
begründeten Entscheidung (ADR 0039 Nr. 130 ff.; Lauf F4b: Nr. 131; Lauf F4c: Nr. 132; Lauf F5c: Nr. 136; Lauf F7a: Nr. 142). **Erfüllt** = umgesetzt mit
Test; **entschieden** = bewusst so belassen, mit Begründung.

| ASVS | Status | Umsetzung bzw. Begründung |
|---|---|---|
| V2.1.2 (≥ 64 Zeichen erlaubt, > 128 abgelehnt) | entschieden (abweichend) | Neue Passphrasen bis 1024 Zeichen (422 `passphrase_too_long`), darüber hinaus begrenzt das Auth-Körperlimit (16 KiB). Die 128er-Grenze soll KDF-Überlast verhindern; Argon2id und SQLCipher-PBKDF2 verarbeiten die Eingabe nahezu längenunabhängig, die KDF läuft einzeln (Semaphore) und erst nach gültigem TOTP. Eine harte Grenze von 128 schadete Nutzern langer Diceware-Sätze ohne Sicherheitsgewinn. |
| V2.1.6 (aktuelle Passphrase beim Wechsel) | erfüllt (gleichwertig) | Der Wechsel verlangt eine Re-Auth der letzten 5 min (Passphrase + TOTP), gebunden an die dabei rotierte Sitzungs-ID (W14); die alte ID verliert das Re-Auth-Recht. Das entspricht dem Mitsenden der aktuellen Passphrase und deckt Akteur A2. |
| V2.1.7 (Prüfung gegen geleakte/häufige Passwörter) | erfüllt (Lauf F4a) | `server::passphrase_policy`: eingebettete Sperrliste (normalisiert: Kleinbuchstaben, nur Buchstaben/Ziffern; bekannte Sätze, Beispiele aus Anleitungen und Comics, häufige lange Passwörter, Anwendungsbegriffe), triviale Muster (≤ 3 verschiedene Zeichen, Wiederholung, ≥ 80 % Tastatur-/Alphabet-/Zahlenfolgen), Rest nach Abzug von Anwendungs-, Firmen- und E-Mail-Namen < 8 Zeichen. Nur für neue Passphrasen (Einrichtung — schon beim Anfordern des QR-Codes vorab —, Wechsel); kein Netzabruf (local-first). Das bisherige Beispiel der Anmelde-Maske steht auf der Liste und wird nicht mehr angezeigt. Stärkeanzeige (V2.1.8) im UI/UX-Lauf. |
| V2.2.1 (Anti-Automatisierung) | erfüllt (W14, Lauf F4a) | Wartezeit je Gerät/IP und globales Budget (W6), kein Aussperren durch Fremde (Gerätecookie gleitend, Einmal-Code außerhalb des globalen Budgets; W14), dazu die Obergrenze von 100 Fehlversuchen in Folge ohne Gerätecookie (W14-004, siehe Threat Model „Passwort-Raten“). Bekannte Grenze (Lauf F4d): Die Obergrenze zählt auch falsche App-Codes ohne jede Kenntnis der Passphrase (der zweite Faktor wird vor der KDF geprüft) und ist damit von jedem auslösbar, der die Anmeldeseite erreicht; das globale Budget streckt das auf etwa zwei Tage ohne Anmeldung des Inhabers. Folge: Auf neuen Geräten kostet jede Anmeldung einen Einmal-Code, bis die Codes verbraucht sind (danach `totp-reset` im Container); bekannte Geräte bleiben frei. Bewusst so belassen, weil nur Fehlversuche nach gültigem App-Code zu zählen das Durchprobieren des zweiten Faktors nicht mehr begrenzen würde. Der Hinweis zur Sperre unterstellt deshalb keine Kenntnis der Passphrase; aussagekräftig ist eine falsche Passphrase nach gültigem App-Code (drei in Folge → WARN `auth.factor_suspect`, dringender Hinweis mit Empfehlung, den App-Code neu einzurichten). |
| V2.2.3 / V2.5.5 (Benachrichtigung bei Wechsel von Passphrase bzw. TOTP) | entschieden | Es gibt einen Inhaber und keinen Kanal außerhalb der Instanz: Eine Mail über das gespeicherte SMTP-Konto der Instanz an die eigene Adresse wäre kein echter Out-of-Band-Schutz (dieselbe womöglich übernommene Instanz). Vorhanden: append-only Audit (`backup.passphrase.changed`, `auth.totp.renewed`, `auth.sessions_revoked`), Hinweis „Fehlversuche seit der letzten Anmeldung“ nach dem Login, Inbox-Hinweise bei Sperre und Freigabe der TOTP-Anmeldung, Sitzungsübersicht. |
| V2.3.1 (zeitlich begrenzte Erst-Geheimnisse) | entschieden | Der Setup-Token (128 bit OsRng) gilt nur vor der Einrichtung bzw. nach `totp-reset`, nur bis zum nächsten Neustart, ist nach Erfolg ungültig, steht nur im Container-Log und ist rate-limitiert. Ein Ablauf erzwänge Neustarts ohne Sicherheitsgewinn (wer das Log lesen kann, kann auch neu starten). |
| V2.3.2 (FIDO/WebAuthn) | entschieden | Nicht vorgesehen: WebAuthn braucht eine stabile RP-ID bzw. Origin je Instanz und eigene Wiederherstellungswege — zu viel Komplexität für Ein-Nutzer-Selbstbetrieb. TOTP + Passphrase + Einmal-Codes erfüllen L2. Mögliche Idee nach v1.0. |
| V2.6.2 / V2.6.3 (Einmal-Codes: Entropie, gesalzene Ablage) | entschieden | 10 Codes zu 50 bit aus OsRng, gespeichert als SHA-256 in `/data/auth/totp.json` neben dem TOTP-Geheimnis. Wer die Datei liest, hat bereits das TOTP-Geheimnis; Salz oder KDF brächten keinen Schutzgewinn. Online-Raten ist durch Rate-Limit und Passphrase-Pflicht aussichtslos. |
| V2.8.2 (TOTP-Geheimnis geschützt) | entschieden | Klartext in `/data/auth/totp.json` (0600, Verzeichnis 0700, Nutzer 65532). Die Datei muss vor dem Öffnen der Datenbank lesbar sein (TOTP wird vor der KDF geprüft); eine Verschlüsselung mit einem Schlüssel aus demselben Verzeichnis wäre nur Verschleierung, ein HSM gibt es im Container nicht. Das Geheimnis allein öffnet ohne Passphrase nichts. |
| V2.8.5 (Wiederholung eines TOTP-Codes erkennen) | erfüllt (Lauf F4a) | Abgelehnt seit W6 (`last_used_step` persistiert); jetzt im Log eigens `reason=totp_replay` (gleiche Antwort an den Client, kein Orakel). Benachrichtigung über die Obergrenze in Folge (Inbox-Hinweis bei Sperre). |
| V3.3.4 (aktive Sitzungen anzeigen und beenden) | erfüllt (Lauf F4a, W14-005) | `GET /api/auth/sessions` (Anmeldezeit, letzte Nutzung, gekürzte Kennung, „diese Sitzung“; keine IP, kein Gerätename) und `POST /api/auth/sessions/others/revoke` (Re-Auth, Audit mit Anzahl); Block in Einstellungen → Sicherheit. |
| V8.2.2 (keine Geheimnisse im Browser-Speicher) | erfüllt (Lauf F4a, Teil von R5-S2-UI) | Passphrasen stehen nur im Zustand der jeweiligen Maske (Anmeldung, Einrichtung, Re-Auth, Passphrase-Wechsel, Restore) und werden nach dem Absenden bzw. Abbrechen geleert — nie in Stores, URL, `localStorage` oder `sessionStorage`. |
| V11.1.4 (Anti-Automatisierung/Re-Auth für sensible Geschäftsfunktionen) | erfüllt (Lauf F4a, W14-028) | DATEV-, ELSTER- und PDF-Export sowie das Aktivieren/Deaktivieren von Zahlungskonten verlangen Re-Auth wie die übrigen Exporte; das Verkäuferprofil bedingt (nur IBAN, BIC; W14-038); Einträge und Widerruf des §19-Steuerstatus (`tax_status_change_add`/`_revoke`, ADR 0042) immer. Registry-Test der Re-Auth-Matrix (`server_auth_test::reauth_commands_require_fresh_reauth`, `seller_profile_reauth_matrix`). |
| V5.1.1 (HTTP-Parameter-Pollution, doppelte JSON-Schlüssel) | entschieden (Lauf F4b) | Ein Körper wird genau einmal geparst (`serde_json` → typisiertes Args-Struct); bei doppelten Schlüsseln gewinnt der letzte. Es gibt keine vorgelagerte Prüfinstanz (WAF, zweiter Parser), die einen anderen Wert sähe als der Kern — damit keine Pollution zwischen Komponenten. |
| 7.4.3 (letzte Fehlerbehandlung für Unerwartetes) | erfüllt (Lauf F4b) | `CatchPanicLayer` im Router (innerhalb von Policy, Headern und Zugriffsprotokoll): eine Panik ergibt 500 im API-Format mit `ref`, ERROR-Log nur mit der Referenz. Test `server_hardening_test::panic_in_handler_returns_500_with_reference`. |
| V11.1.3 (Grenzen für billige Schreibvorgänge) | entschieden (Lauf F4b) | Nummernverbrauchende Entwürfe brauchen eine Sitzung (ein Inhaber); Render/JVM sind je Sitzung begrenzt (30/min), Jobs auf 8. Entwürfe sind keine Lücken im Nummernkreis, Verwerfen dokumentiert den Verbleib (W14-032, Lauf F2a). Ein eigenes Rate-Limit wäre für Ein-Nutzer-Betrieb unverhältnismäßig. |
| V12.2.1 (Dateityp am Inhalt prüfen) | entschieden (Lauf F4b) | Magic-Bytes-Prüfung am Anfang der Datei gegen eine Allowlist je Upload-Art (PDF: `%PDF-`). Eine Textdatei mit diesem Präfix gilt als PDF — folgenlos: fremde Dateien gehen nur als `attachment` mit CSP `sandbox` und `nosniff` hinaus, Vorlagennamen sind geprüft (W14-F). Strukturelles Parsen jedes PDFs (z. B. `lopdf`) brächte eine neue Parser-Angriffsfläche ohne Schutzgewinn. |
| V12.4.2 (Virenscan von Uploads) | entschieden (Lauf F4b) | Kein Scanner im Image: ClamAV kostet über 300 MB und braucht Signatur-Updates aus dem Netz (widerspricht local-first und schlankem Image). Uploads stammen vom Inhaber selbst; Minderung über Typprüfung, Größenlimits, Auslieferung als `attachment` mit CSP `sandbox`, `nosniff`, Archiv `0400`. Wer scannen will, scannt das Volume auf dem Host. |
| V13.1.5 / V13.2.5 (Inhaltstyp prüfen) | erfüllt (Lauf F4b) | `/api/cmd/*` und `/api/auth/*` verlangen bei nicht leerem Körper `application/json` (Parameter erlaubt), sonst 415 `unsupported_media_type`; leere Körper (Abmelden, Sperren, Commands ohne Argumente) ohne Inhaltstyp. Uploads prüfen Multipart und Typ am Inhalt. Test `server_hardening_test::json_body_requires_json_content_type`. |
| V13.2.2 (Schema-Prüfung der Eingaben, Lesen ohne Ende) | erfüllt bzw. entschieden (Lauf F4b) | Körper kleiner Anfragen höchstens 10 s (`dispatch::BODY_READ_TIMEOUT`, Test `server_limits_test::stalled_request_body_times_out`), Uploads mit Leerlauf-Timeout je Datenblock, Header 10 s. Ein Schema steckt in den typisierten Args-Structs mit Domänenprüfung; `deny_unknown_fields` über rund 170 Structs brächte viel Bruchrisiko (Frontend-Felder, ältere Clients) bei wenig Gewinn — unbekannte Felder bleiben wirkungslos. |
| 14.4.1 (Inhaltstyp mit Zeichensatz) | erfüllt (Lauf F4b) | Textantworten ohne Zeichensatz (`text/html`, `text/css`, `text/javascript` aus `ServeDir`, SVG) bekommen `charset=utf-8` in der Header-Middleware; JSON ist per Definition UTF-8. Test `headers::tests::text_responses_get_utf8_charset`, Header-Schnappschuss im Contract-Test. |
| 14.4.2 (`Content-Disposition` für API-Antworten) | entschieden (Lauf F4b) | API-Antworten werden nie als Dokument geöffnet: CSP `default-src 'none'`, `nosniff`, Pflicht-Header für ändernde Anfragen. Downloads tragen eine RFC-6266-Disposition (`attachment`, `inline` nur für selbst erzeugte PDFs). |
| 14.4.5 (HSTS) | entschieden (Lauf F4b) | `Strict-Transport-Security: max-age=31536000` nur bei HTTPS über einen vertrauten Proxy, **ohne** `includeSubDomains`/`preload`: Die Instanz kennt die Subdomains des Betreibers nicht (ADR 0039 Nr. 63). Beides gehört an den Reverse-Proxy bzw. die Domain des Betreibers; das Handbuch zum Reverse-Proxy empfiehlt es dort. |
| 7.1.2 (keine sonstigen sensiblen Daten im Log) | erfüllt (Lauf F4c, W14-016) | Empfängeradressen und Benutzernamen nur auf DEBUG; ein gescheiterter Auto-Versand nennt auf WARN nur Rechnungsnummer und Fehlerart (`Error::kind`), der Volltext (Antwort des Mailservers mit Empfänger) steht auf DEBUG, im Hinweis und im `email_log`. Serverpfade nicht in Meldungen an den Client (W5-ERR-PATHS). Test `recurring_scheduler_test::auto_send_failure_keeps_recipient_out_of_warn_log`. |
| 7.1.3 / 7.2.1 / 7.2.2 (sicherheitsrelevante Ereignisse, Erfolg und Fehlschlag der Anmeldung, Zugriffsfehler) | erfüllt (Lauf F4c, W14-018) | `auth.{login,setup,reauth}.failed` und `auth.{login,setup,reauth}.ok` mit IP (`auth.reauth.ok` neu); Zugriffs- und Validierungsfehler (401/403/404/415/422/423) stehen im Zugriffsprotokoll mit `ip=`, `req=` und `sess=`. Ein eigenes Ereignis je abgelehnter Prüfung entfällt: Status, Pfad, IP und Sitzung reichen zur Auswertung, die fachliche Meldung geht an den Client. |
| 7.1.4 (Angaben für die Untersuchung eines Vorfalls) | erfüllt (Lauf F4c, W14-018) | UTC-Zeitstempel, Ziel, Level; Zugriffsprotokoll mit Client-IP (nach der Proxy-Regel), Kennung der Anfrage (gleich der `ref` interner Fehler, auch aus abgekoppelten Kernen und Jobs) und gekürzter Sitzungskennung (wie in der Sitzungsübersicht, kein Rückschluss auf das Cookie). Datenschutz: IP im Log aus berechtigtem Interesse (Art. 6 Abs. 1 lit. f DSGVO), Aufbewahrung über die Log-Rotation des Containers. Test `server_log_test::no_passphrase_or_session_id_in_logs`. |
| 7.3.1 (Log-Injektion) | erfüllt (Lauf F4c, W14-017) | Der Log-Writer `logging::OneLine` schreibt jedes Ereignis als eine Zeile (Zeilenumbrüche und Steuerzeichen innerhalb escaped; tracing-subscriber escaped selbst nur ANSI); SMTP-/SFTP-Hosts nur als Hostname oder IP (`nettest::check_host`, beim Anlegen, Ändern, Testen und vor jeder Verbindung); Host-Felder per Debug-Escaping; fail2ban-Filter auf Zeilenanfang, Ziel und Zeilenende verankert (Handbuch „Fehlersuche im Container“). Tests `logging::tests::one_line_per_event`, `nettest::tests::host_validation`, Ablehnung im Log-Test. |
| V5.2.6 / V12.6.1 (SSRF über frei wählbare Hosts und Ports) | erfüllt bzw. entschieden (Lauf F5c) | SMTP- und SFTP-Ziele gibt nur der Inhaber unter Re-Auth ein; Hosts nur als Hostname oder IP (W14-017). Ports: gespeicherte SMTP-Ports jetzt wie die Tests nur aus der Allowlist (25/465/587/2525 bzw. `KLEIN_BUCH_ALLOWED_TEST_PORTS`), SFTP-Ziele schon seit W8 (Allowlist und Pflicht-Probe). Private, Link-Local- und Loopback-Adressen bleiben bewusst erlaubt: der Mail-Relay im LAN, der SFTP-Server auf dem NAS oder ein Dienst auf dem Docker-Host sind der Regelfall im Selbstbetrieb, und wer eine Re-Auth besteht, ist der Inhaber. Tests `server_secrets_test::saved_smtp_port_follows_allowlist`, `connection_tests_are_guarded`. |
| 14.2.6 (Parser in einer Sandbox) | entschieden (Lauf F5c) | KoSIT und Mustang laufen als eigener Java-Prozess (Timeout 120 s, `-Xmx512m`, `ulimit -f` 64 MiB, höchstens 2 gleichzeitig). Im Server-Prozess bleiben: Typst (rendert nur Vorlagen des Betreibers aus `/inputs` bzw. eingebaute, Daten als JSON; in `spawn_blocking` unter der Render-Semaphore), `lopdf` (nur Rechtsdokumente des Inhabers, höchstens 64 MiB je entpacktem Stream, aktive Inhalte werden beim Bundle entfernt) und `quick-xml` (Eingangsrechnungen: Upload höchstens 10 MiB, extrahiertes XML höchstens 10 MiB, keine DTD, keine externen oder unbekannten Entitäten, unvollständiges XML abgelehnt). Ein eigener Render- bzw. Parser-Prozess wäre ein großer Umbau ohne passendes Bedrohungsszenario: die fremde Eingabe (E-Rechnung) erreicht nur den Pull-Parser ohne Entitäten. Tests `einvoice::parser::tests::external_entities_and_entity_bombs_are_never_resolved`, `malformed_xml_errors`, `sidecar_real_mustang_extract_survives_decompression_bomb`. |
| 8.1.4 (Erkennen und Alarmieren bei ungewöhnlich vielen Anfragen) | entschieden (Lauf F7a) | Erkennen und Begrenzen in der Anwendung: Wartezeiten je Client und global für Anmeldung, Einrichtung und Re-Auth, Obergrenze für TOTP ohne Gerätecookie, Render/JVM je Sitzung (429), Verbindungs- und Testlimits. Alarmieren nicht in der Anwendung (ein Inhaber, kein Kanal außerhalb der Instanz): Die Logzeilen sind auswertbar und injektionssicher (`auth.*.failed` mit IP und Grund, `auth.totp_locked`, Zugriffsprotokoll mit Status, IP und Sitzung); Benachrichtigung und Sperre auf Netzebene über fail2ban bzw. Log-/Container-Monitoring des Betreibers. Betriebshandbuch „Monitoring und Alarmierung“ nennt die Zeilen. |
| V11.1.7 / V11.1.8 (Auswertung ungewöhnlicher Abläufe, konfigurierbare Alarmierung) | entschieden (Lauf F7a) | Eine Anomalie-Erkennung der Geschäftslogik ist für einen Nutzer nicht verhältnismäßig. Nachvollziehbarkeit: append-only `audit_log` aller Buchungen, Hinweis auf Fehlversuche nach der Anmeldung (W14-004), Zugriffsprotokoll mit IP und Sitzung (W14-018). Konfigurierbare Alarmierung liegt beim Betreiber (fail2ban, Log-Monitoring), wie 8.1.4. |

---

## Write-once unter Linux

**Was der Code tut** (`archive::store`, W2). Eine Archivdatei entsteht als
Temp-Datei mit eindeutigem Namen (`.<name>.<uuid>.tmp`, `create_new`) im
Zielordner, wird mit `fsync` geschrieben, **vor** dem Sichtbarwerden
schreibgeschützt (`0o400`) und per `hard_link` an den Zielnamen gebracht; der
Hardlink scheitert, wenn das Ziel schon existiert, es wird also nie
überschrieben. Auf Dateisystemen ohne Hardlinks fällt der Code auf `rename`
nach vorheriger Prüfung zurück; das ist ohne No-Clobber-Garantie (die Sperre je
Pfad mildert es), deshalb gehören DB und Archiv nicht auf solche Dateisysteme.
Danach wird der Temp-Name entfernt und der Ordner per `fsync` festgeschrieben
(best-effort). Prüfen, Schreiben und `archive_entries`-INSERT laufen unter einer
Sperre je Zielpfad (`locks::ARCHIVE_PATHS`), das Festschreiben einer Rechnung
unter einer Sperre je Rechnung (`locks::INVOICES`).

- Liegt am Zielpfad eine **nicht registrierte** Datei (Rest eines Abbruchs),
  wird sie als `.<name>.orphan-<uuid>` beiseitegelegt — nie gelöscht — und im
  Audit `archive.store` vermerkt.
- Liegt dort ein **registrierter** Eintrag mit anderem Inhalt, ist das ein
  Fehler; wird ein Beleg nach einem Abbruch neu erzeugt, weicht der Code auf eine
  neue Dateiversion aus (`RE-…-2.pdf`). Der alte Eintrag bleibt unverändert im
  Archiv.
- `trg_archive_no_update` verbietet Änderungen an Pfad, Hash und Größe in
  `archive_entries`.
- Nach einem Restore wird das Archiv wieder auf `0o400` gesetzt (§6).

**Was `0o400` nicht leistet.** Unter Linux schützt der Modus nur vor
versehentlichem Überschreiben des Inhalts. Er verhindert **nicht**:

- dass derselbe Benutzer (die UID des Servers) den Modus zurücksetzt — der
  Eigentümer darf `chmod`;
- dass dieser Benutzer die Datei **löscht oder umbenennt** — dafür zählt das
  Schreibrecht auf den Ordner, und das braucht der Server, um neue Belege
  abzulegen;
- dass eine andere Datei unter demselben Namen abgelegt wird, nachdem die
  alte gelöscht wurde;
- Eingriffe von root auf dem Host oder von jedem, der das Volume direkt
  einbindet.

Ein Unveränderbar-Attribut (`chattr +i`) verlangt `CAP_LINUX_IMMUTABLE`, das ein
Nicht-root-Container ohne Capabilities nicht hat. Klein.Buch verspricht unter
Linux deshalb **keine physische Unveränderbarkeit**.

**Worauf der Schutz beruht: Erkennung und Wiederherstellbarkeit.**

1. **SHA-256 beim Schreiben** — Hash, Größe und Pfad stehen unveränderlich in
   `archive_entries` (in der verschlüsselten DB).
2. **Re-Hash beim Lesen** — `read_and_verify` liefert bei Abweichung einen
   Fehler statt der Bytes; im Server läuft jeder Download darüber (W7,
   `GET /files/archive/{id}`).
3. **Monatlicher Integritätslauf** (§8) — manipulierte, fehlende und nicht
   lesbare Dateien werden getrennt gemeldet, Manipulation und Verlust als
   dringender Hinweis.
4. **Off-Site-Backups** — verschlüsselt, mit Archiv, lange aufbewahrt
   (je ein Backup für 30 Tage / 12 Monate / 7 Jahre; SFTP ohne Löschung durch
   Klein.Buch). Eine veränderte oder gelöschte
   Datei lässt sich aus einem älteren Backup zurückholen.
5. **Snapshots des Dateisystems beim Betreiber** — schreibgeschützte Snapshots
   des Daten-Volumes (z. B. ZFS oder Btrfs) liegen außerhalb der Reichweite des
   Server-Benutzers. Das ist Betriebsaufgabe und im Betriebshandbuch empfohlen
   (`../operations-guide.md` §5, W13).

Ergänzend: Selbsttest beim Serverstart (`chmod 0400` im Archiv anlegen und prüfen;
scheitert er, etwa als root oder wegen NFSv4-ACLs, startet der Server nicht;
umgesetzt W5) und der Hinweis, DB und Archiv nicht auf NFS/SMB/exFAT zu legen
(`../operations-guide.md` §2, W13).

**Was bleibt.** Wer Code als Server-Benutzer ausführt, kann Archivdateien löschen
oder ersetzen. Klein.Buch bemerkt das spätestens beim nächsten Lesen oder
Integritätslauf; zurückholen lässt es sich nur aus Backups oder Snapshots, die
dieser Benutzer nicht erreicht.

---

## Letzte Verifikation

Stand: 2026-10-25 (Phase W nach W12, Fließtext in W13 an die Tabellen
angeglichen; Lauf F4a: Anmeldung, Sitzungen, Geheimnisse im Speicher, §11;
Lauf F4b: Verbindungen, Inhaltstyp, Panik, `/healthz`, Exporte, Sidecar;
Schema v32), ADRs 0009/0034/0035/0036/0039.
Quelle: `src/backup/` (`mod.rs`, `snapshot.rs`, `restore.rs`, `target.rs`,
`sftp.rs`, `manifest.rs`, `encrypt.rs`, `rotation.rs`), `src/db/mod.rs`
(Pool-Open, `prepare_filesystem`, Neuverschlüsseln), `src/server/` (`mod.rs`,
`startup.rs`, `auth.rs`, `policy.rs`, `session.rs`, `ratelimit.rs`, `pending.rs`,
`passphrase_policy.rs`, `idempotency.rs`, `http.rs`, `health.rs`, `headers.rs`,
`dispatch.rs`), `src/downloads.rs`, `src/einvoice/sidecar_health.rs`,
`src/commands/reauth.rs`, `src/archive/store.rs`,
`src/archive/integrity_check.rs`, `src/scheduler/integrity_check_cron.rs`,
`src/limits.rs`, `src/locks.rs`, `src/secrets.rs`, `src/nettest.rs`,
`src/mail/smtp.rs`, Cargo.toml (libsqlite3-sys-Pin).
