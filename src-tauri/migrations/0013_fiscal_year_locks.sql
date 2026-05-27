-- Block 15 (Phase 2D) — Geschäftsjahr-Festschreibung (GJ-Lock).
--
-- Festschreibungsprotokoll nach §146 Abs. 4 AO / GoBD: ist ein Geschäftsjahr
-- abgeschlossen, ist es unveränderlich. Diese Tabelle hält den Abschluss-Zeitpunkt
-- und einen Summen-Snapshot (EÜR-Eckwerte zum Zeitpunkt des Abschlusses) fest,
-- damit später nachvollziehbar bleibt, mit welchem Stand festgeschrieben wurde.
--
-- Hard-Lines:
--   * Append-/Close-once: kein UPDATE, kein DELETE (Trigger erzwingen das).
--   * Der eigentliche Schreibschutz auf die Belege des Jahres erfolgt zusätzlich
--     über locked_at auf assets/depreciation_entries (Block 12 vorbereitet) und
--     über den fiscal_year::guard in den Buchungs-Commands.
--   * Storno-Belege bleiben jederzeit möglich (sie tragen das laufende GJ-Datum).

CREATE TABLE fiscal_year_locks (
    fiscal_year                 INTEGER PRIMARY KEY NOT NULL,
    closed_at                   TEXT NOT NULL DEFAULT (datetime('now','utc')),
    income_total_cents          INTEGER NOT NULL,
    expense_total_cents         INTEGER NOT NULL,
    afa_total_cents             INTEGER NOT NULL,
    surplus_cents               INTEGER NOT NULL,
    assets_locked               INTEGER NOT NULL DEFAULT 0,
    depreciation_entries_locked INTEGER NOT NULL DEFAULT 0,
    app_version                 TEXT NOT NULL,
    schema_version              INTEGER NOT NULL,
    notes                       TEXT
) STRICT;

CREATE TRIGGER trg_fiscal_year_locks_no_update BEFORE UPDATE ON fiscal_year_locks
BEGIN SELECT RAISE(ABORT, 'fiscal_year_locks: abgeschlossene Geschäftsjahre sind unveränderlich'); END;

CREATE TRIGGER trg_fiscal_year_locks_no_delete BEFORE DELETE ON fiscal_year_locks
BEGIN SELECT RAISE(ABORT, 'fiscal_year_locks: ein abgeschlossenes Geschäftsjahr kann nicht gelöscht werden'); END;

UPDATE app_settings SET value = '13' WHERE key = 'schema_version';
