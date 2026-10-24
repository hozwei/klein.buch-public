//! Tauri-Commands für Kosten (Block 9; E-Rechnung-Empfang folgt in Block 11).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::expense::validate_expense`]) — Hard-Block.
//! - Lieferanten-Snapshot (aus Kontakt oder freiem Namen).
//! - Counter-Allokation `KO-{YYYY}-{NNNN}` ([`crate::db::numbering`]).
//! - Optionaler Beleg-Upload write-once ([`crate::archive`], `ExpenseOriginal`).
//! - Persistenz + Sofort-Festschreiben ([`crate::db::repo::expenses::create`]).
//! - Audit-Log + Auto-Critical-Backup-Hook (Lock-Event).
//!
//! ## GoBD-/§19-Hardline
//!
//! - Kosten werden bei der Erfassung sofort gelockt; Korrektur = Storno
//!   ([`expenses_cancel`]), nie Edit/Löschung.
//! - §19 betrifft NUR Ausgangsbelege — Eingangsrechnungen DÜRFEN USt enthalten.
//!   Diese Pipeline erzwingt daher keine USt-Freiheit (siehe domain::expense).
//! - §13b ist ein reines Hinweis-Flag (keine USt-Auto-Berechnung).

use crate::app_ctx::AppCtx;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::archive::{self, ArchiveKind};
use crate::backup;
use crate::commands::attachments::sanitize_filename;
use crate::config::Paths;
use crate::db::models::{ExpenseDetail, ExpenseListItem, ExpenseRow};
use crate::db::numbering;
use crate::db::repo::{audit_log, contacts, expenses};
use crate::domain::expense::{self, ExpenseInput};
use crate::domain::numbering::DocType;
use crate::einvoice::parser::{self, ParsedEInvoice};
use crate::einvoice::types::ValidationSummary;
use crate::einvoice::{mustang_bridge, validator};
use crate::error::{Error, Result};
use crate::fiscal_year::guard;
use crate::uploads::UploadKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCreateArgs {
    pub input: ExpenseInput,
    /// Primärer Beleg aus `POST /api/upload/beleg` (Plan W7). `None` → ohne Beleg.
    #[serde(default)]
    pub receipt_upload_id: Option<String>,
    /// Vom Command aus dem Upload gelesen, nie vom Client.
    #[serde(skip)]
    pub receipt: Option<crate::uploads::UploadedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCancelArgs {
    pub expense_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseSetPaymentArgs {
    pub expense_id: String,
    /// `None` → wieder auf „offen" setzen (Fehl-Markierung korrigieren).
    pub paid_date: Option<NaiveDate>,
    pub paid_from_account_id: Option<String>,
}

// ---- E-Rechnung-Empfang (Block 11) -----------------------------------------

/// Ergebnis von [`expenses_parse_einvoice`]: ein vorbefüllter, **noch
/// editierbarer** Kosten-Vorschlag plus die extrahierten Rohfelder und der
/// beratende KoSIT-Befund. Es wird **nichts** persistiert — der Nutzer prüft
/// und bestätigt erst im Formular ([`expenses_create_from_einvoice`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EInvoiceParseResult {
    /// Vorbefüllte Kosten-Eingabe (Mapping aus der E-Rechnung).
    pub input: ExpenseInput,
    /// Die extrahierten Rohfelder (für Anzeige/Diagnose im Formular).
    pub parsed: ParsedEInvoice,
    /// KoSIT-Befund — beratend, blockiert nie. `None`, wenn die Validierung
    /// nicht laufen konnte (z. B. Sidecar nicht verfügbar).
    pub validation: Option<ValidationSummary>,
    /// `zugferd` | `xrechnung-cii` | `xrechnung-ubl`.
    pub source_format: String,
    /// War die Quelle ein PDF (ZUGFeRD) statt einer reinen XML?
    pub is_pdf: bool,
}

/// Argumente für [`expenses_create_from_einvoice`]. Das Frontend schickt die
/// (ggf. korrigierte) Eingabe **und** die `uploadId` des Originals aus dem
/// Einlesen, damit das Original GoBD-konform unverändert archiviert wird.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EInvoiceCreateArgs {
    pub input: ExpenseInput,
    /// Original-Datei (XRechnung-XML oder ZUGFeRD-PDF) aus
    /// `POST /api/upload/erechnung` (Plan W7) — wird write-once als
    /// `ReceivedEinvoice` archiviert (Originalformat-Aufbewahrung §147 AO).
    #[serde(default)]
    pub upload_id: String,
    /// Vom Command aus dem Upload gelesen (bzw. vom Ablage-Ordner), nie vom Client.
    #[serde(skip)]
    pub original: Option<crate::uploads::UploadedFile>,
    /// `zugferd` | `xrechnung-cii` | `xrechnung-ubl` (aus dem Parse-Schritt).
    pub source_format: String,
    /// Beratender KoSIT-Befund aus dem Parse-Schritt (wird mitgespeichert).
    pub validation: Option<ValidationSummary>,
}

// =============================================================================
// Read
// =============================================================================

/// Argumente von [`expenses_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesListArgs {
    #[serde(default)]
    pub filter: Option<expenses::ListFilter>,
}

pub async fn expenses_list(
    ctx: &Arc<AppCtx>,
    args: ExpensesListArgs,
) -> Result<Vec<ExpenseListItem>> {
    let ExpensesListArgs { filter } = args;
    let pool = ctx.pool()?;

    expenses::list(&pool, &filter.unwrap_or_default()).await
}

/// Argumente von [`expenses_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesGetArgs {
    pub id: String,
}

pub async fn expenses_get(
    ctx: &Arc<AppCtx>,
    args: ExpensesGetArgs,
) -> Result<Option<ExpenseDetail>> {
    let ExpensesGetArgs { id } = args;
    let pool = ctx.pool()?;

    expenses::get_detail(&pool, &id).await
}

// =============================================================================
// Create
// =============================================================================

/// Argumente von [`expenses_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesCreateArgs {
    pub args: ExpenseCreateArgs,
}

pub async fn expenses_create(ctx: &Arc<AppCtx>, args: ExpensesCreateArgs) -> Result<ExpenseDetail> {
    let ExpensesCreateArgs { mut args } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let upload_id = args.receipt_upload_id.take().filter(|id| !id.is_empty());
    if let Some(id) = upload_id.as_deref() {
        args.receipt = Some(ctx.uploads.read(id, &[UploadKind::Receipt])?);
    }
    let paths = ctx.paths.clone();
    let detail = create_core(&pool, &paths, session, args).await?;
    if let Some(id) = upload_id.as_deref() {
        ctx.uploads.consume(id);
    }
    Ok(detail)
}

/// Testbarer Kern von [`expenses_create`] (ohne Tauri-State).
pub async fn create_core(
    pool: &SqlitePool,
    paths: &Paths,
    session: &backup::BackupSession,
    args: ExpenseCreateArgs,
) -> Result<ExpenseDetail> {
    let mut input = args.input;

    // Lieferanten-Snapshot: bei verknüpftem Kontakt dessen Namen sicherstellen.
    if let Some(cid) = input.vendor_contact_id.as_deref() {
        let contact = contacts::get(pool, cid)
            .await?
            .ok_or_else(|| Error::Domain(format!("Lieferant nicht gefunden: {cid}")))?;
        if input.vendor_name.trim().is_empty() {
            input.vendor_name = contact.name;
        }
    }

    // Hard-Block bei Validierungsfehlern (Kosten kennen keinen Draft-Zustand).
    if let Err(errs) = expense::validate_expense(&input, today_berlin()) {
        return Err(Error::Domain(format!(
            "Kosten können nicht erfasst werden: {}",
            errs.iter()
                .map(expense::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    // GJ = Jahr des Belegdatums (Plan W1.2/W9, nie vom Client).
    let fiscal_year = i64::from(crate::domain::fiscal_year::fiscal_year_for(
        &input.expense_date,
    ));
    // GJ-Festschreibung (Block 15): keine neue Buchung mit Datum in einem
    // abgeschlossenen Geschäftsjahr — weder Beleg- noch Zahlungs-Datum.
    crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;
    if let Some(pd) = input.paid_date {
        crate::fiscal_year::guard::ensure_year_open(pool, pd.year() as i64).await?;
    }
    let expense_number = numbering::next_number(pool, DocType::Expense, fiscal_year as i32).await?;

    // Optionalen Beleg write-once archivieren.
    let receipt_archive_id = match args.receipt {
        Some(file) if !file.bytes.is_empty() => {
            let sanitized = sanitize_filename(&file.file_name);
            let archive_name = format!("{expense_number}-{sanitized}");
            let stored = archive::store_bytes(
                pool,
                &paths.archive_dir,
                fiscal_year as i32,
                ArchiveKind::ExpenseOriginal,
                &archive_name,
                // Typ aus der Inhaltsprüfung, nicht aus der Endung (Plan W7).
                file.mime,
                &file.bytes,
            )
            .await?;
            Some(stored.archive_id)
        }
        _ => None,
    };

    let row = expenses::create(
        pool,
        &input,
        &expense_number,
        fiscal_year,
        receipt_archive_id.as_deref(),
    )
    .await?;

    audit_log::append(
        pool,
        "expense.create",
        "expense",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","gross":{},"category":"{}","reverse_charge_13b":{},"receipt":{}}}"#,
            expense_number,
            row.gross_amount_cents,
            escape(&row.category),
            row.reverse_charge_13b == 1,
            receipt_archive_id.is_some()
        )),
    )
    .await?;

    // Lock-Event → Auto-Critical-Backup (best-effort).
    backup::auto_backup_if_unlocked(pool, paths, session, "expense.lock")
        .await
        .ok();

    expenses::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("create: detail-load leer".into()))
}

// =============================================================================
// Cancel (Storno statt Löschung)
// =============================================================================

/// Argumente von [`expenses_cancel`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesCancelArgs {
    pub args: ExpenseCancelArgs,
}

pub async fn expenses_cancel(ctx: &Arc<AppCtx>, args: ExpensesCancelArgs) -> Result<ExpenseRow> {
    let ExpensesCancelArgs { args } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();

    if args.reason.trim().is_empty() {
        return Err(Error::Domain("Storno-Grund ist erforderlich.".into()));
    }

    // R2-003 (GJ-Guard): Kosten-Cancel mutiert `status='canceled'` per UPDATE.
    // Liegt die Expense in einem festgeschriebenen GJ, würde die Live-EÜR
    // hinter dem Snapshot abweichen — Guard blockt das.
    let existing = expenses::get(pool, &args.expense_id)
        .await?
        .ok_or_else(|| Error::Domain("cancel: Kostenbeleg nicht gefunden.".into()))?;
    guard::ensure_year_open(pool, existing.fiscal_year).await?;
    if let Some(paid) = existing.paid_date.as_deref() {
        if let Ok(d) = NaiveDate::parse_from_str(paid, "%Y-%m-%d") {
            guard::ensure_year_open(pool, d.year() as i64).await?;
        }
    }

    expenses::cancel(pool, &args.expense_id, args.reason.trim()).await?;
    audit_log::append(
        pool,
        "expense.cancel",
        "expense",
        &args.expense_id,
        Some(&format!(r#"{{"reason":"{}"}}"#, escape(args.reason.trim()))),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, &paths, session, "expense.cancel")
        .await
        .ok();

    expenses::get(pool, &args.expense_id)
        .await?
        .ok_or_else(|| Error::Domain("cancel: post-UPDATE SELECT leer".into()))
}

// =============================================================================
// Zahlung markieren (paid_date / Konto — kein Kernfeld, nach Lock erlaubt)
// =============================================================================

/// Argumente von [`expenses_set_payment`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesSetPaymentArgs {
    pub args: ExpenseSetPaymentArgs,
}

pub async fn expenses_set_payment(
    ctx: &Arc<AppCtx>,
    args: ExpensesSetPaymentArgs,
) -> Result<ExpenseRow> {
    let ExpensesSetPaymentArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    if let Some(d) = args.paid_date {
        if d > today_berlin() {
            return Err(Error::Domain(
                "Das Zahldatum darf nicht in der Zukunft liegen (Abfluss-Prinzip §11 EStG).".into(),
            ));
        }
    }

    // R2-004 (GJ-Guard): Cash-Basis-EÜR rechnet jede Zahlung im paid_date-Jahr.
    // Wer paid_date verschiebt, verändert die EÜR von **zwei** Jahren — dem
    // alten Zahlungsjahr (Zahlung weg) und dem neuen (Zahlung hin). Beide
    // müssen offen sein, sonst kippt der Lock-Snapshot.
    let existing = expenses::get(pool, &args.expense_id)
        .await?
        .ok_or_else(|| Error::Domain("set_payment: Kostenbeleg nicht gefunden.".into()))?;
    if let Some(old_paid) = existing.paid_date.as_deref() {
        if let Ok(d) = NaiveDate::parse_from_str(old_paid, "%Y-%m-%d") {
            guard::ensure_year_open(pool, d.year() as i64).await?;
        }
    }
    if let Some(new_paid) = args.paid_date {
        guard::ensure_year_open(pool, new_paid.year() as i64).await?;
    }

    let paid = args.paid_date.map(|d| d.to_string());
    let row = expenses::set_payment(
        pool,
        &args.expense_id,
        paid.as_deref(),
        args.paid_from_account_id.as_deref(),
    )
    .await?;
    audit_log::append(
        pool,
        "expense.payment_set",
        "expense",
        &args.expense_id,
        Some(&format!(
            r#"{{"paid_date":{},"account":{}}}"#,
            match paid.as_deref() {
                Some(d) => format!("\"{d}\""),
                None => "null".to_string(),
            },
            match args.paid_from_account_id.as_deref() {
                Some(a) => format!("\"{}\"", escape(a)),
                None => "null".to_string(),
            }
        )),
    )
    .await?;
    Ok(row)
}

// =============================================================================
// E-Rechnung-Empfang: Parsen (Schritt 1) + Festschreiben (Schritt 2)
// =============================================================================

/// Argumente von [`expenses_parse_einvoice`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesParseEinvoiceArgs {
    /// Aus `POST /api/upload/erechnung` (Plan W7); bleibt für Schritt 2 liegen.
    pub upload_id: String,
}

/// Schritt 1 — eine eingehende E-Rechnung (XRechnung-XML **oder** ZUGFeRD-PDF)
/// einlesen, extrahieren und validieren, OHNE etwas zu persistieren.
///
/// - PDF (`%PDF`-Magic): XML wird via Mustang-Sidecar extrahiert
///   ([`mustang_bridge::extract_xml`]); CII ist der eingebettete Standard.
/// - XML direkt: CII oder UBL, automatisch erkannt.
///
/// Die KoSIT-Validierung ist **beratend** — schlägt sie fehl oder kann sie
/// nicht laufen, geht der Import trotzdem weiter (`validation = None`/`failed`).
pub async fn expenses_parse_einvoice(
    ctx: &Arc<AppCtx>,
    args: ExpensesParseEinvoiceArgs,
) -> Result<EInvoiceParseResult> {
    let ExpensesParseEinvoiceArgs { upload_id } = args;
    let upload = ctx.uploads.read(&upload_id, &[UploadKind::EInvoice])?;

    let paths = ctx.paths.clone();
    parse_einvoice_with_paths(&paths, upload.bytes, upload.file_name).await
}

/// Headless-Helfer hinter [`expenses_parse_einvoice`]: identische Logik, aber
/// ohne `AppHandle`/`State`-Abhaengigkeit — damit der Drop-Folder-Scheduler
/// (Block PV1-DROP) **dieselbe** Pipeline benutzt wie der UI-Pfad und nicht
/// gabelt. Aufrufer reicht eine fertige [`Paths`]-Instanz.
pub async fn parse_einvoice_with_paths(
    paths: &Paths,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<EInvoiceParseResult> {
    if file_bytes.is_empty() {
        return Err(Error::Domain("Die Datei ist leer.".into()));
    }

    let is_pdf = file_bytes.starts_with(b"%PDF");
    tracing::debug!(
        "E-Rechnung-Import: Datei '{file_name}' ({} Bytes), pdf={is_pdf}",
        file_bytes.len()
    );

    // XML beschaffen.
    let xml = if is_pdf {
        let extracted = mustang_bridge::extract_xml(&file_bytes, &paths.sidecar_dir).await?;
        if extracted.trim().is_empty() {
            return Err(Error::Domain(
                "In dieser PDF wurde keine eingebettete E-Rechnung gefunden \
                 (kein ZUGFeRD/Factur-X). Bitte die XRechnung-XML direkt importieren."
                    .into(),
            ));
        }
        extracted
    } else {
        // R3-006: Encoding-Härtung. `from_utf8_lossy` würde invalide UTF-8-
        // Bytes still durch U+FFFD ersetzen — ein als ISO-8859-1 codierter
        // Lieferantenname „Müller" würde zu „M�ller" und ginge so beschädigt
        // ins append-only Audit/Archiv. EN-16931 + KoSIT verlangen UTF-8.
        // Wer eine Nicht-UTF-8-Datei sendet, bekommt einen klaren Fehler.
        String::from_utf8(file_bytes).map_err(|e| {
            let bad = e.utf8_error().valid_up_to();
            Error::Domain(format!(
                "Die XML-Datei ist nicht UTF-8-codiert (erster ungültiger Byte bei Position {bad}). \
                 EN-16931 / KoSIT verlangen UTF-8. Bitte die Datei beim Absender als UTF-8 neu exportieren."
            ))
        })?
    };

    // Parsen (pure). Parser-Fehler → lesbare Domain-Meldung.
    let parsed = parser::parse(&xml).map_err(|e| Error::Domain(e.to_string()))?;

    let source_format = if is_pdf {
        "zugferd".to_string()
    } else {
        match parsed.syntax {
            Some(parser::Syntax::Ubl) => "xrechnung-ubl".to_string(),
            _ => "xrechnung-cii".to_string(),
        }
    };

    // Beratende KoSIT-Validierung — Fehler/Unverfügbarkeit blockiert nicht.
    let validation = validator::validate(&xml, &paths.sidecar_dir)
        .await
        .ok()
        .map(|r| ValidationSummary::from_report(&r));

    let input = parser::build_expense_input(&parsed, today_berlin());

    Ok(EInvoiceParseResult {
        input,
        parsed,
        validation,
        source_format,
        is_pdf,
    })
}

/// Argumente von [`expenses_create_from_einvoice`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesCreateFromEinvoiceArgs {
    pub args: EInvoiceCreateArgs,
}

/// Schritt 2 — die (ggf. korrigierte) Eingabe als Kosten **festschreiben**.
///
/// Reihenfolge: Domain-Validation (Hard-Block) → Lieferanten-Snapshot →
/// Belegnummer → **Original write-once als `ReceivedEinvoice` archivieren**
/// (GoBD-Originalformat) → `expenses::create` (sofort gelockt) →
/// KoSIT-Befund persistieren → Audit → Auto-Critical-Backup.
pub async fn expenses_create_from_einvoice(
    ctx: &Arc<AppCtx>,
    args: ExpensesCreateFromEinvoiceArgs,
) -> Result<ExpenseDetail> {
    let ExpensesCreateFromEinvoiceArgs { mut args } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let upload_id = std::mem::take(&mut args.upload_id);
    args.original = Some(ctx.uploads.read(&upload_id, &[UploadKind::EInvoice])?);
    let paths = ctx.paths.clone();
    let detail = create_from_einvoice_with(pool, &paths, session, args).await?;
    ctx.uploads.consume(&upload_id);
    Ok(detail)
}

/// Headless-Helfer hinter [`expenses_create_from_einvoice`]: identische
/// Pipeline (Validate → Belegnummer → Archive → Expense → Audit → Auto-Backup),
/// aber ohne `AppHandle`/`State`. Verwendet vom Drop-Folder-Scheduler (Block
/// PV1-DROP) — UI-Pfad und automatischer Pfad teilen sich genau diese Funktion.
pub async fn create_from_einvoice_with(
    pool: &SqlitePool,
    paths: &Paths,
    session: &backup::BackupSession,
    args: EInvoiceCreateArgs,
) -> Result<ExpenseDetail> {
    let mut input = args.input;

    let original = match args.original {
        Some(file) if !file.bytes.is_empty() => file,
        _ => return Err(Error::Domain("Die Original-E-Rechnung fehlt.".into())),
    };

    // Lieferanten-Snapshot: bei verknüpftem Kontakt dessen Namen sicherstellen.
    if let Some(cid) = input.vendor_contact_id.as_deref() {
        let contact = contacts::get(pool, cid)
            .await?
            .ok_or_else(|| Error::Domain(format!("Lieferant nicht gefunden: {cid}")))?;
        if input.vendor_name.trim().is_empty() {
            input.vendor_name = contact.name;
        }
    }

    // Hard-Block bei Validierungsfehlern (Kosten kennen keinen Draft-Zustand).
    if let Err(errs) = expense::validate_expense(&input, today_berlin()) {
        return Err(Error::Domain(format!(
            "Kosten können nicht erfasst werden: {}",
            errs.iter()
                .map(expense::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    // GJ = Jahr des Belegdatums (Plan W1.2/W9, nie vom Client).
    let fiscal_year = i64::from(crate::domain::fiscal_year::fiscal_year_for(
        &input.expense_date,
    ));
    // GJ-Festschreibung (Block 15): keine neue Buchung mit Datum in einem
    // abgeschlossenen Geschäftsjahr — weder Beleg- noch Zahlungs-Datum.
    crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;
    if let Some(pd) = input.paid_date {
        crate::fiscal_year::guard::ensure_year_open(pool, pd.year() as i64).await?;
    }
    let expense_number = numbering::next_number(pool, DocType::Expense, fiscal_year as i32).await?;

    // Original-E-Rechnung GoBD-konform unverändert archivieren (write-once).
    // Das ist zugleich der „Beleg" der Kosten-Position (receipt_archive_id).
    // Typ aus der Inhaltsprüfung des Uploads (Plan W7).
    let mime = original.mime;
    let sanitized = sanitize_filename(&original.file_name);
    let archive_name = format!("{expense_number}-{sanitized}");
    let stored = archive::store_bytes(
        pool,
        &paths.archive_dir,
        fiscal_year as i32,
        ArchiveKind::ReceivedEinvoice,
        &archive_name,
        mime,
        &original.bytes,
    )
    .await?;

    let row = expenses::create(
        pool,
        &input,
        &expense_number,
        fiscal_year,
        Some(&stored.archive_id),
    )
    .await?;

    // Beratenden KoSIT-Befund mitschreiben (kein Kernfeld → post-lock erlaubt).
    let (status_str, report_json) = match &args.validation {
        Some(v) => (
            Some(v.status_str().to_string()),
            Some(serde_json::to_string(v)?),
        ),
        None => (None, None),
    };
    expenses::set_einvoice_validation(pool, &row.id, status_str.as_deref(), report_json.as_deref())
        .await?;

    audit_log::append(
        pool,
        "expense.import_einvoice",
        "expense",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","gross":{},"source_format":"{}","vendor_invoice_number":{},"validation":{}}}"#,
            expense_number,
            row.gross_amount_cents,
            escape(&args.source_format),
            match input.vendor_invoice_number.as_deref() {
                Some(v) => format!("\"{}\"", escape(v)),
                None => "null".to_string(),
            },
            match status_str.as_deref() {
                Some(s) => format!("\"{s}\""),
                None => "null".to_string(),
            }
        )),
    )
    .await?;

    // Lock-Event → Auto-Critical-Backup (best-effort).
    backup::auto_backup_if_unlocked(pool, paths, session, "expense.lock")
        .await
        .ok();

    expenses::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("import: detail-load leer".into()))
}

/// Heutiges Datum in Europe/Berlin (`crate::clock`, Plan W1.8).
fn today_berlin() -> NaiveDate {
    crate::clock::today_berlin()
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// =============================================================================
// PV1-A5: Roh-XML-Viewer für empfangene E-Rechnungen
// =============================================================================

/// Anzeige-Payload für den Roh-XML-Viewer im Eingangsbeleg-Detail (PV1-A5).
///
/// Der Hash und die Größe sind die des archivierten Originals (für ZUGFeRD-PDFs
/// also der PDF, nicht des extrahierten XML). So sieht der Steuerberater den
/// gleichen GoBD-Fingerabdruck, der auch in `archive_entries` steht.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct XmlViewerPayload {
    /// Pretty-Print-freies, unverändertes XML — Frontend rendert es in `<pre>`.
    pub xml: String,
    /// `zugferd` | `xrechnung-cii` | `xrechnung-ubl` (feiner als `ExpenseDetail.sourceFormat`).
    pub source_format: String,
    /// SHA-256 des archivierten Originals (Hex). Tamper-Check ist bereits passiert.
    pub sha256_hex: String,
    /// Byte-Größe des archivierten Originals.
    pub byte_size: u64,
}

/// PV1-A5 — Lädt das archivierte Roh-XML einer empfangenen E-Rechnung.
///
/// Liefert `Ok(None)`, wenn die Kosten-Position kein E-Rechnungs-Original im
/// Archiv hat (kein `receipt_archive_id`, der Archive-Eintrag ist nicht
/// `received_einvoice`, oder MIME ist weder XML noch PDF — z. B. ein
/// gescannter PDF-Beleg ohne E-Rechnungs-XML).
///
/// `extract_pdf_xml` wird nur für ZUGFeRD-PDFs aufgerufen — in Produktion
/// [`mustang_bridge::extract_xml`], in Tests eine Fixture-Closure (umgeht den
/// Java-Sidecar).
///
/// Lesepfad mit **silent**-Tamper-Check (D-72): kein `archive.read`-Audit pro
/// Klick, sonst floodet der Viewer das append-only Audit-Log. Manipulation
/// schreibt weiterhin sofort `archive.tamper` aus dem Integrity-Check.
pub async fn load_receipt_xml<F, Fut>(
    pool: &SqlitePool,
    expense_id: &str,
    extract_pdf_xml: F,
) -> Result<Option<XmlViewerPayload>>
where
    F: FnOnce(Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    use sqlx::Row;

    let Some(expense) = expenses::get(pool, expense_id).await? else {
        return Ok(None);
    };
    let Some(archive_id) = expense.receipt_archive_id.as_deref() else {
        return Ok(None);
    };

    // Archive-Metadaten (source + mime + Hash + Größe) in einem Query holen.
    let Some(row) = sqlx::query(
        "SELECT mime_type, source, file_hash_sha256, file_size_bytes
         FROM archive_entries WHERE id = ?",
    )
    .bind(archive_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let source: String = row.try_get("source")?;
    if source != "received_einvoice" {
        // Manueller Beleg-Scan oder anderer Archive-Typ — kein Roh-XML.
        return Ok(None);
    }
    let mime: String = row.try_get("mime_type")?;
    let archived_hash: String = row.try_get("file_hash_sha256")?;
    let archived_size: i64 = row.try_get("file_size_bytes")?;

    // Bytes lesen + SHA-256 verifizieren (silent = kein Audit-Spam).
    let bytes = archive::store::read_and_verify_silent(pool, archive_id).await?;

    let (xml, source_format) = match mime.as_str() {
        "application/pdf" => {
            let xml = extract_pdf_xml(bytes).await?;
            if xml.trim().is_empty() {
                return Err(Error::Domain(
                    "In dieser PDF wurde keine eingebettete E-Rechnung gefunden.".into(),
                ));
            }
            (xml, "zugferd".to_string())
        }
        "application/xml" => {
            // EN-16931 / KoSIT verlangen UTF-8 — Empfangs-Pipeline lehnt
            // andere Encodings bereits beim Import ab, hier nur Bestätigung.
            let xml = String::from_utf8(bytes).map_err(|e| {
                Error::Domain(format!(
                    "Die archivierte XML ist nicht UTF-8-codiert (Position {}). \
                     Bitte den Beleg neu importieren.",
                    e.utf8_error().valid_up_to()
                ))
            })?;
            let fmt = match parser::detect_syntax(&xml) {
                Some(parser::Syntax::Ubl) => "xrechnung-ubl",
                _ => "xrechnung-cii",
            };
            (xml, fmt.to_string())
        }
        _ => return Ok(None),
    };

    Ok(Some(XmlViewerPayload {
        xml,
        source_format,
        sha256_hex: archived_hash,
        byte_size: archived_size as u64,
    }))
}

/// Argumente von [`expenses_receipt_xml_text`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpensesReceiptXmlTextArgs {
    pub expense_id: String,
}

/// Tauri-Wrapper um [`load_receipt_xml`]: hängt den echten Mustang-Sidecar
/// für ZUGFeRD-PDF-Extraktion ein.
pub async fn expenses_receipt_xml_text(
    ctx: &Arc<AppCtx>,
    args: ExpensesReceiptXmlTextArgs,
) -> Result<Option<XmlViewerPayload>> {
    let ExpensesReceiptXmlTextArgs { expense_id } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    let sidecar_dir = paths.sidecar_dir.clone();
    load_receipt_xml(&pool, &expense_id, move |bytes| async move {
        mustang_bridge::extract_xml(&bytes, &sidecar_dir).await
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
