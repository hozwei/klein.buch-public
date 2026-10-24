# Sicherheits-Modell

> Vertiefung zu „Backup" und „Verschlüsselung at Rest" in
> `../ARCHITECTURE.md` §7. Wie die Geheimnis-Kette gebaut ist, wo welche
> Geheimnisse leben, wie Bootstrap, Restore und Factory Reset race-frei
> zueinander passen — und gegen wen sich Klein.Buch im Internetbetrieb
> (Phase W) schützen muss.

ADR-Basis: 0035 (Verschlüsselung at Rest), 0034 (Backup-Ziele/Retention/Log),
0036 (Factory Reset), 0009 (Backup als First-Class), 0011 + 0028
(Mail-Credentials; seit W8 im SecretStore der DB), 0039 (Web-/Docker-Betrieb).

**Stand des Codes.** Beschrieben ist der Stand nach den Phase-W-Blöcken W1 und
W2: Die Desktop-Shell ist noch vorhanden, eine HTTP-Schnittstelle gibt es noch
nicht. Was der Server-Betrieb ändert, steht an der jeweiligen Stelle mit dem
Block des Arbeitsplans („geplant (Wn)"). Nichts davon gilt als erledigt, bevor
der Block abgeschlossen ist.

---

## Threat Model für den Internetbetrieb (Phase W, ADR 0039)

Mit Phase W wird Klein.Buch ein selbst gehosteter Dienst im Docker-Image: **ein
Nutzer pro Instanz**, bedient im Browser, aus dem Internet erreichbar; TLS
terminiert der Reverse-Proxy des Betreibers. Das bisherige Bedrohungsmodell
(ADR 0035: gestohlenes Gerät, kopierte Datei) bleibt gültig, ist aber nicht mehr
das maßgebliche. Maßgeblich ist jetzt ein Angreifer im Netz.

Status in den Tabellen: **umgesetzt (W1/W2)** = heute im Code; **umgesetzt** =
schon vor Phase W vorhanden; **geplant (Wn)** = Block des Arbeitsplans, noch
nicht im Code.

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
| Aufruf beliebiger Commands, auch vor dem Entsperren | explizite Allowlist (alles andere 404), Sitzung Pflicht, DB-Commands nur im Zustand `unlocked` (sonst 423) | umgesetzt (W5) |
| Übernahme einer frischen Instanz (wer zuerst kommt, setzt die Passphrase) | Setup-Token aus dem Container-Log, Setup atomar mit Passphrase + TOTP | umgesetzt (W6) |
| Passwort-Raten | Passphrase ≥ 16 Zeichen | umgesetzt |
| | TOTP vor der KDF; Wartezeit je Client-IP ab 5 Fehlversuchen (1 min, verdoppelnd bis 1 h, kein Dauer-Lockout); Gerätecookie gegen fremd ausgelöste Wartezeiten | umgesetzt (W6; IPv6 je /64, globales Budget ohne Gerätecookie) |
| | höchstens eine Passphrase-Ableitung gleichzeitig (Semaphore `KDF` = 1) | umgesetzt (W2) |
| Sitzungsdiebstahl | Cookie `__Host-…`, HttpOnly, Secure, SameSite=Strict; ID 256 bit, serverseitig nur als SHA-256; Idle 30 min, absolut 12 h; Re-Auth (5 min) für Restore, Ziele, Mailkonten, Exporte, Passphrase-Wechsel | umgesetzt (W6) |
| CSRF, Clickjacking | Pflicht-Header `X-KleinBuch-Request` + Origin/Host-Prüfung; CSP mit Script-Hashes und `frame-ancestors 'none'`; kein CORS | umgesetzt (W5/W6) |
| Gefälschte Proxy-Header | `KLEIN_BUCH_PUBLIC_URL` Pflicht; `X-Forwarded-*` nur von `KLEIN_BUCH_TRUSTED_PROXIES`; Login nur über HTTPS-Proxy oder Loopback | umgesetzt (W6) |
| Überlast durch teure Arbeit | Typst, Argon2id und ZIP außerhalb der async-Runtime; Semaphoren `RENDER` 2, `JVM` 2, `KDF` 1 (`limits.rs`) | umgesetzt (W2); seit W5 laufen Kerne als eigene Tasks — ein Abbruch des Requests bricht sie nicht mehr ab |
| | Header-Read-Timeout 10 s, Request-Timeouts, `ConcurrencyLimitLayer(64)`, Job-Warteschlange ≤ 8, ≤ 30 Render-/JVM-Aufrufe je Sitzung und Minute, Body-Limits je Route | umgesetzt (W5; globale Grenze 64 ohne Warteschlange) |
| Interna in Fehlermeldungen | Fehlerabbildung je Variante; sonst 500 mit `ref`, Volltext nur im Log | umgesetzt (W5) |
| Versionsangabe vor dem Login | `GET /api/public/info` (Version, Commit, Quell-URL) ist wegen AGPL §13 öffentlich | bewusst akzeptiert (ADR 0039) |

**Uploads** (Belege, Anhänge, E-Rechnungen, Rechtsdokumente, Logo, Unterschrift)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Pfad-Tricks im Dateinamen | `archive::store` lehnt Namen mit `/`, `\`, `:` ab | umgesetzt |
| Übergroße oder falsch deklarierte Dateien (heute JSON-Bytes ohne Limit außer Branding) | Multipart gestreamt, Limits je Art (20 / 10 / 2 MiB), Typ per Magic Bytes gegen Allowlist, `mime_type` aus dieser Prüfung, freier Platz vor dem Archivieren | umgesetzt (W7): `POST /api/upload/{art}`, 413/415/507 |
| Skript in einer ausgelieferten fremden Datei | fremde Dateien nur als `attachment` mit `Content-Security-Policy: sandbox`; `inline` nur für selbst erzeugte PDFs; `nosniff` | umgesetzt (W7): `/files/archive`, `/files/download` |
| Auslieferung manipulierter oder fremder Archivdateien | Download nur über `read_and_verify` (Hash vor dem ersten Byte), `canonicalize` gegen `archive_dir`, Audit `archive.read` | umgesetzt (W7) |

**Restore-Upload** (`.kbk`)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Manipulierte Backup-Datei | GCM-Tag + SHA-256 des Content-ZIP (§3.2) | umgesetzt |
| Pfad-Traversal im Content-ZIP | Prüfung je Pfadkomponente (`..`, Wurzel, Laufwerkspräfix → Abbruch) | umgesetzt (W2) |
| Zip-Bombe, fremde Einträge, Symlinks | Allowlist `klein-buch.sqlite`, `archive/**`, `branding/**`; ≤ 200 000 Einträge; gelesene Bytes begrenzt; Abbruch über 200:1; Upload-Limit `KLEIN_BUCH_MAX_RESTORE_BYTES` (4 GiB) | umgesetzt (W7), dazu nur normale Pfadbestandteile |
| Restore durch eine fremde Sitzung | Re-Auth; Hinweis „danach gilt die Passphrase dieses Backups" | umgesetzt (W6/W7) |
| Schema-Konflikt | heute exakt gleiche Schema-Version; künftig `30 ≤ schema ≤ EXPECTED` mit Pflicht-Backup und Vorwärtsmigration, neuere Backups weiter abgelehnt | umgesetzt (W7); echte Vorwärtsmigration ab 0031 (W8) |

**Sidecar-Eingaben** (KoSIT, Mustang in der JVM)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Hängende oder ausufernde Prüfung präparierter XML/PDF | eigener Prozess, Timeout + `kill_on_drop` | umgesetzt |
| | höchstens 2 JVM-Starts gleichzeitig (`limits::JVM`) | umgesetzt (W2) |
| Mock-Modus im Produktivbetrieb | Release-Build mit gesetztem `KLEIN_BUCH_SIDECAR_MOCK` startet nicht | umgesetzt (W1) |
| Manipulierte Java-Komponenten, zu viel Speicher | Linux-Sidecar mit SHA-256 der Original-JARs in `sidecar/versions.json` (CI vergleicht mit den Quellen), `-Xmx512m`, Temp unter `/data/tmp` | umgesetzt (W11) |

Fehler in den Parsern der Drittkomponenten bleiben ein Restrisiko; begrenzt wird
es durch Prozessgrenze, Speicherlimit und den Nicht-root-Container.

**Logs**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Passphrase im Statement-Log (`PRAGMA key`, `ATTACH … KEY`) | Statement- und Slow-Statement-Logging auf allen verschlüsselten Verbindungen aus (`db::open_pool`, Snapshot, Verschlüsselungs-Migration) | umgesetzt (W2) |
| Geheimnisse über andere Wege | `sqlx::query=off` fest im Filter, Default `info`, Request-Log nur Methode/Pfad ohne Query/Status/Dauer, keine Header und Bodies, Empfänger und Dateinamen nur auf DEBUG; Log-Test auf Passphrase, TOTP, Setup-Token, Session-ID | umgesetzt (W5/W6) |
| Personenbezogene Daten im Audit | Fehlversuche nur als Logzeile `WARN auth.login.failed` (fail2ban-tauglich), nicht ins append-only `audit_log` | umgesetzt (W6) |

**Volumes** (`/data`, `/inputs`, optional `/backup-targets`)

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Lesen der Datenbank | SQLCipher (§2) | umgesetzt |
| Lesen der Belege | **Archivdateien (PDF/XML), Logo und Unterschrift liegen unverschlüsselt im Volume.** Schutz ist Sache des Betreibers (Host-Zugriff, Volume-Verschlüsselung). | bewusst akzeptiert |
| Löschen oder Ersetzen von Archivdateien | Erkennung statt Verhinderung, siehe „Write-once unter Linux" | umgesetzt (W2), Doku W3 |
| Temp-Dateien im RAM-tmpfs oder in fremden Verzeichnissen | DB-Snapshot unter `<data_dir>/tmp` | umgesetzt (W2) |
| | alle Temp-Dateien unter `/data/tmp`, `TMPDIR` im Image | geplant (W7/W11) |
| Start auf falschem oder leerem Volume, Klartext-DB | fail-closed vor dem Port-Bind: Mountpoints, Instanz-Marker, `chmod 0400`-Selbsttest im Archiv, Stichprobe der Archivpfade, Klartext-DB → Start verweigert | umgesetzt (W5; Stichprobe beim Entsperren, die DB ist vorher verschlüsselt) |
| Rechteausweitung im Container | `USER 65532`, read-only Root-FS, `cap_drop: [ALL]`, `no-new-privileges`, Core-Dumps aus, `PR_SET_DUMPABLE` 0 | umgesetzt (W11; Image-Smoke prüft UID, read-only Root-FS und `CapEff` 0) |

**Backups**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Lesen eines Backups | Argon2id + AES-256-GCM (§3.2); die DB im Content-ZIP ist zusätzlich SQLCipher-verschlüsselt | umgesetzt (W2) |
| Inkonsistenter oder überschriebener Stand | Snapshot per `VACUUM INTO`, Backups serialisiert, Dateinamen mit Millisekunden + Kurz-UUID | umgesetzt (W2) |
| Migration ohne Rückweg | Pflicht-Backup vor jeder Migration (Entsperren mit älterem Schema, Restore eines älteren Backups) | umgesetzt (W7) |
| Speicher | das Backup wird im RAM gebaut (etwa 3× Archivgröße) | Doku-Hinweis (W13) |

**Off-Site-Ziele**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Halbe Datei unter dem endgültigen Namen (Verzeichnis-Ziel) | Temp-Datei + `fsync` + `rename` + Verzeichnis-`fsync` | umgesetzt (W2) |
| Halbe Datei auf SFTP | Upload unter `.<name>.part`, dann `rename` | umgesetzt (W7) |
| Schreiben an beliebige Pfade | Verzeichnis-Ziele nur unter `/backup-targets` + `canonicalize`; mehrere Ziele; Status `offsite` = missing / failing / ok | umgesetzt (W7) |
| MITM zum SFTP-Server | Host-Key-Pinning (SHA-256, TOFU) | umgesetzt |
| SSRF und Abgreifen gespeicherter Zugangsdaten über Test-Verbindungen | Zugangsdaten an (Host, Port, Benutzer) gebunden; Tests gespeicherter Ziele nur per ID; Ad-hoc-Tests nur mit eingegebenem Passwort; Port-Allowlist; ≤ 10 Tests je Minute (instanzweit); einheitliche Fehlermeldung; Re-Auth | umgesetzt (W8) |
| Fremder Fingerprint beim Anlegen | `backup_target_add_sftp` übernimmt nur den Fingerprint der letzten Probe für (Host, Port), höchstens 10 min alt | umgesetzt (W8) |

**Lieferkette des Images**

| Risiko | Gegenmaßnahme | Status |
|---|---|---|
| Kompromittierte Abhängigkeit | `Cargo.lock` + `pnpm-lock.yaml`; Build mit `--locked` bzw. `--frozen-lockfile`, `cargo auditable` | umgesetzt (W11) |
| | `cargo deny`, `cargo audit`, `pnpm audit`, gitleaks, `trivy`-Gate, SBOM; Actions per Commit-SHA, Basis-Images per Digest; Renovate/Dependabot | geplant (W12) |
| Untergeschobenes Image | Build nur aus dem öffentlichen Repo, cosign-Signatur (keyless), feste Tags ohne `:latest`, Tags und Images nie löschen | geplant (W12) |
| Selbst-Update als Einfallstor | kein Update-Knopf in der App (bräuchte `docker.sock`, also praktisch root auf dem Host); opt-in Versionsprüfung ohne weitere Daten | Entscheidung E14, Umsetzung geplant (W12) |

**Passphrase im Speicher.** Nach dem Entsperren liegt die Passphrase im
Prozessspeicher (`BackupSession`). Die KDF-Pfade arbeiten mit `Zeroizing`-Kopien
(umgesetzt). „Datenbank sperren" (Pool zu, Scheduler gestoppt, Passphrase per
`zeroize` gelöscht, alle Sitzungen ungültig) ist geplant (W6). Nach einem
Neustart bleibt die Instanz gesperrt, bis der Nutzer im Browser entsperrt; ein
Auto-Unlock ist ausgeschlossen (E3).

### Restrisiken (bewusst akzeptiert)

- Wer den Host oder Code als Server-Benutzer kontrolliert, kann nach dem
  Entsperren alles: Passphrase lesen, Dateien löschen oder ersetzen. Klein.Buch
  erkennt Änderungen am Archiv, verhindert sie aber nicht.
- Archivdateien sind im Volume lesbar (siehe oben).
- Ohne korrekt gesetztes `KLEIN_BUCH_TRUSTED_PROXIES` sieht der Server nur die
  Adresse des Proxys; die Wartezeit je IP wirkt dann global.
- Wer Passphrase **und** zweiten Faktor hat, ist der Inhaber. Es gibt kein
  Rollenmodell.

---

## 1. Eine Geheimnis-Kette

Klein.Buch hat **eine** Passphrase. Sie ist gleichzeitig:

1. **App-Login.** Ohne sie wird kein Pool geöffnet. Im Server-Betrieb kommt ein
   zweiter Faktor (TOTP) dazu, und Entsperren der Datenbank und Anmelden einer
   Browser-Sitzung werden getrennt (geplant W6, Amendment ADR 0035).
2. **SQLCipher-Key-Source.** Aus ihr leitet SQLCipher per PBKDF2-HMAC-SHA512
   (Salt im DB-Header) den Page-Encryption-Key ab.
3. **Backup-Schlüssel-Wurzel.** Die Backup-Hülle wird mit einem
   Argon2id-abgeleiteten Key (m = 64 MiB, t = 3, p = 4) aus derselben Passphrase
   gebildet.

**Konsequenz.** Wer die Passphrase hat, hat die DB *und* die Backups. Wer sie
**verliert**, hat *nichts*. Es gibt **keinen Recovery-Backdoor**. Das ist
Absicht (ADR 0035): wer Belege jahrzehntelang aufbewahren will, muss seine
Passphrase ebenso lange sicher verwahren — typischerweise in einem Passwort-
Manager (das wird im Onboarding-Wizard empfohlen). Mindestlänge: 16 Zeichen,
geprüft im Frontend und im Backend.

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

Build-Tool-Pflicht am Host: **Perl** + (Windows-MSVC) **NASM**. Falls der
Exakt-Pin durch ein sqlx-Update bricht, muss `cargo tree -i libsqlite3-sys`
genau **eine** Version zeigen, sonst greift die Unification nicht.

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
   falsch. Genutzt wird das bei bereits offener DB: Bestätigung vor dem Factory
   Reset und der Klartext-Übergangspfad (§7). Im Server-Betrieb prüft die
   Re-Auth bei entsperrter DB auf diesem Weg, ohne einen zweiten Pool zu öffnen
   (geplant W6).

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

**Inhalt (Backup-Format 2, W1):**

```text
klein-buch.sqlite        ← SQLCipher-Snapshot
archive/<jahr>/<...>     ← write-once GoBD-Archiv (PDF/XML/Belege)
branding/<...>           ← Logo + Unterschrift aus <data_dir>/branding
```

Format 1 enthielt statt `branding/` den Ordner `inputs/branding`. Der Restore
nimmt Format 1 und 2 an; bei Format 1 bleiben Logo und Unterschrift unverändert,
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
4. `ciphertext = aes_256_gcm(key, nonce, content_zip)` inkl. 16-Byte-Tag.
5. Datei = Zeile 1 `KLEIN-BUCH-BACKUP-1`, Zeile 2 Manifest-JSON, danach der
   Ciphertext.

Das **Manifest** ist Klartext, damit der Restore-Wizard ohne Passphrase anzeigen
kann, was er einspielen würde: `formatVersion` (2), `schemaVersion`,
`appVersion`, `createdAt`, KDF-Parameter, Salt, Nonce, SHA-256 und Größe des
entschlüsselten Content-ZIP. Der Inhalt ist verschlüsselt und über das GCM-Tag
gegen Veränderung geschützt; nach dem Entschlüsseln wird zusätzlich der
Content-SHA-256 verglichen. Das Manifest selbst ist **nicht** Teil der
GCM-Authentisierung; maßgeblich für die Schema-Version ist nach dem Einspielen
der Schema-Check der DB selbst.

**Ablauf drumherum (W2):**

- Backups laufen **nacheinander** (prozessweiter Mutex in `create_now`):
  Auto-Backup nach dem Festschreiben, Tages-Backup im Scheduler-Tick, manuelles
  Backup, Pre-Restore-Backup — nie zwei gleichzeitig.
- Dateiname `klein-buch-<JJJJMMTT-hhmmssmmm>-<8 Hex>.kbk` (Europe/Berlin,
  Millisekunden + Kurz-UUID): zwei Backups in derselben Sekunde ergeben zwei
  Dateien.
- Jeder Schreibversuch auf ein Ziel landet im append-only `backup_log` (Erfolg
  und Fehlschlag, je Ziel) — nie mit Passphrase. Fehler **vor** dem Schreiben
  (z. B. beim Snapshot) stehen derzeit nicht im `backup_log`, sondern nur als
  Audit `backup.failed` (Auto-Backup) bzw. als Fehlermeldung (manuell); offener
  Punkt.

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
  `offsite` = missing / failing / ok (Übersicht, Einstellungen,
  Loopback-`/healthz`); ohne Ziel wöchentlich ein Inbox-Hinweis.
- **Schreiben ins Verzeichnis** (Floor und Off-Site, W2): Temp-Datei
  `.<name>.<uuid>.part` im Zielordner, `fsync`, `rename`, Verzeichnis-`fsync`.
  Ein Abbruch hinterlässt nie eine halbe `.kbk` unter dem endgültigen Namen.
- **Aufbewahrung:** Floor 7 tägliche / 3 monatliche / 1 jährliche, solange ein
  Off-Site-Ziel zuletzt erfolgreich war, sonst wie Off-Site 30 / 12 / 7 (§3.5);
  Off-Site je Ziel rotiert.
- **Tages-Backup** (W2): im Scheduler-Tick, sobald das letzte Backup älter als
  24 h ist — im laufenden Betrieb, nicht nur beim Entsperren. Nach einem
  Fehlschlag frühestens nach einer Stunde erneut.

**Kein** proprietäres Cloud-API; Cloud-Speicher nur über einen Ordner, den der
Betreiber als Volume unter `/backup-targets` einbindet. Die Auto-Erkennung von
OneDrive/iCloud ist entfallen (W7).

### 3.4 SFTP

`backup::sftp` benutzt `russh`/`russh-sftp`. Nur **Passwort-Auth** (kein
SSH-Key-Auth — vereinfacht das Onboarding für Nicht-Techniker). **Host-Key-
Pinning** über SHA-256 (Trust-On-First-Use): der Verbindungstest (`probe`)
liefert den Fingerprint (`SHA256:…`), der Nutzer bestätigt ihn, und er wird im
Ziel gespeichert (`app_settings.backup_targets`, Feld `host_fingerprint`). Ein
Upload ohne passenden Pin wird abgelehnt (Man-in-the-Middle-Schutz).
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

`backup::rotation::run(pool, backups_dir)` prunt pro Klasse (`daily` /
`monthly` / `yearly`) und Ablage (Floor bzw. Off-Site, zugeordnet über den
`target_path`). `manual`- und `pre_restore`-Backups werden **nie** automatisch
gelöscht. **Invariante** (G1-HARDEN.5): das **global neueste** Backup wird
**nie** gelöscht, auch wenn keep-Werte auf 0 stehen würden. Eine garantierte
Mindestzahl bleibt immer erhalten.

Test: die Unit-Tests in `src/backup/rotation.rs` (Abschnitt G1-HARDEN.5)
prüfen diese Invariante explizit.

---

## 4. SecretStore (Zugangsdaten in SQLCipher, seit W8)

Zugangsdaten liegen in der Tabelle `secrets` der verschlüsselten DB (Migration
`0031`, STRICT; E6, Amendment ADR 0011/0028). Den OS-Schlüsselbund der
Desktop-Fassung (`keyring`, `libdbus`) gibt es nicht mehr.

| Schlüssel | Inhalt |
|---|---|
| `smtp:{mail_account_id}` | SMTP-Passwort |
| `oauth:{mail_account_id}` | OAuth-Token-Bündel (nur Desktop-Bestand; im Server ohne Funktion, E9) |
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
- App-/Daten-Passphrase liegt **nicht** hier — sie kommt vom Benutzer.
- Kein Übernehmen alter Schlüsselbund-Einträge aus der Desktop-Fassung (E17).

Test: `tests/server_secrets_test.rs` (Klartext weder in DB-Datei, Audit,
Migrations- noch DSGVO-ZIP; Bindung; Port-Allowlist; Kontingent; SFTP gegen
einen Testserver auf `127.0.0.1` mit Fingerprint).

---

## 5. Bootstrap-Reihenfolge

Aus `../ARCHITECTURE.md` §3 — hier mit Sicherheits-Blickwinkel (Desktop-Stand;
Server siehe unten):

1. **Setup-Closure.** Nur App-State + `prepare_filesystem`. **Kein**
   Pool-Open. Wer hier den DB-Pool öffnen würde, ohne Passphrase, kriegt
   eine wertlose Datei oder einen Fehler.
2. **`prepare_filesystem`.** Legt `data_dir` an, kopiert fehlende
   `inputs/`-Dateien (ohne zu überschreiben) und arbeitet dann vorgemerkte
   Marker ab: (a) **Factory-Reset-Marker** `FACTORY_RESET_PENDING.json` —
   `data_dir` leeren, früher Rücksprung in den Onboarding-Zustand; (b)
   **Restore** — erst Reste eines unterbrochenen Tauschs heilen, dann einen
   vorgemerkten Restore (`RESTORE_PENDING.json`) anwenden (§6). `archive/` und
   `backups/` werden erst **danach** angelegt: ein leeres neues `archive/` neben
   `archive.rollback` sah für die Wiederherstellung früher wie „Tausch fertig"
   aus.
3. **Frontend** prüft `backup_needs_onboarding`. Keine DB ⇒ Onboarding.
   Verschlüsselte DB ⇒ Entsperren. Klartext-DB (Übergang, §7) ⇒ Onboarding nur,
   wenn dort noch keine Passphrase eingerichtet ist, sonst Entsperren.
4. **Onboarding** setzt die Passphrase, legt die DB verschlüsselt an (oder
   migriert eine Klartext-DB, §7), schreibt den Verifier (§2.3) und erstellt
   sofort das erste Backup.
5. **Entsperren** öffnet den Pool mit der Passphrase, prüft per Probe-Read,
   wendet Migrationen an (Schema-Check), schreibt einen gepufferten
   Restore-Audit und übergibt die Passphrase an die `BackupSession`
   (Prozessspeicher).
6. **Scheduler-Start** danach; das fällige Tages-Backup legt der erste Tick an.

**Solange ein Restore offen ist** — Marker, Zustandsdatei
`RESTORE_SWAP_STATE.json` oder `*.rollback`-Reste vorhanden — lehnen Onboarding
und Entsperren ab (`restore::restore_incomplete`). Sonst entstünde neben der
beiseitegelegten DB eine neue.

**Was diese Reihenfolge schützt:** ohne Schritt 4 oder 5 gibt es nichts im
Pool-State. Jeder Command, der den Pool braucht, scheitert sauber an „Pool
nicht da" (übersetzt im UI als „App noch nicht entsperrt").

**Server-Betrieb (geplant W5/W6).** Zustandsautomat `setup` (leeres `/data`) →
`locked` (kein Pool, kein Scheduler) ↔ `unlocked`; Browser-Sitzungen sind davon
getrennt. `prepare_filesystem` läuft synchron **vor** dem Port-Bind; die
fail-closed-Prüfungen aus dem Threat Model verweigern den Start. Nach jedem
Neustart ist die Instanz gesperrt, bis der Nutzer entsperrt (E3).

---

## 6. Race-Freiheit durch Zwei-Phasen-Pattern

Sowohl **Restore** als auch **Factory Reset** machen destruktive FS-Ops
(DB-Datei ersetzen, Archiv ersetzen, `data_dir` leeren). Im **Live-Command**
ist der Pool offen — wir können da nicht einfach die Datei ersetzen
(Windows-File-Lock, SQLite-WAL-State). Lösung: zwei Phasen, verbunden über eine
Marker-Datei im Dateisystem (nicht in der DB, die ja ausgetauscht wird).

### Phase A (Live)

- **Restore** (`backup::restore::apply`, W7; Quelle Upload oder lokales Backup
  aus der Historie). Format prüfen (1–2), Schema `30 ≤ s ≤ EXPECTED`, Backup
  entschlüsseln + Content-Hash prüfen (unter der `KDF`-Semaphore; falsche
  Passphrase schreibt nichts), Pflicht-Audit `backup.restore.requested`, dann
  **Pre-Restore-Backup** mit der *aktuellen* Passphrase (enthält den Audit),
  Platz prüfen, gehärtet ins Staging `restore-staging/` entpacken (Allowlist,
  nur normale Pfadbestandteile, Größen- und Verhältnisgrenzen, keine Symlinks),
  gestagte DB mit der Backup-Passphrase öffnen (Schema, Archivpfade,
  Migrations-Prüfsummen), Marker per `rename`. Der Tausch beim Neustart legt
  `RESTORE_APPLIED.json` an; das erste Entsperren schreibt
  `backup.restore.applied` und trägt das Pre-Restore-Backup in die Historie ein.
- **Factory Reset** (nur Desktop). Gating durchlaufen (Export-First-Pflicht
  etc.), Passphrase-Verify, **Marker schreiben** (`FACTORY_RESET_PENDING.json`),
  `AppHandle::restart()`.

In Phase A wird der Pool nicht geschlossen. Phase A endet mit einem Neustart
(Restore: vom Nutzer ausgelöst; Factory Reset: `AppHandle::restart()`); der
gesamte Prozess-State verschwindet. Im Server beendet sich der Prozess nach dem
Restore geordnet, sobald laufende Jobs fertig sind, und die Container-Verwaltung
startet ihn neu (geplant W5).

### Phase B (nächster Start, in `prepare_filesystem`, vor dem Pool-Open)

**Restore** (`restore::apply_pending`) tauscht DB, `archive/` und ab Format 2
`branding/` in drei Stufen. Den Fortschritt hält die Zustandsdatei
`RESTORE_SWAP_STATE.json` fest (W2), jeweils atomar geschrieben (Temp-Datei,
`fsync`, `rename`, Verzeichnis-`fsync`):

1. Zustand `moving`. WAL/SHM der alten DB entfernen; die Originale per `rename`
   beiseitelegen (`klein-buch.sqlite.rollback`, `archive.rollback`,
   `branding.rollback`). Scheitert ein Schritt, werden die schon verschobenen
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

Der Audit `backup.restore.applied` wird nach dem Pool-Open geschrieben
(`PendingRestoreAudit`). Scheitert er, schlägt das Entsperren fehl, und der
Eintrag bleibt für den nächsten Versuch vorgemerkt.

**Factory Reset** (nur Desktop): Datei `FACTORY_RESET_PENDING.json` ⇒
`data_dir`-Inhalt leeren, Kern-Verzeichnisse leer neu anlegen,
Keychain-Wipe best-effort. **Off-Site-Backups** in Cloud/SFTP bleiben
unangetastet (bewusste Architektur-Entscheidung — GoBD und Geräte-Weitergabe
trennen). Im Server-Modus ist der Factory Reset abgeschaltet (E7, Amendment
ADR 0036); der Code entfällt in W11.

**Tests** (G1-HARDEN.1, G1-RESET-Tests, Review W2): Roundtrip mit Crash
zwischen Phase A und Phase B (Marker bleibt, Phase B greift beim nächsten
Start); gezielt injizierte Fehler in Stufe 1 und Stufe 2 hinterlassen die
Originale; nach Backup → Restore hat keine Datei unter `archive/` ein
Schreibbit.

---

## 7. Encryption-Migration (Klartext → SQLCipher)

Eine Installation, die vor G1-ENC angelegt wurde, hat eine **Klartext-DB**
(erkannt an `db::db_file_is_plaintext`: die Datei beginnt mit
`SQLite format 3\0`; SQLCipher verschlüsselt auch den Header). Sie wird beim
Onboarding (Klartext-DB ohne Passphrase) bzw. beim ersten Entsperren (Klartext-DB
mit Verifier; Passphrase per `backup::verify_passphrase` geprüft) einmalig
verschlüsselt — `db::migrate_plaintext_to_encrypted`:

1. **Pflicht-Pre-Migration-Backup** (`create_now`, Auslöser `pre_restore`,
   wird nie rotiert) mit der gerade bestätigten Passphrase. Scheitert es, wird
   **nichts** verschlüsselt; die Klartext-DB bleibt unverändert.
2. Audit `encryption.migration.started` noch in die Klartext-DB — er wandert mit
   in den Export, der Audit-Trail bleibt lückenlos.
3. Pool schließen, dann `db::encrypt_db_file_in_place`:
   - Reste eines früheren Laufs (`<db>.enc-migrating` samt `-wal`/`-shm`)
     entfernen.
   - Eigene Einzelverbindung auf die Klartext-Quelle (Statement-Logging aus),
     `PRAGMA wal_checkpoint(TRUNCATE)`, `ATTACH DATABASE '<db>.enc-migrating'
     AS encrypted KEY …`, `SELECT sqlcipher_export('encrypted')`, `DETACH`.
   - **Probe-Lesen** der Kopie mit der Passphrase. Scheitert es, wird die
     Kopie gelöscht; die Klartext-DB bleibt unverändert.
   - WAL/SHM von Kopie und Quelle entfernen (sonst würde SQLite die
     Klartext-WAL auf die verschlüsselte Datei anwenden).
   - **Atomarer `rename`** der Kopie auf den DB-Pfad. Neben der DB bleibt keine
     Klartext-Kopie zurück.
4. Pool mit Key neu öffnen, Migrationen + Schema-Check, Audit
   `encryption.migration.applied`.

Der Rückweg ist das Pre-Migration-Backup (verschlüsselte `.kbk`), nicht eine
Datei neben der DB.

**Server-Betrieb:** Es gibt keinen Desktop-Bestand zu übernehmen (E17). Der
Klartext-Übergangspfad wird entfernt (geplant W6); eine Klartext-DB lässt den
Server nicht starten (geplant W5).

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
- **OAuth/Graph** läuft über `oauth2` mit **PKCE-S256**, Loopback-Capture
  des Redirects auf `127.0.0.1:<random>`. Refresh-Token in `secrets` (Desktop: Keychain),
  Access-Token nur im Memory. **Im Server-Modus ausgeblendet** (E9; Commands
  gesperrt ab W5, aus der Oberfläche entfernt und neue Microsoft-Konten
  abgelehnt ab W8).
- Jeder Versand-Versuch (erfolgreich oder nicht) landet im append-only
  `email_log`. Provider-Antwort wird kanonisiert (Header-Stripping) und
  gespeichert — kein OAuth-Token darin.

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
  Server-Betrieb ist TOTP der zweite Faktor (geplant W6); die Passphrase gehört
  in einen guten Passwort-Manager.
- **Physische Unveränderbarkeit des Archivs (WORM).** Siehe „Write-once unter
  Linux".

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
   (30 tägliche / 12 monatliche / 7 jährliche). Eine veränderte oder gelöschte
   Datei lässt sich aus einem älteren Backup zurückholen.
5. **Snapshots des Dateisystems beim Betreiber** — schreibgeschützte Snapshots
   des Daten-Volumes (z. B. ZFS oder Btrfs) liegen außerhalb der Reichweite des
   Server-Benutzers. Das ist Betriebsaufgabe und wird im Betriebshandbuch
   empfohlen (W13).

Ergänzend (geplant): Selbsttest beim Start (`chmod 0400` im Archiv anlegen und
prüfen; scheitert er, etwa wegen NFSv4-ACLs, startet der Server nicht, W5) und
der Hinweis, DB und Archiv nicht auf NFS/SMB zu legen (W13).

**Was bleibt.** Wer Code als Server-Benutzer ausführt, kann Archivdateien löschen
oder ersetzen. Klein.Buch bemerkt das spätestens beim nächsten Lesen oder
Integritätslauf; zurückholen lässt es sich nur aus Backups oder Snapshots, die
dieser Benutzer nicht erreicht.

---

## Letzte Verifikation

Stand: 2026-10-24 (Phase W nach W8, Version 2026.5.1, Schema v31), ADRs
0009/0034/0035/0036/0039. Quelle: `src/backup/` (`mod.rs`, `snapshot.rs`,
`restore.rs`, `target.rs`, `sftp.rs`, `manifest.rs`, `encrypt.rs`,
`rotation.rs`), `src/db/mod.rs` (Pool-Open, Bootstrap, Verschlüsselungs-
Migration), `src/archive/store.rs`, `src/archive/integrity_check.rs`,
`src/scheduler/integrity_check_cron.rs`, `src/limits.rs`, `src/locks.rs`,
`src/secrets.rs`, `src/nettest.rs`, `src/mail/smtp.rs`, Cargo.toml (libsqlite3-sys-Pin);
Planwerte aus dem Arbeitsplan Phase W (Blöcke W5–W13).
