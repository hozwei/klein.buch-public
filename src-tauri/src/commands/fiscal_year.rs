//! GJ-Abschluss-Commands (Phase 2D, Block 15).
//!
//! Übersicht aller Geschäftsjahre (offen/geschlossen + EÜR-Eckwerte + AfA-Status),
//! der Abschluss-Wizard-Endpoint, das Festschreibungsprotokoll und der
//! Auto-AfA-Schalter (Manuel-Entscheidung Block 15: Default an, abschaltbar).

use crate::backup::BackupSession;
use crate::config::Paths;
use crate::db::repo::{app_settings, euer as euer_repo};
use crate::error::Result;
use crate::euer::aggregate::aggregate;
use crate::fiscal_year::{lock, transition};
use chrono::Datelike;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeSet;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiscalYearStatus {
    pub fiscal_year: i64,
    pub closed: bool,
    pub closed_at: Option<String>,
    pub income_total_cents: i64,
    pub expense_total_cents: i64,
    pub afa_total_cents: i64,
    pub surplus_cents: i64,
    /// Abschluss möglich: noch offen UND Geschäftsjahr bereits abgelaufen.
    pub closable: bool,
    /// Anzahl aktiver Anlagen ohne gebuchte AfA für dieses Jahr (Warnhinweis).
    pub afa_pending: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiscalYearOverview {
    pub current_year: i64,
    pub auto_year_close: bool,
    pub years: Vec<FiscalYearStatus>,
    pub open_receivables: Vec<transition::OpenReceivable>,
}

#[tauri::command]
pub async fn fiscal_year_overview(pool: State<'_, SqlitePool>) -> Result<FiscalYearOverview> {
    let pool = pool.inner();
    let current = chrono::Local::now().date_naive().year() as i64;
    let auto = app_settings::get_bool(pool, "depreciation_auto_year_close", true).await?;

    let inputs = euer_repo::load_inputs(pool).await?;
    let closed = lock::list_closed(pool).await?;

    // Jahres-Menge: Jahre mit Bewegungsdaten ∪ geschlossene Jahre ∪ Vorjahr.
    let mut years: BTreeSet<i64> = BTreeSet::new();
    for y in euer_repo::available_years(pool).await? {
        years.insert(y as i64);
    }
    for l in &closed {
        years.insert(l.fiscal_year);
    }
    if current >= 2001 {
        years.insert(current - 1);
    }

    let mut out = Vec::new();
    for year in years.into_iter().rev() {
        let lock_row = closed.iter().find(|l| l.fiscal_year == year);
        let (income, expense, afa, surplus, is_closed, closed_at) = match lock_row {
            Some(l) => (
                l.income_total_cents,
                l.expense_total_cents,
                l.afa_total_cents,
                l.surplus_cents,
                true,
                Some(l.closed_at.clone()),
            ),
            None => {
                let r = aggregate(year as i32, &inputs);
                (
                    r.total_income_cents,
                    r.total_expenses_cents,
                    r.depreciation_total_cents,
                    r.surplus_cents,
                    false,
                    None,
                )
            }
        };
        let afa_pending = euer_repo::afa_pending_count(pool, year as i32).await?;
        out.push(FiscalYearStatus {
            fiscal_year: year,
            closed: is_closed,
            closed_at,
            income_total_cents: income,
            expense_total_cents: expense,
            afa_total_cents: afa,
            surplus_cents: surplus,
            closable: !is_closed && year < current,
            afa_pending,
        });
    }

    let open_receivables = transition::open_receivables(pool).await?;
    Ok(FiscalYearOverview {
        current_year: current,
        auto_year_close: auto,
        years: out,
        open_receivables,
    })
}

#[tauri::command]
pub async fn fiscal_year_close(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, BackupSession>,
    year: i64,
) -> Result<lock::FiscalYearLock> {
    let paths = Paths::from_handle(&app)?;
    let today = chrono::Local::now().date_naive();
    lock::close_year(pool.inner(), &paths, session.inner(), year, today).await
}

#[tauri::command]
pub async fn fiscal_year_closed_list(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<lock::FiscalYearLock>> {
    lock::list_closed(pool.inner()).await
}

#[tauri::command]
pub async fn fiscal_year_auto_close_get(pool: State<'_, SqlitePool>) -> Result<bool> {
    app_settings::get_bool(pool.inner(), "depreciation_auto_year_close", true).await
}

#[tauri::command]
pub async fn fiscal_year_auto_close_set(pool: State<'_, SqlitePool>, enabled: bool) -> Result<()> {
    app_settings::set_bool(pool.inner(), "depreciation_auto_year_close", enabled).await
}
