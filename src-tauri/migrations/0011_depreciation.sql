-- Block 12 (Phase 2C): AfA-Buchungen.
--
-- Eine Zeile pro Anlage pro Geschäftsjahr (Jahres-AfA-Buchung). Wird durch die
-- AfA-Buchungs-Shell (depreciation::accrue_yearly) erzeugt und sofort gelockt
-- (GoBD-Hardline: AfA-Buchungen sind nach Lock unveränderlich). UNIQUE über
-- (asset_id, fiscal_year) macht den Buchungslauf idempotent.
--
-- Nummern-Verschiebung wie 0010_assets: PRD nannte `0007_depreciation.sql`.

CREATE TABLE depreciation_entries (
    id                          TEXT PRIMARY KEY NOT NULL,
    asset_id                    TEXT NOT NULL REFERENCES assets(id),
    fiscal_year                 INTEGER NOT NULL,
    depreciation_amount_cents   INTEGER NOT NULL,            -- AfA dieses Jahres (anteilig business_share)
    months_in_year              INTEGER NOT NULL,            -- 1-12 (im Anschaffungsjahr ggf. weniger)
    book_value_before_cents     INTEGER NOT NULL,
    book_value_after_cents      INTEGER NOT NULL,
    is_full_writeoff            INTEGER NOT NULL DEFAULT 0,  -- bei GWG oder Computer-Sonderregel
    computed_at                 TEXT NOT NULL DEFAULT (datetime('now','utc')),
    locked_at                   TEXT,                        -- Lock zur GJ-Wende / bei Buchung
    UNIQUE (asset_id, fiscal_year)
) STRICT;

CREATE INDEX idx_depreciation_fiscal_year ON depreciation_entries(fiscal_year);
CREATE INDEX idx_depreciation_asset ON depreciation_entries(asset_id);

CREATE TRIGGER trg_depreciation_immutable BEFORE UPDATE ON depreciation_entries
WHEN OLD.locked_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'depreciation entry is locked'); END;

UPDATE app_settings SET value = '11' WHERE key = 'schema_version';
