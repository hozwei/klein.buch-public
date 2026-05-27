//! Notification-Commands (Phase 2D, Block 15).
//!
//! In-App-Inbox (lesen / abhaken), Reminder-Regeln (ein-/ausschalten) und ein
//! manueller „Jetzt prüfen"-Trigger, der Integrity-Check + Reminder sofort laufen
//! lässt.

use crate::error::Result;
use crate::notify::{rules, store, NotificationRule};
use crate::scheduler::{integrity_check_cron, reminders};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn notifications_list(
    pool: State<'_, SqlitePool>,
    include_dismissed: bool,
) -> Result<Vec<store::Notification>> {
    store::list(pool.inner(), include_dismissed).await
}

#[tauri::command]
pub async fn notifications_unread_count(pool: State<'_, SqlitePool>) -> Result<i64> {
    store::count_unread(pool.inner()).await
}

#[tauri::command]
pub async fn notifications_dismiss(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    store::dismiss(pool.inner(), &id).await
}

#[tauri::command]
pub async fn notifications_dismiss_all(pool: State<'_, SqlitePool>) -> Result<u64> {
    store::dismiss_all(pool.inner()).await
}

#[tauri::command]
pub async fn notification_rules_list(pool: State<'_, SqlitePool>) -> Result<Vec<NotificationRule>> {
    rules::list(pool.inner()).await
}

#[tauri::command]
pub async fn notification_rules_set_enabled(
    pool: State<'_, SqlitePool>,
    id: String,
    enabled: bool,
) -> Result<()> {
    rules::set_enabled(pool.inner(), &id, enabled).await
}

/// Manueller Trigger: Integrity-Check (falls fällig) + Reminder-Regeln jetzt
/// laufen lassen. Liefert die Anzahl neu erzeugter Hinweise.
#[tauri::command]
pub async fn notifications_run_checks(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
) -> Result<usize> {
    let today = chrono::Local::now().date_naive();
    let pool = pool.inner();
    let paths = crate::config::Paths::from_handle(&app)?;
    integrity_check_cron::run(pool, Some(&app), today)
        .await
        .ok();
    reminders::run(pool, Some(&app), today, &paths.backups_dir).await
}
