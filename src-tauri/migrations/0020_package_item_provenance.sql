-- Migration 0020: Paket-Provenienz auf Beleg-Positionen (Block P3).
-- Verknüpft Beleg-Positionen mit ihrer Paket-Revision (Provenienz) und trägt
-- optionales Markup für formatierte Positionen. Rein additiv: alle neuen Spalten
-- nullable, NULL = exaktes Alt-Verhalten. KEINE Trigger-Änderung an den Items.
--
-- Semantik (Manuel-Entscheidung 2026-05-23 — „Titel + Body nur PDF, XML = Body"):
--   description         = Klartext für XRechnung-BT-154 UND als PDF-Fallback.
--                         Bei Paketen = Klartext(body_markup) (ohne Titel);
--                         der einvoice::generator nutzt UNVERÄNDERT diese Spalte.
--   description_markup  = optional, treibt NUR den PDF-Block (volle Breite).
--                         Bei Paketen = fetter Titel + body_markup → die Vorlagen
--                         rendern ihn via to_typst()/eval(mode:"markup"). NULL =
--                         schmale Beschreibungs-Zelle wie bisher (Alt-Verhalten).
--   source_package_*    = reiner Soft-Zeiger (KEIN FK); beim „Paket anpassen" wird
--                         er auf NULL gesetzt = vollständiger Bruch → reine
--                         Custom-Position. Auflösung über
--                         (source_package_id, source_package_revision) → eindeutige
--                         Zeile in package_revisions (uq_package_revisions).
--
-- Warum kein FK auf source_package_id: der Zeiger wird beim Entkoppeln auf NULL
-- gesetzt und ein Paket kann später archiviert werden — eine harte FK erzeugte nur
-- Reibung; außerdem unterstützt SQLite kein REFERENCES in ALTER TABLE ADD COLUMN.
--
-- schema_version → 20. Migrationen 0001–0019 verbraucht; 0020 ist die nächste.
PRAGMA foreign_keys = ON;

ALTER TABLE invoice_items ADD COLUMN description_markup      TEXT;
ALTER TABLE invoice_items ADD COLUMN source_package_id       TEXT;     -- soft pointer, NULL = custom/detached
ALTER TABLE invoice_items ADD COLUMN source_package_revision INTEGER;  -- Snapshot der Versionsnummer

ALTER TABLE quote_items   ADD COLUMN description_markup      TEXT;
ALTER TABLE quote_items   ADD COLUMN source_package_id       TEXT;
ALTER TABLE quote_items   ADD COLUMN source_package_revision INTEGER;

UPDATE app_settings SET value = '20', updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
