-- Migration 0019: Paket-Katalog (Block P2). Kategorien sind organisatorisch
-- (mutable). Paket-Header ist mutabel; Inhalt + Preis leben in append-only/
-- unveränderlichen Revisionen (legal_documents-Muster, Migration 0006).
-- „Paket bearbeiten" = NEUE Revision. „Rollback" = NEUE Revision, die eine alte
-- kopiert. Nie Update/Delete einer bestehenden Revision (DB-Trigger erzwingen das).
--
-- schema_version → 19. Migrationen 0001–0018 verbraucht; 0019 ist die nächste.
PRAGMA foreign_keys = ON;

-- Kategorien (Hochzeit, Porträt, …) — mutable, kein Beleginhalt, nie gesnapshottet.
CREATE TABLE package_categories (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_package_categories_sort ON package_categories(sort_order);

-- Paket-Header: mutabler Stammsatz. current_revision zeigt auf die zuletzt
-- veröffentlichte Revision (die das Dropdown beim Einfügen verwendet).
CREATE TABLE packages (
    id                TEXT PRIMARY KEY NOT NULL,
    category_id       TEXT REFERENCES package_categories(id),
    name              TEXT NOT NULL,                 -- interner Katalog-Name, z. B. "Hochzeit klein"
    status            TEXT NOT NULL DEFAULT 'active'
                      CHECK (status IN ('active','archived')),
    current_revision  INTEGER,                       -- Versionsnummer der aktiven Revision
    sort_order        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE INDEX idx_packages_category ON packages(category_id);
CREATE INDEX idx_packages_status   ON packages(status);

-- Revisionen: append-only + Kernfelder unveränderlich (legal_documents-Muster).
-- Edit = neue Revision (revision = max+1). Rollback = neue Revision, die Inhalt
-- einer alten kopiert. Nie Update/Delete einer bestehenden Revision.
CREATE TABLE package_revisions (
    id                       TEXT PRIMARY KEY NOT NULL,
    package_id               TEXT NOT NULL REFERENCES packages(id),
    revision                 INTEGER NOT NULL,        -- monoton pro package_id (1,2,3,…)
    title                    TEXT NOT NULL,           -- Positions-Titel auf dem Beleg
    body_markup              TEXT NOT NULL,           -- Markdown-Subset = Quelle der Wahrheit
    default_unit_price_cents INTEGER NOT NULL,        -- Netto-Cent
    unit_code                TEXT NOT NULL DEFAULT 'C62',
    tax_category_code        TEXT NOT NULL DEFAULT 'E'
                             CHECK (tax_category_code IN ('S','Z','E','AE','K','G','O','L','M')),
    note                     TEXT,                    -- optionaler Änderungs-Kommentar (Audit)
    created_at               TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
CREATE UNIQUE INDEX uq_package_revisions ON package_revisions(package_id, revision);
CREATE INDEX idx_package_revisions_package ON package_revisions(package_id);

CREATE TRIGGER trg_package_revisions_no_delete BEFORE DELETE ON package_revisions
BEGIN SELECT RAISE(ABORT, 'package_revisions are append-only: no delete'); END;
CREATE TRIGGER trg_package_revisions_immutable BEFORE UPDATE ON package_revisions
BEGIN SELECT RAISE(ABORT, 'package_revisions are immutable'); END;

UPDATE app_settings SET value = '19', updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
