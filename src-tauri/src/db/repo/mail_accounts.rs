//! Repository für `mail_accounts` (Block 5).
//!
//! Schicht: **Imperative Shell**. Die SMTP-Passphrase wird hier NICHT
//! gespeichert — es liegt in der Tabelle `secrets` (siehe [`crate::secrets`], W8).
//! Diese Tabelle hält nur die Verbindungs-Metadaten + die
//! `keychain_service_id`, unter der die Passphrase abgelegt ist.
//!
//! Die OAuth-Spalten aus Migration 0014 (Microsoft Graph, Desktop-Fassung)
//! bleiben im Schema (forward-only), werden im Server aber nicht mehr befüllt;
//! `update` leert sie, wenn ein übernommenes Microsoft-Konto auf SMTP umgestellt
//! wird (ADR 0039 Nr. 138).

use crate::db::models::MailAccountRow;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Eingabe zum Anlegen eines Mail-Accounts. Das SMTP-Passwort ist NICHT Teil
/// dieses Structs — es geht direkt vom Command in die Tabelle `secrets`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountInput {
    pub label: String,
    /// Im Server nur `"smtp_password"` (E9; Prüfung im Command).
    pub auth_type: String,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_user: Option<String>,
    pub smtp_use_tls: bool,
    pub from_email: String,
    pub from_name: String,
    pub is_default: bool,
}

/// Legt einen Account an. Vergibt UUIDv7 als ID und — bei
/// `auth_type = 'smtp_password'` — die deterministische `keychain_service_id`
/// `kleinbuch::mail::{id}`. Setzt bei `is_default` alle anderen zurück.
///
/// Der Caller (Command) ist dafür verantwortlich, die Passphrase unter der
/// zurückgegebenen `keychain_service_id` im Keychain abzulegen.
pub async fn create(pool: &SqlitePool, input: &MailAccountInput) -> Result<MailAccountRow> {
    let id = Uuid::now_v7().to_string();
    // Markiert ein Konto mit Passwort-Anmeldung; das Passwort selbst liegt seit
    // W8 unter `secrets::smtp_key(id)` in der Datenbank.
    let keychain_service_id = Some(format!("kleinbuch::mail::{id}"));

    let mut tx = crate::db::begin_immediate(pool).await?;
    if input.is_default {
        sqlx::query("UPDATE mail_accounts SET is_default = 0")
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "INSERT INTO mail_accounts (
            id, label, auth_type, smtp_host, smtp_port, smtp_user, smtp_use_tls,
            keychain_service_id, from_email, from_name, is_default
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.label)
    .bind(&input.auth_type)
    .bind(&input.smtp_host)
    .bind(input.smtp_port)
    .bind(&input.smtp_user)
    .bind(if input.smtp_use_tls { 1i64 } else { 0i64 })
    .bind(&keychain_service_id)
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(if input.is_default { 1i64 } else { 0i64 })
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("mail_accounts::create: post-INSERT SELECT leer".into()))
}

/// Alle Accounts, Default zuerst, dann alphabetisch.
pub async fn list(pool: &SqlitePool) -> Result<Vec<MailAccountRow>> {
    let rows = sqlx::query_as::<_, MailAccountRow>(
        "SELECT * FROM mail_accounts ORDER BY is_default DESC, label COLLATE NOCASE ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<MailAccountRow>> {
    let row = sqlx::query_as::<_, MailAccountRow>("SELECT * FROM mail_accounts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Der als Default markierte Account (falls vorhanden).
pub async fn get_default(pool: &SqlitePool) -> Result<Option<MailAccountRow>> {
    let row = sqlx::query_as::<_, MailAccountRow>(
        "SELECT * FROM mail_accounts WHERE is_default = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Aktualisiert `last_used_at` nach einem erfolgreichen Versand.
pub async fn touch_last_used(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("UPDATE mail_accounts SET last_used_at = datetime('now','utc') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Aktualisiert die Metadaten eines Accounts. `keychain_service_id` bleibt
/// unverändert (an die ID gebunden); das Passwort pflegt der Command separat
/// über den Keychain. Setzt bei `is_default` alle anderen zurück.
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: &MailAccountInput,
) -> Result<MailAccountRow> {
    let mut tx = crate::db::begin_immediate(pool).await?;
    if input.is_default {
        sqlx::query("UPDATE mail_accounts SET is_default = 0 WHERE id != ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let res = sqlx::query(
        "UPDATE mail_accounts SET
            label = ?, auth_type = ?, smtp_host = ?, smtp_port = ?, smtp_user = ?,
            smtp_use_tls = ?, from_email = ?, from_name = ?, is_default = ?,
            oauth_tenant_id = NULL, oauth_client_id = NULL, oauth_account_email = NULL,
            oauth_scopes = NULL, oauth_token_expires_at = NULL
         WHERE id = ?",
    )
    .bind(&input.label)
    .bind(&input.auth_type)
    .bind(&input.smtp_host)
    .bind(input.smtp_port)
    .bind(&input.smtp_user)
    .bind(if input.smtp_use_tls { 1i64 } else { 0i64 })
    .bind(&input.from_email)
    .bind(&input.from_name)
    .bind(if input.is_default { 1i64 } else { 0i64 })
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "mail_accounts::update: id {id} nicht gefunden"
        )));
    }
    tx.commit().await?;

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("mail_accounts::update: post-UPDATE SELECT leer".into()))
}

/// Löscht einen Account (DB-Zeile). Der Keychain-Eintrag wird vom Command
/// separat entfernt (Schichten-Trennung). Mail-Accounts sind Konfiguration,
/// keine GoBD-Belege — Hard-Delete ist hier zulässig.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM mail_accounts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "mail_accounts::delete: id {id} nicht gefunden"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
