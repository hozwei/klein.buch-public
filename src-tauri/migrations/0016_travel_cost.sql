-- Migration 0016: Anfahrtskosten-Rechner (Phase 3, Block P1).
--
-- Ein konfigurierbarer Kilometersatz (Netto-Cent). Im Angebot/Rechnung erzeugt
-- der "Anfahrt"-Block daraus eine ganz normale Position (km × Satz). Kein eigenes
-- Beleg-Konzept, keine Immutability — die Position lebt in invoice_items/
-- quote_items wie jede andere und unterliegt deren Lock-Regeln.
--
-- schema_version → 16. Migrationen 0001–0015 verbraucht; 0016 ist die nächste.

PRAGMA foreign_keys = ON;

INSERT INTO app_settings (key, value) VALUES
    ('travel_cost_per_km_cents', '0'),     -- Netto-Cent je km; 0 = noch nicht gesetzt
    ('travel_round_trip_default', '0');    -- '1' = UI schlägt Hin&Rück (×2) vor (reiner UI-Default)

UPDATE app_settings
   SET value = '16',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
