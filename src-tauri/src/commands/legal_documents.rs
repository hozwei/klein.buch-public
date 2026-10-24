//! Tauri-Commands für Rechtsdokumente (AGB + Datenschutz), Block 8.
//!
//! Versionierte PDF-Uploads. Jede Version wird write-once archiviert
//! ([`crate::archive::store_bytes`], `ArchiveKind::LegalDocument`); die
//! Metadaten + der Aktiv-Status leben in `legal_documents`
//! ([`crate::db::repo::legal_documents`]).
//!
//! ## GoBD-Hardline
//!
//! - Append-only: kein Löschen, Kernfelder unveränderlich (DB-Trigger).
//! - Höchstens eine aktive Version pro `doc_type`.
//! - Die Verknüpfung an ein Angebot passiert NICHT hier, sondern bei der
//!   Bundle-/Versand-Erzeugung ([`crate::commands::quotes`]).

use crate::app_ctx::AppCtx;
use chrono::Datelike;
use std::sync::Arc;
use uuid::Uuid;

use crate::archive::{self, ArchiveKind};
use crate::db::models::LegalDocumentRow;
use crate::db::repo::{audit_log, legal_documents};
use crate::error::{Error, Result};

/// Argumente von [`legal_documents_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalDocumentsListArgs {}

pub async fn legal_documents_list(
    ctx: &Arc<AppCtx>,
    _args: LegalDocumentsListArgs,
) -> Result<Vec<LegalDocumentRow>> {
    let pool = ctx.pool()?;

    legal_documents::list(&pool).await
}

/// Argumente von [`legal_documents_upload`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalDocumentsUploadArgs {
    pub doc_type: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Aus `POST /api/upload/rechtsdokument` (Plan W7, nur PDF).
    pub upload_id: String,
}

/// Lädt eine neue Version eines Rechtsdokuments hoch. Das PDF wird write-once
/// archiviert; die neue Version ist zunächst **inaktiv** (separat aktivieren).
pub async fn legal_documents_upload(
    ctx: &Arc<AppCtx>,
    args: LegalDocumentsUploadArgs,
) -> Result<LegalDocumentRow> {
    let LegalDocumentsUploadArgs {
        doc_type,
        title,
        upload_id,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    if !legal_documents::is_valid_doc_type(&doc_type) {
        return Err(Error::Domain(format!(
            "Unbekannter Dokumenttyp '{doc_type}' (erlaubt: agb, privacy)"
        )));
    }
    // Upload-Art „rechtsdokument“ lässt nur PDF zu (Magic Bytes, Plan W7).
    let upload = ctx
        .uploads
        .read(&upload_id, &[crate::uploads::UploadKind::LegalDocument])?;
    let file_name = upload.file_name;
    let file_bytes = upload.bytes;

    let paths = ctx.paths.clone();
    let sanitized = sanitize_filename(&file_name);
    // Write-once-Pfad muss eindeutig sein → kurzer UUID-Token als Präfix
    // (mehrere Versionen mit gleichem Original-Namen kollisionsfrei).
    let token = &Uuid::now_v7().to_string()[..8];
    let archive_name = format!("{doc_type}-{token}-{sanitized}");
    let fiscal_year = crate::clock::today_berlin().year();

    let stored = archive::store_bytes(
        pool,
        &paths.archive_dir,
        fiscal_year,
        ArchiveKind::LegalDocument,
        &archive_name,
        "application/pdf",
        &file_bytes,
    )
    .await?;

    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| sanitized.clone());

    let row = legal_documents::create_version(pool, &doc_type, &stored.archive_id, &title).await?;

    audit_log::append(
        pool,
        "legal_document.upload",
        "legal_document",
        &row.id,
        Some(&format!(
            r#"{{"doc_type":"{}","version":{},"title":"{}","archive":"{}"}}"#,
            escape(&doc_type),
            row.version,
            escape(&title),
            stored.archive_id
        )),
    )
    .await?;
    ctx.uploads.consume(&upload_id);

    Ok(row)
}

/// Argumente von [`legal_documents_activate`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalDocumentsActivateArgs {
    pub id: String,
}

/// Aktiviert eine Version (deaktiviert die bisher aktive desselben Typs).
pub async fn legal_documents_activate(
    ctx: &Arc<AppCtx>,
    args: LegalDocumentsActivateArgs,
) -> Result<()> {
    let LegalDocumentsActivateArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    legal_documents::activate(pool, &id).await?;
    audit_log::append(pool, "legal_document.activate", "legal_document", &id, None).await?;
    Ok(())
}

/// Argumente von [`legal_documents_deactivate`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalDocumentsDeactivateArgs {
    pub id: String,
}

/// Deaktiviert eine Version (danach kein aktives Dokument dieses Typs).
pub async fn legal_documents_deactivate(
    ctx: &Arc<AppCtx>,
    args: LegalDocumentsDeactivateArgs,
) -> Result<()> {
    let LegalDocumentsDeactivateArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    legal_documents::deactivate(pool, &id).await?;
    audit_log::append(
        pool,
        "legal_document.deactivate",
        "legal_document",
        &id,
        None,
    )
    .await?;
    Ok(())
}

// ---- Helpers ---------------------------------------------------------------

/// Reduziert einen Upload-Dateinamen auf einen sicheren Basisnamen ohne
/// Pfad-Trennzeichen (`archive::store_bytes` lehnt `/ \ :` ab).
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "dokument.pdf".to_string()
    } else {
        trimmed.to_string()
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
