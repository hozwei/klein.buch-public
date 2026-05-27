-- Migration 0029: Drop-Folder fuer eingehende E-Rechnungen (Block PV1-DROP).
--
-- Watched-Folder, der per 5-min-Scheduler-Tick + App-Start-Sweep auf XML- und
-- PDF-Dateien geprueft wird. Erfolgreich importierte Dateien wandern nach
-- processed/YYYY-MM/, fehlerhafte nach failed/. Die Import-Pipeline ist
-- IDENTISCH zum UI-Import (commands::expenses::create_from_einvoice ueber die
-- gemeinsamen Helfer parse_einvoice_with_paths + create_from_einvoice_with):
-- Parse -> beratendes KoSIT -> Archiv (ReceivedEinvoice, write-once) -> Expense
-- (sofort gelockt). Polling statt notify-Crate wegen OneDrive-Quirks (ADR 0037,
-- D-71). Failure-Routing behaelt das Original-File in failed/ (kein
-- Auto-Delete; Manuel pruefst manuell).
--
-- Zwei neue app_settings-Keys (idempotent via INSERT OR IGNORE; bestehende
-- Werte werden NICHT ueberschrieben, falls Migration mehrfach laeuft):
--   drop_folder_enabled  '0'/'1'  Default off; Pre-Check beim Aktivieren
--   drop_folder_path     String   Absoluter Ordner-Pfad; '' = nicht gesetzt
--
-- Zwei neue Notification-Rules (deliver-Flags spiegeln ADR 0037 D-78):
--   rule_drop_folder_import_ok      Inbox-only, default OFF
--                                   (Routine, kein OS-Toast-Spam)
--   rule_drop_folder_import_failed  Inbox + OS-Toast, default ON
--                                   (braucht Sichtbarkeit fuer Lieferanten-Klaerung)
--
-- rule_type='custom' ist bereits im notification_rules-CHECK enthalten (vgl.
-- 0027_backup_result_rule.sql) -> keine Tabellen-Aenderung noetig. Lookup im
-- Code erfolgt ueber die stabile id (rules::get).

INSERT OR IGNORE INTO app_settings (key, value, updated_at) VALUES
    ('drop_folder_enabled', '0', datetime('now','utc')),
    ('drop_folder_path',    '',  datetime('now','utc'));

INSERT INTO notification_rules
    (id, rule_type, label, enabled, config_json, deliver_in_app, deliver_os_native)
VALUES
    ('rule_drop_folder_import_ok',
     'custom',
     'Drop-Folder: Import erfolgreich',
     0, '{}', 1, 0),
    ('rule_drop_folder_import_failed',
     'custom',
     'Drop-Folder: Import fehlgeschlagen',
     1, '{}', 1, 1);

UPDATE app_settings
   SET value = '29',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
