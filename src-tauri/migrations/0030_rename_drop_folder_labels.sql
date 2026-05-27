-- Migration 0030: UI-Labels der Drop-Folder-Notification-Regeln umbenennen
-- (Block PV1-RENAME).
--
-- PV1-DROP (Migration 0029) hat den UI-Begriff "Drop-Folder" geseedet. Das
-- verstoesst gegen die UI-Sprachregel in CLAUDE.md ("UI-Texte auf Deutsch")
-- und gegen die Plain-Language-Hardline aus G2-DOC.2 ("eine Putzhilfe mit
-- Gewerbeschein muss das Handbuch verstehen"). Diese Migration zieht die
-- sichtbaren Labels auf "Rechnungs-Eingang" nach (analog "Rechnungs-Layout"
-- und "Privat-Geld" in der Sidebar) — die Code-Identifier (rule_id,
-- Settings-Keys, Route, Module) bleiben englisch nach CLAUDE.md-Hard-Rule
-- ("Code/Identifier/Commits englisch").
--
-- Idempotent: UPDATE ohne WHERE-Bedingung auf den exakt-alten Label-Wert
-- waere brittle bei manueller DB-Korrektur. Stattdessen WHERE auf die stabile
-- rule_id; das Label wird unkonditional auf den neuen Wert gesetzt. Mehrfach-
-- Ausfuehrung der Migration ist harmlos (Migration-Runner laeuft jede
-- Migration ohnehin nur einmal; die Idempotenz ist als Schutz gegen
-- ad-hoc-Reruns gedacht).

UPDATE notification_rules
   SET label = 'Rechnungs-Eingang: Rechnung übernommen'
 WHERE id = 'rule_drop_folder_import_ok';

UPDATE notification_rules
   SET label = 'Rechnungs-Eingang: Übernahme fehlgeschlagen'
 WHERE id = 'rule_drop_folder_import_failed';

UPDATE app_settings
   SET value = '30',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
