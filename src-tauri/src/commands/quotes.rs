//! Commands für Angebote (Block 6).
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
    ContactRow, InvoiceDetail, QuoteDetail, QuoteLegalDocumentView, QuoteListItem, QuoteRow,
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
    /// Leistungszeitraum statt Leistungsdatum (ADR 0042 Nr. 10), beide oder
    /// keiner.
    #[serde(default)]
    pub service_period_start: Option<NaiveDate>,
    #[serde(default)]
    pub service_period_end: Option<NaiveDate>,
    /// Leitweg-ID bzw. Käuferreferenz (BT-10) der Rechnung (ADR 0042 Nr. 12).
    #[serde(default)]
    pub buyer_reference: Option<String>,
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

/// Testbarer Kern von [`quotes_create_draft`] (ohne `AppCtx`).
pub async fn create_draft_core(
    pool: &SqlitePool,
    mut args: CreateDraftArgs,
) -> Result<QuoteDetail> {
    // P3: Positionen mit Markdown-Body → `description` aus dem Markup als Klartext
    // neu berechnen (konsistente Beschreibung, auch nach Body-Edits).
    recompute_markup_descriptions(&mut args.input.items);
    // KB-0046 (F5-fix): bearbeitete Paket-Positionen verlieren die Provenienz
    // auch am Angebot, nicht erst bei der Umwandlung in eine Rechnung.
    crate::commands::packages::enforce_package_provenance(pool, &mut args.input.items).await?;

    let buyer_row = contacts::get(pool, &args.contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Empfänger nicht gefunden: {}", args.contact_id)))?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;

    // ADR 0042 Nr. 3: Status am Angebotsdatum.
    let seller_view = make_seller_view(
        &seller_row,
        quote_tax_status(pool, args.input.quote_date).await?,
    );
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
                "Der Entwurf kann nicht gespeichert werden: {}",
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
        is_kleinunternehmer: seller_view.is_kleinunternehmer,
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
        Some(serde_json::json!({ "number": quote_number, "gross": totals.gross_amount_cents, "items": args.input.items.len() })),
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
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    let buyer_row = detail
        .buyer
        .as_ref()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let input = detail_to_input(&detail)?;
    let seller_view =
        make_seller_view(&seller_row, quote_tax_status(pool, input.quote_date).await?);
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

    // Lauf F3b: je Angebot nie parallel zu Zurückziehen, Umwandeln, Annahme
    // oder Ablehnung — die Sperre vor dem Lesen des Status.
    let _quote_guard = crate::locks::QUOTES.lock(&id).await;
    let detail = quotes::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {id}")))?;
    // Nur Entwürfe — vor dem Auffrischen des Empfänger-Snapshots prüfen, damit
    // ein zurückgezogener (verworfener) Entwurf unverändert bleibt (W14-032).
    if detail.quote.locked_at.is_some() || detail.quote.status != "draft" {
        return Err(Error::Domain(format!(
            "Angebot {} ist nicht im Entwurf (status={}) — Festschreiben nicht möglich",
            detail.quote.quote_number, detail.quote.status
        )));
    }
    // GJ-Festschreibung (Block 15): kein Angebot mit Datum in einem
    // abgeschlossenen Geschäftsjahr festschreiben.
    let _fy = crate::fiscal_year::guard::ensure_year_open(pool, detail.quote.fiscal_year).await?;
    // F1d: wiederhergestellter Entwurf mit vor dem Restore festgeschriebener
    // Nummer erst nach Bestätigung.
    crate::backup::after_restore::ensure_no_conflict(pool, "quote", &id).await?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    let buyer_row = detail
        .buyer
        .clone()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let input = detail_to_input(&detail)?;
    // KB-0010, ADR 0042 Nr. 3: Status am Angebotsdatum, beim Festschreiben
    // eingefroren.
    let seller_view =
        make_seller_view(&seller_row, quote_tax_status(pool, input.quote_date).await?);
    let buyer_view = QuoteBuyerView {
        name: &buyer_row.name,
    };

    quote::validate_quote(&input, &seller_view, &buyer_view, today_berlin()).map_err(|errs| {
        Error::Domain(format!(
            "Das Angebot kann nicht fertiggestellt werden: {}",
            crate::commands::invoices::issue_reasons(&errs, quote::message)
        ))
    })?;

    // Block 19: Empfänger-Snapshot beim Festschreiben aus dem aktuellen
    // Live-Kontakt auffrischen (maßgeblicher Stand zur Festschreibung), bevor
    // gelockt wird. Danach rendert `ensure_quote_pdf` aus dem Snapshot.
    // Lauf F3b: Kontakt frisch lesen, Status prüfen, Snapshot, Festschreiben
    // und Audit in **einer** Schreib-Transaktion — ein inzwischen
    // zurückgezogener Entwurf (und mit ihm anonymisierter Kontakt) bekommt
    // die Personendaten nicht zurück.
    let mut tx = crate::db::begin_immediate(pool).await?;
    let buyer_row: ContactRow = sqlx::query_as("SELECT * FROM contacts WHERE id = ?")
        .bind(&buyer_row.id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    if buyer_row.anonymized_at.is_some() {
        return Err(Error::Domain(
            "Der Empfänger ist anonymisiert — an ihn wird kein Angebot mehr festgeschrieben."
                .into(),
        ));
    }
    let buyer_snapshot = BuyerSnapshot {
        name: buyer_row.name.as_str(),
        street: buyer_row.street.as_deref(),
        postal_code: buyer_row.postal_code.as_deref(),
        city: buyer_row.city.as_deref(),
        country_code: buyer_row.country_code.as_str(),
        vat_id: buyer_row.vat_id.as_deref(),
        email: buyer_row.email.as_deref(),
    };
    quotes::set_buyer_snapshot_in(&mut tx, &id, &buyer_snapshot).await?;
    // ADR 0042 Nr. 3 (V1-R04): Status in der Schreibtransaktion erneut prüfen;
    // ein Statuseintrag seit der Prüfung oben bricht das Fertigstellen ab.
    if crate::db::repo::tax_status::history_in(&mut tx)
        .await?
        .is_klein_at(input.quote_date)
        != seller_view.is_kleinunternehmer
    {
        return Err(Error::Domain(
            "Der Steuerstatus wurde während des Fertigstellens geändert. Das Angebot ist nicht \
             fertiggestellt; prüfe die Positionen und stelle es erneut fertig."
                .into(),
        ));
    }
    // KB-0010: §19-Status zum Festschreiben einfrieren (Render und Gate nutzen
    // danach genau diesen Wert).
    quotes::issue_in(&mut tx, &id, Some(seller_view.is_kleinunternehmer)).await?;
    audit_log::append_in(
        &mut tx,
        "quote.issue",
        "quote",
        &id,
        Some(serde_json::json!({ "number": detail.quote.quote_number })),
    )
    .await?;
    tx.commit().await?;

    // Backup-Hardline: lock-Event → Auto-Critical-Backup (best-effort).
    backup::request_auto_backup(pool, &paths, session, "quote.issue", ctx.clock().now_utc())
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

    // Lauf F3b: je Angebot nie parallel zu Festschreiben, Zurückziehen,
    // Umwandeln oder Ablehnen (Sperre vor dem Lesen des Status).
    let _quote_guard = crate::locks::QUOTES.lock(&args.quote_id).await;
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
    // R5-S2-QUOTEDATE: Annahmedatum zwischen Angebotsdatum und heute
    // (Europe/Berlin, Uhr des Servers) — vor dem Archivieren eines Vertrags.
    let today = ctx.clock().today_berlin();
    let accepted_date = args.accepted_date.unwrap_or(today);
    quote::validate_accepted_date(parse_date(&quote_row.quote_date)?, accepted_date, today)
        .map_err(Error::Domain)?;
    if let Some(contract) = contract.as_ref() {
        // R2-024 (GJ-Guard): Beilagen werden mit `quote_row.fiscal_year` ins
        // write-once-Archiv abgelegt. Liegt das Angebot in einem festgeschrieben
        // GJ, würden wir nachträglich an dessen Archiv anhängen — also den
        // Lock-Snapshot rückwirkend erweitern. Vor dem Archiv-Schreiben blocken.
        let _fy = guard::ensure_year_open(pool, quote_row.fiscal_year).await?;

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
            Some(serde_json::json!({ "attachment": attachment_id, "archive": stored.archive_id, "file": &file_name })),
        )
        .await?;
    }

    // 2. Status-Übergang sent → accepted.
    let accepted_iso = accepted_date.to_string();
    quotes::accept(pool, &args.quote_id, &accepted_iso).await?;

    audit_log::append(
        pool,
        "quote.accepted",
        "quote",
        &args.quote_id,
        Some(serde_json::json!({ "number": quote_row.quote_number, "accepted_at": accepted_iso, "with_contract": contract.is_some() })),
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
    // Lauf F3b: je Angebot nie parallel zu den übrigen Statuswechseln.
    let _quote_guard = crate::locks::QUOTES.lock(&args.quote_id).await;
    quotes::reject(pool, &args.quote_id).await?;
    audit_log::append(
        pool,
        "quote.rejected",
        "quote",
        &args.quote_id,
        Some(serde_json::json!({ "reason": args.reason.as_deref().unwrap_or("") })),
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

/// Mindestlänge des Grundes beim Zurückziehen (Lauf F2c) — wie beim Verwerfen
/// eines Rechnungs-Entwurfs: Zurückziehen ist endgültig und verwirft einen
/// Angebots-Entwurf (W14-032), der Grund dokumentiert die verbrauchte Nummer.
const CANCEL_REASON_MIN_CHARS: usize = 10;

/// Zieht ein Angebot zurück (endgültig, kein Löschen). Grund Pflicht
/// (mindestens 10 Zeichen); Statuswechsel, Audit `quote.cancel` und das
/// Erledigen einer Restore-Vormerkung laufen in **einer** Transaktion und je
/// Angebot nie parallel zur Umwandlung in eine Rechnung (Lauf F2c).
pub async fn quotes_cancel(ctx: &Arc<AppCtx>, args: QuotesCancelArgs) -> Result<QuoteRow> {
    let QuotesCancelArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let reason: String = args
        .reason
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(500)
        .collect();
    if reason.chars().count() < CANCEL_REASON_MIN_CHARS {
        return Err(Error::BadRequest(
            "Bitte einen Grund angeben (mindestens 10 Zeichen) — er steht am Angebot und im \
             Audit-Log."
                .into(),
        ));
    }
    let _quote_guard = crate::locks::QUOTES.lock(&args.quote_id).await;
    let mut tx = crate::db::begin_immediate(pool).await?;
    let number = quotes::cancel_in(&mut tx, &args.quote_id, &reason).await?;
    let locked: Option<String> = sqlx::query_scalar("SELECT locked_at FROM quotes WHERE id = ?")
        .bind(&args.quote_id)
        .fetch_one(&mut *tx)
        .await?;
    // Ein zurückgezogener Entwurf ist verworfen (W14-032) und wird nie
    // festgeschrieben — eine Restore-Vormerkung (F1d) ist damit erledigt.
    if locked.is_none() {
        sqlx::query("DELETE FROM app_settings WHERE key = ?")
            .bind(format!(
                "{}quote:{}",
                crate::backup::after_restore::CONFLICT_PREFIX,
                args.quote_id
            ))
            .execute(&mut *tx)
            .await?;
    }
    audit_log::append_in(
        &mut tx,
        "quote.cancel",
        "quote",
        &args.quote_id,
        Some(serde_json::json!({
            "number": number,
            "reason": reason,
            "draft": locked.is_none(),
        })),
    )
    .await?;
    tx.commit().await?;
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
/// [`quote::convert_to_invoice`] (pure) → Entwurf vorbereiten
/// ([`crate::commands::invoices::prepare_invoice_draft`]) → in **einer**
/// Transaktion (Lauf F8): Entwurf mit `derived_from_quote_id` + Audit
/// `invoice.draft_create`, Angebot auf `converted` ([`quotes::mark_converted_in`])
/// und Audit `quote.converted`.
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

    // Lauf F2c: nie parallel zum Zurückziehen desselben Angebots — sonst entstünde
    // ein Entwurf zu einem zurückgezogenen Angebot (Status wird unter der Sperre
    // geprüft; `mark_converted` und `trg_quotes_converted_final` sichern in der DB).
    let _quote_guard = crate::locks::QUOTES.lock(&args.quote_id).await;
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
        service_period_start: args.service_period_start,
        service_period_end: args.service_period_end,
        buyer_reference: args.buyer_reference,
        due_date: args.due_date,
        currency_code: quote.currency_code.clone(),
        notes: args.notes.or_else(|| quote.notes.clone()),
        payment_note: args.payment_note,
        pdf_template: args
            .pdf_template
            .unwrap_or_else(|| quote.pdf_template.clone()),
    };
    let mut input = quote::convert_to_invoice(&items, &opts);
    // ADR 0042 Nr. 3: Die Rechnung folgt dem Status an ihrem Leistungs- bzw.
    // Rechnungsdatum, nicht dem des Angebots. Gilt dort §19, weist sie keine
    // Umsatzsteuer aus (§14c-Schutz): Positionen werden steuerfrei („E“, 0 %),
    // der Nettopreis bleibt. Gilt dort die Regelbesteuerung, bleiben „E“-
    // Positionen eines §19-Angebots stehen; die Prüfung des Entwurfs meldet sie,
    // den Steuersatz wählt der Nutzer (Klein.Buch rät keinen Satz).
    if crate::commands::invoices::invoice_tax_status(pool, &input).await? {
        for it in &mut input.items {
            it.tax_category_code = crate::domain::kleinunternehmer::EXEMPT_CATEGORY.into();
            it.tax_rate_percent = 0.0;
        }
    }
    // GJ leitet create_invoice_draft_from_input aus dem Rechnungsdatum ab (W1.2).

    // Gemeinsamer Draft-Helper: Snapshots, Belegnummer, derived_from_quote_id,
    // Audit invoice.draft_create. Lauf F8: Entwurf, Abschluss des Angebots
    // (accepted → converted) und Audit in **einer** Transaktion — ein Abbruch
    // dazwischen lässt kein Angebot auf „angenommen“ mit fertigem Entwurf
    // zurück, eine zweite Umwandlung scheitert an der Status-Bedingung.
    let prepared = crate::commands::invoices::prepare_invoice_draft(
        pool,
        &quote.contact_id,
        &input,
        Some(&quote.id),
        ctx.clock().today_berlin(),
    )
    .await?;
    let mut tx = crate::db::begin_immediate(pool).await?;
    let (invoice_id, invoice_number) =
        crate::commands::invoices::insert_prepared_draft(&mut tx, &prepared).await?;
    quotes::mark_converted_in(&mut tx, &quote.id, &invoice_id).await?;
    audit_log::append_in(
        &mut tx,
        "quote.converted",
        "quote",
        &quote.id,
        Some(serde_json::json!({ "quote_number": quote.quote_number, "invoice": &invoice_id, "invoice_number": &invoice_number })),
    )
    .await?;
    tx.commit().await?;
    drop(prepared);

    crate::db::repo::invoices::get_detail(pool, &invoice_id)
        .await?
        .ok_or_else(|| Error::Domain("Umwandlung: Entwurf nicht gefunden".into()))
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
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    let buyer_row = detail
        .buyer
        .clone()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    // KB-0009/KB-0010: Gerendert wird mit dem beim Festschreiben eingefrorenen
    // §19-Status des Angebots, nicht mit dem heutigen Stand — und genau dieser
    // Status steuert auch den Klausel-Check unten (Hardline „verifiziert vor
    // jedem Render“).
    let seller_view = make_seller_view(&seller_row, quote.is_kleinunternehmer == 1);
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
    if seller_view.is_kleinunternehmer {
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
        Some(serde_json::json!({ "number": &quote.quote_number, "pdf": stored.archive_id })),
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
                "Für das Senden und das Druck-PDF mit AGB müssen AGB und Datenschutzerklärung \
                 hinterlegt sein (Einstellungen → „AGB & Datenschutz“)."
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
/// (mit `pdfArchiveId`). Das Frontend lädt das PDF danach über
/// `GET /files/archive/{pdfArchiveId}`.
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
        Some(serde_json::json!({ "parts": part_count })),
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

/// Verkäufer-Sicht mit dem §19-Status des Angebots (ADR 0042 Nr. 3; nie aus dem
/// Profil).
fn make_seller_view(
    row: &crate::db::models::SellerProfileRow,
    is_kleinunternehmer: bool,
) -> SellerView<'_> {
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
        is_kleinunternehmer,
    }
}

/// §19-Status eines Angebots (ADR 0042 Nr. 3): Status am Angebotsdatum (ein
/// Angebot hat kein Leistungsdatum).
async fn quote_tax_status(pool: &SqlitePool, quote_date: NaiveDate) -> Result<bool> {
    Ok(crate::db::repo::tax_status::history(pool)
        .await?
        .is_klein_at(quote_date))
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
    let s = s.get(..10).unwrap_or(s);
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
