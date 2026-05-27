-- Migration 0017: „Konten" als einzige Quelle der Bankdaten für Belege.
--
-- show_on_invoice: pro Konto wählbar — MEHRERE erlaubt. Alle so geflaggten,
-- aktiven Konten erscheinen auf Rechnung/Angebot (Bank: Kontoinhaber/IBAN/BIC;
-- Nicht-Bank wie PayPal: `details`). `details`: freie Zahlungsadresse/Referenz
-- für Nicht-Bank-Konten.
--
-- Bestehende Firmendaten-IBAN/BIC (seller_profile) werden einmalig aufs
-- Standard-Bankkonto übernommen und dieses auf Belegen angezeigt — damit beim
-- Umzug der Bankdaten nach „Konten" nichts verloren geht.
--
-- schema_version → 17. Migrationen 0001–0016 verbraucht; 0017 ist die nächste.

PRAGMA foreign_keys = ON;

ALTER TABLE payment_accounts ADD COLUMN show_on_invoice INTEGER NOT NULL DEFAULT 0;
ALTER TABLE payment_accounts ADD COLUMN details TEXT;

-- Firmen-IBAN/BIC aufs Standard-Bankkonto übernehmen, falls dort noch leer.
UPDATE payment_accounts
   SET iban = COALESCE(NULLIF(TRIM(COALESCE(iban, '')), ''), (SELECT iban FROM seller_profile WHERE id = 1)),
       bic  = COALESCE(NULLIF(TRIM(COALESCE(bic, '')), ''),  (SELECT bic  FROM seller_profile WHERE id = 1))
 WHERE is_default = 1 AND "type" = 'bank';

-- Standard-Bankkonto mit IBAN automatisch auf Belegen anzeigen.
UPDATE payment_accounts
   SET show_on_invoice = 1
 WHERE is_default = 1 AND "type" = 'bank'
   AND iban IS NOT NULL AND TRIM(iban) != '';

UPDATE app_settings
   SET value = '17', updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
