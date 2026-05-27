# Datenmodell-Referenz

> Vertiefung zu `../ARCHITECTURE.md`. Diese Datei beschreibt das SQLite-Schema
> auf Tabellen-Ebene: Verantwortung, Schlüssel-Felder, Constraints, Trigger,
> Migrations-Historie. Die *vollständigen* `CREATE TABLE`-Statements liegen
> in `klein-buch/src-tauri/migrations/*.sql` und sind die kanonische Quelle —
> diese Doku ist die navigationsfreundliche Sicht darauf.

**Schema-Stand: v30.** Migrationen 0001–0030, forward-only. Schema-Version
liegt in `app_settings.schema_version`; Mismatch beim Start ⇒ App stoppt
(Down-Migration-Schutz).

**Globale Konventionen** (ADR 0003):

- **`STRICT`-Tabellen** überall — SQLite prüft die Spalten-Typen wirklich.
- **`PRAGMA foreign_keys = ON`** für jeden Pool.
- **UUIDv7-PKs** als `TEXT` (kanonische 36-Zeichen-Form, zeitlich sortierbar).
- **Geld als Integer-Cents** (`INTEGER NOT NULL`). Niemals `REAL` für Beträge.
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
| 0014 | `0014_oauth.sql` | `mail_accounts` um OAuth-Felder erweitert (Refresh-Token-Keychain-Referenz, nicht Token selbst) |
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
| 0029 | `0029_drop_folder.sql` | Drop-Folder-Settings (`drop_folder_enabled`, `drop_folder_path`) + Notification-Regeln `rule_drop_folder_import_ok` (default off, Inbox-only) und `rule_drop_folder_import_failed` (default on, Inbox + OS-Toast) — ADR 0037 |
| 0030 | `0030_rename_drop_folder_labels.sql` | UI-Label-Update auf den Notification-Regeln aus 0029 (Block PV1-RENAME): „Drop-Folder: …" → „Rechnungs-Eingang: …" (CLAUDE.md-UI-Sprachregel, Plain-Language-Hardline). Code-Identifier (rule_id, Settings-Keys, Route, Module) bleiben englisch. |

**Down-Migrations gibt es nicht.** Wer eine Migration „rückgängig" braucht,
muss eine neue, höher nummerierte Migration schreiben, die den Zustand
korrigiert.

---

## 2. Tabellen-Übersicht

### 2a. Stammdaten

**`seller_profile`** — Verkäufer (es gibt genau einen Eintrag pro Installation).
Felder: `id`, `legal_name`, `address_*`, `tax_number` (optional, §19),
`vat_id` (optional), `is_kleinunternehmer` (Default 1), `waived_paragraph_19_since`
(Datum, NULL solange §19 aktiv), `logo_archive_id`, `signature_archive_id`,
`default_pdf_template`, `travel_*` (km-Satz), Banking-Felder.

**`contacts`** — Kunden + Lieferanten. Felder: `id`, `display_name`,
`legal_name`, `kind` (`customer`/`supplier`/`both`), `address_*`, `email`,
`tax_number`, `vat_id`, `notes` (interne, **nicht** im DSGVO-Export),
`anonymized_at` (DSGVO Art. 17), `archived_at`. Anonymisierung: `display_name`
wird zu `'Anonymisiert #<8hex>'`, alle PII auf NULL — die in Belege
eingefrorenen Buyer-Snapshots bleiben (GoBD gewinnt).

**`payment_accounts`** — Zahlungs-Konten (Bank/Bar/PayPal). Felder: `id`,
`label`, `kind`, `iban`, `bic`, `bank_name`, `is_active`, `is_default`.

**`mail_accounts`** — SMTP- + OAuth-Mail-Konten. Felder: `id`, `kind`
(`smtp`/`graph`), `display_name`, `from_address`, `smtp_host`, `smtp_port`,
`smtp_username` (Passphrase im Keychain), OAuth-Felder (Tenant, Client-ID,
Scopes; Refresh-Token im Keychain, gechunkt wegen Windows-2560-Limit),
`last_used_at`.

### 2b. Beleg-Stamm (festschreibbar)

**`invoices`** — Ausgangsrechnungen. Felder: `id`, `invoice_number`
(`RE-{YYYY}-{NNNN}` oder `ST-…` für Storno), `date`, `delivery_date`,
`contact_id`, `is_kleinunternehmer`, `direction` (`outgoing`/`incoming`),
`fiscal_year`, `status` (`draft`/`issued`/`sent`/`canceled`), `is_storno_for`
(FK, NULL bei Original), `derived_from_quote_id`, `subtotal_cents`,
`tax_amount_cents`, `total_cents`, `paid_amount_cents`, `paid_at`, `sent_at`,
`canceled_at`, `locked_at`, `pdf_archive_id`, `xml_archive_id`, Buyer-Snapshot
(`buyer_name`, `buyer_address_*`, `buyer_tax_number`, `buyer_vat_id`, …),
`payment_account_id`, `payment_note`.

**`invoice_items`** — Rechnungs-Positionen. Felder: `id`, `invoice_id`,
`position`, `description`, `description_title`, `quantity_thousandths`,
`unit_price_cents`, `total_cents`, `tax_category_code` (`E` für §19,
`S`/`Z`/`AE` sonst), `tax_rate_per_mille`, `unit`, `source_package_id`,
`source_package_revision` (Soft-Zeiger).

**`quotes`** + **`quote_items`** — analog zu Rechnungen, eigener Belegkreis
`AN-…`, eigener Lifecycle (`draft → sent → accepted → converted`/`rejected`/`canceled`).

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

**`audit_log`** — Generische Audit-Ereignisse. Felder: `id`, `at`, `actor`
(meist `system` oder Command-Name), `event` (z. B. `invoice.issued`,
`expense.created`, `archive.integrity_tamper`, `dsgvo.export`), `target_type`,
`target_id`, `details` (JSON). **Trigger:** `trg_audit_no_update`,
`trg_audit_no_delete`. Diese Tabelle ist die mit Abstand am häufigsten
geschriebene; **niemals** Passphrase oder Token im `details`.

**`fiscal_year_locks`** — EÜR-Snapshot pro abgeschlossenes Jahr. Felder:
`fiscal_year`, `closed_at`, `snapshot_json` (komplette aggregate EÜR zum
Zeitpunkt des Abschlusses). **Trigger:**
`trg_fiscal_year_locks_no_update`/`_no_delete`.

**`email_log`** — Versand-Protokoll. Felder: `id`, `sent_at`, `mail_account_id`,
`recipient`, `subject`, `attachment_archive_ids` (JSON-Array), `status`
(`ok`/`failed`), `provider_response` (kanonisiert). **Trigger:**
`trg_email_log_no_update`/`_no_delete`.

**`backup_log`** (G1-LOG, Migration 0026) — Backup-Protokoll. Felder: `id`,
`at`, `name` (Backup-Datei-Basisname), `size_bytes`, `path` (voller Pfad
am Ziel), `target_kind` (`floor`/`directory`/`sftp`), `retention_tag`
(`daily`/`weekly`/`monthly`), `status` (`ok`/`failed`), `trigger_reason`
(`manual`/`auto_critical`/`auto_daily`/`onboarding`/…), `error_message`
(bei `failed`). **Trigger:** `trg_backup_log_no_update`/`_no_delete`.
**Niemals** die Passphrase.

### 2f. Infrastruktur

**`app_settings`** — Key-Value-Store. Felder: `key` (PK), `value`, `updated_at`.
Wichtige Keys: `schema_version`, `fiscal_year_auto_close` (1/0), `last_backup_at`,
`onboarding_completed_at`, `dsgvo_disclaimer_acknowledged_at`,
`pdf_template_default` (wenn nicht aus `seller_profile` geliefert),
`drop_folder_enabled` (1/0, ADR 0037), `drop_folder_path` (absoluter Pfad
zum überwachten Ordner; leer = inaktiv).

**`archive_entries`** — Index aller Archiv-Dateien. Felder: `id` (= ArchiveId),
`kind` (`InvoicePdf`/`InvoiceXml`/`QuotePdf`/`LegalDocument`/`ReceivedEinvoice`/
`Attachment`/…), `path` (relativ zu `archive_dir`), `hash` (SHA-256 hex),
`size_bytes`, `created_at`. **Trigger:** `trg_archive_no_update` (hash/path/size
unveränderlich).

**`archive_integrity_checks`** — Re-Hash-Läufe. Felder: `id`, `started_at`,
`finished_at`, `files_total`, `files_ok`, `files_failed`, `files_missing`
(G1-HARDEN.4: getrennt von `files_failed`), `missing_archive_ids` (JSON),
`tamper_archive_ids` (JSON).

**`backup_history`** (Altbestand) — vor G1-LOG genutzte Aggregat-Tabelle.
Wird **nicht** mehr beschrieben; `backup_log` ist die Quelle der Wahrheit ab
Migration 0026. Lesen ist OK (Migrations-Kompatibilität); Schreiben läuft
ausschließlich über `backup_log`.

**`doc_number_counters`** — Atomare Nummern-Allokation. Felder:
`{typ, year}` (PK), `next_n`. Ein einziger UPDATE-RETURNING-Cycle pro Allokation,
keine Race-Conditions.

**`notifications`** — In-App-Inbox. Felder: `id`, `at`, `dedup_key`,
`category`, `title`, `body`, `severity`, `dismissed_at`, `target_route` (für
Deeplink).

**`notification_rules`** — Schaltbare Regel-Engine. Felder: `key`
(z. B. `backup_overdue`, `rule_backup_result`, `recurring_due_soon`,
`integrity_warning`), `is_enabled`, `threshold_days`.

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
| `trg_invoices_immutable` | `invoices` | nach `locked_at`: UPDATE auf Kernfelder verboten (`invoice_number`, `date`, Beträge, `contact_id`, `fiscal_year`, `is_kleinunternehmer`, `direction`). Erlaubt: State-Transitions (`status`, `paid_*`, `sent_*`, `canceled_*`, `notes`, `archive_ids`). |
| `trg_quotes_immutable` | `quotes` | nach `locked_at`: analog zu Invoices |
| `trg_expenses_immutable` | `expenses` | nach `locked_at`: UPDATE auf Kernfelder verboten (Kosten sind sofort gelockt) |
| `trg_private_movements_immutable` | `private_movements` | UPDATE auf Kernfelder verboten (sofort immutable) |
| `trg_assets_immutable` | `assets` | nach `locked_after_year`-Setzung (GJ-Lock): UPDATE auf Kernfelder verboten |
| `trg_depreciation_immutable` | `depreciation_entries` | nach `is_locked=1`: UPDATE verboten |
| `trg_fiscal_year_locks_no_update`/`_no_delete` | `fiscal_year_locks` | append-only |
| `trg_legal_documents_immutable` | `legal_documents` | UPDATE auf Kernfelder verboten (Versionierung statt Update) |
| `trg_legal_documents_no_delete` | `legal_documents` | DELETE verboten |
| `trg_quote_legal_documents_immutable`/`_no_delete` | `quote_legal_documents` | append-only Bindungs-Tabelle |
| `trg_package_revisions_immutable`/`_no_delete` | `package_revisions` | append-only Revisionen |
| `trg_email_log_no_update`/`_no_delete` | `email_log` | append-only |
| `trg_backup_log_no_update`/`_no_delete` | `backup_log` | append-only |

**Reason-Mapping.** `db::triggers::db_trigger_reason()` mappt eine
verletzte Constraint zurück auf einen sprechenden `trigger_reason` für das
Audit-Log. Catch-all `_ => auto_critical` ist CHECK-bruchsicher (G1-HARDEN.2).

---

## 4. Schema-Version-Management

Die `app_settings.schema_version`-Zeile wird in der höchsten relevanten
Migration aktualisiert (per Migration eine Zeile höhergezählt). Beim
App-Start liest `db::schema_version::read_or_init` den Stand und gleicht
gegen die in `db::schema_version::EXPECTED_VERSION` einkompilierte Zahl ab.
Mismatch:

- **DB-Version < EXPECTED:** sqlx::migrate! läuft die fehlenden Migrationen.
- **DB-Version > EXPECTED:** App stoppt mit klarer Fehlermeldung (alte App
  liest neue DB — verboten, weil unklar, welche neuen Constraints existieren).

Damit kann man **keine Down-Migration aus Versehen** machen. Wer eine ältere
App-Version mit einer neueren DB startet, wird zum Update gezwungen, nicht zum
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

---

## 6. Sensible Felder — was steht nirgends drin

Aus dem Schema lesbar, hier als explizite Negativ-Liste:

- Es gibt **keine** Spalte für die App-/Daten-Passphrase. Sie lebt nur als
  PBKDF2-Eingabe in `SQLCipher` (im DB-Header) und in `BackupSession` im
  Memory der laufenden Session.
- **Keine** Spalten für SMTP-Passwörter oder OAuth-Refresh-Tokens. Beide
  leben im OS-Keychain mit einer Referenz-ID im jeweiligen `mail_accounts`-
  Datensatz.
- **Keine** SFTP-Backup-Passwörter im Klartext. Auch im Keychain mit
  Referenz in der Backup-Settings-JSON in `app_settings`.

---

## Letzte Verifikation

Stand: 2026-05-27, Schema v30. Quelle: `klein-buch/src-tauri/migrations/
0001..0030.sql` und die `db::repo`-Module. Bei Schema-Erweiterungen diese
Datei in der entsprechenden Sektion ergänzen und die Migrations-Tabelle
verlängern.
