-- Migration 0021: Paket-Positions-Titel (Block P3, Nachbesserung).
-- Eigenständiges Titel-Feld auf Beleg-Positionen, damit eine Paket-/Rich-Position
-- wie im Paket-Katalog bearbeitet wird (Titel-Eingabe + Markdown-Body) und der
-- Titel auf dem PDF in der Positionszeile (Beschreibungs-Spalte) steht — nicht
-- als Vorspann im Body-Block.
--
-- Rein additiv: nullable, NULL = exaktes Alt-Verhalten (Custom-Position ohne Titel).
--
-- Zusammenspiel (Manuel-Entscheidung „Titel nur PDF, XML = Body"):
--   description_title  = Positions-Titel; NUR PDF-Zelle, NICHT im XRechnung-XML.
--   description_markup = Body-Markdown (ohne Titel); treibt den PDF-Block.
--   description        = Klartext(body_markup) → XRechnung-BT-154 (generator
--                        unverändert). Wird beim Draft-Save aus dem Markup
--                        neu berechnet, damit das XML auch nach Edits stimmt.
--
-- schema_version → 21. Migrationen 0001–0020 verbraucht; 0021 ist die nächste.
PRAGMA foreign_keys = ON;

ALTER TABLE invoice_items ADD COLUMN description_title TEXT;
ALTER TABLE quote_items   ADD COLUMN description_title TEXT;

UPDATE app_settings SET value = '21', updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
