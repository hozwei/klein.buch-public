//! Tauri-Commands für Zahlungs-Konten (Block 9).
//!
//! Stammdaten (Bank/Bargeld/PayPal/…). Kein Hard-Delete — Konten werden
//! deaktiviert (`active=0`), weil sie von Kosten/Rechnungen/Privatbewegungen
//! referenziert werden. Genau ein Konto kann `is_default` sein.

use sqlx::SqlitePool;
use tauri::State;

use crate::db::models::PaymentAccountRow;
use crate::db::repo::payment_accounts::PaymentAccountInput;
use crate::db::repo::{audit_log, payment_accounts};
use crate::error::Result;

#[tauri::command]
pub async fn payment_accounts_list(
    pool: State<'_, SqlitePool>,
    include_inactive: Option<bool>,
) -> Result<Vec<PaymentAccountRow>> {
    payment_accounts::list(pool.inner(), include_inactive.unwrap_or(false)).await
}

/// Idempotenter Seed der Standard-Konten ("Hauptkonto", "Bargeld") und Rückgabe
/// der aktuellen Liste. Wird vom Settings-UI beim ersten Öffnen aufgerufen.
#[tauri::command]
pub async fn payment_accounts_ensure_defaults(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<PaymentAccountRow>> {
    let pool = pool.inner();
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

#[tauri::command]
pub async fn payment_accounts_create(
    pool: State<'_, SqlitePool>,
    input: PaymentAccountInput,
) -> Result<PaymentAccountRow> {
    let pool = pool.inner();
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

#[tauri::command]
pub async fn payment_accounts_update(
    pool: State<'_, SqlitePool>,
    id: String,
    input: PaymentAccountInput,
) -> Result<PaymentAccountRow> {
    let pool = pool.inner();
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

#[tauri::command]
pub async fn payment_accounts_set_active(
    pool: State<'_, SqlitePool>,
    id: String,
    active: bool,
) -> Result<()> {
    let pool = pool.inner();
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
