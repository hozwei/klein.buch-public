-- Migration 0033: Steuerstatus nach § 19 UStG als Historie (ADR 0042 Nr. 1–6,
-- Lauf 2026.11.1, Block V1).
--
-- Bisher stand der Status als Schalter im Verkäuferprofil
-- (`seller_profile.is_kleinunternehmer`, `waived_paragraph_19_since`): ohne
-- Grund, ohne Historie, nur je Kalenderjahr. Jetzt steht er in der
-- append-only Tabelle `tax_status_changes`: ab `effective_from` gilt `status`,
-- mit `reason` (ADR 0042 Nr. 2). Kein Löschen, kein Ändern; ein irrtümlicher
-- Eintrag wird mit Grund widerrufen (`revoked_at`, `revoke_reason`), wie beim
-- Backup- und Mail-Protokoll (§ 146 Abs. 4 AO, GoBD Rz. 58 ff.).
--
-- Die beiden Spalten im Verkäuferprofil bleiben als Spiegel stehen (ein DROP
-- verlangte einen Neuaufbau der Tabelle); maßgeblich sind sie nicht mehr,
-- gepflegt werden sie nur noch vom Server (db::repo::tax_status).
--
-- Übernahme des Bestands (ADR 0042 Nr. 6): Für jedes Jahr entsteht dasselbe
-- Ergebnis wie bisher (`is_klein_for_year`: Jahre vor dem Jahr des
-- Verzichtsdatums §19, ab dann der Schalter), damit abgeschlossene Jahre und
-- ihre Snapshots unverändert bleiben:
--
--   * kein Profil                         → kein Eintrag (ohne Eintrag gilt §19)
--   * Schalter an                         → Anfangsstand §19
--   * Schalter aus, Verzichtsdatum D      → Anfangsstand §19 und ab 01.01. des
--                                           Jahres von D Regelbesteuerung
--                                           (Grund „legacy“), Bindung bis
--                                           01.01. (Jahr + 5) nach Nr. 2 — der
--                                           Grund des alten Wechsels ist
--                                           unbekannt, die Bindung des
--                                           Verzichts gilt vorsichtshalber
--   * Schalter aus, ohne gültiges Datum   → Anfangsstand Regelbesteuerung
--
-- Ein Verzichtsdatum gilt wie bisher nur, wenn es ein Datum JJJJ-MM-TT ist
-- (die App schrieb nie etwas anderes). Verzicht-/Rückkehr-Paare aus dem
-- audit_log werden nicht nachgebildet: Sie änderten das bisherige Ergebnis
-- nicht (eine Rückkehr löschte das Datum), eine Rekonstruktion würde
-- festgeschriebene Jahre kippen.
--
-- IDs: UUIDv7 lässt sich in SQL nicht erzeugen. Die übernommenen Einträge
-- tragen feste IDs im UUIDv7-Format mit Zeitstempel 0
-- (00000000-0000-7000-8000-00000000000N): gültige Version und Variante,
-- erkennbar als „aus der Migration“, und deterministisch — ein Restore eines
-- älteren Backups ergibt nach der Vorwärtsmigration dieselben IDs. Neue
-- Einträge erzeugt der Server mit `Uuid::now_v7()`.

CREATE TABLE tax_status_changes (
    id               TEXT PRIMARY KEY NOT NULL,
    -- Wirkungsdatum (JJJJ-MM-TT); „0001-01-01“ = seit Beginn (Anfangsstand).
    effective_from   TEXT NOT NULL CHECK (date(effective_from) IS effective_from),
    status           TEXT NOT NULL CHECK (status IN ('kleinunternehmer','regelbesteuerung')),
    reason           TEXT NOT NULL CHECK (reason IN
                     ('initial','verzicht','wegfall_vorjahr','wegfall_laufend',
                      'wegfall_gruendungsjahr','rueckkehr','legacy')),
    -- Erster Tag, an dem nach einem Verzicht die Rückkehr möglich ist.
    binding_until    TEXT CHECK (binding_until IS NULL OR date(binding_until) IS binding_until),
    -- Belegnummer des Umsatzes, mit dem eine Grenze überschritten wurde.
    trigger_document TEXT CHECK (trigger_document IS NULL
                                 OR length(trigger_document) BETWEEN 1 AND 100),
    note             TEXT CHECK (note IS NULL OR length(note) <= 2000),
    created_at       TEXT NOT NULL DEFAULT (datetime('now','utc')),
    revoked_at       TEXT,
    revoke_reason    TEXT CHECK (revoke_reason IS NULL
                                 OR length(trim(revoke_reason)) BETWEEN 1 AND 2000),
    -- Grund ↔ Status (ADR 0042 Nr. 2).
    CHECK (reason IN ('initial','legacy')
           OR (reason IN ('verzicht','wegfall_vorjahr','wegfall_laufend',
                          'wegfall_gruendungsjahr') AND status = 'regelbesteuerung')
           OR (reason = 'rueckkehr' AND status = 'kleinunternehmer')),
    -- Wirkung zum 1. Januar: Verzicht, Wegfall wegen des Vorjahres, Rückkehr,
    -- übernommener Wechsel.
    CHECK (reason NOT IN ('verzicht','wegfall_vorjahr','rueckkehr','legacy')
           OR substr(effective_from, 6) = '01-01'),
    -- Anfangsstand seit Beginn; alle anderen nicht vor 1900.
    CHECK ((reason = 'initial') = (effective_from = '0001-01-01')),
    CHECK (effective_from = '0001-01-01' OR effective_from >= '1900-01-01'),
    -- Bindung nur nach einem Verzicht (bzw. übernommen), fünf Kalenderjahre
    -- einschließlich des Wirkungsjahres.
    CHECK (binding_until IS NULL OR reason IN ('verzicht','legacy')),
    CHECK (reason NOT IN ('verzicht','legacy')
           OR binding_until IS date(effective_from, '+5 years')),
    -- Widerruf nur mit Grund.
    CHECK ((revoked_at IS NULL) = (revoke_reason IS NULL))
) STRICT;

-- Keine zwei gültigen Einträge mit demselben Wirkungsdatum.
CREATE UNIQUE INDEX uq_tax_status_changes_active
    ON tax_status_changes(effective_from)
 WHERE revoked_at IS NULL;

CREATE TRIGGER trg_tax_status_changes_no_delete
BEFORE DELETE ON tax_status_changes
BEGIN
    SELECT RAISE(ABORT, 'tax_status_changes ist append-only (kein DELETE).');
END;

-- UPDATE nur als Widerruf: revoked_at und revoke_reason von leer auf einen
-- Wert, alle anderen Spalten unverändert; danach endgültig. Eine spätere
-- Migration mit neuen Spalten muss diese Liste erweitern.
CREATE TRIGGER trg_tax_status_changes_revoke_only
BEFORE UPDATE ON tax_status_changes
WHEN OLD.revoked_at IS NOT NULL
  OR NEW.revoked_at IS NULL
  OR NEW.id IS NOT OLD.id
  OR NEW.effective_from IS NOT OLD.effective_from
  OR NEW.status IS NOT OLD.status
  OR NEW.reason IS NOT OLD.reason
  OR NEW.binding_until IS NOT OLD.binding_until
  OR NEW.trigger_document IS NOT OLD.trigger_document
  OR NEW.note IS NOT OLD.note
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'tax_status_changes: nur ein Widerruf ist erlaubt (revoked_at und revoke_reason einmalig setzen).');
END;

-- ============================================================================
-- Übernahme des Bestands (ADR 0042 Nr. 6)
-- ============================================================================
INSERT INTO tax_status_changes (id, effective_from, status, reason, note)
SELECT '00000000-0000-7000-8000-000000000001', '0001-01-01',
       CASE WHEN is_kleinunternehmer = 0
                 AND (waived_paragraph_19_since IS NULL
                      OR date(waived_paragraph_19_since) IS NOT waived_paragraph_19_since)
            THEN 'regelbesteuerung'
            ELSE 'kleinunternehmer'
       END,
       'initial',
       'Aus den Firmendaten übernommen (Migration 0033).'
  FROM seller_profile
 WHERE id = 1;

INSERT INTO tax_status_changes (id, effective_from, status, reason, binding_until, note)
SELECT '00000000-0000-7000-8000-000000000002',
       substr(waived_paragraph_19_since, 1, 4) || '-01-01',
       'regelbesteuerung',
       'legacy',
       date(substr(waived_paragraph_19_since, 1, 4) || '-01-01', '+5 years'),
       'Aus den Firmendaten übernommen (Migration 0033): Wechsel zur Regelbesteuerung am '
           || waived_paragraph_19_since
           || ', wirksam ab Jahresbeginn wie bisher; Grund nicht erfasst.'
  FROM seller_profile
 WHERE id = 1
   AND is_kleinunternehmer = 0
   AND waived_paragraph_19_since IS NOT NULL
   AND date(waived_paragraph_19_since) IS waived_paragraph_19_since;

INSERT INTO audit_log (action, entity_type, entity_id, details_json)
SELECT 'tax_status.migrated', 'tax_status_change', id,
       json_object('effective_from', effective_from, 'status', status,
                   'reason', reason, 'binding_until', binding_until)
  FROM tax_status_changes;

-- ============================================================================
-- Hinweis zu den Umsatzgrenzen (ADR 0042 Nr. 5)
-- ============================================================================
INSERT OR IGNORE INTO notification_rules
    (id, rule_type, label, enabled, config_json, deliver_in_app, deliver_os_native)
VALUES
    ('rule_tax_threshold', 'custom', 'Umsatzgrenzen nach §19 UStG', 1, '{}', 1, 0);

-- ============================================================================
-- Schema-Version 32 → 33
-- ============================================================================
UPDATE app_settings
   SET value = '33',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
