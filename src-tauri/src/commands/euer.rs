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
use crate::domain::kleinunternehmer::{self, KleinunternehmerStatus, HINWEIS_TEXT};
use crate::error::Result;
use crate::euer::aggregate::{self, EuerReport};
use crate::euer::datev_csv::{self, DatevHeader, Skr};
use crate::euer::detail::{self, AveeurItem, DisposalItem, ExpenseItem, IncomeItem, StornoItem};
use crate::euer::elster_csv::{self, ElsterForm};
use crate::fiscal_year::snapshot::{self, SnapshotCheck};
use crate::pdf::{templates, typst_render};
use sha2::{Digest, Sha256};

/// Period-aware §19-Status für ein konkretes Geschäftsjahr. Liest das
/// aktuelle `seller_profile` und reicht das (zusammen mit dem Verzichts-
/// Stichtag) durch [`kleinunternehmer::is_active_for_year`]. Default
/// (kein Profil) ist §19 = aktiv, weil Klein.Buch §19-Kleinunternehmer
/// als Zielgruppe hat. Fix: R2-002.
fn klein_status_from_profile(profile: Option<&SellerProfileRow>) -> KleinunternehmerStatus {
    profile
        .map(|p| KleinunternehmerStatus {
            is_kleinunternehmer: p.is_kleinunternehmer == 1,
            waived_since: p
                .waived_paragraph_19_since
                .as_deref()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        })
        .unwrap_or(KleinunternehmerStatus {
            is_kleinunternehmer: true,
            waived_since: None,
        })
}

fn is_klein_for_year(profile: Option<&SellerProfileRow>, fiscal_year: i32) -> bool {
    kleinunternehmer::is_active_for_year(&klein_status_from_profile(profile), fiscal_year)
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

/// Berechnet den EÜR-Report für ein Geschäftsjahr (Cash-Basis).
pub async fn euer_compute_report(
    ctx: &Arc<AppCtx>,
    args: EuerComputeReportArgs,
) -> Result<EuerReport> {
    let EuerComputeReportArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    let inputs = euer_repo::load_inputs(&pool).await?;
    Ok(aggregate::aggregate(fiscal_year as i32, &inputs))
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
/// AfA nach Zeile aufteilen, §19-Status des Verkäuferprofils berücksichtigen.
async fn build_form_for_year(pool: &SqlitePool, fiscal_year: i32) -> Result<ElsterForm> {
    let inputs = euer_repo::load_inputs(pool).await?;
    let report = aggregate::aggregate(fiscal_year, &inputs);
    let afa = euer_repo::depreciation_split_for_year(pool, fiscal_year).await?;
    // R2-002: §19-Status period-aware (Klein→Regel-Wechsel darf das Vor-Jahr
    // nicht rückwirkend in Zeile 15 kippen).
    let profile = seller_profile::get(pool).await?;
    let is_kleinunternehmer = is_klein_for_year(profile.as_ref(), fiscal_year);
    Ok(elster_csv::build_form(&report, &afa, is_kleinunternehmer))
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
    // R2-002: §19-Status period-aware.
    let is_klein = is_klein_for_year(seller.as_ref(), y);

    let income_sum: i64 = income.iter().map(|i| i.amount_cents).sum();
    let expense_sum: i64 = expenses.iter().map(|e| e.gross_cents).sum();

    let data = json!({
        "year": fiscal_year,
        // Lauf F2c (KB-0031): Hinweiskasten bei Abweichung vom Jahresabschluss.
        "snapshotMismatch": snapshot::pdf_block(check),
        "generatedAt": crate::clock::today_berlin().to_string(),
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
                "zeile": if l.zeile == 0 { "—".to_string() } else { l.zeile.to_string() },
                "bezeichnung": l.bezeichnung,
                "betrag": detail::eur_grouped(l.amount_cents),
                "isEntry": l.is_entry,
            })).collect::<Vec<_>>(),
            "incomeTotal": detail::eur_grouped(form.income_total_cents),
            "expenseTotal": detail::eur_grouped(form.expense_total_cents),
            "surplus": detail::eur_grouped(form.surplus_cents),
        },
        "assets": assets.iter().map(|a| json!({
            "assetNumber": a.asset_number,
            "label": a.label,
            "acquisitionDate": a.acquisition_date,
            "acquisitionCost": detail::eur_grouped(a.acquisition_cost_cents),
            "method": detail::method_label(&a.depreciation_method),
            "afaYear": detail::eur_grouped(a.afa_year_cents),
            "bookValueEnd": detail::eur_grouped(a.book_value_end_cents),
            "disposalNote": if a.disposed_in_year {
                format!("Abgang {}", a.disposal_date.clone().unwrap_or_default())
            } else {
                String::new()
            },
        })).collect::<Vec<_>>(),
        "income": income.iter().map(|i| json!({
            "paidDate": i.paid_date,
            "invoiceNumber": i.invoice_number,
            "customer": i.customer,
            "description": i.description,
            "amount": detail::eur_grouped(i.amount_cents),
        })).collect::<Vec<_>>(),
        "storno": storno.iter().map(|s| json!({
            "stornoDate": s.storno_date,
            "stornoNumber": s.storno_number,
            "originalNumber": s.original_number,
            "amount": detail::eur_grouped(s.refunded_cents),
        })).collect::<Vec<_>>(),
        "incomeSum": detail::eur_grouped(income_sum),
        "expenses": expenses.iter().map(|e| json!({
            "paidDate": e.paid_date,
            "expenseNumber": e.expense_number,
            "vendor": e.vendor,
            "category": detail::category_label(&e.category),
            "description": e.description,
            "amount": detail::eur_grouped(e.gross_cents),
        })).collect::<Vec<_>>(),
        "expenseSum": detail::eur_grouped(expense_sum),
        "disposals": disposals.iter().map(|d| json!({
            "disposalDate": d.disposal_date,
            "assetNumber": d.asset_number,
            "label": d.label,
            "proceeds": detail::eur_grouped(d.proceeds_cents),
            "residual": detail::eur_grouped(d.residual_book_value_cents),
            "gainLoss": detail::eur_grouped(d.gain_loss_cents),
        })).collect::<Vec<_>>(),
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
}

/// Erzeugt den DATEV-Buchungsstapel (EXTF) eines Geschäftsjahres für die
/// Steuerberater-Übergabe und schreibt ihn in den Download-Ordner (Plan W7). `skr` =
/// "SKR03" (Default) oder "SKR04". Keine Buchung; Pflicht-Audit
/// `euer.datev_export` (Lauf F).
pub async fn euer_export_datev(
    ctx: &Arc<AppCtx>,
    args: EuerExportDatevArgs,
) -> Result<DatevExport> {
    let EuerExportDatevArgs { fiscal_year, skr } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let y = fiscal_year as i32;

    let income = euer_repo::income_detail(pool, y).await?;
    let storno = euer_repo::storno_detail(pool, y).await?;
    let expenses = euer_repo::expense_detail(pool, y).await?;
    let disposals = euer_repo::disposal_detail(pool, y).await?;
    let assets = euer_repo::aveeur_items(pool, y).await?;
    // R2-009: Privatbewegungen gehören in den DATEV-Stapel (EÜR-neutral, aber
    // für die Bank-Abstimmung beim STB nötig).
    let private_movements = euer_repo::private_movement_detail(pool, y).await?;

    let skr_enum = Skr::from_code(&skr);
    let bookings = datev_csv::build_bookings(
        skr_enum,
        y,
        &income,
        &storno,
        &expenses,
        &disposals,
        &assets,
        &private_movements,
    );
    let header = DatevHeader {
        fiscal_year: y,
        skr: skr_enum,
        generated_at: crate::clock::now_berlin()
            .format("%Y%m%d%H%M%S%3f")
            .to_string(),
    };
    let bytes = datev_csv::to_datev(&header, &bookings);

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
            "bookings": bookings.len(),
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
        booking_count: bookings.len(),
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
    let EuerExportStbZipArgs { fiscal_year, skr } = args;
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
    let storno = euer_repo::storno_detail(pool, y).await?;
    let expenses = euer_repo::expense_detail(pool, y).await?;
    let disposals = euer_repo::disposal_detail(pool, y).await?;
    let assets = euer_repo::aveeur_items(pool, y).await?;
    // R2-009: Privatbewegungen für den STB-DATEV-Stapel.
    let private_movements = euer_repo::private_movement_detail(pool, y).await?;
    let seller = seller_profile::get(pool).await?;
    // R2-002: §19-Status period-aware.
    let is_klein = is_klein_for_year(seller.as_ref(), y);

    // DATEV-Buchungsstapel.
    let skr_enum = Skr::from_code(&skr);
    let bookings = datev_csv::build_bookings(
        skr_enum,
        y,
        &income,
        &storno,
        &expenses,
        &disposals,
        &assets,
        &private_movements,
    );
    let datev_header = DatevHeader {
        fiscal_year: y,
        skr: skr_enum,
        generated_at: crate::clock::now_berlin()
            .format("%Y%m%d%H%M%S%3f")
            .to_string(),
    };
    let datev_bytes = datev_csv::to_datev(&datev_header, &bookings);

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
        "generatedAt": crate::clock::today_berlin().to_string(),
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
    let mut liesmich = build_stb_readme(fiscal_year, is_klein, skr_enum, &unissued);
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
                let mut hasher = Sha256::new();
                hasher.update(bytes);
                let digest = hasher.finalize();
                let hex_str: String = digest.iter().map(|b| format!("{b:02x}")).collect();
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
    skr: Skr,
    unissued: &[detail::UnissuedNumberItem],
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Klein.Buch — Steuerberater-Paket für das Geschäftsjahr {fiscal_year}\n"
    ));
    s.push_str("===============================================================\n\n");
    if is_klein {
        s.push_str("§19 UStG (Kleinunternehmer-Regelung):\n");
        s.push_str(&format!("    {HINWEIS_TEXT}\n\n"));
    } else {
        s.push_str("Steuerstatus: Regelbesteuerung (Verzicht auf §19 UStG).\n\n");
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

/// Argumente von [`euer_reveal_path`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EuerRevealPathArgs {
    pub path: String,
}

/// Zeigt eine exportierte Datei im Datei-Explorer/Finder (markiert sie im
/// enthaltenden Ordner). Gleiches Muster wie `invoices_reveal_pdf`.
pub async fn euer_reveal_path(ctx: &Arc<AppCtx>, args: EuerRevealPathArgs) -> Result<()> {
    let EuerRevealPathArgs { path } = args;

    ctx.reveal(&path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
