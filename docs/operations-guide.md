# Klein.Buch — Betriebshandbuch (Docker)

Für Betreiber einer Klein.Buch-Instanz: Container, Volumes, Reverse-Proxy,
Backups, Updates, Restore, Logs und Ressourcen. Grundlage ist ADR 0039
(Web-/Docker-Betrieb); Werte und Variablen sind gegen den Code geprüft.

Schritt-für-Schritt-Anleitungen stehen im Handbuch (in der App unter Hilfe, im
Repository unter `src-tauri/resources/handbook/<Kapitel>.md`). Dieses Dokument
fasst die Fakten zusammen und verweist dorthin:

| Kapitel | Inhalt |
|---|---|
| `installation-docker` | Erstinstallation mit Docker Compose |
| `reverse-proxy-und-tls` | Proxy, TLS, `KLEIN_BUCH_PUBLIC_URL`, `KLEIN_BUCH_TRUSTED_PROXIES` |
| `volumes-und-rechte` | Volumes, UID, Dateisysteme, Sicherung von `/inputs` |
| `totp-einrichten-und-wiederherstellen` | zweiter Faktor, Einmal-Codes, `totp-reset` |
| `backup-ziel-einrichten` | Off-Site-Ziele (Ordner, SFTP) |
| `updates-und-rueckfall` | Update auf eine neue Version, Rückfall |
| `disaster-recovery` | Neuaufbau auf einem anderen Host aus einem Backup |
| `speicherbedarf` | RAM und Plattenplatz |
| `fehlersuche-im-container` | Startfehler, Log-Zeilen, fail2ban |
| `lizenz-und-quellcode` | AGPL §13, eigene Änderungen, `KLEIN_BUCH_SOURCE_URL` |

---

## 1. Überblick

- **Ein Image, ein Container, ein Nutzer.** `ghcr.io/hozwei/klein-buch:<Version>`
  enthält den Server `klein-buch-server` (axum), die Oberfläche (SPA), den
  Java-Sidecar (KoSIT-Validator, Mustang) mit eigener Java-Laufzeit, die
  mitgelieferten Vorlagen und die Lizenzseite. Nur `linux/amd64`.
- **Zustände:** `setup` (leeres `/data`) → `locked` (Datenbank zu, kein
  Scheduler) ↔ `unlocked`. Nach **jedem** Start ist die Instanz `locked`, bis sich
  der Nutzer im Browser anmeldet (E3). Bis dahin laufen weder Tages-Backup noch
  Abo-Rechnungen oder Integritätsprüfung (Log-Zeile „Instanz gesperrt …“ beim
  Serverstart und alle 6 h); der Scheduler holt Fälliges ab einer Minute nach dem
  Entsperren gestaffelt nach (ein Backup je Tick, Vollscan frühestens im
  zweiten Tick). Mehr als zwei Tage ohne Aktivität meldet die Inbox.
- **TLS** terminiert ein Reverse-Proxy des Betreibers; der Container spricht nur
  HTTP auf Port 8080.
- **Verbindungen nach außen** nur für Versand (SMTP), Off-Site-Backup (SFTP) und
  die opt-in Versionsprüfung (GitHub-Releases). Keine Telemetrie, kein
  Auto-Update.

## 2. Voraussetzungen

- Linux-Host `linux/amd64` mit Docker und Docker Compose.
- Ein eigener Hostname. Klein.Buch läuft nur an der Wurzel eines Hosts
  (`KLEIN_BUCH_PUBLIC_URL` ohne Pfad), nicht unter `/klein-buch/` o. Ä.
- Ein Reverse-Proxy mit gültigem TLS-Zertifikat für diesen Hostname.
- Lokales Dateisystem mit POSIX-Rechten und Hardlinks für `/data` und `/inputs`
  (z. B. ext4, XFS, ZFS, Btrfs). **Nicht** NFS, SMB/CIFS oder exFAT: Der
  Schreibschutz des Archivs und die No-Clobber-Garantie beim Archivieren hängen
  daran; der Start prüft den Schreibschutz und verweigert sonst den Dienst.
  Off-Site-Ziele unter `/backup-targets` dürfen Netzfreigaben sein.
- RAM und Platz siehe §10.

## 3. Installation

Kurzfassung im `README.md` („Schnellstart mit Docker Compose“), ausführlich im
Kapitel `installation-docker`. Ausgangspunkt ist `compose.example.yaml`; die
Einstellungen darin und ihr Zweck:

| Einstellung | Zweck |
|---|---|
| `image: ghcr.io/hozwei/klein-buch:<Version>` | feste Version; es gibt kein `:latest` (E15) |
| `restart: unless-stopped` | Neustart nach Absturz und nach einem Restore (der Server beendet sich danach mit Exit 0, §9). `on-failure` reicht deshalb **nicht**. |
| `user: "1000:1000"` | UID/GID des Prozesses; die Volumes müssen ihr gehören. Ohne `user:` läuft das Image als 65532:65532. Nie als root: Der Start-Selbsttest verweigert root, weil der Schreibschutz der Belege dann nicht greift. |
| `KLEIN_BUCH_PUBLIC_URL`, `KLEIN_BUCH_TRUSTED_PROXIES` | §6 |
| Volumes `/data`, `/inputs`, `/backup-targets/<Name>` | §5 |
| `ports: "127.0.0.1:8080:8080"` | nur lokal erreichbar; nach außen nur über den Proxy. Läuft der Proxy im selben Docker-Netz, entfällt die Port-Freigabe ganz. |
| `read_only: true`, `tmpfs: /tmp` | Root-Dateisystem schreibgeschützt; Temp-Dateien der Anwendung liegen unter `/data/tmp` (`TMPDIR`), `/tmp` ist `HOME` |
| `cap_drop: [ALL]`, `no-new-privileges`, `ulimits.core: 0` | keine Capabilities, keine Rechteausweitung, keine Core-Dumps (der Server setzt zusätzlich `PR_SET_DUMPABLE` 0) |
| `pids_limit: 256`, `mem_limit: 2g` | Grenzen gegen Ausreißer; Speicher siehe §10 |
| `stop_grace_period: 90s` | Herunterfahren (ADR 0040): Scheduler sofort anhalten (Abo-Lauf beendet den Termin), offene Verbindungen höchstens 30 s, laufende Jobs und Tick höchstens 30 s, Pool schließen höchstens 5 s — zusammen unter 70 s. Ein abgebrochenes Backup ist unkritisch (Temp-Datei + rename, Reste räumt der Start). Der Docker-Standard von 10 s ist zu kurz. |
| `logging: json-file`, `max-size`/`max-file` | Log-Rotation |

Erster Start: `docker compose up -d`, dann den Setup-Token aus dem Log lesen
(`docker compose logs klein-buch | grep Setup-Token`). Der Token steht nur im
Container-Log (stderr, nie im übrigen Log), gilt bis zum nächsten Neustart und
wird im Browser zusammen mit Daten-Passwort (≥ 16 Zeichen) und TOTP-Bestätigung
eingegeben. Die Einrichtung ist atomar: Scheitert sie, bleibt kein halber Stand
zurück.

## 4. Umgebungsvariablen

**Pflicht**

| Variable | Bedeutung |
|---|---|
| `KLEIN_BUCH_PUBLIC_URL` | Öffentliche Adresse, z. B. `https://buch.example.org`: `https` oder `http`, Host, optional Port; ohne Pfad, Query, Fragment und Benutzerangaben. Fehlt sie, startet der Server nicht. Anfragen mit anderem Host → 421, fremder `Origin` bei ändernden Anfragen → 403. |

**Optional**

| Variable | Standard | Bedeutung |
|---|---|---|
| `KLEIN_BUCH_TRUSTED_PROXIES` | leer | Adressen vertrauter Reverse-Proxys, CIDR oder einzelne IP, getrennt durch Komma oder Leerzeichen. Nur von dort gelten `X-Forwarded-For/-Proto/-Host`. `…/0` erzeugt eine Warnung. §6. |
| `KLEIN_BUCH_BACKUP_TARGETS_DIR` | `/backup-targets` | Wurzel der Ordner-Ziele; nicht `/`, nicht in oder über `/data` bzw. `/inputs`. |
| `KLEIN_BUCH_MIN_FREE_BYTES` | `2147483648` (2 GiB) | Mindest-Platzreserve auf `/data` für Uploads und Restore (ganze Byte-Zahl, keine Einheiten). |
| `KLEIN_BUCH_MAX_RESTORE_BYTES` | `4294967296` (4 GiB) | Höchstgröße einer hochgeladenen Backup-Datei und ihres entpackten Inhalts. |
| `KLEIN_BUCH_ALLOWED_TEST_PORTS` | SMTP 25, 465, 587, 2525; SFTP 22, 2222 | Ports für Verbindungstests (Komma-Liste); ersetzt die Standardlisten beider Arten. |
| `KLEIN_BUCH_SMTP_HELO_NAME` | Hostname des Containers | EHLO-Name beim SMTP-Versand. |
| `KLEIN_BUCH_SMTP_CA_FILE` | – | eigene CA als PEM-Datei (in einem Volume), ergänzt die eingebauten Wurzeln. Eine Option „Zertifikat ignorieren“ gibt es nicht. |
| `KLEIN_BUCH_SOURCE_URL` | Build-Wert, sonst `https://github.com/hozwei/klein.buch-public` | Quell-URL für den §13-Hinweis (Forks); nur `http(s)` ohne Benutzerangaben, Query oder Fragment, sonst Warnung und ignoriert. |
| `KLEIN_BUCH_UPDATE_URL` | aus der Quell-URL abgeleitet (`…/releases/latest` der GitHub-API) | Abfrage-URL der opt-in Versionsprüfung. |
| `RUST_LOG` | `info` | Log-Filter (`tracing`-Syntax); `sqlx::query=off` wird immer angehängt. |

Ungültige Werte von `KLEIN_BUCH_LISTEN`, `KLEIN_BUCH_TRUSTED_PROXIES`,
`KLEIN_BUCH_MIN_FREE_BYTES`, `KLEIN_BUCH_MAX_RESTORE_BYTES` und
`KLEIN_BUCH_ALLOWED_TEST_PORTS` brechen den Start ab.

**Im Image gesetzt** (nicht ändern): `KLEIN_BUCH_DATA_DIR=/data`,
`KLEIN_BUCH_INPUTS_DIR=/inputs`, `KLEIN_BUCH_RESOURCE_DIR=/opt/klein-buch`,
`KLEIN_BUCH_WEB_DIR=/opt/klein-buch/web`, `KLEIN_BUCH_LISTEN=0.0.0.0:8080`,
`TZ=Europe/Berlin`, `TMPDIR=/data/tmp`, `HOME=/tmp`, `JAVA_HOME`. Fachliche Daten
(Belegdatum, Geschäftsjahr) rechnet der Server immer in Europe/Berlin; intern
setzt er für seinen Prozess `TZ=UTC`, damit die Zeitstempel der Datenbank stimmen.

**Nur für Tests und Entwicklung** — im Betrieb nie setzen:
`KLEIN_BUCH_ALLOW_UNMOUNTED=1` (keine Volume-Prüfung), `KLEIN_BUCH_INSECURE_HTTP=1`
(Anmeldung über unverschlüsseltes HTTP, mit Warnung in der Oberfläche),
`KLEIN_BUCH_SIDECAR_MOCK` (ein Release-Image startet damit nicht).

## 5. Volumes und Rechte

Alle Volumes müssen der UID aus `user:` gehören und für sie beschreibbar sein.
`/data` und `/inputs` müssen eingebundene Volumes sein (der Start prüft das in
`/proc/self/mountinfo`), sonst gingen die Daten mit dem Container verloren.

**`/data`** — Datenbank und Archiv, auf lokalem Dateisystem (§2):

| Pfad | Inhalt |
|---|---|
| `klein-buch.sqlite` (+ `-wal`, `-shm`) | SQLCipher-verschlüsselte Datenbank |
| `archive/` | festgeschriebene Belege (PDF, XML, Belegdateien), je Datei `0400`; **unverschlüsselt** |
| `branding/` | Logo und Unterschrift |
| `backups/` | lokale Backups (Floor), verschlüsselt |
| `auth/totp.json` | TOTP-Geheimnis, gehashte Einmal-Codes (Datei 0600, Ordner 0700); nicht Teil der Backups |
| `auth/device.key` | Schlüssel für die Gerätecookies des Rate-Limits; nicht Teil der Backups |
| `tmp/` | Uploads, Downloads, Snapshots, Java-Temp; wird beim Serverstart geleert, im Betrieb stündlich von Resten älter als 6 h befreit (Up-/Downloads nach 1 h) |
| `.klein-buch-instance`, `.klein-buch.lock` | Instanz-Marker; Sperre gegen einen zweiten Prozess |
| `restore-staging/`, `RESTORE_*.json`, `*.rollback` | nur während bzw. nach einem Restore |

Archivdateien, Logo und Unterschrift liegen im Klartext; ihr Schutz ist Sache
des Hosts (Zugriffsrechte, Volume-Verschlüsselung). Empfohlen sind
schreibgeschützte Snapshots des Daten-Volumes (z. B. ZFS oder Btrfs): Unter Linux
kann der Server-Benutzer Archivdateien löschen oder ersetzen; Klein.Buch erkennt
das (SHA-256 bei jedem Abruf, monatliche Integritätsprüfung), zurückholen lässt
es sich nur aus Backups oder Snapshots (`docs/architecture/security.md`,
„Write-once unter Linux“).

**`/inputs`** — vom Menschen gepflegte Dateien: PDF-Vorlagen
(`pdf-templates/`), Mail-Vorlagen, AfA-Tabelle, Beispiele. Beim Containerstart ergänzt
Klein.Buch fehlende Dateien aus dem Image. `/inputs/.seed-manifest.json` merkt
sich die zuletzt kopierte Fassung: Eine unveränderte Datei ersetzt ein Update
durch die neue Fassung; eine vom Nutzer geänderte bleibt unangetastet, die neue
Fassung liegt einmal daneben als `<name>.neu` (Hinweis in der Inbox nach dem
Entsperren). Das Manifest merkt sich angebotene Fassungen (`offered`): Eine
gelöschte `.neu`-Datei kommt erst mit einer weiteren Image-Fassung wieder
(Lauf F, D-10). **`/inputs` ist nicht Teil der Klein.Buch-Backups** und braucht eine
eigene Sicherung beim Betreiber. Nach einem Restore meldet die Inbox Vorlagen, die
in der Datenbank referenziert sind, aber in `/inputs` fehlen.

**`/backup-targets/<Name>`** — je Off-Site-Ordner ein eigenes Volume (z. B.
NAS-Freigabe oder Ordner eines Sync-Clients). Die erste Ebene unter
`/backup-targets` legt Klein.Buch nie selbst an: Ein Ziel ist nur nutzbar, wenn
das Volume eingebunden ist. Darunter dürfen Unterordner entstehen.

**Prüfungen vor dem Port-Bind** (fail-closed, jede bricht den Start ab): `/data`
und `/inputs` vorhanden, beschreibbar und eingebunden; kein zweiter Prozess auf
demselben `/data`; ein vorgemerkter Factory Reset der Desktop-Fassung wird nicht
angewendet, sondern verweigert den Start; Datenbank und Instanz-Marker passen
zusammen (ohne beide muss `archive/` leer sein); keine unverschlüsselte
Datenbank; Selbsttest des Schreibschutzes im Archiv (scheitert als root, mit
NFSv4-ACLs und auf ungeeigneten Dateisystemen). Beim Entsperren prüft eine
Stichprobe, ob die Archivpfade der Datenbank unter `/data/archive` liegen, und
`PRAGMA foreign_key_check` die Fremdschlüssel der ganzen Datenbank — nach einem
Update schon vor der Umstellung und vor dem Pflicht-Backup (die Datenbank bleibt
dann auf dem alten Schema, das vorige Image öffnet sie), danach erneut. Ein
Treffer (oder eine gescheiterte Vorab-Prüfung einer Migration) lässt die
Instanz gesperrt (409 `db_integrity_failed`, Einzelheiten im Log, Handbuch
`fehlersuche-im-container`).

Kapitel: `volumes-und-rechte`.

## 6. Reverse-Proxy und TLS

- `KLEIN_BUCH_PUBLIC_URL` muss genau die Adresse sein, unter der Nutzer die
  Instanz aufrufen (Schema, Host, Port). Der Proxy reicht `Host` durch oder setzt
  `X-Forwarded-Host`.
- Der Proxy **setzt bzw. überschreibt** `X-Forwarded-For` und
  `X-Forwarded-Proto: https`. Ein Proxy, der diese Header vom Client ungeprüft
  durchreicht, hebelt die HTTPS-Regel aus.
- `KLEIN_BUCH_TRUSTED_PROXIES` ist die Adresse, mit der der Proxy **im Container
  ankommt**: bei Port-Freigabe auf dem Host meist das Gateway des Docker-Netzes,
  im gemeinsamen Docker-Netz die Adresse des Proxy-Containers. Die Client-IP ist
  der von rechts erste Eintrag in `X-Forwarded-For` außerhalb dieser Liste.
- **Einrichten und Anmelden** (auch Re-Auth, Passphrase- und TOTP-Wechsel) gehen
  nur über einen vertrauten Proxy mit `X-Forwarded-Proto: https` oder direkt über
  Loopback ohne Weiterleitungs-Header. Sonst antwortet der Server 403
  „Anmeldung nur über HTTPS (Reverse-Proxy) oder lokal möglich.“.
- Stimmt `KLEIN_BUCH_TRUSTED_PROXIES` nicht, sieht der Server nur die Adresse des
  Proxys: Anmelden scheitert an der HTTPS-Regel, und die Wartezeiten nach
  Fehlversuchen gelten für alle Clients gemeinsam. Prüfen lässt sich die Liste mit
  einem absichtlich falschen Anmeldeversuch: Die Log-Zeile
  `auth.login.failed ip=…` (§11) muss die Adresse des Clients zeigen, nicht die
  des Proxys.
- Der Server setzt die Sicherheits-Header selbst (CSP mit Skript-Hashes, HSTS bei
  HTTPS ohne `includeSubDomains`, `nosniff`, `frame-ancestors 'none'`). Der Proxy
  sollte sie nicht überschreiben.
- Uploads: Belege 20 MiB, E-Rechnungen 10 MiB, Logo/Unterschrift 2 MiB,
  Backup-Dateien bis `KLEIN_BUCH_MAX_RESTORE_BYTES`. Das Upload-Limit des Proxys
  (bei nginx `client_max_body_size`) und seine Timeouts müssen dazu passen, sonst
  scheitert ein Restore schon am Proxy.

Kapitel: `reverse-proxy-und-tls`.

## 7. Backups und Off-Site

**Was gesichert wird.** Ein Backup (`.kbk`) enthält einen konsistenten
SQLCipher-Snapshot der Datenbank (samt der verschlüsselten Zugangsdaten für SMTP
und SFTP), das Archiv und `branding/`, verschlüsselt mit Argon2id + AES-256-GCM
und dem Daten-Passwort zum Zeitpunkt des Backups. **Nicht** enthalten:
`/inputs`, `/data/auth/totp.json` und die Compose-Datei bzw. Umgebung — die sichert
der Betreiber selbst.

**Wann.** Bei jedem Festschreiben (Rechnung, Storno, Kosten, Privatbewegung;
Abo-Läufe eines Ticks ergeben zusammen genau ein Backup), einmal je
Kalendertag (Europe/Berlin) ab der eingestellten Uhrzeit (Standard 02:30,
Einstellungen → Backups; beim ersten Tick danach, nach einer Sperre genau
eines; nach einem Fehlschlag frühestens nach 1 h erneut, persistiert; nur
entsperrt), manuell, vor jedem Restore (Pre-Restore), vor jeder
Datenbank-Umstellung (§8) und nach einem Passphrase-Wechsel. Das Erst-Backup
der Einrichtung zählt als Tages-Backup dieses Tages.

**Wohin.**

- **Floor** unter `/data/backups`, immer. Aufbewahrung nach Kalender-Eimern
  (ADR 0040): je das jüngste Backup der letzten 7 Tage mit Backup, der
  letzten 3 Monate und des letzten Jahres, solange ein Off-Site-Ziel zuletzt
  erfolgreich war, sonst 30 / 12 / 7.
- **Off-Site-Ziele**, beliebig kombinierbar (höchstens 10): Ordner unter
  `/backup-targets/…` und SFTP-Server (Passwort-Anmeldung, Host-Key-Fingerprint
  wird beim Anlegen aus dem Verbindungstest übernommen und danach geprüft). Jedes
  Backup geht an jedes Ziel, je Ziel mit Zeitlimit; Aufbewahrung je Ordner-Ziel
  30 Tage / 12 Monate / 7 Jahre. **Auf SFTP-Zielen löscht Klein.Buch nichts**
  (Schutz der Off-Site-Kopien vor einer kompromittierten Instanz); dort rotiert
  der Betreiber selbst (Snapshots oder zeitgesteuertes Aufräumen, Platzbedarf im
  Handbuch „Speicher und Ressourcen“). SFTP-Ziele lassen sich nicht ändern, nur
  entfernen und neu anlegen.
- Manuelle Backups (dazu zählt das Backup vor einer Umstellung), Pre-Restore-
  Backups und nach einem Restore übernommene Backups werden **nie** automatisch
  gelöscht; der Platz auf `/data` und den Zielen wächst damit.
- Während einer Pause der automatischen Abläufe (nach einem Restore oder einem
  Uhrsprung, ADR 0040) ruht die Rotation; nach einem Restore gehen Backups bis
  zur Freigabe nur in den Floor.

**Überwachung.** Ein gescheitertes Ziel steht im append-only Backup-Protokoll
und als Hinweis in der Inbox; ohne Ziel kommt wöchentlich ein Hinweis, die
Übersicht zeigt eine Aufgabe. Das Festschreiben blockiert das nie. Den Status
`offsite` (`missing` | `failing` | `ok`) liefert auch der Healthcheck (§11).

Off-Site ist Pflicht: Die lokalen Backups liegen auf demselben Volume wie die
Daten. Kapitel: `backup-ziel-einrichten`, `backup-und-wiederherstellen`,
`backup-pruefen`.

## 8. Updates und Rückfall

Images tragen nur volle Versionen; Tags werden nie verschoben oder gelöscht
(E15). Die App kann neue Versionen melden (opt-in, Einstellungen → Sicherheit &
Anmeldung); das Update selbst löst der Betreiber aus. Einen Update-Knopf in der
App gibt es nicht (er bräuchte `docker.sock`, also praktisch root auf dem Host).

**Update:**

1. `CHANGELOG.md` bzw. Release-Notizen lesen; Signatur prüfen (`SECURITY.md`).
2. Sicherstellen, dass ein aktuelles Off-Site-Backup und eine Sicherung von
   `/inputs` existieren.
3. In der Compose-Datei das Tag ändern, dann
   `docker compose pull && docker compose up -d`.
4. Im Browser anmelden. Bringt die neue Version eine Datenbank-Umstellung mit,
   legt Klein.Buch **vor** der Migration ein Backup an, liest es vollständig
   zurück und migriert erst danach (Audit `backup.pre_migration`). Scheitert
   das Backup, wird nicht migriert und die Instanz bleibt gesperrt (Meldung im Log
   und in der Inbox).
5. Neue Fassungen der mitgelieferten Vorlagen landen in `/inputs` (§5).

**Rückfall** nach einem gescheiterten Update: vorheriges Tag starten und das
Backup von vor der Migration einspielen. Eine bereits migrierte Datenbank öffnet
das ältere Image nicht (Schema-Check, keine Down-Migration). Dann: Container
stoppen, das bisherige `/data` beiseitelegen (nicht löschen), mit leerem `/data`
und dem vorherigen Tag neu einrichten und das Backup per Upload wiederherstellen
(§9). Die Backup-Datei liegt im beiseitegelegten `/data/backups` und auf den
Off-Site-Zielen.

Kapitel: `updates-und-rueckfall`.

## 9. Restore und Disaster Recovery

**Restore** (Einstellungen → Backups → Wiederherstellen; Re-Auth): Backup-Datei
hochladen oder ein lokales Backup aus der Liste wählen und das Daten-Passwort
**dieses Backups** eingeben. Ablauf: Manifest und Format prüfen → Schema
`30 ≤ Version ≤ aktuelle Version` (neuere Backups werden abgelehnt) →
entschlüsseln (eine falsche Passphrase verändert nichts) → Pre-Restore-Backup des
aktuellen Stands → Platzprüfung → gehärtet entpacken → gestagte Datenbank prüfen.
Danach lehnt der Server Änderungen ab, beendet sich geordnet (Exit 0), und die
Container-Verwaltung startet ihn neu; der Tausch läuft vor dem Port-Bind. Danach
gilt das Daten-Passwort des Backups; der zweite Faktor (TOTP) bleibt der der
Instanz.

Nach dem Restore (erstes Entsperren, ADR 0040): Die **automatischen Abläufe
pausieren** (Abo-Rechnungen, Abo-Kosten, Auto-AfA, Versand; Grund `restore`),
Backups gehen **nur in den Floor**, die Rotation ruht — bis der Inhaber im
Browser freigibt (Re-Auth, Frage „produktive Instanz?“). Eine Probe-Instanz
wird nie freigegeben. Die **Nummernkreise** der ersetzten Datenbank werden
übernommen, wo sie höher waren (keine Doppelvergabe, Audit
`numbering.raised_after_restore`); bei Disaster Recovery in eine leere Instanz
hebt der Inhaber sie unter Einstellungen → Backups → Nummernkreise an (Re-Auth,
Begründung, nur aufwärts). Backups im Floor, die nach dem eingespielten Stand
entstanden, stehen wieder in der Historie (als manuell).

Voraussetzungen und Grenzen:

- Restart-Policy, die auch nach Exit 0 neu startet (`unless-stopped`, §3).
- Freier Platz auf `/data`: das Doppelte der entpackten Größe plus
  `KLEIN_BUCH_MIN_FREE_BYTES`.
- RAM: etwa das Dreifache der Backup-Größe (§10).
- Backups der Desktop-Fassung mit festgeschriebenen Belegen werden abgelehnt
  (Windows-Archivpfade; kein Datenumzug, E17). Backups ab Schema 30 einer
  Server-Instanz sind einspielbar.

**Disaster Recovery** (Host verloren, Off-Site-Backup vorhanden): neuen Host mit
derselben oder einer neueren Version aufsetzen, `/inputs` aus der eigenen
Sicherung zurückspielen, mit leerem `/data` einrichten (Setup-Token, beliebiges
Daten-Passwort, neuer TOTP), dann das Backup per Upload wiederherstellen. Nach dem
Neustart gilt das Daten-Passwort des Backups und der neu eingerichtete TOTP.
Off-Site-Ziele (die Ziel-Liste steht in der Datenbank) müssen auf dem neuen Host
wieder unter denselben Namen eingebunden sein.

Kapitel: `backup-und-wiederherstellen`, `disaster-recovery`.

## 10. Ressourcen

**Arbeitsspeicher** — größte Posten:

- **Backup erstellen:** Das Backup entsteht im Speicher (Inhalts-ZIP,
  verschlüsselte Kopie, fertige Datei): etwa das **Dreifache der Archivgröße**.
- **Restore:** Die Backup-Datei wird zum Entschlüsseln ganz gelesen (Datei,
  Ciphertext-Kopie, Klartext): etwa das **Dreifache der Backup-Größe**.
- **Java-Sidecar** (Validierung, ZUGFeRD): je Lauf höchstens 512 MiB Heap
  (`-Xmx512m`), höchstens zwei Läufe gleichzeitig.
- **Passphrase-Ableitung** (Argon2id): 64 MiB je Ableitung, höchstens eine
  gleichzeitig.

`compose.example.yaml` setzt `mem_limit: 2g` mit der Faustregel „512 MiB
Grundbedarf + 3 × größtes Backup“. Mit wachsendem Archiv `mem_limit` mitziehen,
sonst beendet der Kernel den Prozess mitten im Backup.

**Plattenplatz** auf `/data`: Datenbank + Archiv + lokale Backups (Floor, plus
nie rotierte manuelle und Pre-Restore-Backups) + Reserve
`KLEIN_BUCH_MIN_FREE_BYTES`; für einen Restore zusätzlich das Doppelte der
entpackten Größe. Uploads unter der Reserve lehnt der Server mit 507 ab.

**Lastgrenzen** (fest): höchstens 64 Anfragen gleichzeitig; je Kostenklasse
PDF-Erzeugung 2, Java 2, Passphrase-Ableitung 1, Backup/Export 1; je Sitzung
höchstens 30 PDF-/Java-Aufrufe pro Minute; 30 s Timeout je API-Aufruf (lange
Arbeiten laufen als Job weiter).

Kapitel: `speicherbedarf`.

## 11. Logs, Healthcheck, Fehlersuche

**Logs** gehen nach stdout/stderr (`docker compose logs klein-buch`). Standard
ist `info`; `RUST_LOG` ändert den Filter. Nie im Log: Passphrase, TOTP- und
Einmal-Codes, Sitzungs-IDs, Zugangsdaten; der Setup-Token steht nur als eigene
Zeile auf stderr. Das Zugriffsprotokoll nennt Methode, Pfad ohne Query, Status
und Dauer.

Fehlversuche stehen nur im Log (nicht im Audit-Log), als `WARN` mit Ziel
`klein_buch_lib::auth`:

```text
auth.login.failed ip=<Client-IP> reason=totp|passphrase|rate_limited
auth.setup.failed ip=<Client-IP> reason=token|passphrase
auth.reauth.failed ip=<Client-IP> reason=totp|passphrase
```

Die Zeilen eignen sich für fail2ban (Regex auf `auth\.\w+\.failed ip=<HOST>`).
Klein.Buch selbst antwortet ab dem fünften Fehlversuch je Client mit Wartezeiten
(429 mit `Retry-After`, verdoppelnd bis 1 h, keine Dauersperre).

**Healthcheck.** Das Image prüft sich selbst (`HEALTHCHECK`, alle 30 s, über den
Subcommand `klein-buch-server healthcheck`). `GET /healthz` antwortet von außen
nur 200 bzw. 503 (Herunterfahren, Neustart nach Restore) ohne Inhalt; über
Loopback zusätzlich mit Zustand und Off-Site-Status:

```sh
docker compose exec klein-buch klein-buch-server healthcheck
# {"offsite":null,"state":"locked"}
```

`state` ist `setup`, `locked` oder `unlocked`; `locked` gilt als gesund (sonst
entstünde nach jedem Neustart eine Neustartschleife). `offsite` (`missing` |
`failing` | `ok`) ist nur entsperrt lesbar, sonst `null`. Wer überwachen will, ob
nach einem Neustart jemand entsperrt hat, wertet `state` aus.

**Subcommands** des Binaries: `serve` (Standard), `healthcheck`, `totp-reset`,
`version`.

**Häufige Startfehler** (Log-Meldung → Ursache):

| Meldung (Auszug) | Ursache |
|---|---|
| `KLEIN_BUCH_PUBLIC_URL fehlt` / `… ungültig` | Variable fehlt oder hat Pfad, Query, Benutzerangaben |
| `…-Verzeichnis … ist kein eingebundenes Volume` | `/data` oder `/inputs` nicht als Volume eingebunden |
| `…-Verzeichnis … ist nicht beschreibbar` | Volume gehört nicht der UID aus `user:` |
| `Auf … läuft bereits ein Klein.Buch-Prozess` | zweiter Container auf demselben `/data` |
| `Selbsttest im Archiv … gescheitert` | Container läuft als root, ACLs (NFSv4) oder ungeeignetes Dateisystem |
| `Datenbank … ohne Instanz-Marker` / `Instanz-Marker … ohne Datenbank` | fremdes oder unvollständiges Datenverzeichnis |
| `Archiv … enthält Dateien, aber es gibt weder Datenbank noch Instanz-Marker` | keine Neueinrichtung über einem vorhandenen Archiv |
| `… ist unverschlüsselt` | Klartext-Datenbank (z. B. Desktop-Bestand); wird nicht übernommen |
| `FACTORY_RESET_PENDING.json gefunden` | Marker der Desktop-Fassung; Datei prüfen und entfernen |
| `KLEIN_BUCH_SIDECAR_MOCK ist in Release-Builds nicht erlaubt` | Test-Schalter gesetzt |

Beim Entsperren: `Archiv-Eintrag … liegt nicht unter …` → das Datenverzeichnis
passt nicht zur Datenbank (Entsperren verweigert, Anmeldung antwortet 409).

Kapitel: `fehlersuche-im-container`.

## 12. Anmeldung und zweiter Faktor

- Anmeldung mit Daten-Passwort **und** TOTP (oder einem der zehn Einmal-Codes);
  ein Code gilt nur einmal, auch über Neustarts. Sitzungen enden nach 30 min ohne
  Nutzung bzw. nach 12 h; Re-Auth gilt 5 min. Sitzungen leben nur im Speicher:
  Ein Neustart meldet alle Geräte ab.
- „Datenbank sperren“ (in der Oberfläche) meldet sofort alle Geräte ab,
  schließt die Datenbank und pausiert den Scheduler. Ein laufender Abo-Lauf
  beendet den aktuellen Termin, der Archiv-Vollscan die aktuelle Datei;
  gesperrt wird spätestens nach 10 s.
- **Gerät mit der Authenticator-App verloren** und keine Einmal-Codes mehr:

  ```sh
  docker compose exec klein-buch klein-buch-server totp-reset
  ```

  Das löscht `/data/auth/totp.json`, beendet alle Sitzungen und erzeugt beim
  nächsten Aufruf der Anmeldeseite einen neuen Setup-Token im Container-Log. Neu
  eingerichtet wird mit Setup-Token und dem bisherigen Daten-Passwort; die Daten
  bleiben unverändert.
- **Daten-Passwort verloren:** Totalverlust (Datenbank und Backups), by design.

Kapitel: `totp-einrichten-und-wiederherstellen`.

## 13. Fachliche Abläufe mit Betriebsbezug

- **Migrations-Export** (Einstellungen → Daten exportieren; Re-Auth): offenes
  ZIP mit einer JSON-Datei je Tabelle, dem Archiv, dem Schema, `erd.md`,
  `manifest.json` und `read_export.py` zum Sichten ohne Klein.Buch. Zugangsdaten
  sind nicht enthalten. Wie alle Exporte entsteht er unter `/data/tmp` und kommt
  als Browser-Download (einmal abrufbar, nach 1 h gelöscht).
- **Geschäftsjahres-Abschluss** (Menü → Geschäftsjahr): nur für
  abgelaufene Jahre, unumkehrbar; Korrekturen danach nur per Storno-Beleg.
- **E-Mail-Versandprotokoll:** jeder Versandversuch append-only mit
  Provider-Antwort (Einstellungen → E-Mail-Versand). Eine angenommene Mail ist
  beim Provider eingeliefert, nicht zwingend im Posteingang des Empfängers.
- **Sidecar-Versionen** (KoSIT-Validator, Mustang, XRechnung-Konfiguration) sind
  Teil des Images; ein Update kommt mit einem neuen Release
  (`src-tauri/sidecar/versions.json`).
- **Neubeginn ohne Daten:** Es gibt keine Totallöschung in der App (E7). Eine neue
  Instanz bekommt ein leeres Volume; was mit dem alten geschieht, entscheidet der
  Betreiber — unter Beachtung der zehnjährigen Aufbewahrungspflicht.

---

## Letzte Verifikation

Stand: 2026-10-24 (Phase W, Block W13; Version 2026.5.1, Schema v31). Quellen:
`Dockerfile`, `compose.example.yaml`, `src-tauri/src/server/` (`config.rs`,
`startup.rs`, `health.rs`, `auth.rs`, `policy.rs`, `logging.rs`, `mod.rs`),
`src-tauri/src/config.rs`, `src-tauri/src/uploads.rs`, `src-tauri/src/nettest.rs`,
`src-tauri/src/mail/smtp.rs`, `src-tauri/src/build_info.rs`,
`src-tauri/src/scheduler/update_check.rs`, `src-tauri/src/backup/`, ADR 0039.
