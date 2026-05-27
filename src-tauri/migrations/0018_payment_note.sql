-- Migration 0018: optionaler Zahlungs-/Bezahlt-Hinweis pro Rechnung.
--
-- Reiner PDF-Hinweistext (z. B. „Betrag dankend bar erhalten am 23.05.2026"),
-- den Manuel je Rechnung beim Erstellen hinterlegt. Wird am Entwurf gesetzt und
-- mit dem Festschreiben unveränderlich (GoBD) — die invoices-Immutability greift
-- über `locked_at`, eine Whitelist-Änderung ist nicht nötig, weil payment_note
-- nach dem Lock nicht mehr verändert wird.
--
-- KEINE EÜR-Wirkung und KEINE XRechnung-Auswirkung: rein informativ aufs PDF.
-- Die tatsächliche Zahlung wird weiterhin separat über „Zahlung erfassen"
-- gebucht (cash-basis EÜR über paid_amount_cents/paid_at).
--
-- schema_version → 18. Migrationen 0001–0017 verbraucht; 0018 ist die nächste.

PRAGMA foreign_keys = ON;

ALTER TABLE invoices ADD COLUMN payment_note TEXT;

UPDATE app_settings
   SET value = '18', updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
