//! Notification-Commands (Phase 2D, Block 15).
//!
//! In-App-Inbox (lesen / abhaken), Reminder-Regeln (ein-/ausschalten) und ein
//! manueller „Jetzt prüfen"-Trigger, der Integrity-Check + Reminder sofort laufen
//! lässt.

use crate::app_ctx::AppCtx;
use crate::error::Result;
use crate::notify::{rules, store, NotificationRule};
use crate::scheduler::{integrity_check_cron, reminders};
use std::sync::Arc;

/// Argumente von [`notifications_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsListArgs {
    pub include_dismissed: bool,
}

pub async fn notifications_list(
    ctx: &Arc<AppCtx>,
    args: NotificationsListArgs,
) -> Result<Vec<store::Notification>> {
    let NotificationsListArgs { include_dismissed } = args;
    let pool = ctx.pool()?;

    store::list(&pool, include_dismissed).await
}

/// Argumente von [`notifications_unread_count`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsUnreadCountArgs {}

pub async fn notifications_unread_count(
    ctx: &Arc<AppCtx>,
    _args: NotificationsUnreadCountArgs,
) -> Result<i64> {
    let pool = ctx.pool()?;

    store::count_unread(&pool).await
}

/// Argumente von [`notifications_dismiss`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsDismissArgs {
    pub id: String,
}

pub async fn notifications_dismiss(
    ctx: &Arc<AppCtx>,
    args: NotificationsDismissArgs,
) -> Result<()> {
    let NotificationsDismissArgs { id } = args;
    let pool = ctx.pool()?;

    store::dismiss(&pool, &id).await
}

/// Argumente von [`notifications_dismiss_all`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsDismissAllArgs {}

pub async fn notifications_dismiss_all(
    ctx: &Arc<AppCtx>,
    _args: NotificationsDismissAllArgs,
) -> Result<u64> {
    let pool = ctx.pool()?;

    store::dismiss_all(&pool).await
}

/// Argumente von [`notification_rules_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRulesListArgs {}

pub async fn notification_rules_list(
    ctx: &Arc<AppCtx>,
    _args: NotificationRulesListArgs,
) -> Result<Vec<NotificationRule>> {
    let pool = ctx.pool()?;

    rules::list(&pool).await
}

/// Argumente von [`notification_rules_set_enabled`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRulesSetEnabledArgs {
    pub id: String,
    pub enabled: bool,
}

pub async fn notification_rules_set_enabled(
    ctx: &Arc<AppCtx>,
    args: NotificationRulesSetEnabledArgs,
) -> Result<()> {
    let NotificationRulesSetEnabledArgs { id, enabled } = args;
    let pool = ctx.pool()?;

    rules::set_enabled(&pool, &id, enabled).await
}

/// Argumente von [`notifications_run_checks`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsRunChecksArgs {}

/// Manueller Trigger: Integrity-Check (falls fällig) + Reminder-Regeln jetzt
/// laufen lassen. Liefert die Anzahl neu erzeugter Hinweise.
pub async fn notifications_run_checks(
    ctx: &Arc<AppCtx>,
    _args: NotificationsRunChecksArgs,
) -> Result<usize> {
    let pool = ctx.pool()?;

    let today = crate::clock::today_berlin();
    let pool = &pool;
    let paths = ctx.paths.clone();
    integrity_check_cron::run(pool, ctx.notifier(), today)
        .await
        .ok();
    reminders::run(pool, ctx.notifier(), today, &paths.backups_dir).await
}
