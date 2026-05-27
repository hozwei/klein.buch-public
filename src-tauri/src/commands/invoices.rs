//! Tauri-Commands für die Invoice-Pipeline (Block 3b).
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

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

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
use crate::domain::storno::{build_storno_input, OriginalInvoiceView, OriginalItemView};
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
    pub fiscal_year: i64,
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

#[tauri::command]
pub async fn invoices_list(
    pool: State<'_, SqlitePool>,
    filter: Option<invoices::ListFilter>,
) -> Result<Vec<InvoiceListItem>> {
    let f = filter.unwrap_or_default();
    invoices::list(pool.inner(), &f).await
}

#[tauri::command]
pub async fn invoices_get(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<InvoiceDetail>> {
    invoices::get_detail(pool.inner(), &id).await
}

/// Öffnet das archivierte ZUGFeRD-PDF einer ausgestellten Rechnung im
/// Standard-PDF-Viewer des Betriebssystems.
#[tauri::command]
pub async fn invoices_open_pdf(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<()> {
    let path = pdf_archive_path(pool.inner(), &id).await?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| Error::Other(anyhow::anyhow!("PDF konnte nicht geöffnet werden: {e}")))?;
    Ok(())
}

/// Zeigt das archivierte PDF im Datei-Explorer/Finder (zum Anhängen an
/// eine E-Mail, solange der SMTP-Versand aus Block 5 noch nicht da ist).
#[tauri::command]
pub async fn invoices_reveal_pdf(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<()> {
    let path = pdf_archive_path(pool.inner(), &id).await?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| Error::Other(anyhow::anyhow!("Ordner konnte nicht geöffnet werden: {e}")))?;
    Ok(())
}

/// Liefert die Archiv-Pfade (PDF + XML) einer Rechnung — für Anzeige im UI.
#[tauri::command]
pub async fn invoices_archive_paths(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<ArchivePathsDto> {
    let pool = pool.inner();
    let inv = invoices::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {id}")))?;
    let pdf_path = match inv.pdf_archive_id {
        Some(aid) => Some(archive_file_path(pool, &aid).await?),
        None => None,
    };
    let xml_path = match inv.xml_archive_id {
        Some(aid) => Some(archive_file_path(pool, &aid).await?),
        None => None,
    };
    Ok(ArchivePathsDto { pdf_path, xml_path })
}

// =============================================================================
// Commands — Draft
// =============================================================================

#[tauri::command]
pub async fn invoices_create_draft(
    pool: State<'_, SqlitePool>,
    args: CreateDraftArgs,
) -> Result<InvoiceDetail> {
    create_invoice_draft_from_input(
        pool.inner(),
        &args.contact_id,
        args.fiscal_year,
        &args.input,
        None,
    )
    .await
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
    fiscal_year: i64,
    input: &InvoiceInput,
    derived_from_quote_id: Option<&str>,
) -> Result<InvoiceDetail> {
    // P3: Positionen mit Markdown-Body → `description` (XRechnung-BT-154) aus dem
    // Markup als Klartext neu berechnen, damit das XML auch nach Edits stimmt.
    let owned_input;
    let input = if input.items.iter().any(item_has_markup) {
        owned_input = {
            let mut o = input.clone();
            recompute_markup_descriptions(&mut o.items);
            o
        };
        &owned_input
    } else {
        input
    };

    let buyer_row = contacts::get(pool, contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Empfänger nicht gefunden: {contact_id}")))?;
    let seller_row = seller_profile::get(pool).await?.ok_or_else(|| {
        Error::Domain(
            "Stammdaten (seller_profile) noch nicht gepflegt — bitte unter Einstellungen anlegen."
                .into(),
        )
    })?;

    let seller_view = make_seller_view(&seller_row);
    let buyer_view = make_buyer_view(&buyer_row);

    let today = today_berlin();
    if let Err(errs) = invoice::validate_for_issue(input, &seller_view, &buyer_view, today) {
        let blockers: Vec<_> = errs
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    invoice::InvoiceValidationError::NoItems
                        | invoice::InvoiceValidationError::CurrencyEmpty
                        | invoice::InvoiceValidationError::TotalNotPositive
                        | invoice::InvoiceValidationError::ItemInvalidTaxCategoryCode { .. }
                        | invoice::InvoiceValidationError::ItemDuplicatePosition(_)
                )
            })
            .cloned()
            .collect();
        if !blockers.is_empty() {
            return Err(Error::Domain(format!(
                "Draft kann nicht angelegt werden: {}",
                blockers
                    .iter()
                    .map(invoice::message)
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
    }

    // Counter allokieren — Draft "verbraucht" eine Nummer (siehe
    // db::repo::invoices::create_draft Docs).
    let doc_type = if input.is_storno_for.is_some() {
        DocType::StornoInvoice
    } else {
        DocType::Invoice
    };
    let invoice_number = numbering::next_number(pool, doc_type, fiscal_year as i32).await?;

    let totals = invoice::compute_totals(&input.items);
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
        contact_id: contact_id.to_string(),
        fiscal_year,
        is_kleinunternehmer: seller_row.is_kleinunternehmer == 1,
        input: input.clone(),
        derived_from_quote_id: derived_from_quote_id.map(|s| s.to_string()),
    };

    let row = invoices::create_draft(
        pool,
        &payload,
        &invoice_number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await?;

    audit_log::append(
        pool,
        "invoice.draft_create",
        "invoice",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","gross":{},"items":{}{}}}"#,
            invoice_number,
            totals.gross_amount_cents,
            input.items.len(),
            match derived_from_quote_id {
                Some(qid) => format!(r#","derived_from_quote_id":"{}""#, escape(qid)),
                None => String::new(),
            }
        )),
    )
    .await?;

    invoices::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("create_draft: detail-load leer".into()))
}

#[tauri::command]
pub async fn invoices_validate_draft(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Vec<ValidationIssueDto>> {
    let pool = pool.inner();
    let detail = invoices::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("invoice nicht gefunden: {id}")))?;
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten fehlen — kann nicht validieren".into()))?;
    let buyer_row = detail
        .buyer
        .as_ref()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;
    let input = detail_to_input(&detail)?;
    let seller_view = make_seller_view(&seller_row);
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

#[tauri::command]
pub async fn invoices_lock_and_issue(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    id: String,
    buyer_reference: Option<String>,
) -> Result<LockResponse> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    // GJ-Festschreibung (Block 15): eine Rechnung mit Datum in einem
    // abgeschlossenen Geschäftsjahr kann nicht (mehr) ausgestellt werden.
    let draft = invoices::get_detail(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {id}")))?;
    crate::fiscal_year::guard::ensure_year_open(pool, draft.invoice.fiscal_year).await?;
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
    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "invoice.lock")
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
/// Public, damit Integration-Tests sie ohne Tauri-AppHandle ausführen
/// können (Tests bauen Paths direkt aus tempdir + Mock-Sidecar).
pub async fn run_lock_pipeline(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
    buyer_reference: &str,
) -> Result<LockResponse> {
    run_lock_pipeline_inner(pool, paths, invoice_id, buyer_reference, None).await
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
    run_lock_pipeline_inner(pool, paths, invoice_id, buyer_reference, Some(pair)).await
}

async fn run_lock_pipeline_inner(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
    buyer_reference: &str,
    cancel_pair: Option<LockCancelPair<'_>>,
) -> Result<LockResponse> {
    let detail = invoices::get_detail(pool, invoice_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("invoice nicht gefunden: {invoice_id}")))?;

    if detail.invoice.locked_at.is_some() {
        return Err(Error::Domain(format!(
            "invoice {invoice_id} ist bereits gelockt"
        )));
    }
    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten fehlen".into()))?;
    let buyer_row = detail
        .buyer
        .clone()
        .ok_or_else(|| Error::Domain("Empfänger-Kontakt fehlt".into()))?;

    let seller_view = make_seller_view(&seller_row);

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
    let today = today_berlin();

    // 1. Domain-Validation — Hard-Block bei §14/§19-Fehlern.
    invoice::validate_for_issue(&input, &seller_view, &buyer_view, today).map_err(|errs| {
        Error::Domain(format!(
            "Validation failed ({} Fehler): {}",
            errs.len(),
            errs.iter()
                .take(5)
                .map(invoice::message)
                .collect::<Vec<_>>()
                .join("; ")
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
    let xml = generator::to_xrechnung(
        &detail.invoice.invoice_number,
        &xml_input,
        &seller_view,
        &buyer_view,
        buyer_reference,
        &bank_accounts,
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
    let template_source = templates::resolve_invoice_template(&paths.inputs_dir, template_name)?;
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
    let pdf_bytes = typst_render::render_invoice(
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
    )?;

    // 6. Mustang: ZUGFeRD-PDF/A-3 mit eingebettetem XML.
    let zugferd_bytes =
        mustang_bridge::create_zugferd(&pdf_bytes, &xml, &paths.sidecar_dir).await?;

    // 7. Archivierung — PDF + XML in `archive_entries`.
    let fy = detail.invoice.fiscal_year as i32;
    let pdf_archive = archive::store_bytes(
        pool,
        &paths.archive_dir,
        fy,
        ArchiveKind::InvoicePdf,
        &format!("{}.pdf", detail.invoice.invoice_number),
        "application/pdf",
        &zugferd_bytes,
    )
    .await?;
    let xml_archive = archive::store_bytes(
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
    };
    let pair_repo = cancel_pair.map(|p| invoices::PairCancel {
        original_id: p.original_id,
        reason: p.reason,
    });
    invoices::lock_with_pair_cancel(pool, invoice_id, &lock_update, pair_repo).await?;

    // 9. Audit-Log.
    audit_log::append(
        pool,
        "invoice.lock",
        "invoice",
        invoice_id,
        Some(&format!(
            r#"{{"number":"{}","pdf":"{}","xml":"{}","kosit":"{}","warnings":{}}}"#,
            detail.invoice.invoice_number,
            pdf_archive.archive_id,
            xml_archive.archive_id,
            validation_status_str,
            report.warning_count
        )),
    )
    .await?;

    // 10. Auto-Backup-Marker (GoBD-Trace innerhalb der Pipeline).
    // Das eigentliche Auto-Critical-Backup wird vom Command-Wrapper
    // (`invoices_lock_and_issue` / `invoices_cancel`) via
    // `backup::auto_backup_if_unlocked` ausgelöst — nur dort ist die
    // Session-Passphrase (Tauri-State) verfügbar. `run_lock_pipeline` selbst
    // bleibt I/O-frei bzgl. Backup, damit Integration-Tests sie ohne
    // entsperrte Session ausführen können.
    audit_log::append(
        pool,
        "invoice.lock.backup_pending",
        "invoice",
        invoice_id,
        Some(r#"{"hook":"backup::auto_backup_if_unlocked(auto_critical)","module":"block-4"}"#),
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

#[tauri::command]
pub async fn invoices_record_payment(
    pool: State<'_, SqlitePool>,
    args: RecordPaymentArgs,
) -> Result<InvoiceRow> {
    let pool = pool.inner();
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
    crate::fiscal_year::guard::ensure_year_open(
        pool,
        chrono::Datelike::year(&args.paid_date) as i64,
    )
    .await?;
    let row = invoices::record_payment(
        pool,
        &args.invoice_id,
        args.amount_cents,
        &args.paid_date.to_string(),
        args.note.as_deref(),
    )
    .await?;
    audit_log::append(
        pool,
        "invoice.payment_recorded",
        "invoice",
        &args.invoice_id,
        Some(&format!(
            r#"{{"amount":{},"paid_date":"{}","status":"{}"}}"#,
            args.amount_cents, args.paid_date, row.status
        )),
    )
    .await?;
    Ok(row)
}

// =============================================================================
// Commands — Cancel (Storno)
// =============================================================================

#[tauri::command]
pub async fn invoices_cancel(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    args: CancelArgs,
) -> Result<CancelResponse> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;

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
            "Storno nur für gelockte Rechnungen möglich. Drafts bitte einfach löschen.".into(),
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

    let seller_row = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten fehlen".into()))?;
    let storno_date = args.storno_date.unwrap_or_else(today_berlin);

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

    // Storno-Counter allokieren (eigener Sequenz-Block ST-YYYY-NNNN).
    let storno_number = numbering::next_number(
        pool,
        DocType::StornoInvoice,
        original.invoice.fiscal_year as i32,
    )
    .await?;
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
        fiscal_year: original.invoice.fiscal_year,
        is_kleinunternehmer: seller_row.is_kleinunternehmer == 1,
        input: storno_input,
        derived_from_quote_id: None,
    };
    let storno_draft = invoices::create_draft(
        pool,
        &payload,
        &storno_number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await?;

    // Storno locken (Pipeline läuft inkl. KoSIT + Mustang + Archive). Der
    // DB-Lock + die Original-Cancel-Markierung laufen im SELBEN DB-TX
    // (R1-003-Fix, v2026.5-Re-Review): Stromaus zwischen Storno-Lock und
    // Original-Cancel ist damit kein GoBD-relevanter Schwebezustand mehr —
    // beide UPDATEs sind atomar oder die TX rollt zurück.
    let lock = run_lock_pipeline_with_pair(
        pool,
        &paths,
        &storno_draft.id,
        "N/A",
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
        Some(&format!(
            r#"{{"storno_id":"{}","storno_number":"{}","reason":"{}"}}"#,
            storno_draft.id,
            storno_number,
            escape(&args.reason)
        )),
    )
    .await?;

    // Block 4 — Auto-Critical-Backup nach Storno-Lock (best-effort).
    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "storno.create")
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

// =============================================================================
// Helpers
// =============================================================================

/// Löst die PDF-Archiv-Datei einer ausgestellten Rechnung auf.
async fn pdf_archive_path(pool: &SqlitePool, invoice_id: &str) -> Result<String> {
    let inv = invoices::get(pool, invoice_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {invoice_id}")))?;
    let archive_id = inv.pdf_archive_id.ok_or_else(|| {
        Error::Domain("Diese Rechnung hat noch kein PDF — erst 'Lock & Issue' ausführen.".into())
    })?;
    archive_file_path(pool, &archive_id).await
}

/// Liest `archive_entries.file_path` für eine Archive-ID.
async fn archive_file_path(pool: &SqlitePool, archive_id: &str) -> Result<String> {
    use sqlx::Row;
    let row = sqlx::query("SELECT file_path FROM archive_entries WHERE id = ?")
        .bind(archive_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| Error::Domain(format!("Archiv-Eintrag nicht gefunden: {archive_id}")))?;
    Ok(row.try_get("file_path")?)
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
    // Europe/Berlin via chrono-tz wäre sauberer, aber zieht weitere
    // Dependency. Local::now() folgt dem System-TZ; Block 0 hat
    // Europe/Berlin als Host-TZ gepinnt (CLAUDE.md). Wenn das ein
    // Problem wird, migrieren wir auf chrono-tz.
    Local::now().date_naive()
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
        iban: row.iban.as_deref(),
        bic: row.bic.as_deref(),
        is_kleinunternehmer: row.is_kleinunternehmer == 1,
        waived_since: row
            .waived_paragraph_19_since
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
    }
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

fn parse_date(s: &str) -> Result<NaiveDate> {
    // SQLite TEXT-Format ist ISO-8601 — entweder "YYYY-MM-DD" oder
    // "YYYY-MM-DD HH:MM:SS"; nimm den ersten 10 Zeichen.
    let s = &s[..s.len().min(10)];
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
        E::BuyerNameMissing => "BuyerNameMissing",
        E::BuyerAddressIncomplete => "BuyerAddressIncomplete",
        E::ItemDescriptionMissing(_) => "ItemDescriptionMissing",
        E::ItemQuantityNotPositive(_) => "ItemQuantityNotPositive",
        E::ItemUnitPriceNegative(_) => "ItemUnitPriceNegative",
        E::ItemDuplicatePosition(_) => "ItemDuplicatePosition",
        E::ItemInvalidTaxCategoryCode { .. } => "ItemInvalidTaxCategoryCode",
        E::ItemTaxRateNegative { .. } => "ItemTaxRateNegative",
        E::Paragraph19VatViolation(_) => "Paragraph19VatViolation",
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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
