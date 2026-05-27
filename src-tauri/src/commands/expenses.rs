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

use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::archive::{self, ArchiveKind};
use crate::backup;
use crate::commands::attachments::{guess_mime, sanitize_filename};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseCreateArgs {
    pub input: ExpenseInput,
    /// Optional — sonst aus dem Beleg-Datum (Kalenderjahr) abgeleitet.
    pub fiscal_year: Option<i64>,
    /// Roh-Bytes des primären Belegs (PDF/Bild). `None` → ohne Beleg.
    pub receipt_bytes: Option<Vec<u8>>,
    pub receipt_filename: Option<String>,
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
/// (ggf. korrigierte) Eingabe **und** die Original-Bytes erneut mit, damit das
/// Original GoBD-konform unverändert archiviert werden kann.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EInvoiceCreateArgs {
    pub input: ExpenseInput,
    pub fiscal_year: Option<i64>,
    /// Original-Datei (XRechnung-XML oder ZUGFeRD-PDF) — wird write-once
    /// als `ReceivedEinvoice` archiviert (Originalformat-Aufbewahrung §147 AO).
    pub original_bytes: Vec<u8>,
    pub original_file_name: String,
    /// `zugferd` | `xrechnung-cii` | `xrechnung-ubl` (aus dem Parse-Schritt).
    pub source_format: String,
    /// Beratender KoSIT-Befund aus dem Parse-Schritt (wird mitgespeichert).
    pub validation: Option<ValidationSummary>,
}

// =============================================================================
// Read
// =============================================================================

#[tauri::command]
pub async fn expenses_list(
    pool: State<'_, SqlitePool>,
    filter: Option<expenses::ListFilter>,
) -> Result<Vec<ExpenseListItem>> {
    expenses::list(pool.inner(), &filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn expenses_get(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<ExpenseDetail>> {
    expenses::get_detail(pool.inner(), &id).await
}

// =============================================================================
// Create
// =============================================================================

#[tauri::command]
pub async fn expenses_create(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    args: ExpenseCreateArgs,
) -> Result<ExpenseDetail> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
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

    let fiscal_year = args
        .fiscal_year
        .unwrap_or_else(|| input.expense_date.year() as i64);
    // GJ-Festschreibung (Block 15): keine neue Buchung mit Datum in einem
    // abgeschlossenen Geschäftsjahr — weder Beleg- noch Zahlungs-Datum.
    crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;
    if let Some(pd) = input.paid_date {
        crate::fiscal_year::guard::ensure_year_open(pool, pd.year() as i64).await?;
    }
    let expense_number = numbering::next_number(pool, DocType::Expense, fiscal_year as i32).await?;

    // Optionalen Beleg write-once archivieren.
    let receipt_archive_id = match args.receipt_bytes {
        Some(bytes) if !bytes.is_empty() => {
            let sanitized = sanitize_filename(args.receipt_filename.as_deref().unwrap_or("beleg"));
            let archive_name = format!("{expense_number}-{sanitized}");
            let mime = guess_mime(&sanitized);
            let stored = archive::store_bytes(
                pool,
                &paths.archive_dir,
                fiscal_year as i32,
                ArchiveKind::ExpenseOriginal,
                &archive_name,
                mime,
                &bytes,
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
    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "expense.lock")
        .await
        .ok();

    expenses::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("create: detail-load leer".into()))
}

// =============================================================================
// Cancel (Storno statt Löschung)
// =============================================================================

#[tauri::command]
pub async fn expenses_cancel(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    args: ExpenseCancelArgs,
) -> Result<ExpenseRow> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;

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

    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "expense.cancel")
        .await
        .ok();

    expenses::get(pool, &args.expense_id)
        .await?
        .ok_or_else(|| Error::Domain("cancel: post-UPDATE SELECT leer".into()))
}

// =============================================================================
// Zahlung markieren (paid_date / Konto — kein Kernfeld, nach Lock erlaubt)
// =============================================================================

#[tauri::command]
pub async fn expenses_set_payment(
    pool: State<'_, SqlitePool>,
    args: ExpenseSetPaymentArgs,
) -> Result<ExpenseRow> {
    let pool = pool.inner();
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

/// Schritt 1 — eine eingehende E-Rechnung (XRechnung-XML **oder** ZUGFeRD-PDF)
/// einlesen, extrahieren und validieren, OHNE etwas zu persistieren.
///
/// - PDF (`%PDF`-Magic): XML wird via Mustang-Sidecar extrahiert
///   ([`mustang_bridge::extract_xml`]); CII ist der eingebettete Standard.
/// - XML direkt: CII oder UBL, automatisch erkannt.
///
/// Die KoSIT-Validierung ist **beratend** — schlägt sie fehl oder kann sie
/// nicht laufen, geht der Import trotzdem weiter (`validation = None`/`failed`).
#[tauri::command]
pub async fn expenses_parse_einvoice(
    app: AppHandle,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<EInvoiceParseResult> {
    let paths = Paths::from_handle(&app)?;
    parse_einvoice_with_paths(&paths, file_bytes, file_name).await
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

/// Schritt 2 — die (ggf. korrigierte) Eingabe als Kosten **festschreiben**.
///
/// Reihenfolge: Domain-Validation (Hard-Block) → Lieferanten-Snapshot →
/// Belegnummer → **Original write-once als `ReceivedEinvoice` archivieren**
/// (GoBD-Originalformat) → `expenses::create` (sofort gelockt) →
/// KoSIT-Befund persistieren → Audit → Auto-Critical-Backup.
#[tauri::command]
pub async fn expenses_create_from_einvoice(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    args: EInvoiceCreateArgs,
) -> Result<ExpenseDetail> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    create_from_einvoice_with(pool, &paths, session.inner(), args).await
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

    if args.original_bytes.is_empty() {
        return Err(Error::Domain("Die Original-E-Rechnung fehlt.".into()));
    }

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

    let fiscal_year = args
        .fiscal_year
        .unwrap_or_else(|| input.expense_date.year() as i64);
    // GJ-Festschreibung (Block 15): keine neue Buchung mit Datum in einem
    // abgeschlossenen Geschäftsjahr — weder Beleg- noch Zahlungs-Datum.
    crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;
    if let Some(pd) = input.paid_date {
        crate::fiscal_year::guard::ensure_year_open(pool, pd.year() as i64).await?;
    }
    let expense_number = numbering::next_number(pool, DocType::Expense, fiscal_year as i32).await?;

    // Original-E-Rechnung GoBD-konform unverändert archivieren (write-once).
    // Das ist zugleich der „Beleg" der Kosten-Position (receipt_archive_id).
    let mime = if args.source_format == "zugferd" {
        "application/pdf"
    } else {
        "application/xml"
    };
    let sanitized = sanitize_filename(&args.original_file_name);
    let archive_name = format!("{expense_number}-{sanitized}");
    let stored = archive::store_bytes(
        pool,
        &paths.archive_dir,
        fiscal_year as i32,
        ArchiveKind::ReceivedEinvoice,
        &archive_name,
        mime,
        &args.original_bytes,
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

/// Heutiges Datum (Europe/Berlin = System-TZ, in Block 0 gepinnt). Wie in
/// `commands::invoices`; bei Bedarf später auf chrono-tz migrieren.
fn today_berlin() -> NaiveDate {
    Local::now().date_naive()
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

/// Tauri-Wrapper um [`load_receipt_xml`]: hängt den echten Mustang-Sidecar
/// für ZUGFeRD-PDF-Extraktion ein.
#[tauri::command]
pub async fn expenses_receipt_xml_text(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    expense_id: String,
) -> Result<Option<XmlViewerPayload>> {
    let paths = Paths::from_handle(&app)?;
    let sidecar_dir = paths.sidecar_dir.clone();
    load_receipt_xml(pool.inner(), &expense_id, move |bytes| async move {
        mustang_bridge::extract_xml(&bytes, &sidecar_dir).await
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
