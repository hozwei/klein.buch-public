-- Migration 0025: Eindeutige Positionen je Abo-Vorlage erzwingen.
-- KB-0056 (Review-Bereich Q): `recurring_invoice_items` hatte nur den
-- nicht-eindeutigen Index `idx_recurring_invoice_items_parent`. Das etablierte
-- `invoice_items`-Muster (0001) erzwingt Eindeutigkeit per UNIQUE-Index. Hier
-- nachziehen, damit doppelte `position`-Werte schon in der Stammdaten-Vorlage
-- ausgeschlossen sind (Defense-in-Depth; die Materialisierung in `invoice_items`
-- hat eine eigene UNIQUE-Constraint, die sonst erst spät zuschlagen würde).
--
-- Forward-only: Index droppen + als UNIQUE neu anlegen (gleicher Name). Eine
-- frische DB hat keine Duplikate; das UI/Repo nummeriert Positionen geordnet.

PRAGMA foreign_keys = ON;

DROP INDEX IF EXISTS idx_recurring_invoice_items_parent;
CREATE UNIQUE INDEX idx_recurring_invoice_items_parent
    ON recurring_invoice_items(recurring_invoice_id, position);

UPDATE app_settings
   SET value = '25',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
