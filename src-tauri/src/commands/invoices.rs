//! Commands für die Invoice-Pipeline (Block 3b).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::invoice::validate_for_issue`])
//! - Counter-Allokation ([`crate::db::numbering::next_number`])
//! - XRechnung-Generierung ([`crate::einvoice::generator::to_xrechnung`])
//! - KoSIT-Validierung ([`crate::einvoice::validator::validate`])
//! - §19-Klausel-Check ([`crate::pdf::klausel_check::verify_for_kleinunternehmer`])
//! - PDF-Rendering ([`crate::pdf::typst_render::render_invoice`])
//! - ZUGFeRD-PDF/A-3-Erzeugung ([`crate::einvoice::mustang_bridge::create_zugferd`])
//! - Write-Once-Archivierung ([`crate::archive::store_bytes`])
//! - DB-Lock + Status-Übergang ([`crate::db::repo::invoices::lock`])
//!
//! ## GoBD-Hardline
//!
//! - Nach `lock_and_issue` ist die Rechnung unveränderlich (DB-Trigger
//!   `trg_invoices_immutable`).
//! - Storno produziert **neue** Rechnung mit `is_storno_for`-Referenz;
//!   das Original bekommt `status='canceled'`, wird aber nicht gelöscht.
//! - Jeder Schritt schreibt einen Audit-Log-Eintrag.
//!
//! ## §19-Hardline
//!
//! - `is_kleinunternehmer = true` → Generator setzt BT-22 + BT-120 mit
//!   wortgleicher Klausel, Klausel-Check erzwingt Template-Konformität.
//! - UI-Sperre für USt-Felder ist Frontend-Verantwortung; Backend
//!   blockt zusätzlich via `validate_for_issue`
//!   (`Paragraph19VatViolation`-Fehler).
//!
//! ## Auto-Backup-Hook
//!
//! Nach erfolgreichem `lock_and_issue` wird ein `auto_critical`-Backup
//! getriggert. Block 4 implementiert die Backup-Schicht; bis dahin ist
//! das ein Trace-Log + Audit-Eintrag (`invoice.lock.backup_pending`).

use crate::app_ctx::AppCtx;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::archive::{self, ArchiveKind};
use crate::backup;
use crate::config::Paths;
use crate::db::models::{InvoiceDetail, InvoiceListItem, InvoiceRow};
use crate::db::numbering;
use crate::db::repo::audit_log;
use crate::db::repo::{contacts, invoices, seller_profile};
use crate::domain::invoice::{
    self, BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use crate::domain::numbering::DocType;
use crate::domain::storno::{
    build_storno_input, OriginalInvoiceView, OriginalItemView, PrecedingInvoice,
};
use crate::einvoice::generator;
use crate::einvoice::mustang_bridge;
use crate::einvoice::types::ValidationStatus;
use crate::einvoice::validator;
use crate::error::{Error, Result};
use crate::pdf::{klausel_check, templates, typst_render};

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDraftArgs {
    pub contact_id: String,
    /// Buyer-Reference (BT-10). Default `"N/A"` für B2C; B2G muss
    /// Leitweg-ID setzen.
    pub buyer_reference: Option<String>,
    pub input: InvoiceInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssueDto {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockResponse {
    pub invoice: InvoiceRow,
    pub pdf_archive_id: String,
    pub xml_archive_id: String,
    pub validation_status: ValidationStatus,
    pub validation_findings_count: u32,
    pub validation_warnings_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelArgs {
    pub invoice_id: String,
    pub reason: String,
    /// Wenn `None`, wird `today` (Europe/Berlin) genutzt.
    pub storno_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelResponse {
    pub original: InvoiceRow,
    pub storno: LockResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordPaymentArgs {
    pub invoice_id: String,
    pub amount_cents: i64,
    pub paid_date: NaiveDate,
    pub note: Option<String>,
    /// Zahlungskonto, auf dem das Geld einging (R2-011; optional, DATEV-Geldkonto).
    #[serde(default)]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePathsDto {
    pub pdf_path: Option<String>,
    pub xml_path: Option<String>,
}

// =============================================================================
// Commands — Read
// =============================================================================

/// Argumente von [`invoices_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesListArgs {
    #[serde(default)]
    pub filter: Option<invoices::ListFilter>,
}

pub async fn invoices_list(
    ctx: &Arc<AppCtx>,
    args: InvoicesListArgs,
) -> Result<Vec<InvoiceListItem>> {
    let InvoicesListArgs { filter } = args;
    let pool = ctx.pool()?;

    let f = filter.unwrap_or_default();
    invoices::list(&pool, &f).await
}

/// Argumente von [`invoices_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesGetArgs {
    pub id: String,
}

pub async fn invoices_get(
    ctx: &Arc<AppCtx>,
    args: InvoicesGetArgs,
) -> Result<Option<InvoiceDetail>> {
    let InvoicesGetArgs { id } = args;
    let pool = ctx.pool()?;

    invoices::get_detail(&pool, &id).await
}

// =============================================================================
// Commands — Draft
// =============================================================================

/// Argumente von [`invoices_create_draft`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesCreateDraftArgs {
    pub args: CreateDraftArgs,
}

pub async fn invoices_create_draft(
    ctx: &Arc<AppCtx>,
    args: InvoicesCreateDraftArgs,
) -> Result<InvoiceDetail> {
    let InvoicesCreateDraftArgs { args } = args;
    let pool = ctx.pool()?;

    // Storno-Belege entstehen nur über `invoices_cancel` (Prüfungen, Paar-Lock,
    // Audit; Gegenprüfung Lauf E).
    if args.input.is_storno_for.is_some() {
        return Err(Error::Domain(
            "Storno-Belege entstehen nur über „Stornieren“ der Original-Rechnung.".into(),
        ));
    }
    let mut input = args.input;
    input.cancel_reason = None;
    create_invoice_draft_from_input(&pool, &args.contact_id, &input, None).await
}

/// Gemeinsame Draft-Erzeugung für Ausgangsrechnungen. Genutzt von
/// [`invoices_create_draft`] (direkte Neuanlage) und von
/// [`crate::commands::quotes::quotes_convert_to_invoice`] (Angebot → Rechnung,
/// Block 7), damit die Belegnummern-, Snapshot- und Audit-Logik nur an einer
/// Stelle lebt.
///
/// Der Draft darf trotz Validierungs-Hinweisen entstehen — die harten
/// §14/§19-Checks erzwingt erst `lock_and_issue`. Geblockt werden hier nur
/// strukturelle Pre-Conditions (keine Positionen, leere Währung, ungültiger
/// Tax-Category-Code, doppelte Position).
///
/// `derived_from_quote_id` verknüpft die Rechnung mit ihrem Ursprungsangebot
/// (Konvertierung); bei direkter Neuanlage und Storno-Belegen ist es `None`.
pub async fn create_invoice_draft_from_input(
    pool: &SqlitePool,
    contact_id: &str,
    input: &InvoiceInput,
    derived_from_quote_id: Option<&str>,
) -> Result<InvoiceDetail> {
    create_invoice_draft_on(
        pool,
        contact_id,
        input,
        derived_from_quote_id,
        today_berlin(),
    )
    .await
}

/// Wie [`create_invoice_draft_from_input`], mit ausdrücklichem „heute“
/// (Europe/Berlin) für die Plausibilitätsprüfung — der Scheduler übergibt das
/// Datum seiner injizierten Uhr (D-20, ADR 0040). Nummer, Entwurf und Audit
/// entstehen in **einer** Transaktion (Lauf F): scheitert ein Schritt, ist
/// keine Nummer verbraucht.
pub async fn create_invoice_draft_on(
    pool: &SqlitePool,
    contact_id: &str,
    input: &InvoiceInput,
    derived_from_quote_id: Option<&str>,
    today: NaiveDate,
) -> Result<InvoiceDetail> {
    let prepared =
        prepare_invoice_draft(pool, contact_id, input, derived_from_quote_id, today).await?;
    let mut tx = crate::db::begin_immediate(pool).await?;
    let (id, _number) = insert_prepared_draft(&mut tx, &prepared).await?;
    tx.commit().await?;
    invoices::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create_draft: detail-load leer".into()))
}

/// Geprüfter, noch nicht gespeicherter Rechnungsentwurf (Ergebnis von
/// [`prepare_invoice_draft`]). Alle Lesezugriffe und Prüfungen sind erledigt;
/// [`insert_prepared_draft`] schreibt ihn in einer laufenden Transaktion.
pub struct PreparedDraft {
    contact_id: String,
    fiscal_year: i64,
    doc_type: DocType,
    input: InvoiceInput,
    buyer_row: crate::db::models::ContactRow,
    seller_row: crate::db::models::SellerProfileRow,
    /// §19-Status am maßgeblichen Datum (ADR 0042 Nr. 3), beim Storno der des
    /// Originals; vorläufig bis zum Festschreiben.
    is_kleinunternehmer: bool,
    totals: invoice::Totals,
    derived_from_quote_id: Option<String>,
    /// Kein Jahresabschluss, bevor der Entwurf geschrieben ist (Lauf F,
    /// R2-S2-GJLOCK).
    _year_permit: crate::fiscal_year::guard::BookingPermit,
}

/// Liest Empfänger und Stammdaten, prüft die strukturellen Vorbedingungen und
/// das Geschäftsjahr — ohne zu schreiben.
pub async fn prepare_invoice_draft(
    pool: &SqlitePool,
    contact_id: &str,
    input: &InvoiceInput,
    derived_from_quote_id: Option<&str>,
    today: NaiveDate,
) -> Result<PreparedDraft> {
    // GJ = Jahr des Rechnungsdatums (Plan W1.2/W9, nie vom Client) — bestimmt
    // den Nummernkreis.
    let fiscal_year = i64::from(crate::domain::fiscal_year::fiscal_year_for(
        &input.invoice_date,
    ));
    // P3: Positionen mit Markdown-Body → `description` (XRechnung-BT-154) aus dem
    // Markup als Klartext neu berechnen, damit das XML auch nach Edits stimmt.
    let mut input = input.clone();
    if input.items.iter().any(item_has_markup) {
        recompute_markup_descriptions(&mut input.items);
    }

    let buyer_row = contacts::get(pool, contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Empfänger nicht gefunden: {contact_id}")))?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;

    if buyer_row.anonymized_at.is_some() {
        return Err(Error::Domain(
            "Der Empfänger ist anonymisiert — an ihn entsteht keine neue Rechnung.".into(),
        ));
    }
    crate::commands::packages::enforce_package_provenance(pool, &mut input.items).await?;
    let is_kleinunternehmer = invoice_tax_status(pool, &input).await?;
    check_draft_blockers(&input, &seller_row, is_kleinunternehmer, &buyer_row, today)?;

    // Kein Entwurf (und keine Nummer) in einem abgeschlossenen Geschäftsjahr
    // (Gegenprüfung Lauf E; wie bei Kosten und Privatbewegungen). Die
    // Erlaubnis reist im Entwurf mit, bis er geschrieben ist (Lauf F).
    let year_permit = crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;

    // Draft "verbraucht" eine Nummer (siehe db::repo::invoices::create_draft Docs).
    let doc_type = if input.is_storno_for.is_some() {
        DocType::StornoInvoice
    } else {
        DocType::Invoice
    };
    let totals = invoice::compute_totals(&input.items);
    Ok(PreparedDraft {
        contact_id: contact_id.to_string(),
        fiscal_year,
        doc_type,
        input,
        buyer_row,
        seller_row,
        is_kleinunternehmer,
        totals,
        derived_from_quote_id: derived_from_quote_id.map(str::to_string),
        _year_permit: year_permit,
    })
}

/// Schreibt einen [`PreparedDraft`] in einer laufenden Transaktion: Nummer,
/// Entwurf (Kopf + Positionen) und Audit `invoice.draft_create`. Liefert
/// `(id, nummer)`.
pub async fn insert_prepared_draft(
    conn: &mut sqlx::SqliteConnection,
    prepared: &PreparedDraft,
) -> Result<(String, String)> {
    let invoice_number =
        numbering::next_number_in(conn, prepared.doc_type, prepared.fiscal_year as i32).await?;
    let seller_row = &prepared.seller_row;
    let buyer_row = &prepared.buyer_row;
    let snapshot = invoices::SellerSnapshot {
        name: seller_row.name.as_str(),
        street: seller_row.street.as_str(),
        postal_code: seller_row.postal_code.as_str(),
        city: seller_row.city.as_str(),
        tax_number: seller_row.tax_number.as_deref(),
        vat_id: seller_row.vat_id.as_deref(),
    };
    let buyer_snapshot = invoices::BuyerSnapshot {
        name: buyer_row.name.as_str(),
        street: buyer_row.street.as_deref(),
        postal_code: buyer_row.postal_code.as_deref(),
        city: buyer_row.city.as_deref(),
        country_code: buyer_row.country_code.as_str(),
        vat_id: buyer_row.vat_id.as_deref(),
        email: buyer_row.email.as_deref(),
    };

    let payload = invoices::DraftCreatePayload {
        contact_id: prepared.contact_id.clone(),
        fiscal_year: prepared.fiscal_year,
        is_kleinunternehmer: prepared.is_kleinunternehmer,
        input: prepared.input.clone(),
        derived_from_quote_id: prepared.derived_from_quote_id.clone(),
    };

    let id = invoices::create_draft_in(
        conn,
        &payload,
        &invoice_number,
        &snapshot,
        &buyer_snapshot,
        &prepared.totals,
    )
    .await?;

    audit_log::append_in(
        conn,
        "invoice.draft_create",
        "invoice",
        &id,
        Some({
            let mut details = serde_json::json!({
                "number": &invoice_number,
                "gross": prepared.totals.gross_amount_cents,
                "items": prepared.input.items.len(),
            });
            if let Some(qid) = prepared.derived_from_quote_id.as_deref() {
                details["derived_from_quote_id"] = serde_json::Value::from(qid);
            }
            details
        }),
    )
    .await?;
    Ok((id, invoice_number))
}

/// Strukturelle Prüfungen, die schon das Speichern eines Entwurfs blocken
/// (Anlegen und Bearbeiten): keine Positionen, leere Währung, Gesamtbetrag
/// ≤ 0, ungültige Steuerart, doppelte Position, Beträge außerhalb der Grenzen
/// (ASVS V5.4.3) und der §14c-Schutz (USt-Ausweis bei §19). Die übrigen
/// §14-Pflichtangaben erzwingt erst das Festschreiben.
fn check_draft_blockers(
    input: &InvoiceInput,
    seller_row: &crate::db::models::SellerProfileRow,
    is_kleinunternehmer: bool,
    buyer_row: &crate::db::models::ContactRow,
    today: NaiveDate,
) -> Result<()> {
    let seller_view = make_seller_view(seller_row, is_kleinunternehmer);
    let buyer_view = make_buyer_view(buyer_row);
    let Err(errs) = invoice::validate_for_issue(input, &seller_view, &buyer_view, today) else {
        return Ok(());
    };
    use invoice::InvoiceValidationError as E;
    let blockers: Vec<_> = errs
        .iter()
        .filter(|e| {
            matches!(
                e,
                E::NoItems
                    | E::CurrencyEmpty
                    | E::TotalNotPositive
                    | E::ItemInvalidTaxCategoryCode { .. }
                    | E::ItemDuplicatePosition(_)
                    | E::ItemAmountOutOfRange(_)
                    | E::TotalOutOfRange
                    | E::Paragraph19VatViolation(_)
            )
        })
        .collect();
    if blockers.is_empty() {
        return Ok(());
    }
    Err(Error::Domain(format!(
        "Der Entwurf kann nicht gespeichert werden: {}",
        blockers
            .iter()
            .map(|e| invoice::message(e))
            .collect::<Vec<_>>()
            .join("; ")
    )))
}

/// Gründe einer Prüf-Ablehnung (Ausstellen/Fertigstellen), „; “ getrennt: die
/// ersten fünf, darüber hinaus „und N weitere Gründe“ (U4, V1-03).
pub(crate) fn issue_reasons<E>(errs: &[E], message: impl Fn(&E) -> String) -> String {
    let mut parts: Vec<String> = errs.iter().take(5).map(message).collect();
    if errs.len() > 5 {
        parts.push(format!(
            "… und {} weitere Gründe (siehe „Prüfen“).",
            errs.len() - 5
        ));
    }
    parts.join("; ")
}

/// Argumente von [`invoices_update_draft`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesUpdateDraftArgs {
    pub id: String,
    pub args: CreateDraftArgs,
}

/// Bearbeitet einen gespeicherten Rechnungs-Entwurf (Neuer Fund F2a, Lauf F5).
/// Dieselben Prüfungen wie beim Anlegen; die Nummer bleibt.
pub async fn invoices_update_draft(
    ctx: &Arc<AppCtx>,
    args: InvoicesUpdateDraftArgs,
) -> Result<InvoiceDetail> {
    let InvoicesUpdateDraftArgs { id, args } = args;
    let pool = ctx.pool()?;
    update_draft_core(
        &pool,
        &id,
        &args.contact_id,
        args.input,
        ctx.clock().today_berlin(),
    )
    .await
}

/// Testbarer Kern von [`invoices_update_draft`] (`today` = Europe/Berlin aus der
/// Uhr des Aufrufers). Prüft: nur ein offener Entwurf (Status `draft`, nicht
/// festgeschrieben, nicht verworfen), kein Storno-Entwurf, kein Entwurf eines
/// laufenden Abo-Laufs; das Rechnungsdatum bleibt im Geschäftsjahr der Nummer,
/// das Geschäftsjahr ist offen; der Empfänger existiert und ist nicht
/// anonymisiert (beim Entwurf aus einem Angebot bleibt er der des Angebots);
/// Strukturprüfungen und §14c-Schutz wie beim Anlegen; Paket-Provenienz.
/// Schreibt Entwurf, Positionen und Audit `invoice.draft_updated` in **einer**
/// Transaktion, nie parallel zum Festschreiben, Stornieren oder Verwerfen
/// desselben Belegs.
pub async fn update_draft_core(
    pool: &SqlitePool,
    id: &str,
    contact_id: &str,
    input: InvoiceInput,
    today: NaiveDate,
) -> Result<InvoiceDetail> {
    if input.is_storno_for.is_some() {
        return Err(Error::Domain(
            "Storno-Belege entstehen nur über „Stornieren“ der Original-Rechnung.".into(),
        ));
    }
    if input.direction != InvoiceDirection::Issued {
        return Err(Error::Domain(
            "Nur Ausgangsrechnungen lassen sich als Entwurf bearbeiten.".into(),
        ));
    }
    let mut input = input;
    input.cancel_reason = None;
    if input.items.iter().any(item_has_markup) {
        recompute_markup_descriptions(&mut input.items);
    }

    let _invoice_guard = crate::locks::INVOICES.lock(id).await;
    let current = invoices::get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {id}")))?;
    if current.locked_at.is_some() || current.status != "draft" {
        return Err(Error::Domain(format!(
            "Die Rechnung {} ist festgeschrieben und lässt sich nicht mehr ändern (GoBD). \
             Korrigieren lässt sie sich nur mit einem Storno.",
            current.invoice_number
        )));
    }
    if current.voided_at.is_some() {
        return Err(Error::Domain(format!(
            "Der Entwurf {} ist verworfen und lässt sich nicht mehr ändern.",
            current.invoice_number
        )));
    }
    if current.is_storno_for.is_some() {
        return Err(Error::Domain(
            "Ein Storno-Entwurf lässt sich nicht bearbeiten; er spiegelt die Original-Rechnung."
                .into(),
        ));
    }
    if current.direction != "issued" {
        return Err(Error::Domain(
            "Nur Ausgangsrechnungen lassen sich als Entwurf bearbeiten.".into(),
        ));
    }
    let fiscal_year = i64::from(crate::domain::fiscal_year::fiscal_year_for(
        &input.invoice_date,
    ));
    if fiscal_year != current.fiscal_year {
        return Err(Error::Domain(format!(
            "Das Rechnungsdatum muss im Geschäftsjahr {} bleiben — die Nummer {} gehört zu \
             dessen Nummernkreis. Für ein anderes Jahr den Entwurf verwerfen und neu anlegen.",
            current.fiscal_year, current.invoice_number
        )));
    }
    if current.derived_from_quote_id.is_some() && current.contact_id != contact_id {
        return Err(Error::Domain(
            "Der Entwurf stammt aus einem Angebot; der Empfänger bleibt der des Angebots.".into(),
        ));
    }
    // Ein Abo-Lauf, der diesen Entwurf schon angelegt hat, schreibt ihn beim
    // nächsten Durchlauf fest — kein Ändern dazwischen (verwerfen geht).
    let pending_prefix = crate::scheduler::recurring_invoice::PENDING_PREFIX;
    let pending: Option<String> = sqlx::query_scalar(
        "SELECT key FROM app_settings
          WHERE substr(key, 1, length(?1)) = ?1
            AND substr(value, 1, length(?2) + 1) = ?2 || '|'
          LIMIT 1",
    )
    .bind(pending_prefix)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    if pending.is_some() {
        return Err(Error::Domain(
            "Der Entwurf gehört zu einem laufenden Abo-Lauf und wird beim nächsten Durchlauf \
             festgeschrieben; ändern lässt er sich nicht (verwerfen geht)."
                .into(),
        ));
    }

    let buyer_row = contacts::get(pool, contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Empfänger nicht gefunden: {contact_id}")))?;
    if buyer_row.anonymized_at.is_some() {
        return Err(Error::Domain(
            "Der Empfänger ist anonymisiert — an ihn entsteht keine neue Rechnung.".into(),
        ));
    }
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    crate::commands::packages::enforce_package_provenance(pool, &mut input.items).await?;
    // ADR 0042 Nr. 3: Status am Leistungs- bzw. Rechnungsdatum des Entwurfs.
    let is_kleinunternehmer = invoice_tax_status(pool, &input).await?;
    check_draft_blockers(&input, &seller_row, is_kleinunternehmer, &buyer_row, today)?;
    // Kein Jahresabschluss, bevor der Entwurf geschrieben ist.
    let _year_permit = crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;

    let totals = invoice::compute_totals(&input.items);
    let seller = invoices::SellerSnapshot {
        name: seller_row.name.as_str(),
        street: seller_row.street.as_str(),
        postal_code: seller_row.postal_code.as_str(),
        city: seller_row.city.as_str(),
        tax_number: seller_row.tax_number.as_deref(),
        vat_id: seller_row.vat_id.as_deref(),
    };
    let buyer = OwnedBuyer::from_contact(&buyer_row);
    let buyer_snapshot = buyer.snapshot();
    let mut tx = crate::db::begin_immediate(pool).await?;
    invoices::update_draft_in(
        &mut tx,
        id,
        &invoices::DraftUpdate {
            contact_id,
            input: &input,
            is_kleinunternehmer,
            seller: &seller,
            buyer: &buyer_snapshot,
            totals: &totals,
        },
    )
    .await?;
    audit_log::append_in(
        &mut tx,
        "invoice.draft_updated",
        "invoice",
        id,
        Some(serde_json::json!({
            "number": current.invoice_number,
            "gross_before": current.gross_amount_cents,
            "gross": totals.gross_amount_cents,
            "items": input.items.len(),
            "contact_changed": current.contact_id != contact_id,
            "invoice_date": input.invoice_date,
        })),
    )
    .await?;
    // Lauf F8: Ein nach dem Restore bestätigter Entwurf (Nummer war schon
    // festgeschrieben) gilt nach jeder Änderung wieder als vorgemerkt — die
    // Bestätigung galt dem Stand, der dem versandten Original entsprach.
    crate::backup::after_restore::reflag_confirmed_in(
        &mut tx,
        "invoice",
        id,
        &current.invoice_number,
    )
    .await?;
    tx.commit().await?;
    invoices::get_detail(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("update_draft: detail-load leer".into()))
}

/// Argumente von [`invoices_validate_draft`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesValidateDraftArgs {
    pub id: String,
}

pub async fn invoices_validate_draft(
    ctx: &Arc<AppCtx>,
    args: InvoicesValidateDraftArgs,
) -> Result<Vec<ValidationIssueDto>> {
    let InvoicesValidateDraftArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let detail = invoices::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {id}")))?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    let buyer_row = detail
        .buyer
        .as_ref()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let input = detail_to_input(&detail)?;
    // Wie beim Festschreiben (KB-0010, ADR 0042 Nr. 3): Status am Leistungs- bzw.
    // Rechnungsdatum; ein Storno prüft unter dem §19-Status des Originals.
    let seller_view = make_seller_view(&seller_row, invoice_tax_status(pool, &input).await?);
    let buyer_view = make_buyer_view(buyer_row);
    let today = today_berlin();

    let issues = match invoice::validate_for_issue(&input, &seller_view, &buyer_view, today) {
        Ok(()) => Vec::new(),
        Err(errs) => errs
            .into_iter()
            .map(|e| ValidationIssueDto {
                code: variant_name(&e).to_string(),
                message: invoice::message(&e),
            })
            .collect(),
    };
    Ok(issues)
}

// =============================================================================
// Commands — Lock & Issue
// =============================================================================

/// Argumente von [`invoices_lock_and_issue`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesLockAndIssueArgs {
    pub id: String,
    #[serde(default)]
    pub buyer_reference: Option<String>,
}

pub async fn invoices_lock_and_issue(
    ctx: &Arc<AppCtx>,
    args: InvoicesLockAndIssueArgs,
) -> Result<LockResponse> {
    let InvoicesLockAndIssueArgs {
        id,
        buyer_reference,
    } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    // GJ-Festschreibung (Block 15): eine Rechnung mit Datum in einem
    // abgeschlossenen Geschäftsjahr kann nicht (mehr) ausgestellt werden.
    let draft = invoices::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {id}")))?;
    // Ein liegengebliebener Storno-Entwurf (gescheiterte Pipeline) darf nicht ohne
    // das Original festgeschrieben werden — sonst bliebe das Original offen und ein
    // zweiter Storno zählte die Erstattung doppelt. „Stornieren“ verwendet den
    // Entwurf wieder (Gegenprüfung Lauf E).
    if draft.invoice.is_storno_for.is_some() {
        return Err(Error::Domain(
            "Dieser Storno-Beleg wird über „Stornieren“ der Original-Rechnung abgeschlossen."
                .into(),
        ));
    }
    let _fy = crate::fiscal_year::guard::ensure_year_open(pool, draft.invoice.fiscal_year).await?;
    let resp = run_lock_pipeline(
        pool,
        &paths,
        &id,
        buyer_reference.as_deref().unwrap_or("N/A"),
    )
    .await?;
    // Block 4 — Auto-Critical-Backup nach erfolgreichem Lock. Best-effort:
    // ein nicht erreichbares Backup-Ziel darf die festgeschriebene Rechnung
    // nicht zurückrollen.
    backup::request_auto_backup(pool, &paths, session, "invoice.lock", ctx.clock().now_utc())
        .await
        .ok();
    Ok(resp)
}

/// Storno-Paar-Modus für [`run_lock_pipeline`]: wenn gesetzt, markiert die
/// Pipeline beim Lock des Storno-Belegs gleichzeitig die Original-Rechnung
/// als `canceled` — beide UPDATEs atomar in einer Transaktion. Schließt die
/// GoBD-Lücke aus R1-003 (v2026.5-Re-Review): stromaus zwischen
/// `lock(storno)` und `mark_canceled(original)` ließ einen
/// inkonsistenten Zustand zurück (storno gelockt + original uncanceled
/// = EÜR-Doppel-Refund).
#[derive(Debug, Clone, Copy)]
pub struct LockCancelPair<'a> {
    pub original_id: &'a str,
    pub reason: Option<&'a str>,
}

/// Core-Pipeline für `lock_and_issue`. Auch von `invoices_cancel` für
/// den Storno-Beleg verwendet.
///
/// Public, damit Integration-Tests sie ohne Server ausführen
/// können (Tests bauen Paths direkt aus tempdir + Mock-Sidecar).
pub async fn run_lock_pipeline(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
    buyer_reference: &str,
) -> Result<LockResponse> {
    run_lock_pipeline_inner(
        pool,
        paths,
        invoice_id,
        buyer_reference,
        None,
        today_berlin(),
    )
    .await
}

/// Wie [`run_lock_pipeline`], mit ausdrücklichem „heute“ (Europe/Berlin) für
/// die §14-Prüfung — der Scheduler übergibt das Datum seiner injizierten Uhr
/// (D-20, ADR 0040).
pub async fn run_lock_pipeline_on(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
    buyer_reference: &str,
    today: NaiveDate,
) -> Result<LockResponse> {
    run_lock_pipeline_inner(pool, paths, invoice_id, buyer_reference, None, today).await
}

/// Wie [`run_lock_pipeline`], aber mit optionalem Storno-Paar-Cancel im
/// SELBEN DB-TX wie der Lock-UPDATE. Siehe [`LockCancelPair`].
pub async fn run_lock_pipeline_with_pair(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
    buyer_reference: &str,
    pair: LockCancelPair<'_>,
) -> Result<LockResponse> {
    run_lock_pipeline_inner(
        pool,
        paths,
        invoice_id,
        buyer_reference,
        Some(pair),
        today_berlin(),
    )
    .await
}

async fn run_lock_pipeline_inner(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
    buyer_reference: &str,
    cancel_pair: Option<LockCancelPair<'_>>,
    today: NaiveDate,
) -> Result<LockResponse> {
    // Review W2 S0-1: Festschreiben derselben Rechnung läuft nie doppelt
    // (Doppelklick, zwei Browser-Tabs); der zweite Lauf sieht danach `locked_at`.
    let _invoice_guard = crate::locks::INVOICES.lock(invoice_id).await;
    let detail = invoices::get_detail(pool, invoice_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {invoice_id}")))?;

    if detail.invoice.locked_at.is_some() {
        return Err(Error::Domain(format!(
            "invoice {invoice_id} ist bereits gelockt"
        )));
    }
    // W14-032: Ein verworfener Entwurf wird nie festgeschrieben (DB-Backstop
    // `trg_invoices_voided_final`).
    if detail.invoice.voided_at.is_some() {
        return Err(Error::Domain(format!(
            "Der Entwurf {} ist verworfen und lässt sich nicht mehr festschreiben.",
            detail.invoice.invoice_number
        )));
    }
    // Lauf F (R2-S2-GJLOCK): Geschäftsjahr offen — und bis zum Festschreiben
    // beginnt kein Jahresabschluss (gilt auch für Abo-Läufe und Storno).
    let _fy = crate::fiscal_year::guard::ensure_year_open(pool, detail.invoice.fiscal_year).await?;
    // F1d: Ein wiederhergestellter Entwurf, dessen Nummer vor dem Restore schon
    // festgeschrieben war, wird erst nach der Bestätigung des Inhabers
    // festgeschrieben (nie zwei verschiedene Belege mit derselben Nummer).
    crate::backup::after_restore::ensure_no_conflict(pool, "invoice", invoice_id).await?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    let buyer_row = detail
        .buyer
        .clone()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;

    // BT-41 (Ansprechpartner) = gepflegter Inhaber-Name, sonst der Verkäufer-Name.
    let owner_name = crate::db::repo::app_settings::get(pool, "seller_owner_name")
        .await?
        .filter(|s| !s.trim().is_empty());
    // §19-Status folgt unten (KB-0010, ADR 0042 Nr. 3).
    let mut seller_view = make_seller_view(&seller_row, true);
    seller_view.contact_name = owner_name.as_deref();

    // Block 19: Generator + PDF aus dem Buyer-SNAPSHOT (nicht dem Live-Kontakt),
    // damit eine spätere DSGVO-Anonymisierung des Kontakts den festgeschriebenen
    // Beleg nicht verändert.
    // - Normale Rechnung: Snapshot bei Lock aus dem aktuellen Live-Kontakt
    //   auffrischen (Manuel: "Refresh bei Lock") und persistieren → der Beleg
    //   friert den Empfänger-Stand zur Ausstellung ein.
    // - Storno: den vom Original übernommenen Snapshot BEIBEHALTEN (er spiegelt
    //   den Empfänger-Stand der Original-Rechnung; NICHT vom Live-Kontakt
    //   überschreiben — sonst zöge eine Anonymisierung zwischen Original und
    //   Storno in den Storno-Beleg ein).
    let effective_buyer = if detail.invoice.is_storno_for.is_some() {
        OwnedBuyer::from_invoice_snapshot(&detail.invoice)
            .unwrap_or_else(|| OwnedBuyer::from_contact(&buyer_row))
    } else {
        let snap = OwnedBuyer::from_contact(&buyer_row);
        invoices::set_buyer_snapshot(pool, invoice_id, &snap.snapshot()).await?;
        snap
    };
    let buyer_view = effective_buyer.view();
    let input = detail_to_input(&detail)?;
    // Storno: Bezug auf die stornierte Rechnung über deren Nummer und Datum
    // (BT-25/BT-26, PDF-Zeile), nicht über die interne ID (Lauf E).
    let original = match detail.invoice.is_storno_for.as_deref() {
        Some(original_id) => Some(
            invoices::get(pool, original_id)
                .await?
                .ok_or_else(|| Error::Domain("Storno: Original-Rechnung fehlt".into()))?,
        ),
        None => None,
    };
    let preceding = match original.as_ref() {
        Some(original) => Some(PrecedingInvoice {
            number: original.invoice_number.clone(),
            issue_date: parse_date(&original.invoice_date)?,
        }),
        None => None,
    };
    // KB-0010, ADR 0042 Nr. 3: §19-Status, unter dem der Beleg entsteht. Eine
    // Rechnung folgt der Statushistorie an ihrem Leistungsdatum, ersatzweise am
    // Rechnungsdatum; ein Storno spiegelt das Original (§14c: es hebt genau
    // dessen Steuerausweis auf — auch nach einem Wechsel des Status). Der Wert
    // steuert Prüfung, XML, Klausel-Check und PDF und wird beim Festschreiben am
    // Beleg eingefroren. Das Datum geht mit in den Lock, der den Status in seiner
    // Schreibtransaktion gegen die Historie prüft (V1-R04).
    let status_day =
        crate::domain::tax_status::document_date(input.delivery_date, input.invoice_date);
    seller_view.is_kleinunternehmer = match original.as_ref() {
        Some(original) => original.is_kleinunternehmer == 1,
        None => crate::db::repo::tax_status::history(pool)
            .await?
            .is_klein_at(status_day),
    };
    let tax_status_date = original.is_none().then_some(status_day);

    // Summen des Belegs aus den Positionen — dieselbe Rechnung wie Generator
    // (BT-109/110/112) und PDF; beim Festschreiben in den Datensatz übernommen.
    let issued_totals = invoice::compute_totals(&input.items);

    // 1. Domain-Validation — Hard-Block bei §14/§19-Fehlern. U4 (V1-03): deutscher
    // Vorsatz, Gründe mit „; “ getrennt (die Detailseite zeigt sie als Liste).
    invoice::validate_for_issue(&input, &seller_view, &buyer_view, today).map_err(|errs| {
        let what = if original.is_some() {
            "Der Storno-Beleg"
        } else {
            "Die Rechnung"
        };
        Error::Domain(format!(
            "{what} kann nicht ausgestellt werden: {}",
            issue_reasons(&errs, invoice::message)
        ))
    })?;

    // Konten für Belege: Bankkonten fürs XML (BT-84), alle geflaggten fürs PDF.
    // Inhaber-Name (Einzelunternehmer §14) dient als Kontoinhaber.
    let owner_name = crate::db::repo::app_settings::get(pool, "seller_owner_name")
        .await?
        .filter(|s| !s.trim().is_empty());
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
    let bank_accounts: Vec<(&str, Option<&str>)> = acct_rows
        .iter()
        .filter(|a| a.account_type == "bank")
        .filter_map(|a| a.iban.as_deref().map(|iban| (iban, a.bic.as_deref())))
        .collect();

    // Leistungsdatum-Fallback (§14 Abs. 4 Nr. 6 UStG): fehlt das Datum, gilt das
    // Rechnungsdatum als Leistungszeitpunkt — fürs XML (BT-72) als konkretes Datum,
    // fürs PDF als Hinweis „entspricht dem Rechnungsdatum". Seit dem Pflichtfeld
    // (UI) greift das nur noch als Backstop (Alt-Entwürfe/API). Immer aktiv,
    // keine Einstellung mehr (Manuel 2026-05-23).
    let service_date_fallback = true;
    let xml_input = InvoiceInput {
        delivery_date: effective_delivery_date(
            input.delivery_date,
            input.invoice_date,
            service_date_fallback,
        ),
        ..input.clone()
    };

    // 2. XRechnung-XML erzeugen.
    let xml = generator::to_xrechnung_with_preceding(
        &detail.invoice.invoice_number,
        &xml_input,
        &seller_view,
        &buyer_view,
        buyer_reference,
        &bank_accounts,
        preceding.as_ref(),
    )
    .map_err(|e| Error::Domain(format!("Generator: {e}")))?;

    // 3. KoSIT-Validator — muss `Passed` sein.
    let report = validator::validate(&xml, &paths.sidecar_dir).await?;
    if matches!(report.status, ValidationStatus::Failed) {
        return Err(Error::Domain(format!(
            "KoSIT-Validator: {} Fehler, {} Warnungen. Erste Findings: {}",
            report.error_count,
            report.warning_count,
            report
                .findings
                .iter()
                .take(3)
                .map(|f| format!("{}: {}", f.rule_id.as_deref().unwrap_or("?"), f.message))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    // 4. Template laden und §19-Klausel-Check (nur wenn kleinunternehmer).
    // Sentinel 'default' = "globalem Default aus den Stammdaten folgen" (Block 17a).
    // Greift auch für Storno (läuft durch dieselbe Pipeline). Das gerenderte PDF
    // wird archiviert (das ist der GoBD-Snapshot); der Beleg-Wert bleibt 'default'.
    let template_name = if input.pdf_template == "default" {
        seller_row.default_pdf_template.as_str()
    } else {
        input.pdf_template.as_str()
    };
    let mut template_source =
        templates::resolve_invoice_template(&paths.inputs_dir, template_name)?;
    // LE-CUSTOM-TEMPLATE-STORNO: Eine eigene Vorlage ohne Storno-Darstellung
    // (`storno_of`) zeigte einen Storno als „Rechnung“ mit Überweisungshinweis.
    // Für Stornos dann die eingebaute Vorlage nehmen und einen Hinweis in die
    // Inbox legen (kein falsches Belegbild, kein Abbruch des Stornos).
    // Der Inbox-Hinweis entsteht erst nach dem erfolgreichen Festschreiben
    // (F5-fix): ein später gescheiterter Storno hinterlässt keinen Hinweis und
    // verbraucht die Dedup der Vorlage nicht.
    let storno_template_fallback =
        preceding.is_some() && !klausel_check::supports_storno(&template_source);
    if storno_template_fallback {
        let fallback = templates::storno_fallback_template(template_name);
        template_source = fallback.to_string();
    }
    if seller_view.is_kleinunternehmer {
        klausel_check::verify_for_kleinunternehmer(&template_source)
            .map_err(|e| Error::Domain(format!("§19-Klausel-Check: {e}")))?;
    }

    // 5. Typst-Render: Plain-PDF.
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };
    // Firmen-Logo (data/branding/logo.*) für den PDF-Kopf, falls hochgeladen.
    let logo = crate::branding::find_logo(&crate::branding::branding_dir(&paths.data_dir));
    let logo_vpath = logo.as_ref().map(|(name, _)| format!("/branding/{name}"));
    let logo_opt = match (logo_vpath.as_deref(), logo.as_ref()) {
        (Some(vp), Some((_, bytes))) => Some((vp, bytes.as_slice())),
        _ => None,
    };
    let pdf_bytes = typst_render::invoice_job_with_storno(
        &template_source,
        &detail.invoice.invoice_number,
        &input,
        &seller_view,
        &buyer_view,
        branding_opt,
        logo_opt,
        owner_name.as_deref(),
        &accounts_json,
        service_date_fallback,
        preceding.as_ref(),
    )?
    .compile_async()
    .await?;

    // 6. Mustang: ZUGFeRD-PDF/A-3 mit eingebettetem XML.
    let zugferd_bytes =
        mustang_bridge::create_zugferd(&pdf_bytes, &xml, &paths.sidecar_dir).await?;

    // 7. Archivierung — PDF + XML in `archive_entries`. Bei einer Wiederholung
    //    nach Abbruch (Datei schon registriert, Rechnung nicht festgeschrieben)
    //    weicht der Name auf eine neue Version aus (Review W2 S1-1).
    let fy = detail.invoice.fiscal_year as i32;
    let pdf_archive = archive::store_bytes_new_version(
        pool,
        &paths.archive_dir,
        fy,
        ArchiveKind::InvoicePdf,
        &format!("{}.pdf", detail.invoice.invoice_number),
        "application/pdf",
        &zugferd_bytes,
    )
    .await?;
    let xml_archive = archive::store_bytes_new_version(
        pool,
        &paths.archive_dir,
        fy,
        ArchiveKind::InvoiceXml,
        &format!("{}.xml", detail.invoice.invoice_number),
        "application/xml",
        xml.as_bytes(),
    )
    .await?;

    // 8. DB-Lock — bei Storno-Paar im selben TX inkl. Original-Cancel (R1-003).
    let validation_status_str = match report.status {
        ValidationStatus::Passed => "passed",
        ValidationStatus::Warning => "warning",
        ValidationStatus::Failed => "failed",
    };
    let lock_update = invoices::LockUpdate {
        validation_status: validation_status_str,
        validation_report: Some(&report.raw_xml),
        pdf_archive_id: &pdf_archive.archive_id,
        xml_archive_id: &xml_archive.archive_id,
        is_kleinunternehmer: Some(seller_view.is_kleinunternehmer),
        tax_status_date,
        // F5-fix: Summen wie im XML/PDF (USt je Steuergruppe), nicht die beim
        // Speichern des Entwurfs abgelegten (ältere Summenlogik, ältere Backups).
        totals: Some(invoices::LockTotals {
            net_amount_cents: issued_totals.net_amount_cents,
            tax_amount_cents: issued_totals.tax_amount_cents,
            gross_amount_cents: issued_totals.gross_amount_cents,
        }),
    };
    let pair_repo = cancel_pair.map(|p| invoices::PairCancel {
        original_id: p.original_id,
        reason: p.reason,
    });
    invoices::lock_with_pair_cancel(pool, invoice_id, &lock_update, pair_repo).await?;
    if storno_template_fallback {
        tracing::warn!(
            "Vorlage {template_name:?} kann keine Stornos darstellen — Storno {} mit der \
             eingebauten Vorlage gerendert",
            detail.invoice.invoice_number
        );
        notify_storno_template_fallback(pool, template_name, &detail.invoice.invoice_number).await;
    }

    // 9. Audit-Log.
    audit_log::append(
        pool,
        "invoice.lock",
        "invoice",
        invoice_id,
        Some(serde_json::json!({ "number": detail.invoice.invoice_number, "pdf": pdf_archive.archive_id, "xml": xml_archive.archive_id, "kosit": validation_status_str, "warnings": report.warning_count })),
    )
    .await?;

    // 10. Auto-Backup-Marker (GoBD-Trace innerhalb der Pipeline).
    // Das eigentliche Auto-Critical-Backup wird vom Command-Wrapper
    // (`invoices_lock_and_issue` / `invoices_cancel`) via
    // `backup::request_auto_backup` angefordert — nur dort ist die
    // Session-Passphrase (`AppCtx::session`) verfügbar. `run_lock_pipeline` selbst
    // bleibt I/O-frei bzgl. Backup, damit Integration-Tests sie ohne
    // entsperrte Session ausführen können.
    audit_log::append(
        pool,
        "invoice.lock.backup_pending",
        "invoice",
        invoice_id,
        Some(serde_json::json!({"hook":"backup::request_auto_backup(auto_critical)","module":"block-4"})),
    )
    .await
    .ok();

    let locked_invoice = invoices::get(pool, invoice_id)
        .await?
        .ok_or_else(|| Error::Domain("lock: post-UPDATE SELECT leer".into()))?;

    Ok(LockResponse {
        invoice: locked_invoice,
        pdf_archive_id: pdf_archive.archive_id,
        xml_archive_id: xml_archive.archive_id,
        validation_status: report.status.clone(),
        validation_findings_count: report.error_count,
        validation_warnings_count: report.warning_count,
    })
}

// =============================================================================
// Commands — Payment
// =============================================================================

/// Argumente von [`invoices_record_payment`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesRecordPaymentArgs {
    pub args: RecordPaymentArgs,
}

pub async fn invoices_record_payment(
    ctx: &Arc<AppCtx>,
    args: InvoicesRecordPaymentArgs,
) -> Result<InvoiceRow> {
    let InvoicesRecordPaymentArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    // R5-002: Future-Datum verbieten (Symmetrie zu `expenses_set_payment`).
    // Cash-Basis-EÜR rechnet jede Einnahme im paid_at-Jahr (§11 EStG Zufluss-
    // Prinzip) — ein versehentliches Zukunftsdatum verschiebt die Einnahme ins
    // falsche Steuerjahr und macht den späteren Jahresabschluss inkonsistent.
    if args.paid_date > today_berlin() {
        return Err(Error::Domain(
            "Das Zahldatum darf nicht in der Zukunft liegen (Zufluss-Prinzip §11 EStG).".into(),
        ));
    }
    // GJ-Festschreibung (Block 15): eine Zahlung mit Zahldatum in einem
    // abgeschlossenen Geschäftsjahr ist nicht mehr erfassbar (Storno bleibt möglich).
    let _fy = crate::fiscal_year::guard::ensure_year_open(
        pool,
        chrono::Datelike::year(&args.paid_date) as i64,
    )
    .await?;
    let row = invoices::record_payment_to_account(
        pool,
        &args.invoice_id,
        args.amount_cents,
        &args.paid_date.to_string(),
        args.note.as_deref(),
        args.account_id.as_deref(),
    )
    .await?;
    audit_log::append(
        pool,
        "invoice.payment_recorded",
        "invoice",
        &args.invoice_id,
        Some(serde_json::json!({ "amount": args.amount_cents, "paid_date": args.paid_date, "status": row.status, "account": args.account_id })),
    )
    .await?;
    Ok(row)
}

// =============================================================================
// Commands — Cancel (Storno)
// =============================================================================

/// Argumente von [`invoices_cancel`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesCancelArgs {
    pub args: CancelArgs,
}

pub async fn invoices_cancel(
    ctx: &Arc<AppCtx>,
    args: InvoicesCancelArgs,
) -> Result<CancelResponse> {
    let InvoicesCancelArgs { args } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    // Zwei Storno-Anfragen für dieselbe Rechnung laufen nacheinander; die zweite
    // sieht dann „bereits storniert“ (Review W2, Sperre je Beleg).
    let _original_guard = crate::locks::INVOICES.lock(&args.invoice_id).await;

    let original = invoices::get_detail(pool, &args.invoice_id)
        .await?
        .ok_or_else(|| {
            Error::Domain(format!(
                "Original-Rechnung nicht gefunden: {}",
                args.invoice_id
            ))
        })?;

    if original.invoice.locked_at.is_none() {
        return Err(Error::Domain(
            "Storno nur für festgeschriebene Rechnungen möglich. Einen Entwurf bitte verwerfen."
                .into(),
        ));
    }
    if original.invoice.status == "canceled" {
        return Err(Error::Domain(
            "Original-Rechnung ist bereits storniert.".into(),
        ));
    }
    if original.invoice.is_storno_for.is_some() {
        return Err(Error::Domain(
            "Storno einer Storno-Rechnung ist nicht zulässig (Buchhaltung).".into(),
        ));
    }
    // Lauf F2c: Altbestand aus der Desktop-Zeit vor R1-003 — zur Rechnung gibt
    // es schon einen festgeschriebenen Storno, das Original steht aber nicht
    // auf 'canceled' (Absturz zwischen beiden Schritten). Kein zweiter Storno
    // (der Index `ux_invoices_one_locked_storno` ließe ihn nicht zu, PDF und XML
    // lägen verwaist im Archiv): das Paar wird vervollständigt.
    if let Some(storno) = invoices::locked_storno_for(pool, &original.invoice.id).await? {
        return complete_storno_pair(pool, &original.invoice, storno, &args.reason).await;
    }

    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain(crate::domain::invoice::SELLER_PROFILE_MISSING.into()))?;
    let storno_date = args.storno_date.unwrap_or_else(today_berlin);
    // W14 (GoBD): Das Storno wirkt in der EÜR im Jahr seines Datums. Nicht vor
    // der Original-Rechnung, nicht in der Zukunft und nur in einem offenen
    // Geschäftsjahr — sonst änderte sich ein festgeschriebenes Jahr nachträglich
    // (wie beim Kosten-Storno, R2-003, und bei Zahlungen).
    let original_date = parse_date(&original.invoice.invoice_date)?;
    if storno_date < original_date || storno_date > today_berlin() {
        return Err(Error::Domain(format!(
            "Das Storno-Datum muss zwischen dem Rechnungsdatum ({}) und heute liegen.",
            original_date.format("%d.%m.%Y")
        )));
    }
    // Lauf F8 (§ 11 EStG): Die EÜR setzt die Erstattung des gezahlten Betrags
    // am Storno-Datum an. Vor dem (letzten) Zahlungseingang läge sie in einem
    // Zeitraum, in dem das Geld noch gar nicht zugeflossen war.
    if original.invoice.paid_amount_cents > 0 {
        if let Some(last_paid) = last_payment_date(&original.invoice)? {
            if storno_date < last_paid {
                return Err(Error::Domain(format!(
                    "Das Storno-Datum liegt vor dem letzten Zahlungseingang ({0}). \
                     Erstatten lässt sich erst, was zugeflossen ist (§ 11 EStG): \
                     bitte ein Storno-Datum ab dem {0} wählen.",
                    last_paid.format("%d.%m.%Y")
                )));
            }
        }
    }
    let _fy = crate::fiscal_year::guard::ensure_year_open(
        pool,
        chrono::Datelike::year(&storno_date) as i64,
    )
    .await?;
    // U7 (R1-01): Der Storno-Beleg trägt dieselbe Käuferreferenz (BT-10) wie das
    // Original, bei Behörden die Leitweg-ID; ohne sie ordnet die Eingangsplattform
    // den Storno nicht zu. Gespeichert ist sie nur in der archivierten XML des
    // Originals (Lesen mit Hash-Prüfung, eine Manipulation bricht ab).
    let buyer_reference = original_buyer_reference(pool, &original.invoice).await?;

    // Domain-Storno bauen.
    let original_view = OriginalInvoiceView {
        invoice_number: &original.invoice.invoice_number,
        currency_code: &original.invoice.currency_code,
        pdf_template: &original.invoice.pdf_template,
        items: original
            .items
            .iter()
            .map(|it| OriginalItemView {
                position: it.position as u32,
                description: &it.description,
                quantity: it.quantity,
                unit_code: &it.unit_code,
                unit_price_cents: it.unit_price_cents,
                tax_rate_percent: it.tax_rate_percent,
                tax_category_code: &it.tax_category_code,
            })
            .collect(),
    };
    let storno_input = build_storno_input(
        &original_view,
        original.invoice.id.clone(),
        storno_date,
        Some(args.reason.clone()),
    );

    // Storno-Counter allokieren (eigener Sequenz-Block ST-YYYY-NNNN). Geschäftsjahr
    // und Nummernkreis folgen dem Storno-Datum, nicht dem Original (GoBD Rz. 51,
    // Lauf E): ein Storno vom Januar gehört ins neue Jahr, ein abgeschlossenes
    // Vorjahr bekommt keine neuen Belege.
    let storno_fy = crate::domain::storno::storno_fiscal_year(&storno_date);
    // Ein liegengebliebener Storno-Entwurf (gescheiterte Pipeline) desselben Jahres
    // wird wiederverwendet: keine zweite ST-Nummer, kein weiterer Entwurf. Aus
    // einem anderen Jahr bleibt er liegen (festschreiben lässt er sich nicht).
    let reuse = invoices::open_storno_draft(pool, &original.invoice.id)
        .await?
        .filter(|(_, _, fy)| *fy == i64::from(storno_fy));
    let (storno_id, storno_number) = if let Some((id, number, _)) = reuse {
        invoices::refresh_storno_draft(pool, &id, storno_date, &args.reason).await?;
        (id, number)
    } else {
        let totals = invoice::compute_totals(&storno_input.items);
        let snapshot = invoices::SellerSnapshot {
            name: seller_row.name.as_str(),
            street: seller_row.street.as_str(),
            postal_code: seller_row.postal_code.as_str(),
            city: seller_row.city.as_str(),
            tax_number: seller_row.tax_number.as_deref(),
            vat_id: seller_row.vat_id.as_deref(),
        };
        // Buyer-Snapshot für den Storno-Beleg aus dem Original übernehmen —
        // selber Empfänger-Stand wie auf der Original-Rechnung.
        let buyer_snapshot = invoices::BuyerSnapshot {
            name: original.invoice.buyer_name.as_deref().unwrap_or(
                original
                    .buyer
                    .as_ref()
                    .map(|b| b.name.as_str())
                    .unwrap_or(""),
            ),
            street: original.invoice.buyer_street.as_deref(),
            postal_code: original.invoice.buyer_postal_code.as_deref(),
            city: original.invoice.buyer_city.as_deref(),
            country_code: original
                .invoice
                .buyer_country_code
                .as_deref()
                .unwrap_or("DE"),
            vat_id: original.invoice.buyer_vat_id.as_deref(),
            email: original.invoice.buyer_email.as_deref(),
        };
        let payload = invoices::DraftCreatePayload {
            contact_id: original.invoice.contact_id.clone(),
            fiscal_year: i64::from(storno_fy),
            // KB-0010: Ein Storno spiegelt den eingefrorenen Status des Originals.
            is_kleinunternehmer: original.invoice.is_kleinunternehmer == 1,
            input: storno_input,
            derived_from_quote_id: None,
        };
        // Lauf F8: Nummer, Entwurf und Audit in **einer** Transaktion wie bei
        // jedem anderen Entwurf (F1) — scheitert etwas, ist keine ST-Nummer
        // ohne Spur verbraucht.
        let mut tx = crate::db::begin_immediate(pool).await?;
        let storno_number =
            numbering::next_number_in(&mut tx, DocType::StornoInvoice, storno_fy).await?;
        let storno_draft_id = invoices::create_draft_in(
            &mut tx,
            &payload,
            &storno_number,
            &snapshot,
            &buyer_snapshot,
            &totals,
        )
        .await?;
        audit_log::append_in(
            &mut tx,
            "invoice.draft_create",
            "invoice",
            &storno_draft_id,
            Some(serde_json::json!({
                "number": &storno_number,
                "gross": totals.gross_amount_cents,
                "items": payload.input.items.len(),
                "storno_for": &original.invoice.id,
            })),
        )
        .await?;
        tx.commit().await?;
        (storno_draft_id, storno_number)
    };

    // Storno locken (Pipeline läuft inkl. KoSIT + Mustang + Archive). Der
    // DB-Lock + die Original-Cancel-Markierung laufen im SELBEN DB-TX
    // (R1-003-Fix, v2026.5-Re-Review): Stromaus zwischen Storno-Lock und
    // Original-Cancel ist damit kein GoBD-relevanter Schwebezustand mehr —
    // beide UPDATEs sind atomar oder die TX rollt zurück.
    let lock = run_lock_pipeline_with_pair(
        pool,
        &paths,
        &storno_id,
        &buyer_reference,
        LockCancelPair {
            original_id: &original.invoice.id,
            reason: Some(&args.reason),
        },
    )
    .await?;

    // Audit-Log für den Original-Cancel — nach erfolgreichem TX-Commit.
    // (best-effort: ein fehlgeschlagener Audit-Eintrag rollt die bereits
    // committete Storno-Pair-Operation nicht zurück.)
    audit_log::append(
        pool,
        "invoice.cancel",
        "invoice",
        &original.invoice.id,
        Some(serde_json::json!({ "storno_id": storno_id, "storno_number": storno_number, "reason": &args.reason })),
    )
    .await?;

    // Block 4 — Auto-Critical-Backup nach Storno-Lock (best-effort).
    backup::request_auto_backup(
        pool,
        &paths,
        session,
        "storno.create",
        ctx.clock().now_utc(),
    )
    .await
    .ok();

    let original_refreshed = invoices::get(pool, &original.invoice.id)
        .await?
        .ok_or_else(|| Error::Domain("cancel: post-UPDATE SELECT leer".into()))?;
    Ok(CancelResponse {
        original: original_refreshed,
        storno: lock,
    })
}

/// Käuferreferenz BT-10 einer festgeschriebenen Rechnung aus ihrer archivierten
/// XML (U7, R1-01). Ohne XML-Archiv oder ohne Element „N/A“ wie bei B2C.
async fn original_buyer_reference(pool: &SqlitePool, invoice: &InvoiceRow) -> Result<String> {
    let Some(archive_id) = invoice.xml_archive_id.as_deref() else {
        return Ok("N/A".to_string());
    };
    let bytes = archive::read_and_verify(pool, archive_id).await?;
    let xml = String::from_utf8_lossy(&bytes);
    Ok(crate::einvoice::parser::cii_buyer_reference(&xml).unwrap_or_else(|| "N/A".to_string()))
}

/// Vervollständigt ein Storno-Paar aus dem Altbestand (Lauf F2c): der Storno
/// ist festgeschrieben, das Original noch nicht storniert. In **einer**
/// Transaktion: Original auf `canceled` mit Verweis auf den vorhandenen Storno
/// und dessen Festschreibe-Zeitpunkt, Audit `invoice.cancel_pair_completed`.
/// Kein neuer Beleg, keine Nummer, kein Archiv. Die EÜR ändert sich nicht: die
/// Erstattung rechnet sie schon über den festgeschriebenen Storno
/// (`storno_reversals`), die Einnahme bleibt am Zahlungsdatum.
async fn complete_storno_pair(
    pool: &SqlitePool,
    original: &InvoiceRow,
    storno: InvoiceRow,
    reason: &str,
) -> Result<CancelResponse> {
    let mut tx = crate::db::begin_immediate(pool).await?;
    let res = sqlx::query(
        "UPDATE invoices
            SET status = 'canceled',
                canceled_at = ?,
                canceled_by_storno_id = ?,
                cancel_reason = COALESCE(cancel_reason, ?, ?),
                updated_at = datetime('now','utc')
          WHERE id = ? AND status != 'canceled' AND locked_at IS NOT NULL",
    )
    .bind(storno.locked_at.as_deref())
    .bind(&storno.id)
    .bind(storno.cancel_reason.as_deref())
    .bind(reason)
    .bind(&original.id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(
            "Original-Rechnung ist bereits storniert.".into(),
        ));
    }
    audit_log::append_in(
        &mut tx,
        "invoice.cancel_pair_completed",
        "invoice",
        &original.id,
        Some(serde_json::json!({
            "number": original.invoice_number,
            "storno_id": storno.id,
            "storno_number": storno.invoice_number,
            "storno_locked_at": storno.locked_at,
            "reason": reason,
        })),
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(
        "Storno-Paar aus dem Altbestand vervollständigt: {} ist durch {} storniert",
        original.invoice_number,
        storno.invoice_number
    );
    let original_refreshed = invoices::get(pool, &original.id)
        .await?
        .ok_or_else(|| Error::Domain("cancel: post-UPDATE SELECT leer".into()))?;
    let validation_status = match storno.validation_status.as_deref() {
        Some("warning") => ValidationStatus::Warning,
        Some("failed") => ValidationStatus::Failed,
        _ => ValidationStatus::Passed,
    };
    Ok(CancelResponse {
        original: original_refreshed,
        storno: LockResponse {
            pdf_archive_id: storno.pdf_archive_id.clone().unwrap_or_default(),
            xml_archive_id: storno.xml_archive_id.clone().unwrap_or_default(),
            validation_status,
            validation_findings_count: 0,
            validation_warnings_count: 0,
            invoice: storno,
        },
    })
}

// =============================================================================
// Commands — Entwurf verwerfen (W14-032)
// =============================================================================

/// Argumente von [`invoices_void_draft`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesVoidDraftArgs {
    pub id: String,
    /// Begründung (Pflicht, mindestens 10 Zeichen) — steht am Beleg und im
    /// Audit-Log.
    pub reason: String,
}

/// Verwirft einen nummerierten Rechnungs- oder Storno-Entwurf (W14-032,
/// Re-Auth). Kein Löschen: Zeitpunkt und Grund stehen am Beleg, die Nummer
/// bleibt verbraucht und ist über den verworfenen Entwurf und das Audit
/// `invoice.draft_voided` nachvollziehbar (GoBD: Vollständigkeit und
/// Nachvollziehbarkeit des Nummernkreises). Danach ist der Entwurf
/// unveränderlich, lässt sich nicht festschreiben und blockiert die
/// DSGVO-Anonymisierung seines Kontakts nicht mehr. Festgeschriebene
/// Rechnungen korrigiert weiterhin nur ein Storno.
pub async fn invoices_void_draft(
    ctx: &Arc<AppCtx>,
    args: InvoicesVoidDraftArgs,
) -> Result<InvoiceRow> {
    let InvoicesVoidDraftArgs { id, reason } = args;
    let pool = ctx.pool()?;
    void_draft_core(&pool, &id, &reason, ctx.clock().now_utc()).await
}

/// Testbarer Kern von [`invoices_void_draft`] (`now` aus der Uhr des
/// Aufrufers). Verwirft in **einer** Transaktion den Entwurf, einen offenen
/// Fortsetzungs-Marker eines Abo-Laufs, der auf ihn zeigt (sonst versuchte der
/// nächste Tick, ihn festzuschreiben), und eine Restore-Vormerkung (F1d), nimmt
/// die Umwandlung des Angebots zurück, aus dem der Entwurf entstand (F2c,
/// Audit `quote.conversion_reverted`), und schreibt das Audit.
pub async fn void_draft_core(
    pool: &SqlitePool,
    id: &str,
    reason: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<InvoiceRow> {
    let reason: String = reason
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(500)
        .collect();
    if reason.chars().count() < 10 {
        return Err(Error::BadRequest(
            "Bitte eine Begründung angeben (mindestens 10 Zeichen) — sie steht am Beleg und im \
             Audit-Log."
                .into(),
        ));
    }
    // Nie parallel zum Festschreiben oder Stornieren desselben Belegs.
    let _invoice_guard = crate::locks::INVOICES.lock(id).await;
    // Lauf F2c: Stammt der Entwurf aus einer Angebots-Umwandlung, nie parallel
    // zum Zurückziehen oder erneuten Umwandeln dieses Angebots.
    let source_quote: Option<String> =
        sqlx::query_scalar("SELECT id FROM quotes WHERE converted_invoice_id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let _quote_guard = match source_quote.as_deref() {
        Some(qid) => Some(crate::locks::QUOTES.lock(qid).await),
        None => None,
    };
    let mut tx = crate::db::begin_immediate(pool).await?;
    let voided =
        invoices::void_draft_in(&mut tx, id, &reason, &crate::clock::to_db_utc(now)).await?;
    // Lauf F2c: Das Angebot, aus dem der Entwurf entstand, wird wieder
    // „angenommen“ und lässt sich neu umwandeln — sonst bliebe es für immer auf
    // einen verworfenen Beleg zeigend blockiert (wer den Beleg nicht bearbeiten,
    // sondern neu anlegen will, verwirft ihn). Die DB lässt
    // das nur für einen verworfenen Entwurf zu (`trg_quotes_converted_final`).
    let reverted: Option<(String, String)> = sqlx::query_as(
        "UPDATE quotes
            SET status = 'accepted', converted_at = NULL, converted_invoice_id = NULL,
                updated_at = datetime('now','utc')
          WHERE converted_invoice_id = ? AND status = 'converted'
         RETURNING id, quote_number",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((quote_id, quote_number)) = &reverted {
        audit_log::append_in(
            &mut tx,
            "quote.conversion_reverted",
            "quote",
            quote_id,
            Some(serde_json::json!({
                "quote_number": quote_number,
                "invoice": id,
                "invoice_number": voided.invoice_number,
                "reason": reason,
            })),
        )
        .await?;
    }
    let pending_prefix = crate::scheduler::recurring_invoice::PENDING_PREFIX;
    let markers: Vec<String> = sqlx::query_scalar(
        "DELETE FROM app_settings
          WHERE substr(key, 1, length(?1)) = ?1
            AND substr(value, 1, length(?2) + 1) = ?2 || '|'
         RETURNING key",
    )
    .bind(pending_prefix)
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let conflict_key = format!(
        "{}invoice:{id}",
        crate::backup::after_restore::CONFLICT_PREFIX
    );
    let conflict_cleared = sqlx::query("DELETE FROM app_settings WHERE key = ?")
        .bind(&conflict_key)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        > 0;
    audit_log::append_in(
        &mut tx,
        "invoice.draft_voided",
        "invoice",
        id,
        Some(serde_json::json!({
            "number": voided.invoice_number,
            "fiscal_year": voided.fiscal_year,
            "gross": voided.gross_amount_cents,
            "storno_for": voided.is_storno_for,
            "reason": reason,
            "recurring_markers_cleared": markers.len(),
            "restore_conflict_cleared": conflict_cleared,
            "quote_conversion_reverted": reverted.as_ref().map(|(_, n)| n),
        })),
    )
    .await?;
    tx.commit().await?;
    tracing::info!("Rechnungs-Entwurf {} verworfen", voided.invoice_number);
    invoices::get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("void_draft: post-UPDATE SELECT leer".into()))
}

// =============================================================================
// Helpers
// =============================================================================

/// Inbox-Hinweis, wenn ein Storno wegen einer eigenen Vorlage ohne
/// Storno-Darstellung mit der eingebauten Vorlage entstand
/// (LE-CUSTOM-TEMPLATE-STORNO). Einmal je Vorlage (Dedup), best-effort.
async fn notify_storno_template_fallback(pool: &SqlitePool, template: &str, storno_number: &str) {
    let body = format!(
        "Die Vorlage „{template}“ stellt Stornos nicht dar (sie nutzt `storno_of` nicht). Der \
         Storno {storno_number} wurde deshalb mit der eingebauten Vorlage erzeugt, damit er nicht \
         als Rechnung mit Zahlungsaufforderung erscheint. Ergänze die Vorlage um die \
         Storno-Darstellung (siehe Handbuch „Einstellungen“, Abschnitt „Eigene Vorlagen“), dann \
         nutzen auch Stornos wieder Dein Layout."
    );
    let dedup = format!("storno_template_fallback:{template}");
    let n = crate::notify::NewNotification {
        title: "Eigene Vorlage ohne Storno-Darstellung",
        body: &body,
        severity: "warning",
        related_entity_type: Some("invoice"),
        action_url: Some("/settings/pdf-templates"),
        dedup_key: Some(&dedup),
        ..Default::default()
    };
    if let Err(e) = crate::notify::emit(pool, n).await {
        tracing::warn!("Hinweis zur Storno-Vorlage nicht angelegt: {e}");
    }
}

/// Effektives Leistungsdatum fürs XRechnung-XML (BT-72 / `ActualDeliverySupplyChainEvent`):
/// liegt kein Datum vor und ist der Fallback aktiv, gilt das Rechnungsdatum als
/// Leistungszeitpunkt (§14 Abs. 4 Nr. 6 UStG). Reine Funktion — unit-getestet.
fn effective_delivery_date(
    delivery_date: Option<NaiveDate>,
    invoice_date: NaiveDate,
    fallback: bool,
) -> Option<NaiveDate> {
    match delivery_date {
        Some(d) => Some(d),
        None if fallback => Some(invoice_date),
        None => None,
    }
}

fn today_berlin() -> NaiveDate {
    // Europe/Berlin unabhängig von der Prozess-TZ (Plan W1.8).
    crate::clock::today_berlin()
}

/// Verkäufer-Sicht mit dem §19-Status des Belegs (ADR 0042 Nr. 3; nie aus dem
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

/// §19-Status eines Rechnungsbelegs (ADR 0042 Nr. 3): ein Storno übernimmt den
/// eingefrorenen Status seines Originals (KB-0010, §14c); sonst gilt der Status
/// am Leistungsdatum, ersatzweise am Rechnungsdatum. Vor dem Festschreiben
/// vorläufig; das Festschreiben bestimmt ihn neu und friert ihn ein.
pub async fn invoice_tax_status(pool: &SqlitePool, input: &InvoiceInput) -> Result<bool> {
    if let Some(original_id) = input.is_storno_for.as_deref() {
        if let Some(original) = invoices::get(pool, original_id).await? {
            return Ok(original.is_kleinunternehmer == 1);
        }
    }
    Ok(crate::db::repo::tax_status::history(pool)
        .await?
        .is_klein_at(crate::domain::tax_status::document_date(
            input.delivery_date,
            input.invoice_date,
        )))
}

/// True, wenn die Position einen nicht-leeren Markdown-Body trägt (Paket-/Rich-Position).
fn item_has_markup(it: &InvoiceItemInput) -> bool {
    it.description_markup
        .as_deref()
        .map(|m| !m.trim().is_empty())
        .unwrap_or(false)
}

/// P3: Für Positionen mit Markdown-Body wird `description` aus dem Markup als
/// Klartext neu berechnet — so bleibt die XRechnung-BT-154 konsistent, egal was
/// das Frontend als `description` mitgeschickt hat (Body-Edits inklusive).
fn recompute_markup_descriptions(items: &mut [InvoiceItemInput]) {
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

fn make_buyer_view(row: &crate::db::models::ContactRow) -> BuyerView<'_> {
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

/// Besitzende Buyer-Daten (Block 19), aus denen ein `BuyerView<'_>` oder ein
/// `BuyerSnapshot<'_>` geborgt werden kann. Quelle ist entweder der Live-Kontakt
/// (Refresh bei Lock) oder der bereits eingefrorene Beleg-Snapshot (Storno).
struct OwnedBuyer {
    name: String,
    street: Option<String>,
    postal_code: Option<String>,
    city: Option<String>,
    country_code: String,
    vat_id: Option<String>,
    email: Option<String>,
}

impl OwnedBuyer {
    fn from_contact(c: &crate::db::models::ContactRow) -> Self {
        Self {
            name: c.name.clone(),
            street: c.street.clone(),
            postal_code: c.postal_code.clone(),
            city: c.city.clone(),
            country_code: c.country_code.clone(),
            vat_id: c.vat_id.clone(),
            email: c.email.clone(),
        }
    }

    /// Aus dem invoice-Snapshot (`buyer_*`). `None`, wenn kein Snapshot vorliegt
    /// (Alt-Beleg vor Migration 0004 — Fallback auf Live-Kontakt).
    fn from_invoice_snapshot(inv: &crate::db::models::InvoiceRow) -> Option<Self> {
        inv.buyer_name.as_ref().map(|name| Self {
            name: name.clone(),
            street: inv.buyer_street.clone(),
            postal_code: inv.buyer_postal_code.clone(),
            city: inv.buyer_city.clone(),
            country_code: inv
                .buyer_country_code
                .clone()
                .unwrap_or_else(|| "DE".to_string()),
            vat_id: inv.buyer_vat_id.clone(),
            email: inv.buyer_email.clone(),
        })
    }

    fn view(&self) -> BuyerView<'_> {
        BuyerView {
            name: &self.name,
            street: self.street.as_deref(),
            postal_code: self.postal_code.as_deref(),
            city: self.city.as_deref(),
            country_code: &self.country_code,
            vat_id: self.vat_id.as_deref(),
            email: self.email.as_deref(),
        }
    }

    fn snapshot(&self) -> invoices::BuyerSnapshot<'_> {
        invoices::BuyerSnapshot {
            name: &self.name,
            street: self.street.as_deref(),
            postal_code: self.postal_code.as_deref(),
            city: self.city.as_deref(),
            country_code: &self.country_code,
            vat_id: self.vat_id.as_deref(),
            email: self.email.as_deref(),
        }
    }
}

fn detail_to_input(detail: &InvoiceDetail) -> Result<InvoiceInput> {
    let invoice = &detail.invoice;
    let direction = match invoice.direction.as_str() {
        "issued" => InvoiceDirection::Issued,
        "received" => InvoiceDirection::Received,
        other => return Err(Error::Domain(format!("unbekannte direction '{other}'"))),
    };
    let invoice_date = parse_date(&invoice.invoice_date)?;
    let delivery_date = invoice
        .delivery_date
        .as_deref()
        .map(parse_date)
        .transpose()?;
    let due_date = invoice.due_date.as_deref().map(parse_date).transpose()?;
    let items = detail
        .items
        .iter()
        .map(|it| InvoiceItemInput {
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
    Ok(InvoiceInput {
        direction,
        invoice_date,
        delivery_date,
        due_date,
        currency_code: invoice.currency_code.clone(),
        items,
        notes: invoice.notes.clone(),
        payment_note: invoice.payment_note.clone(),
        pdf_template: invoice.pdf_template.clone(),
        is_storno_for: invoice.is_storno_for.clone(),
        cancel_reason: invoice.cancel_reason.clone(),
    })
}

/// Jüngstes Zahlungsdatum einer Rechnung (Lauf F8): aus der Zahlungshistorie,
/// ohne Historie `paid_at`.
fn last_payment_date(invoice: &InvoiceRow) -> Result<Option<NaiveDate>> {
    let history = invoices::parse_payment_history(invoice.payment_history_json.as_deref())?;
    let mut last: Option<NaiveDate> = None;
    for entry in history.iter().filter(|e| e.amount_cents != 0) {
        let d = parse_date(&entry.paid_date)?;
        last = Some(last.map_or(d, |l| l.max(d)));
    }
    if last.is_none() {
        if let Some(at) = invoice.paid_at.as_deref() {
            last = Some(parse_date(at)?);
        }
    }
    Ok(last)
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    // SQLite TEXT-Format ist ISO-8601 — entweder "YYYY-MM-DD" oder
    // "YYYY-MM-DD HH:MM:SS"; nimm den ersten 10 Zeichen.
    let s = s.get(..10).unwrap_or(s);
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("ungültiges Datum '{s}': {e}")))
}

fn variant_name(e: &invoice::InvoiceValidationError) -> &'static str {
    use invoice::InvoiceValidationError as E;
    match e {
        E::InvoiceDateInFuture { .. } => "InvoiceDateInFuture",
        E::DueDateBeforeInvoiceDate { .. } => "DueDateBeforeInvoiceDate",
        E::DeliveryDateAfterInvoiceDate { .. } => "DeliveryDateAfterInvoiceDate",
        E::NoItems => "NoItems",
        E::CurrencyEmpty => "CurrencyEmpty",
        E::CurrencyUnsupported(_) => "CurrencyUnsupported",
        E::StornoIdEmpty => "StornoIdEmpty",
        E::TotalNotPositive => "TotalNotPositive",
        E::SellerNameMissing => "SellerNameMissing",
        E::SellerAddressIncomplete => "SellerAddressIncomplete",
        E::SellerMissingTaxIdAndVatId => "SellerMissingTaxIdAndVatId",
        E::XRechnungSellerPhoneMissing => "XRechnungSellerPhoneMissing",
        E::XRechnungDueDateMissing => "XRechnungDueDateMissing",
        E::XRechnungSellerIdMissing => "XRechnungSellerIdMissing",
        E::BuyerNameMissing => "BuyerNameMissing",
        E::BuyerAddressIncomplete => "BuyerAddressIncomplete",
        E::ItemDescriptionMissing(_) => "ItemDescriptionMissing",
        E::ItemQuantityNotPositive(_) => "ItemQuantityNotPositive",
        E::ItemUnitPriceNegative(_) => "ItemUnitPriceNegative",
        E::ItemDuplicatePosition(_) => "ItemDuplicatePosition",
        E::ItemInvalidTaxCategoryCode { .. } => "ItemInvalidTaxCategoryCode",
        E::ItemTaxRateNegative { .. } => "ItemTaxRateNegative",
        E::ItemAmountOutOfRange(_) => "ItemAmountOutOfRange",
        E::TotalOutOfRange => "TotalOutOfRange",
        E::ItemExemptWithoutKleinunternehmer(_) => "ItemExemptWithoutKleinunternehmer",
        E::Paragraph19VatViolation(_) => "Paragraph19VatViolation",
    }
}

#[cfg(test)]
mod tests {
    use super::effective_delivery_date;
    use chrono::NaiveDate;

    #[test]
    fn it_compiles() {}

    #[test]
    fn effective_delivery_date_uses_fallback_only_when_empty() {
        let inv = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let svc = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        // vorhandenes Leistungsdatum bleibt unangetastet (egal ob Fallback an/aus)
        assert_eq!(effective_delivery_date(Some(svc), inv, true), Some(svc));
        assert_eq!(effective_delivery_date(Some(svc), inv, false), Some(svc));
        // leer + Fallback an → Rechnungsdatum (BT-72)
        assert_eq!(effective_delivery_date(None, inv, true), Some(inv));
        // leer + Fallback aus → kein BT-72 (wie bisher)
        assert_eq!(effective_delivery_date(None, inv, false), None);
    }
}
