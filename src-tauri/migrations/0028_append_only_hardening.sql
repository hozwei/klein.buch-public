-- Migration 0028: Append-only / Immutability-Hardening (R1-Re-Review v2026.5).
--
-- Adressiert die R1-Findings aus `docs/REVIEW-V2026.5.md`:
--
--   * R1-001/-002  S0 — kein BEFORE-DELETE-Trigger auf invoices/quotes
--                       (CASCADE auf invoice_items/quote_items hätte einen
--                       Direkt-DELETE silent durchgewunken). GoBD-Hardline
--                       „Storno statt Löschung" Schema-seitig festgenagelt.
--   * R1-004       S0 — `trg_invoices_immutable` war Blacklist statt Whitelist;
--                       buyer_*-Snapshot, seller_*-Snapshot, delivery_date,
--                       currency_code, pdf_template, is_storno_for, archives
--                       waren nach Lock änderbar. DSGVO-Anonymisierungs-
--                       Snapshot-Schutz + Storno-Pair-Verkettung + §14-Pflicht-
--                       angabe Leistungsdatum sind damit DB-seitig fix.
--   * R1-005       S1 — Unlock-Schutz: ein UPDATE, das `locked_at = NULL` setzt
--                       oder den Lock-Zeitstempel mutiert, war zulässig. Jetzt
--                       blockt der Trigger jede Mutation auf `locked_at` nach
--                       dem ersten Setzen.
--   * R1-006..-009 S1 — DELETE-Trigger fehlten auf expenses/private_movements/
--                       assets/depreciation_entries (gelockt). Ergänzt — Reset-
--                       Pfad in `depreciation` (löscht ungelockte Einträge)
--                       bleibt funktional, weil der Trigger condition-gated ist.
--
-- Mutations-Pfade (App-Layer), die DURCHKOMMEN müssen:
--
--   invoices:   lock (locked_at, status, validation_*, validated_at,
--                     pdf_archive_id, xml_archive_id, updated_at) — feuert NICHT,
--                     weil OLD.locked_at noch IS NULL ist;
--               record_payment (paid_amount_cents, paid_at,
--                     payment_history_json, status, updated_at);
--               mark_canceled (status, canceled_at, canceled_by_storno_id,
--                     cancel_reason, updated_at);
--               mark_sent (sent_at, status, updated_at);
--               set_buyer_snapshot — pre-lock, WHERE locked_at IS NULL.
--
--   quotes:     issue (locked_at, sent_at, status, updated_at) — feuert NICHT;
--               accept/reject/cancel/mark_converted (status, *_at, *_reason,
--                     converted_invoice_id, updated_at);
--               set_pdf_archive (pdf_archive_id, updated_at) — gewollt mutabel
--                     (ensure_quote_pdf Re-Render-Pfad).
--
--   expenses:   record/lock-at-create — feuert NICHT;
--               cancel (status, canceled_at, canceled_reason);
--               set_payment (paid_date, paid_from_account_id);
--               set_receipt_archive_id (receipt_archive_id);
--               set_einvoice_validation (einvoice_validation_status,
--                     einvoice_validation_report);
--               set_recurring_subscription_id / set_capitalized_as_asset_id.
--
--   private_movements: kein App-Layer-UPDATE nach Lock (single-shot record);
--               nur receipt_archive_id + notes als Anpassungs-Slots offen.
--
--   assets:     set_book_value (book_value_cents, last_depreciation_year);
--               dispose (disposed, disposal_*, updated_at).
--
--   depreciation_entries: lock_for_year setzt locked_at; danach komplett
--               immutable (bedingungslos). DELETE nur auf ungelockten Einträgen
--               (Reset-Pfad).

PRAGMA foreign_keys = ON;

-- ============================================================================
-- invoices: erweitertes Immutability-Trigger + No-Delete
-- ============================================================================
DROP TRIGGER IF EXISTS trg_invoices_immutable;

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
  )
BEGIN SELECT RAISE(ABORT, 'invoice is locked: immutable fields cannot change after lock'); END;

CREATE TRIGGER trg_invoices_no_delete BEFORE DELETE ON invoices
BEGIN SELECT RAISE(ABORT, 'invoice records are immutable: use storno (§14 UStG)'); END;

-- ============================================================================
-- quotes: erweitertes Immutability-Trigger + No-Delete
-- ============================================================================
DROP TRIGGER IF EXISTS trg_quotes_immutable;

CREATE TRIGGER trg_quotes_immutable BEFORE UPDATE ON quotes
WHEN OLD.locked_at IS NOT NULL
  AND (
    -- Unlock-Schutz (R1-005)
    NEW.locked_at IS NULL
    OR NEW.locked_at != OLD.locked_at
    -- Bestehende Kernfelder (0005)
    OR NEW.quote_number != OLD.quote_number
    OR NEW.quote_date != OLD.quote_date
    OR NEW.net_amount_cents != OLD.net_amount_cents
    OR NEW.gross_amount_cents != OLD.gross_amount_cents
    OR NEW.tax_amount_cents != OLD.tax_amount_cents
    OR NEW.contact_id != OLD.contact_id
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.is_kleinunternehmer != OLD.is_kleinunternehmer
    -- Pflichtangaben + Stammdaten-Snapshot (R1-004)
    OR NEW.valid_until != OLD.valid_until
    OR NEW.currency_code != OLD.currency_code
    OR NEW.pdf_template != OLD.pdf_template
    -- Buyer-Snapshot (DSGVO; Migration 0023)
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
    -- pdf_archive_id NICHT geschützt — ensure_quote_pdf darf das nach Lock
    -- noch ersetzen (Re-Render-Pfad).
  )
BEGIN SELECT RAISE(ABORT, 'quote is locked: immutable fields cannot change after lock'); END;

CREATE TRIGGER trg_quotes_no_delete BEFORE DELETE ON quotes
BEGIN SELECT RAISE(ABORT, 'quote records are immutable: use cancel/storno'); END;

-- ============================================================================
-- expenses: erweitertes Immutability-Trigger + No-Delete
-- ============================================================================
DROP TRIGGER IF EXISTS trg_expenses_immutable;

CREATE TRIGGER trg_expenses_immutable BEFORE UPDATE ON expenses
WHEN OLD.locked_at IS NOT NULL
  AND (
    -- Unlock-Schutz (R1-005)
    NEW.locked_at IS NULL
    OR NEW.locked_at != OLD.locked_at
    -- Bestehende Kernfelder (0007)
    OR NEW.expense_number != OLD.expense_number
    OR NEW.expense_date != OLD.expense_date
    OR NEW.net_amount_cents != OLD.net_amount_cents
    OR NEW.gross_amount_cents != OLD.gross_amount_cents
    OR NEW.tax_amount_cents != OLD.tax_amount_cents
    OR NEW.vendor_contact_id IS NOT OLD.vendor_contact_id
    -- EÜR-relevante Klassifikation + Snapshot (Sub3 R1-011)
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.category != OLD.category
    OR NEW.vendor_name_snapshot != OLD.vendor_name_snapshot
    OR NEW.description != OLD.description
    OR NEW.currency_code != OLD.currency_code
    OR NEW.reverse_charge_13b != OLD.reverse_charge_13b
  )
BEGIN SELECT RAISE(ABORT, 'expense is locked: immutable fields cannot change after lock'); END;

CREATE TRIGGER trg_expenses_no_delete BEFORE DELETE ON expenses
BEGIN SELECT RAISE(ABORT, 'expense records are immutable: use cancel (status=canceled)'); END;

-- ============================================================================
-- private_movements: erweitertes Immutability-Trigger + No-Delete
-- ============================================================================
DROP TRIGGER IF EXISTS trg_private_movements_immutable;

CREATE TRIGGER trg_private_movements_immutable BEFORE UPDATE ON private_movements
WHEN OLD.locked_at IS NOT NULL
  AND (
    -- Unlock-Schutz (R1-005)
    NEW.locked_at IS NULL
    OR NEW.locked_at != OLD.locked_at
    -- Bestehende Kernfelder (0008)
    OR NEW.movement_number != OLD.movement_number
    OR NEW.movement_date != OLD.movement_date
    OR NEW.amount_cents != OLD.amount_cents
    OR NEW.movement_type != OLD.movement_type
    OR NEW.fiscal_year != OLD.fiscal_year
    OR NEW.account_id IS NOT OLD.account_id
    -- Beleg-Beschreibung (Sub3 R1-012)
    OR NEW.description != OLD.description
  )
BEGIN SELECT RAISE(ABORT, 'private_movement is locked: immutable fields cannot change after lock'); END;

CREATE TRIGGER trg_private_movements_no_delete BEFORE DELETE ON private_movements
BEGIN SELECT RAISE(ABORT, 'private_movement records are append-only: post a counter-movement'); END;

-- ============================================================================
-- assets: erweitertes Immutability-Trigger + No-Delete
-- ============================================================================
DROP TRIGGER IF EXISTS trg_assets_immutable;

CREATE TRIGGER trg_assets_immutable BEFORE UPDATE ON assets
WHEN OLD.locked_at IS NOT NULL
  AND (
    -- Unlock-Schutz (R1-005)
    NEW.locked_at IS NULL
    OR NEW.locked_at != OLD.locked_at
    -- Bestehende Kernfelder (0010)
    OR NEW.asset_number != OLD.asset_number
    OR NEW.acquisition_date != OLD.acquisition_date
    OR NEW.acquisition_cost_cents != OLD.acquisition_cost_cents
    OR NEW.depreciation_method != OLD.depreciation_method
    OR NEW.useful_life_years IS NOT OLD.useful_life_years
    OR NEW.business_share_percent != OLD.business_share_percent
    -- Anschaffungs-Snapshot (Sub3 R1-013)
    OR NEW.acquisition_fiscal_year != OLD.acquisition_fiscal_year
    OR NEW.afa_category IS NOT OLD.afa_category
    OR NEW.expense_id IS NOT OLD.expense_id
    OR NEW.vendor_contact_id IS NOT OLD.vendor_contact_id
    OR NEW.label != OLD.label
    -- book_value_cents, last_depreciation_year + disposal_* bleiben mutabel:
    -- set_book_value (AfA-Fortschreibung) und dispose (Veräußerung) sind
    -- legitime Post-Lock-Pfade.
  )
BEGIN SELECT RAISE(ABORT, 'asset is locked: immutable fields cannot change after lock'); END;

CREATE TRIGGER trg_assets_no_delete BEFORE DELETE ON assets
BEGIN SELECT RAISE(ABORT, 'asset records are append-only: use dispose (disposed=1)'); END;

-- ============================================================================
-- depreciation_entries: nur No-Delete-Trigger (Update-Schutz bedingungslos
-- bereits in 0011_depreciation.sql für gelockte Einträge)
-- ============================================================================
-- Reset-Pfad in `depreciation::reset_unlocked_for_asset` löscht ungelockte
-- Einträge (`WHERE … AND locked_at IS NULL`). Dieser Trigger blockt nur
-- gelockte Einträge — der Reset-Pfad bleibt funktional.
CREATE TRIGGER trg_depreciation_entries_no_delete_locked BEFORE DELETE ON depreciation_entries
WHEN OLD.locked_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'locked depreciation entries are immutable'); END;

-- ============================================================================
-- Schema-Version 27 → 28
-- ============================================================================
UPDATE app_settings
   SET value = '28',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
