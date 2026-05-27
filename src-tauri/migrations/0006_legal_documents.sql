-- Migration 0006: Rechtsdokumente (AGB + Datenschutzrichtlinie) zentral
-- versioniert + append-only Verknüpfung mit Angeboten. Phase 2A, Block 8.
--
-- Anforderung (Manuel, 2026-05-20): AGB und Datenschutzrichtlinie als
-- versionierte PDFs (Upload pro Version), nicht löschbar (GoBD append-only,
-- kein Löschen-UI — wie Archive/Audit), und jede an ein Angebot ausgegebene
-- Version fest + unveränderlich mit dem Angebot verknüpft (rechtlicher
-- Nachweis "welche Version ging an welches Angebot / an wen"). Ausgabe immer
-- als Bundle (Angebot + AGB + Datenschutz), Druck (zusammengeführtes PDF) oder
-- Mail (Multi-Attachment, Block 5).
--
-- Design-Entscheidung (Manuel): Speicherung als PDF-Upload pro Version. Die
-- PDF-Bytes liegen write-once in `archive_entries` (ArchiveKind::LegalDocument);
-- hier nur die Metadaten + Versions-/Aktiv-Status.
--
-- GoBD-/Design-Tension: Angebote sind ab `sent` gelockt (trg_quotes_immutable,
-- 0005). Die Verknüpfung Angebot↔Version darf daher KEIN mutables Quote-
-- Kernfeld sein, sondern ist eine eigene append-only Assoziation
-- (`quote_legal_documents`), gesetzt bei der Bundle-/Versand-Erzeugung.
--
-- schema_version → 6. Migrationen 0001–0005 verbraucht; 0006 ist die nächste.

PRAGMA foreign_keys = ON;

-- Zentrale, versionierte Rechtsdokumente. Eine Zeile = eine Version eines
-- doc_type. `version` zählt pro doc_type monoton hoch (1, 2, 3, …). Höchstens
-- eine Version pro doc_type ist gleichzeitig aktiv.
CREATE TABLE legal_documents (
    id                  TEXT PRIMARY KEY NOT NULL,
    doc_type            TEXT NOT NULL CHECK (doc_type IN ('agb','privacy')),
    version             INTEGER NOT NULL,
    title               TEXT NOT NULL,                       -- Anzeige-Label, z. B. "AGB Stand 05/2026"
    archive_entry_id    TEXT NOT NULL REFERENCES archive_entries(id),  -- write-once PDF
    is_active           INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0,1)),
    created_at          TEXT NOT NULL DEFAULT (datetime('now','utc')),
    activated_at        TEXT,
    deactivated_at      TEXT
) STRICT;
-- Lückenlose, eindeutige Versionierung pro doc_type.
CREATE UNIQUE INDEX uq_legal_documents_type_version ON legal_documents(doc_type, version);
-- Höchstens eine aktive Version pro doc_type (partial unique index).
CREATE UNIQUE INDEX uq_legal_documents_active ON legal_documents(doc_type) WHERE is_active = 1;
CREATE INDEX idx_legal_documents_type ON legal_documents(doc_type);

-- Append-only: Versionen werden nie gelöscht (10-Jahres-Aufbewahrung, kein
-- Löschen-UI — wie archive_entries/audit_log).
CREATE TRIGGER trg_legal_documents_no_delete BEFORE DELETE ON legal_documents
BEGIN SELECT RAISE(ABORT, 'legal_documents are append-only: no delete'); END;

-- Kernfelder unveränderlich. Nur is_active/activated_at/deactivated_at dürfen
-- sich ändern (Aktivieren/Deaktivieren einer Version).
CREATE TRIGGER trg_legal_documents_immutable BEFORE UPDATE ON legal_documents
WHEN NEW.id != OLD.id
  OR NEW.doc_type != OLD.doc_type
  OR NEW.version != OLD.version
  OR NEW.title != OLD.title
  OR NEW.archive_entry_id != OLD.archive_entry_id
  OR NEW.created_at != OLD.created_at
BEGIN SELECT RAISE(ABORT, 'legal_documents core fields are immutable'); END;

-- Append-only Assoziation: welche Legal-Version ging fest mit welchem Angebot
-- raus. `version` ist als Snapshot mitgeführt (rechtlicher Nachweis bleibt
-- lesbar, selbst wenn jemand die legal_documents-Zeile-Anzeige ändert — die
-- Kernfelder sind ohnehin immutable, aber der Snapshot ist robust).
CREATE TABLE quote_legal_documents (
    id                  TEXT PRIMARY KEY NOT NULL,
    quote_id            TEXT NOT NULL REFERENCES quotes(id),
    legal_document_id   TEXT NOT NULL REFERENCES legal_documents(id),
    doc_type            TEXT NOT NULL CHECK (doc_type IN ('agb','privacy')),
    version             INTEGER NOT NULL,
    bound_at            TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;
-- Genau eine Bindung pro (Angebot, doc_type) — idempotent + eindeutig.
CREATE UNIQUE INDEX uq_quote_legal_documents ON quote_legal_documents(quote_id, doc_type);
CREATE INDEX idx_quote_legal_documents_quote ON quote_legal_documents(quote_id);

CREATE TRIGGER trg_quote_legal_documents_no_delete BEFORE DELETE ON quote_legal_documents
BEGIN SELECT RAISE(ABORT, 'quote_legal_documents are append-only: no delete'); END;

CREATE TRIGGER trg_quote_legal_documents_immutable BEFORE UPDATE ON quote_legal_documents
BEGIN SELECT RAISE(ABORT, 'quote_legal_documents are immutable'); END;

UPDATE app_settings
   SET value = '6',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
