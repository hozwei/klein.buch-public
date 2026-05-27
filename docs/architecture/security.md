# Sicherheits-Modell

> Vertiefung zu „Backup" und „Verschlüsselung at Rest" in
> `../ARCHITECTURE.md` §7. Wie die Geheimnis-Kette gebaut ist, wo welche
> Geheimnisse leben, wie Bootstrap und Restore und Factory Reset
> race-frei zueinander passen.

ADR-Basis: 0035 (Verschlüsselung at Rest), 0034 (Backup-Ziele/Retention/Log),
0036 (Factory Reset), 0009 (Backup als First-Class), 0011 + 0028
(Mail-Credentials im Keychain).

---

## 1. Eine Geheimnis-Kette

Klein.Buch hat **eine** Passphrase. Sie ist gleichzeitig:

1. **App-Login.** Beim Start wird sie verlangt, sonst kein Pool-Open.
2. **SQLCipher-Key-Source.** Aus ihr leitet SQLCipher per PBKDF2-HMAC-SHA512
   (Salt im DB-Header) den Page-Encryption-Key ab.
3. **Backup-Schlüssel-Wurzel.** Die Backup-Hülle wird mit einem
   Argon2id-abgeleiteten Key (m=64 MB, t=3, p=4) aus derselben Passphrase
   gebildet.

**Konsequenz.** Wer die Passphrase hat, hat die DB *und* die Backups. Wer sie
**verliert**, hat *nichts*. Es gibt **keinen Recovery-Backdoor**. Das ist
Absicht (ADR 0035): wer Belege jahrzehntelang aufbewahren will, muss seine
Passphrase ebenso lange sicher verwahren — typischerweise in einem Passwort-
Manager (das wird im Onboarding-Wizard empfohlen).

**Warum zwei verschiedene KDFs?**

- **SQLCipher's eigene PBKDF2-HMAC-SHA512** ist im DB-Format eingebaut, mit
  Salt im DB-Header. Das macht die DB-Datei selbstbeschreibend (jede DB-Datei
  enthält ihr eigenes Salt) und cross-OS-portabel. Argon2id-Raw-Key als
  SQLCipher-Schlüssel haben wir bewusst nicht genommen — der Cargo-Vorteil
  (FIPS-Argon2id-fester Key) wäre durch den Verlust der Selbstbeschreibung
  zunichte gemacht (ADR-0035-Amendment 2026-05-24).
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
    → SqliteConnectOptions::new()
        .filename(path)
        .pragma("key", key)
        .pragma("foreign_keys", "ON");
```

Der `PRAGMA key`-Befehl wird beim ersten Connect ausgeführt und entschlüsselt
die DB seitenweise. Ohne Passphrase ist die Datei eine Zufallszahl. Mit
falscher Passphrase liefert der nächste SELECT einen `file is not a database`-
Fehler — sauber abfangbar.

### 2.3 Verifier-Datei

Klein.Buch persistiert **keinen** Hash der Passphrase irgendwo. Stattdessen
gibt es eine **Verifier-Tabelle** in der DB (`app_settings.key_verifier`),
die beim Setup mit einem bekannten Klartext geschrieben wird (z. B. einem
fixen Tag). Beim Unlock-Versuch wird die DB mit der eingegebenen Passphrase
geöffnet und der Verifier ausgelesen. Stimmt er, ist die Passphrase richtig;
stimmt er nicht oder ist der Eintrag unleserlich, ist sie falsch.

**Vorteil:** der Verifier liegt selbst verschlüsselt in der DB; wer ihn aus
einer kopierten DB-Datei rausholen will, braucht schon die Passphrase. Es
gibt also kein offline-rate-bares Geheimnis außerhalb der DB.

---

## 3. Die Backup-Hülle

### 3.1 Snapshot

`backup::snapshot::create(db_path, target_path)` kopiert die SQLCipher-DB-
Datei **as-is** in einen Snapshot. Inkl. Header-Salt. Der Snapshot ist also
weiterhin verschlüsselt — er wird in derselben Geheimnis-Kette nochmal
„umverpackt".

Implementation: `VACUUM INTO 'target_path'` über die Live-DB (das ist
SQLCipher-aware und respektiert WAL-State), oder als Fallback Datei-Kopie
unter Read-Lock.

### 3.2 Hülle

`backup::encrypt::wrap(snapshot_bytes, manifest, passphrase)`:

1. Random-Salt (16 Byte) für Argon2id.
2. Random-Nonce (12 Byte) für AES-GCM.
3. `key = argon2id(passphrase, salt, m=64MB, t=3, p=4)` → 32 Byte.
4. `nonce` + `salt` + `aes_gcm(key, nonce, plaintext = snapshot || manifest_json)`
   → Container.
5. Manifest separat vorhängen (lesbar, beschreibt Schema-Version + App-Version
   + Hash des Plaintexts).

Container-Format ist versioniert (`Magic + Version + Salt + Nonce + Length +
Ciphertext + Tag`). Header sind klar lesbar, der Inhalt nicht.

### 3.3 Ziele

`backup::target::resolve_target(settings)` liefert mindestens den **Floor**
(lokal, `%APPDATA%\…\backups\`, immer aktiv, 7 Tages/3 Wochen/1 Monat) und
nach Möglichkeit ein **Off-Site-Ziel** (Verzeichnis-Mount **oder** SFTP,
30/12/7 Aufbewahrung). „Immer zweifach" ist best-effort: wenn das Off-Site-
Ziel temporär nicht erreichbar ist, gibt es ein `backup_log`-Eintrag mit
`status='failed'` und eine Notification, der Floor läuft trotzdem.

Auto-Detect (G1-BKP.2): OneDrive (Windows) und iCloud (macOS) als Cloud-
Ordner erkannt; manuelle Wahl als Fallback (Linux/Rest). **Kein**
proprietäres Cloud-API; alles über lokale Ordner, die ein OS-Sync-Client
hochlädt.

### 3.4 SFTP

`backup::sftp` benutzt `russh`/`russh-sftp`. Nur **Passwort-Auth** (kein
SSH-Key-Auth — vereinfacht das Onboarding für Nicht-Techniker). **Host-Key-
Pinning** über SHA-256 TOFU (Trust-On-First-Use): beim ersten Verbinden
wird der Host-Key gemerkt; jede spätere Abweichung führt zum Abbruch
(Man-in-the-Middle-Schutz). Pin steht in `app_settings.sftp_host_key_sha256`.

Passwort liegt im **OS-Keychain**, Referenz steht in den Backup-Settings.

### 3.5 Rotation

`backup::rotation::prune(target, keep_n)` läuft pro Klasse (`daily` /
`weekly` / `monthly`). **Invariante** (G1-HARDEN.5): das **global neueste**
Backup wird **nie** gelöscht, auch wenn keep-Werte auf 0 stehen würden. Eine
garantierte Mindestzahl bleibt immer erhalten.

Test: `tests/backup_rotation_test.rs` prüft diese Invariante explizit.

---

## 4. Keychain-Topologie

Was im OS-Keychain liegt (`keyring`-Crate, native Backends — `apple-native`/
`windows-native`/`sync-secret-service`):

| Service | Key | Inhalt |
|---|---|---|
| `de.wildbach.kleinbuch` | `smtp:{mail_account_id}` | SMTP-Passphrase |
| `de.wildbach.kleinbuch` | `oauth:{mail_account_id}:refresh:0..N` | Refresh-Token, gechunkt (Windows-2560-Byte-Limit pro Eintrag) |
| `de.wildbach.kleinbuch` | `sftp_backup:{settings_id}` | SFTP-Passwort |

**Disziplin.**

- Access-Token (OAuth) liegen **nie** im Keychain — sie werden bei Bedarf
  aus dem Refresh-Token frisch geholt und im RAM gehalten.
- App-/Daten-Passphrase liegt **nicht** im Keychain — sie kommt vom Benutzer.
- In Tests (`#[cfg(test)]`) ist der Mock-Store aktiv; **Produktion** hat
  keinen In-Memory-Fallback (G1-HARDEN.3).

**Factory Reset** wischt die Keychain-Einträge best-effort (`backup::
factory_reset::apply_pending` ruft `keyring::Entry::delete_password` für jeden
bekannten Service-Key). „Best-effort" heißt: Fehler beim Wipen blockieren den
Reset nicht (der wäre sonst nicht durchzubekommen, wenn das OS-Keychain
gerade meckert).

---

## 5. Bootstrap-Reihenfolge

Aus `../ARCHITECTURE.md` §3 — hier mit Sicherheits-Blickwinkel:

1. **Setup-Closure.** Nur Tauri-State + `prepare_filesystem`. **Kein**
   Pool-Open. Wer hier den DB-Pool öffnen würde, ohne Passphrase, kriegt
   eine wertlose Datei oder einen Fehler.
2. **`prepare_filesystem`.** Macht zwei sicherheits-relevante Dinge: (a)
   vorgemerkte **Restore-Marker** abarbeiten — DB-Datei + Archiv swappen,
   bevor der Pool jemand sieht; (b) vorgemerkte **Factory-Reset-Marker**
   abarbeiten — `data_dir` nuken, Kern-Verzeichnisse leer neu anlegen. Beides
   in Phase B (siehe §6).
3. **Frontend** prüft `backup_needs_onboarding`. Klartext-DB ohne Verifier
   ⇒ Onboarding. Verschlüsselte DB ⇒ Unlock-Screen. Keine DB ⇒ Onboarding.
4. **Onboarding** setzt Passphrase, schreibt Verifier, baut DB an oder
   migriert eine bestehende Klartext-DB (siehe §7).
5. **Unlock** öffnet den Pool, prüft Verifier, übergibt die Passphrase in
   die `BackupSession` (Memory).
6. **Migrations + Scheduler-Start** danach, in der entsperrten DB.

**Was diese Reihenfolge schützt:** ohne Schritt 4 oder 5 gibt es nichts im
Pool-State. Jeder Command, der den Pool braucht, scheitert sauber an „Pool
nicht da" (übersetzt im UI als „App noch nicht entsperrt").

---

## 6. Race-Freiheit durch Zwei-Phasen-Pattern

Sowohl **Restore** als auch **Factory Reset** machen destruktive FS-Ops
(DB-Datei ersetzen, Archiv ersetzen, `data_dir` nuken). Im **Live-Command**
ist der Pool offen — wir können da nicht einfach die Datei ersetzen
(Windows-File-Lock, sqlite-WAL-State). Lösung:

### Phase A (Live)

- **Restore.** Manifest lesen, Hash-Verify, Schema-Check, Pre-Restore-Backup,
  **Marker schreiben** (eine `.pending_restore`-Datei im `data_dir` mit dem
  Pfad des entpackten Snapshots), `AppHandle::restart()`.
- **Factory Reset.** Gating durchlaufen (Export-First-Pflicht etc.),
  Passphrase-Verify, **Marker schreiben** (`.pending_factory_reset`),
  `AppHandle::restart()`.

In keiner der beiden Phasen wird der Pool geschlossen. Phase A endet mit
einem Restart, der gesamte Prozess-State verschwindet.

### Phase B (nächster App-Start, in `prepare_filesystem`)

- Datei `.pending_restore` ⇒ DB-Datei und Archiv atomar swappen
  (`rename`-basiert), `PendingRestoreAudit` mit dem Audit-Event befüllen
  (wird nach Pool-Open ins `audit_log` geschrieben).
- Datei `.pending_factory_reset` ⇒ `data_dir`-Inhalt nuken, Kern-Verzeichnisse
  leer neu anlegen, Keychain-Wipe-Best-Effort. **Off-Site-Backups** in Cloud/
  SFTP bleiben unangetastet (das ist eine bewusste Architektur-Entscheidung —
  GoBD und Geräte-Weitergabe trennen).

Phase B läuft **vor** dem Pool-Open. Wenn dort etwas fehlschlägt, ist die
DB-Datei in einem definierten Zustand (entweder noch alt oder schon neu, nie
halb).

**Tests** (G1-HARDEN.1, G1-RESET-Tests): Roundtrip mit Crash zwischen Phase
A und Phase B funktioniert (Marker bleibt, Phase B greift beim nächsten
Start). Crash mitten in Phase B macht keinen halben Swap (atomic-rename
oder Failure).

---

## 7. Encryption-Migration (Klartext → SQLCipher)

Eine Installation, die vor G1-ENC angelegt wurde, hat eine **Klartext-DB**.
Beim ersten Start mit der neuen App-Version läuft folgendes:

1. `db::prepare_filesystem` sieht eine DB-Datei und keinen Verifier
   (Klartext-DB hat den `key_verifier`-Eintrag nicht oder lesbar im
   Klartext).
2. Onboarding-Screen lädt mit Hinweis „Wir richten ab jetzt ein App-Passwort
   ein, das gleichzeitig Ihre Daten verschlüsselt. **Pflicht-Pre-Migration-
   Backup wird erstellt**, bevor wir die DB neu schreiben."
3. Passphrase wird gesetzt.
4. `backup::encrypt::migrate_plaintext_to_encrypted(old_path, passphrase)`:
   - **Pre-Migration-Backup** anlegen (Klartext-DB als verschlüsselte Backup-
     Hülle in Floor + Off-Site).
   - SQLCipher öffnet eine neue verschlüsselte DB, `sqlcipher_export` aus der
     alten Klartext-DB.
   - **Probe-Lese** der neuen DB (öffnen + select count(*) auf jeder Kern-
     Tabelle).
   - Bei Erfolg: atomarer Swap (`rename` von alt nach `.bak` und neu nach
     final-Pfad). Bei jedem Fehler bleibt die Klartext-DB intakt.
5. Audit-Eintrag `db.encryption_migrated`.

Damit ist der Übergang reversibel, solange das `.bak` noch da ist. Nach
manueller Bestätigung („Migration ok") wird `.bak` gelöscht.

---

## 8. Tamper vs. Waisen

`scheduler::integrity_check_cron` läuft monatlich und unterscheidet zwei
Befunde am Archiv:

- **Tamper.** Datei existiert, aber SHA-256 stimmt nicht mit
  `archive_entries.hash` überein. Audit-Event `archive.integrity_tamper`,
  `archive_integrity_checks.tamper_archive_ids` += diese ID.
- **Waisen** (G1-HARDEN.4). Datei existiert nicht mehr, aber
  `archive_entries`-Eintrag schon. Audit-Event `archive.integrity_missing`,
  `files_missing` += 1, `missing_archive_ids` += diese ID.

Eine Waise ist kein Tamper — sie kann auch eine schiefgegangene Sync,
ein gelöschtes externes Backup-Volume oder ein verschobenes `archive/`
sein. Sie ist trotzdem ein Problem, weil der Beleg dann nicht mehr lesbar
ist; aber sie ist nicht „jemand hat die Datei manipuliert".

Cron-Notification meldet beides getrennt: „X Manipulationen, Y verlorene
Belege". `files_failed` (Summe der Probleme) bleibt ehrlich und ist die
Headline-Zahl.

---

## 9. Versand-Sicherheit

- **SMTP** läuft über `lettre` async mit **TLS oder STARTTLS** (kein Plain-
  SMTP). Passphrase aus dem Keychain, Audit-Eintrag `mail.sent` ohne
  Passphrase.
- **OAuth/Graph** läuft über `oauth2` mit **PKCE-S256**, Loopback-Capture
  des Redirects auf `127.0.0.1:<random>`. Refresh-Token ins Keychain gechunkt,
  Access-Token nur im Memory.
- Jeder Versand-Versuch (erfolgreich oder nicht) landet im append-only
  `email_log`. Provider-Antwort wird kanonisiert (Header-Stripping) und
  gespeichert — kein OAuth-Token darin.

---

## 10. Was nicht im Scope ist

- **Multi-User / Rollen.** Klein.Buch ist Single-User. Jeder, der die
  Passphrase hat, ist der Inhaber. Wer mehrere Personen Zugriff geben will,
  muss das auf OS-Ebene lösen (Mehrbenutzer-Konten, getrennte Passphrasen
  pro Profil).
- **Cloud-Hosted-Variante.** v1.0 ist local-only. Eine Hosted-Variante
  bräuchte DSGVO-AVV, Server-Backup-Konzept, ISO-27001-Spuren — das ist
  Post-v1.0, eigene Produkt-Linie.
- **Anti-Forensik / Plausible Deniability.** Wer eine verschlüsselte DB
  hat, hat eine. Klein.Buch behauptet nicht, dass die *Existenz* der DB
  verborgen wäre.
- **Hardware-Token.** Yubikey/Smartcard-Auth ist nicht vorgesehen. Passphrase
  in einem guten Passwort-Manager ist die empfohlene Praxis.

---

## Letzte Verifikation

Stand: 2026-05-26, ADRs 0009/0034/0035/0036. Quelle: `src/backup/`,
`src/db/` (Pool-Open + Bootstrap), `src/mail/keyring.rs`, Cargo.toml
(libsqlite3-sys-Pin).
