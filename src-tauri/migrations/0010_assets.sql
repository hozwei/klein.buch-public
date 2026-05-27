-- Block 12 (Phase 2C): Anlagenverzeichnis.
--
-- Eine Anlage (Wirtschaftsgut) mit AfA-Methode, Privatanteil, laufendem
-- Restbuchwert und Veräußerungs-Feldern. Die Anlage ist bis zur ersten
-- AfA-Buchung editierbar (locked_at IS NULL); danach sperrt
-- trg_assets_immutable die Kernfelder (GoBD-Hardline).
--
-- Hinweis: Die PRD-Erstfassung nannte diese Migration `0006_assets.sql`. Da die
-- Phase-2B-Blöcke 9/10 die Nummern 0007–0009 belegt haben, läuft Phase 2C um
-- vier Nummern verschoben (0010/0011) — Inhalt unverändert zur PRD-Spezifikation.

CREATE TABLE assets (
    id                          TEXT PRIMARY KEY NOT NULL,
    asset_number                TEXT NOT NULL UNIQUE,        -- "AV-2026-0001"
    label                       TEXT NOT NULL,               -- "MacBook Pro 14, 2024"
    -- Anschaffung
    acquisition_date            TEXT NOT NULL,
    acquisition_cost_cents      INTEGER NOT NULL,            -- Netto-Anschaffungskosten (voll, vor Privatanteil)
    acquisition_fiscal_year     INTEGER NOT NULL,
    expense_id                  TEXT REFERENCES expenses(id),  -- Quell-Kosten-Beleg (optional)
    vendor_contact_id           TEXT REFERENCES contacts(id),
    -- AfA-Methode
    depreciation_method         TEXT NOT NULL CHECK (depreciation_method IN
                                ('gwg_sofort','linear','computer_special_2021')),
    useful_life_years           REAL,                        -- bei 'linear' Pflicht; 'computer_special_2021' = 1; 'gwg_sofort' = NULL
    afa_category                TEXT,                        -- BMF-Kategorie-Code (z. B. "computer_hardware")
    -- Privatanteil (0-100)
    business_share_percent      REAL NOT NULL DEFAULT 100.0
                                CHECK (business_share_percent >= 0 AND business_share_percent <= 100),
    -- Restbuchwert (laufend gepflegt durch AfA-Buchung)
    book_value_cents            INTEGER NOT NULL,            -- Start = acquisition_cost (anteilig business_share)
    last_depreciation_year      INTEGER,
    -- Veräußerung / Verschrottung
    disposed                    INTEGER NOT NULL DEFAULT 0 CHECK (disposed IN (0,1)),
    disposal_date               TEXT,
    disposal_type               TEXT CHECK (disposal_type IN ('sale','scrap','given_away')),
    disposal_proceeds_cents     INTEGER,                     -- bei sale: Verkaufserlös; sonst 0
    disposal_residual_book_value_cents INTEGER,              -- Snapshot beim Disposal
    -- Lock
    locked_at                   TEXT,                        -- Lock nach erster AfA-Berechnung
    notes                       TEXT,
    created_at                  TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

CREATE INDEX idx_assets_acquisition_year ON assets(acquisition_fiscal_year);
CREATE INDEX idx_assets_disposed ON assets(disposed);
CREATE INDEX idx_assets_category ON assets(afa_category);

-- GoBD-Hardline: nach dem Lock (erste AfA-Buchung) sind die Kernfelder
-- unveränderlich. Veräußerungs-/Restbuchwert-Felder sind bewusst NICHT in der
-- Whitelist — Disposal + book_value-Fortschreibung müssen auch nach Lock gehen.
CREATE TRIGGER trg_assets_immutable BEFORE UPDATE ON assets
WHEN OLD.locked_at IS NOT NULL
  AND (NEW.asset_number != OLD.asset_number
    OR NEW.acquisition_date != OLD.acquisition_date
    OR NEW.acquisition_cost_cents != OLD.acquisition_cost_cents
    OR NEW.depreciation_method != OLD.depreciation_method
    OR NEW.useful_life_years IS NOT OLD.useful_life_years
    OR NEW.business_share_percent != OLD.business_share_percent)
BEGIN SELECT RAISE(ABORT, 'asset is locked: core fields immutable'); END;

UPDATE app_settings SET value = '10' WHERE key = 'schema_version';
