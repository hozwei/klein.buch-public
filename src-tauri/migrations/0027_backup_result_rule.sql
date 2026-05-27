-- G1-NOTIFY (ADR 0027 + ADR 0034) — Pro-Lauf-Backup-Ergebnis als eigene,
-- abschaltbare Hinweis-Regel ("Backup-Ergebnis").
--
-- Eigener Schalter neben 'backup_overdue' (Manuel-Entscheidung 2026-05-25: zwei
-- getrennte Regeln). Der Pro-Lauf-Hinweis meldet:
--   * Fehlschlag der lokalen Pflichtkopie ODER der externen Spiegelung — IMMER,
--   * Erfolg NUR bei manuell ausgelöstem Backup (Auto-Lock-/Tages-Backups bleiben
--     bei Erfolg lautlos, sonst Hinweis-Spam bei jedem festgeschriebenen Beleg).
--
-- Emittiert aus `backup::create_now` ⇒ schreibt in die In-App-Inbox; KEIN
-- OS-Push (create_now hat keinen AppHandle) → deliver_os_native = 0. Die
-- OS-native Eskalation übernimmt die periodische 'backup_overdue'-Regel.
--
-- rule_type 'custom' ist bereits im CHECK von notification_rules → keine
-- Tabellen-Neuanlage nötig. Lookup im Code erfolgt über die stabile id.

INSERT INTO notification_rules
    (id, rule_type, label, enabled, config_json, deliver_in_app, deliver_os_native)
VALUES
    ('rule_backup_result', 'custom', 'Backup-Ergebnis', 1, '{}', 1, 0);

UPDATE app_settings SET value = '27' WHERE key = 'schema_version';
