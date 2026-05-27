-- Migration 0004: Buyer-Snapshot auf invoices.
--
-- Bisher hielt invoices nur einen Seller-Snapshot (seller_*); der Empfänger
-- wurde live aus contacts geladen. Das ist eine GoBD- UND DSGVO-Schwäche:
-- - GoBD: die strukturierte Rechnungssicht sollte den Empfänger-Stand zur
--   Rechnungszeit einfrieren (konsistent zum archivierten PDF/XML).
-- - DSGVO: nach Anonymisierung eines Kontakts (Recht auf Vergessen, beschränkt
--   durch §147 AO) darf die Rechnung nicht plötzlich "Anonymisiert" zeigen —
--   der Snapshot hält den Originalstand fest.
--
-- Nullable, weil Drafts den Snapshot erst beim Anlegen befüllen und
-- Altbestände (vor dieser Migration) keinen haben. ADD COLUMN ist bei
-- STRICT-Tabellen erlaubt.

ALTER TABLE invoices ADD COLUMN buyer_name         TEXT;
ALTER TABLE invoices ADD COLUMN buyer_street       TEXT;
ALTER TABLE invoices ADD COLUMN buyer_postal_code  TEXT;
ALTER TABLE invoices ADD COLUMN buyer_city         TEXT;
ALTER TABLE invoices ADD COLUMN buyer_country_code TEXT;
ALTER TABLE invoices ADD COLUMN buyer_vat_id       TEXT;
ALTER TABLE invoices ADD COLUMN buyer_email        TEXT;

UPDATE app_settings
   SET value = '4',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
