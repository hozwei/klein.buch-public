-- Migration 0005: Angebote (quotes + quote_items). Phase 2A, Block 6.
--
-- Angebote sind KEINE E-Rechnungen (kein XRechnung/ZUGFeRD). Sie haben einen
-- eigenen Belegkreis (AN-{YYYY}-{NNNN}, Counter `quote` in doc_number_counters,
-- bereits in domain::numbering vorgesehen).
--
-- Lifecycle (Block 6):
--   draft ──issue/festschreiben──▶ sent ──accept──▶ accepted ──(Block 7)──▶ converted
--                                   │
--                                   ├──reject──▶ rejected
--                                   └──cancel──▶ canceled
-- Drafts (locked_at IS NULL) sind frei änderbar. Mit dem Festschreiben
-- (`issue`) wird `locked_at` gesetzt und Status → 'sent'; ab da greift
-- trg_quotes_immutable auf den Kernfeldern (GoBD-Hardline: "Angebote (sent)
-- sind nach Lock unveränderlich").
--
-- ABWEICHUNG vom Original-PRD (0002_quotes.sql):
--   * Migrations-Nummer 0005 statt 0002 (0001–0004 verbraucht; R4-Revision).
--   * schema_version → 5.
--   * seller_tax_number NULLBAR statt NOT NULL — konsistent mit
--     seller_profile.tax_number (nullbar seit 0002) und invoices.seller_tax_number
--     (nullbar seit 0003, §33-Kleinbetragsrechnung-Compat). Ein Kleinunternehmer
--     im Onboarding hat ggf. noch keine Steuernummer erfasst.
--   * Status-Enum + accepted_at/rejected_at: Annahme-Workflow (Manuel-Feature).

PRAGMA foreign_keys = ON;

CREATE TABLE quotes (
    id                          TEXT PRIMARY KEY NOT NULL,
    quote_number                TEXT NOT NULL UNIQUE,        -- "AN-2026-0001"
    fiscal_year                 INTEGER NOT NULL,
    quote_date                  TEXT NOT NULL,
    valid_until                 TEXT NOT NULL,
    contact_id                  TEXT NOT NULL REFERENCES contacts(id),
    -- Seller-Snapshot (immutable nach Lock)
    seller_name                 TEXT NOT NULL,
    seller_street               TEXT NOT NULL,
    seller_postal_code          TEXT NOT NULL,
    seller_city                 TEXT NOT NULL,
    seller_tax_number           TEXT,
    seller_vat_id               TEXT,
    -- Beträge in Cent
    net_amount_cents            INTEGER NOT NULL,
    tax_amount_cents            INTEGER NOT NULL DEFAULT 0,
    gross_amount_cents          INTEGER NOT NULL,
    currency_code               TEXT NOT NULL DEFAULT 'EUR',
    is_kleinunternehmer         INTEGER NOT NULL DEFAULT 1 CHECK (is_kleinunternehmer IN (0,1)),
    pdf_template                TEXT NOT NULL DEFAULT 'default',
    -- Status-Flow: draft → sent → accepted|rejected → converted | canceled
    status                      TEXT NOT NULL DEFAULT 'draft'
                                CHECK (status IN ('draft','sent','accepted','rejected','canceled','converted')),
    sent_at                     TEXT,
    accepted_at                 TEXT,
    rejected_at                 TEXT,
    canceled_at                 TEXT,
    canceled_reason             TEXT,
    converted_at                TEXT,
    converted_invoice_id        TEXT REFERENCES invoices(id),
    -- Archive (Angebots-PDF kommt in Block 8)
    pdf_archive_id              TEXT REFERENCES archive_entries(id),
    -- Lock
    locked_at                   TEXT,
    notes                       TEXT,
    created_at                  TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_quotes_fiscal_year ON quotes(fiscal_year);
CREATE INDEX idx_quotes_status ON quotes(status);
CREATE INDEX idx_quotes_contact ON quotes(contact_id);

CREATE TABLE quote_items (
    id                  TEXT PRIMARY KEY NOT NULL,
    quote_id            TEXT NOT NULL REFERENCES quotes(id) ON DELETE CASCADE,
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
CREATE UNIQUE INDEX uq_quote_items_position ON quote_items(quote_id, position);

-- GoBD-Immutability: nach Lock (locked_at gesetzt) sind Kernfelder fix.
-- Erlaubt bleiben State-Transitions: status, sent_at, accepted_at, rejected_at,
-- canceled_at, canceled_reason, converted_at, converted_invoice_id,
-- pdf_archive_id, notes, updated_at.
CREATE TRIGGER trg_quotes_immutable BEFORE UPDATE ON quotes
WHEN OLD.locked_at IS NOT NULL
  AND (NEW.quote_number != OLD.quote_number
    OR NEW.quote_date != OLD.quote_date
    OR NEW.net_amount_cents != OLD.net_amount_cents
    OR NEW.gross_amount_cents != OLD.gross_amount_cents
    OR NEW.tax_amount_cents != OLD.tax_amount_cents
    OR NEW.contact_id != OLD.contact_id
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.is_kleinunternehmer != OLD.is_kleinunternehmer)
BEGIN SELECT RAISE(ABORT, 'quote is locked: core fields immutable'); END;

-- Verknüpfung Rechnung → Ursprungsangebot (Konvertierung in Block 7).
ALTER TABLE invoices ADD COLUMN derived_from_quote_id TEXT REFERENCES quotes(id);

UPDATE app_settings
   SET value = '5',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
