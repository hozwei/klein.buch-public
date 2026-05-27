//! Row-Typen für Phase-1-Tabellen.
//!
//! Konvention:
//! - `FromRow` für sqlx-Mappings (alle Spalten in DB-Reihenfolge).
//! - `Serialize` für Tauri-Bridge ans Frontend (camelCase).
//! - Booleans sind in SQLite INTEGER (0/1); wir mappen über `i64` und reichen
//!   das so ans Frontend. Optional kann das Frontend einen Boolean draus machen.
//! - Timestamps als `String` (ISO-8601), weil chrono+sqlx-Default für TEXT-Spalten
//!   nicht direkt `NaiveDateTime` liefert — Frontend bekommt es genau so.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ---- Contacts ----------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactRow {
    pub id: String,
    pub contact_type: String,
    pub name: String,
    pub legal_form: Option<String>,
    pub vat_id: Option<String>,
    pub tax_number: Option<String>,
    pub street: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country_code: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub accepts_einvoice: i64,
    pub archived: i64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// DSGVO Art. 17 (Block 19, Migration 0022): NULL = aktiver Kontakt;
    /// gesetzt = anonymisiert (Zeitpunkt). Bei gesetztem Wert sind die
    /// personenbezogenen Felder überschrieben (name = Platzhalter, Rest NULL).
    pub anonymized_at: Option<String>,
}

// ---- Seller Profile ----------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerProfileRow {
    pub id: i64,
    pub name: String,
    pub legal_form: Option<String>,
    pub street: String,
    pub postal_code: String,
    pub city: String,
    pub country_code: String,
    pub tax_number: Option<String>,
    pub vat_id: Option<String>,
    pub email: String,
    pub phone: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub logo_filename: Option<String>,
    pub is_kleinunternehmer: i64,
    pub waived_paragraph_19_since: Option<String>,
    pub default_pdf_template: String,
    pub default_currency: String,
    pub updated_at: String,
}

// ---- Mail Accounts ---------------------------------------------------------

/// Ein konfigurierter Versand-Account. Die SMTP-Passphrase bzw. die OAuth-Token
/// stehen **nicht** hier, sondern im OS-Keychain unter `keychain_service_id`.
/// Die OAuth-Spalten (Block 16, Migration 0014) halten nur nicht-geheime
/// Metadaten: die nutzer-eigene Azure-App (`oauth_tenant_id`/`oauth_client_id`),
/// das verbundene Postfach (`oauth_account_email`), die gewährten Scopes und den
/// Access-Token-Ablauf für die Refresh-Entscheidung.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountRow {
    pub id: String,
    pub label: String,
    pub auth_type: String,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_user: Option<String>,
    pub smtp_use_tls: i64,
    pub keychain_service_id: Option<String>,
    pub from_email: String,
    pub from_name: String,
    pub is_default: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    // ---- OAuth (Block 16) — nicht-geheime Metadaten ----
    pub oauth_tenant_id: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_account_email: Option<String>,
    pub oauth_scopes: Option<String>,
    pub oauth_token_expires_at: Option<String>,
}

// ---- E-Mail-Versandprotokoll (Block 16b) -----------------------------------

/// Ein Eintrag im append-only E-Mail-Versandprotokoll (`email_log`). Hält je
/// Versandversuch (Erfolg/Fehler) die Provider-Antwort (SMTP-Code/-Reply bzw.
/// Graph-Status + request-id) als Nachweis + Troubleshooting-Anker.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailLogRow {
    pub id: String,
    pub created_at: String,
    pub account_id: Option<String>,
    pub account_label: Option<String>,
    pub channel: String,
    pub related_kind: String,
    pub related_id: Option<String>,
    pub related_number: Option<String>,
    pub from_email: String,
    pub to_email: String,
    pub subject: String,
    pub attachment_count: i64,
    pub status: String,
    pub provider_code: Option<String>,
    pub provider_message: Option<String>,
    pub request_id: Option<String>,
    pub error: Option<String>,
}

// ---- Backup-Protokoll (G1-LOG, ADR 0034) -----------------------------------

/// Ein Eintrag im append-only Backup-Protokoll (`backup_log`). Hält je
/// Sicherungslauf (Floor + ggf. Off-Site-Spiegelung, jeweils Erfolg/Fehler)
/// Datum, Auslöser, Ziel-Typ, Datei/Pfad, Größe und Status. Enthält **niemals**
/// die Passphrase — `detail` trägt nur Fehlertext.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupLogRow {
    pub id: String,
    pub created_at: String,
    pub trigger: String,
    pub target_kind: String,
    pub target_label: Option<String>,
    pub file_name: String,
    pub full_path: String,
    pub size_bytes: i64,
    pub status: String,
    pub detail: Option<String>,
}

// ---- Invoices --------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceRow {
    pub id: String,
    pub invoice_number: String,
    pub fiscal_year: i64,
    pub direction: String,
    pub invoice_date: String,
    pub delivery_date: Option<String>,
    pub due_date: Option<String>,
    pub contact_id: String,
    pub seller_name: String,
    pub seller_street: String,
    pub seller_postal_code: String,
    pub seller_city: String,
    pub seller_tax_number: Option<String>,
    pub seller_vat_id: Option<String>,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub is_kleinunternehmer: i64,
    pub pdf_template: String,
    pub status: String,
    pub sent_at: Option<String>,
    pub paid_amount_cents: i64,
    pub paid_at: Option<String>,
    pub payment_history_json: Option<String>,
    pub canceled_at: Option<String>,
    pub canceled_by_storno_id: Option<String>,
    pub is_storno_for: Option<String>,
    pub cancel_reason: Option<String>,
    pub validation_status: Option<String>,
    pub validation_report: Option<String>,
    pub validated_at: Option<String>,
    pub pdf_archive_id: Option<String>,
    pub xml_archive_id: Option<String>,
    pub locked_at: Option<String>,
    pub notes: Option<String>,
    /// Optionaler Bezahlt-/Zahlungshinweis (Migration 0018) — reiner PDF-Text.
    pub payment_note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    // Buyer-Snapshot (Migration 0004) — Empfänger-Stand zur Rechnungszeit,
    // eingefroren für GoBD-Konsistenz + DSGVO-Anonymisierungs-Robustheit.
    pub buyer_name: Option<String>,
    pub buyer_street: Option<String>,
    pub buyer_postal_code: Option<String>,
    pub buyer_city: Option<String>,
    pub buyer_country_code: Option<String>,
    pub buyer_vat_id: Option<String>,
    pub buyer_email: Option<String>,
    // Verknüpfung zum Ursprungsangebot (Migration 0005, gesetzt bei der
    // Konvertierung in Block 7). `None` bei direkt erstellten Rechnungen.
    pub derived_from_quote_id: Option<String>,
    // Zahlungs-Eingangs-Konto (Migration 0007, Block 9). Spalte additiv;
    // in den Zahlungs-Workflow wird sie erst bei Bedarf verdrahtet.
    pub paid_to_account_id: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceItemRow {
    pub id: String,
    pub invoice_id: String,
    pub position: i64,
    pub description: String,
    pub quantity: f64,
    pub unit_code: String,
    pub unit_price_cents: i64,
    pub net_amount_cents: i64,
    pub tax_rate_percent: f64,
    pub tax_category_code: String,
    // Paket-Provenienz + PDF-Markup (Block P3, Migr. 0020/0021). NULL = Alt-Verhalten.
    pub description_title: Option<String>,
    pub description_markup: Option<String>,
    pub source_package_id: Option<String>,
    pub source_package_revision: Option<i64>,
}

/// Aggregat-Type: Invoice + Items + Buyer-Snapshot. Wird für Detail-Views
/// und für die `lock_and_issue`-Pipeline gebraucht (Generator + Renderer
/// brauchen beide Komponenten zusammen).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceDetail {
    pub invoice: InvoiceRow,
    pub items: Vec<InvoiceItemRow>,
    pub buyer: Option<ContactRow>,
}

/// Kondensierter Listen-Eintrag — nur die Felder, die das UI in der
/// Übersicht braucht.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceListItem {
    pub id: String,
    pub invoice_number: String,
    pub fiscal_year: i64,
    pub invoice_date: String,
    pub due_date: Option<String>,
    pub contact_id: String,
    pub contact_name: String,
    pub gross_amount_cents: i64,
    pub paid_amount_cents: i64,
    pub currency_code: String,
    pub status: String,
    pub is_storno_for: Option<String>,
}

// ---- Quotes (Block 6, Migration 0005) --------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRow {
    pub id: String,
    pub quote_number: String,
    pub fiscal_year: i64,
    pub quote_date: String,
    pub valid_until: String,
    pub contact_id: String,
    // Seller-Snapshot (immutable nach Lock)
    pub seller_name: String,
    pub seller_street: String,
    pub seller_postal_code: String,
    pub seller_city: String,
    pub seller_tax_number: Option<String>,
    pub seller_vat_id: Option<String>,
    // Beträge in Cent
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub is_kleinunternehmer: i64,
    pub pdf_template: String,
    pub status: String,
    pub sent_at: Option<String>,
    pub accepted_at: Option<String>,
    pub rejected_at: Option<String>,
    pub canceled_at: Option<String>,
    pub canceled_reason: Option<String>,
    pub converted_at: Option<String>,
    pub converted_invoice_id: Option<String>,
    pub pdf_archive_id: Option<String>,
    pub locked_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    // Buyer-Snapshot (Migration 0023, Block 19) — Empfänger-Stand zur Angebots-
    // zeit, eingefroren für DSGVO-Anonymisierungs-Robustheit (analog invoices 0004).
    pub buyer_name: Option<String>,
    pub buyer_street: Option<String>,
    pub buyer_postal_code: Option<String>,
    pub buyer_city: Option<String>,
    pub buyer_country_code: Option<String>,
    pub buyer_vat_id: Option<String>,
    pub buyer_email: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteItemRow {
    pub id: String,
    pub quote_id: String,
    pub position: i64,
    pub description: String,
    pub quantity: f64,
    pub unit_code: String,
    pub unit_price_cents: i64,
    pub net_amount_cents: i64,
    pub tax_rate_percent: f64,
    pub tax_category_code: String,
    // Paket-Provenienz + PDF-Markup (Block P3, Migr. 0020/0021). NULL = Alt-Verhalten.
    pub description_title: Option<String>,
    pub description_markup: Option<String>,
    pub source_package_id: Option<String>,
    pub source_package_revision: Option<i64>,
}

/// Aggregat: Quote + Items + Buyer-Kontakt + verknüpfte Anhänge
/// (z. B. der unterschriebene Vertrag beim Annahme-Workflow).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteDetail {
    pub quote: QuoteRow,
    pub items: Vec<QuoteItemRow>,
    pub buyer: Option<ContactRow>,
    pub attachments: Vec<AttachmentView>,
}

// ---- Packages (Block P2, Migration 0019) -----------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCategoryRow {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRow {
    pub id: String,
    pub category_id: Option<String>,
    pub name: String,
    pub status: String,
    pub current_revision: Option<i64>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRevisionRow {
    pub id: String,
    pub package_id: String,
    pub revision: i64,
    pub title: String,
    pub body_markup: String,
    pub default_unit_price_cents: i64,
    pub unit_code: String,
    pub tax_category_code: String,
    pub note: Option<String>,
    pub created_at: String,
}

/// Kondensierter Listen-Eintrag für die Angebots-Übersicht.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteListItem {
    pub id: String,
    pub quote_number: String,
    pub fiscal_year: i64,
    pub quote_date: String,
    pub valid_until: String,
    pub contact_id: String,
    pub contact_name: String,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub status: String,
}

// ---- Legal Documents (Block 8, Migration 0006) -----------------------------

/// Eine Version eines Rechtsdokuments (AGB oder Datenschutz). Die PDF-Bytes
/// liegen write-once in `archive_entries`; hier nur Metadaten + Aktiv-Status.
/// Anzeige-Sicht: JOIN auf `archive_entries` für Dateiname/Größe.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalDocumentRow {
    pub id: String,
    pub doc_type: String, // 'agb' | 'privacy'
    pub version: i64,
    pub title: String,
    pub archive_entry_id: String,
    pub is_active: i64, // 0|1
    pub created_at: String,
    pub activated_at: Option<String>,
    pub deactivated_at: Option<String>,
    // JOIN archive_entries
    pub file_name: String,
    pub file_size_bytes: i64,
    pub mime_type: String,
}

/// Verknüpfung Angebot ↔ ausgegebene Legal-Version (append-only), Anzeige-Sicht
/// mit Titel + Dateiname aus `legal_documents`/`archive_entries`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteLegalDocumentView {
    pub id: String,
    pub quote_id: String,
    pub legal_document_id: String,
    pub doc_type: String,
    pub version: i64,
    pub bound_at: String,
    // JOIN legal_documents / archive_entries
    pub title: String,
    pub archive_entry_id: String,
    pub file_name: String,
}

// ---- Attachments (Block 6) -------------------------------------------------

/// Anzeige-Sicht eines Anhangs: `attachments` JOIN `archive_entries`. Liefert
/// genug fürs UI (Dateiname, Größe, MIME) ohne separaten Archive-Lookup.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentView {
    pub id: String,
    pub parent_type: String,
    pub parent_id: String,
    pub archive_entry_id: String,
    pub label: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub mime_type: String,
}

// ---- Payment Accounts (Block 9, Migration 0007) ----------------------------

/// Ein Zahlungs-Konto (Bank/Bargeld/PayPal/…). Die DB-Spalte heißt `type`
/// (SQL-Keyword); die Repo-Queries aliasen sie als `account_type` (kein
/// `#[sqlx(rename)]` nötig). Booleans als INTEGER 0/1.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountRow {
    pub id: String,
    pub label: String,
    pub account_type: String, // 'bank' | 'cash' | 'paypal' | 'stripe' | 'other'
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub is_default: i64,         // 0|1
    pub active: i64,             // 0|1
    pub show_on_invoice: i64,    // 0|1 — auf Belegen anzeigen (mehrere möglich)
    pub details: Option<String>, // Zahlungsadresse/Referenz für Nicht-Bank (PayPal …)
    pub created_at: String,
}

// ---- Expenses (Block 9, Migration 0007) ------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseRow {
    pub id: String,
    pub expense_number: String,
    pub fiscal_year: i64,
    pub expense_date: String,
    pub paid_date: Option<String>,
    pub paid_from_account_id: Option<String>,
    pub vendor_contact_id: Option<String>,
    pub vendor_name_snapshot: String,
    pub vendor_invoice_number: Option<String>,
    pub category: String,
    pub description: String,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub reverse_charge_13b: i64, // 0|1
    pub receipt_archive_id: Option<String>,
    pub einvoice_validation_status: Option<String>,
    pub einvoice_validation_report: Option<String>,
    pub recurring_subscription_id: Option<String>,
    pub capitalized_as_asset_id: Option<String>,
    pub status: String, // 'recorded' | 'canceled'
    pub canceled_at: Option<String>,
    pub canceled_reason: Option<String>,
    pub locked_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Kondensierter Listen-Eintrag für die Kosten-Übersicht (mit Konto-Label-Join).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseListItem {
    pub id: String,
    pub expense_number: String,
    pub fiscal_year: i64,
    pub expense_date: String,
    pub paid_date: Option<String>,
    pub vendor_name_snapshot: String,
    pub vendor_invoice_number: Option<String>,
    pub category: String,
    pub description: String,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub status: String,
    pub reverse_charge_13b: i64,
    pub receipt_archive_id: Option<String>,
    /// Gesetzt, wenn die Position aus einem wiederkehrenden Abo erzeugt wurde
    /// (Block 10) — für das „Abo"-Badge in der Kosten-Liste.
    pub recurring_subscription_id: Option<String>,
}

/// Aggregat: Expense + Lieferanten-Kontakt (optional) + zusätzliche Anhänge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseDetail {
    pub expense: ExpenseRow,
    pub vendor: Option<ContactRow>,
    pub attachments: Vec<AttachmentView>,
    /// PV1-A5: erkanntes E-Rechnungs-Format des primären Belegs, abgeleitet
    /// aus `archive_entries.mime_type` (`application/pdf` → `zugferd`,
    /// `application/xml` → `xrechnung-cii` als grober Anker). Die feine
    /// CII/UBL-Unterscheidung passiert erst im Roh-XML-Viewer-Command. `None`,
    /// wenn der Beleg keine empfangene E-Rechnung ist (normale Kosten ohne
    /// XML-Original). Steuert die Sichtbarkeit des „Roh-XML anzeigen"-Buttons.
    #[serde(default)]
    pub source_format: Option<String>,
}

// ---- Private Movements (Block 9, Migration 0008) ---------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementRow {
    pub id: String,
    pub movement_number: String,
    pub fiscal_year: i64,
    pub movement_date: String,
    pub movement_type: String, // 'entnahme' | 'einlage'
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub description: String,
    pub receipt_archive_id: Option<String>,
    pub locked_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Listen-Eintrag mit aufgelöstem Konto-Label (LEFT JOIN payment_accounts).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementListItem {
    pub id: String,
    pub movement_number: String,
    pub fiscal_year: i64,
    pub movement_date: String,
    pub movement_type: String,
    pub amount_cents: i64,
    pub account_id: Option<String>,
    pub account_label: Option<String>,
    pub description: String,
}

// ---- Recurring Subscriptions (Block 10, Migration 0009) --------------------

/// Ein wiederkehrendes Abo (Template/Stammdatum, KEIN GoBD-Beleg — editierbar
/// und pausierbar). Booleans als INTEGER 0/1. Die daraus erzeugten Kosten sind
/// dagegen sofort gelockt (siehe `expenses`).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringSubscriptionRow {
    pub id: String,
    pub label: String,
    pub vendor_contact_id: Option<String>,
    pub frequency: String, // 'monthly' | 'quarterly' | 'semiannually' | 'annually'
    pub day_of_period: i64,
    pub next_due_date: String,
    pub expected_amount_cents: i64,
    pub category: String,
    pub description_template: String,
    pub auto_create_expense: i64,        // 0|1
    pub reverse_charge_13b_default: i64, // 0|1
    pub active: i64,                     // 0|1
    pub last_executed_at: Option<String>,
    pub last_expense_id: Option<String>,
    pub created_at: String,
}

// ---- Wiederkehrende Ausgangsrechnungen (Block RI-1, Migration 0024) --------

/// Eine Abo-Rechnungs-Vorlage (Stammdatum — editierbar/pausierbar). Booleans
/// als INTEGER 0/1. Die daraus erzeugten Rechnungen sind nach dem Festschreiben
/// unveränderlich (invoices-Trigger). Beträge der Positionen sind Netto-Cent.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoiceRow {
    pub id: String,
    pub label: String,
    pub contact_id: String,
    pub frequency: String, // 'monthly' | 'quarterly' | 'semiannually' | 'annually'
    pub day_of_period: i64,
    pub next_due_date: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub auto_mode: String, // 'draft' | 'issue' | 'issue_send'
    pub payment_terms_days: i64,
    pub pdf_template: String,
    pub service_period_note: i64, // 0|1
    pub active: i64,              // 0|1
    pub last_executed_at: Option<String>,
    pub last_invoice_id: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Eine Position einer Abo-Rechnungs-Vorlage. Spiegelt [`InvoiceItemRow`]
/// (inkl. PDF-Titel/Markup + Paket-Provenienz), damit der Scheduler sie 1:1
/// in `invoice_items` materialisieren kann.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoiceItemRow {
    pub id: String,
    pub recurring_invoice_id: String,
    pub position: i64,
    pub description: String,
    pub quantity: f64,
    pub unit_code: String,
    pub unit_price_cents: i64,
    pub net_amount_cents: i64,
    pub tax_rate_percent: f64,
    pub tax_category_code: String,
    pub description_title: Option<String>,
    pub description_markup: Option<String>,
    pub source_package_id: Option<String>,
    pub source_package_revision: Option<i64>,
}

/// Aggregat: Vorlagen-Kopf + Positionen. Für Detail-Views und für die
/// Scheduler-Materialisierung (Block RI-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoiceDetail {
    pub template: RecurringInvoiceRow,
    pub items: Vec<RecurringInvoiceItemRow>,
}

// ---- Assets (Block 12, Migration 0010) -------------------------------------

/// Eine Anlage (Wirtschaftsgut). Bis zur ersten AfA-Buchung editierbar
/// (`locked_at` NULL); danach sind die Kernfelder durch `trg_assets_immutable`
/// gesperrt. Booleans als INTEGER 0/1; Geld als Integer-Cents; Nutzungsdauer +
/// Privatanteil als REAL.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRow {
    pub id: String,
    pub asset_number: String,
    pub label: String,
    pub acquisition_date: String,
    pub acquisition_cost_cents: i64,
    pub acquisition_fiscal_year: i64,
    pub expense_id: Option<String>,
    pub vendor_contact_id: Option<String>,
    pub depreciation_method: String, // 'gwg_sofort' | 'linear' | 'computer_special_2021'
    pub useful_life_years: Option<f64>,
    pub afa_category: Option<String>,
    pub business_share_percent: f64,
    pub book_value_cents: i64,
    pub last_depreciation_year: Option<i64>,
    pub disposed: i64, // 0|1
    pub disposal_date: Option<String>,
    pub disposal_type: Option<String>, // 'sale' | 'scrap' | 'given_away'
    pub disposal_proceeds_cents: Option<i64>,
    pub disposal_residual_book_value_cents: Option<i64>,
    pub locked_at: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Kondensierter Listen-Eintrag für die Anlagen-Übersicht.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetListItem {
    pub id: String,
    pub asset_number: String,
    pub label: String,
    pub acquisition_date: String,
    pub acquisition_fiscal_year: i64,
    pub acquisition_cost_cents: i64,
    pub depreciation_method: String,
    pub business_share_percent: f64,
    pub book_value_cents: i64,
    pub last_depreciation_year: Option<i64>,
    pub disposed: i64,
    pub disposal_date: Option<String>,
    pub locked_at: Option<String>,
}

/// Eine Jahres-AfA-Buchung (Block 12, Migration 0011).
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepreciationEntryRow {
    pub id: String,
    pub asset_id: String,
    pub fiscal_year: i64,
    pub depreciation_amount_cents: i64,
    pub months_in_year: i64,
    pub book_value_before_cents: i64,
    pub book_value_after_cents: i64,
    pub is_full_writeoff: i64, // 0|1
    pub computed_at: String,
    pub locked_at: Option<String>,
}

/// Aggregat: Anlage + Lieferanten-Kontakt (optional) + AfA-Historie + (falls
/// aus einer Kosten-Position aktiviert) deren Belegnummer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDetail {
    pub asset: AssetRow,
    pub vendor: Option<ContactRow>,
    pub depreciation_entries: Vec<DepreciationEntryRow>,
    pub source_expense_number: Option<String>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
