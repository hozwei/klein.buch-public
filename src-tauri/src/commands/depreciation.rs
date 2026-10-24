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

use crate::app_ctx::AppCtx;
use std::sync::Arc;

use crate::db::models::{AssetDetail, DepreciationEntryRow};
use crate::db::repo::{assets, depreciation as depreciation_repo};
use crate::depreciation::accrue_yearly::{self, AccrueReport};
use crate::error::{Error, Result};

/// Argumente von [`depreciation_accrue_year`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepreciationAccrueYearArgs {
    pub fiscal_year: i64,
}

/// Bucht die AfA bis einschließlich `fiscal_year` (Catch-up je Anlage). Idempotent.
pub async fn depreciation_accrue_year(
    ctx: &Arc<AppCtx>,
    args: DepreciationAccrueYearArgs,
) -> Result<AccrueReport> {
    let DepreciationAccrueYearArgs { fiscal_year } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    let today = crate::clock::today_berlin();
    accrue_yearly::accrue_for_year(pool, &paths, session, fiscal_year, today).await
}

/// Argumente von [`depreciation_reset_asset`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepreciationResetAssetArgs {
    pub asset_id: String,
}

/// Setzt die noch nicht festgeschriebene AfA einer Anlage zurück (Korrektur im
/// offenen Geschäftsjahr). Liefert die aktualisierte Anlage zurück.
pub async fn depreciation_reset_asset(
    ctx: &Arc<AppCtx>,
    args: DepreciationResetAssetArgs,
) -> Result<AssetDetail> {
    let DepreciationResetAssetArgs { asset_id } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    accrue_yearly::reset_asset(pool, &paths, session, &asset_id).await?;
    assets::get_detail(pool, &asset_id)
        .await?
        .ok_or_else(|| Error::Domain("reset: detail-load leer".into()))
}

/// Argumente von [`depreciation_list_for_year`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepreciationListForYearArgs {
    pub fiscal_year: i64,
}

/// Alle AfA-Buchungen eines Geschäftsjahres.
pub async fn depreciation_list_for_year(
    ctx: &Arc<AppCtx>,
    args: DepreciationListForYearArgs,
) -> Result<Vec<DepreciationEntryRow>> {
    let DepreciationListForYearArgs { fiscal_year } = args;
    let pool = ctx.pool()?;

    depreciation_repo::list_for_year(&pool, fiscal_year).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
