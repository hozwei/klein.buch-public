-- G1-LOG (Security/Backup-Härtung, ADR 0034) — Backup-Protokoll.
--
-- Lückenloser, unveränderlicher Nachweis JEDES Sicherungslaufs (Erfolg UND
-- Fehlschlag) — analog zu `email_log` (Block 16b). Zweck:
--   1. Nachweis, dass (und wann) eine Sicherung lief: Datum, Name, Größe, Pfad.
--   2. Troubleshooting (z. B. Off-Site-Ziel offline → status='failed' + detail).
--   3. Vertrauen: der Nutzer sieht schwarz auf weiß, dass gesichert wird.
--
-- Hard-Line: append-only — kein UPDATE, kein DELETE (Trigger erzwingen das),
-- damit das Protokoll als belastbarer Nachweis taugt.
--
-- KEINE Passphrase, kein Geheimnis — `detail` trägt nur Fehlertext/Antwort.
--
-- Abweichung von der ADR-0034-Entwurfs-SQL (dokumentiert als ADR-0034-Amendment
-- "Umsetzungsnotiz G1-LOG", 2026-05-25): die CHECK-Mengen folgen der REALEN
-- Laufzeit, nicht dem Entwurf, sonst gäbe es CHECK-Verletzungen (G1-HARDEN.2):
--   * `trigger`     = die tatsächlichen `create_now`-Auslöser (= `backup_history
--                     .trigger_reason`): manual/auto_daily/auto_critical/pre_restore.
--                     (Der Entwurf nannte start/lock/weekly/monthly/manual — diese
--                     Werte erzeugt der Code nie; G1-BKP.4 hat das Tier-/Auslöser-
--                     Modell finalisiert: Floor + Off-Site je Lauf, kein weekly.)
--   * `target_kind` = local (Floor, immer) / directory (Off-Site-Ordner: USB/NAS/
--                     Cloud-Sync) / sftp. (Der Entwurf nannte 'cloud_folder' — ein
--                     Off-Site-Ordner ist aber oft USB/NAS, daher 'directory'.)

CREATE TABLE backup_log (
    id           TEXT PRIMARY KEY NOT NULL,                       -- UUIDv7
    created_at   TEXT NOT NULL DEFAULT (datetime('now','utc')),
    trigger      TEXT NOT NULL CHECK (trigger IN
                 ('manual','auto_daily','auto_critical','pre_restore')),
    target_kind  TEXT NOT NULL CHECK (target_kind IN
                 ('local','directory','sftp')),
    target_label TEXT,                                            -- z. B. SFTP-Host; NULL für lokal/Ordner
    file_name    TEXT NOT NULL,                                   -- 'klein-buch-YYYYMMDD-HHMMSS.kbk'
    full_path    TEXT NOT NULL,                                   -- vollständiger Pfad bzw. sftp://…-URI
    size_bytes   INTEGER NOT NULL,
    status       TEXT NOT NULL CHECK (status IN ('ok','failed')),
    detail       TEXT                                             -- Fehlertext bei status='failed', KEINE Passphrase
) STRICT;

CREATE INDEX idx_backup_log_created ON backup_log(created_at DESC);
CREATE INDEX idx_backup_log_status ON backup_log(status, created_at DESC);

CREATE TRIGGER trg_backup_log_no_update BEFORE UPDATE ON backup_log
BEGIN SELECT RAISE(ABORT, 'backup_log ist append-only (kein UPDATE).'); END;

CREATE TRIGGER trg_backup_log_no_delete BEFORE DELETE ON backup_log
BEGIN SELECT RAISE(ABORT, 'backup_log ist append-only (kein DELETE).'); END;

UPDATE app_settings SET value = '26' WHERE key = 'schema_version';
