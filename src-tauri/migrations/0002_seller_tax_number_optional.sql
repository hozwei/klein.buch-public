-- Migration 0002: tax_number im seller_profile darf NULL sein.
--
-- Rationale: Kleinunternehmer-Onboarding läuft bevor das Finanzamt eine
-- Betriebs-Steuernummer erteilt hat. Profil muss trotzdem speicherbar
-- sein. §14-UStG-Pflichtangaben-Check (Steuernummer ODER USt-IdNr.)
-- läuft in Block 3 beim Rechnungs-Issue, nicht beim Profil-Save.
--
-- SQLite kann NOT NULL nicht ohne Tabellen-Rebuild entfernen.

PRAGMA foreign_keys = OFF;

CREATE TABLE seller_profile_new (
    id                          INTEGER PRIMARY KEY CHECK (id = 1),
    name                        TEXT NOT NULL,
    legal_form                  TEXT,
    street                      TEXT NOT NULL,
    postal_code                 TEXT NOT NULL,
    city                        TEXT NOT NULL,
    country_code                TEXT NOT NULL DEFAULT 'DE',
    tax_number                  TEXT,
    vat_id                      TEXT,
    email                       TEXT NOT NULL,
    phone                       TEXT,
    iban                        TEXT,
    bic                         TEXT,
    logo_filename               TEXT,
    is_kleinunternehmer         INTEGER NOT NULL DEFAULT 1 CHECK (is_kleinunternehmer IN (0,1)),
    waived_paragraph_19_since   TEXT,
    default_pdf_template        TEXT NOT NULL DEFAULT 'default',
    default_currency            TEXT NOT NULL DEFAULT 'EUR',
    updated_at                  TEXT NOT NULL DEFAULT (datetime('now','utc'))
) STRICT;

INSERT INTO seller_profile_new
    SELECT id, name, legal_form, street, postal_code, city, country_code,
           tax_number, vat_id, email, phone, iban, bic, logo_filename,
           is_kleinunternehmer, waived_paragraph_19_since,
           default_pdf_template, default_currency, updated_at
    FROM seller_profile;

DROP TABLE seller_profile;
ALTER TABLE seller_profile_new RENAME TO seller_profile;

PRAGMA foreign_keys = ON;

UPDATE app_settings
   SET value = '2',
       updated_at = datetime('now','utc')
 WHERE key = 'schema_version';
