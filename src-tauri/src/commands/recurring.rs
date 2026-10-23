//! Tauri-Commands für wiederkehrende Abos (Block 10).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::recurring::validate_recurring`]).
//! - CRUD + Pausieren ([`crate::db::repo::recurring`]).
//! - Manuelle/automatische Auto-Anlage von Kosten ([`crate::scheduler::recurring`]).
//! - Audit-Log.
//!
//! ## Abgrenzung
//!
//! Ein Abo ist ein Stammdatum/Template — editierbar und pausierbar (kein
//! GoBD-Beleg). Die daraus erzeugten Kosten sind sofort gelockt (Block 9).

use crate::app_ctx::AppCtx;
use std::sync::Arc;

use crate::db::models::{ExpenseRow, RecurringSubscriptionRow};
use crate::db::repo::{audit_log, recurring};
use crate::domain::recurring::{self as domain, RecurringInput};
use crate::error::{Error, Result};
use crate::scheduler::recurring::{process_due, run_now, ProcessReport};

// =============================================================================
// Read
// =============================================================================

/// Argumente von [`recurring_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringListArgs {
    #[serde(default)]
    pub include_inactive: Option<bool>,
}

pub async fn recurring_list(
    ctx: &Arc<AppCtx>,
    args: RecurringListArgs,
) -> Result<Vec<RecurringSubscriptionRow>> {
    let RecurringListArgs { include_inactive } = args;
    let pool = ctx.pool()?;

    recurring::list(&pool, include_inactive.unwrap_or(false)).await
}

/// Argumente von [`recurring_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringGetArgs {
    pub id: String,
}

pub async fn recurring_get(
    ctx: &Arc<AppCtx>,
    args: RecurringGetArgs,
) -> Result<Option<RecurringSubscriptionRow>> {
    let RecurringGetArgs { id } = args;
    let pool = ctx.pool()?;

    recurring::get(&pool, &id).await
}

// =============================================================================
// Create / Update / Pause
// =============================================================================

/// Argumente von [`recurring_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringCreateArgs {
    pub input: RecurringInput,
}

pub async fn recurring_create(
    ctx: &Arc<AppCtx>,
    args: RecurringCreateArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringCreateArgs { input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    let row = recurring::create(&pool, &input).await?;
    audit_log::append(
        &pool,
        "recurring.create",
        "recurring",
        &row.id,
        Some(&format!(
            r#"{{"label":"{}","frequency":"{}","auto":{}}}"#,
            esc(&row.label),
            esc(&row.frequency),
            row.auto_create_expense == 1
        )),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`recurring_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringUpdateArgs {
    pub id: String,
    pub input: RecurringInput,
}

pub async fn recurring_update(
    ctx: &Arc<AppCtx>,
    args: RecurringUpdateArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringUpdateArgs { id, input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    let row = recurring::update(&pool, &id, &input).await?;
    audit_log::append(
        &pool,
        "recurring.update",
        "recurring",
        &row.id,
        Some(&format!(
            r#"{{"label":"{}","frequency":"{}","auto":{}}}"#,
            esc(&row.label),
            esc(&row.frequency),
            row.auto_create_expense == 1
        )),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`recurring_set_active`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringSetActiveArgs {
    pub id: String,
    pub active: bool,
}

pub async fn recurring_set_active(
    ctx: &Arc<AppCtx>,
    args: RecurringSetActiveArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringSetActiveArgs { id, active } = args;
    let pool = ctx.pool()?;

    recurring::set_active(&pool, &id, active).await?;
    audit_log::append(
        &pool,
        if active {
            "recurring.activate"
        } else {
            "recurring.deactivate"
        },
        "recurring",
        &id,
        None,
    )
    .await?;
    recurring::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))
}

// =============================================================================
// Auslösen (manuell einzeln + Due-Check für alle)
// =============================================================================

/// Argumente von [`recurring_run_now`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringRunNowArgs {
    pub id: String,
}

/// „Jetzt erfassen" für ein fälliges Abo — legt eine Kosten-Position für den
/// aktuellen Stichtag an und rückt das Abo um eine Periode vor. Liefert die
/// erzeugte Kosten-Position (das Frontend navigiert dorthin).
pub async fn recurring_run_now(ctx: &Arc<AppCtx>, args: RecurringRunNowArgs) -> Result<ExpenseRow> {
    let RecurringRunNowArgs { id } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let paths = ctx.paths.clone();
    run_now(&pool, &paths, session, &id, today_berlin()).await
}

/// Argumente von [`recurring_run_due_check`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringRunDueCheckArgs {}

/// Manueller Due-Check für ALLE Auto-Abos (gleiche Logik wie der Scheduler-Tick).
/// Nützlich direkt nach dem Entsperren, ohne auf den nächsten Tick zu warten.
pub async fn recurring_run_due_check(
    ctx: &Arc<AppCtx>,
    _args: RecurringRunDueCheckArgs,
) -> Result<ProcessReport> {
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let paths = ctx.paths.clone();
    process_due(&pool, &paths, session, today_berlin()).await
}

// =============================================================================
// Helpers
// =============================================================================

fn validate(input: &RecurringInput) -> Result<()> {
    if let Err(errs) = domain::validate_recurring(input) {
        return Err(Error::Domain(format!(
            "Abo kann nicht gespeichert werden: {}",
            errs.iter()
                .map(domain::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    Ok(())
}

/// Heutiges Datum in Europe/Berlin (`crate::clock`, Plan W1.8).
fn today_berlin() -> chrono::NaiveDate {
    crate::clock::today_berlin()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
