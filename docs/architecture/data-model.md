# Datenmodell-Referenz

> Vertiefung zu `../ARCHITECTURE.md`. Diese Datei beschreibt das SQLite-Schema
> auf Tabellen-Ebene: Verantwortung, Schlüssel-Felder, Constraints, Trigger,
> Migrations-Historie. Die *vollständigen* `CREATE TABLE`-Statements liegen
> in `klein-buch/src-tauri/migrations/*.sql` und sind die kanonische Quelle —
> diese Doku ist die navigationsfreundliche Sicht darauf.

**Schema-Stand: v33.** Migrationen 0001–0033, forward-only; nächste freie
Migration 0034. Schema-Version liegt in `app_settings.schema_version`; geprüft
wird beim Entsperren gegen `EXPECTED_SCHEMA_VERSION` (33) — eine neuere DB
lässt sich nicht entsperren (Down-Migration-Schutz, §4).

**Globale Konventionen** (ADR 0003):

- **`STRICT`-Tabellen** überall — SQLite prüft die Spalten-Typen wirklich.
- **`PRAGMA foreign_keys = ON`** für jeden Pool.
- **UUIDv7-PKs** als `TEXT` (kanonische 36-Zeichen-Form, zeitlich sortierbar).
- **Geld als Integer-Cents** (`INTEGER NOT NULL`). Niemals `REAL` für Beträge.
  Gerechnet wird in `i64` (`domain::money`). Einzige dokumentierte Ausnahme:
  `Menge × Einzelpreis` und `Netto × Satz` laufen über `f64` mit einmaliger
  kaufmännischer Rundung je Position bzw. Steuergruppe — die Menge ist kein
  Geldbetrag (höchstens 4 Nachkommastellen, wie CII sie überträgt), bis 2^53
  Cent ist das Produkt exakt. AfA, betrieblicher Anteil und Fehlertexte sind
  ganzzahlig (ADR 0039 Nr. 134, R1-S2-FLOAT). Die USt eines Belegs entsteht je
  (Steuerart, Satz) aus der Nettosumme (EN 16931 BR-CO-17). Grenzen: Menge
  ≤ 1 000 000, Einzelpreis (Betrag) ≤ 1 Mrd. €, Satz ≤ 100 %, Summen geprüft;
  jeder einzeln erfasste Betrag (Kosten, Zahlung, Anschaffung, Abgangserlös,
  Privatbewegung, Kosten-Abo) ≤ 1 Mrd. € (ADR 0039 Nr. 137).
- **Datum/Zeit als ISO-8601-`TEXT`** (`YYYY-MM-DD` bzw. `YYYY-MM-DDTHH:MM:SS`).
- **Booleans als `INTEGER NOT NULL CHECK (… IN (0,1))`**.
- **Forward-only-Migrationen** mit aufsteigender Nummer; Down-Migrations
  existieren nicht.

---

## 1. Migrations-Liste

| Nr. | Datei | Zweck |
|---|---|---|
| 0001 | `0001_init.sql` | Foundation: `seller_profile`, `contacts`, `invoices`, `invoice_items`, `doc_number_counters`, `archive_entries`, `audit_log`, `app_settings` + GoBD-Trigger (Invoice-Lock, Audit-append-only, Archive-immutable) |
| 0002 | `0002_seller_tax_number_optional.sql` | `seller_profile.tax_number` nullable (§19 hat oft keine StNr.) |
| 0003 | `0003_invoices_seller_tax_number_optional.sql` | Buyer-Snapshot-Spalten in `invoices` (Tax-No optional) |
| 0004 | `0004_invoices_buyer_snapshot.sql` | Vollständiger Buyer-Snapshot in `invoices` (Name/Adresse einfrieren beim Lock — GoBD) |
| 0005 | `0005_quotes.sql` | `quotes` + `quote_items` mit eigenem Belegkreis `AN-…`, Trigger `trg_quotes_immutable` |
| 0006 | `0006_legal_documents.sql` | `legal_documents` (AGB/Datenschutz versioniert) + `quote_legal_documents` (append-only Bindung) |
| 0007 | `0007_expenses.sql` | `expenses` + Trigger `trg_expenses_immutable` (Kosten sofort gelockt) |
| 0008 | `0008_private_movements.sql` | `private_movements` (EÜR-neutral, immutable nach Insert) + `payment_accounts` |
| 0009 | `0009_recurring.sql` | `recurring_subscriptions` (eingangsseitige Abos für Kosten) |
| 0010 | `0010_assets.sql` | `assets` (Anlagenverzeichnis) + Trigger `trg_assets_immutable` (nach GJ-Lock) |
| 0011 | `0011_depreciation.sql` | `depreciation_entries` + Trigger `trg_depreciation_immutable` |
| 0012 | `0012_notifications.sql` | `notifications` + `notification_rules` (In-App-Inbox) |
| 0013 | `0013_fiscal_year_locks.sql` | `fiscal_year_locks` (append-only, EÜR-Snapshot pro abgeschlossenes Jahr) |
| 0014 | `0014_oauth.sql` | `mail_accounts` um nicht-geheime OAuth-Metadaten erweitert (Tenant, Client-ID, Postfach, Scopes, Token-Ablauf); der Refresh-Token lag in der Desktop-Fassung im OS-Keychain, seit 0031 in `secrets` |
| 0015 | `0015_email_log.sql` | `email_log` (append-only, Versand-Protokoll inkl. Provider-Antwort) |
| 0016 | `0016_travel_cost.sql` | Anfahrt-Settings (km-Satz pro `seller_profile`) |
| 0017 | `0017_payment_on_invoice.sql` | `payment_account_id`-FK in `invoices` (Bankverbindung der Rechnung) |
| 0018 | `0018_payment_note.sql` | `invoices.payment_note` (Bezahlt-Hinweis, nur PDF, EÜR-neutral) |
| 0019 | `0019_packages.sql` | `package_categories`, `packages`, `package_revisions` (append-only) |
| 0020 | `0020_package_item_provenance.sql` | `source_package_id`/`source_package_revision`-Spalten in `invoice_items`/`quote_items` (Soft-Zeiger) |
| 0021 | `0021_package_item_title.sql` | `description_title`-Spalte in `invoice_items`/`quote_items` (PDF-Zeile, XML=Body) |
| 0022 | `0022_contact_anonymization.sql` | `contacts.anonymized_at` für DSGVO Art. 17 |
| 0023 | `0023_quote_buyer_snapshot.sql` | Buyer-Snapshot-Spalten auch in `quotes` (Anonymize-Resilienz) |
| 0024 | `0024_recurring_invoices.sql` | `recurring_invoices` + `recurring_invoice_items` (ausgangsseitige Abo-Rechnungen) |
| 0025 | `0025_recurring_invoice_item_position_unique.sql` | UNIQUE-Index `(recurring_invoice_id, position)` |
| 0026 | `0026_backup_log.sql` | `backup_log` (append-only) — Backup-Protokoll je Ziel |
| 0027 | `0027_backup_result_rule.sql` | `notification_rules` um `rule_backup_result` ergänzt (Erfolg/Fehler) |
| 0028 | `0028_append_only_hardening.sql` | Append-only-Trigger nachgezogen (R1-Review): zusätzliche no-update/no-delete-Trigger auf Audit-relevante Hilfstabellen, Storno-Paar als atomare Transaktion abgesichert |
| 0029 | `0029_drop_folder.sql` | Drop-Folder-Settings (`drop_folder_enabled`, `drop_folder_path`) + Notification-Regeln `rule_drop_folder_import_ok` (default off, Inbox-only) und `rule_drop_folder_import_failed` (default on, Inbox + OS-Toast) — ADR 0037. Im Server ohne Wirkung: Drop-Folder entfallen (E11, Code entfernt in W11), OS-Benachrichtigungen gibt es nicht; die Zeilen bleiben (forward-only). |
| 0030 | `0030_rename_drop_folder_labels.sql` | UI-Label-Update auf den Notification-Regeln aus 0029 (Block PV1-RENAME): „Drop-Folder: …" → „Rechnungs-Eingang: …" (CLAUDE.md-UI-Sprachregel, Plain-Language-Hardline). Code-Identifier (rule_id, Settings-Keys, Route, Module) bleiben englisch. Im Server wie 0029 ohne Wirkung. |
| 0031 | `0031_secrets.sql` | `secrets` (STRICT) — Zugangsdaten in der verschlüsselten DB statt im OS-Keychain (Plan W8, ADR 0039 E6, Amendment ADR 0011/0028): Schlüssel `smtp:<konto-id>`, `oauth:<konto-id>`, `sftp:<ziel-id>`; nie im Migrations- oder DSGVO-Export, nie in Log oder Audit |
| 0032 | `0032_gobd_hardening.sql` | GoBD- und Schema-Absicherungen (Lauf F, Block F2a; ADR 0039 Nr. 124): Unique-Index `ux_invoices_one_locked_storno` (höchstens ein festgeschriebener Storno je Original; Vorab-Prüfung bricht bei Bestandsduplikaten mit klarer Meldung ab), `invoices.voided_at`/`void_reason` (Entwurf verwerfen ohne Löschen), `archive_integrity_checks.files_missing`/`files_unreadable`/`missing_archive_ids_json`/`unreadable_archive_ids_json`, Trigger für Positionen festgeschriebener Belege, Archiv ohne DELETE, endgültigen Storno, zurückgezogene Angebote, verworfene Entwürfe und den Anlagen-Abgang, Partial Unique Index `uq_payment_accounts_default` (Bestand mit Audit bereinigt), Seed `rule_recurring_due` |
| 0033 | `0033_tax_status_history.sql` | Steuerstatus nach § 19 UStG als Historie (Lauf 2026.11.1, Block V1; ADR 0042 Nr. 1–6): `tax_status_changes` (STRICT, append-only: kein DELETE, UPDATE nur als einmaliger Widerruf mit Grund; CHECKs Grund ↔ Status, 1. Januar bei Verzicht/Vorjahr/Rückkehr, Bindung = 01.01. Wirkungsjahr + 5; Teil-Unique-Index je Wirkungsdatum), Übernahme des Bestands aus `seller_profile` mit festen IDs im UUIDv7-Format (für jedes Jahr dasselbe Ergebnis wie bisher), Audit `tax_status.migrated`, Seed `rule_tax_threshold` |

**Down-Migrations gibt es nicht.** Wer eine Migration „rückgängig" braucht,
muss eine neue, höher nummerierte Migration schreiben, die den Zustand
korrigiert.

---

## 2. Tabellen-Übersicht

### 2a. Stammdaten

**`seller_profile`** — Verkäufer (es gibt genau einen Eintrag pro Installation).
Felder: `id`, `legal_name`, `address_*`, `tax_number` (optional, §19),
`vat_id` (optional), `is_kleinunternehmer` (Default 1) und `waived_paragraph_19_since` —
seit Migration 0033 nur noch Spiegel der Statushistorie, gepflegt von
`db::repo::tax_status`, von keinem Code gelesen (ADR 0042 Nr. 1) —, `logo_archive_id`, `signature_archive_id`,
`default_pdf_template`, `travel_*` (km-Satz), Banking-Felder.

**`tax_status_changes`** — Steuerstatus nach § 19 UStG als Historie (Migration
0033, ADR 0042): `id` (UUIDv7; übernommener Bestand mit festen IDs),
`effective_from` (`0001-01-01` = seit Beginn), `status`
(`kleinunternehmer`/`regelbesteuerung`), `reason` (`initial`, `verzicht`,
`wegfall_vorjahr`, `wegfall_laufend`, `wegfall_gruendungsjahr`, `rueckkehr`,
`legacy`), `binding_until` (nach einem Verzicht: erster Tag der möglichen
Rückkehr), `trigger_document`, `note`, `created_at`, `revoked_at`/`revoke_reason`.
Ohne gültigen Eintrag gilt §19. Den Status an einem Datum liefert
`domain::tax_status::TaxStatusHistory::status_at`; maßgeblich ist bei
Ausgangsbelegen das Leistungsdatum, sonst das Rechnungs- bzw. Angebotsdatum,
bei Kosten das Belegdatum.

**`contacts`** — Kunden + Lieferanten. Felder: `id`, `display_name`,
`legal_name`, `kind` (`customer`/`supplier`/`both`), `address_*`, `email`,
`tax_number`, `vat_id`, `notes` (interne, **nicht** im DSGVO-Export),
`anonymized_at` (DSGVO Art. 17), `archived_at`. Anonymisierung: `display_name`
wird zu `'Anonymisiert #<8hex>'`, alle PII auf NULL — die in Belege
eingefrorenen Buyer-Snapshots bleiben (GoBD gewinnt).

**`payment_accounts`** — Zahlungs-Konten (Bank/Bar/PayPal). Felder: `id`,
`label`, `kind`, `iban`, `bic`, `bank_name`, `is_active`, `is_default`.

**`mail_accounts`** — Versand-Konten. Felder: `id`, `label`, `auth_type`
(`smtp_password`/`oauth_microsoft`; OAuth ist im Server gesperrt, E9),
`smtp_host`, `smtp_port`, `smtp_user`, `smtp_use_tls`, `keychain_service_id`
(Name aus der Keychain-Zeit; heute nur Kennzeichen, dass ein Passwort
hinterlegt sein kann — das Passwort selbst steht in `secrets` unter
`smtp:<id>`), `from_email`, `from_name`, `is_default`, `last_used_at`,
`created_at`, OAuth-Metadaten aus 0014 (`oauth_tenant_id`, `oauth_client_id`,
`oauth_account_email`, `oauth_scopes`, `oauth_token_expires_at`; der
Refresh-Token steht in `secrets` unter `oauth:<id>`). Ändern sich Host, Port
oder Benutzer, verfällt das gespeicherte Passwort (Amendment ADR 0011).

### 2b. Beleg-Stamm (festschreibbar)

**`invoices`** — Ausgangsrechnungen. Felder: `id`, `invoice_number`
(`RE-{YYYY}-{NNNN}` oder `ST-…` für Storno), `date`, `delivery_date`,
`contact_id`, `is_kleinunternehmer`, `direction` (`outgoing`/`incoming`),
`fiscal_year` (Jahr des Belegdatums, auch beim Storno — ADR 0039 Nr. 122), `status`
(`draft`/`issued`/`sent`/`canceled`), `is_storno_for` (FK, NULL bei Original;
höchstens ein festgeschriebener Storno je Original, Migration 0032), `derived_from_quote_id`, `subtotal_cents`,
`tax_amount_cents`, `total_cents`, `paid_amount_cents`, `paid_at`, `sent_at`,
`canceled_at`, `locked_at`, `pdf_archive_id`, `xml_archive_id`, Buyer-Snapshot
(`buyer_name`, `buyer_address_*`, `buyer_tax_number`, `buyer_vat_id`, …),
`payment_account_id`, `payment_note`, `voided_at`/`void_reason` (Migration
0032: verworfener Entwurf — `status` bleibt `draft`, die Nummer bleibt
verbraucht, danach unveränderlich; kein DELETE).

**`invoice_items`** — Rechnungs-Positionen. Felder: `id`, `invoice_id`,
`position`, `description`, `description_title`, `quantity_thousandths`,
`unit_price_cents`, `total_cents`, `tax_category_code` (`E` für §19,
`S`/`Z`/`AE` sonst), `tax_rate_per_mille`, `unit`, `source_package_id`,
`source_package_revision` (Soft-Zeiger).

**`quotes`** + **`quote_items`** — analog zu Rechnungen, eigener Belegkreis
`AN-…`, eigener Lifecycle (`draft → sent → accepted → converted`/`rejected`/`canceled`).
Ein zurückgezogener Entwurf (`canceled`, `locked_at` NULL) gilt als verworfen;
`canceled` ist endgültig (Migration 0032). Die Angebotsliste liefert dafür
`locked_at` mit (§2g). Der Status `sent` bedeutet festgeschrieben, nicht
versendet; die Oberfläche nennt ihn „Fertiggestellt“ und liest den Versand aus
`email_log` (ADR 0041 Nr. 170).

**`expenses`** — Kosten (Eingangsseite). Felder: `id`, `expense_number`,
`date`, `paid_date`, `category` (z. B. `office`, `travel`, `equipment`,
`other`), `gross_amount_cents`, `tax_amount_cents`, `net_amount_cents`,
`tax_category_code` (eingangsseitige USt-Klassifikation), `description`,
`supplier_contact_id`, `pdf_archive_id`, `xml_archive_id` (bei Empfang von
E-Rechnungen), `payment_account_id`, `locked_at`, `is_einvoice_received`.

**`private_movements`** — Privatentnahmen/-einlagen. Felder: `id`, `date`,
`kind` (`deposit`/`withdrawal`), `amount_cents`, `description`,
`payment_account_id`, `locked_at`. EÜR-neutral, kein Storno.

**`assets`** — Anlagenverzeichnis. Felder: `id`, `label`, `acquired_on`,
`acquisition_cost_cents`, `useful_life_years`, `method` (`linear`/`gwg`/
`computer_one_third`/`mixed_private`), `private_share_percent`, `disposed_on`
(NULL aktiv), `inventory_number`, `locked_after_year`.

**`depreciation_entries`** — AfA-Buchungen. Felder: `id`, `asset_id`, `year`,
`amount_cents`, `booked_at`, `is_locked` (gesetzt nach GJ-Abschluss).
UNIQUE `(asset_id, year)`.

### 2c. Belege-Erweiterungen (Phase 3 + 4)

**`packages`** — Paket-Stammdaten. Felder: `id`, `category_id`, `slug`,
`current_revision_id`, `archived_at`.

**`package_revisions`** — Append-only Revisionen. Felder: `id`, `package_id`,
`revision_number`, `title`, `description_title`, `description_markup`
(Markdown-Subset), `unit_price_cents`, `tax_category_code`, `created_at`.
**Trigger:** `trg_package_revisions_immutable` (kein UPDATE),
`trg_package_revisions_no_delete` (kein DELETE).

**`package_categories`** — Kategorien für die Katalog-Broschüre. Felder: `id`,
`label`, `position`.

**`recurring_subscriptions`** — Vorlagen für eingangsseitige Recurring-Kosten.
Felder: `id`, `label`, `interval`, `next_due_on`, `payment_account_id`, …

**`recurring_invoices`** — Vorlagen für ausgangsseitige Abo-Rechnungen.
Felder: `id`, `label`, `contact_id`, `interval` (`monthly`/`quarterly`/`yearly`),
`auto_mode` (`draft`/`issue`/`issue_send`), `next_due_on`, `is_active`,
Buyer-Snapshot-Felder, Sender-Snapshot (Default-Konto).

**`recurring_invoice_items`** — Items der Vorlage. UNIQUE
`(recurring_invoice_id, position)`.

### 2d. Bindungen + Provenienz

**`attachments`** — Generische Anhänge (write-once via Archive). Felder: `id`,
`parent_type` (`invoice`/`quote`/`expense`/`asset`/`contact`/`package`/…),
`parent_id`, `kind` (`contract`/`receipt`/`order`/`other`), `archive_id`,
`label`, `uploaded_at`.

**`legal_documents`** — Versionierte AGB/Datenschutz. Felder: `id`, `kind`
(`terms`/`privacy`), `version`, `effective_from`, `is_active`, `pdf_archive_id`.
**Trigger:** `trg_legal_documents_immutable`, `trg_legal_documents_no_delete`.

**`quote_legal_documents`** — Append-only Bindung zwischen Angebot und
Legal-Versionen. Felder: `quote_id`, `legal_document_id`, `bound_at`.
**Trigger:** `trg_quote_legal_documents_immutable`/`_no_delete`. Einer der
GoBD-relevanten Nachweise „welche Version ging an welches Angebot".

### 2e. Protokolle (append-only)

**`audit_log`** — Generische Audit-Ereignisse. Felder: `id`, `timestamp_utc`,
`actor` (Lauf F: `owner` für Aufrufe über die Sitzung des Inhabers,
`scheduler` für Abläufe des Schedulers, `system` für Start, Migrationen und
Restore-Nacharbeit; ältere Einträge tragen durchweg `system`), `action` (z. B.
`invoice.lock`, `expense.create`, `archive.integrity_fail`, `dsgvo.export`,
`euer.pdf_export`), `entity_type`, `entity_id`, `details_json` (JSON, seit
Lauf F immer per `serde_json` serialisiert). **Trigger:**
`trg_audit_no_update`, `trg_audit_no_delete`. Diese Tabelle ist die mit
Abstand am häufigsten geschriebene; **niemals** Passphrase oder Token in
`details_json`.

**`fiscal_year_locks`** — EÜR-Snapshot pro abgeschlossenes Jahr. Felder:
`fiscal_year`, `closed_at`, `income_total_cents`, `expense_total_cents`,
`afa_total_cents`, `surplus_cents`, `assets_locked`,
`depreciation_entries_locked`, `app_version`, `schema_version`, `notes`.
Auswertungen und Exporte abgeschlossener Jahre gleichen die live gerechneten
Eckwerte damit ab (Lauf F, KB-0031, `gobd.md` §6.1); die EÜR-Ansicht zeigt den
Abgleich als Lesefeld `snapshotCheck` (§2g). **Trigger:**
`trg_fiscal_year_locks_no_update`/`_no_delete`.

**`email_log`** — Versand-Protokoll. Felder: `id`, `created_at`, `account_id`,
`account_label`, `channel`, `related_kind` (`invoice`/`quote`/`test`/
`package_catalog`), `related_id` (Rechnung bzw. Angebot; beim Katalogversand
seit Lauf F der Kontakt, KB-0049), `related_number`, `from_email`, `to_email`,
`subject`, `attachment_count`, `status` (`success`/`failed`), `provider_code`,
`provider_message`, `request_id`, `error`. **Trigger:**
`trg_email_log_no_update`/`_no_delete`.

**`backup_log`** (G1-LOG, Migration 0026) — Backup-Protokoll, eine Zeile je
Lauf und Ablage (Erfolg und Fehlschlag). Felder: `id`, `created_at`,
`trigger` (`manual`/`auto_daily`/`auto_critical`/`pre_restore`),
`target_kind` (`local` = Floor, `directory`, `sftp`), `target_label` (z. B.
SFTP-Host), `file_name`, `full_path` (voller Pfad bzw. `sftp://`-URI),
`size_bytes`, `status` (`ok`/`failed`), `detail` (Fehlertext bei `failed`).
**Trigger:** `trg_backup_log_no_update`/`_no_delete`. **Niemals** die
Passphrase. Aus `backup_log` stammt die letzte gute Kopie eines Ziels, dessen
letzter Lauf scheiterte (Lesefeld `offsiteLastCopyAt`, §2g).

### 2f. Infrastruktur

**`app_settings`** — Key-Value-Store. Felder: `key` (PK), `value`, `updated_at`.
Wichtige Keys (u. a.): `schema_version`, `depreciation_auto_year_close`
(Auto-AfA zur GJ-Wende, Standard an), `backup_passphrase_set` und
`backup_verifier_*` (verschlüsselter Passphrase-Verifier, kein Klartext, kein
Hash der Passphrase), `backup_targets` (JSON-Liste der Off-Site-Ziele, W7),
`backup_offsite_results` (Ergebnis je Ziel für den Status `offsite`),
`update_check_enabled`/`update_check_last_at` (opt-in Versionsprüfung, W12),
`invoice_due_days`, `quote_valid_days`, `travel_cost_per_km_cents`.
Dauerbetrieb (Lauf F, ADR 0040, ohne Migration): `scheduler_hold`/
`scheduler_hold_since` (Pause der automatischen Belegerzeugung, Grund
`clock_backwards`/`clock_jump`/`restore`), `scheduler_last_seen_at`
(Uhr-Bremse), `recurring_invoice_pending:<vorlage>` (Fortsetzungs-Marker einer
Abo-Rechnung), `backup_daily_time`/`backup_daily_last_on`/
`backup_daily_last_failure_at` (kalenderfestes Tages-Backup),
`integrity_last_failure_at` und `depreciation_auto_last_failure_at` (Backoff),
`payment_accounts_defaults_seeded` (Standardkonten einmal je Instanz).
`drop_folder_enabled`/`drop_folder_path` (Migration 0029) bleiben aus der
Desktop-Fassung stehen, werden im Server aber nicht mehr gelesen (E11).
Zugangsdaten stehen **nicht** hier, sondern in `secrets`.

**`secrets`** (Migration 0031, W8) — Zugangsdaten. Felder: `key` (PK:
`smtp:<mail_account.id>`, `oauth:<mail_account.id>`, `sftp:<Ziel-ID>`),
`value`, `updated_at`. Liegt wie alle Tabellen in der SQLCipher-verschlüsselten
DB (Passphrase = DB-Key, ADR 0035), geht damit nur verschlüsselt in Backups
und ist vom Migrations-Export (`json_dump::EXCLUDED_TABLES`) und vom
DSGVO-Export ausgenommen. Lesen und Schreiben nur über `crate::secrets`.

**`archive_entries`** — Index aller Archiv-Dateien. Felder: `id` (= ArchiveId),
`kind` (`InvoicePdf`/`InvoiceXml`/`QuotePdf`/`LegalDocument`/`ReceivedEinvoice`/
`Attachment`/…), `path` (relativ zu `archive_dir`), `hash` (SHA-256 hex),
`size_bytes`, `created_at`. **Trigger:** `trg_archive_no_update` (hash/path/size
unveränderlich).

**`archive_integrity_checks`** — Re-Hash-Läufe. Felder: `id`, `started_at`,
`finished_at`, `files_checked`, `files_passed`, `files_failed` (Manipulation +
fehlende Dateien), `failed_archive_ids_json`; seit Migration 0032 zusätzlich
`files_missing` (Teilmenge von `files_failed`), `files_unreadable`
(vorhanden, aber nicht lesbar — kein Manipulationsbefund),
`missing_archive_ids_json`, `unreadable_archive_ids_json` (JSON-Listen per
`serde_json`). Bei Läufen vor 0032 sind die neuen Spalten NULL (nicht erfasst).

**`backup_history`** (Migration 0001) — Bestand der vorhandenen
Backup-Dateien, eine Zeile je erfolgreich geschriebener Datei (Floor und jedes
Off-Site-Ziel, unterschieden am `target_path`). Felder: `id`, `created_at`,
`target_path`, `file_hash_sha256`, `file_size_bytes`, `is_encrypted`,
`retention_tag` (`daily`/`monthly`/`yearly`/`manual`), `trigger_reason`
(`auto_daily`/`auto_critical`/`manual`/`pre_restore`), `db_schema_version`,
`app_version`, `verified_at`. `backup::create_now` schreibt sie (nach einem
Restore trägt `record_existing_backup` das Pre-Restore-Backup nach), die
Rotation löscht Zeilen zusammen mit den Dateien; sie speist die Backup-Liste, den
Restore aus der Historie, die Fälligkeit des Tages-Backups und
`backup_overdue` ohne Off-Site-Ziel. **Nicht** append-only — der lückenlose
Nachweis aller Läufe ist `backup_log`.

**`doc_number_counters`** — Atomare Nummern-Allokation. Felder:
`{typ, year}` (PK), `next_n`. Ein einziger UPDATE-RETURNING-Cycle pro Allokation,
keine Race-Conditions.

**`notifications`** — In-App-Inbox. Felder: `id`, `at`, `dedup_key`,
`category`, `title`, `body`, `severity`, `dismissed_at`, `target_route` (für
Deeplink).

**`notification_rules`** — Schaltbare Regel-Engine. Felder: `id`
(z. B. `rule_backup_overdue`, `rule_backup_result`, `rule_recurring_due` —
Kosten-Abo im Modus „Erinnerung“ fällig, Migration 0032), `rule_type`, `label`,
`enabled`, `config_json`, `deliver_in_app`, `deliver_os_native` (ohne Wirkung).

**`payment_accounts`** — höchstens ein Konto mit `is_default = 1`
(`uq_payment_accounts_default`, Migration 0032); ein inaktives Konto ist nie
Standard (`ensure_one_default`).

### 2g. Abgeleitete Lesefelder (Lauf 2026.11, ohne Migration)

Für die Oberfläche liefern einige Lese-Commands Felder, die keine eigene Spalte
haben, sondern aus bestehenden Daten abgeleitet werden (ADR 0041 Nr. 80–86).
Schema und Schreibwege bleiben unverändert (Schema v32).

| Feld (Frontend-Typ) | Command | Quelle | ADR 0041 |
|---|---|---|---|
| `QuoteListItem.lockedAt` | `quotes_list` | `quotes.locked_at`: `canceled` ohne Wert = verworfener Entwurf, mit Wert = zurückgezogenes Angebot | Nr. 83 |
| `PdfTemplateMeta.supportsStorno`, `stornoFallback` | `pdf_templates_list` | Quelltext der Vorlage (nutzt sie `storno_of`?) bzw. die eingebaute Ersatzvorlage, mit der Stornos entstehen | Nr. 84 |
| `OffsiteResult.suspended` | `backup_create_now` | Pause nach einem Restore bis zur Freigabe: Spiegelung ausgesetzt, kein Fehlschlag (`error` nennt dann den Grund) | Nr. 85 |
| `BackupSettings.offsiteLastCopyAt`, `offsiteAgeDays` | `backup_get_settings` | älteste der je Ziel jüngsten Kopien: letztes Ergebnis je Ziel (`app_settings.backup_offsite_results`), war es ein Fehlschlag, der jüngste erfolgreiche Eintrag in `backup_log`; Alter in Berliner Kalendertagen wie der Status `stale` | Nr. 86 |
| `EuerReport.isKleinunternehmer`, `yearStatus`, `statusChange`, `snapshotCheck` | `euer_compute_report` | `tax_status_changes` (§19 das ganze Jahr, dreiwertiger Jahresstatus, Wechsel im Jahr als Text; ADR 0042) bzw. `fiscal_year_locks` (Abgleich, `null` bei offenem Jahr); ohne Audit und Hinweis | Nr. 80 |
| `ParsedEInvoice.taxCategories`, `intraCommunity` | `expenses_parse_einvoice` | Steuerkategorien der eingelesenen Rechnung (UNTDID 5305), Kennzeichen für Kategorie K; nicht gespeichert, nur Hinweis | Nr. 82 |
| `AssetDisposalPreview` (neuer Command) | `assets_disposal_preview` | `assets` und `depreciation_entries` über `plan_disposal`, ohne zu buchen; `blockedReason` mit den Texten von `assets_dispose` | Nr. 81 |
| `SellerProfile.isKleinunternehmer`, `waivedParagraph19Since` | `seller_profile_get`/`_upsert` | Status heute aus `tax_status_changes` (ADR 0042 Nr. 1), nicht aus den Spiegelspalten; die Oberfläche nutzt ihn nur für Hinweise „anders als heute“, Belege rechnen mit dem Status ihres Datums (`tax_status_at`) | ADR 0042 |
| `TaxStatusOverview` (neuer Command) | `tax_status_overview` | `tax_status_changes` (Verlauf mit widerrufenen Einträgen, Status heute, Bindung, frühestmögliche Rückkehr, widerrufbarer Eintrag) und Zahlungseingänge (Gesamtumsatz Vorjahr/laufendes Jahr, Grenzhinweise); Abschnitt „Steuerstatus“ in „Meine Firmendaten“ | ADR 0042 |

Berater- und Mandantennummer für den DATEV-Export stehen nicht in der Datenbank:
Die Oberfläche merkt sie im Browser (`localStorage`) und übergibt sie je Export
(ADR 0041 Nr. 113).

---

## 3. Trigger-Übersicht

Die DB-Trigger sind die letzte Verteidigungslinie der Hardlines. Sie greifen
auch dann, wenn ein Bug im Repo-Layer eine eigentlich verbotene Mutation
versucht. Detail-Diskussion in `gobd.md`. Hier die Liste mit Tabelle und Wirkung:

| Trigger | Tabelle | Wirkung |
|---|---|---|
| `trg_audit_no_update` | `audit_log` | UPDATE verboten |
| `trg_audit_no_delete` | `audit_log` | DELETE verboten |
| `trg_archive_no_update` | `archive_entries` | UPDATE auf `hash`/`path`/`size` verboten |
| `trg_archive_no_delete` | `archive_entries` | DELETE verboten (0032, KB-0004) |
| `trg_invoices_immutable` | `invoices` | nach `locked_at`: UPDATE auf Kernfelder verboten (`invoice_number`, `date`, Beträge, `contact_id`, `fiscal_year`, `is_kleinunternehmer`, `direction`). Erlaubt: State-Transitions (`status`, `paid_*`, `sent_*`, `canceled_*`, `notes`, `archive_ids`). |
| `trg_invoices_no_delete` | `invoices` | DELETE verboten (0028) |
| `trg_invoices_voided_final` | `invoices` | verworfener Entwurf: jede Änderung verboten, auch das Festschreiben; einzige Ausnahme: Anonymisierung des Kontakts setzt `buyer_*` (außer Land) und `notes` auf Platzhalter bzw. `NULL` (0032, F2c) |
| `trg_invoices_void_draft_only` | `invoices` | `voided_at` nur auf offenen Entwürfen, nur mit Grund, nie zugleich mit `locked_at` (0032) |
| `trg_invoices_void_not_on_insert` | `invoices` | INSERT mit `voided_at`/`void_reason` verboten (0032) |
| `trg_invoices_canceled_final` | `invoices` | `status = 'canceled'` endgültig; `canceled_at`, `canceled_by_storno_id`, `cancel_reason` unveränderlich (0032) |
| `trg_invoice_items_frozen_insert`/`_update`/`_delete` | `invoice_items` | Positionen festgeschriebener oder verworfener Rechnungen unveränderlich (0032, KB-0002) |
| `trg_quotes_immutable` | `quotes` | nach `locked_at`: analog zu Invoices |
| `trg_quotes_no_delete` | `quotes` | DELETE verboten (0028) |
| `trg_quotes_canceled_final` | `quotes` | zurückgezogen bleibt zurückgezogen (`status`, `locked_at`, `canceled_*`) (0032) |
| `trg_quotes_converted_final` | `quotes` | umgewandelt bleibt umgewandelt (Status, `converted_*`); Rücknahme auf `accepted` nur, wenn der Rechnungs-Entwurf verworfen ist (0032, F2c) |
| `trg_quote_items_frozen_insert`/`_update`/`_delete` | `quote_items` | Positionen festgeschriebener oder zurückgezogener Angebote unveränderlich (0032, KB-0002) |
| `trg_expenses_immutable` | `expenses` | nach `locked_at`: UPDATE auf Kernfelder verboten (Kosten sind sofort gelockt) |
| `trg_private_movements_immutable` | `private_movements` | UPDATE auf Kernfelder verboten (sofort immutable) |
| `trg_assets_immutable` | `assets` | nach `locked_after_year`-Setzung (GJ-Lock): UPDATE auf Kernfelder verboten |
| `trg_assets_disposal_final` | `assets` | nach der Veräußerung (`disposed = 1`): `disposed`, `disposal_*`, Bezeichnung, Anschaffung, AfA-Grundlage und Restbuchwert unveränderlich (0032, F2c) |
| `trg_depreciation_entries_no_delete_disposed` | `depreciation_entries` | AfA-Buchungen einer veräußerten Anlage nicht löschbar (0032, F2c) |
| `trg_depreciation_immutable` | `depreciation_entries` | nach `is_locked=1`: UPDATE verboten |
| `trg_fiscal_year_locks_no_update`/`_no_delete` | `fiscal_year_locks` | append-only |
| `trg_legal_documents_immutable` | `legal_documents` | UPDATE auf Kernfelder verboten (Versionierung statt Update) |
| `trg_legal_documents_no_delete` | `legal_documents` | DELETE verboten |
| `trg_quote_legal_documents_immutable`/`_no_delete` | `quote_legal_documents` | append-only Bindungs-Tabelle |
| `trg_package_revisions_immutable`/`_no_delete` | `package_revisions` | append-only Revisionen |
| `trg_email_log_no_update`/`_no_delete` | `email_log` | append-only |
| `trg_backup_log_no_update`/`_no_delete` | `backup_log` | append-only |
| `trg_tax_status_changes_no_delete` | `tax_status_changes` | DELETE verboten (0033) |
| `trg_tax_status_changes_revoke_only` | `tax_status_changes` | UPDATE nur als einmaliger Widerruf (`revoked_at` + `revoke_reason`), alle übrigen Spalten unveränderlich (0033) |

**Indizes mit Hardline-Wirkung (0032):** `ux_invoices_one_locked_storno`
(`invoices(is_storno_for) WHERE is_storno_for IS NOT NULL AND locked_at IS NOT
NULL`), `uq_payment_accounts_default` (`payment_accounts(is_default) WHERE
is_default = 1`). Seit 0033: `uq_tax_status_changes_active`
(`tax_status_changes(effective_from) WHERE revoked_at IS NULL`). Tests:
`tests/migration_0028_test.rs`, `tests/migration_0032_test.rs`,
`tests/migration_0033_test.rs`.

**Reason-Mapping.** `backup::db_trigger_reason` (privat in `backup/mod.rs`)
bildet die Auslöser-Labels der Lock-Events auf die vom CHECK erlaubten Werte
von `backup_history.trigger_reason` ab (`auto_daily`/`auto_critical`/`manual`/
`pre_restore`). Catch-all `_ => auto_critical` ist CHECK-bruchsicher
(G1-HARDEN.2).

---

## 4. Schema-Version-Management

Jede Migration setzt die `app_settings.schema_version`-Zeile auf ihre Nummer.
Geprüft wird beim **Entsperren** (`server::auth`, `commands::backup::
migrate_with_backup`), nicht beim Prozessstart — vorher ist die DB zu:

- **DB-Version < EXPECTED:** zuerst die Vorab-Prüfung
  `db::precheck_before_migration` (Fremdschlüssel per `PRAGMA
  foreign_key_check`, für Schema < 32 auch doppelte festgeschriebene Stornos;
  Lauf F2c): ein Altfehler lehnt mit 409 `db_integrity_failed` ab, solange die
  DB unverändert auf dem alten Schema steht — das vorige Image öffnet sie
  weiter. Dann ein Pflicht-Backup (`manual`) mit der
  alten Version, vollständig gelesen und geprüft, Audit `backup.pre_migration`
  (ADR 0039 E15, W7). Scheitert es, wird nicht migriert und die Instanz bleibt
  gesperrt (409 `pre_migration_backup_failed`). Danach läuft
  `db::run_migrations` (`sqlx::migrate!`) die fehlenden Migrationen.
- **DB-Version > EXPECTED** (`EXPECTED_SCHEMA_VERSION`, 32): `db::run_migrations`
  lehnt vor dem Migrations-Runner mit `Error::SchemaMismatch` und dem Hinweis
  auf das Image der passenden Version ab, kein Entsperren (ein älteres Programm
  liest keine neuere DB — unklar, welche neuen Constraints existieren; Lauf F).
- **Danach** vergleicht `db::schema_version::check_compatible` den Stand mit
  dem einkompilierten `EXPECTED_SCHEMA_VERSION`, und
  `db::foreign_key_violations` prüft die ganze DB erneut mit `PRAGMA
  foreign_key_check` (R1-012). Ein verletzter Fremdschlüssel oder eine
  gescheiterte Vorab-Prüfung einer Migration (benannter CHECK „Migration 00NN
  abgebrochen: …“, z. B. 0032, zweite Linie hinter der Rust-Vorabprüfung)
  blockiert das Entsperren mit 409 `db_integrity_failed`; Einzelheiten
  (Tabelle, rowid, Zieltabelle, keine Inhalte) stehen im Server-Log. Ohne
  Umstellung sind die Daten unangetastet; schlägt die Prüfung erst **nach**
  einer gespeicherten Umstellung an (ein Fehler der Migration selbst), sagt
  die Meldung das („nach der Umstellung“) und nennt den Rückweg über das
  Pflicht-Backup in einer neuen Instanz mit dem vorigen Image (Handbuch
  „Fehlersuche im Container“).
- **Restore** nimmt Backups mit `MIN_RESTORE_SCHEMA_VERSION` (30) ≤ Schema ≤
  EXPECTED an und migriert sie beim nächsten Entsperren vorwärts.

Damit kann man **keine Down-Migration aus Versehen** machen. Wer eine ältere
Version mit einer neueren DB startet, wird zum Update gezwungen, nicht zum
Schaden.

---

## 5. Fremdschlüssel und Snapshots

GoBD verlangt, dass ein einmal festgeschriebener Beleg keine Daten verliert,
wenn ein anderer Datensatz später geändert oder gelöscht wird. Klein.Buch
löst das mit **Snapshot-Spalten direkt auf dem Beleg**, nicht über FKs:

- `invoices` hat `buyer_name`, `buyer_address_*`, `buyer_tax_number`,
  `buyer_vat_id` etc. als eingefrorene Kopie zum Zeitpunkt des Lock.
  Wird der `contacts`-Eintrag später anonymisiert, bleibt der Beleg lesbar.
- `quotes` hat seit Migration 0023 dieselben Buyer-Snapshot-Spalten
  (Migration nachgereicht im DSGVO-Kontext).
- `invoice_items`/`quote_items` halten `source_package_id` und
  `source_package_revision` als **Soft-Zeiger** (kann auf NULL gesetzt
  werden, wenn die Position „angepasst" wurde — siehe ADR 0031).

FKs gibt es für Stammdaten-Beziehungen (`contact_id`, `payment_account_id`,
`mail_account_id`), aber ein DELETE auf einem referenzierten Stamm scheitert
am FK; gelöscht wird ohnehin selten — DSGVO-Anonymisierung ist ein **UPDATE
auf NULL**, keine Löschung des Datensatzes.

**`ON DELETE CASCADE`** steht im Schema an genau drei Stellen (R1-S2-ONDELETE,
dokumentiert in Lauf F7):

| Fremdschlüssel | Migration | Wirkung |
|---|---|---|
| `invoice_items.invoice_id → invoices(id)` | 0001 | tot: `trg_invoices_no_delete` (0028) verbietet jedes DELETE auf `invoices`, die Kaskade kann nie auslösen. Positionen eines Entwurfs ersetzt der Repo-Layer per DELETE auf `invoice_items` selbst; bei festgeschriebenen oder verworfenen Rechnungen sperren das `trg_invoice_items_frozen_*` (0032). |
| `quote_items.quote_id → quotes(id)` | 0005 | tot: `trg_quotes_no_delete` (0028); Positionen festgeschriebener Angebote schützt `trg_quote_items_frozen_*` (0032). |
| `recurring_invoice_items.recurring_invoice_id → recurring_invoices(id)` | 0024 | gewollt: Positionen einer Abo-**Vorlage** sind kein Beleg und gehören nur zu ihrer Vorlage. Derzeit löscht kein Code eine Vorlage (Pausieren bzw. Laufzeit-Ende über `active` und `end_date`); käme ein Löschen dazu, gingen die Vorlagen-Positionen mit. Die daraus erzeugten Rechnungen sind eigenständige Kopien und hängen nicht an der Vorlage. |

Neue Kaskaden auf Beleg-Tabellen sind nicht zulässig (GoBD: Storno statt
Löschung); neue Fremdschlüssel bekommen das Standardverhalten (DELETE scheitert).

---

## 6. Sensible Felder — was steht nirgends drin

Aus dem Schema lesbar, hier als explizite Negativ-Liste:

- Es gibt **keine** Spalte für die App-/Daten-Passphrase. Sie ist nur
  Eingabe der SQLCipher-Schlüsselableitung (PBKDF2, Salt im DB-Header) und
  liegt, solange entsperrt, in `BackupSession` im RAM; `app_settings` hält nur
  einen verschlüsselten Verifier (`backup_verifier_*`).
- SMTP-Passwörter, OAuth-Refresh-Tokens und SFTP-Passwörter stehen **nur** in
  der Tabelle `secrets` (Migration 0031, W8) — also in der verschlüsselten DB,
  nicht in `mail_accounts`, nicht im Ziel-JSON `app_settings.backup_targets`,
  nie in Log oder `audit_log`, nie im Migrations- oder DSGVO-Export. Bis W8
  lagen sie im OS-Keychain der Desktop-Fassung.
- Das TOTP-Geheimnis und die gehashten Einmal-Codes liegen **außerhalb** der
  DB in `/data/auth/totp.json` (E5) und gehören nicht zu Snapshot oder
  Restore.

---

## Letzte Verifikation

Stand: 2026-10-24 (Lauf F, Block F2a), Schema v32; 2026-10-25 (F7b) §5 um `ON DELETE CASCADE` ergänzt (gegen Migrationen und Repo-Code geprüft); 2026-10-27 (Lauf 2026.11, U6) §2g „Abgeleitete Lesefelder“ ergänzt (gegen `src/lib/types.ts`, `src/lib/api.ts` und die Kerne in `commands/{quotes,settings,backup,euer,expenses,assets}.rs` geprüft). 2026-10-27 (Lauf 2026.11.1, V1): Schema v33, `tax_status_changes`, Spiegelspalten im Profil (gegen `migrations/0033_tax_status_history.sql` und `src/db/repo/tax_status.rs` geprüft). Quelle:
`klein-buch/src-tauri/migrations/0001..0033.sql`,
`src/db/schema_version.rs`, `src/secrets.rs`, `src/backup/{mod,target}.rs`,
`src/commands/{mail,backup}.rs`, `src/db/mod.rs`. Gegen den Code geprüft:
Migrations-Liste, `mail_accounts`, `app_settings`, `secrets`, `backup_log`,
`backup_history`, `archive_integrity_checks`, `notification_rules`, die
Trigger-Übersicht (§3), §4 und §6. Die übrigen Feldlisten in §2 stammen vom Stand 2026-05-27 und sind
nicht neu abgeglichen; maßgeblich sind die Migrationen. Bei
Schema-Erweiterungen diese Datei in der entsprechenden Sektion ergänzen und
die Migrations-Tabelle verlängern.
