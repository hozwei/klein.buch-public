-- Migration 0024: Wiederkehrende Ausgangsrechnungen (recurring_invoices).
-- Phase 4, Block RI-1 — Abo-Rechnungen (z. B. "Wartung Server, monatlich").
--
-- Abgrenzung zur Eingangsseite:
--   * `recurring_subscriptions` (0009, Block 10) = wiederkehrende KOSTEN
--     (Lieferant, Brutto). Bleibt unangetastet.
--   * DIESE Tabellen = wiederkehrende RECHNUNGEN an Kunden (Ausgangsseite).
--     Der Scheduler (Block RI-2) erzeugt daraus echte Rechnungen über die
--     bestehende draft -> lock_and_issue-Pipeline (Nummer, XRechnung, PDF,
--     KoSIT, Archiv, Lock). §19-Klausel + Pflichtangaben kommen dadurch
--     automatisch aus der Pipeline.
--
-- Fachlicher Hintergrund:
--   * Eine Abo-Vorlage ist ein STAMMDATEN-Template (kein GoBD-Beleg) — daher
--     KEIN Immutability-Trigger: Vorlagen sind editierbar und pausierbar
--     (active=0). Die DARAUS erzeugten Rechnungen sind nach dem Festschreiben
--     unveränderlich (invoices-Trigger) — die GoBD-Hardline greift dort.
--   * auto_mode steuert je Vorlage, wie weit der Scheduler automatisiert
--     (Manuel-Entscheidung 2026-05-24, im UI in Klartext wählbar):
--       'draft'      -> nur Rechnungs-Entwurf anlegen + Benachrichtigung
--                       (prüfungssicher; Festschreiben/Versand macht der Nutzer).
--       'issue'      -> automatisch festschreiben (volle Pipeline), kein Versand.
--       'issue_send' -> festschreiben + automatisch per E-Mail senden.
--   * RECHTLICHE FESTLEGUNG Belegdatum (Manuel-Entscheidung "das was rechtlich
--     vorgegeben ist"): Das Ausstellungsdatum der erzeugten Rechnung ist immer
--     der TATSÄCHLICHE Erstellungstag (heute), NIE rückdatiert
--     (§14 Abs. 4 Nr. 3 UStG + GoBD: zeitnahe, lückenlose, fortlaufende Nummern).
--     Der Leistungszeitraum der Periode (z. B. "Mai 2026") wird getrennt als
--     Leistungsdatum + in der Positionsbeschreibung geführt (§14 Abs. 4 Nr. 6).
--     Catch-up legt pro verpasster Periode eine Rechnung an — alle mit
--     heutigem Belegdatum, korrektem Leistungszeitraum.
--   * day_of_period 1..=31 wie bei 0009: bei kürzeren Monaten klemmt der
--     Scheduler auf das Monatsende (domain::recurring::compute_next_due_date,
--     wird wiederverwendet).
--   * §19-Hardline: Solange is_kleinunternehmer = true, tragen die Positionen
--     tax_category_code 'E' und tax_rate 0. Der Default unten ist 'E'; die
--     §14c-Durchsetzung passiert zusätzlich in der draft-/issue-Pipeline.
--   * Beträge sind NETTO-Cent (Ausgangsseite). Bei §19 ist Netto = Brutto.

PRAGMA foreign_keys = ON;

-- ---- Vorlagen-Kopf ---------------------------------------------------------
CREATE TABLE recurring_invoices (
    id                   TEXT PRIMARY KEY NOT NULL,
    label                TEXT NOT NULL,                 -- interne Bezeichnung, z. B. "Wartung Server – Müller GmbH"
    contact_id           TEXT NOT NULL REFERENCES contacts(id),
    frequency            TEXT NOT NULL CHECK (frequency IN
                         ('monthly','quarterly','semiannually','annually')),
    day_of_period        INTEGER NOT NULL CHECK (day_of_period BETWEEN 1 AND 31),
    next_due_date        TEXT NOT NULL,                 -- nächster fälliger Stichtag (YYYY-MM-DD)
    start_date           TEXT,                          -- optional, Dokumentation
    end_date             TEXT,                          -- optional Laufzeit-Ende; danach keine Erzeugung mehr
    auto_mode            TEXT NOT NULL DEFAULT 'draft' CHECK (auto_mode IN
                         ('draft','issue','issue_send')),
    payment_terms_days   INTEGER NOT NULL DEFAULT 14 CHECK (payment_terms_days >= 0),
    pdf_template         TEXT NOT NULL DEFAULT 'default',
    -- Leistungszeitraum der Periode automatisch in Leistungsdatum + Beschreibung setzen.
    service_period_note  INTEGER NOT NULL DEFAULT 1 CHECK (service_period_note IN (0,1)),
    active               INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
    last_executed_at     TEXT,
    last_invoice_id      TEXT REFERENCES invoices(id),
    notes                TEXT,
    created_at           TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at           TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_recurring_invoices_due ON recurring_invoices(next_due_date, active);

-- ---- Vorlagen-Positionen ---------------------------------------------------
-- Spiegelt invoice_items (inkl. PDF-Titel/Markup + Paket-Provenienz aus
-- 0020/0021), damit die Scheduler-Materialisierung 1:1 in invoice_items
-- übernehmen kann. net_amount_cents wird vom Aufrufer berechnet gespeichert.
CREATE TABLE recurring_invoice_items (
    id                       TEXT PRIMARY KEY NOT NULL,
    recurring_invoice_id     TEXT NOT NULL REFERENCES recurring_invoices(id) ON DELETE CASCADE,
    position                 INTEGER NOT NULL,
    description              TEXT NOT NULL,
    quantity                 REAL NOT NULL,
    unit_code                TEXT NOT NULL DEFAULT 'C62',
    unit_price_cents         INTEGER NOT NULL,
    net_amount_cents         INTEGER NOT NULL,
    tax_rate_percent         REAL NOT NULL DEFAULT 0.0,
    tax_category_code        TEXT NOT NULL DEFAULT 'E'
                             CHECK (tax_category_code IN ('S','Z','E','AE','K','G','O','L','M')),
    -- PDF-Titel/Markup (NUR PDF-Block, nicht im XRechnung-XML) + Paket-Provenienz.
    description_title        TEXT,
    description_markup       TEXT,
    source_package_id        TEXT,
    source_package_revision  INTEGER
) STRICT;
CREATE INDEX idx_recurring_invoice_items_parent
    ON recurring_invoice_items(recurring_invoice_id, position);

UPDATE app_settings
   SET value = '24',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
