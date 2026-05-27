-- Migration 0023: Buyer-Snapshot auf quotes (Block 19, DSGVO Art. 17 — Parität zu invoices).
--
-- Analog zu Migration 0004 (invoices): friert den Empfänger-Stand zur Angebots-
-- zeit ein. Ohne den Snapshot würde ein festgeschriebenes/versendetes Angebot
-- beim erneuten Rendern (`ensure_quote_pdf`) auf den Live-Kontakt zurückfallen
-- und nach einer Anonymisierung plötzlich "Anonymisiert" zeigen.
--
-- Nullable + additiv (ADD COLUMN bei STRICT erlaubt). Der Immutability-Trigger
-- `trg_quotes_immutable` schützt nur Kernfelder (Nummer/Datum/Beträge/
-- contact_id/fiscal_year/is_kleinunternehmer) — NICHT die buyer_*-Spalten;
-- der Backfill unten ist daher auch auf gelockten Angeboten zulässig (er friert
-- den ohnehin angezeigten Stand ein, das archivierte PDF bleibt unverändert).

ALTER TABLE quotes ADD COLUMN buyer_name         TEXT;
ALTER TABLE quotes ADD COLUMN buyer_street       TEXT;
ALTER TABLE quotes ADD COLUMN buyer_postal_code  TEXT;
ALTER TABLE quotes ADD COLUMN buyer_city         TEXT;
ALTER TABLE quotes ADD COLUMN buyer_country_code TEXT;
ALTER TABLE quotes ADD COLUMN buyer_vat_id       TEXT;
ALTER TABLE quotes ADD COLUMN buyer_email        TEXT;

-- Bestandsdaten aus dem aktuellen Kontaktstand einfrieren, solange die Kontakte
-- noch im Original vorliegen (vor jeder möglichen Anonymisierung).
UPDATE quotes
   SET buyer_name         = (SELECT c.name         FROM contacts c WHERE c.id = quotes.contact_id),
       buyer_street       = (SELECT c.street        FROM contacts c WHERE c.id = quotes.contact_id),
       buyer_postal_code  = (SELECT c.postal_code   FROM contacts c WHERE c.id = quotes.contact_id),
       buyer_city         = (SELECT c.city          FROM contacts c WHERE c.id = quotes.contact_id),
       buyer_country_code = (SELECT c.country_code   FROM contacts c WHERE c.id = quotes.contact_id),
       buyer_vat_id       = (SELECT c.vat_id         FROM contacts c WHERE c.id = quotes.contact_id),
       buyer_email        = (SELECT c.email          FROM contacts c WHERE c.id = quotes.contact_id)
 WHERE buyer_name IS NULL
   AND contact_id IN (SELECT id FROM contacts);

UPDATE app_settings
   SET value = '23',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
