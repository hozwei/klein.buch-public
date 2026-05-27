-- Migration 0009: Wiederkehrende Abos (recurring_subscriptions).
-- Phase 2B, Block 10.
--
-- ABWEICHUNG vom Original-PRD (0004_recurring.sql):
--   * Migrations-Nummer 0009 statt 0004 (0001–0008 verbraucht; R4-Revision +
--     Block 8 legal_documents + Block 9 expenses/private_movements).
--     schema_version → 9.
--
-- Fachlicher Hintergrund:
--   * Ein Abo ist ein STAMMDATEN-Template (kein GoBD-Beleg) — daher KEIN
--     Immutability-Trigger: Abos sind editierbar und pausierbar (active=0).
--     Die DARAUS erzeugten Kosten (expenses) sind dagegen sofort gelockt
--     (trg_expenses_immutable, Block 9) — die GoBD-Hardline greift dort.
--   * day_of_period 1..=31: der Stichtag im Periodenraster. Hat ein Monat
--     weniger Tage (z. B. 31 im Februar), klemmt der Scheduler auf das
--     Monatsende (siehe domain::recurring::compute_next_due_date).
--   * next_due_date ist der nächste fällige Stichtag. Der Scheduler legt für
--     auto_create_expense=1 alle verpassten Perioden nach (Catch-up), bis
--     next_due_date wieder in der Zukunft liegt; für =0 erscheint das Abo nur
--     als „fällig" in der Liste (Reminder).
--   * expected_amount_cents ist der erwartete BRUTTO-Betrag (Eingangsseite,
--     §19 betrifft nur Ausgangsbelege). Im Template gibt es keinen USt-Split;
--     die auto-erzeugte Kostenposition setzt net=brutto, tax=0 (bei Bedarf per
--     Storno + Neuerfassung korrigieren).
--   * last_expense_id verweist auf die zuletzt erzeugte Kostenposition.

PRAGMA foreign_keys = ON;

CREATE TABLE recurring_subscriptions (
    id                          TEXT PRIMARY KEY NOT NULL,
    label                       TEXT NOT NULL,               -- "Microsoft 365 Business"
    vendor_contact_id           TEXT REFERENCES contacts(id),
    frequency                   TEXT NOT NULL CHECK (frequency IN
                                ('monthly','quarterly','semiannually','annually')),
    day_of_period               INTEGER NOT NULL CHECK (day_of_period BETWEEN 1 AND 31),
    next_due_date               TEXT NOT NULL,
    expected_amount_cents       INTEGER NOT NULL,
    category                    TEXT NOT NULL CHECK (category IN
                                ('office','software','hardware','travel','services','goods',
                                 'communications','vehicle','rent','insurance','training',
                                 'fees','marketing','other')),
    description_template        TEXT NOT NULL,
    auto_create_expense         INTEGER NOT NULL DEFAULT 0 CHECK (auto_create_expense IN (0,1)),
    -- §13b vorgeben (z. B. immer für MS365 Reverse-Charge)
    reverse_charge_13b_default  INTEGER NOT NULL DEFAULT 0 CHECK (reverse_charge_13b_default IN (0,1)),
    active                      INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
    last_executed_at            TEXT,
    last_expense_id             TEXT REFERENCES expenses(id),
    created_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_recurring_due ON recurring_subscriptions(next_due_date, active);

UPDATE app_settings
   SET value = '9',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
