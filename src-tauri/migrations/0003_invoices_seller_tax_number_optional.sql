-- Migration 0003: invoices.seller_tax_number darf NULL sein.
--
-- Rationale: Kleinbetragsrechnungen (§33 UStDV, brutto ≤ 250 €) sind nicht
-- verpflichtet, Steuernummer oder USt-IdNr. zu nennen. validate_for_issue()
-- erlaubt das im Domain-Layer bereits — das Schema blockierte es weiterhin
-- mit NOT NULL und machte den Insert unmöglich.
--
-- SQLite kann NOT NULL nicht ohne Tabellen-Rebuild entfernen.

PRAGMA foreign_keys = OFF;

CREATE TABLE invoices_new (
    id                          TEXT PRIMARY KEY NOT NULL,
    invoice_number              TEXT NOT NULL UNIQUE,
    fiscal_year                 INTEGER NOT NULL,
    direction                   TEXT NOT NULL CHECK (direction IN ('issued','received')),
    invoice_date                TEXT NOT NULL,
    delivery_date               TEXT,
    due_date                    TEXT,
    contact_id                  TEXT NOT NULL REFERENCES contacts(id),
    seller_name                 TEXT NOT NULL,
    seller_street               TEXT NOT NULL,
    seller_postal_code          TEXT NOT NULL,
    seller_city                 TEXT NOT NULL,
    seller_tax_number           TEXT,
    seller_vat_id               TEXT,
    net_amount_cents            INTEGER NOT NULL,
    tax_amount_cents            INTEGER NOT NULL DEFAULT 0,
    gross_amount_cents          INTEGER NOT NULL,
    currency_code               TEXT NOT NULL DEFAULT 'EUR',
    is_kleinunternehmer         INTEGER NOT NULL DEFAULT 1 CHECK (is_kleinunternehmer IN (0,1)),
    pdf_template                TEXT NOT NULL DEFAULT 'default',
    status                      TEXT NOT NULL DEFAULT 'draft'
                                CHECK (status IN ('draft','issued','sent','partially_paid','paid','canceled')),
    sent_at                     TEXT,
    paid_amount_cents           INTEGER NOT NULL DEFAULT 0,
    paid_at                     TEXT,
    payment_history_json        TEXT,
    canceled_at                 TEXT,
    -- Self-FKs auf den FINALEN Tabellennamen `invoices` (existiert während
    -- dieser DDL noch als Original; nach RENAME wird die Selbst-Referenz
    -- automatisch auf die neue Tabelle aufgelöst).
    canceled_by_storno_id       TEXT REFERENCES invoices(id),
    is_storno_for               TEXT REFERENCES invoices(id),
    cancel_reason               TEXT,
    validation_status           TEXT CHECK (validation_status IN ('passed','failed','warning')),
    validation_report           TEXT,
    validated_at                TEXT,
    pdf_archive_id              TEXT REFERENCES archive_entries(id),
    xml_archive_id              TEXT REFERENCES archive_entries(id),
    locked_at                   TEXT,
    notes                       TEXT,
    created_at                  TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

INSERT INTO invoices_new SELECT * FROM invoices;

-- Indexe + Trigger neu anlegen
DROP INDEX IF EXISTS idx_invoices_fiscal_year;
DROP INDEX IF EXISTS idx_invoices_status;
DROP INDEX IF EXISTS idx_invoices_date;
DROP INDEX IF EXISTS idx_invoices_paid_at;
DROP INDEX IF EXISTS idx_invoices_contact;
DROP INDEX IF EXISTS idx_invoices_locked;
DROP TRIGGER IF EXISTS trg_invoices_immutable;

DROP TABLE invoices;
ALTER TABLE invoices_new RENAME TO invoices;

CREATE INDEX idx_invoices_fiscal_year ON invoices(fiscal_year);
CREATE INDEX idx_invoices_status ON invoices(status);
CREATE INDEX idx_invoices_date ON invoices(invoice_date);
CREATE INDEX idx_invoices_paid_at ON invoices(paid_at);
CREATE INDEX idx_invoices_contact ON invoices(contact_id);
CREATE INDEX idx_invoices_locked ON invoices(locked_at);

-- Re-create Immutability-Trigger (Self-Reference funktioniert nach RENAME).
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

PRAGMA foreign_keys = ON;

UPDATE app_settings
   SET value = '3',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
