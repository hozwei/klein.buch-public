-- Block 15 (Phase 2D) — Notifications + Reminder-Rules.
--
-- PRD §5.4 nennt diese Migration "0008_notifications"; real verschoben auf 0012
-- (Migrations-Nummern liegen +4 gegenüber dem PRD, siehe TASKS.md).
--
-- notification_rules: User-konfigurierbare Reminder-Engine (enable/disable je Regel,
--   Kanäle In-App + OS-native getrennt schaltbar, Schedule-Parameter in config_json).
-- notifications:      persistente In-App-Inbox. Append + Dismiss; kein Hard-Delete
--   im UI (Inbox-Historie bleibt nachvollziehbar). related_entity_* verlinkt z. B.
--   eine überfällige Rechnung; action_url führt die UI direkt dorthin.

CREATE TABLE notification_rules (
    id                  TEXT PRIMARY KEY NOT NULL,
    rule_type           TEXT NOT NULL CHECK (rule_type IN
                        ('monthly_doc_check','recurring_due','invoice_overdue',
                         'quote_expiring','tax_deadline','fiscal_year_lock_pending',
                         'archive_integrity_failed','backup_overdue','custom')),
    label               TEXT NOT NULL,
    enabled             INTEGER NOT NULL DEFAULT 1,
    config_json         TEXT NOT NULL,
    deliver_in_app      INTEGER NOT NULL DEFAULT 1,
    deliver_os_native   INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

CREATE TABLE notifications (
    id                  TEXT PRIMARY KEY NOT NULL,
    rule_id             TEXT REFERENCES notification_rules(id),
    title               TEXT NOT NULL,
    body                TEXT NOT NULL,
    severity            TEXT NOT NULL CHECK (severity IN ('info','warning','urgent')),
    related_entity_type TEXT,
    related_entity_id   TEXT,
    triggered_at        TEXT NOT NULL DEFAULT (datetime('now','utc')),
    dismissed_at        TEXT,
    action_url          TEXT,
    -- Dedup-Schlüssel: verhindert, dass derselbe Reminder pro Periode mehrfach
    -- erzeugt wird (z. B. "monthly_doc_check:2026-06"). Pro Schlüssel nur eine Zeile.
    dedup_key           TEXT
) STRICT;
CREATE INDEX idx_notifications_dismissed ON notifications(dismissed_at);
CREATE INDEX idx_notifications_triggered ON notifications(triggered_at);
CREATE UNIQUE INDEX uq_notifications_dedup ON notifications(dedup_key) WHERE dedup_key IS NOT NULL;

-- Default-Regeln (deterministisch geseedet, stabile IDs; vom User per UI
-- enable/disable-bar). Manuel-Auswahl Block 15: alle vier aktiv.
INSERT INTO notification_rules (id, rule_type, label, enabled, config_json) VALUES
    ('rule_monthly_doc_check', 'monthly_doc_check',
     'Monats-Doku-Check', 1, '{"day_of_month":10}'),
    ('rule_fiscal_year_lock_pending', 'fiscal_year_lock_pending',
     'Geschäftsjahr abschließen fällig', 1, '{"month":6,"day":1}'),
    ('rule_backup_overdue', 'backup_overdue',
     'Backup überfällig', 1, '{"max_age_days":7}'),
    ('rule_invoice_overdue', 'invoice_overdue',
     'Rechnung überfällig', 1, '{}'),
    ('rule_archive_integrity_failed', 'archive_integrity_failed',
     'Archiv-Integrität gestört', 1, '{}');

-- Auto-AfA zur GJ-Wende (01.01.): Default an, in den Einstellungen abschaltbar
-- (Manuel-Entscheidung Block 15). Der Cron bucht die Vorjahres-AfA automatisch.
INSERT INTO app_settings (key, value) VALUES ('depreciation_auto_year_close', '1');

UPDATE app_settings SET value = '12' WHERE key = 'schema_version';
