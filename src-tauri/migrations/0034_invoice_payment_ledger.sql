-- Migration 0034: Zahlungsbuch für Ausgangsrechnungen (ADR 0042 Nr. 7–9,
-- Lauf 2026.11.1, Block V2).
--
-- Bisher stand jede Zahlung als Eintrag in der JSON-Spalte
-- `invoices.payment_history_json`; daraus abgeleitet waren
-- `paid_amount_cents`, `paid_at` und `status`. Kein Trigger schützte diese
-- Spalten (0028 ließ sie nach dem Festschreiben bewusst offen), eine
-- Fehlbuchung ließ sich nicht korrigieren.
--
-- Jetzt stehen Zahlungen in der append-only Tabelle `invoice_payments`
-- (§ 146 Abs. 4 AO, § 239 Abs. 3 HGB, GoBD Rz. 58–60, 64):
--
--   * kind 'payment'   Zahlungseingang, Betrag > 0.
--   * kind 'reversal'  Gegenbuchung („Zahlung stornieren“, ADR 0042 Nr. 8):
--                      Betrag, Datum und Konto der stornierten Zahlung mit
--                      umgekehrtem Vorzeichen, Grund Pflicht, Verweis
--                      `reverses_payment_id`; höchstens eine je Zahlung.
--
-- Kein UPDATE, kein DELETE. `paid_amount_cents`, `paid_at` und `status` der
-- Rechnung sind ein Cache des Buchs: Trigger lassen sie nur noch konsistent
-- zum Buch ändern (Summe, jüngstes Zahldatum der nicht stornierten
-- Zahlungen, Status aus der Summe). `payment_history_json` bleibt als
-- Altbestand unverändert stehen (nur lesbar, eingefroren); maßgeblich ist das
-- Buch. Der Whitelist-Trigger aus 0028 bleibt unverändert (nur verschärft
-- durch die zusätzlichen Trigger hier).
--
-- Übernahme des Bestands: je Eintrag der JSON-Historie eine Zahlung (in der
-- Reihenfolge der Historie, Einträge mit Betrag 0 entfallen — sie wirkten nie),
-- bei Altbestand ohne Historie eine Zahlung über `paid_amount_cents` am
-- `paid_at` (wie bisher `record_payment`). Vorab bricht ein benannter CHECK
-- ab (ADR 0039 Nr. 124), wenn die Historie nicht lesbar ist, ihre Summe nicht
-- `paid_amount_cents` ergibt, eine bezahlte Rechnung kein Zahldatum hat oder
-- ein Eintrag auf ein unbekanntes Zahlungskonto zeigt. Dieselben Prüfungen
-- macht `db::payment_ledger_problem` schon vor dem Pflicht-Backup und in der
-- Restore-Prüfung; der CHECK ist die zweite Linie.
--
-- IDs: UUIDv7 lässt sich in SQL nicht erzeugen. Wie in 0033 (ADR 0042
-- Nr. 20) tragen die übernommenen Zahlungen feste IDs im UUIDv7-Format mit
-- Zeitstempel 0 und dem Kennzeichen 034 im Feld rand_a
-- (00000000-0000-7034-8000-NNNNNNNNNNNN, laufend nach Rechnungs-ID und
-- Reihenfolge): gültige Version und Variante, erkennbar als „aus der
-- Migration“, deterministisch — ein Restore eines älteren Backups ergibt nach
-- der Vorwärtsmigration dieselben IDs. Neue Buchungen erzeugt der Server mit
-- `Uuid::now_v7()`.

PRAGMA foreign_keys = ON;

-- ============================================================================
-- Vorab-Prüfung (ADR 0039 Nr. 124)
-- ============================================================================
-- SQLite kennt außerhalb von Triggern kein bedingtes RAISE; ein benannter
-- CHECK auf einer temporären Tabelle bricht mit genau diesem Namen ab.
-- json_each läuft nur über gültige JSON-Arrays (sonst über '[]'), damit eine
-- kaputte Historie als Befund zählt statt die Abfrage abzubrechen.
CREATE TEMP TABLE m0034_precheck (
    unreadable INTEGER NOT NULL
        CONSTRAINT "Migration 0034 abgebrochen: Die Zahlungshistorie mindestens einer Rechnung ist nicht lesbar. Es wurde nichts umgestellt (Handbuch: Fehlersuche im Container)."
        CHECK (unreadable = 0),
    mismatched INTEGER NOT NULL
        CONSTRAINT "Migration 0034 abgebrochen: Bei mindestens einer Rechnung ergibt die Zahlungshistorie nicht den gezahlten Betrag. Es wurde nichts umgestellt (Handbuch: Fehlersuche im Container)."
        CHECK (mismatched = 0),
    undated INTEGER NOT NULL
        CONSTRAINT "Migration 0034 abgebrochen: Mindestens eine Rechnung ist bezahlt, hat aber weder Zahlungshistorie noch Zahldatum. Es wurde nichts umgestellt (Handbuch: Fehlersuche im Container)."
        CHECK (undated = 0),
    unknown_accounts INTEGER NOT NULL
        CONSTRAINT "Migration 0034 abgebrochen: Die Zahlungshistorie mindestens einer Rechnung verweist auf ein unbekanntes Zahlungskonto. Es wurde nichts umgestellt (Handbuch: Fehlersuche im Container)."
        CHECK (unknown_accounts = 0)
) STRICT;

CREATE TEMP VIEW m0034_entries AS
SELECT i.id AS invoice_id,
       CAST(j.key AS INTEGER) AS pos,
       j.value AS entry
  FROM invoices i,
       json_each(CASE WHEN json_valid(i.payment_history_json)
                       AND json_type(i.payment_history_json) = 'array'
                      THEN i.payment_history_json ELSE '[]' END) j
 WHERE i.payment_history_json IS NOT NULL;

INSERT INTO m0034_precheck (unreadable, mismatched, undated, unknown_accounts)
SELECT
    -- Spalte kein JSON-Array oder ein Eintrag ohne ganzzahligen Betrag ≥ 0,
    -- ohne Datum JJJJ-MM-TT oder mit einem Konto, das kein Text ist.
    (SELECT COUNT(*) FROM invoices
      WHERE payment_history_json IS NOT NULL
        AND (NOT json_valid(payment_history_json)
             OR json_type(payment_history_json) IS NOT 'array'))
  + (SELECT COUNT(*) FROM m0034_entries
      WHERE json_type(entry) IS NOT 'object'
         OR json_type(entry, '$.amountCents') IS NOT 'integer'
         OR json_extract(entry, '$.amountCents') < 0
         OR json_type(entry, '$.paidDate') IS NOT 'text'
         OR date(substr(json_extract(entry, '$.paidDate'), 1, 10))
                IS NOT substr(json_extract(entry, '$.paidDate'), 1, 10)
         OR json_type(entry, '$.accountId') NOT IN ('text', 'null')),
    -- Summe der Historie ≠ paid_amount_cents (nur bei nicht leerer Historie).
    (SELECT COUNT(*) FROM invoices i
      WHERE json_valid(i.payment_history_json)
        AND json_type(i.payment_history_json) = 'array'
        AND json_array_length(i.payment_history_json) > 0
        AND i.paid_amount_cents IS NOT
            (SELECT COALESCE(SUM(json_extract(e.entry, '$.amountCents')), 0)
               FROM m0034_entries e WHERE e.invoice_id = i.id)),
    -- Bezahlt ohne Historie und ohne gültiges Zahldatum.
    (SELECT COUNT(*) FROM invoices
      WHERE paid_amount_cents > 0
        AND (payment_history_json IS NULL
             OR (json_valid(payment_history_json)
                 AND json_type(payment_history_json) = 'array'
                 AND json_array_length(payment_history_json) = 0))
        AND (paid_at IS NULL
             OR date(substr(paid_at, 1, 10)) IS NOT substr(paid_at, 1, 10))),
    (SELECT COUNT(*) FROM m0034_entries e
      WHERE json_type(e.entry, '$.accountId') = 'text'
        AND NOT EXISTS (SELECT 1 FROM payment_accounts pa
                         WHERE pa.id = json_extract(e.entry, '$.accountId')));

DROP TABLE m0034_precheck;

-- ============================================================================
-- Zahlungsbuch
-- ============================================================================
CREATE TABLE invoice_payments (
    id                  TEXT PRIMARY KEY NOT NULL,
    invoice_id          TEXT NOT NULL REFERENCES invoices(id),
    -- Laufende Nummer je Rechnung (Reihenfolge der Erfassung).
    seq                 INTEGER NOT NULL CHECK (seq >= 1),
    kind                TEXT NOT NULL CHECK (kind IN ('payment','reversal')),
    amount_cents        INTEGER NOT NULL,
    -- Zahldatum (Zufluss, § 11 EStG); bei der Gegenbuchung das der Zahlung.
    paid_date           TEXT NOT NULL CHECK (date(paid_date) IS paid_date),
    account_id          TEXT REFERENCES payment_accounts(id),
    note                TEXT,
    reverses_payment_id TEXT REFERENCES invoice_payments(id),
    reason              TEXT,
    created_at          TEXT NOT NULL,
    UNIQUE (invoice_id, seq),
    CHECK ((kind = 'payment' AND amount_cents > 0
            AND reverses_payment_id IS NULL AND reason IS NULL)
        OR (kind = 'reversal' AND amount_cents < 0
            AND reverses_payment_id IS NOT NULL
            AND reason IS NOT NULL AND length(trim(reason)) BETWEEN 10 AND 500))
) STRICT;

-- Höchstens eine Gegenbuchung je Zahlung.
CREATE UNIQUE INDEX ux_invoice_payments_one_reversal
    ON invoice_payments(reverses_payment_id)
 WHERE reverses_payment_id IS NOT NULL;

CREATE INDEX idx_invoice_payments_paid_date ON invoice_payments(paid_date);

-- ============================================================================
-- Übernahme des Bestands
-- ============================================================================
INSERT INTO invoice_payments
    (id, invoice_id, seq, kind, amount_cents, paid_date, account_id, note, created_at)
SELECT '00000000-0000-7034-8000-'
           || printf('%012x', ROW_NUMBER() OVER (ORDER BY invoice_id, seq)),
       invoice_id, seq, 'payment', amount_cents, paid_date, account_id, note, created_at
  FROM (
    SELECT e.invoice_id,
           ROW_NUMBER() OVER (PARTITION BY e.invoice_id ORDER BY e.pos) AS seq,
           json_extract(e.entry, '$.amountCents') AS amount_cents,
           substr(json_extract(e.entry, '$.paidDate'), 1, 10) AS paid_date,
           json_extract(e.entry, '$.accountId') AS account_id,
           json_extract(e.entry, '$.note') AS note,
           COALESCE(datetime(json_extract(e.entry, '$.recordedAt')),
                    (SELECT updated_at FROM invoices WHERE id = e.invoice_id)) AS created_at
      FROM m0034_entries e
     WHERE json_extract(e.entry, '$.amountCents') > 0
    UNION ALL
    SELECT id, 1, paid_amount_cents, substr(paid_at, 1, 10), NULL,
           'Übernommen aus dem Altbestand (ohne Zahlungshistorie)', updated_at
      FROM invoices
     WHERE paid_amount_cents > 0
       AND (payment_history_json IS NULL OR json_array_length(payment_history_json) = 0)
  );

DROP VIEW m0034_entries;

-- Das Audit hält neben Anzahl und Summe das bisherige und das aus dem Buch
-- abgeleitete Zahldatum fest (das UPDATE darunter gleicht es an).
INSERT INTO audit_log (action, entity_type, entity_id, details_json)
SELECT 'invoice.payments_migrated', 'invoice', p.invoice_id,
       json_object('payments', COUNT(*), 'amount', SUM(p.amount_cents), 'migration', 34,
                   'paid_at_before', (SELECT paid_at FROM invoices WHERE id = p.invoice_id),
                   'paid_at', MAX(p.paid_date))
  FROM invoice_payments p
 GROUP BY p.invoice_id;

-- paid_at ist ab hier aus dem Buch abgeleitet (ADR 0042 Nr. 7): jüngstes
-- Zahldatum. Der Altbestand trug das Datum der zuletzt ERFASSTEN Zahlung
-- (record_payment bis Schema 33) und teils eine Uhrzeit ('JJJJ-MM-TT
-- 00:00:00'). Vor den Triggern unten; 0028 lässt paid_at nach dem
-- Festschreiben offen, trg_invoices_canceled_final (0032) sperrt nur status
-- und canceled_*. Verworfene Entwürfe haben keine Zahlungen und sind
-- zusätzlich ausgenommen (0032 friert sie ganz ein).
UPDATE invoices
   SET paid_at = (SELECT MAX(p.paid_date) FROM invoice_payments p
                   WHERE p.invoice_id = invoices.id)
 WHERE paid_amount_cents > 0
   AND voided_at IS NULL
   AND paid_at IS NOT (SELECT MAX(p.paid_date) FROM invoice_payments p
                        WHERE p.invoice_id = invoices.id);

-- ============================================================================
-- Buch: append-only
-- ============================================================================
CREATE TRIGGER trg_invoice_payments_no_update
BEFORE UPDATE ON invoice_payments
BEGIN
    SELECT RAISE(ABORT, 'invoice_payments ist append-only (kein UPDATE; Korrektur über eine Gegenbuchung).');
END;

CREATE TRIGGER trg_invoice_payments_no_delete
BEFORE DELETE ON invoice_payments
BEGIN
    SELECT RAISE(ABORT, 'invoice_payments ist append-only (kein DELETE; Korrektur über eine Gegenbuchung).');
END;

-- Neue Buchungen (nach der Übernahme angelegt, sonst scheiterte sie an
-- abgeschlossenen Jahren und stornierten Rechnungen). Ein Trigger mit den
-- Prüfungen in fester Reihenfolge (die erste verletzte bestimmt die Meldung):
--   1. nur auf festgeschriebene, nicht verworfene, nicht stornierte
--      Ausgangsrechnungen, die selbst kein Storno-Beleg sind und keinen
--      festgeschriebenen Storno haben;
--   2. nur in einem offenen Geschäftsjahr (zweite Linie hinter
--      fiscal_year::guard);
--   3. Gegenbuchung: Zahlung noch nicht storniert; Original derselben
--      Rechnung, eine Zahlung (keine Gegenbuchung), genau der umgekehrte
--      Betrag, dasselbe Datum und Konto; kein offener Storno-Entwurf (sonst
--      änderte sich die Erstattung eines Storno-Jahres rückwirkend);
--   4. laufende Nummer lückenlos;
--   5. Summe zwischen 0 und dem Bruttobetrag (keine Überzahlung).
CREATE TRIGGER trg_invoice_payments_insert_checks
BEFORE INSERT ON invoice_payments
BEGIN
    SELECT RAISE(ABORT, 'invoice_payments: nur auf festgeschriebene, nicht stornierte Ausgangsrechnungen.')
     WHERE (SELECT i.locked_at IS NULL
                OR i.voided_at IS NOT NULL
                OR i.status = 'canceled'
                OR i.is_storno_for IS NOT NULL
                OR i.direction != 'issued'
                OR EXISTS (SELECT 1 FROM invoices s
                            WHERE s.is_storno_for = i.id AND s.locked_at IS NOT NULL)
              FROM invoices i WHERE i.id = NEW.invoice_id) IS NOT 0;

    SELECT RAISE(ABORT, 'invoice_payments: das Geschäftsjahr des Zahldatums ist abgeschlossen.')
     WHERE EXISTS (SELECT 1 FROM fiscal_year_locks
                    WHERE fiscal_year = CAST(substr(NEW.paid_date, 1, 4) AS INTEGER));

    SELECT RAISE(ABORT, 'invoice_payments: die Zahlung ist bereits storniert.')
     WHERE NEW.kind = 'reversal'
       AND EXISTS (SELECT 1 FROM invoice_payments x
                    WHERE x.reverses_payment_id = NEW.reverses_payment_id);

    SELECT RAISE(ABORT, 'invoice_payments: Gegenbuchung passt nicht zur Zahlung (Rechnung, Betrag, Datum, Konto) oder es gibt einen offenen Storno-Entwurf.')
     WHERE NEW.kind = 'reversal'
       AND ((SELECT o.invoice_id = NEW.invoice_id
                AND o.kind = 'payment'
                AND o.amount_cents = -NEW.amount_cents
                AND o.paid_date = NEW.paid_date
                AND o.account_id IS NEW.account_id
               FROM invoice_payments o WHERE o.id = NEW.reverses_payment_id) IS NOT 1
            OR EXISTS (SELECT 1 FROM invoices s
                        WHERE s.is_storno_for = NEW.invoice_id
                          AND s.locked_at IS NULL AND s.voided_at IS NULL));

    SELECT RAISE(ABORT, 'invoice_payments: laufende Nummer nicht fortlaufend.')
     WHERE NEW.seq IS NOT (SELECT COALESCE(MAX(seq), 0) + 1 FROM invoice_payments
                            WHERE invoice_id = NEW.invoice_id);

    SELECT RAISE(ABORT, 'invoice_payments: Summe der Zahlungen außerhalb von 0 bis Bruttobetrag (Überzahlung).')
     WHERE (SELECT COALESCE(SUM(amount_cents), 0) FROM invoice_payments
             WHERE invoice_id = NEW.invoice_id) + NEW.amount_cents
           NOT BETWEEN 0 AND (SELECT gross_amount_cents FROM invoices WHERE id = NEW.invoice_id);
END;

-- ============================================================================
-- Rechnung: Cache an das Buch gebunden
-- ============================================================================
-- Die Altbestand-Historie ist eingefroren.
CREATE TRIGGER trg_invoices_payment_history_frozen
BEFORE UPDATE ON invoices
WHEN NEW.payment_history_json IS NOT OLD.payment_history_json
BEGIN
    SELECT RAISE(ABORT, 'invoices.payment_history_json ist Altbestand und unveränderlich; Zahlungen stehen in invoice_payments.');
END;

-- Neue Rechnungen beginnen ohne Zahlung.
CREATE TRIGGER trg_invoices_payment_cache_insert
BEFORE INSERT ON invoices
WHEN NEW.paid_amount_cents != 0
  OR NEW.paid_at IS NOT NULL
  OR NEW.payment_history_json IS NOT NULL
  OR NEW.status IN ('partially_paid','paid')
BEGIN
    SELECT RAISE(ABORT, 'invoices: Zahlungen nur über invoice_payments (neue Rechnung ohne Zahlung).');
END;

-- Geänderte Cache-Felder müssen zum Buch passen (ADR 0042 Nr. 7): Summe und
-- jüngstes Zahldatum der nicht stornierten Zahlungen. Ändert sich die Summe
-- oder der Status, muss der neue Status zum Buch passen: 'draft' nur
-- unfestgeschrieben und ohne Zahlung, 'issued'/'sent' ohne Zahlung,
-- 'partially_paid' darunter, 'paid' voll; 'canceled' (Storno mit Erstattung)
-- bindet das Buch nicht.
CREATE TRIGGER trg_invoices_payment_cache_bound
BEFORE UPDATE ON invoices
WHEN (NEW.paid_amount_cents IS NOT OLD.paid_amount_cents
      AND NEW.paid_amount_cents IS NOT
          (SELECT COALESCE(SUM(amount_cents), 0) FROM invoice_payments
            WHERE invoice_id = NEW.id))
  OR (NEW.paid_at IS NOT OLD.paid_at
      AND NEW.paid_at IS NOT
          (SELECT MAX(p.paid_date) FROM invoice_payments p
            WHERE p.invoice_id = NEW.id AND p.kind = 'payment'
              AND NOT EXISTS (SELECT 1 FROM invoice_payments r
                               WHERE r.reverses_payment_id = p.id)))
  OR ((NEW.status IS NOT OLD.status
       OR NEW.paid_amount_cents IS NOT OLD.paid_amount_cents)
      -- Aus 'canceled' heraus wacht trg_invoices_canceled_final (0032).
      AND OLD.status IS NOT 'canceled'
      AND (SELECT CASE NEW.status
                    WHEN 'paid' THEN NOT (s > 0 AND s = NEW.gross_amount_cents)
                    WHEN 'partially_paid' THEN NOT (s > 0 AND s < NEW.gross_amount_cents)
                    WHEN 'issued' THEN s != 0
                    WHEN 'sent' THEN s != 0
                    WHEN 'draft' THEN NOT (NEW.locked_at IS NULL AND s = 0)
                    ELSE 0
                  END
             FROM (SELECT COALESCE(SUM(amount_cents), 0) AS s FROM invoice_payments
                    WHERE invoice_id = NEW.id)) = 1)
BEGIN
    SELECT RAISE(ABORT, 'invoices: Zahlungsstand (paid_amount_cents, paid_at, status) nur passend zum Zahlungsbuch.');
END;

-- ============================================================================
-- Schema-Version 33 → 34
-- ============================================================================
UPDATE app_settings
   SET value = '34',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
