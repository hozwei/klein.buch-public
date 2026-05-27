# ADR 0034 — Backup-Ziele, Aufbewahrung + Backup-Log

**Status:** Akzeptiert · 2026-05-24 · v1.0 (Umsetzung im Security-Block G1). Migration `0026_backup_log`.

## Kontext

Verschlüsselte Backups (ADR 0009) brauchen ein **Off-Site-Ziel** — der Default
liegt heute lokal in `%APPDATA%\…\backups` (gleiche Platte, keine Redundanz).
Zielgruppe ist **OSS für beliebige §19-Kleinunternehmer**: technisch meist Laien,
oft nur OneDrive vorhanden, viele sichern gar nichts oder auf USB. Gewünscht sind
„alle Anbieter" (Cloud, USB, eigener Server) — aber bedienbar für Laien.

## Entscheidung

1. **Zwei reale Ziel-Mechanismen statt fünf Integrationen.** Eine
   `BackupTarget`-Abstraktion mit genau zwei Varianten:
   - **Verzeichnis** — deckt **USB, NAS, OneDrive, iCloud, Dropbox, Nextcloud** ab.
     Alle „Cloud-Anbieter" sind auf dem Desktop nur ein **lokaler Ordner**, den der
     jeweilige Sync-Client selbst hochlädt. Die App schreibt einen verschlüsselten
     Blob in einen Pfad — **keine Cloud-API**.
   - **SFTP** — eigener Server über SSH (`russh`/`russh-sftp`). Der einzige echte
     Protokoll-Fall. **In v1.0 enthalten** (kein Post-v1.0).
   - **Bewusst NICHT:** rohes FTP (unverschlüsselt/Legacy) und rsync-als-Protokoll
     (braucht rsync-Binary beidseitig, Delta-Transfer bringt bei MB-großen ZIPs
     nichts). SFTP deckt „Remote-Server" sauber ab.
2. **Auto-Detect des OS-Cloud-Ordners** für 1-Klick-UX: OneDrive (Windows),
   iCloud Drive / OneDrive (macOS). Erkennung füllt das Ziel vor; **manuelle
   Ordnerwahl bleibt immer Fallback**. Linux + alles Übrige = „Ordner wählen".
3. **Drei Tiers / Aufbewahrungs-Policy** (kein „voll vs. normal" als Backup-Art —
   Snapshots sind ohnehin Vollkopien):

   | Tier | Wann | Wohin | Aufbewahrung |
   |---|---|---|---|
   | Floor (existiert) | jeder Lock + täglich beim Start | lokal `%APPDATA%\…\backups` | kurz (z. B. 7) |
   | Off-Site-Mirror | beim Start | gewähltes Ziel | kurz |
   | Langzeit-Voll | wöchentlich + monatlich | gewähltes Ziel | lang (z. B. 12 Monate) |

   Floor ist der Sicherheitsboden ohne Konfiguration; alles darüber ist **opt-in**.
4. **Append-only `backup_log`** (Migration `0026`, no-update/no-delete-Trigger,
   `email_log`-Muster). Jeder Lauf wird protokolliert; eine Log-Ansicht wie
   `settings/mail-log`. **Passphrase niemals im Log.**

   ```sql
   CREATE TABLE backup_log (
       id          TEXT PRIMARY KEY NOT NULL,           -- uuidv7
       created_at  TEXT NOT NULL DEFAULT (datetime('now','utc')),
       trigger     TEXT NOT NULL CHECK (trigger IN
                   ('start','lock','weekly','monthly','manual')),
       target_kind TEXT NOT NULL CHECK (target_kind IN
                   ('local','cloud_folder','sftp')),
       target_label TEXT,                                -- z. B. "OneDrive" / SFTP-Host
       file_name   TEXT NOT NULL,
       full_path   TEXT NOT NULL,                        -- vollständiger Pfad / Remote-Pfad
       size_bytes  INTEGER NOT NULL,
       status      TEXT NOT NULL CHECK (status IN ('ok','failed')),
       detail      TEXT                                  -- Fehlertext / Antwort, KEINE Passphrase
   ) STRICT;
   ```
5. **Backup-Notifications** als first-class Option (ADR 0027): Reminder „kein
   erfolgreiches Off-Site-Backup seit > N Tagen" (Schwelle einstellbar) + Erfolgs-/
   Fehler-Hinweis nach jedem Lauf. Nutzt die bestehende Reminder-Regel.

## Konsequenzen

- Eine Abstraktion (Verzeichnis + SFTP) deckt 100 % der gewünschten „Anbieter"
  ohne fünf Einzelintegrationen.
- Laien-Default: Floor läuft eh; falls OneDrive/iCloud erkannt → 1 Klick; sonst USB-Ordner.
- `backup_log` macht Datensicherung **nachweisbar** (Datum/Größe/Name/Pfad/Status) —
  wichtig für Vertrauen + Fehlersuche.
- Langzeit-Voll-Tier ist die Disaster-/Aufbewahrungs-Reserve (GoBD-Geist).

## Alternativen

| Option | Contra |
|---|---|
| Einzelne Cloud-APIs (OneDrive/Dropbox/GDrive SDK) | unnötig — alle sind nur Ordner; vervielfacht Code + OAuth-Aufwand |
| FTP/FTPS | unverschlüsselt/Legacy bzw. Gefrickel; SFTP ist sicherer und reicht |
| rsync als Protokoll | rsync-Binary beidseitig nötig; Delta sinnlos bei kleinen verschlüsselten ZIPs |
| Nur ein Ziel ohne Tiers | kein Langzeit-/Off-Site-Konzept; GoBD-Reserve fehlt |

## Umsetzungsnotiz G1-BKP.3 — SFTP-Ziel (2026-05-25)

- **Crates:** `russh = "0.60"` + `russh-sftp = "2.1"` (reine-Rust-Krypto-Backend,
  kein OpenSSL/libssh → konsistent mit dem rustls-/no-system-deps-Stack).
- **Achtung RC-Fläche:** russh 0.60 zieht **pre-release**-RustCrypto-Transport-
  Krypto (aes-gcm 0.11-rc, sha2 0.11, signature 3.0-rc, ed25519-dalek 3.0-pre).
  Diese koexistieren **semver-getrennt** mit unserem stabilen `aes-gcm 0.10`/
  `sha2 0.10` (Backup-Hülle, ADR 0009) — keine Feature-Unification, kein
  Konflikt — und werden **ausschließlich** für die SSH-Transportschicht genutzt,
  nie für Beleg-/Backup-Krypto. Wer die RC-Fläche vermeiden will, pinnt eine
  stabile-Krypto-russh-Linie (~0.52) — dann ist die Client-API neu zu verifizieren.
- **MITM-Schutz = Host-Key-Pinning.** `BackupTarget::Sftp` trägt einen gepinnten
  **SHA-256-Host-Key-Fingerprint** (`SHA256:…`). Der Upload lehnt ohne passenden
  Pin **ab** (kein Blind-Trust). Den Fingerprint ermittelt der einmalige
  Verbindungstest (`backup_test_sftp` → `sftp::probe`, TOFU); der Nutzer bestätigt
  ihn im UI, danach wird er im Ziel gespeichert.
- **Passwort nur im OS-Keychain** (`kleinbuch::backup::sftp`), nie in DB/Log/audit
  — wie die SMTP-Passphrase. **Auth = nur Passwort** in v1.0; Public-Key-/Agent-
  Auth ist bewusst verschoben (Follow-up).
- **Restore-Caveat:** Der lokale Restore-Wizard liest **lokale** Pfade. Ein
  SFTP-Backup (target_path = `sftp://…`-URI) wird zum Wiederherstellen erst
  **manuell heruntergeladen** und dann als lokale Datei eingespielt. Off-Site-
  Restore-Direktzugriff ist Post-v1.0.
- **Kein DB-Schema-Change** (v25 bleibt; Migration `0026` weiter für G1-LOG
  reserviert). Commit-Prefix `block-g1-bkp-2`.

## Umsetzungsnotiz G1-BKP.4 — Tiers/Retention (2026-05-25)

Konkretisiert (und in zwei Punkten **bewusst vereinfacht** ggü. der Tier-Tabelle
oben) — entschieden mit Manuel 2026-05-25:

- **„Immer zweifach" statt „Mirror nur beim Start".** Jede Sicherung schreibt
  **erst den Floor** (lokal, `paths.backups_dir`, Pflicht) und spiegelt **direkt
  danach** auf das konfigurierte Off-Site-Ziel (best-effort). Damit ist die
  Off-Site-Kopie sofort aktuell. Trade-off (akzeptiert): ein Lock-Event hängt an
  der Erreichbarkeit/Timeout des Off-Site-Ziels — aber der Mirror-Fehler ist
  **nie fatal** (Floor ist gesichert; ein offline-Ziel rollt keinen
  GoBD-festgeschriebenen Vorgang zurück). `pre_restore`-Backups werden nicht
  gespiegelt (lokaler Sicherheits-Snapshot).
- **Kein eigenes „weekly"-Tier.** Der `backup_history.retention_tag`-CHECK
  (`daily/monthly/yearly/manual`) bleibt unverändert → **keine Migration**
  (schema v25, `0026` bleibt für G1-LOG). „Langzeit" wird über
  monatlich (12) + jährlich (7) abgedeckt.
- **Tier-Zuordnung am Pfad (keine Schema-Spalte).** Ein `backup_history`-Eintrag
  ist Floor, wenn sein `target_path` unter `paths.backups_dir` liegt
  (`Path::starts_with`), sonst Off-Site (anderer Ordner **oder** `sftp://`-URI).
  Jede gespiegelte Sicherung erzeugt **zwei** History-Zeilen (Floor + Off-Site,
  gleicher Hash/`retention_tag`).
- **Getrennte Aufbewahrung:** Floor = **kurz** (`RetentionPolicy::floor()` =
  7/3/1), Off-Site/Langzeit = **lang** (`offsite()` = 30/12/7). Ist **kein**
  Off-Site-Ziel konfiguriert, erbt der Floor die **lange** Policy (er ist dann die
  einzige Kopie — kein Verlust für Nutzer ohne Off-Site).
- **Pruning lokal vs. SFTP:** Lokale/Cloud-Ordner-Dateien werden physisch
  gelöscht; Off-Site-**SFTP**-Einträge werden nur aus der History entfernt — die
  Remote-Datei bleibt (kein Netzwerk-Delete nach jedem Lock; der eigene Server
  wird vom Nutzer verwaltet). *Follow-up:* optionales Remote-Pruning + Start-
  Catch-up für Backups, die bei offline-Ziel nur lokal entstanden sind.
- **Code:** `backup::target::offsite_target`, `backup::rotation::{is_floor_path,
  plan_tiered_deletions, run}` (run nimmt jetzt `floor_dir`), `backup::create_now`
  (Floor + best-effort Mirror, `BackupOutcome.mirror_target/mirror_error`),
  `commands::backup` (`BackupSettings.floor_path`, `backup_open_folder` +
  `backup_reveal_path` via `tauri-plugin-opener`), Frontend `settings/backup`
  (Floor-Anzeige, Ort-Spalte **mit vollem Pfad**, „Ordner öffnen"-Buttons bei
  Floor/Off-Site-Verzeichnis + je Verlaufs-Zeile — **außer SFTP**,
  Mirror-Rückmeldung). **„Durchsuchen…"**-Ordner-Picker via neuem
  `tauri-plugin-dialog` (Capability `dialog:allow-open`, Init in `lib.rs`,
  npm `@tauri-apps/plugin-dialog` → `pnpm install` nötig). Off-Site-„Ordner
  öffnen" hängt jetzt am Pfad-**Feld** (`targetInput`), nicht mehr nur am
  gespeicherten Ziel. Commit-Prefix `block-g1-bkp-3`.

## Umsetzungsnotiz G1-LOG — Backup-Protokoll (2026-05-25)

Migration `0026_backup_log` (schema **v25 → v26**), Repo `db::repo::backup_log`
({`insert`, `list`, `search`}), Commands `backup_log_list`/`backup_log_search`,
Ansicht `settings/backup-log` (Struktur/Stil wie `settings/mail-log`).

Zwei **bewusste Abweichungen von der Entwurfs-SQL oben** — die CHECK-Mengen
folgen der **realen Laufzeit**, sonst gäbe es garantierte CHECK-Verletzungen
(genau das Risiko aus G1-HARDEN.2):

- **`trigger`** = die tatsächlichen `create_now`-Auslöser, identisch zu
  `backup_history.trigger_reason`: `('manual','auto_daily','auto_critical',
  'pre_restore')`. Der Entwurf nannte `('start','lock','weekly','monthly',
  'manual')` — diese Werte erzeugt der Code **nie**. G1-BKP.4 hat das Modell
  finalisiert: Floor + Off-Site **je Lauf**, **kein** weekly-Tier; Lock-Events
  werden über `db_trigger_reason` auf `auto_critical` abgebildet (das konkrete
  Lock-Event-Label bleibt im `audit_log` nachvollziehbar).
- **`target_kind`** = `('local','directory','sftp')`. Der Entwurf nannte
  `'cloud_folder'` für das Off-Site-Verzeichnis; ein Off-Site-Ordner ist aber
  oft USB/NAS, nicht Cloud → `'directory'` ist korrekter. `local` = der immer
  geschriebene Floor; `directory`/`sftp` = die `BackupTarget`-Varianten.

Weitere Festlegungen:

- **Ein Eintrag pro Ziel und Lauf.** Eine gespiegelte Sicherung erzeugt **zwei**
  `backup_log`-Zeilen (Floor `local` + Off-Site `directory`/`sftp`), jeweils mit
  eigenem Status. `pre_restore` wird nicht gespiegelt → nur die Floor-Zeile.
- **Erfolg UND Fehlschlag** werden protokolliert (wie `email_log`). Der für den
  Nutzer wichtigste Fall — Off-Site-Ziel offline — landet als `status='failed'`
  mit Fehlertext in `detail`. Die Log-Schreibung ist **best-effort** (`.ok()`):
  ein Protokoll-Fehler darf einen GoBD-festgeschriebenen Vorgang nie zurückrollen.
- **Append-only** (Trigger gegen UPDATE/DELETE), **kein** Geheimnis im Log —
  `detail` trägt ausschließlich Fehlertext, **nie** die Passphrase.
- **`full_path`** ist bei Erfolg der reale Pfad/URI aus `write_backup`; bei einem
  Fehlschlag eine Best-Wissen-Beschreibung (`target::log_failed_path`:
  Zielordner+Datei bzw. `sftp://user@host:port/remote/datei`, ohne Passwort).

## Umsetzungsnotiz G1-NOTIFY — Backup-Hinweise (2026-05-25)

Knüpft an `backup_log` (G1-LOG) an; nutzt die bestehende Notify-Engine (ADR 0027,
Block 15). Migration `0027` (schema **v26 → v27**), Commit-Prefix `block-g1-notify`.

- **`backup_overdue` ist jetzt Off-Site-bewusst.** Ist ein Off-Site-Ziel
  konfiguriert (`target::offsite_target` ≠ Floor), zählt das letzte **erfolgreiche
  Off-Site-Backup** aus `backup_log` (`status='ok' AND target_kind IN
  ('directory','sftp')`) statt „irgendein Backup" — der Floor ist durch Lock-/
  Tages-Backup ohnehin fast immer frisch, die Off-Site-Kopie ist das, was ausfällt.
  Ohne Off-Site-Ziel bleibt es beim Floor (`backup_history`). Schwelle weiter über
  `config_json.max_age_days` (Default 7). `reminders::run` bekommt dafür `floor_dir`.
- **Neue Regel „Backup-Ergebnis"** (`rule_backup_result`, `rule_type='custom'` —
  bereits im CHECK, daher keine Tabellen-Neuanlage). Eigener Schalter neben
  „Backup überfällig" (Manuel-Entscheidung: zwei Schalter). Verhalten
  (Manuel-Entscheidung „Fehler + Erfolg nur manuell"):
  - **Fehlschläge immer** — lokale Pflichtkopie *oder* externe Spiegelung scheitert
    → Warnung, Dedup pro Tag (`backup_result_failed_floor|offsite:<date>`).
  - **Erfolg nur bei `trigger_reason='manual'`** — Auto-Lock-/Tages-Backups bleiben
    bei Erfolg lautlos (sonst Hinweis-Spam bei jedem festgeschriebenen Beleg).
- **Inbox-only.** Emittiert aus `backup::create_now` (kein `AppHandle` →
  `notify::emit(pool, None, …)`), daher `deliver_os_native = 0`. Die In-App-Inbox
  ist die Quelle der Wahrheit (ADR 0027); die **OS-native Eskalation** liefert die
  periodische `backup_overdue`-Regel (läuft im Scheduler mit `AppHandle`). Best-
  effort: ein Hinweis-Fehler beeinflusst ein Backup nie; **nie** ein Geheimnis im
  Hinweis. Ein späteres Durchreichen des `AppHandle` für OS-Push pro Lauf ist ein
  optionales Follow-up.

## Referenzen

`backup/{mod,target,rotation,encrypt}.rs` + `backup/sftp.rs` (`russh`/`russh-sftp`,
G1-BKP.3 fertig), `db::repo::backup_log` (geplant), Migration `0026_backup_log`
(geplant), Frontend `routes/settings/backup` + Log-Ansicht; ADR 0009
(Backup-Krypto), ADR 0035 (Verschlüsselung at Rest), ADR 0027 (Reminder).
RELEASE-1.0-GUIDE.md §G1.
