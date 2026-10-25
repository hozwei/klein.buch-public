-- Migration 0032: GoBD- und Schema-Absicherungen (Lauf F, Block F2a).
--
-- Die einzige Migration von Lauf F. Sie verschärft nur (kein Trigger wird
-- gelockert) und ändert keine festgeschriebenen Belege:
--
--   * LE-STORNO-UNIQUE   höchstens ein festgeschriebener Storno je Original
--                        (Partial Unique Index). Gibt es im Bestand schon zwei,
--                        bricht die Migration mit klarer Meldung ab — die
--                        Belege sind unveränderlich, bereinigen ließe sich das
--                        nur durch Ändern festgeschriebener Belege. Seit F2c
--                        lehnt das Entsperren das schon vorher ab
--                        (db::precheck_before_migration, vor dem Pflicht-Backup
--                        und vor jeder Migration); der CHECK hier ist die
--                        zweite Linie: die Transaktion rollt zurück, die
--                        Instanz bleibt gesperrt (ADR 0039 Nr. 124, 126).
--   * W2-INTEGRITY-UNREADABLE  Integritäts-Historie mit „fehlt“ und „nicht
--                        lesbar“ (NULL = vor 0032 nicht erfasst).
--   * W14-032            Verwerfen nummerierter Rechnungs-Entwürfe ohne
--                        Löschen: voided_at + void_reason, danach endgültig.
--                        Angebots-Entwürfe verwirft das bestehende
--                        „Zurückziehen“ (status = 'canceled'); ein
--                        zurückgezogenes Angebot ist jetzt ebenfalls endgültig.
--   * KB-0002            Positionen festgeschriebener (und verworfener)
--                        Rechnungen bzw. festgeschriebener oder zurückgezogener
--                        Angebote sind unveränderlich.
--   * KB-0004            archive_entries ohne DELETE (write-once).
--   * R1-013             höchstens ein Standard-Zahlungskonto (Bestand vorher
--                        bereinigt, mit Audit).
--   * R1-S2-ASSET-DISPOSAL  Abgangsfelder nach der Veräußerung unveränderlich;
--                        seit F2c auch Stammdaten, Restbuchwert und die
--                        AfA-Buchungen einer veräußerten Anlage.
--   * R1-S2-MARKCANCELED Storno einer Rechnung ist endgültig (Status und
--                        Storno-Verweis ändern sich danach nicht mehr).
--   * F2c                Ein in eine Rechnung umgewandeltes Angebot bleibt
--                        umgewandelt; nur das Verwerfen des daraus
--                        entstandenen Entwurfs macht die Umwandlung rückgängig.
--   * F2c (ADR 0039 Nr. 124 (3)) Einzige Ausnahme von „verworfen ist
--                        endgültig“: die Anonymisierung des Kontakts
--                        (Art. 17 DSGVO) entfernt den Empfänger-Snapshot eines
--                        nie ausgestellten, verworfenen Entwurfs.
--   * F1 (ADR 0040 Nr. 8) Seed der abschaltbaren Regel rule_recurring_due.
--
-- App-Pfade, die weiter durchkommen müssen (geprüft in
-- tests/migration_0032_test.rs):
--   invoices:  Entwurf anlegen (INSERT ohne voided_*), Festschreiben
--              (locked_at NULL → gesetzt), Storno-Paar (Original einmal auf
--              'canceled'), Versand/Zahlung auf nicht stornierten Rechnungen,
--              Verwerfen eines offenen Entwurfs mit Grund, Anonymisierung
--              des Kontakts eines verworfenen Entwurfs (nur buyer_* außer
--              Land und notes, nur auf die Werte der Anonymisierung).
--   invoice_items/quote_items: INSERT beim Anlegen des Entwurfs.
--   quotes:    Zurückziehen (einmal), ensure_quote_pdf (pdf_archive_id),
--              Umwandlung zurücknehmen beim Verwerfen des Entwurfs.
--   assets:    dispose (einmal, disposed 0 → 1), AfA-Fortschreibung vor dem
--              Abgang, Festschreiben (locked_at) auch nach dem Abgang.
--   payment_accounts: ensure_one_default (erst räumen, dann befördern).

PRAGMA foreign_keys = ON;

-- ============================================================================
-- Vorab-Prüfung: höchstens ein festgeschriebener Storno je Original
-- ============================================================================
-- SQLite kennt außerhalb von Triggern kein bedingtes RAISE. Ein benannter
-- CHECK auf einer temporären Tabelle bricht mit genau diesem Namen ab
-- („CHECK constraint failed: Migration 0032 …“) — das ist die Meldung im
-- Server-Log.
CREATE TEMP TABLE m0032_precheck (
    duplicate_locked_stornos INTEGER NOT NULL
        CONSTRAINT "Migration 0032 abgebrochen: Zu mindestens einer Rechnung gibt es mehr als einen festgeschriebenen Storno-Beleg. Es wurde nichts umgestellt (Handbuch: Fehlersuche im Container)."
        CHECK (duplicate_locked_stornos = 0)
) STRICT;

INSERT INTO m0032_precheck (duplicate_locked_stornos)
SELECT COUNT(*) FROM (
    SELECT is_storno_for
      FROM invoices
     WHERE is_storno_for IS NOT NULL AND locked_at IS NOT NULL
     GROUP BY is_storno_for
    HAVING COUNT(*) > 1
);

DROP TABLE m0032_precheck;

CREATE UNIQUE INDEX ux_invoices_one_locked_storno
    ON invoices(is_storno_for)
 WHERE is_storno_for IS NOT NULL AND locked_at IS NOT NULL;

-- ============================================================================
-- Integritäts-Historie: „fehlt“ und „nicht lesbar“ (W2-INTEGRITY-UNREADABLE)
-- ============================================================================
-- files_failed zählt weiterhin Manipulation + fehlende Dateien; files_missing
-- ist die Teilmenge der fehlenden, files_unreadable steht daneben (kein
-- Manipulationsbefund). NULL = Lauf vor Migration 0032, nicht erfasst.
ALTER TABLE archive_integrity_checks ADD COLUMN files_missing INTEGER;
ALTER TABLE archive_integrity_checks ADD COLUMN files_unreadable INTEGER;
ALTER TABLE archive_integrity_checks ADD COLUMN missing_archive_ids_json TEXT;
ALTER TABLE archive_integrity_checks ADD COLUMN unreadable_archive_ids_json TEXT;

-- ============================================================================
-- Verworfene Rechnungs-Entwürfe (W14-032)
-- ============================================================================
-- Ein Entwurf trägt seine Nummer ab dem Anlegen. Verwerfen löscht nichts: die
-- Nummer bleibt verbraucht, Zeitpunkt und Grund stehen am Beleg und im Audit
-- (`invoice.draft_voided`). Der status-CHECK bleibt unverändert (kein
-- Tabellenumbau); ein verworfener Entwurf hat status = 'draft' und
-- voided_at IS NOT NULL.
ALTER TABLE invoices ADD COLUMN voided_at TEXT;
ALTER TABLE invoices ADD COLUMN void_reason TEXT;

-- Verworfen ist endgültig: keine Änderung mehr, auch kein Festschreiben.
--
-- Einzige Ausnahme (F2c, ADR 0039 Nr. 124 (3)): Die Anonymisierung des
-- Kontakts (Art. 17 DSGVO) ersetzt den Empfänger-Snapshot eines verworfenen
-- Entwurfs — eines nie ausgestellten Belegs, der weder Buchungsbeleg noch
-- Handelsbrief ist — durch den Platzhalter des anonymisierten Kontakts und
-- leert die übrigen Empfängerfelder und die interne Notiz. Nur in genau
-- diese Richtung: der Kontakt muss anonymisiert sein, buyer_name wird sein
-- Platzhalter, die übrigen Felder NULL. Nummer, Jahr, Datum, Beträge, Land,
-- Zeitpunkt und Grund des Verwerfens, Positionen und alle übrigen Spalten
-- bleiben unverändert (jede Spalte ist hier einzeln aufgeführt;
-- tests/migration_0032_test.rs prüft, dass keine fehlt).
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
  )
BEGIN SELECT RAISE(ABORT, 'invoice draft is voided: it cannot change any more'); END;

-- Verwerfen nur für offene Entwürfe, nur mit Grund, nie zugleich mit dem
-- Festschreiben.
CREATE TRIGGER trg_invoices_void_draft_only BEFORE UPDATE ON invoices
WHEN OLD.voided_at IS NULL
  AND (NEW.voided_at IS NOT NULL OR NEW.void_reason IS NOT NULL)
  AND (
    OLD.locked_at IS NOT NULL
    OR NEW.locked_at IS NOT NULL
    OR NEW.voided_at IS NULL
    OR NEW.void_reason IS NULL
    OR trim(NEW.void_reason) = ''
  )
BEGIN SELECT RAISE(ABORT, 'only unlocked invoice drafts can be voided, and only with a reason'); END;

-- Verworfen wird ein bestehender Entwurf (dokumentierter Schritt), nie beim
-- Anlegen.
CREATE TRIGGER trg_invoices_void_not_on_insert BEFORE INSERT ON invoices
WHEN NEW.voided_at IS NOT NULL OR NEW.void_reason IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'invoices are created as drafts; void them afterwards'); END;

-- ============================================================================
-- Storno ist endgültig (R1-S2-MARKCANCELED)
-- ============================================================================
-- Das Original wird im Storno-Paar genau einmal auf 'canceled' gesetzt; ein
-- zweiter Lauf darf weder den Status zurückdrehen noch canceled_at oder den
-- Verweis auf den Storno-Beleg überschreiben.
CREATE TRIGGER trg_invoices_canceled_final BEFORE UPDATE ON invoices
WHEN OLD.status = 'canceled'
  AND (
    NEW.status != 'canceled'
    OR NEW.canceled_at IS NOT OLD.canceled_at
    OR NEW.canceled_by_storno_id IS NOT OLD.canceled_by_storno_id
    OR NEW.cancel_reason IS NOT OLD.cancel_reason
  )
BEGIN SELECT RAISE(ABORT, 'invoice is canceled: the storno pair is final'); END;

-- Zurückgezogene Angebote (auch zurückgezogene Entwürfe = verworfen) bleiben
-- zurückgezogen; ensure_quote_pdf darf pdf_archive_id weiter setzen.
CREATE TRIGGER trg_quotes_canceled_final BEFORE UPDATE ON quotes
WHEN OLD.status = 'canceled'
  AND (
    NEW.status != 'canceled'
    OR NEW.locked_at IS NOT OLD.locked_at
    OR NEW.canceled_at IS NOT OLD.canceled_at
    OR NEW.canceled_reason IS NOT OLD.canceled_reason
  )
BEGIN SELECT RAISE(ABORT, 'quote is canceled: it cannot change any more'); END;

-- Ein umgewandeltes Angebot bleibt umgewandelt (F2c): kein Zurückziehen, kein
-- anderer Status und kein anderer Bezug auf die Rechnung. Einzige Ausnahme:
-- Wurde der aus der Umwandlung entstandene Entwurf verworfen, geht das
-- Angebot in derselben Transaktion zurück auf 'accepted' (ohne Bezug), damit
-- es sich erneut umwandeln lässt (`void_draft_core`, Audit
-- `quote.conversion_reverted`).
CREATE TRIGGER trg_quotes_converted_final BEFORE UPDATE ON quotes
WHEN OLD.status = 'converted'
  AND (
    (NEW.status = 'converted'
      AND (NEW.converted_invoice_id IS NOT OLD.converted_invoice_id
           OR NEW.converted_at IS NOT OLD.converted_at))
    OR (NEW.status != 'converted'
      AND NOT (
            NEW.status = 'accepted'
        AND NEW.converted_invoice_id IS NULL
        AND NEW.converted_at IS NULL
        AND (SELECT i.voided_at IS NOT NULL FROM invoices i
              WHERE i.id = OLD.converted_invoice_id) IS 1
      ))
  )
BEGIN SELECT RAISE(ABORT, 'quote is converted: only voiding the derived invoice draft reverts it'); END;

-- ============================================================================
-- Positionen festgeschriebener Belege (KB-0002)
-- ============================================================================
-- Die Kopf-Trigger (0028) schützten nur die Summen; Positionen eines
-- festgeschriebenen Belegs ließen sich per Raw-SQL ändern, löschen oder
-- ergänzen. Rechnungen: festgeschrieben oder verworfen. Angebote:
-- festgeschrieben oder nicht mehr im Entwurf (zurückgezogen).
CREATE TRIGGER trg_invoice_items_frozen_insert BEFORE INSERT ON invoice_items
WHEN (SELECT locked_at IS NOT NULL OR voided_at IS NOT NULL
        FROM invoices WHERE id = NEW.invoice_id)
BEGIN SELECT RAISE(ABORT, 'invoice is locked or voided: its items are immutable'); END;

CREATE TRIGGER trg_invoice_items_frozen_update BEFORE UPDATE ON invoice_items
WHEN (SELECT locked_at IS NOT NULL OR voided_at IS NOT NULL
        FROM invoices WHERE id = OLD.invoice_id)
  OR (SELECT locked_at IS NOT NULL OR voided_at IS NOT NULL
        FROM invoices WHERE id = NEW.invoice_id)
BEGIN SELECT RAISE(ABORT, 'invoice is locked or voided: its items are immutable'); END;

CREATE TRIGGER trg_invoice_items_frozen_delete BEFORE DELETE ON invoice_items
WHEN (SELECT locked_at IS NOT NULL OR voided_at IS NOT NULL
        FROM invoices WHERE id = OLD.invoice_id)
BEGIN SELECT RAISE(ABORT, 'invoice is locked or voided: its items are immutable'); END;

CREATE TRIGGER trg_quote_items_frozen_insert BEFORE INSERT ON quote_items
WHEN (SELECT locked_at IS NOT NULL OR status != 'draft'
        FROM quotes WHERE id = NEW.quote_id)
BEGIN SELECT RAISE(ABORT, 'quote is locked or canceled: its items are immutable'); END;

CREATE TRIGGER trg_quote_items_frozen_update BEFORE UPDATE ON quote_items
WHEN (SELECT locked_at IS NOT NULL OR status != 'draft'
        FROM quotes WHERE id = OLD.quote_id)
  OR (SELECT locked_at IS NOT NULL OR status != 'draft'
        FROM quotes WHERE id = NEW.quote_id)
BEGIN SELECT RAISE(ABORT, 'quote is locked or canceled: its items are immutable'); END;

CREATE TRIGGER trg_quote_items_frozen_delete BEFORE DELETE ON quote_items
WHEN (SELECT locked_at IS NOT NULL OR status != 'draft'
        FROM quotes WHERE id = OLD.quote_id)
BEGIN SELECT RAISE(ABORT, 'quote is locked or canceled: its items are immutable'); END;

-- ============================================================================
-- Archiv: write-once auch gegen DELETE (KB-0004)
-- ============================================================================
-- Kein App-Pfad löscht Archiv-Einträge (verwaiste Dateien meldet die
-- Integritätsprüfung, der Eintrag bleibt).
CREATE TRIGGER trg_archive_no_delete BEFORE DELETE ON archive_entries
BEGIN SELECT RAISE(ABORT, 'archive_entries are write-once: entries are never deleted'); END;

-- ============================================================================
-- Anlagen: Abgang nach der Veräußerung unveränderlich (R1-S2-ASSET-DISPOSAL)
-- ============================================================================
-- Nach dem Abgang steht die Anlage im Anlageverzeichnis und in der EÜR des
-- Abgangsjahres (AfA bis zum Abgang + Restbuchwert als Abgangsverlust). Darum
-- bleiben nicht nur die Abgangsfelder, sondern auch Bezeichnung, Anschaffung,
-- AfA-Grundlage und Restbuchwert fest (F2c) — auch solange das Jahr noch nicht
-- abgeschlossen ist. Erlaubt bleiben `locked_at` (Jahresabschluss), `notes`
-- und `updated_at`.
CREATE TRIGGER trg_assets_disposal_final BEFORE UPDATE ON assets
WHEN OLD.disposed = 1
  AND (
    NEW.disposed IS NOT 1
    OR NEW.disposal_date IS NOT OLD.disposal_date
    OR NEW.disposal_type IS NOT OLD.disposal_type
    OR NEW.disposal_proceeds_cents IS NOT OLD.disposal_proceeds_cents
    OR NEW.disposal_residual_book_value_cents IS NOT OLD.disposal_residual_book_value_cents
    OR NEW.asset_number IS NOT OLD.asset_number
    OR NEW.label IS NOT OLD.label
    OR NEW.acquisition_date IS NOT OLD.acquisition_date
    OR NEW.acquisition_cost_cents IS NOT OLD.acquisition_cost_cents
    OR NEW.acquisition_fiscal_year IS NOT OLD.acquisition_fiscal_year
    OR NEW.expense_id IS NOT OLD.expense_id
    OR NEW.vendor_contact_id IS NOT OLD.vendor_contact_id
    OR NEW.depreciation_method IS NOT OLD.depreciation_method
    OR NEW.useful_life_years IS NOT OLD.useful_life_years
    OR NEW.afa_category IS NOT OLD.afa_category
    OR NEW.business_share_percent IS NOT OLD.business_share_percent
    OR NEW.book_value_cents IS NOT OLD.book_value_cents
    OR NEW.last_depreciation_year IS NOT OLD.last_depreciation_year
  )
BEGIN SELECT RAISE(ABORT, 'asset is disposed: disposal fields are immutable'); END;

-- Die AfA-Buchungen einer veräußerten Anlage bleiben (F2c): ein Zurücksetzen
-- nach dem Abgang ließe die AfA des Abgangsjahres endgültig verschwinden, weil
-- der AfA-Lauf veräußerte Anlagen nicht mehr bucht.
CREATE TRIGGER trg_depreciation_entries_no_delete_disposed BEFORE DELETE ON depreciation_entries
WHEN (SELECT disposed FROM assets WHERE id = OLD.asset_id) = 1
BEGIN SELECT RAISE(ABORT, 'asset is disposed: its depreciation entries are final'); END;

-- ============================================================================
-- Zahlungskonten: höchstens ein Standardkonto (R1-013)
-- ============================================================================
-- Bestand bereinigen wie `ensure_one_default` (Stammdaten, nicht
-- festgeschrieben): ein inaktives Konto ist nie Standard; von mehreren aktiven
-- Standardkonten bleibt das älteste (UUIDv7 = zeitsortiert). Jede Änderung
-- steht im Audit.
INSERT INTO audit_log (action, entity_type, entity_id, details_json)
SELECT 'payment_account.default_cleared', 'payment_account', id,
       json_object('migration', 32, 'reason', 'inactive')
  FROM payment_accounts
 WHERE is_default = 1 AND active = 0;

UPDATE payment_accounts SET is_default = 0 WHERE is_default = 1 AND active = 0;

INSERT INTO audit_log (action, entity_type, entity_id, details_json)
SELECT 'payment_account.default_cleared', 'payment_account', id,
       json_object('migration', 32, 'reason', 'second_default')
  FROM payment_accounts
 WHERE is_default = 1 AND active = 1
   AND id != (SELECT id FROM payment_accounts
               WHERE is_default = 1 AND active = 1
               ORDER BY id ASC LIMIT 1);

UPDATE payment_accounts SET is_default = 0
 WHERE is_default = 1 AND active = 1
   AND id != (SELECT id FROM payment_accounts
               WHERE is_default = 1 AND active = 1
               ORDER BY id ASC LIMIT 1);

CREATE UNIQUE INDEX uq_payment_accounts_default
    ON payment_accounts(is_default)
 WHERE is_default = 1;

-- ============================================================================
-- Regel „Kosten-Abo fällig“ (Lauf F1, ADR 0040 Nr. 8)
-- ============================================================================
INSERT OR IGNORE INTO notification_rules
    (id, rule_type, label, enabled, config_json, deliver_in_app, deliver_os_native)
VALUES
    ('rule_recurring_due', 'recurring_due', 'Kosten-Abo fällig', 1, '{}', 1, 0);

-- ============================================================================
-- Schema-Version 31 → 32
-- ============================================================================
UPDATE app_settings
   SET value = '32',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
