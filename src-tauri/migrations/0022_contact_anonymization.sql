-- Migration 0022: DSGVO Art. 17 — Kontakt-Anonymisierung (beschränkt durch §147 AO).
--
-- "Recht auf Vergessen" kollidiert bei steuerrelevanten Daten mit der 10-Jahres-
-- Aufbewahrungspflicht (§147 AO / §14b UStG). Lösung (PRD R4.3): nicht löschen,
-- sondern die personenbezogenen STAMMDATEN des Kontakts überschreiben
-- (name → Platzhalter, alle übrigen Personenfelder NULL) und den Zeitpunkt
-- festhalten. Die rechnungsgebundenen Daten bleiben über den Buyer-Snapshot
-- (Migration 0004 auf invoices, 0023 auf quotes) unverändert erhalten.
--
-- `anonymized_at` ist NULL = aktiver Kontakt, gesetzt = anonymisiert (Datum/Zeit
-- der Anonymisierung). Nullable + additiv; ADD COLUMN ist bei STRICT erlaubt.

ALTER TABLE contacts ADD COLUMN anonymized_at TEXT;

-- GoBD-Härtung des Buyer-Snapshots (Migration 0004): Alt-Rechnungen, die VOR
-- Einführung des Snapshots erstellt wurden, haben buyer_* = NULL und würden
-- nach einer Anonymisierung auf den (dann überschriebenen) Live-Kontakt
-- zurückfallen. Wir frieren ihren Empfänger-Stand jetzt — solange die Kontakte
-- noch im Original vorliegen — aus den aktuellen Stammdaten ein. Nur AUSGANGS-
-- rechnungen (issued); Eingangsrechnungen tragen den Lieferanten, deren Beleg
-- ist die archivierte Original-Datei.
UPDATE invoices
   SET buyer_name         = (SELECT c.name         FROM contacts c WHERE c.id = invoices.contact_id),
       buyer_street       = (SELECT c.street       FROM contacts c WHERE c.id = invoices.contact_id),
       buyer_postal_code  = (SELECT c.postal_code  FROM contacts c WHERE c.id = invoices.contact_id),
       buyer_city         = (SELECT c.city         FROM contacts c WHERE c.id = invoices.contact_id),
       buyer_country_code = (SELECT c.country_code  FROM contacts c WHERE c.id = invoices.contact_id),
       buyer_vat_id       = (SELECT c.vat_id        FROM contacts c WHERE c.id = invoices.contact_id),
       buyer_email        = (SELECT c.email         FROM contacts c WHERE c.id = invoices.contact_id)
 WHERE direction = 'issued'
   AND buyer_name IS NULL
   AND contact_id IN (SELECT id FROM contacts);

UPDATE app_settings
   SET value = '22',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
