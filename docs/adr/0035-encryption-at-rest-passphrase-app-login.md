# ADR 0035 — Verschlüsselung at Rest + Passphrase = App-Login

**Status:** Akzeptiert · 2026-05-24 · v1.0 (Umsetzung im Security-Block G1). Größter Einzel-Eingriff der Security-Säule.

## Kontext

Die Live-SQLite-DB liegt heute **im Klartext** auf der Platte — verschlüsselt sind
nur die Backup-ZIPs (ADR 0009). Klein.Buch verwaltet **Steuerdaten** (sensibel,
DSGVO-relevant). Anforderung Manuel: **„Kein Passwort eingegeben → kein Zugriff auf
die App."** Eine reine Zugangs-Sperre (Lock-Screen) wäre Fassade: wer die DB-Datei
kopiert (Backup-Ordner, Cloud-Sync, geklauter Laptop ohne Disk-Crypto), liest alles.

## Entscheidung

1. **Verschlüsselung at Rest via SQLCipher.** Die DB-Datei wird seitenweise
   AES-verschlüsselt; geöffnet nur mit `PRAGMA key` aus dem abgeleiteten Schlüssel.
   Ohne korrekte Passphrase ist die Datei **unlesbar** — „kein Zugriff" wird damit *wahr*.
2. **Eine Geheimnis-Kette.** Die **Passphrase = App-Login = DB-Key = Backup-Key**.
   Schlüsselableitung über Argon2id (gleiche KDF-Familie + Parameter wie die
   Backup-Krypto, ADR 0009). Es gibt nur **ein** Geheimnis, das der Nutzer kennt.
3. **Bootstrap-Reihenfolge dreht sich:** Passphrase-Abfrage **vor** dem DB-Open;
   `PRAGMA key` setzen; danach Schema-Migrationen in der entschlüsselten DB. Kein
   DB-Zugriff (lesend oder schreibend) ohne entsperrte Passphrase.
4. **Onboarding** erzwingt das Passphrase-Setup (war teils da). **Passphrase-Verlust
   = Totalverlust** (DB **und** Backups) — **by design**, kein Recovery-Backdoor.
   Drastischer, unübersehbarer Warnhinweis beim Setup + Empfehlung Passwort-Manager.
5. **Bestands-Migration** (vorhandene Klartext-DB → verschlüsselt): einmaliger
   Bootstrap-Schritt beim ersten Start der verschlüsselnden Version
   (`sqlcipher_export` in eine neue verschlüsselte Datei + Atomic-Swap), **mit
   Pre-Migration-Backup**. Kein normales SQL-Migrations-File.
6. **Passphrase bleibt** niemals in DB/Logs/`audit_log`/`backup_log` — lebt nur im
   Prozess-Speicher (bestehende `BackupSession` wird zur App-Session).

## Konsequenzen

- „Kein Passwort → kein Zugriff" ist echt, auch bei Dateizugriff/Diebstahl;
  OS-Full-Disk-Crypto wird nicht vorausgesetzt.
- Die OneDrive-/WAL-Sorge ist endgültig erledigt (selbst eine kopierte/synchronisierte
  DB-Datei ist wertlos ohne Passphrase).
- Security-Säule wird in sich schlüssig: DB, Backups und Zugang teilen ein Geheimnis.
- **Risiko (bewusst akzeptiert):** vergessene Passphrase = alle Steuerdaten weg.
  Für Laien real — daher harter Onboarding-Warnhinweis Pflicht (RELEASE-1.0-GUIDE §G1).
- Build-Dependency: `libsqlite3-sys`/`bundled-sqlcipher` (bzw. `rusqlite`+`sqlcipher`);
  Pool-Open-Pfad anzupassen. Performance bei Single-User-MB-DB vernachlässigbar.

## Alternativen

| Option | Contra |
|---|---|
| **A — Lock-Screen** (Gate, DB bleibt Klartext) | Fassade: DB-Datei kopierbar + lesbar → falsches Sicherheitsgefühl bei Steuerdaten |
| OS-Full-Disk-Crypto (BitLocker/FileVault) | nicht garantiert aktiv, nicht portabel, schützt nicht die einzelne Datei in der Cloud |
| App-verwalteter Key in Datei/Keychain ohne Nutzer-Passphrase | kein „kein Passwort kein Zugriff"; Key-Diebstahl = Klartext |
| Nur Feld-Verschlüsselung einzelner Spalten | Teilschutz, komplex, Metadaten/Indizes bleiben lesbar |

## Referenzen

`db::mod` (Pool-Open + `PRAGMA key`), `backup/encrypt.rs` (KDF wiederverwenden),
`backup/mod.rs` (`BackupSession` → App-Session), Onboarding/`BackupGate.svelte`;
ADR 0009 (Backup-Krypto), ADR 0034 (Backup-Ziele). RELEASE-1.0-GUIDE.md §G1.

## Amendment 2026-05-24 — KDF des DB-Layers: SQLCipher-nativ statt Argon2id-Raw-Key

**Status:** Akzeptiert (Manuels Delegation: „sicher, cross-OS, passt zum Programm —
entscheide du"). Verfeinert Entscheidung Pt. 2 für den **DB-at-Rest-Schlüssel**.
ADR-0009-Backup-Krypto (Argon2id) bleibt **unverändert**.

**Entscheidung.** Der DB-Seitenschlüssel wird **nicht** per Argon2id als Raw-Key
abgeleitet, sondern die Nutzer-Passphrase geht direkt über `PRAGMA key='…'` an
SQLCipher; SQLCipher leitet den Schlüssel selbst ab (PBKDF2-HMAC-SHA512,
SQLCipher-4-Default 256 000 Iterationen). Der **KDF-Salt liegt im DB-Header** →
die verschlüsselte Datei ist **selbstbeschreibend**.

**Warum (entscheidend: keine neue Totalverlust-Fläche).** Ein Argon2id-Raw-Key
bräuchte einen **extern** gespeicherten Salt (Chicken-and-Egg: der Salt wird vor
dem DB-Open gebraucht, kann also nicht in der verschlüsselten DB liegen). Eine
solche Salt-Sidecar-Datei müsste zwingend synchron mit **jedem** Backup mitwandern;
geht sie verloren oder läuft sie auseinander, ist die DB **trotz korrekter
Passphrase** unwiederbringlich. Für eine 10-Jahre-Aufbewahrungs-App (GoBD/AO) ist
das inakzeptabel. SQLCipher-nativ vermeidet das: ein File, das man Win→Mac kopieren
kann, Passphrase rein, fertig. Zusätzlich: ein File statt zwei atomar zu swappen
(Schritt 3) — robuster auf dem nicht lokal testbaren, datenkritischen Pfad.

**Was bleibt invariant.** Es gibt weiterhin **ein** Nutzer-Geheimnis (Passphrase =
App-Login, gated DB **und** Backups). Nur der *interne* KDF-Algorithmus
unterscheidet sich: DB = SQLCipher-PBKDF2-HMAC-SHA512; Backup-Hülle = Argon2id
(ADR 0009). PBKDF2-HMAC-SHA512 @ 256k ist Industriestandard und für das
Bedrohungsmodell (gestohlener Laptop / kopierte Datei) robust; der harte
Onboarding-Warnhinweis + die Empfehlung Passwort-Manager (Pt. 4) bleiben Pflicht.

**Build.** `libsqlite3-sys = "=0.30.1"` (Version von `sqlx-sqlite 0.8.6`, via
Cargo.lock verifiziert) mit `bundled-sqlcipher-vendored-openssl`; die exakte
Versions-Pin erzwingt Cargo-Feature-Unification auf sqlx' libsqlite3-sys.
Build-Tools am Host: Perl + (Windows-MSVC) NASM.

**Verworfen.** Argon2id-Raw-Key + Salt-Sidecar (neue Totalverlust-Fläche,
Zwei-Datei-Atomicity); SQLCipher-Plaintext-Header (`cipher_plaintext_header_size`/
`cipher_salt`) für Argon2id-im-Header (versions-/plattformabhängige SQLCipher-
Interna, hier nicht testbar → zu riskant für v1.0; ggf. Post-v1.0-Enhancement).
