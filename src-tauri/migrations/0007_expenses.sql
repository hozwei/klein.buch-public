-- Migration 0007: Kosten (expenses) + Zahlungs-Konten (payment_accounts).
-- Phase 2B, Block 9.
--
-- ABWEICHUNG vom Original-PRD (0003_expenses.sql):
--   * Migrations-Nummer 0007 statt 0003 (0001–0006 verbraucht; R4-Revision +
--     Block 8 legal_documents). schema_version → 7.
--   * Recurring (PRD 0004) folgt separat in Block 10; private_movements
--     (PRD 0005) als 0008 im selben Block 9.
--
-- Fachlicher Hintergrund:
--   * Kosten sind die EINGANGS-Seite. Die §19-Hardline (kein USt-Ausweis) gilt
--     NUR für ausgehende Belege (Rechnungen/Angebote). Eingangsrechnungen von
--     Lieferanten DÜRFEN USt enthalten — ein Kleinunternehmer zahlt sie, kann
--     aber keine Vorsteuer ziehen. Für die EÜR (Block 13, Cash-Basis) zählt
--     daher der BRUTTO-Betrag (gross_amount_cents) am Zahlungsausgang (paid_date).
--   * §13b Reverse-Charge ist nur ein Hinweis-Flag (keine USt-Auto-Berechnung;
--     PRD G16: "Berechnung extern").
--   * GoBD-Hardline: festgeschriebene Kosten (recorded + locked_at) sind auf den
--     Kernfeldern unveränderlich (trg_expenses_immutable). Korrektur = Storno
--     (status='canceled' + neue korrigierte Kosten), nie Update/Löschung.

PRAGMA foreign_keys = ON;

-- Zahlungs-Konten (1 Bank-Konto + optional Bargeld/PayPal/…).
CREATE TABLE payment_accounts (
    id          TEXT PRIMARY KEY NOT NULL,
    label       TEXT NOT NULL,                    -- "Hauptkonto", "Bargeld-Kasse"
    type        TEXT NOT NULL CHECK (type IN ('bank','cash','paypal','stripe','other')),
    iban        TEXT,
    bic         TEXT,
    is_default  INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0,1)),
    active      INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
    created_at  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

CREATE TABLE expenses (
    id                          TEXT PRIMARY KEY NOT NULL,
    expense_number              TEXT NOT NULL UNIQUE,        -- "KO-2026-0001"
    fiscal_year                 INTEGER NOT NULL,
    expense_date                TEXT NOT NULL,               -- Beleg-Datum
    paid_date                   TEXT,                        -- Zahlungsausgang (cash-basis!)
    paid_from_account_id        TEXT REFERENCES payment_accounts(id),
    vendor_contact_id           TEXT REFERENCES contacts(id),
    vendor_name_snapshot        TEXT NOT NULL,
    vendor_invoice_number       TEXT,
    -- EÜR-Kategorie (BMF-orientiert)
    category                    TEXT NOT NULL CHECK (category IN
                                ('office','software','hardware','travel','services','goods',
                                 'communications','vehicle','rent','insurance','training',
                                 'fees','marketing','other')),
    description                 TEXT NOT NULL,
    net_amount_cents            INTEGER NOT NULL,
    tax_amount_cents            INTEGER NOT NULL DEFAULT 0,
    gross_amount_cents          INTEGER NOT NULL,
    currency_code               TEXT NOT NULL DEFAULT 'EUR',
    -- §13b Reverse-Charge Hinweis-Flag (kein USt-Auto-Calc)
    reverse_charge_13b          INTEGER NOT NULL DEFAULT 0 CHECK (reverse_charge_13b IN (0,1)),
    -- Beleg-Archiv (write-once, primärer Beleg)
    receipt_archive_id          TEXT REFERENCES archive_entries(id),
    -- E-Rechnung-Validierung (wenn empfangen, Block 11)
    einvoice_validation_status  TEXT,
    einvoice_validation_report  TEXT,
    -- Recurring-Verknüpfung (Block 10; plain TEXT, FK kommt später)
    recurring_subscription_id   TEXT,
    -- Asset-Verknüpfung (Phase 2C; plain TEXT, FK kommt später)
    capitalized_as_asset_id     TEXT,
    -- Status
    status                      TEXT NOT NULL DEFAULT 'recorded'
                                CHECK (status IN ('recorded','canceled')),
    canceled_at                 TEXT,
    canceled_reason             TEXT,
    locked_at                   TEXT,
    notes                       TEXT,
    created_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_expenses_fiscal_year ON expenses(fiscal_year);
CREATE INDEX idx_expenses_paid_date ON expenses(paid_date);
CREATE INDEX idx_expenses_status ON expenses(status);
CREATE INDEX idx_expenses_category ON expenses(category);

-- GoBD-Immutability: nach Lock (locked_at gesetzt) sind Kernfelder fix.
-- Erlaubt bleiben: status (cancel), canceled_at, canceled_reason, paid_date,
-- paid_from_account_id, receipt_archive_id, einvoice_*, recurring/asset-Verknüpfung,
-- notes. (tax_amount_cents bewusst nicht im Schutz — siehe PRD-Original.)
CREATE TRIGGER trg_expenses_immutable BEFORE UPDATE ON expenses
WHEN OLD.locked_at IS NOT NULL
  AND (NEW.expense_number != OLD.expense_number
    OR NEW.expense_date != OLD.expense_date
    OR NEW.net_amount_cents != OLD.net_amount_cents
    OR NEW.gross_amount_cents != OLD.gross_amount_cents
    OR NEW.vendor_contact_id != OLD.vendor_contact_id)
BEGIN SELECT RAISE(ABORT, 'expense is locked: core fields immutable'); END;

-- Zahlungs-Konto auf Invoices (Zahlungseingang). Spalte additiv; Verdrahtung in
-- den Zahlungs-Workflow folgt bei Bedarf — Block 9 legt nur die Spalte an.
ALTER TABLE invoices ADD COLUMN paid_to_account_id TEXT REFERENCES payment_accounts(id);

UPDATE app_settings
   SET value = '7',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
