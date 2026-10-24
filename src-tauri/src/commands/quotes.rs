//! Tauri-Commands für Angebote (Block 6).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::quote::validate_quote`])
//! - Counter-Allokation ([`crate::db::numbering::next_number`], DocType::Quote)
//! - DB-CRUD + State-Transitions ([`crate::db::repo::quotes`])
//! - Anhang-Archivierung für den unterschriebenen Vertrag
//!   ([`crate::archive::store_bytes`] + [`crate::db::repo::attachments`])
//!
//! ## Lifecycle
//!
//! `draft → (issue/festschreiben) → sent → (accept|reject) → accepted|rejected`,
//! plus `cancel` (GoBD-konformes Zurückziehen). Konvertierung in Rechnung:
//! Block 7. PDF + Mail-Versand: Block 8.
//!
//! ## GoBD-Hardline
//!
//! - `issue` lockt das Angebot (DB-Trigger `trg_quotes_immutable`); ab da
//!   sind Kernfelder unveränderlich.
//! - `cancel` löscht nie, sondern setzt Status `canceled` + Grund.
//! - Jeder Übergang schreibt einen Audit-Log-Eintrag.
//! - Festschreiben triggert ein `auto_critical`-Backup (Backup-Hardline:
//!   Auto-Backup bei jedem lock-Event).
//!
//! ## §19-Hardline
//!
//! `validate_quote` blockt jeden USt-Ausweis bei `is_kleinunternehmer = true`
//! (über `kleinunternehmer::assert_no_vat`) — §14c-Schutz auch im Angebot.

use crate::app_ctx::AppCtx;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::archive::{self, ArchiveKind};
use crate::backup;
use crate::config::Paths;
use crate::db::models::{
    AttachmentView, ContactRow, InvoiceDetail, QuoteDetail, QuoteLegalDocumentView, QuoteListItem,
    QuoteRow,
};
use crate::db::numbering;
use crate::db::repo::audit_log;
use crate::db::repo::invoices::{BuyerSnapshot, SellerSnapshot};
use crate::db::repo::{attachments, contacts, legal_documents, quotes, seller_profile};
use crate::domain::invoice::{BuyerView, SellerView};
use crate::domain::numbering::DocType;
use crate::domain::quote::{self, QuoteBuyerView, QuoteInput, QuoteItemInput};
use crate::error::{Error, Result};
use crate::fiscal_year::guard;
use crate::pdf::typst_render::QuoteRenderInput;
use crate::pdf::{bundle, klausel_check, templates, typst_render};

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDraftArgs {
    pub contact_id: String,
    pub input: QuoteInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssueDto {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptArgs {
    pub quote_id: String,
    /// Fachliches Annahmedatum. `None` → heute (Europe/Berlin).
    pub accepted_date: Option<NaiveDate>,
    /// Unterschriebener Vertrag aus `POST /api/upload/vertrag` (Plan W7).
    /// Wird write-once archiviert und als Attachment verknüpft. `None` →
    /// Annahme ohne Doc.
    #[serde(default)]
    pub signed_contract_upload_id: Option<String>,
    /// Optionales Label für den Anhang (Default: Original-Dateiname).
    pub attachment_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectArgs {
    pub quote_id: String,
    /// Optionaler Ablehnungsgrund (nur fürs Audit-Log).
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelArgs {
    pub quote_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertArgs {
    pub quote_id: String,
    /// Rechnungsdatum (Pflicht). Bestimmt auch das Geschäftsjahr der Rechnung
    /// (das Angebotsdatum ist für die Rechnung irrelevant).
    pub invoice_date: NaiveDate,
    pub delivery_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    /// Überschreibt das Angebots-Template; `None` → Template des Angebots.
    pub pdf_template: Option<String>,
    /// Überschreibt die Angebots-Notiz; `None` → Notiz des Angebots.
    pub notes: Option<String>,
    /// Optionaler Bezahlt-/Zahlungshinweis (reiner PDF-Text auf der Rechnung).
    pub payment_note: Option<String>,
    /// Angepasste Positionen; `None` → 1:1 aus dem Angebot übernehmen.
    pub items: Option<Vec<QuoteItemInput>>,
}

// =============================================================================
// Commands — Read
// =============================================================================

/// Argumente von [`quotes_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesListArgs {
    #[serde(default)]
    pub filter: Option<quotes::ListFilter>,
}

pub async fn quotes_list(ctx: &Arc<AppCtx>, args: QuotesListArgs) -> Result<Vec<QuoteListItem>> {
    let QuotesListArgs { filter } = args;
    let pool = ctx.pool()?;

    quotes::list(&pool, &filter.unwrap_or_default()).await
}

/// Argumente von [`quotes_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesGetArgs {
    pub id: String,
}

pub async fn quotes_get(ctx: &Arc<AppCtx>, args: QuotesGetArgs) -> Result<Option<QuoteDetail>> {
    let QuotesGetArgs { id } = args;
    let pool = ctx.pool()?;

    quotes::get_detail(&pool, &id).await
}

/// Argumente von [`quotes_attachments_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesAttachmentsListArgs {
    pub quote_id: String,
}

pub async fn quotes_attachments_list(
    ctx: &Arc<AppCtx>,
    args: QuotesAttachmentsListArgs,
) -> Result<Vec<AttachmentView>> {
    let QuotesAttachmentsListArgs { quote_id } = args;
    let pool = ctx.pool()?;

    attachments::list_for_parent(&pool, "quote", &quote_id).await
}

// =============================================================================
// Commands — Draft
// =============================================================================

/// Argumente von [`quotes_create_draft`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesCreateDraftArgs {
    pub args: CreateDraftArgs,
}

pub async fn quotes_create_draft(
    ctx: &Arc<AppCtx>,
    args: QuotesCreateDraftArgs,
) -> Result<QuoteDetail> {
    let QuotesCreateDraftArgs { args } = args;
    let pool = ctx.pool()?;

    create_draft_core(&pool, args).await
}

/// Testbarer Kern von [`quotes_create_draft`] (ohne Tauri-State).
pub async fn create_draft_core(
    pool: &SqlitePool,
    mut args: CreateDraftArgs,
) -> Result<QuoteDetail> {
    // P3: Positionen mit Markdown-Body → `description` aus dem Markup als Klartext
    // neu berechnen (konsistente Beschreibung, auch nach Body-Edits).
    recompute_markup_descriptions(&mut args.input.items);

    let buyer_row = contacts::get(pool, &args.contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Empfänger nicht gefunden: {}", args.contact_id)))?;
    let seller_row = seller_profile::get(pool).await?.ok_or_else(|| {
        Error::Domain(
            "Stammdaten (seller_profile) noch nicht gepflegt — bitte unter Einstellungen anlegen."
                .into(),
        )
    })?;

    let seller_view = make_seller_view(&seller_row);
    let buyer_view = QuoteBuyerView {
        name: &buyer_row.name,
    };

    // Strukturelle Pre-Conditions blocken schon den Draft (analog Rechnung).
    let today = today_berlin();
    if let Err(errs) = quote::validate_quote(&args.input, &seller_view, &buyer_view, today) {
        let blockers: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    quote::QuoteValidationError::NoItems
                        | quote::QuoteValidationError::CurrencyEmpty
                        | quote::QuoteValidationError::TotalNotPositive
                        | quote::QuoteValidationError::ItemInvalidTaxCategoryCode { .. }
                        | quote::QuoteValidationError::ItemDuplicatePosition(_)
                )
            })
            .cloned()
            .collect();
        if !blockers.is_empty() {
            return Err(Error::Domain(format!(
                "Draft kann nicht angelegt werden: {}",
                blockers
                    .iter()
                    .map(quote::message)
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
    }

    // GJ = Jahr des Angebotsdatums (Plan W1.2/W9, nie vom Client) — bestimmt
    // den Nummernkreis.
    let fiscal_year = i64::from(crate::domain::fiscal_year::fiscal_year_for(
        &args.input.quote_date,
    ));
    let quote_number = numbering::next_number(pool, DocType::Quote, fiscal_year as i32).await?;
    let totals = quote::compute_totals(&args.input.items);
    let snapshot = SellerSnapshot {
        name: seller_row.name.as_str(),
        street: seller_row.street.as_str(),
        postal_code: seller_row.postal_code.as_str(),
        city: seller_row.city.as_str(),
        tax_number: seller_row.tax_number.as_deref(),
        vat_id: seller_row.vat_id.as_deref(),
    };
    // Block 19: Empfänger-Snapshot zur Angebotszeit einfrieren (analog Rechnung).
    let buyer_snapshot = BuyerSnapshot {
        name: buyer_row.name.as_str(),
        street: buyer_row.street.as_deref(),
        postal_code: buyer_row.postal_code.as_deref(),
        city: buyer_row.city.as_deref(),
        country_code: buyer_row.country_code.as_str(),
        vat_id: buyer_row.vat_id.as_deref(),
        email: buyer_row.email.as_deref(),
    };

    let payload = quotes::DraftCreatePayload {
        contact_id: args.contact_id.clone(),
        fiscal_year,
        is_kleinunternehmer: seller_row.is_kleinunternehmer == 1,
        input: args.input.clone(),
    };

    let row = quotes::create_draft(
        pool,
        &payload,
        &quote_number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await?;

    audit_log::append(
        pool,
        "quote.draft_create",
        "quote",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","gross":{},"items":{}}}"#,
            quote_number,
            totals.gross_amount_cents,
            args.input.items.len()
        )),
    )
    .await?;

    quotes::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("create_draft: detail-load leer".into()))
}

/// Argumente von [`quotes_validate_draft`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesValidateDraftArgs {
    pub id: String,
}

pub async fn quotes_validate_draft(
    ctx: &Arc<AppCtx>,
    args: QuotesValidateDraftArgs,
) -> Result<Vec<ValidationIssueDto>> {
    let QuotesValidateDraftArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let detail = quotes::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {id}")))?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten fehlen — kann nicht validieren".into()))?;
    let buyer_row = detail
        .buyer
        .as_ref()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let input = detail_to_input(&detail)?;
    let seller_view = make_seller_view(&seller_row);
    let buyer_view = QuoteBuyerView {
        name: &buyer_row.name,
    };

    let issues = match quote::validate_quote(&input, &seller_view, &buyer_view, today_berlin()) {
        Ok(()) => Vec::new(),
        Err(errs) => errs
            .into_iter()
            .map(|e| ValidationIssueDto {
                code: quote::variant_name(&e).to_string(),
                message: quote::message(&e),
            })
            .collect(),
    };
    Ok(issues)
}

// =============================================================================
// Commands — State Transitions
// =============================================================================

/// Argumente von [`quotes_issue`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesIssueArgs {
    pub id: String,
}

/// Festschreiben ("issue"): validiert hart, lockt das Angebot und setzt
/// Status `sent`. Triggert ein Auto-Critical-Backup (Backup-Hardline).
pub async fn quotes_issue(ctx: &Arc<AppCtx>, args: QuotesIssueArgs) -> Result<QuoteRow> {
    let QuotesIssueArgs { id } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();

    let detail = quotes::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {id}")))?;
    // GJ-Festschreibung (Block 15): kein Angebot mit Datum in einem
    // abgeschlossenen Geschäftsjahr festschreiben.
    crate::fiscal_year::guard::ensure_year_open(pool, detail.quote.fiscal_year).await?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten fehlen".into()))?;
    let buyer_row = detail
        .buyer
        .clone()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let seller_view = make_seller_view(&seller_row);
    let buyer_view = QuoteBuyerView {
        name: &buyer_row.name,
    };
    let input = detail_to_input(&detail)?;

    quote::validate_quote(&input, &seller_view, &buyer_view, today_berlin()).map_err(|errs| {
        Error::Domain(format!(
            "Validation failed ({} Fehler): {}",
            errs.len(),
            errs.iter()
                .take(5)
                .map(quote::message)
                .collect::<Vec<_>>()
                .join("; ")
        ))
    })?;

    // Block 19: Empfänger-Snapshot beim Festschreiben aus dem aktuellen
    // Live-Kontakt auffrischen (maßgeblicher Stand zur Festschreibung), bevor
    // gelockt wird. Danach rendert `ensure_quote_pdf` aus dem Snapshot.
    let buyer_snapshot = BuyerSnapshot {
        name: buyer_row.name.as_str(),
        street: buyer_row.street.as_deref(),
        postal_code: buyer_row.postal_code.as_deref(),
        city: buyer_row.city.as_deref(),
        country_code: buyer_row.country_code.as_str(),
        vat_id: buyer_row.vat_id.as_deref(),
        email: buyer_row.email.as_deref(),
    };
    quotes::set_buyer_snapshot(pool, &id, &buyer_snapshot).await?;

    quotes::issue(pool, &id).await?;

    audit_log::append(
        pool,
        "quote.issue",
        "quote",
        &id,
        Some(&format!(r#"{{"number":"{}"}}"#, detail.quote.quote_number)),
    )
    .await?;

    // Backup-Hardline: lock-Event → Auto-Critical-Backup (best-effort).
    backup::auto_backup_if_unlocked(pool, &paths, session, "quote.issue")
        .await
        .ok();

    quotes::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("issue: post-UPDATE SELECT leer".into()))
}

/// Argumente von [`quotes_accept`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesAcceptArgs {
    pub args: AcceptArgs,
}

/// Annahme durch den Kunden. Optional wird der unterschriebene Vertrag
/// write-once archiviert und als Attachment (`parent_type='quote'`) verknüpft.
pub async fn quotes_accept(ctx: &Arc<AppCtx>, args: QuotesAcceptArgs) -> Result<QuoteDetail> {
    let QuotesAcceptArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let paths = ctx.paths.clone();

    let quote_row = quotes::get(pool, &args.quote_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {}", args.quote_id)))?;

    // 1. Optionaler Vertrags-Upload → Archiv + Attachment.
    let upload_id = args
        .signed_contract_upload_id
        .as_deref()
        .filter(|id| !id.is_empty());
    let contract = match upload_id {
        Some(id) => Some(
            ctx.uploads
                .read(id, &[crate::uploads::UploadKind::Contract])?,
        ),
        None => None,
    };
    // Status vor dem Archivieren prüfen — sonst bleibt bei einem schon
    // angenommenen Angebot ein unlöschbarer Anhang zurück (Gegenprüfung W7).
    if quote_row.status != "sent" {
        return Err(Error::Domain(format!(
            "Annahme nur für versendete Angebote möglich (status={})",
            quote_row.status
        )));
    }
    if let Some(contract) = contract.as_ref() {
        // R2-024 (GJ-Guard): Beilagen werden mit `quote_row.fiscal_year` ins
        // write-once-Archiv abgelegt. Liegt das Angebot in einem festgeschrieben
        // GJ, würden wir nachträglich an dessen Archiv anhängen — also den
        // Lock-Snapshot rückwirkend erweitern. Vor dem Archiv-Schreiben blocken.
        guard::ensure_year_open(pool, quote_row.fiscal_year).await?;

        let original_name = sanitize_filename(&contract.file_name);
        // Eindeutiger Archiv-Dateiname (write-once, UNIQUE-Pfad): mit
        // Angebotsnummer prefixen, damit gleiche Original-Namen kollisionsfrei
        // bleiben.
        let file_name = format!("{}-{}", quote_row.quote_number, original_name);
        // Typ aus der Inhaltsprüfung, nicht aus der Endung (Plan W7).
        let mime = contract.mime;

        let stored = archive::store_bytes(
            pool,
            &paths.archive_dir,
            quote_row.fiscal_year as i32,
            ArchiveKind::Attachment,
            &file_name,
            mime,
            &contract.bytes,
        )
        .await?;

        let sort_order = attachments::count_for_parent(pool, "quote", &args.quote_id).await?;
        let label = args.attachment_label.as_deref().unwrap_or(&original_name);
        let attachment_id = attachments::create(
            pool,
            "quote",
            &args.quote_id,
            &stored.archive_id,
            Some(label),
            sort_order,
        )
        .await?;

        audit_log::append(
            pool,
            "quote.attachment_added",
            "quote",
            &args.quote_id,
            Some(&format!(
                r#"{{"attachment":"{}","archive":"{}","file":"{}"}}"#,
                attachment_id,
                stored.archive_id,
                escape(&file_name)
            )),
        )
        .await?;
    }

    // 2. Status-Übergang sent → accepted.
    let accepted_iso = args.accepted_date.unwrap_or_else(today_berlin).to_string();
    quotes::accept(pool, &args.quote_id, &accepted_iso).await?;

    audit_log::append(
        pool,
        "quote.accepted",
        "quote",
        &args.quote_id,
        Some(&format!(
            r#"{{"number":"{}","accepted_at":"{}","with_contract":{}}}"#,
            quote_row.quote_number,
            accepted_iso,
            contract.is_some()
        )),
    )
    .await?;
    if let Some(id) = upload_id {
        ctx.uploads.consume(id);
    }

    quotes::get_detail(pool, &args.quote_id)
        .await?
        .ok_or_else(|| Error::Domain("accept: detail-load leer".into()))
}

/// Argumente von [`quotes_reject`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesRejectArgs {
    pub args: RejectArgs,
}

pub async fn quotes_reject(ctx: &Arc<AppCtx>, args: QuotesRejectArgs) -> Result<QuoteRow> {
    let QuotesRejectArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    quotes::reject(pool, &args.quote_id).await?;
    audit_log::append(
        pool,
        "quote.rejected",
        "quote",
        &args.quote_id,
        Some(&format!(
            r#"{{"reason":"{}"}}"#,
            escape(args.reason.as_deref().unwrap_or(""))
        )),
    )
    .await?;
    quotes::get(pool, &args.quote_id)
        .await?
        .ok_or_else(|| Error::Domain("reject: post-UPDATE SELECT leer".into()))
}

/// Argumente von [`quotes_cancel`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesCancelArgs {
    pub args: CancelArgs,
}

pub async fn quotes_cancel(ctx: &Arc<AppCtx>, args: QuotesCancelArgs) -> Result<QuoteRow> {
    let QuotesCancelArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    quotes::cancel(pool, &args.quote_id, &args.reason).await?;
    audit_log::append(
        pool,
        "quote.cancel",
        "quote",
        &args.quote_id,
        Some(&format!(r#"{{"reason":"{}"}}"#, escape(&args.reason))),
    )
    .await?;
    quotes::get(pool, &args.quote_id)
        .await?
        .ok_or_else(|| Error::Domain("cancel: post-UPDATE SELECT leer".into()))
}

// =============================================================================
// Commands — Konvertierung (Block 7)
// =============================================================================

/// Argumente von [`quotes_convert_to_invoice`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesConvertToInvoiceArgs {
    pub args: ConvertArgs,
}

/// Konvertiert ein **angenommenes** Angebot in eine Rechnungs-**Draft**.
///
/// **Hard-Rule (Manuel):** nur aus `status='accepted'` — aus dem
/// unterschriebenen/angenommenen Angebot wird die Rechnung. Andere Status
/// (inkl. eines bereits konvertierten Angebots) werden abgelehnt.
///
/// Ablauf: Positionen übernehmen (oder im Frontend angepasst) →
/// [`quote::convert_to_invoice`] (pure) → gemeinsamer Draft-Helper
/// [`crate::commands::invoices::create_invoice_draft_from_input`] (setzt
/// `derived_from_quote_id` + Audit `invoice.draft_create`) → Angebot auf
/// `converted` setzen ([`quotes::mark_converted`]) → Audit `quote.converted`.
///
/// Die erzeugte Rechnung ist ein normaler Draft und wird über die übliche
/// Invoice-Pipeline (`invoices_lock_and_issue`) festgeschrieben — inkl.
/// KoSIT-Validierung, ZUGFeRD/Mustang und §19-Klausel-Check. Damit greift die
/// §19-Hardline beim Festschreiben unverändert; die Konvertierung übernimmt
/// die USt-Felder 1:1 (bei §19: Category `E`, Rate `0`).
pub async fn quotes_convert_to_invoice(
    ctx: &Arc<AppCtx>,
    args: QuotesConvertToInvoiceArgs,
) -> Result<InvoiceDetail> {
    let QuotesConvertToInvoiceArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;

    let detail = quotes::get_detail(pool, &args.quote_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {}", args.quote_id)))?;
    let quote = &detail.quote;

    if quote.status != "accepted" {
        return Err(Error::Domain(format!(
            "Konvertierung nur aus einem angenommenen Angebot möglich (aktueller Status: {}). \
             Ein Angebot muss erst angenommen werden, bevor daraus eine Rechnung wird.",
            quote.status
        )));
    }

    // Positionen: angepasst (Frontend) oder 1:1 aus dem festgeschriebenen Angebot.
    let items: Vec<QuoteItemInput> = match args.items {
        Some(its) => its,
        None => detail
            .items
            .iter()
            .map(|it| QuoteItemInput {
                position: it.position as u32,
                description: it.description.clone(),
                quantity: it.quantity,
                unit_code: it.unit_code.clone(),
                unit_price_cents: it.unit_price_cents,
                tax_rate_percent: it.tax_rate_percent,
                tax_category_code: it.tax_category_code.clone(),
                description_title: it.description_title.clone(),
                description_markup: it.description_markup.clone(),
                source_package_id: it.source_package_id.clone(),
                source_package_revision: it.source_package_revision,
            })
            .collect(),
    };

    let opts = quote::ConvertToInvoiceOptions {
        invoice_date: args.invoice_date,
        delivery_date: args.delivery_date,
        due_date: args.due_date,
        currency_code: quote.currency_code.clone(),
        notes: args.notes.or_else(|| quote.notes.clone()),
        payment_note: args.payment_note,
        pdf_template: args
            .pdf_template
            .unwrap_or_else(|| quote.pdf_template.clone()),
    };
    let input = quote::convert_to_invoice(&items, &opts);
    // GJ leitet create_invoice_draft_from_input aus dem Rechnungsdatum ab (W1.2).

    // Gemeinsamer Draft-Helper: Snapshots, Belegnummer, derived_from_quote_id,
    // Audit invoice.draft_create.
    let invoice_detail = crate::commands::invoices::create_invoice_draft_from_input(
        pool,
        &quote.contact_id,
        &input,
        Some(&quote.id),
    )
    .await?;

    // Angebot abschließen: accepted → converted (Guard im Repo).
    quotes::mark_converted(pool, &quote.id, &invoice_detail.invoice.id).await?;

    audit_log::append(
        pool,
        "quote.converted",
        "quote",
        &quote.id,
        Some(&format!(
            r#"{{"quote_number":"{}","invoice":"{}","invoice_number":"{}"}}"#,
            quote.quote_number, invoice_detail.invoice.id, invoice_detail.invoice.invoice_number
        )),
    )
    .await?;

    Ok(invoice_detail)
}

// =============================================================================
// Commands — PDF + Bundle + Versand-Vorbereitung (Block 8)
// =============================================================================

/// Alles, was für den Bundle-Versand/Druck eines Angebots gebraucht wird:
/// das (sichergestellte) Angebots-PDF + die fest gebundenen Legal-Versionen.
#[derive(Debug, Clone)]
pub struct QuoteDispatch {
    pub quote: QuoteRow,
    pub quote_pdf_archive_id: String,
    pub legal: Vec<QuoteLegalDocumentView>,
}

/// Stellt sicher, dass das Angebots-PDF existiert (rendert + archiviert es beim
/// ersten Mal, idempotent danach) und liefert dessen Archive-ID.
///
/// Nur für **festgeschriebene** Angebote (GoBD: das PDF spiegelt den gelockten
/// Stand). Das PDF ist ein Plain-PDF (Angebote sind keine E-Rechnungen → keine
/// Mustang/ZUGFeRD-Stufe). §19-Klausel-Check läuft wie bei Rechnungen.
pub async fn ensure_quote_pdf(pool: &SqlitePool, paths: &Paths, quote_id: &str) -> Result<String> {
    let detail = quotes::get_detail(pool, quote_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {quote_id}")))?;
    let quote = &detail.quote;
    if quote.locked_at.is_none() {
        return Err(Error::Domain(
            "Angebot ist noch nicht festgeschrieben — erst 'Festschreiben' ausführen.".into(),
        ));
    }
    if let Some(aid) = quote.pdf_archive_id.clone() {
        return Ok(aid); // idempotent: PDF schon erzeugt + archiviert.
    }

    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten fehlen".into()))?;
    let buyer_row = detail
        .buyer
        .clone()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let seller_view = make_seller_view(&seller_row);
    // Block 19: festgeschriebenes Angebot → Empfänger aus dem eingefrorenen
    // Snapshot rendern (robust gegen spätere DSGVO-Anonymisierung des Kontakts);
    // Fallback auf den Live-Kontakt nur für Alt-Angebote ohne Snapshot.
    let buyer_view = match make_buyer_view_from_quote(quote) {
        Some(v) => v,
        None => make_buyer_view(&buyer_row),
    };

    let input = QuoteRenderInput {
        quote_date: date10(&quote.quote_date),
        valid_until: date10(&quote.valid_until),
        currency_code: quote.currency_code.clone(),
        items: detail
            .items
            .iter()
            .map(|it| QuoteItemInput {
                position: it.position as u32,
                description: it.description.clone(),
                quantity: it.quantity,
                unit_code: it.unit_code.clone(),
                unit_price_cents: it.unit_price_cents,
                tax_rate_percent: it.tax_rate_percent,
                tax_category_code: it.tax_category_code.clone(),
                description_title: it.description_title.clone(),
                description_markup: it.description_markup.clone(),
                source_package_id: it.source_package_id.clone(),
                source_package_revision: it.source_package_revision,
            })
            .collect(),
    };

    // §19-Hardline: Template muss die Klausel rendern (Marker + Datenfeld).
    // Sentinel 'default' folgt dem globalen Default aus den Stammdaten (Block 17a);
    // 'default' nutzt NIE das Rechnungs-Template, sondern das Angebots-Default.
    let template_name = if quote.pdf_template == "default" {
        seller_row.default_pdf_template.as_str()
    } else {
        quote.pdf_template.as_str()
    };
    let source = templates::resolve_quote_template(&paths.inputs_dir, template_name);
    if quote.is_kleinunternehmer == 1 {
        klausel_check::verify_for_kleinunternehmer(&source)
            .map_err(|e| Error::Domain(format!("§19-Klausel-Check: {e}")))?;
    }
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };

    let logo = crate::branding::find_logo(&crate::branding::branding_dir(&paths.data_dir));
    let logo_vpath = logo.as_ref().map(|(name, _)| format!("/branding/{name}"));
    let logo_opt = match (logo_vpath.as_deref(), logo.as_ref()) {
        (Some(vp), Some((_, bytes))) => Some((vp, bytes.as_slice())),
        _ => None,
    };

    // Angebots-Unterschrift (global an-/abschaltbar; nur auf Angeboten).
    let signature_enabled =
        crate::db::repo::app_settings::get_bool(pool, "quote_signature_enabled", false).await?;
    let signature = if signature_enabled {
        crate::branding::find_signature(&crate::branding::branding_dir(&paths.data_dir))
    } else {
        None
    };
    let sig_vpath = signature
        .as_ref()
        .map(|(name, _)| format!("/branding/{name}"));
    let sig_opt = match (sig_vpath.as_deref(), signature.as_ref()) {
        (Some(vp), Some((_, bytes))) => Some((vp, bytes.as_slice())),
        _ => None,
    };
    // Inhaber-Name (Einzelunternehmer §14) — auch auf dem Angebot. Leere Werte → None.
    let owner_name = crate::db::repo::app_settings::get(pool, "seller_owner_name")
        .await?
        .filter(|s| !s.trim().is_empty());

    // Zahlungs-Konten für das Impressum (alle geflaggten Konten, identisch zur
    // Rechnung). Angebote haben kein XRechnung-XML → nur PDF-Darstellung.
    let acct_rows = crate::db::repo::payment_accounts::invoice_accounts(pool).await?;
    let holder: &str = owner_name.as_deref().unwrap_or(seller_view.name);
    let accounts_json = serde_json::Value::Array(
        acct_rows
            .iter()
            .map(|a| {
                serde_json::json!({
                    "type": a.account_type,
                    "label": a.label,
                    "holder": holder,
                    "iban": a.iban,
                    "bic": a.bic,
                    "details": a.details,
                })
            })
            .collect(),
    );

    let pdf_bytes = typst_render::quote_job(
        &source,
        &quote.quote_number,
        &input,
        &seller_view,
        &buyer_view,
        branding_opt,
        logo_opt,
        sig_opt,
        signature_enabled,
        owner_name.as_deref(),
        &accounts_json,
    )?
    .compile_async()
    .await?;

    let stored = archive::store_bytes(
        pool,
        &paths.archive_dir,
        quote.fiscal_year as i32,
        ArchiveKind::QuotePdf,
        &format!("{}.pdf", quote.quote_number),
        "application/pdf",
        &pdf_bytes,
    )
    .await?;
    quotes::set_pdf_archive_id(pool, quote_id, &stored.archive_id).await?;

    audit_log::append(
        pool,
        "quote.pdf_generated",
        "quote",
        quote_id,
        Some(&format!(
            r#"{{"number":"{}","pdf":"{}"}}"#,
            escape(&quote.quote_number),
            stored.archive_id
        )),
    )
    .await?;

    Ok(stored.archive_id)
}

/// Bindet die aktuell aktiven Legal-Versionen append-only ans Angebot
/// (idempotent). Bei `require = true` schlägt der Aufruf fehl, wenn keine aktive
/// AGB **und** Datenschutz-Version existiert (Pflicht für Bundle/Versand).
pub async fn bind_legal_docs_for_quote(
    pool: &SqlitePool,
    quote_id: &str,
    require: bool,
) -> Result<Vec<QuoteLegalDocumentView>> {
    if require {
        let agb = legal_documents::get_active(pool, "agb").await?;
        let privacy = legal_documents::get_active(pool, "privacy").await?;
        if agb.is_none() || privacy.is_none() {
            return Err(Error::Domain(
                "Für Versand/Bundle müssen aktive AGB UND Datenschutz hinterlegt sein \
                 (Einstellungen → Rechtsdokumente)."
                    .into(),
            ));
        }
    }
    legal_documents::bind_active_for_quote(pool, quote_id).await
}

/// Bereitet alles für den Bundle-Versand/Druck vor: Vorbedingungen prüfen,
/// PDF sicherstellen, Legal-Versionen verpflichtend binden. Public, weil der
/// Mail-Versand ([`crate::commands::mail::send_quote_core`]) das hier nutzt.
pub async fn prepare_quote_dispatch(
    pool: &SqlitePool,
    paths: &Paths,
    quote_id: &str,
) -> Result<QuoteDispatch> {
    let quote = quotes::get(pool, quote_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {quote_id}")))?;
    if quote.locked_at.is_none() {
        return Err(Error::Domain(
            "Angebot ist noch nicht festgeschrieben — erst 'Festschreiben' ausführen.".into(),
        ));
    }
    if quote.status == "canceled" || quote.status == "rejected" {
        return Err(Error::Domain(format!(
            "Angebot im Status '{}' kann nicht als Bundle ausgegeben/versendet werden.",
            quote.status
        )));
    }
    let quote_pdf_archive_id = ensure_quote_pdf(pool, paths, quote_id).await?;
    let legal = bind_legal_docs_for_quote(pool, quote_id, true).await?;
    let quote = quotes::get(pool, quote_id)
        .await?
        .ok_or_else(|| Error::Domain("prepare_quote_dispatch: refresh leer".into()))?;
    Ok(QuoteDispatch {
        quote,
        quote_pdf_archive_id,
        legal,
    })
}

/// Argumente von [`quotes_generate_pdf`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesGeneratePdfArgs {
    pub id: String,
}

/// Erzeugt (falls nötig) das Angebots-PDF und liefert das aktualisierte Angebot
/// (mit `pdfArchiveId`). Das Frontend öffnet das PDF danach via
/// `attachments_open(pdfArchiveId)`.
pub async fn quotes_generate_pdf(
    ctx: &Arc<AppCtx>,
    args: QuotesGeneratePdfArgs,
) -> Result<QuoteRow> {
    let QuotesGeneratePdfArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let paths = ctx.paths.clone();
    ensure_quote_pdf(pool, &paths, &id).await?;
    quotes::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("generate_pdf: post-SELECT leer".into()))
}

/// Argumente von [`quotes_open_bundle`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesOpenBundleArgs {
    pub id: String,
}

/// Erzeugt das **zusammengeführte** Bundle-PDF (Angebot + AGB + Datenschutz),
/// bindet die Legal-Versionen (Pflicht) und liefert es als Download-Ticket
/// (Plan W7, inline im Browser) — für „Drucken/Ansehen" als ein Dokument. Das
/// Bundle ist abgeleitet und wird nicht archiviert (kanonisch sind die
/// Einzel-PDFs + die Bindung).
pub async fn quotes_open_bundle(
    ctx: &Arc<AppCtx>,
    args: QuotesOpenBundleArgs,
) -> Result<crate::downloads::DownloadTicket> {
    let QuotesOpenBundleArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let paths = ctx.paths.clone();
    let dispatch = prepare_quote_dispatch(pool, &paths, &id).await?;

    let mut parts: Vec<Vec<u8>> = Vec::new();
    parts.push(archive::read_and_verify(pool, &dispatch.quote_pdf_archive_id).await?);
    for ld in &dispatch.legal {
        parts.push(archive::read_and_verify(pool, &ld.archive_entry_id).await?);
    }
    let part_count = parts.len();
    let merged = bundle::merge_pdfs(&parts)?;

    let file_name = format!(
        "{}-Angebot-Bundle.pdf",
        sanitize_filename(&dispatch.quote.quote_number)
    );
    let ticket =
        ctx.downloads
            .put_bytes(&file_name, &merged, crate::downloads::DownloadKind::Preview)?;

    audit_log::append(
        pool,
        "quote.bundle_opened",
        "quote",
        &id,
        Some(&format!(r#"{{"parts":{part_count}}}"#)),
    )
    .await?;
    Ok(ticket)
}

/// Argumente von [`quotes_legal_bindings`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotesLegalBindingsArgs {
    pub quote_id: String,
}

/// Liefert die fest mit einem Angebot verknüpften Legal-Versionen (Anzeige).
pub async fn quotes_legal_bindings(
    ctx: &Arc<AppCtx>,
    args: QuotesLegalBindingsArgs,
) -> Result<Vec<QuoteLegalDocumentView>> {
    let QuotesLegalBindingsArgs { quote_id } = args;
    let pool = ctx.pool()?;

    legal_documents::list_for_quote(&pool, &quote_id).await
}

// =============================================================================
// Helpers
// =============================================================================

/// Erste 10 Zeichen eines ISO-Datums ("YYYY-MM-DD"), auch wenn ein Zeitanteil
/// dranhängt.
fn date10(s: &str) -> String {
    s.chars().take(10).collect()
}

fn make_buyer_view(row: &ContactRow) -> BuyerView<'_> {
    BuyerView {
        name: &row.name,
        street: row.street.as_deref(),
        postal_code: row.postal_code.as_deref(),
        city: row.city.as_deref(),
        country_code: &row.country_code,
        vat_id: row.vat_id.as_deref(),
        email: row.email.as_deref(),
    }
}

/// Block 19: Empfänger-View aus dem eingefrorenen Angebots-Snapshot (`buyer_*`).
/// `None` für Alt-Angebote ohne Snapshot (Fallback auf den Live-Kontakt).
fn make_buyer_view_from_quote(q: &QuoteRow) -> Option<BuyerView<'_>> {
    q.buyer_name.as_deref().map(|name| BuyerView {
        name,
        street: q.buyer_street.as_deref(),
        postal_code: q.buyer_postal_code.as_deref(),
        city: q.buyer_city.as_deref(),
        country_code: q.buyer_country_code.as_deref().unwrap_or("DE"),
        vat_id: q.buyer_vat_id.as_deref(),
        email: q.buyer_email.as_deref(),
    })
}

fn today_berlin() -> NaiveDate {
    crate::clock::today_berlin()
}

fn make_seller_view(row: &crate::db::models::SellerProfileRow) -> SellerView<'_> {
    SellerView {
        name: &row.name,
        street: &row.street,
        postal_code: &row.postal_code,
        city: &row.city,
        country_code: &row.country_code,
        tax_number: row.tax_number.as_deref(),
        vat_id: row.vat_id.as_deref(),
        email: &row.email,
        phone: row.phone.as_deref(),
        contact_name: None,
        iban: row.iban.as_deref(),
        bic: row.bic.as_deref(),
        is_kleinunternehmer: row.is_kleinunternehmer == 1,
        waived_since: row
            .waived_paragraph_19_since
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
    }
}

/// P3: Für Positionen mit Markdown-Body wird `description` aus dem Markup als
/// Klartext neu berechnet (konsistente Positions-Beschreibung, auch nach Edits).
fn recompute_markup_descriptions(items: &mut [QuoteItemInput]) {
    for it in items.iter_mut() {
        if let Some(m) = it.description_markup.as_deref() {
            if !m.trim().is_empty() {
                let plain =
                    crate::domain::package::to_plaintext(&crate::domain::package::parse_markup(m));
                if !plain.trim().is_empty() {
                    it.description = plain;
                }
            }
        }
    }
}

fn detail_to_input(detail: &QuoteDetail) -> Result<QuoteInput> {
    let quote = &detail.quote;
    let quote_date = parse_date(&quote.quote_date)?;
    let valid_until = parse_date(&quote.valid_until)?;
    let items = detail
        .items
        .iter()
        .map(|it| quote::QuoteItemInput {
            position: it.position as u32,
            description: it.description.clone(),
            quantity: it.quantity,
            unit_code: it.unit_code.clone(),
            unit_price_cents: it.unit_price_cents,
            tax_rate_percent: it.tax_rate_percent,
            tax_category_code: it.tax_category_code.clone(),
            description_title: it.description_title.clone(),
            description_markup: it.description_markup.clone(),
            source_package_id: it.source_package_id.clone(),
            source_package_revision: it.source_package_revision,
        })
        .collect();
    Ok(QuoteInput {
        quote_date,
        valid_until,
        currency_code: quote.currency_code.clone(),
        items,
        notes: quote.notes.clone(),
        pdf_template: quote.pdf_template.clone(),
    })
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = &s[..s.len().min(10)];
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("ungültiges Datum '{s}': {e}")))
}

/// Reduziert einen Upload-Dateinamen auf einen sicheren Basisnamen ohne
/// Pfad-Trennzeichen (`archive::store_bytes` lehnt `/ \ :` ab) und ohne
/// führende Punkte. Leerer Rest → Fallback.
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "vertrag.pdf".to_string()
    } else {
        trimmed.to_string()
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
