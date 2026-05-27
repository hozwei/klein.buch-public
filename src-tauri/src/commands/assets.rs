//! Tauri-Commands für Anlagen (Block 12, Phase 2C).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::asset::validate_asset`]) — Hard-Block.
//! - AfA-Methoden-Auflösung (Nutzungsdauer je Methode) + Start-Restbuchwert
//!   (anteilig Privatanteil).
//! - Lieferanten-/Quell-Kosten-Verknüpfung (inkl. Schutz gegen Doppel-Aktivierung).
//! - Counter-Allokation `AV-{YYYY}-{NNNN}` ([`crate::db::numbering`]).
//! - Persistenz ([`crate::db::repo::assets`]) + Audit + Auto-Backup (best-effort).
//!
//! ## Lebenszyklus
//!
//! Anlegen → (bei Bedarf korrigieren, solange unlocked) → AfA buchen (lockt) →
//! ggf. veräußern. Gelöscht wird nie (GoBD-Hardline).

use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::assets::afa_tabellen::{self, AfaTabellen};
use crate::backup;
use crate::config::Paths;
use crate::db::models::AssetDetail;
use crate::db::numbering;
use crate::db::repo::{assets, audit_log, depreciation as depreciation_repo, expenses};
use crate::domain::asset::{
    self, business_book_value_start_cents, AssetInput, DepreciationMethod, DisposalType,
    MethodSuggestion,
};
use crate::domain::depreciation::{compute_disposal_year_partial, DepreciationAsset};
use crate::domain::numbering::DocType;
use crate::error::{Error, Result};
use crate::fiscal_year::guard;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDisposeArgs {
    pub asset_id: String,
    /// Veräußerungsdatum (YYYY-MM-DD).
    pub disposal_date: NaiveDate,
    /// 'sale' | 'scrap' | 'given_away'.
    pub disposal_type: String,
    /// Verkaufserlös in Cent (nur bei 'sale' relevant; sonst 0).
    pub proceeds_cents: i64,
}

// =============================================================================
// Read
// =============================================================================

#[tauri::command]
pub async fn assets_list(
    pool: State<'_, SqlitePool>,
    filter: Option<assets::ListFilter>,
) -> Result<Vec<crate::db::models::AssetListItem>> {
    assets::list(pool.inner(), &filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn assets_get(pool: State<'_, SqlitePool>, id: String) -> Result<Option<AssetDetail>> {
    assets::get_detail(pool.inner(), &id).await
}

/// Liefert die geladene BMF-AfA-Tabelle (Kategorien + GWG-Grenze) fürs Formular.
#[tauri::command]
pub async fn assets_afa_table(app: AppHandle) -> Result<AfaTabellen> {
    let paths = Paths::from_handle(&app)?;
    afa_tabellen::load(&paths.inputs_dir)
}

/// Schlägt AfA-Methode + Nutzungsdauer vor (PRD §6.17). `expense_category` ist die
/// EÜR-Kategorie der Quell-Kosten (falls vorhanden).
#[tauri::command]
pub async fn assets_suggest_method(
    app: AppHandle,
    expense_category: Option<String>,
    acquisition_cost_cents: i64,
) -> Result<MethodSuggestion> {
    let paths = Paths::from_handle(&app)?;
    let table = afa_tabellen::load(&paths.inputs_dir)?;
    Ok(asset::suggest_method(
        expense_category.as_deref(),
        acquisition_cost_cents,
        table.gwg_threshold_cents,
    ))
}

// =============================================================================
// Create
// =============================================================================

#[tauri::command]
pub async fn assets_create(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    input: AssetInput,
) -> Result<AssetDetail> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;

    // Hard-Block bei Validierungsfehlern.
    if let Err(errs) = asset::validate_asset(&input, today_berlin()) {
        return Err(Error::Domain(format!(
            "Anlage kann nicht angelegt werden: {}",
            errs.iter()
                .map(asset::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    // Quell-Kosten prüfen + Doppel-Aktivierung verhindern.
    if let Some(eid) = input.expense_id.as_deref() {
        let expense = expenses::get(pool, eid)
            .await?
            .ok_or_else(|| Error::Domain(format!("Quell-Kosten nicht gefunden: {eid}")))?;
        if let Some(existing) = expense.capitalized_as_asset_id.as_deref() {
            return Err(Error::Domain(format!(
                "Diese Kosten wurden bereits als Anlage aktiviert ({existing})."
            )));
        }
    }

    let (method_db, useful_life) = resolve_method(&input)?;
    let book_value =
        business_book_value_start_cents(input.acquisition_cost_cents, input.business_share_percent);
    let fiscal_year = input.acquisition_date.year() as i64;

    // R2-022 (GJ-Guard): Anlage darf nicht in ein festgeschriebenes GJ
    // eingebucht werden — sonst würde die AfA dieses Jahres nachträglich
    // den Lock-Snapshot verbiegen.
    guard::ensure_year_open(pool, fiscal_year).await?;

    let asset_number = numbering::next_number(pool, DocType::Asset, fiscal_year as i32).await?;

    let row = assets::create(
        pool,
        &input,
        &asset_number,
        fiscal_year,
        &method_db,
        useful_life,
        book_value,
    )
    .await?;

    // Quell-Kosten mit der Anlage verknüpfen (Sprung-Ziel + Doppel-Schutz).
    if let Some(eid) = input.expense_id.as_deref() {
        expenses::set_capitalized_asset_id(pool, eid, &row.id).await?;
    }

    audit_log::append(
        pool,
        "asset.create",
        "asset",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","cost":{},"method":"{}","business_share":{},"book_value":{}}}"#,
            esc(&asset_number),
            row.acquisition_cost_cents,
            esc(&method_db),
            row.business_share_percent,
            book_value
        )),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "asset.create")
        .await
        .ok();

    assets::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("create: detail-load leer".into()))
}

// =============================================================================
// Update (nur solange unlocked / nicht veräußert)
// =============================================================================

#[tauri::command]
pub async fn assets_update(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    id: String,
    input: AssetInput,
) -> Result<AssetDetail> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;

    if let Err(errs) = asset::validate_asset(&input, today_berlin()) {
        return Err(Error::Domain(format!(
            "Anlage kann nicht geändert werden: {}",
            errs.iter()
                .map(asset::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    // Solange AfA gebucht ist, sind die Stammdaten nicht änderbar (sonst würden die
    // Buchungen inkonsistent). Erst die Abschreibung des offenen GJ zurücksetzen.
    if depreciation_repo::count_for_asset(pool, &id).await? > 0 {
        return Err(Error::Domain(
            "Für diese Anlage ist bereits eine Abschreibung gebucht. Bitte erst die \
             Abschreibung zurücksetzen (offenes Geschäftsjahr), dann bearbeiten."
                .into(),
        ));
    }

    // R2-022 (GJ-Guard): existierender Anschaffungs-GJ und neu gewähltes
    // Anschaffungs-Jahr müssen offen sein — sonst lässt sich rückwirkend
    // in ein festgeschriebenes GJ schreiben.
    let existing = assets::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {id}")))?;
    guard::ensure_year_open(pool, existing.acquisition_fiscal_year).await?;
    guard::ensure_year_open(pool, input.acquisition_date.year() as i64).await?;

    let (method_db, useful_life) = resolve_method(&input)?;
    let book_value =
        business_book_value_start_cents(input.acquisition_cost_cents, input.business_share_percent);

    let row = assets::update(pool, &id, &input, &method_db, useful_life, book_value).await?;

    audit_log::append(
        pool,
        "asset.update",
        "asset",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","cost":{},"method":"{}","business_share":{}}}"#,
            esc(&row.asset_number),
            row.acquisition_cost_cents,
            esc(&method_db),
            row.business_share_percent
        )),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "asset.update")
        .await
        .ok();

    assets::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("update: detail-load leer".into()))
}

// =============================================================================
// Dispose (Veräußerung/Verschrottung — kein Löschen)
// =============================================================================

#[tauri::command]
pub async fn assets_dispose(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    args: AssetDisposeArgs,
) -> Result<AssetDetail> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;

    let dtype = DisposalType::from_db(&args.disposal_type).ok_or_else(|| {
        Error::Domain(format!("Ungültige Veräußerungsart: {}", args.disposal_type))
    })?;
    if args.disposal_date > today_berlin() {
        return Err(Error::Domain(
            "Das Veräußerungsdatum darf nicht in der Zukunft liegen.".into(),
        ));
    }
    // Erlös nur bei Verkauf; sonst hart auf 0.
    let proceeds = match dtype {
        DisposalType::Sale => {
            if args.proceeds_cents < 0 {
                return Err(Error::Domain(
                    "Der Verkaufserlös darf nicht negativ sein.".into(),
                ));
            }
            args.proceeds_cents
        }
        _ => 0,
    };

    let existing = assets::get(pool, &args.asset_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {}", args.asset_id)))?;
    if existing.disposed == 1 {
        return Err(Error::Domain("Anlage ist bereits veräußert.".into()));
    }

    // R2-023 (GJ-Guard): Disposal-Datum muss in einem offenen GJ liegen —
    // die Veräußerungs-Buchung kippt sonst rückwirkend Restbuchwert und
    // Erlös ins festgeschriebene Jahr.
    guard::ensure_year_open(pool, args.disposal_date.year() as i64).await?;

    // R2-021 (Pro-rata-AfA im Veräußerungsjahr, R 7.4 Abs. 7 EStR / §7 Abs. 1
    // Satz 4 EStG). Wenn für (Anlage, Disposal-Jahr) noch keine AfA-Zeile
    // existiert, wird vor dem Abgang eine partielle AfA gebucht (M/12 bis zum
    // Veräußerungsmonat). Der gekürzte Restbuchwert wandert als Snapshot in die
    // Disposal-Rechnung. Hat der reguläre AfA-Lauf für dieses Jahr schon
    // gebucht (z. B. nach manuellem `depreciation_accrue_year`), bleibt sein
    // Wert vorrangig.
    let disposal_year = args.disposal_date.year() as i64;
    let residual = match depreciation_repo::get_for_asset_year(pool, &args.asset_id, disposal_year)
        .await?
    {
        Some(entry) => {
            // AfA-Lauf war schon: existing.book_value_cents ist bereits der
            // gedrückte Wert; sicherheitshalber gegen den gebuchten
            // book_value_after abgleichen (Konsistenz).
            entry.book_value_after_cents
        }
        None => {
            // Pure-FC: M/12 bis Veräußerungsmonat. Asset-Stammdaten in
            // den Domain-Typ überführen.
            let acquisition_date =
                NaiveDate::parse_from_str(&existing.acquisition_date, "%Y-%m-%d").map_err(
                    |_| {
                        Error::Domain(format!(
                            "Ungültiges Anschaffungs-Datum in DB: {}",
                            existing.acquisition_date
                        ))
                    },
                )?;
            let method =
                DepreciationMethod::from_db(&existing.depreciation_method).ok_or_else(|| {
                    Error::Domain(format!(
                        "Unbekannte AfA-Methode: {}",
                        existing.depreciation_method
                    ))
                })?;
            let dep_asset = DepreciationAsset {
                depreciation_method: method,
                acquisition_date,
                acquisition_cost_cents: existing.acquisition_cost_cents,
                business_share_percent: existing.business_share_percent,
                useful_life_years: existing.useful_life_years,
            };
            let calc = compute_disposal_year_partial(
                &dep_asset,
                existing.book_value_cents,
                args.disposal_date,
            );
            if !calc.is_noop() {
                depreciation_repo::book_entry(pool, &args.asset_id, disposal_year, &calc).await?;
                assets::set_book_value(
                    pool,
                    &args.asset_id,
                    calc.book_value_after_cents,
                    Some(disposal_year),
                )
                .await?;
                audit_log::append(
                    pool,
                    "depreciation.disposal_partial",
                    "asset",
                    &args.asset_id,
                    Some(&format!(
                        r#"{{"fiscal_year":{},"months":{},"amount":{},"book_value_after":{}}}"#,
                        disposal_year,
                        calc.months_in_year,
                        calc.depreciation_amount_cents,
                        calc.book_value_after_cents
                    )),
                )
                .await?;
            }
            calc.book_value_after_cents
        }
    };

    let row = assets::dispose(
        pool,
        &args.asset_id,
        &args.disposal_date.to_string(),
        dtype.as_db(),
        proceeds,
        residual,
    )
    .await?;

    audit_log::append(
        pool,
        "asset.dispose",
        "asset",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","type":"{}","date":"{}","proceeds":{},"residual_book_value":{}}}"#,
            esc(&row.asset_number),
            dtype.as_db(),
            args.disposal_date,
            proceeds,
            residual
        )),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "asset.dispose")
        .await
        .ok();

    assets::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("dispose: detail-load leer".into()))
}

// ---- intern ----------------------------------------------------------------

/// Leitet aus der (validierten) Eingabe den DB-Methoden-Slug + die effektive
/// Nutzungsdauer ab: linear = Eingabe, Computer-Sonderregel = 1 Jahr, GWG = keine.
fn resolve_method(input: &AssetInput) -> Result<(String, Option<f64>)> {
    let method = DepreciationMethod::from_db(&input.depreciation_method).ok_or_else(|| {
        Error::Domain(format!(
            "Ungültige AfA-Methode: {}",
            input.depreciation_method
        ))
    })?;
    let useful_life = match method {
        DepreciationMethod::Linear => input.useful_life_years,
        DepreciationMethod::ComputerSpecial2021 => Some(1.0),
        DepreciationMethod::GwgSofort => None,
    };
    Ok((method.as_db().to_string(), useful_life))
}

/// Heutiges Datum (Europe/Berlin = System-TZ, in Block 0 gepinnt).
fn today_berlin() -> NaiveDate {
    Local::now().date_naive()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
