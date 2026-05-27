-- Migration 0008: Privatentnahmen / -einlagen (private_movements).
-- Phase 2B, Block 9.
--
-- ABWEICHUNG vom Original-PRD (0005_private_movements.sql):
--   * Migrations-Nummer 0008 statt 0005. schema_version → 8.
--   * Zusätzlicher trg_private_movements_immutable-Trigger (im PRD nicht
--     vorgesehen): defense-in-depth, konsistent mit der GoBD-Hardline
--     (festgeschriebene Belege unveränderlich). Privatbewegungen erhalten
--     PV-Belegnummern und sind ein Lock-Event (Backup-Hook), daher hier ein
--     locked_at + Trigger.
--
-- Fachlicher Hintergrund:
--   * Privatbewegungen sind EÜR-NEUTRAL — sie tauchen NICHT in der EÜR auf
--     (Block 13 klammert sie aus). Sie dienen nur der Vollständigkeit der Kasse
--     (Geld rein/raus zwischen Geschäft und Privat).
--   * amount_cents ist für entnahme UND einlage positiv (Richtung über
--     movement_type).
--   * Kein status/cancel: die Tabelle hat (PRD-konform) keine Storno-Spalten.
--     Korrektur = Gegenbewegung (eine Einlage neutralisiert eine versehentliche
--     Entnahme), append-only.

PRAGMA foreign_keys = ON;

CREATE TABLE private_movements (
    id                  TEXT PRIMARY KEY NOT NULL,
    movement_number     TEXT NOT NULL UNIQUE,                -- "PV-2026-0001"
    fiscal_year         INTEGER NOT NULL,
    movement_date       TEXT NOT NULL,
    movement_type       TEXT NOT NULL CHECK (movement_type IN ('entnahme','einlage')),
    amount_cents        INTEGER NOT NULL,                    -- positiv für beide
    account_id          TEXT REFERENCES payment_accounts(id),
    description         TEXT NOT NULL,
    receipt_archive_id  TEXT REFERENCES archive_entries(id),
    locked_at           TEXT,
    notes               TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_private_movements_fiscal_year ON private_movements(fiscal_year);
CREATE INDEX idx_private_movements_date ON private_movements(movement_date);

-- GoBD-Immutability (defense-in-depth, über PRD hinaus): Kernfelder nach Lock fix.
-- Mutabel bleiben: receipt_archive_id (nachträglicher Beleg) + notes.
CREATE TRIGGER trg_private_movements_immutable BEFORE UPDATE ON private_movements
WHEN OLD.locked_at IS NOT NULL
  AND (NEW.movement_number != OLD.movement_number
    OR NEW.movement_date != OLD.movement_date
    OR NEW.amount_cents != OLD.amount_cents
    OR NEW.movement_type != OLD.movement_type
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.account_id != OLD.account_id)
BEGIN SELECT RAISE(ABORT, 'private_movement is locked: core fields immutable'); END;

UPDATE app_settings
   SET value = '8',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
