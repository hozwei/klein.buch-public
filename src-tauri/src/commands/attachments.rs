//! Commands für Anhänge.
//!
//! - **Lesen** ([`attachments_list`]): Anhänge einer Eltern-Entität; die Datei
//!   selbst liefert `GET /files/archive/{id}` (W7).
//! - **Hinzufügen** ([`attachments_add`], Block 9): generischer Upload — Datei
//!   write-once archivieren ([`crate::archive::store_bytes`],
//!   `ArchiveKind::Attachment`) und mit einer Eltern-Entität verknüpfen
//!   (`attachments`-Tabelle). Wiederverwendbar für Kosten, Anlagen, etc.
//!
//! Der **primäre** Beleg einer Kosten-Position (`expenses.receipt_archive_id`)
//! läuft NICHT hier, sondern in [`crate::commands::expenses`] (eigene
//! `ArchiveKind::ExpenseOriginal`-Klassifizierung). Hier nur **zusätzliche**
//! Anhänge.

use crate::app_ctx::AppCtx;
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::archive::{self, ArchiveKind};
use crate::db::models::AttachmentView;
use crate::db::repo::{attachments, audit_log};
use crate::error::{Error, Result};

/// Erlaubte Eltern-Typen — synchron zum CHECK in `0001_init.sql`.
const VALID_PARENT_TYPES: &[&str] = &[
    "invoice",
    "quote",
    "expense",
    "asset",
    "contact",
    "recurring",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAttachmentArgs {
    pub parent_type: String,
    pub parent_id: String,
    /// Aus `POST /api/upload/anhang` (Plan W7).
    pub upload_id: String,
    pub label: Option<String>,
}

/// Argumente von [`attachments_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentsListArgs {
    pub parent_type: String,
    pub parent_id: String,
}

/// Listet die Anhänge einer Eltern-Entität (Anzeige-Sicht).
pub async fn attachments_list(
    ctx: &Arc<AppCtx>,
    args: AttachmentsListArgs,
) -> Result<Vec<AttachmentView>> {
    let AttachmentsListArgs {
        parent_type,
        parent_id,
    } = args;
    let pool = ctx.pool()?;

    attachments::list_for_parent(&pool, &parent_type, &parent_id).await
}

/// Argumente von [`attachments_add`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentsAddArgs {
    pub args: AddAttachmentArgs,
}

/// Generischer Anhang-Upload (Block 9): Datei write-once archivieren und mit der
/// Eltern-Entität verknüpfen. Liefert die aktualisierte Anhang-Liste.
pub async fn attachments_add(
    ctx: &Arc<AppCtx>,
    args: AttachmentsAddArgs,
) -> Result<Vec<AttachmentView>> {
    let AttachmentsAddArgs { args } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    if !VALID_PARENT_TYPES.contains(&args.parent_type.as_str()) {
        return Err(Error::Domain(format!(
            "Unbekannter Anhang-Typ '{}'.",
            args.parent_type
        )));
    }
    let upload = ctx
        .uploads
        .read(&args.upload_id, &[crate::uploads::UploadKind::Attachment])?;

    let paths = ctx.paths.clone();
    let sanitized = sanitize_filename(&upload.file_name);
    let token = &uuid::Uuid::now_v7().to_string()[..8];
    let archive_name = format!("{}-{token}-{sanitized}", args.parent_type);
    // Typ aus der Inhaltsprüfung des Uploads, nicht aus der Endung (Plan W7).
    let mime = upload.mime;
    let fiscal_year = crate::clock::today_berlin().year();

    let stored = archive::store_bytes(
        pool,
        &paths.archive_dir,
        fiscal_year,
        ArchiveKind::Attachment,
        &archive_name,
        mime,
        &upload.bytes,
    )
    .await?;

    let label = args
        .label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let sort = attachments::count_for_parent(pool, &args.parent_type, &args.parent_id).await?;
    let attachment_id = attachments::create(
        pool,
        &args.parent_type,
        &args.parent_id,
        &stored.archive_id,
        label,
        sort,
    )
    .await?;

    audit_log::append(
        pool,
        "attachment.add",
        &args.parent_type,
        &args.parent_id,
        Some(serde_json::json!({ "attachment": attachment_id, "archive": stored.archive_id, "file": &sanitized })),
    )
    .await?;

    // Erst nach Erfolg freigeben (ADR 0039 Nr. 69).
    ctx.uploads.consume(&args.upload_id);
    attachments::list_for_parent(pool, &args.parent_type, &args.parent_id).await
}

// ---- Shared Helpers (von expenses/private_movements mitgenutzt) ------------

/// Reduziert einen Upload-Dateinamen auf einen sicheren Basisnamen ohne
/// Pfad-Trennzeichen (`archive::store_bytes` lehnt `/ \ :` ab).
pub(crate) fn sanitize_filename(name: &str) -> String {
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
        "datei".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_paths() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("C:\\temp\\beleg.pdf"), "beleg.pdf");
        assert_eq!(sanitize_filename("   "), "datei");
    }
}
