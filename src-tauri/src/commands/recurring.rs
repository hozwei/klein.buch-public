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

use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::backup;
use crate::config::Paths;
use crate::db::models::{ExpenseRow, RecurringSubscriptionRow};
use crate::db::repo::{audit_log, recurring};
use crate::domain::recurring::{self as domain, RecurringInput};
use crate::error::{Error, Result};
use crate::scheduler::recurring::{process_due, run_now, ProcessReport};

// =============================================================================
// Read
// =============================================================================

#[tauri::command]
pub async fn recurring_list(
    pool: State<'_, SqlitePool>,
    include_inactive: Option<bool>,
) -> Result<Vec<RecurringSubscriptionRow>> {
    recurring::list(pool.inner(), include_inactive.unwrap_or(false)).await
}

#[tauri::command]
pub async fn recurring_get(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<RecurringSubscriptionRow>> {
    recurring::get(pool.inner(), &id).await
}

// =============================================================================
// Create / Update / Pause
// =============================================================================

#[tauri::command]
pub async fn recurring_create(
    pool: State<'_, SqlitePool>,
    input: RecurringInput,
) -> Result<RecurringSubscriptionRow> {
    validate(&input)?;
    let row = recurring::create(pool.inner(), &input).await?;
    audit_log::append(
        pool.inner(),
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

#[tauri::command]
pub async fn recurring_update(
    pool: State<'_, SqlitePool>,
    id: String,
    input: RecurringInput,
) -> Result<RecurringSubscriptionRow> {
    validate(&input)?;
    let row = recurring::update(pool.inner(), &id, &input).await?;
    audit_log::append(
        pool.inner(),
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

#[tauri::command]
pub async fn recurring_set_active(
    pool: State<'_, SqlitePool>,
    id: String,
    active: bool,
) -> Result<RecurringSubscriptionRow> {
    recurring::set_active(pool.inner(), &id, active).await?;
    audit_log::append(
        pool.inner(),
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
    recurring::get(pool.inner(), &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))
}

// =============================================================================
// Auslösen (manuell einzeln + Due-Check für alle)
// =============================================================================

/// „Jetzt erfassen" für ein fälliges Abo — legt eine Kosten-Position für den
/// aktuellen Stichtag an und rückt das Abo um eine Periode vor. Liefert die
/// erzeugte Kosten-Position (das Frontend navigiert dorthin).
#[tauri::command]
pub async fn recurring_run_now(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    id: String,
) -> Result<ExpenseRow> {
    let paths = Paths::from_handle(&app)?;
    run_now(pool.inner(), &paths, session.inner(), &id, today_berlin()).await
}

/// Manueller Due-Check für ALLE Auto-Abos (gleiche Logik wie der Scheduler-Tick).
/// Nützlich direkt nach dem Entsperren, ohne auf den nächsten Tick zu warten.
#[tauri::command]
pub async fn recurring_run_due_check(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
) -> Result<ProcessReport> {
    let paths = Paths::from_handle(&app)?;
    process_due(pool.inner(), &paths, session.inner(), today_berlin()).await
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

/// Heutiges Datum (Europe/Berlin = System-TZ, in Block 0 gepinnt). Wie in
/// `commands::expenses`.
fn today_berlin() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
