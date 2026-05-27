//! GJ-Abschluss / Festschreibung (Block 15).
//!
//! [`close_year`] schließt ein abgelaufenes Geschäftsjahr prüfungssicher ab:
//! 1. AfA des Jahres sicher buchen (idempotent, holt fehlende Jahre nach),
//! 2. AfA-Buchungen + betroffene Anlagen festschreiben (`locked_at`),
//! 3. EÜR-Eckwerte als Snapshot ins Festschreibungsprotokoll (`fiscal_year_locks`),
//! 4. Audit-Log-Eintrag `fiscal_year.close`,
//! 5. Auto-Critical-Backup (§6.15).
//!
//! Der Abschluss ist **unumkehrbar** (kein Entsperren-UI; DB-Trigger verbieten
//! Update/Delete auf `fiscal_year_locks`). Ab dem Abschluss verhindert der
//! [`crate::fiscal_year::guard`] jede neue Buchung mit Datum im geschlossenen Jahr.

use crate::backup::{self, BackupSession};
use crate::config::Paths;
use crate::db::repo::{
    assets as assets_repo, audit_log, depreciation as depreciation_repo, euer as euer_repo,
};
use crate::error::{Error, Result};
use crate::euer::aggregate::aggregate;
use crate::fiscal_year::guard;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FiscalYearLock {
    pub fiscal_year: i64,
    pub closed_at: String,
    pub income_total_cents: i64,
    pub expense_total_cents: i64,
    pub afa_total_cents: i64,
    pub surplus_cents: i64,
    pub assets_locked: i64,
    pub depreciation_entries_locked: i64,
    pub app_version: String,
    pub schema_version: i64,
    pub notes: Option<String>,
}

/// Schließt das Geschäftsjahr `year` ab. `today` wird injiziert (Testbarkeit).
pub async fn close_year(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    year: i64,
    today: NaiveDate,
) -> Result<FiscalYearLock> {
    let current = today.year() as i64;

    // 1. Plausibilität — nur abgelaufene Geschäftsjahre schließen.
    if year >= current {
        return Err(Error::Domain(format!(
            "Das Geschäftsjahr {year} kann erst nach seinem Ablauf abgeschlossen werden \
             (aktuell läuft das Geschäftsjahr {current})."
        )));
    }
    // 2. Doppel-Abschluss verhindern.
    if guard::is_closed(pool, year).await? {
        return Err(Error::Domain(format!(
            "Das Geschäftsjahr {year} ist bereits abgeschlossen."
        )));
    }
    // 3. Backup-Session muss entsperrt sein — der Abschluss bucht AfA und erzeugt
    //    eine Sicherung; beides braucht die Passphrase.
    if !session.is_unlocked() {
        return Err(Error::Domain(
            "Bitte zuerst das Backup entsperren (Passphrase eingeben). \
             Der Geschäftsjahres-Abschluss erzeugt eine Sicherung."
                .into(),
        ));
    }

    // 4. AfA des Jahres sicher buchen (idempotent; bereits gebuchte bleiben).
    crate::depreciation::accrue_yearly::accrue_for_year(pool, paths, session, year, today).await?;

    // 5. Festschreiben: AfA-Buchungen + betroffene (noch offene) Anlagen locken.
    let dep_locked = depreciation_repo::lock_for_year(pool, year).await?;
    let mut assets_locked: u64 = 0;
    for asset in assets_repo::list_active_for_year(pool, year).await? {
        if asset.locked_at.is_none() {
            assets_repo::lock(pool, &asset.id).await?;
            assets_locked += 1;
        }
    }

    // 6. EÜR-Snapshot für das Protokoll.
    let inputs = euer_repo::load_inputs(pool).await?;
    let report = aggregate(year as i32, &inputs);

    // 7. Festschreibungsprotokoll schreiben.
    let app_version = env!("CARGO_PKG_VERSION");
    sqlx::query(
        "INSERT INTO fiscal_year_locks
            (fiscal_year, income_total_cents, expense_total_cents, afa_total_cents,
             surplus_cents, assets_locked, depreciation_entries_locked,
             app_version, schema_version)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(year)
    .bind(report.total_income_cents)
    .bind(report.total_expenses_cents)
    .bind(report.depreciation_total_cents)
    .bind(report.surplus_cents)
    .bind(assets_locked as i64)
    .bind(dep_locked as i64)
    .bind(app_version)
    .bind(crate::db::schema_version::EXPECTED_SCHEMA_VERSION as i64)
    .execute(pool)
    .await?;

    // 8. Audit-Log (append-only).
    audit_log::append(
        pool,
        "fiscal_year.close",
        "fiscal_year",
        &year.to_string(),
        Some(&format!(
            r#"{{"income":{},"expenses":{},"afa":{},"surplus":{},"assets_locked":{},"depreciation_locked":{}}}"#,
            report.total_income_cents,
            report.total_expenses_cents,
            report.depreciation_total_cents,
            report.surplus_cents,
            assets_locked,
            dep_locked
        )),
    )
    .await?;

    // 9. Auto-Critical-Backup (§6.15: nach fiscal_year.close).
    backup::auto_backup_if_unlocked(pool, paths, session, "fiscal_year.close")
        .await
        .ok();

    // 10. Protokoll zurückgeben.
    get(pool, year)
        .await?
        .ok_or_else(|| Error::Domain("Abschluss-Protokoll konnte nicht gelesen werden.".into()))
}

pub async fn get(pool: &SqlitePool, year: i64) -> Result<Option<FiscalYearLock>> {
    let row = sqlx::query_as::<_, FiscalYearLock>(
        "SELECT * FROM fiscal_year_locks WHERE fiscal_year = ?",
    )
    .bind(year)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_closed(pool: &SqlitePool) -> Result<Vec<FiscalYearLock>> {
    let rows = sqlx::query_as::<_, FiscalYearLock>(
        "SELECT * FROM fiscal_year_locks ORDER BY fiscal_year DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
