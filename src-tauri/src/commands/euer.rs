//! Commands für die EÜR-Auswertung (Block 13).
//!
//! **Lese**-Auswertung (Cash-Basis, §4 Abs. 3 EStG): keine Buchung, kein Lock,
//! kein Backup-Hook. Die Periodenzuordnung + Summierung liegt im Functional
//! Core [`crate::euer::aggregate`]; hier wird nur geladen
//! ([`crate::db::repo::euer`]) und durchgereicht.
//!
//! **Exporte** (PDF, DATEV, ELSTER, Einzelaufstellung, Steuerberater-Paket)
//! schreiben je einen Pflicht-Audit-Eintrag (Lauf F, R2-016 / ASVS 8.3.5:
//! Geschäftsjahr, Dateiname, Zählwerte, keine Inhalte). Für abgeschlossene
//! Jahre gleichen Anzeige und Exporte die Eckwerte mit dem
//! Festschreibungs-Snapshot ab ([`crate::fiscal_year::snapshot`], KB-0031).

use crate::app_ctx::AppCtx;
use chrono::Datelike;
use serde::Serialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::archive;
use crate::config::Paths;
use crate::db::models::SellerProfileRow;
use crate::db::repo::{audit_log, euer as euer_repo, seller_profile};
use crate::domain::kleinunternehmer::HINWEIS_TEXT;
use crate::error::Result;
use crate::euer::aggregate::{self, EuerReport};
use crate::euer::datev_csv::{self, DatevHeader, DatevInputs, Skr};
use crate::euer::detail::{
    self, de_date, AveeurItem, DisposalItem, ExpenseItem, IncomeItem, StornoItem,
};
use crate::euer::elster_csv::{self, ElsterForm, ElsterInputs};
use crate::fiscal_year::snapshot::{self, SnapshotCheck};
use crate::pdf::{templates, typst_render};

/// §19-Status eines Geschäftsjahres aus der Statushistorie (R2-002, ADR 0042
/// Nr. 3): `(§19 das ganze Jahr, Wechsel im Jahr als Text)`. Ein Jahr mit
/// Wechsel gilt im Kopf nicht als §19-Jahr; Einnahmen und Kosten tragen ihren
/// eigenen Status.
pub(crate) async fn year_tax_status(
    pool: &SqlitePool,
    fiscal_year: i32,
) -> Result<(bool, Option<String>)> {
    let hist = crate::db::repo::tax_status::history(pool).await?;
    let ys = hist.year_status(fiscal_year);
    let change = (ys == crate::domain::tax_status::YearStatus::Wechsel)
        .then(|| crate::commands::tax_status::year_status_text(&hist, fiscal_year));
    Ok((ys.is_klein(), change))
}

/// Pflicht-Audit eines Exports (Lauf F, R2-016 / ASVS 8.3.5): Geschäftsjahr,
/// Dateiname und Zählwerte — keine Inhalte, keine personenbezogenen Daten —
/// sowie der Snapshot-Status (`open`/`match`/`mismatch`). Eine Abweichung
/// meldet zusätzlich [`snapshot::report_mismatch`].
async fn audit_export(
    pool: &SqlitePool,
    action: &str,
    fiscal_year: i64,
    mut details: serde_json::Value,
    check: Option<&SnapshotCheck>,
) -> Result<()> {
    details["fiscal_year"] = serde_json::Value::from(fiscal_year);
    details["snapshot"] = serde_json::Value::from(snapshot::audit_status(check));
    audit_log::append(
        pool,
        action,
        "euer",
        &fiscal_year.to_string(),
        Some(details),
    )
    .await?;
    if let Some(check) = check {
        snapshot::report_mismatch(pool, check, action).await?;
    }
    Ok(())
}

/// Argumente von [`euer_compute_report`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerComputeReportArgs {
    pub fiscal_year: i64,
}

/// Antwort von [`euer_compute_report`]: der Report (Felder flach wie bisher)
/// plus zwei Lesefelder für die Anzeige (Lauf 2026.11 U4, Warteliste 3).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerReportView {
    #[serde(flatten)]
    pub report: EuerReport,
    /// §19 das ganze Jahr (wie Exporte und ELSTER-Ausfüllhilfe, R2-002);
    /// `false` = Regelbesteuerung oder Wechsel im Jahr.
    pub is_kleinunternehmer: bool,
    /// Status des Jahres dreiwertig (ADR 0042 Nr. 3).
    pub year_status: crate::domain::tax_status::YearStatus,
    /// Wechsel im Jahr als Text; `None` = einheitlicher Status.
    pub status_change: Option<String>,
    /// Abgleich mit dem Festschreibungs-Snapshot (nur abgeschlossene Jahre,
    /// KB-0031); `None` = Jahr offen. Nur Anzeige — anders als die Exporte
    /// schreibt die Auswertung weder Audit noch Hinweis.
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Berechnet den EÜR-Report für ein Geschäftsjahr (Cash-Basis) samt §19-Status
/// des Jahres und Snapshot-Abgleich. Liest nur.
pub async fn euer_compute_report(
    ctx: &Arc<AppCtx>,
    args: EuerComputeReportArgs,
) -> Result<EuerReportView> {
    let EuerComputeReportArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    let inputs = euer_repo::load_inputs(&pool).await?;
    let report = aggregate::aggregate(fiscal_year as i32, &inputs);
    let hist = crate::db::repo::tax_status::history(&pool).await?;
    let year_status = hist.year_status(fiscal_year as i32);
    // Gleiche Rechnung wie `snapshot::check`, ohne die Eingaben ein zweites Mal
    // zu laden.
    let snapshot_check = crate::fiscal_year::lock::get(&pool, fiscal_year)
        .await?
        .map(|lock| snapshot::compare(&lock, &report));
    Ok(EuerReportView {
        is_kleinunternehmer: year_status.is_klein(),
        year_status,
        status_change: (year_status == crate::domain::tax_status::YearStatus::Wechsel)
            .then(|| crate::commands::tax_status::year_status_text(&hist, fiscal_year as i32)),
        snapshot_check,
        report,
    })
}

/// Argumente von [`euer_available_years`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerAvailableYearsArgs {}

/// Geschäftsjahre mit EÜR-relevanten Bewegungen — für den Jahres-Selector.
/// Das aktuelle Jahr (Europe/Berlin) ist immer enthalten, damit auch ohne
/// Daten ein sinnvoller Default angeboten wird. Absteigend sortiert.
pub async fn euer_available_years(
    ctx: &Arc<AppCtx>,
    _args: EuerAvailableYearsArgs,
) -> Result<Vec<i64>> {
    let pool = ctx.pool()?;

    let mut years: Vec<i64> = euer_repo::available_years(&pool)
        .await?
        .into_iter()
        .map(|y| y as i64)
        .collect();

    let current = crate::clock::today_berlin().year() as i64;
    if !years.contains(&current) {
        years.push(current);
    }
    years.sort_unstable_by(|a, b| b.cmp(a));
    Ok(years)
}

// ============================================================================
// Block 14a — ELSTER-Ausfüllhilfe (Anlage EÜR)
// ============================================================================

/// Ergebnis eines CSV-Exports der ELSTER-Ausfüllhilfe.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsterExportResult {
    /// Abruf über `GET /files/download/{id}` (Plan W7).
    pub download: crate::downloads::DownloadTicket,
    /// Anzahl aller Positionen (inkl. Kontrollsummen).
    pub line_count: usize,
    /// Anzahl der ins ELSTER-Formular einzutragenden Positionen.
    pub entry_count: usize,
    /// Abgleich mit dem Festschreibungs-Snapshot (nur abgeschlossene Jahre).
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Baut die ELSTER-Ausfüllhilfe für ein Geschäftsjahr: EÜR-Report aggregieren,
/// AfA nach Zeile aufteilen, Einnahmen nach dem eingefrorenen Status der
/// Rechnung (Lauf F5b) und Ausgaben nach dem §19-Status bei Leistungsbezug
/// (Jahr des Belegdatums, Lauf F8d) aufteilen.
async fn build_form_for_year(pool: &SqlitePool, fiscal_year: i32) -> Result<ElsterForm> {
    let inputs = euer_repo::load_inputs(pool).await?;
    let report = aggregate::aggregate(fiscal_year, &inputs);
    let afa = euer_repo::depreciation_split_for_year(pool, fiscal_year).await?;
    let income = euer_repo::income_detail(pool, fiscal_year).await?;
    let storno = euer_repo::storno_detail(pool, fiscal_year).await?;
    let expenses = euer_repo::expense_detail(pool, fiscal_year).await?;
    let capitalized = euer_repo::capitalized_expense_detail(pool, fiscal_year).await?;
    // R2-002, ADR 0042 Nr. 3: §19-Status des Jahres aus der Historie.
    let (is_kleinunternehmer, status_change) = year_tax_status(pool, fiscal_year).await?;
    Ok(elster_csv::build_form(&ElsterInputs {
        report: &report,
        afa: &afa,
        income: &income,
        storno: &storno,
        expenses: &expenses,
        capitalized: &capitalized,
        is_kleinunternehmer,
        status_change,
    }))
}

/// Komplettes EÜR-Paket eines Geschäftsjahres für die Anzeige: Anlage-EÜR-
/// Zeilen (Ausfüllhilfe) + Einzelaufstellung (Einnahmen/Stornos/Ausgaben/
/// Veräußerungen) + Anlageverzeichnis (AVEÜR). Read-only.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerPackage {
    pub form: ElsterForm,
    pub income: Vec<IncomeItem>,
    pub storno: Vec<StornoItem>,
    pub expenses: Vec<ExpenseItem>,
    pub disposals: Vec<DisposalItem>,
    pub assets: Vec<AveeurItem>,
    /// Abgleich mit dem Festschreibungs-Snapshot (nur abgeschlossene Jahre,
    /// KB-0031) — die Anzeige warnt bei `matches = false`.
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Argumente von [`euer_package`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerPackageArgs {
    pub fiscal_year: i64,
}

pub async fn euer_package(ctx: &Arc<AppCtx>, args: EuerPackageArgs) -> Result<EuerPackage> {
    let EuerPackageArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let y = fiscal_year as i32;
    Ok(EuerPackage {
        form: build_form_for_year(pool, y).await?,
        income: euer_repo::income_detail(pool, y).await?,
        storno: euer_repo::storno_detail(pool, y).await?,
        expenses: euer_repo::expense_detail(pool, y).await?,
        disposals: euer_repo::disposal_detail(pool, y).await?,
        assets: euer_repo::aveeur_items(pool, y).await?,
        snapshot_check: snapshot::check(pool, fiscal_year).await?,
    })
}

/// Ergebnis des Einzelaufstellung-ZIP-Exports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerDetailExport {
    /// Abruf über `GET /files/download/{id}` (Plan W7).
    pub download: crate::downloads::DownloadTicket,
    pub income_count: usize,
    pub expense_count: usize,
    pub asset_count: usize,
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Argumente von [`euer_export_detail_zip`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerExportDetailZipArgs {
    pub fiscal_year: i64,
}

/// Schreibt die Einzelaufstellung als ZIP (einnahmen.csv, ausgaben.csv,
/// anlageverzeichnis.csv) in den Download-Ordner (Plan W7). Keine Buchung;
/// Pflicht-Audit `euer.detail_export` (Lauf F).
pub async fn euer_export_detail_zip(
    ctx: &Arc<AppCtx>,
    args: EuerExportDetailZipArgs,
) -> Result<EuerDetailExport> {
    let EuerExportDetailZipArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    use std::io::Write;

    let pool = &pool;
    let y = fiscal_year as i32;
    let income = euer_repo::income_detail(pool, y).await?;
    let expenses = euer_repo::expense_detail(pool, y).await?;
    let assets = euer_repo::aveeur_items(pool, y).await?;

    // Plan W7: kein Pfad vom Client — Datei im Download-Ordner.
    let (download_id, download_path) = ctx
        .downloads
        .prepare(&format!("EUER-{fiscal_year}-Einzelaufstellung.zip"))?;
    let path = download_path.as_path();
    let file = std::fs::File::create(path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zw.start_file("einnahmen.csv", opts)?;
    zw.write_all(detail::income_csv(&income).as_bytes())?;
    zw.start_file("ausgaben.csv", opts)?;
    zw.write_all(detail::expenses_csv(&expenses).as_bytes())?;
    zw.start_file("anlageverzeichnis.csv", opts)?;
    zw.write_all(detail::aveeur_csv(&assets, y).as_bytes())?;
    // Lauf F2c (KB-0031): Abweichung vom Jahresabschluss als Begleitdatei.
    let check = snapshot::check(pool, fiscal_year).await?;
    if let Some(c) = check.as_ref().filter(|c| !c.matches) {
        zw.start_file(SNAPSHOT_NOTE_FILE, opts)?;
        zw.write_all(snapshot::readme_block(c).as_bytes())?;
    }
    zw.finish()?;

    audit_export(
        pool,
        "euer.detail_export",
        fiscal_year,
        json!({
            "file_name": format!("EUER-{fiscal_year}-Einzelaufstellung.zip"),
            "income": income.len(),
            "expenses": expenses.len(),
            "assets": assets.len(),
        }),
        check.as_ref(),
    )
    .await?;

    Ok(EuerDetailExport {
        download: ctx.downloads.register(
            download_id,
            path,
            crate::downloads::DownloadKind::Export,
        )?,
        income_count: income.len(),
        expense_count: expenses.len(),
        asset_count: assets.len(),
        snapshot_check: check,
    })
}

/// Argumente von [`euer_export_elster`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerExportElsterArgs {
    pub fiscal_year: i64,
}

/// Schreibt die ELSTER-Ausfüllhilfe als CSV in den Download-Ordner (Plan W7).
/// Keine Buchung; Pflicht-Audit `euer.elster_export` (Lauf F).
pub async fn euer_export_elster(
    ctx: &Arc<AppCtx>,
    args: EuerExportElsterArgs,
) -> Result<ElsterExportResult> {
    let EuerExportElsterArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    let form = build_form_for_year(&pool, fiscal_year as i32).await?;
    // Lauf F2c: Abweichung vom Jahresabschluss als Kommentarzeile in der Datei.
    let check = snapshot::check(&pool, fiscal_year).await?;
    let csv = elster_csv::to_csv_with_notes(&form, &snapshot::csv_note_lines(check.as_ref()));

    // Plan W7: kein Pfad vom Client — Datei im Download-Ordner.
    let (download_id, download_path) = ctx
        .downloads
        .prepare(&format!("EUER-{fiscal_year}-ELSTER.csv"))?;
    let path = download_path.as_path();
    std::fs::write(path, csv.as_bytes())?;

    audit_export(
        &pool,
        "euer.elster_export",
        fiscal_year,
        json!({
            "file_name": format!("EUER-{fiscal_year}-ELSTER.csv"),
            "lines": form.lines.len(),
        }),
        check.as_ref(),
    )
    .await?;

    Ok(ElsterExportResult {
        download: ctx.downloads.register(
            download_id,
            path,
            crate::downloads::DownloadKind::Export,
        )?,
        line_count: form.lines.len(),
        entry_count: form.lines.iter().filter(|l| l.is_entry).count(),
        snapshot_check: check,
    })
}

/// Ergebnis des PDF-Exports „Anlage EÜR {Jahr}".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerPdfExport {
    /// Abruf über `GET /files/download/{id}` (Plan W7).
    pub download: crate::downloads::DownloadTicket,
    pub size_bytes: i64,
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Baut das vollständige EÜR-PDF (Anlage EÜR + Anlageverzeichnis +
/// Einzelaufstellung) als Bytes — gemeinsam genutzt von `euer_export_pdf` und dem
/// Steuerberater-Paket.
async fn build_euer_pdf_bytes(
    paths: &Paths,
    pool: &SqlitePool,
    fiscal_year: i64,
    check: Option<&SnapshotCheck>,
) -> Result<Vec<u8>> {
    let y = fiscal_year as i32;

    let form = build_form_for_year(pool, y).await?;
    let income = euer_repo::income_detail(pool, y).await?;
    let storno = euer_repo::storno_detail(pool, y).await?;
    let expenses = euer_repo::expense_detail(pool, y).await?;
    let disposals = euer_repo::disposal_detail(pool, y).await?;
    let assets = euer_repo::aveeur_items(pool, y).await?;
    let seller = seller_profile::get(pool).await?;
    // R2-002, ADR 0042 Nr. 3: §19 das ganze Jahr (ein Wechsel steht in den
    // Hinweisen der Ausfüllhilfe).
    let is_klein = year_tax_status(pool, y).await?.0;

    let data = euer_pdf_data(&EuerPdfParts {
        fiscal_year,
        today: crate::clock::today_berlin(),
        snapshot_block: snapshot::pdf_block(check),
        is_klein,
        seller: seller.as_ref(),
        form: &form,
        income: &income,
        storno: &storno,
        expenses: &expenses,
        disposals: &disposals,
        assets: &assets,
    });
    let data_json = serde_json::to_string(&data)?;

    let source = templates::load_euer_source(&paths.inputs_dir);
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };

    typst_render::plain_job(&source, &data_json, branding_opt)
        .compile_async()
        .await
}

/// Geladene Bestandteile des EÜR-PDFs.
struct EuerPdfParts<'a> {
    fiscal_year: i64,
    today: chrono::NaiveDate,
    snapshot_block: serde_json::Value,
    is_klein: bool,
    seller: Option<&'a SellerProfileRow>,
    form: &'a ElsterForm,
    income: &'a [IncomeItem],
    storno: &'a [StornoItem],
    expenses: &'a [ExpenseItem],
    disposals: &'a [DisposalItem],
    assets: &'a [AveeurItem],
}

/// Daten des EÜR-PDFs (pure): Beträge formatiert, Datumsangaben als
/// TT.MM.JJJJ (KB-0018), Summe der Einnahmen-Tabelle abzüglich der dort
/// aufgeführten Stornos (KB-0023).
fn euer_pdf_data(p: &EuerPdfParts<'_>) -> serde_json::Value {
    let EuerPdfParts {
        fiscal_year,
        today,
        is_klein,
        seller,
        form,
        income,
        storno,
        expenses,
        disposals,
        assets,
        ..
    } = *p;
    let check_block = p.snapshot_block.clone();

    // KB-0023: Summe der Tabelle = Zahlungen abzüglich der dort aufgeführten
    // Storno-Erstattungen.
    let income_sum: i64 = income.iter().map(|i| i.amount_cents).sum::<i64>()
        - storno.iter().map(|s| s.refunded_cents).sum::<i64>();
    let expense_sum: i64 = expenses.iter().map(|e| e.gross_cents).sum();

    json!({
        "year": fiscal_year,
        // Lauf F2c (KB-0031): Hinweiskasten bei Abweichung vom Jahresabschluss.
        "snapshotMismatch": check_block,
        // KB-0018: Datumsangaben im PDF als TT.MM.JJJJ.
        "generatedAt": today.format("%d.%m.%Y").to_string(),
        "isKleinunternehmer": is_klein,
        "kleinunternehmerHinweis": if is_klein { Some(HINWEIS_TEXT) } else { None },
        "seller": {
            "name": seller.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            "street": seller.as_ref().map(|s| s.street.clone()).unwrap_or_default(),
            "postalCode": seller.as_ref().map(|s| s.postal_code.clone()).unwrap_or_default(),
            "city": seller.as_ref().map(|s| s.city.clone()).unwrap_or_default(),
            "taxNumber": seller.as_ref().and_then(|s| s.tax_number.clone()),
            "vatId": seller.as_ref().and_then(|s| s.vat_id.clone()),
            "email": seller.as_ref().map(|s| s.email.clone()).unwrap_or_default(),
        },
        "form": {
            "lines": form.lines.iter().map(|l| json!({
                "zeile": l.zeile.to_string(),
                "kennzahl": l.kennzahl.map(|k| k.to_string()).unwrap_or_default(),
                "bezeichnung": l.bezeichnung,
                "betrag": detail::eur_grouped(l.amount_cents),
                "isEntry": l.is_entry,
            })).collect::<Vec<_>>(),
            "incomeTotal": detail::eur_grouped(form.income_total_cents),
            "expenseTotal": detail::eur_grouped(form.expense_total_cents),
            "surplus": detail::eur_grouped(form.surplus_cents),
            "formYear": form.form_year,
            "notes": form.notes,
        },
        "assets": assets.iter().map(|a| json!({
            "assetNumber": a.asset_number,
            "label": a.label,
            "acquisitionDate": de_date(&a.acquisition_date),
            "acquisitionCost": detail::eur_grouped(a.acquisition_cost_cents),
            "method": detail::method_label(&a.depreciation_method),
            "afaYear": detail::eur_grouped(a.afa_year_cents),
            "bookValueEnd": detail::eur_grouped(a.book_value_end_cents),
            "disposalNote": if a.disposed_in_year {
                format!("Abgang {}", de_date(a.disposal_date.as_deref().unwrap_or_default()))
            } else {
                String::new()
            },
        })).collect::<Vec<_>>(),
        "income": income.iter().map(|i| json!({
            "paidDate": de_date(&i.paid_date),
            "invoiceNumber": i.invoice_number,
            "customer": i.customer,
            "description": i.description,
            "amount": detail::eur_grouped(i.amount_cents),
        })).collect::<Vec<_>>(),
        "storno": storno.iter().map(|s| json!({
            "stornoDate": de_date(&s.storno_date),
            "stornoNumber": s.storno_number,
            "originalNumber": s.original_number,
            "amount": detail::eur_grouped(s.refunded_cents),
        })).collect::<Vec<_>>(),
        "incomeSum": detail::eur_grouped(income_sum),
        "expenses": expenses.iter().map(|e| json!({
            "paidDate": de_date(&e.paid_date),
            "expenseNumber": e.expense_number,
            "vendor": e.vendor,
            "category": detail::category_label(&e.category),
            "description": e.description,
            "amount": detail::eur_grouped(e.gross_cents),
        })).collect::<Vec<_>>(),
        "expenseSum": detail::eur_grouped(expense_sum),
        "disposals": disposals.iter().map(|d| json!({
            "disposalDate": de_date(&d.disposal_date),
            "assetNumber": d.asset_number,
            "label": d.label,
            "proceeds": detail::eur_grouped(d.proceeds_cents),
            "residual": detail::eur_grouped(d.residual_book_value_cents),
            "gainLoss": detail::eur_grouped(d.gain_loss_cents),
        })).collect::<Vec<_>>(),
    })
}

/// Argumente von [`euer_export_pdf`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerExportPdfArgs {
    pub fiscal_year: i64,
}

/// Erzeugt das vollständige EÜR-Dokument als PDF im Download-Ordner. Keine
/// Buchung; Pflicht-Audit `euer.pdf_export` (Lauf F).
pub async fn euer_export_pdf(ctx: &Arc<AppCtx>, args: EuerExportPdfArgs) -> Result<EuerPdfExport> {
    let EuerExportPdfArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    let check = snapshot::check(&pool, fiscal_year).await?;
    let pdf_bytes = build_euer_pdf_bytes(&ctx.paths, &pool, fiscal_year, check.as_ref()).await?;
    // Plan W7: kein Pfad vom Client — Datei im Download-Ordner.
    let (download_id, download_path) = ctx.downloads.prepare(&format!("EUER-{fiscal_year}.pdf"))?;
    let path = download_path.as_path();
    std::fs::write(path, &pdf_bytes)?;
    audit_export(
        &pool,
        "euer.pdf_export",
        fiscal_year,
        json!({
            "file_name": format!("EUER-{fiscal_year}.pdf"),
            "bytes": pdf_bytes.len(),
        }),
        check.as_ref(),
    )
    .await?;
    Ok(EuerPdfExport {
        download: ctx.downloads.register(
            download_id,
            path,
            crate::downloads::DownloadKind::Export,
        )?,
        size_bytes: pdf_bytes.len() as i64,
        snapshot_check: check,
    })
}

/// Ergebnis des DATEV-Buchungsstapel-Exports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatevExport {
    /// Abruf über `GET /files/download/{id}` (Plan W7).
    pub download: crate::downloads::DownloadTicket,
    pub booking_count: usize,
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Argumente von [`euer_export_datev`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerExportDatevArgs {
    pub fiscal_year: i64,
    pub skr: String,
    /// Beraternummer für den DATEV-Kopf (optional, 1001–9999999).
    #[serde(default)]
    pub consultant_number: Option<u32>,
    /// Mandantennummer für den DATEV-Kopf (optional, 1–99999).
    #[serde(default)]
    pub client_number: Option<u32>,
}

/// Lädt die Einzelposten eines Jahres und baut den DATEV-Buchungsstapel
/// (gemeinsam für DATEV-Export und Steuerberater-Paket).
async fn build_datev_bytes(
    pool: &SqlitePool,
    fiscal_year: i32,
    skr: Skr,
    consultant_number: Option<u32>,
    client_number: Option<u32>,
) -> Result<(Vec<u8>, usize)> {
    datev_csv::validate_header_numbers(consultant_number, client_number)
        .map_err(crate::error::Error::Domain)?;
    let income = euer_repo::income_detail(pool, fiscal_year).await?;
    let storno = euer_repo::storno_detail(pool, fiscal_year).await?;
    let expenses = euer_repo::expense_detail(pool, fiscal_year).await?;
    let capitalized = euer_repo::capitalized_expense_detail(pool, fiscal_year).await?;
    let disposals = euer_repo::disposal_detail(pool, fiscal_year).await?;
    let assets = euer_repo::aveeur_items(pool, fiscal_year).await?;
    // R2-009: Privatbewegungen gehören in den DATEV-Stapel (EÜR-neutral, aber
    // für die Abstimmung der Geldkonten beim STB nötig).
    let private_movements = euer_repo::private_movement_detail(pool, fiscal_year).await?;
    // Lauf F8d: Die Kosten tragen ihren §19-Status bei Leistungsbezug selbst.
    let bookings = datev_csv::build_bookings(
        skr,
        fiscal_year,
        &DatevInputs {
            income: &income,
            storno: &storno,
            expenses: &expenses,
            capitalized: &capitalized,
            disposals: &disposals,
            assets: &assets,
            private_movements: &private_movements,
        },
    );
    let header = DatevHeader {
        fiscal_year,
        skr,
        generated_at: crate::clock::now_berlin()
            .format("%Y%m%d%H%M%S%3f")
            .to_string(),
        consultant_number,
        client_number,
    };
    Ok((datev_csv::to_datev(&header, &bookings), bookings.len()))
}

/// Erzeugt den DATEV-Buchungsstapel (EXTF) eines Geschäftsjahres für die
/// Steuerberater-Übergabe und schreibt ihn in den Download-Ordner (Plan W7). `skr` =
/// "SKR03" (Default) oder "SKR04". Keine Buchung; Pflicht-Audit
/// `euer.datev_export` (Lauf F).
pub async fn euer_export_datev(
    ctx: &Arc<AppCtx>,
    args: EuerExportDatevArgs,
) -> Result<DatevExport> {
    let EuerExportDatevArgs {
        fiscal_year,
        skr,
        consultant_number,
        client_number,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let skr_enum = Skr::from_code(&skr);
    let (bytes, booking_count) = build_datev_bytes(
        pool,
        fiscal_year as i32,
        skr_enum,
        consultant_number,
        client_number,
    )
    .await?;

    // Plan W7: kein Pfad vom Client — Datei im Download-Ordner.
    let (download_id, download_path) = ctx
        .downloads
        .prepare(&format!("DATEV-{fiscal_year}-{skr}.csv"))?;
    let path = download_path.as_path();
    std::fs::write(path, &bytes)?;

    let check = snapshot::check(pool, fiscal_year).await?;
    audit_export(
        pool,
        "euer.datev_export",
        fiscal_year,
        json!({
            "file_name": format!("DATEV-{fiscal_year}-{skr}.csv"),
            "skr": skr_enum.code(),
            "bookings": booking_count,
        }),
        check.as_ref(),
    )
    .await?;

    Ok(DatevExport {
        download: ctx.downloads.register(
            download_id,
            path,
            crate::downloads::DownloadKind::Export,
        )?,
        booking_count,
        snapshot_check: check,
    })
}

/// Ergebnis des Steuerberater-Paket-Exports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StbExport {
    /// Abruf über `GET /files/download/{id}` (Plan W7).
    pub download: crate::downloads::DownloadTicket,
    pub file_count: usize,
    pub snapshot_check: Option<SnapshotCheck>,
}

/// Argumente von [`euer_export_stb_zip`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerExportStbZipArgs {
    pub fiscal_year: i64,
    pub skr: String,
    /// Beraternummer für den DATEV-Kopf (optional, 1001–9999999).
    #[serde(default)]
    pub consultant_number: Option<u32>,
    /// Mandantennummer für den DATEV-Kopf (optional, 1–99999).
    #[serde(default)]
    pub client_number: Option<u32>,
}

/// Erzeugt das **Steuerberater-Paket** als ZIP: Deckblatt-PDF, vollständiges
/// EÜR-PDF, DATEV-Buchungsstapel, ELSTER-Ausfüllhilfe, Einzelaufstellungs-CSVs,
/// Anlageverzeichnis, Mandanten-Stammdaten, Original-Belege (PDF/XML aus dem
/// write-once-Archiv), SHA-256-Manifest und LIESMICH.txt mit §19-Klausel und
/// Disclaimer. `skr` = "SKR03" (Default) / "SKR04". Keine Buchung — schreibt
/// genau einen `euer.stb_export`-Audit-Eintrag (bei einer Abweichung vom
/// Snapshot zusätzlich `euer.snapshot_mismatch`).
///
/// Alle Dateien liegen unter dem ZIP-Top-Level-Ordner `EUER-{year}/` (R2-018).
pub async fn euer_export_stb_zip(
    ctx: &Arc<AppCtx>,
    args: EuerExportStbZipArgs,
) -> Result<StbExport> {
    let EuerExportStbZipArgs {
        fiscal_year,
        skr,
        consultant_number,
        client_number,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let y = fiscal_year as i32;

    // KB-0031: Abgleich mit dem Festschreibungs-Snapshot (LIESMICH, PDF, ELSTER).
    let check = snapshot::check(pool, fiscal_year).await?;
    // EÜR-PDF (gemeinsamer Helfer).
    let euer_pdf = build_euer_pdf_bytes(&ctx.paths, pool, fiscal_year, check.as_ref()).await?;

    // Bewegungen + Profil laden (für DATEV, Detail-CSVs, Deckblatt, Stammdaten).
    let form = build_form_for_year(pool, y).await?;
    let income = euer_repo::income_detail(pool, y).await?;
    let expenses = euer_repo::expense_detail(pool, y).await?;
    let assets = euer_repo::aveeur_items(pool, y).await?;
    let seller = seller_profile::get(pool).await?;
    // R2-002, ADR 0042 Nr. 3: §19 das ganze Jahr (ein Wechsel steht in den
    // Hinweisen der Ausfüllhilfe).
    let is_klein = year_tax_status(pool, y).await?.0;

    // DATEV-Buchungsstapel.
    let skr_enum = Skr::from_code(&skr);
    let (datev_bytes, _) =
        build_datev_bytes(pool, y, skr_enum, consultant_number, client_number).await?;

    // ELSTER-Ausfüllhilfe (R2-015).
    let elster_csv_bytes =
        elster_csv::to_csv_with_notes(&form, &snapshot::csv_note_lines(check.as_ref()))
            .into_bytes();

    // Einzelaufstellungs-CSVs.
    let income_csv = detail::income_csv(&income);
    let expenses_csv = detail::expenses_csv(&expenses);
    let aveeur_csv = detail::aveeur_csv(&assets, y);

    // Mandanten-Stammdaten als JSON.
    let stammdaten = json!({
        "fiscalYear": fiscal_year,
        "isKleinunternehmer": is_klein,
        "name": seller.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
        "legalForm": seller.as_ref().and_then(|s| s.legal_form.clone()),
        "street": seller.as_ref().map(|s| s.street.clone()).unwrap_or_default(),
        "postalCode": seller.as_ref().map(|s| s.postal_code.clone()).unwrap_or_default(),
        "city": seller.as_ref().map(|s| s.city.clone()).unwrap_or_default(),
        "countryCode": seller.as_ref().map(|s| s.country_code.clone()).unwrap_or_default(),
        "taxNumber": seller.as_ref().and_then(|s| s.tax_number.clone()),
        "vatId": seller.as_ref().and_then(|s| s.vat_id.clone()),
        "email": seller.as_ref().map(|s| s.email.clone()).unwrap_or_default(),
        "phone": seller.as_ref().and_then(|s| s.phone.clone()),
        "iban": seller.as_ref().and_then(|s| s.iban.clone()),
        "bic": seller.as_ref().and_then(|s| s.bic.clone()),
    });
    let stammdaten_json = serde_json::to_string_pretty(&stammdaten)?;

    // Deckblatt-PDF (R2-019: §19-Klausel + Disclaimer in den Cover-Daten).
    let cover_data = json!({
        "year": fiscal_year,
        "generatedAt": crate::clock::today_berlin().format("%d.%m.%Y").to_string(),
        "skr": skr_enum.code(),
        "isKleinunternehmer": is_klein,
        "kleinunternehmerHinweis": if is_klein { Some(HINWEIS_TEXT) } else { None },
        "disclaimer": "Klein.Buch ist ein Werkzeug, kein Steuerberater. Diese Auswertung ist eine technische Aufbereitung der erfassten Belege; die steuerliche Würdigung obliegt dem Steuerberater.",
        "incomeTotal": detail::eur_grouped(form.income_total_cents),
        "expenseTotal": detail::eur_grouped(form.expense_total_cents),
        "surplus": detail::eur_grouped(form.surplus_cents),
        "seller": {
            "name": seller.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
            "street": seller.as_ref().map(|s| s.street.clone()).unwrap_or_default(),
            "postalCode": seller.as_ref().map(|s| s.postal_code.clone()).unwrap_or_default(),
            "city": seller.as_ref().map(|s| s.city.clone()).unwrap_or_default(),
            "taxNumber": seller.as_ref().and_then(|s| s.tax_number.clone()),
            "vatId": seller.as_ref().and_then(|s| s.vat_id.clone()),
            "email": seller.as_ref().map(|s| s.email.clone()).unwrap_or_default(),
        },
        "contents": [
            "00-deckblatt.pdf — dieses Deckblatt",
            "LIESMICH.txt — §19-Klausel, Disclaimer, Inhaltsverzeichnis",
            format!("anlage-euer-{y}.pdf — Anlage EÜR, Anlageverzeichnis, Einzelaufstellung"),
            format!("datev-buchungsstapel.csv — DATEV-Buchungsstapel ({})", skr_enum.code()),
            "elster-ausfuellhilfe.csv — ELSTER-Anlage-EÜR-Felder (Eintipphilfe)",
            "einnahmen.csv / ausgaben.csv / anlageverzeichnis.csv — Einzelaufstellung",
            format!("{UNISSUED_NUMBERS_FILE} — Rechnungs- und Storno-Nummern ohne festgeschriebenen Beleg (verworfen oder offen)"),
            "stammdaten.json — Mandanten-Stammdaten".to_string(),
            "belege/rechnungen/ — Originale (PDF + XRechnung-XML) der festgeschriebenen Rechnungen",
            "belege/kosten/ — Originale der festgeschriebenen Kostenbelege",
            "manifest.json — SHA-256-Hash über jede Datei (Tamper-Detection beim Steuerberater)",
        ],
    });
    let cover_json = serde_json::to_string(&cover_data)?;

    let paths = ctx.paths.clone();
    let cover_source = templates::load_cover_source(&paths.inputs_dir);
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };
    let cover_pdf = typst_render::plain_job(&cover_source, &cover_json, branding_opt)
        .compile_async()
        .await?;

    // LIESMICH.txt (R2-019); KB-0031: Abweichung vom Snapshot vorangestellt;
    // F2c: nicht festgeschriebene Nummern des Jahres.
    let unissued = euer_repo::unissued_numbers_for_year(pool, y).await?;
    let mut liesmich = build_stb_readme(
        fiscal_year,
        is_klein,
        form.status_change.as_deref(),
        skr_enum,
        &unissued,
        consultant_number.is_some() && client_number.is_some(),
        &form.notes,
    );
    if let Some(c) = check.as_ref().filter(|c| !c.matches) {
        liesmich = format!("{}{liesmich}", snapshot::readme_block(c));
    }

    // ZIP zusammenstellen.
    // Plan W7: kein Pfad vom Client — Datei im Download-Ordner.
    let (download_id, download_path) = ctx
        .downloads
        .prepare(&format!("Steuerberater-{fiscal_year}.zip"))?;
    let path = download_path.as_path();
    let file = std::fs::File::create(path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Helper-Sammler: jede Datei einmal ins ZIP, SHA-256 fürs Manifest.
    // (R2-017 — Tamper-Detection-Manifest, R2-018 — EUER-{year}/-Wurzelprefix.)
    let root = format!("EUER-{y}/");
    let mut manifest: Vec<serde_json::Value> = Vec::new();
    let mut file_count: usize = 0;
    {
        use std::io::Write;
        let mut add =
            |zw: &mut zip::ZipWriter<std::fs::File>, name: &str, bytes: &[u8]| -> Result<()> {
                let full = format!("{root}{name}");
                zw.start_file(&full, opts)?;
                zw.write_all(bytes)?;
                let hex_str = crate::hash::sha256_hex(bytes);
                manifest.push(json!({
                    "name": full,
                    "sha256": hex_str,
                    "bytes": bytes.len(),
                }));
                file_count += 1;
                Ok(())
            };

        add(&mut zw, "00-deckblatt.pdf", &cover_pdf)?;
        add(&mut zw, "LIESMICH.txt", liesmich.as_bytes())?;
        add(&mut zw, &format!("anlage-euer-{y}.pdf"), &euer_pdf)?;
        add(&mut zw, "datev-buchungsstapel.csv", &datev_bytes)?;
        add(&mut zw, "elster-ausfuellhilfe.csv", &elster_csv_bytes)?;
        add(&mut zw, "einnahmen.csv", income_csv.as_bytes())?;
        add(&mut zw, "ausgaben.csv", expenses_csv.as_bytes())?;
        add(&mut zw, "anlageverzeichnis.csv", aveeur_csv.as_bytes())?;
        add(
            &mut zw,
            UNISSUED_NUMBERS_FILE,
            detail::unissued_numbers_csv(&unissued).as_bytes(),
        )?;
        add(&mut zw, "stammdaten.json", stammdaten_json.as_bytes())?;

        // R2-014: Original-Belege aus dem write-once-Archiv.
        // Tamper-Detection bleibt aktiv (`archive::read_and_verify` hashed neu);
        // bei Hash-Mismatch wird der einzelne Beleg übersprungen + im Log
        // gemeldet, damit ein einzelner kaputter Beleg den STB-Export nicht
        // komplett kippt.
        let invoice_archives = euer_repo::invoice_archives_for_year(pool, y).await?;
        for (number, pdf_id, xml_id) in invoice_archives {
            if let Some(id) = pdf_id.as_deref() {
                match archive::read_and_verify(pool, id).await {
                    Ok(bytes) => {
                        add(&mut zw, &format!("belege/rechnungen/{number}.pdf"), &bytes)?;
                    }
                    Err(e) => tracing::warn!(
                        "STB-Export: Rechnung {number} (pdf) konnte nicht beigelegt werden — {e}"
                    ),
                }
            }
            if let Some(id) = xml_id.as_deref() {
                match archive::read_and_verify(pool, id).await {
                    Ok(bytes) => {
                        add(&mut zw, &format!("belege/rechnungen/{number}.xml"), &bytes)?;
                    }
                    Err(e) => tracing::warn!(
                        "STB-Export: Rechnung {number} (xml) konnte nicht beigelegt werden — {e}"
                    ),
                }
            }
        }
        let expense_archives = euer_repo::expense_archives_for_year(pool, y).await?;
        for (number, receipt_id) in expense_archives {
            if let Some(id) = receipt_id.as_deref() {
                match archive::read_and_verify(pool, id).await {
                    Ok(bytes) => {
                        // Datei-Endung aus den Archiv-Metadaten zu raten ist
                        // hier overkill — wir packen sie als `<nummer>.bin` mit
                        // dem Original-Bytes; STB öffnet das anhand des
                        // Mime-Headers. Saubere Endung wäre eine R6-Iteration.
                        add(&mut zw, &format!("belege/kosten/{number}.bin"), &bytes)?;
                    }
                    Err(e) => tracing::warn!(
                        "STB-Export: Kostenbeleg {number} konnte nicht beigelegt werden — {e}"
                    ),
                }
            }
        }

        // Manifest am Ende (selbst nicht im Manifest enthalten — sonst
        // wäre der Hash zirkulär).
        let manifest_json = serde_json::to_string_pretty(&json!({
            "fiscalYear": fiscal_year,
            "generatedAt": crate::clock::now_berlin().to_rfc3339(),
            "algorithm": "SHA-256",
            "files": manifest,
        }))?;
        let manifest_path = format!("{root}manifest.json");
        zw.start_file(&manifest_path, opts)?;
        zw.write_all(manifest_json.as_bytes())?;
        file_count += 1;
    }
    zw.finish()?;

    // R2-016 — Audit-Eintrag (DSGVO Art. 5(2) Rechenschaftspflicht +
    // GoBD-Audit-Trail über Daten-Auslieferungen).
    audit_export(
        pool,
        "euer.stb_export",
        fiscal_year,
        json!({
            "file_name": format!("Steuerberater-{fiscal_year}.zip"),
            "skr": skr_enum.code(),
            "file_count": file_count,
        }),
        check.as_ref(),
    )
    .await?;

    Ok(StbExport {
        download: ctx.downloads.register(
            download_id,
            path,
            crate::downloads::DownloadKind::Export,
        )?,
        file_count,
        snapshot_check: check,
    })
}

/// LIESMICH-Inhalt für den STB-Export (R2-019). §19-Klausel + Disclaimer +
/// Datei-Inhalts-Übersicht. Plain-ASCII (UTF-8), damit auch ältere Editoren
/// auf STB-Seite das problemlos öffnen.
/// Begleitdatei der Einzelaufstellung bei einer Abweichung vom
/// Jahresabschluss (Lauf F2c, KB-0031).
const SNAPSHOT_NOTE_FILE: &str = "HINWEIS-ABWEICHUNG-JAHRESABSCHLUSS.txt";

/// Liste der nie festgeschriebenen Rechnungs-/Storno-Nummern im
/// Steuerberater-Paket (Lauf F2c).
const UNISSUED_NUMBERS_FILE: &str = "nicht-festgeschriebene-nummern.csv";

fn build_stb_readme(
    fiscal_year: i64,
    is_klein: bool,
    status_change: Option<&str>,
    skr: Skr,
    unissued: &[detail::UnissuedNumberItem],
    datev_numbers_set: bool,
    elster_notes: &[String],
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Klein.Buch — Steuerberater-Paket für das Geschäftsjahr {fiscal_year}\n"
    ));
    s.push_str("===============================================================\n\n");
    if is_klein {
        s.push_str("§19 UStG (Kleinunternehmer-Regelung):\n");
        s.push_str(&format!("    {HINWEIS_TEXT}\n\n"));
    } else if let Some(change) = status_change {
        // ADR 0042 Nr. 3: Wechsel im Jahr.
        s.push_str(&format!("Steuerstatus: Wechsel im Jahr — {change}.\n\n"));
    } else {
        s.push_str("Steuerstatus: Regelbesteuerung.\n\n");
    }
    s.push_str("Disclaimer:\n");
    s.push_str("    Klein.Buch ist ein Werkzeug, kein Steuerberater. Diese\n");
    s.push_str("    Auswertung ist eine technische Aufbereitung der erfassten\n");
    s.push_str("    Belege; die steuerliche Würdigung obliegt dem Steuerberater.\n\n");
    s.push_str("Inhalt des Pakets:\n");
    s.push_str("    00-deckblatt.pdf          — Deckblatt mit Eckwerten\n");
    s.push_str("    LIESMICH.txt              — diese Datei\n");
    s.push_str(&format!(
        "    anlage-euer-{fiscal_year}.pdf  — Anlage EÜR + Anlageverzeichnis + Einzelaufstellung\n"
    ));
    s.push_str(&format!(
        "    datev-buchungsstapel.csv  — DATEV-Buchungsstapel ({})\n",
        skr.code()
    ));
    s.push_str("    elster-ausfuellhilfe.csv  — ELSTER-Anlage-EÜR-Eintipphilfe\n");
    s.push_str("    einnahmen.csv             — Einzelne Zahlungseingänge (Cash-Basis)\n");
    s.push_str("    ausgaben.csv              — Einzelne Kostenbelege\n");
    s.push_str("    anlageverzeichnis.csv    — AVEÜR (Anlagen + AfA)\n");
    s.push_str(&format!(
        "    {UNISSUED_NUMBERS_FILE} — Nummern ohne festgeschriebenen Beleg\n"
    ));
    s.push_str("    stammdaten.json           — Mandanten-Stammdaten\n");
    s.push_str("    belege/rechnungen/        — Original-Rechnungen (PDF + XRechnung-XML)\n");
    s.push_str("    belege/kosten/            — Original-Kostenbelege\n");
    s.push_str("    manifest.json             — SHA-256-Hash je Datei (Tamper-Detection)\n\n");
    // Lauf F5b (R2-011, R2-S2-DATEV-HEADER, R2-S2-DATEV-MAPPING): Grenzen des
    // DATEV-Stapels, die der Steuerberater beim Import kennen muss.
    s.push_str("DATEV-Buchungsstapel:\n");
    s.push_str("    Die Konten sind ein Vorschlag nach dem gewählten Kontenrahmen und\n");
    s.push_str("    vor der Verbuchung zu prüfen. Geldkonto je hinterlegtem Zahlungskonto:\n");
    s.push_str("    Bank 1200/1800, Kasse 1000/1600, PayPal/Stripe/sonstige 1360/1460\n");
    s.push_str("    (Geldtransit). Zahlungen ohne hinterlegtes Konto laufen über Bank.\n");
    // ADR 0042 Nr. 9: Gegenbuchungen und Doppelimport.
    s.push_str("    Eine stornierte Zahlung (Fehlbuchung) steht als Gegenbuchung mit\n");
    s.push_str("    umgekehrtem Soll/Haben, Belegdatum und Belegfeld 1 der Zahlung und\n");
    s.push_str("    dem Buchungstext \"Storno Zahlung ...\". Der Stapel enthält immer das\n");
    s.push_str("    ganze Jahr; wurde ein früherer Stapel schon importiert, nicht doppelt\n");
    s.push_str("    importieren.\n");
    if datev_numbers_set {
        s.push_str("    Berater- und Mandantennummer stehen im Kopf der Datei.\n\n");
    } else {
        s.push_str("    Berater- und Mandantennummer sind leer; beim Import über die\n");
        s.push_str("    Stapelverarbeitung gilt der geöffnete Mandant.\n\n");
    }
    if !elster_notes.is_empty() {
        s.push_str("Hinweise zur ELSTER-Ausfüllhilfe:\n");
        for n in elster_notes {
            s.push_str(&format!("    - {n}\n"));
        }
        s.push('\n');
    }
    s.push_str("Tamper-Detection:\n");
    s.push_str("    manifest.json enthält pro Datei den SHA-256-Hash zum\n");
    s.push_str("    Zeitpunkt des Exports. Beim Empfang kann der Steuerberater\n");
    s.push_str("    jede Datei gegen ihren Hash prüfen (z. B. mit `sha256sum`),\n");
    s.push_str("    um eine Manipulation auf dem Transportweg auszuschließen.\n\n");
    // Lauf F2c: Lücken in der Nummernfolge der beigelegten Belege erklären
    // (GoBD Rz. 58 ff.: eine Lücke ist zulässig, wenn sie dokumentiert ist).
    s.push_str("Nummernfolge der Rechnungen und Stornos:\n");
    let voided = unissued.iter().filter(|u| u.voided_at.is_some()).count();
    let open = unissued.len() - voided;
    if unissued.is_empty() {
        s.push_str("    Jede vergebene Rechnungs- und Storno-Nummer dieses Jahres gehört\n");
        s.push_str("    zu einem festgeschriebenen Beleg in belege/rechnungen/.\n");
    } else {
        s.push_str(&format!(
            "    {} Nummer(n) dieses Jahres haben keinen festgeschriebenen Beleg:\n",
            unissued.len()
        ));
        s.push_str(&format!(
            "    {voided} verworfene(r) Entwurf/Entwürfe (nie ausgestellt, mit Zeitpunkt\n"
        ));
        s.push_str(&format!(
            "    und Grund des Verwerfens) und {open} noch offene(r) Entwurf/Entwürfe.\n"
        ));
        s.push_str(&format!(
            "    Einzelheiten: {UNISSUED_NUMBERS_FILE}. Verworfene Nummern werden nie\n"
        ));
        s.push_str("    neu vergeben; die Lücke ist im Audit-Protokoll dokumentiert.\n");
    }
    s
}

/// Wie viele aktive Anlagen für das Geschäftsjahr noch keine gebuchte AfA haben.
/// `pending_count > 0` ⇒ der EÜR-Export würde Abschreibungen auslassen, bis sie
/// nachgebucht werden (Safeguard auf der Export-Seite).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AfaPending {
    pub fiscal_year: i64,
    pub pending_count: i64,
}

/// Argumente von [`euer_afa_pending`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerAfaPendingArgs {
    pub fiscal_year: i64,
}

pub async fn euer_afa_pending(ctx: &Arc<AppCtx>, args: EuerAfaPendingArgs) -> Result<AfaPending> {
    let EuerAfaPendingArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    let pending_count = euer_repo::afa_pending_count(&pool, fiscal_year as i32).await?;
    Ok(AfaPending {
        fiscal_year,
        pending_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::euer::aggregate::EuerReport;
    use crate::euer::elster_csv::AfaSplit;

    fn report() -> EuerReport {
        EuerReport {
            fiscal_year: 2026,
            invoice_income_cents: 100_000,
            storno_refunds_cents: 30_000,
            disposal_proceeds_cents: 0,
            total_income_cents: 70_000,
            expenses_by_category: Vec::new(),
            expenses_total_cents: 2_000,
            depreciation_total_cents: 0,
            disposal_book_value_cents: 0,
            total_expenses_cents: 2_000,
            disposal_gain_loss_cents: 0,
            surplus_cents: 68_000,
        }
    }

    #[allow(clippy::type_complexity)]
    fn sample() -> (
        ElsterForm,
        Vec<IncomeItem>,
        Vec<StornoItem>,
        Vec<ExpenseItem>,
        Vec<DisposalItem>,
        Vec<AveeurItem>,
    ) {
        let income = vec![IncomeItem {
            paid_date: "2026-03-01".into(),
            invoice_number: "RE-2026-0001".into(),
            customer: "ACME".into(),
            description: "Beratung".into(),
            amount_cents: 100_000,
            is_kleinunternehmer: true,
            ..Default::default()
        }];
        let storno = vec![StornoItem {
            storno_date: "2026-04-02".into(),
            storno_number: "ST-2026-0001".into(),
            original_number: "RE-2025-0009".into(),
            refunded_cents: 30_000,
            is_kleinunternehmer: true,
            ..Default::default()
        }];
        let expenses = vec![ExpenseItem {
            paid_date: "2026-05-03".into(),
            expense_number: "KO-2026-0001".into(),
            vendor: "Shop".into(),
            category: "office".into(),
            description: "Papier".into(),
            gross_cents: 2_000,
            net_cents: 2_000,
            ..Default::default()
        }];
        let disposals = vec![DisposalItem {
            disposal_date: "2026-06-04".into(),
            asset_number: "AV-2024-0001".into(),
            label: "Drucker".into(),
            depreciation_method: "linear".into(),
            ..Default::default()
        }];
        let assets = vec![AveeurItem {
            asset_number: "AV-2024-0001".into(),
            label: "Drucker".into(),
            acquisition_date: "2024-01-15".into(),
            acquisition_cost_cents: 50_000,
            depreciation_method: "linear".into(),
            useful_life_years: Some(3.0),
            business_share_percent: 100.0,
            afa_year_cents: 0,
            book_value_start_cents: 0,
            book_value_end_cents: 0,
            disposed_in_year: true,
            disposal_date: Some("2026-06-04".into()),
            disposal_proceeds_cents: Some(0),
        }];
        let form = elster_csv::build_form(&ElsterInputs {
            report: &report(),
            afa: &AfaSplit::default(),
            income: &income,
            storno: &storno,
            expenses: &expenses,
            capitalized: &[],
            is_kleinunternehmer: true,
            status_change: None,
        });
        (form, income, storno, expenses, disposals, assets)
    }

    /// KB-0018 / KB-0023: Datumsangaben im EÜR-PDF als TT.MM.JJJJ, die Summe
    /// der Einnahmen-Tabelle zieht die dort aufgeführten Stornos ab; im
    /// gesetzten Dokument stehen Datum, Summe und die Kennzahl-Spalte.
    #[test]
    fn euer_pdf_uses_german_dates_and_net_income_sum() {
        let (form, income, storno, expenses, disposals, assets) = sample();
        let data = euer_pdf_data(&EuerPdfParts {
            fiscal_year: 2026,
            today: chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
            snapshot_block: serde_json::Value::Null,
            is_klein: true,
            seller: None,
            form: &form,
            income: &income,
            storno: &storno,
            expenses: &expenses,
            disposals: &disposals,
            assets: &assets,
        });
        assert_eq!(data["generatedAt"], "25.10.2026");
        assert_eq!(data["income"][0]["paidDate"], "01.03.2026");
        assert_eq!(data["storno"][0]["stornoDate"], "02.04.2026");
        assert_eq!(data["expenses"][0]["paidDate"], "03.05.2026");
        assert_eq!(data["disposals"][0]["disposalDate"], "04.06.2026");
        assert_eq!(data["assets"][0]["acquisitionDate"], "15.01.2024");
        assert_eq!(data["assets"][0]["disposalNote"], "Abgang 04.06.2026");
        assert_eq!(data["incomeSum"], "700,00 €");
        assert_eq!(data["form"]["lines"][0]["kennzahl"], "111");

        let text = crate::pdf::typst_render::laid_out_text(
            templates::DEFAULT_EUER_TEMPLATE,
            &serde_json::to_string(&data).unwrap(),
        );
        let flat: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(flat.contains("Erstelltam25.10.2026"), "{flat}");
        assert!(flat.contains("01.03.2026"));
        assert!(!flat.contains("2026-03-01"));
        assert!(flat.contains("SummeZahlungseingängeabzüglichStorno-Erstattungen:700,00€"));
        assert!(flat.contains("Kz"));
    }

    /// KB-0021: Hinweise der Ausfüllhilfe stehen im PDF.
    #[test]
    fn euer_pdf_shows_form_notes() {
        let (mut form, income, storno, expenses, disposals, assets) = sample();
        form.notes = vec!["Regelbesteuerung: Testhinweis".into()];
        let data = euer_pdf_data(&EuerPdfParts {
            fiscal_year: 2026,
            today: chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
            snapshot_block: serde_json::Value::Null,
            is_klein: false,
            seller: None,
            form: &form,
            income: &income,
            storno: &storno,
            expenses: &expenses,
            disposals: &disposals,
            assets: &assets,
        });
        let text = crate::pdf::typst_render::laid_out_text(
            templates::DEFAULT_EUER_TEMPLATE,
            &serde_json::to_string(&data).unwrap(),
        );
        let flat: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(flat.contains("HinweisezurAusfüllhilfe"));
        assert!(flat.contains("Regelbesteuerung:Testhinweis"));
    }

    /// Referenz-PDF des EÜR-Dokuments für Typst-Updates (siehe `pdf::snapshot`).
    #[test]
    #[ignore = "erzeugt Referenz-PDFs nach KB_PDF_SNAPSHOT_DIR (Typst-Update)"]
    fn pdf_snapshot_euer() {
        let Some(dir) = crate::pdf::snapshot::out_dir() else {
            return;
        };
        let (mut form, income, storno, expenses, disposals, assets) = sample();
        for (name, is_klein) in [("euer", true), ("euer-regel", false)] {
            if !is_klein {
                form.notes = vec!["Regelbesteuerung: Umsatzsteuer gesondert erklären".into()];
            }
            let data = euer_pdf_data(&EuerPdfParts {
                fiscal_year: 2026,
                today: chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
                snapshot_block: serde_json::Value::Null,
                is_klein,
                seller: None,
                form: &form,
                income: &income,
                storno: &storno,
                expenses: &expenses,
                disposals: &disposals,
                assets: &assets,
            });
            let pdf = crate::pdf::typst_render::render_euer(
                templates::DEFAULT_EUER_TEMPLATE,
                &serde_json::to_string(&data).unwrap(),
                None,
            )
            .expect("EÜR-PDF");
            crate::pdf::snapshot::write(&dir, name, &pdf);
        }
    }

    #[test]
    fn stb_readme_names_money_accounts_and_header_numbers() {
        let r = build_stb_readme(
            2026,
            true,
            None,
            Skr::Skr03,
            &[],
            false,
            &["Hinweis A".into()],
        );
        assert!(r.contains("Kasse 1000/1600"));
        assert!(r.contains("Berater- und Mandantennummer sind leer"));
        assert!(r.contains("- Hinweis A"));
        let r = build_stb_readme(2026, true, None, Skr::Skr03, &[], true, &[]);
        assert!(r.contains("Berater- und Mandantennummer stehen im Kopf"));
        assert!(!r.contains("Hinweise zur ELSTER-Ausfüllhilfe"));
    }
}
