-- 0031 — Zugangsdaten in der Datenbank (Plan W8, ADR 0039 E-Amendment
-- ADR 0011/0028): SMTP-/SFTP-Passwörter und OAuth-Refresh-Token liegen nicht
-- mehr im OS-Schlüsselbund (im Container nicht vorhanden), sondern hier — die
-- Datei ist mit SQLCipher verschlüsselt (Passphrase = DB-Key, ADR 0035).
--
-- Schlüssel:
--   smtp:<mail_account.id>       SMTP-Passwort
--   oauth:<mail_account.id>      OAuth-Refresh-Token (Graph, im Server ausgeblendet)
--   sftp:<backup-target-id>      SFTP-Passwort
--
-- Nie im Migrations- oder DSGVO-Export, nie im Log oder Audit.

CREATE TABLE secrets (
    key         TEXT PRIMARY KEY NOT NULL,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

UPDATE app_settings
   SET value = '31',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
