-- Klein.Buch — Phase-1-Schema
-- Konventionen: STRICT überall, foreign_keys = ON, Geld in Cent (i64),
-- Datums-Felder ISO-8601-TEXT, UUIDv7 als PKs.

PRAGMA foreign_keys = ON;

-- ============================================================================
-- App-Settings (Singleton-Style, key-value)
-- ============================================================================
CREATE TABLE app_settings (
    key             TEXT PRIMARY KEY NOT NULL,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

INSERT INTO app_settings (key, value) VALUES
    ('schema_version', '1'),
    ('default_fiscal_year_start_month', '1'),
    ('app_initialized_at', datetime('now','utc'));

-- ============================================================================
-- Contacts: Kunden + Lieferanten + Partner (unified)
-- ============================================================================
CREATE TABLE contacts (
    id                  TEXT PRIMARY KEY NOT NULL,
    contact_type        TEXT NOT NULL CHECK (contact_type IN ('customer','vendor','both','partner')),
    name                TEXT NOT NULL,
    legal_form          TEXT,
    vat_id              TEXT,
    tax_number          TEXT,
    street              TEXT,
    postal_code         TEXT,
    city                TEXT,
    country_code        TEXT NOT NULL DEFAULT 'DE',
    email               TEXT,
    phone               TEXT,
    iban                TEXT,
    bic                 TEXT,
    accepts_einvoice    INTEGER NOT NULL DEFAULT 1 CHECK (accepts_einvoice IN (0,1)),
    archived            INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
    notes               TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_contacts_name ON contacts(name);
CREATE INDEX idx_contacts_type ON contacts(contact_type);

-- ============================================================================
-- Seller-Profile (Singleton, id = 1)
-- ============================================================================
CREATE TABLE seller_profile (
    id                          INTEGER PRIMARY KEY CHECK (id = 1),
    name                        TEXT NOT NULL,
    legal_form                  TEXT,
    street                      TEXT NOT NULL,
    postal_code                 TEXT NOT NULL,
    city                        TEXT NOT NULL,
    country_code                TEXT NOT NULL DEFAULT 'DE',
    tax_number                  TEXT NOT NULL,
    vat_id                      TEXT,
    email                       TEXT NOT NULL,
    phone                       TEXT,
    iban                        TEXT,
    bic                         TEXT,
    logo_filename               TEXT,
    is_kleinunternehmer         INTEGER NOT NULL DEFAULT 1 CHECK (is_kleinunternehmer IN (0,1)),
    waived_paragraph_19_since   TEXT,
    default_pdf_template        TEXT NOT NULL DEFAULT 'default',
    default_currency            TEXT NOT NULL DEFAULT 'EUR',
    updated_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

-- ============================================================================
-- Mail-Accounts
-- ============================================================================
CREATE TABLE mail_accounts (
    id                  TEXT PRIMARY KEY NOT NULL,
    label               TEXT NOT NULL,
    auth_type           TEXT NOT NULL CHECK (auth_type IN ('smtp_password','oauth_microsoft')),
    smtp_host           TEXT,
    smtp_port           INTEGER,
    smtp_user           TEXT,
    smtp_use_tls        INTEGER NOT NULL DEFAULT 1 CHECK (smtp_use_tls IN (0,1)),
    keychain_service_id TEXT,
    from_email          TEXT NOT NULL,
    from_name           TEXT NOT NULL,
    is_default          INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0,1)),
    last_used_at        TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_mail_accounts_default ON mail_accounts(is_default);

-- ============================================================================
-- Archive-Entries (write-once, immutable via hash)
-- Wird vor invoices angelegt, damit Foreign-Keys aufgehen.
-- ============================================================================
CREATE TABLE archive_entries (
    id                  TEXT PRIMARY KEY NOT NULL,
    file_path           TEXT NOT NULL UNIQUE,
    file_name           TEXT NOT NULL,
    file_hash_sha256    TEXT NOT NULL,
    file_size_bytes     INTEGER NOT NULL,
    mime_type           TEXT NOT NULL,
    source              TEXT NOT NULL,
    received_at         TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

-- ============================================================================
-- Invoices
-- ============================================================================
CREATE TABLE invoices (
    id                          TEXT PRIMARY KEY NOT NULL,
    invoice_number              TEXT NOT NULL UNIQUE,
    fiscal_year                 INTEGER NOT NULL,
    direction                   TEXT NOT NULL CHECK (direction IN ('issued','received')),
    invoice_date                TEXT NOT NULL,
    delivery_date               TEXT,
    due_date                    TEXT,
    contact_id                  TEXT NOT NULL REFERENCES contacts(id),
    -- Seller-Snapshot (immutable nach Lock)
    seller_name                 TEXT NOT NULL,
    seller_street               TEXT NOT NULL,
    seller_postal_code          TEXT NOT NULL,
    seller_city                 TEXT NOT NULL,
    seller_tax_number           TEXT NOT NULL,
    seller_vat_id               TEXT,
    -- Beträge in Cent
    net_amount_cents            INTEGER NOT NULL,
    tax_amount_cents            INTEGER NOT NULL DEFAULT 0,
    gross_amount_cents          INTEGER NOT NULL,
    currency_code               TEXT NOT NULL DEFAULT 'EUR',
    is_kleinunternehmer         INTEGER NOT NULL DEFAULT 1 CHECK (is_kleinunternehmer IN (0,1)),
    pdf_template                TEXT NOT NULL DEFAULT 'default',
    -- Status-Flow: draft → issued → sent → partially_paid → paid | canceled
    status                      TEXT NOT NULL DEFAULT 'draft'
                                CHECK (status IN ('draft','issued','sent','partially_paid','paid','canceled')),
    sent_at                     TEXT,
    -- Zahlungs-Tracking (Cash-Basis für EÜR)
    paid_amount_cents           INTEGER NOT NULL DEFAULT 0,
    paid_at                     TEXT,
    payment_history_json        TEXT,
    -- Storno
    canceled_at                 TEXT,
    canceled_by_storno_id       TEXT REFERENCES invoices(id),
    is_storno_for               TEXT REFERENCES invoices(id),
    cancel_reason               TEXT,
    -- E-Rechnung-Validierung
    validation_status           TEXT CHECK (validation_status IN ('passed','failed','warning')),
    validation_report           TEXT,
    validated_at                TEXT,
    -- Archive
    pdf_archive_id              TEXT REFERENCES archive_entries(id),
    xml_archive_id              TEXT REFERENCES archive_entries(id),
    -- Lock
    locked_at                   TEXT,
    notes                       TEXT,
    created_at                  TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_invoices_fiscal_year ON invoices(fiscal_year);
CREATE INDEX idx_invoices_status ON invoices(status);
CREATE INDEX idx_invoices_date ON invoices(invoice_date);
CREATE INDEX idx_invoices_paid_at ON invoices(paid_at);
CREATE INDEX idx_invoices_contact ON invoices(contact_id);
CREATE INDEX idx_invoices_locked ON invoices(locked_at);

CREATE TABLE invoice_items (
    id                  TEXT PRIMARY KEY NOT NULL,
    invoice_id          TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    position            INTEGER NOT NULL,
    description         TEXT NOT NULL,
    quantity            REAL NOT NULL,
    unit_code           TEXT NOT NULL DEFAULT 'C62',
    unit_price_cents    INTEGER NOT NULL,
    net_amount_cents    INTEGER NOT NULL,
    tax_rate_percent    REAL NOT NULL DEFAULT 0.0,
    tax_category_code   TEXT NOT NULL DEFAULT 'E'
                        CHECK (tax_category_code IN ('S','Z','E','AE','K','G','O','L','M'))
) STRICT;
CREATE UNIQUE INDEX uq_invoice_items_position ON invoice_items(invoice_id, position);

-- ============================================================================
-- Attachments (Lieferschein, Auftragsbestätigung, Garantie, etc.)
-- ============================================================================
CREATE TABLE attachments (
    id                  TEXT PRIMARY KEY NOT NULL,
    parent_type         TEXT NOT NULL CHECK (parent_type IN
                        ('invoice','quote','expense','asset','contact','recurring')),
    parent_id           TEXT NOT NULL,
    archive_entry_id    TEXT NOT NULL REFERENCES archive_entries(id),
    label               TEXT,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_attachments_parent ON attachments(parent_type, parent_id);

-- ============================================================================
-- Audit-Log (append-only)
-- ============================================================================
CREATE TABLE audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp_utc   TEXT NOT NULL DEFAULT (datetime('now','utc')),
    actor           TEXT NOT NULL DEFAULT 'system',
    action          TEXT NOT NULL,
    entity_type     TEXT,
    entity_id       TEXT,
    details_json    TEXT
) STRICT;
CREATE INDEX idx_audit_timestamp ON audit_log(timestamp_utc);
CREATE INDEX idx_audit_action ON audit_log(action);

-- ============================================================================
-- Doc-Number-Counters (pro GJ + Doc-Typ)
-- ============================================================================
CREATE TABLE doc_number_counters (
    doc_type        TEXT NOT NULL,
    fiscal_year     INTEGER NOT NULL,
    last_value      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (doc_type, fiscal_year)
) STRICT;

-- ============================================================================
-- Backup-History
-- ============================================================================
CREATE TABLE backup_history (
    id                  TEXT PRIMARY KEY NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc')),
    target_path         TEXT NOT NULL,
    file_hash_sha256    TEXT NOT NULL,
    file_size_bytes     INTEGER NOT NULL,
    is_encrypted        INTEGER NOT NULL DEFAULT 1 CHECK (is_encrypted IN (0,1)),
    retention_tag       TEXT NOT NULL CHECK (retention_tag IN ('daily','monthly','yearly','manual')),
    trigger_reason      TEXT NOT NULL CHECK (trigger_reason IN
                        ('auto_daily','auto_critical','manual','pre_restore')),
    db_schema_version   INTEGER NOT NULL,
    app_version         TEXT NOT NULL,
    verified_at         TEXT
) STRICT;
CREATE INDEX idx_backup_history_created ON backup_history(created_at);

-- ============================================================================
-- Archive-Integrity-Checks (periodische Hash-Verifizierung)
-- ============================================================================
CREATE TABLE archive_integrity_checks (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at              TEXT NOT NULL DEFAULT (datetime('now','utc')),
    finished_at             TEXT,
    files_checked           INTEGER NOT NULL DEFAULT 0,
    files_passed            INTEGER NOT NULL DEFAULT 0,
    files_failed            INTEGER NOT NULL DEFAULT 0,
    failed_archive_ids_json TEXT
) STRICT;
CREATE INDEX idx_integrity_checks_started ON archive_integrity_checks(started_at);

-- ============================================================================
-- GoBD-Triggers: Immutability
-- ============================================================================

-- Audit-Log ist append-only
CREATE TRIGGER trg_audit_no_update BEFORE UPDATE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;

CREATE TRIGGER trg_audit_no_delete BEFORE DELETE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;

-- Locked invoices: Kernfelder unveränderlich
-- Erlaubt: status, paid_at, paid_amount_cents, payment_history_json,
-- canceled_at, sent_at, validation_*, notes, archive_ids
CREATE TRIGGER trg_invoices_immutable BEFORE UPDATE ON invoices
WHEN OLD.locked_at IS NOT NULL
  AND (NEW.invoice_number != OLD.invoice_number
    OR NEW.invoice_date != OLD.invoice_date
    OR NEW.net_amount_cents != OLD.net_amount_cents
    OR NEW.gross_amount_cents != OLD.gross_amount_cents
    OR NEW.tax_amount_cents != OLD.tax_amount_cents
    OR NEW.contact_id != OLD.contact_id
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.is_kleinunternehmer != OLD.is_kleinunternehmer
    OR NEW.direction != OLD.direction)
BEGIN SELECT RAISE(ABORT, 'invoice is locked: core fields immutable'); END;

-- Archive-Entries: keine Hash- oder Path-Änderungen erlaubt
CREATE TRIGGER trg_archive_no_update BEFORE UPDATE ON archive_entries
WHEN OLD.file_hash_sha256 != NEW.file_hash_sha256
  OR OLD.file_path != NEW.file_path
  OR OLD.file_size_bytes != NEW.file_size_bytes
BEGIN SELECT RAISE(ABORT, 'archive_entries: hash, path, size are immutable'); END;
