//! Tauri-Commands für Zahlungs-Konten (Block 9).
//!
//! Stammdaten (Bank/Bargeld/PayPal/…). Kein Hard-Delete — Konten werden
//! deaktiviert (`active=0`), weil sie von Kosten/Rechnungen/Privatbewegungen
//! referenziert werden. Genau ein Konto kann `is_default` sein.

use crate::app_ctx::AppCtx;
use std::sync::Arc;

use crate::db::models::PaymentAccountRow;
use crate::db::repo::payment_accounts::PaymentAccountInput;
use crate::db::repo::{audit_log, payment_accounts};
use crate::error::Result;

/// Argumente von [`payment_accounts_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountsListArgs {
    #[serde(default)]
    pub include_inactive: Option<bool>,
}

pub async fn payment_accounts_list(
    ctx: &Arc<AppCtx>,
    args: PaymentAccountsListArgs,
) -> Result<Vec<PaymentAccountRow>> {
    let PaymentAccountsListArgs { include_inactive } = args;
    let pool = ctx.pool()?;

    payment_accounts::list(&pool, include_inactive.unwrap_or(false)).await
}

/// Argumente von [`payment_accounts_ensure_defaults`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountsEnsureDefaultsArgs {}

/// Idempotenter Seed der Standard-Konten ("Hauptkonto", "Bargeld") und Rückgabe
/// der aktuellen Liste. Wird vom Settings-UI beim ersten Öffnen aufgerufen.
pub async fn payment_accounts_ensure_defaults(
    ctx: &Arc<AppCtx>,
    _args: PaymentAccountsEnsureDefaultsArgs,
) -> Result<Vec<PaymentAccountRow>> {
    let pool = ctx.pool()?;

    let pool = &pool;
    let seeded = payment_accounts::ensure_defaults(pool).await?;
    if seeded {
        audit_log::append(
            pool,
            "payment_account.seed_defaults",
            "payment_account",
            "-",
            Some(r#"{"created":["Hauptkonto","Bargeld"]}"#),
        )
        .await?;
    }
    payment_accounts::list(pool, false).await
}

/// Argumente von [`payment_accounts_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountsCreateArgs {
    pub input: PaymentAccountInput,
}

pub async fn payment_accounts_create(
    ctx: &Arc<AppCtx>,
    args: PaymentAccountsCreateArgs,
) -> Result<PaymentAccountRow> {
    let PaymentAccountsCreateArgs { input } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let row = payment_accounts::create(pool, &input).await?;
    audit_log::append(
        pool,
        "payment_account.create",
        "payment_account",
        &row.id,
        Some(&format!(
            r#"{{"label":"{}","type":"{}","default":{}}}"#,
            escape(&row.label),
            escape(&row.account_type),
            row.is_default == 1
        )),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`payment_accounts_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountsUpdateArgs {
    pub id: String,
    pub input: PaymentAccountInput,
}

pub async fn payment_accounts_update(
    ctx: &Arc<AppCtx>,
    args: PaymentAccountsUpdateArgs,
) -> Result<PaymentAccountRow> {
    let PaymentAccountsUpdateArgs { id, input } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let row = payment_accounts::update(pool, &id, &input).await?;
    audit_log::append(
        pool,
        "payment_account.update",
        "payment_account",
        &id,
        Some(&format!(
            r#"{{"label":"{}","type":"{}","default":{}}}"#,
            escape(&row.label),
            escape(&row.account_type),
            row.is_default == 1
        )),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`payment_accounts_set_active`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountsSetActiveArgs {
    pub id: String,
    pub active: bool,
}

pub async fn payment_accounts_set_active(
    ctx: &Arc<AppCtx>,
    args: PaymentAccountsSetActiveArgs,
) -> Result<()> {
    let PaymentAccountsSetActiveArgs { id, active } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    payment_accounts::set_active(pool, &id, active).await?;
    audit_log::append(
        pool,
        if active {
            "payment_account.activate"
        } else {
            "payment_account.deactivate"
        },
        "payment_account",
        &id,
        None,
    )
    .await?;
    Ok(())
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
