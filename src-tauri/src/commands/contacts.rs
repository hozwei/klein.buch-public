//! Commands für Kontakte (Block 2).
//!
//! Frontend ruft via `invoke("contacts_list", { includeArchived: false })` etc.
//! (`POST /api/cmd/{name}`). Argumente kommen als JSON-Objekt mit
//! camelCase-Schlüsseln; die Args-Structs übersetzen per
//! `serde(rename_all = "camelCase")` in snake_case.

use crate::app_ctx::AppCtx;
use crate::db::models::ContactRow;
use crate::db::repo::{audit_log, contacts};
use crate::domain::contact::ContactInput;
use crate::error::{Error, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Arc;

/// Argumente von [`contacts_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsListArgs {
    #[serde(default)]
    pub include_archived: Option<bool>,
}

pub async fn contacts_list(ctx: &Arc<AppCtx>, args: ContactsListArgs) -> Result<Vec<ContactRow>> {
    let ContactsListArgs { include_archived } = args;
    let pool = ctx.pool()?;

    contacts::list(&pool, include_archived.unwrap_or(false)).await
}

/// Argumente von [`contacts_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsGetArgs {
    pub id: String,
}

pub async fn contacts_get(ctx: &Arc<AppCtx>, args: ContactsGetArgs) -> Result<Option<ContactRow>> {
    let ContactsGetArgs { id } = args;
    let pool = ctx.pool()?;

    contacts::get(&pool, &id).await
}

/// Argumente von [`contacts_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsCreateArgs {
    pub input: ContactInput,
}

pub async fn contacts_create(ctx: &Arc<AppCtx>, args: ContactsCreateArgs) -> Result<ContactRow> {
    let ContactsCreateArgs { input } = args;
    let pool = ctx.pool()?;

    contacts::create(&pool, &input).await
}

/// Argumente von [`contacts_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsUpdateArgs {
    pub id: String,
    pub input: ContactInput,
}

pub async fn contacts_update(ctx: &Arc<AppCtx>, args: ContactsUpdateArgs) -> Result<ContactRow> {
    let ContactsUpdateArgs { id, input } = args;
    let pool = ctx.pool()?;

    contacts::update(&pool, &id, &input).await
}

/// Argumente von [`contacts_archive`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsArchiveArgs {
    pub id: String,
}

pub async fn contacts_archive(ctx: &Arc<AppCtx>, args: ContactsArchiveArgs) -> Result<()> {
    let ContactsArchiveArgs { id } = args;
    let pool = ctx.pool()?;

    contacts::archive(&pool, &id).await
}

/// Argumente von [`contacts_unarchive`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsUnarchiveArgs {
    pub id: String,
}

pub async fn contacts_unarchive(ctx: &Arc<AppCtx>, args: ContactsUnarchiveArgs) -> Result<()> {
    let ContactsUnarchiveArgs { id } = args;
    let pool = ctx.pool()?;

    contacts::unarchive(&pool, &id).await
}

/// Argumente von [`contacts_search`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsSearchArgs {
    pub query: String,
    #[serde(default)]
    pub include_archived: Option<bool>,
}

pub async fn contacts_search(
    ctx: &Arc<AppCtx>,
    args: ContactsSearchArgs,
) -> Result<Vec<ContactRow>> {
    let ContactsSearchArgs {
        query,
        include_archived,
    } = args;
    let pool = ctx.pool()?;

    contacts::search(&pool, &query, include_archived.unwrap_or(false)).await
}

// =============================================================================
// DSGVO Art. 17 — Anonymisierung (Block 19)
// =============================================================================

/// Vorab-Prüfung für die UI: darf der Kontakt anonymisiert werden, und was
/// bleibt erhalten? Treibt den Aufklärungs-Dialog vor der irreversiblen Aktion.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymizeCheck {
    /// `true`, wenn die Anonymisierung jetzt möglich ist.
    pub can_anonymize: bool,
    /// `true`, wenn der Kontakt bereits anonymisiert wurde.
    pub already_anonymized: bool,
    pub open_invoice_drafts: i64,
    pub open_quote_drafts: i64,
    /// Festgeschriebene Belege, die über den Buyer-Snapshot erhalten bleiben.
    pub locked_invoices: i64,
    pub locked_quotes: i64,
    /// Klartext-Grund, falls blockiert (sonst `None`).
    pub blocker: Option<String>,
}

/// Argumente von [`contacts_anonymize_check`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsAnonymizeCheckArgs {
    pub id: String,
}

pub async fn contacts_anonymize_check(
    ctx: &Arc<AppCtx>,
    args: ContactsAnonymizeCheckArgs,
) -> Result<AnonymizeCheck> {
    let ContactsAnonymizeCheckArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let contact = contacts::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Kontakt nicht gefunden: {id}")))?;
    let (open_invoice_drafts, open_quote_drafts) = contacts::count_open_drafts(pool, &id).await?;
    let (locked_invoices, locked_quotes) = contacts::count_locked_documents(pool, &id).await?;
    let already_anonymized = contact.anonymized_at.is_some();
    let blocker = if already_anonymized {
        Some("Kontakt ist bereits anonymisiert.".to_string())
    } else {
        crate::domain::anonymize::anonymization_blocker(open_invoice_drafts, open_quote_drafts)
    };
    Ok(AnonymizeCheck {
        can_anonymize: !already_anonymized && blocker.is_none(),
        already_anonymized,
        open_invoice_drafts,
        open_quote_drafts,
        locked_invoices,
        locked_quotes,
        blocker,
    })
}

/// Testbarer Kern der Anonymisierung (analog `dsgvo::export_core`): überschreibt
/// die Stammdaten via [`contacts::anonymize`] und schreibt genau EINEN
/// `contact.anonymize`-Audit-Eintrag mit NUR Zählwerten — keine
/// personenbezogenen Inhalte. Der Audit-Eintrag wird nur bei erfolgreicher
/// Anonymisierung geschrieben (Guard/Already-Fehler ⇒ kein Audit).
pub async fn anonymize_core(pool: &SqlitePool, id: &str) -> Result<ContactRow> {
    // Vor dem Überschreiben zählen, was erhalten bleibt (fürs Audit-Detail).
    let (locked_invoices, locked_quotes) = contacts::count_locked_documents(pool, id).await?;
    // W14-031: Vorlagen und Kosten-Abos dieses Kontakts werden mit pausiert.
    let (row, paused_templates, paused_subscriptions) =
        contacts::anonymize_with_pauses(pool, id).await?;
    audit_log::append(
        pool,
        "contact.anonymize",
        "contact",
        id,
        Some(
            &serde_json::json!({
                "retainedInvoices": locked_invoices,
                "retainedQuotes": locked_quotes,
                "pausedRecurringInvoices": paused_templates,
                "pausedRecurringSubscriptions": paused_subscriptions,
            })
            .to_string(),
        ),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`contacts_anonymize`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactsAnonymizeArgs {
    pub id: String,
}

/// Anonymisiert den Kontakt (irreversibel). Siehe [`anonymize_core`].
pub async fn contacts_anonymize(
    ctx: &Arc<AppCtx>,
    args: ContactsAnonymizeArgs,
) -> Result<ContactRow> {
    let ContactsAnonymizeArgs { id } = args;
    let pool = ctx.pool()?;

    anonymize_core(&pool, &id).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
