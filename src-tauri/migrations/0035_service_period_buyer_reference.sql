-- Migration 0035: Leistungszeitraum und Leitweg-ID am Beleg (ADR 0042 Nr. 10–12,
-- Lauf 2026.11.1, Block V3).
--
-- Bisher kannte eine Rechnung nur ein Leistungsdatum (`delivery_date`, BT-72).
-- Jetzt kann sie statt dessen einen Leistungszeitraum tragen
-- (§ 14 Abs. 4 Satz 1 Nr. 6 UStG, § 31 Abs. 4 UStDV; EN 16931 BG-14 mit
-- BT-73/BT-74): `service_period_start` und `service_period_end`, beide oder
-- keiner, Beginn nicht nach dem Ende, nie zugleich mit einem Leistungsdatum.
-- Die Domäne prüft dasselbe mit Meldung am Feld; die CHECKs sind die zweite
-- Linie.
--
-- Die Leitweg-ID bzw. Käuferreferenz (BT-10) stand bisher nur in der
-- archivierten E-Rechnung; am Entwurf ging sie verloren (ADR 0041 Nr. 91).
-- Jetzt steht sie in `invoices.buyer_reference` (NULL = keine, im XML „N/A“)
-- und an der Abo-Vorlage (`recurring_invoices.buyer_reference`), die sie an
-- jede erzeugte Rechnung weitergibt. Der Storno liest sie weiter aus der
-- archivierten E-Rechnung des Originals (ADR 0041 Nr. 250).
--
-- Den Zeitraum einer Abo-Rechnung leitet der Lauf aus Stichtag und Frequenz
-- ab (Stichtag bis Stichtag + Periode − 1 Tag); die Vorlage braucht dafür
-- keine eigene Spalte.
--
-- GoBD: Zeitraum und Leitweg-ID sind nach dem Festschreiben unveränderlich.
-- `trg_invoices_immutable` (0028) und `trg_invoices_voided_final` (0032)
-- werden dazu mit allen bisherigen Bedingungen und den neuen Spalten neu
-- angelegt (nur verschärft, nichts gelockert). Das Festschreiben setzt die
-- wirksame Leitweg-ID in derselben Anweisung wie `locked_at`.

ALTER TABLE invoices ADD COLUMN service_period_start TEXT
    CHECK (service_period_start IS NULL OR date(service_period_start) IS service_period_start);
ALTER TABLE invoices ADD COLUMN service_period_end TEXT
    CHECK (service_period_end IS NULL OR date(service_period_end) IS service_period_end)
    CHECK ((service_period_start IS NULL) = (service_period_end IS NULL))
    CHECK (service_period_end IS NULL OR service_period_start <= service_period_end)
    CHECK (service_period_end IS NULL OR delivery_date IS NULL);
ALTER TABLE invoices ADD COLUMN buyer_reference TEXT
    CHECK (buyer_reference IS NULL OR length(buyer_reference) BETWEEN 1 AND 200);

ALTER TABLE recurring_invoices ADD COLUMN buyer_reference TEXT
    CHECK (buyer_reference IS NULL OR length(buyer_reference) BETWEEN 1 AND 200);

-- ============================================================================
-- Festgeschriebene Rechnung: Whitelist aus 0028 plus Zeitraum und Leitweg-ID
-- ============================================================================
DROP TRIGGER trg_invoices_immutable;

CREATE TRIGGER trg_invoices_immutable BEFORE UPDATE ON invoices
WHEN OLD.locked_at IS NOT NULL
  AND (
    -- Unlock-Schutz (R1-005)
    NEW.locked_at IS NULL
    OR NEW.locked_at != OLD.locked_at
    -- Bestehende Kernfelder (0001/0003)
    OR NEW.invoice_number != OLD.invoice_number
    OR NEW.invoice_date != OLD.invoice_date
    OR NEW.net_amount_cents != OLD.net_amount_cents
    OR NEW.gross_amount_cents != OLD.gross_amount_cents
    OR NEW.tax_amount_cents != OLD.tax_amount_cents
    OR NEW.contact_id != OLD.contact_id
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.is_kleinunternehmer != OLD.is_kleinunternehmer
    OR NEW.direction != OLD.direction
    -- §14-Pflichtangaben + Stamm-Daten-Snapshot (R1-004)
    OR NEW.delivery_date IS NOT OLD.delivery_date
    OR NEW.due_date IS NOT OLD.due_date
    OR NEW.currency_code != OLD.currency_code
    OR NEW.pdf_template != OLD.pdf_template
    -- Storno-Pair-Verkettung (R1-004)
    OR NEW.is_storno_for IS NOT OLD.is_storno_for
    OR NEW.derived_from_quote_id IS NOT OLD.derived_from_quote_id
    -- Konto-Snapshot + Zahlungsvermerk
    OR NEW.paid_to_account_id IS NOT OLD.paid_to_account_id
    OR NEW.payment_note IS NOT OLD.payment_note
    -- Archive-Slots write-once nach Lock
    OR NEW.pdf_archive_id IS NOT OLD.pdf_archive_id
    OR NEW.xml_archive_id IS NOT OLD.xml_archive_id
    -- Buyer-Snapshot (DSGVO-Anonymisierungs-Schutz)
    OR NEW.buyer_name IS NOT OLD.buyer_name
    OR NEW.buyer_street IS NOT OLD.buyer_street
    OR NEW.buyer_postal_code IS NOT OLD.buyer_postal_code
    OR NEW.buyer_city IS NOT OLD.buyer_city
    OR NEW.buyer_country_code IS NOT OLD.buyer_country_code
    OR NEW.buyer_vat_id IS NOT OLD.buyer_vat_id
    OR NEW.buyer_email IS NOT OLD.buyer_email
    -- Seller-Snapshot
    OR NEW.seller_name != OLD.seller_name
    OR NEW.seller_street != OLD.seller_street
    OR NEW.seller_postal_code != OLD.seller_postal_code
    OR NEW.seller_city != OLD.seller_city
    OR NEW.seller_tax_number IS NOT OLD.seller_tax_number
    OR NEW.seller_vat_id IS NOT OLD.seller_vat_id
    -- ADR 0042 Nr. 10–12: Leistungszeitraum (BG-14) und Leitweg-ID (BT-10)
    OR NEW.service_period_start IS NOT OLD.service_period_start
    OR NEW.service_period_end IS NOT OLD.service_period_end
    OR NEW.buyer_reference IS NOT OLD.buyer_reference
  )
BEGIN SELECT RAISE(ABORT, 'invoice is locked: immutable fields cannot change after lock'); END;

-- ============================================================================
-- Verworfener Entwurf: Spaltenliste aus 0032 plus Zeitraum und Leitweg-ID
-- ============================================================================
-- Die Anonymisierung (Art. 17 DSGVO) lässt Zeitraum und Leitweg-ID stehen:
-- Die Leitweg-ID adressiert eine Behörde, keine Person.
DROP TRIGGER trg_invoices_voided_final;

CREATE TRIGGER trg_invoices_voided_final BEFORE UPDATE ON invoices
WHEN OLD.voided_at IS NOT NULL
  AND NOT (
        (SELECT c.anonymized_at IS NOT NULL AND NEW.buyer_name IS c.name
           FROM contacts c WHERE c.id = OLD.contact_id) IS 1
    AND NEW.buyer_street IS NULL
    AND NEW.buyer_postal_code IS NULL
    AND NEW.buyer_city IS NULL
    AND NEW.buyer_vat_id IS NULL
    AND NEW.buyer_email IS NULL
    AND NEW.notes IS NULL
    AND NEW.id IS OLD.id
    AND NEW.invoice_number IS OLD.invoice_number
    AND NEW.fiscal_year IS OLD.fiscal_year
    AND NEW.direction IS OLD.direction
    AND NEW.invoice_date IS OLD.invoice_date
    AND NEW.delivery_date IS OLD.delivery_date
    AND NEW.due_date IS OLD.due_date
    AND NEW.contact_id IS OLD.contact_id
    AND NEW.seller_name IS OLD.seller_name
    AND NEW.seller_street IS OLD.seller_street
    AND NEW.seller_postal_code IS OLD.seller_postal_code
    AND NEW.seller_city IS OLD.seller_city
    AND NEW.seller_tax_number IS OLD.seller_tax_number
    AND NEW.seller_vat_id IS OLD.seller_vat_id
    AND NEW.net_amount_cents IS OLD.net_amount_cents
    AND NEW.tax_amount_cents IS OLD.tax_amount_cents
    AND NEW.gross_amount_cents IS OLD.gross_amount_cents
    AND NEW.currency_code IS OLD.currency_code
    AND NEW.is_kleinunternehmer IS OLD.is_kleinunternehmer
    AND NEW.pdf_template IS OLD.pdf_template
    AND NEW.status IS OLD.status
    AND NEW.sent_at IS OLD.sent_at
    AND NEW.paid_amount_cents IS OLD.paid_amount_cents
    AND NEW.paid_at IS OLD.paid_at
    AND NEW.payment_history_json IS OLD.payment_history_json
    AND NEW.canceled_at IS OLD.canceled_at
    AND NEW.canceled_by_storno_id IS OLD.canceled_by_storno_id
    AND NEW.is_storno_for IS OLD.is_storno_for
    AND NEW.cancel_reason IS OLD.cancel_reason
    AND NEW.validation_status IS OLD.validation_status
    AND NEW.validation_report IS OLD.validation_report
    AND NEW.validated_at IS OLD.validated_at
    AND NEW.pdf_archive_id IS OLD.pdf_archive_id
    AND NEW.xml_archive_id IS OLD.xml_archive_id
    AND NEW.locked_at IS OLD.locked_at
    AND NEW.created_at IS OLD.created_at
    AND NEW.updated_at IS OLD.updated_at
    AND NEW.buyer_country_code IS OLD.buyer_country_code
    AND NEW.derived_from_quote_id IS OLD.derived_from_quote_id
    AND NEW.paid_to_account_id IS OLD.paid_to_account_id
    AND NEW.payment_note IS OLD.payment_note
    AND NEW.voided_at IS OLD.voided_at
    AND NEW.void_reason IS OLD.void_reason
    AND NEW.service_period_start IS OLD.service_period_start
    AND NEW.service_period_end IS OLD.service_period_end
    AND NEW.buyer_reference IS OLD.buyer_reference
  )
BEGIN SELECT RAISE(ABORT, 'invoice draft is voided: it cannot change any more'); END;

-- ============================================================================
-- Schema-Version 34 → 35
-- ============================================================================
UPDATE app_settings
   SET value = '35',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
