-- Block 16b (Phase 2D) — E-Mail-Versandprotokoll.
--
-- Lückenloser, unveränderlicher Nachweis JEDES Versandversuchs (Erfolg UND
-- Fehlschlag) für Rechnungen, Angebote und Test-Mails. Zweck:
--   1. Nachweis, dass eine E-Mail versendet wurde (Zeitpunkt, Empfänger, Beleg).
--   2. Troubleshooting im Fehlerfall (Fehlermeldung).
--   3. Nachvollziehbarkeit der Provider-Antwort: SMTP-Code + Server-Reply bzw.
--      Microsoft-Graph-HTTP-Status + request-id.
--
-- Hard-Line: append-only — kein UPDATE, kein DELETE (Trigger erzwingen das),
-- damit das Protokoll als belastbarer Nachweis taugt.

CREATE TABLE email_log (
    id                 TEXT PRIMARY KEY NOT NULL,                       -- UUIDv7
    created_at         TEXT NOT NULL DEFAULT (datetime('now','utc')),
    account_id         TEXT,                                            -- mail_accounts.id (Snapshot, kein FK: Account löschbar, Log bleibt)
    account_label      TEXT,                                            -- Label-Snapshot
    channel            TEXT NOT NULL,                                   -- 'smtp' | 'graph'
    related_kind       TEXT NOT NULL,                                   -- 'invoice' | 'quote' | 'test'
    related_id         TEXT,                                            -- invoices.id / quotes.id
    related_number     TEXT,                                            -- 'RE-2026-0001' / 'AN-2026-0001' / NULL
    from_email         TEXT NOT NULL,
    to_email           TEXT NOT NULL,
    subject            TEXT NOT NULL,
    attachment_count   INTEGER NOT NULL DEFAULT 0,
    status             TEXT NOT NULL CHECK (status IN ('success','failed')),
    provider_code      TEXT,                                            -- SMTP '250' / HTTP '202' | '403'
    provider_message   TEXT,                                            -- SMTP-Reply (enthält oft Queue-ID) / Graph-Status
    request_id         TEXT,                                            -- Graph request-id (für MS-Support)
    error              TEXT                                             -- Fehlermeldung bei status='failed'
) STRICT;

CREATE INDEX idx_email_log_created ON email_log(created_at DESC);
CREATE INDEX idx_email_log_related ON email_log(related_kind, related_id);

CREATE TRIGGER trg_email_log_no_update BEFORE UPDATE ON email_log
BEGIN SELECT RAISE(ABORT, 'email_log ist append-only (kein UPDATE).'); END;

CREATE TRIGGER trg_email_log_no_delete BEFORE DELETE ON email_log
BEGIN SELECT RAISE(ABORT, 'email_log ist append-only (kein DELETE).'); END;

UPDATE app_settings SET value = '15' WHERE key = 'schema_version';
