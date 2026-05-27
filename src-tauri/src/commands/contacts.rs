//! Tauri-Commands für Kontakte (Block 2).
//!
//! Frontend ruft via `invoke("contacts_list", { includeArchived: false })` etc.
//! Argumente kommen camelCase rein (Tauri-Default), wir nennen sie hier
//! snake_case und überlassen die Konvertierung Tauri.

use crate::db::models::ContactRow;
use crate::db::repo::{audit_log, contacts};
use crate::domain::contact::ContactInput;
use crate::error::{Error, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn contacts_list(
    pool: State<'_, SqlitePool>,
    include_archived: Option<bool>,
) -> Result<Vec<ContactRow>> {
    contacts::list(pool.inner(), include_archived.unwrap_or(false)).await
}

#[tauri::command]
pub async fn contacts_get(pool: State<'_, SqlitePool>, id: String) -> Result<Option<ContactRow>> {
    contacts::get(pool.inner(), &id).await
}

#[tauri::command]
pub async fn contacts_create(
    pool: State<'_, SqlitePool>,
    input: ContactInput,
) -> Result<ContactRow> {
    contacts::create(pool.inner(), &input).await
}

#[tauri::command]
pub async fn contacts_update(
    pool: State<'_, SqlitePool>,
    id: String,
    input: ContactInput,
) -> Result<ContactRow> {
    contacts::update(pool.inner(), &id, &input).await
}

#[tauri::command]
pub async fn contacts_archive(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    contacts::archive(pool.inner(), &id).await
}

#[tauri::command]
pub async fn contacts_unarchive(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    contacts::unarchive(pool.inner(), &id).await
}

#[tauri::command]
pub async fn contacts_search(
    pool: State<'_, SqlitePool>,
    query: String,
    include_archived: Option<bool>,
) -> Result<Vec<ContactRow>> {
    contacts::search(pool.inner(), &query, include_archived.unwrap_or(false)).await
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

#[tauri::command]
pub async fn contacts_anonymize_check(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<AnonymizeCheck> {
    let pool = pool.inner();
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
    let row = contacts::anonymize(pool, id).await?;
    audit_log::append(
        pool,
        "contact.anonymize",
        "contact",
        id,
        Some(&format!(
            r#"{{"retainedInvoices":{locked_invoices},"retainedQuotes":{locked_quotes}}}"#
        )),
    )
    .await?;
    Ok(row)
}

/// Anonymisiert den Kontakt (irreversibel). Siehe [`anonymize_core`].
#[tauri::command]
pub async fn contacts_anonymize(pool: State<'_, SqlitePool>, id: String) -> Result<ContactRow> {
    anonymize_core(pool.inner(), &id).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
