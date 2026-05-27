//! Tauri-Commands für AfA-Buchungen (Block 12, Phase 2C).
//!
//! - [`depreciation_accrue_year`] — „AfA für Jahr X jetzt buchen" (Erstanwendung
//!   + manuelles Nachbuchen). Ruft die Shell [`crate::depreciation::accrue_yearly`].
//! - [`depreciation_reset_asset`] — gebuchte AfA eines offenen GJ zurücksetzen
//!   (Korrektur, solange nicht festgeschrieben).
//! - [`depreciation_list_for_year`] — alle Buchungen eines GJ (Anzeige/EÜR-Vorschau).
//!
//! Der automatische GJ-Wende-Lauf (01.01.) + die Festschreibung werden erst in
//! Block 15 verdrahtet.

use chrono::Local;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::backup;
use crate::config::Paths;
use crate::db::models::{AssetDetail, DepreciationEntryRow};
use crate::db::repo::{assets, depreciation as depreciation_repo};
use crate::depreciation::accrue_yearly::{self, AccrueReport};
use crate::error::{Error, Result};

/// Bucht die AfA bis einschließlich `fiscal_year` (Catch-up je Anlage). Idempotent.
#[tauri::command]
pub async fn depreciation_accrue_year(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    fiscal_year: i64,
) -> Result<AccrueReport> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    let today = Local::now().date_naive();
    accrue_yearly::accrue_for_year(pool, &paths, session.inner(), fiscal_year, today).await
}

/// Setzt die noch nicht festgeschriebene AfA einer Anlage zurück (Korrektur im
/// offenen Geschäftsjahr). Liefert die aktualisierte Anlage zurück.
#[tauri::command]
pub async fn depreciation_reset_asset(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    asset_id: String,
) -> Result<AssetDetail> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    accrue_yearly::reset_asset(pool, &paths, session.inner(), &asset_id).await?;
    assets::get_detail(pool, &asset_id)
        .await?
        .ok_or_else(|| Error::Domain("reset: detail-load leer".into()))
}

/// Alle AfA-Buchungen eines Geschäftsjahres.
#[tauri::command]
pub async fn depreciation_list_for_year(
    pool: State<'_, SqlitePool>,
    fiscal_year: i64,
) -> Result<Vec<DepreciationEntryRow>> {
    depreciation_repo::list_for_year(pool.inner(), fiscal_year).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
