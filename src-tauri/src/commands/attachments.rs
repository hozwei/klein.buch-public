//! Tauri-Commands für Anhänge.
//!
//! - **Lesen/Öffnen** ([`attachments_open`] / [`attachments_reveal`]): zeigt
//!   einen bereits archivierten Anhang (z. B. unterschriebener Vertrag, Beleg)
//!   über das opener-Plugin — gleiches Muster wie `invoices_open_pdf`.
//! - **Hinzufügen** ([`attachments_add`], Block 9): generischer Upload — Datei
//!   write-once archivieren ([`crate::archive::store_bytes`],
//!   `ArchiveKind::Attachment`) und mit einer Eltern-Entität verknüpfen
//!   (`attachments`-Tabelle). Wiederverwendbar für Kosten, Anlagen, etc.
//!
//! Der **primäre** Beleg einer Kosten-Position (`expenses.receipt_archive_id`)
//! läuft NICHT hier, sondern in [`crate::commands::expenses`] (eigene
//! `ArchiveKind::ExpenseOriginal`-Klassifizierung). Hier nur **zusätzliche**
//! Anhänge.

use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::archive::{self, ArchiveKind};
use crate::config::Paths;
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
    pub file_bytes: Vec<u8>,
    pub file_name: String,
    pub label: Option<String>,
}

/// Löst `archive_entries.file_path` zu einer Archive-ID auf.
async fn archive_file_path(pool: &SqlitePool, archive_entry_id: &str) -> Result<String> {
    use sqlx::Row;
    let row = sqlx::query("SELECT file_path FROM archive_entries WHERE id = ?")
        .bind(archive_entry_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            Error::Domain(format!("Archiv-Eintrag nicht gefunden: {archive_entry_id}"))
        })?;
    Ok(row.try_get("file_path")?)
}

/// Öffnet den archivierten Anhang im Standard-Programm des Betriebssystems.
#[tauri::command]
pub async fn attachments_open(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    archive_entry_id: String,
) -> Result<()> {
    let path = archive_file_path(pool.inner(), &archive_entry_id).await?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| Error::Other(anyhow::anyhow!("Anhang konnte nicht geöffnet werden: {e}")))?;
    Ok(())
}

/// Zeigt den archivierten Anhang im Datei-Explorer/Finder.
#[tauri::command]
pub async fn attachments_reveal(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    archive_entry_id: String,
) -> Result<()> {
    let path = archive_file_path(pool.inner(), &archive_entry_id).await?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| Error::Other(anyhow::anyhow!("Ordner konnte nicht geöffnet werden: {e}")))?;
    Ok(())
}

/// Listet die Anhänge einer Eltern-Entität (Anzeige-Sicht).
#[tauri::command]
pub async fn attachments_list(
    pool: State<'_, SqlitePool>,
    parent_type: String,
    parent_id: String,
) -> Result<Vec<AttachmentView>> {
    attachments::list_for_parent(pool.inner(), &parent_type, &parent_id).await
}

/// Generischer Anhang-Upload (Block 9): Datei write-once archivieren und mit der
/// Eltern-Entität verknüpfen. Liefert die aktualisierte Anhang-Liste.
#[tauri::command]
pub async fn attachments_add(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    args: AddAttachmentArgs,
) -> Result<Vec<AttachmentView>> {
    let pool = pool.inner();
    if !VALID_PARENT_TYPES.contains(&args.parent_type.as_str()) {
        return Err(Error::Domain(format!(
            "Unbekannter Anhang-Typ '{}'.",
            args.parent_type
        )));
    }
    if args.file_bytes.is_empty() {
        return Err(Error::Domain("Datei ist leer.".into()));
    }

    let paths = Paths::from_handle(&app)?;
    let sanitized = sanitize_filename(&args.file_name);
    let token = &uuid::Uuid::now_v7().to_string()[..8];
    let archive_name = format!("{}-{token}-{sanitized}", args.parent_type);
    let mime = guess_mime(&sanitized);
    let fiscal_year = Local::now().year();

    let stored = archive::store_bytes(
        pool,
        &paths.archive_dir,
        fiscal_year,
        ArchiveKind::Attachment,
        &archive_name,
        mime,
        &args.file_bytes,
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
        Some(&format!(
            r#"{{"attachment":"{}","archive":"{}","file":"{}"}}"#,
            attachment_id,
            stored.archive_id,
            escape(&sanitized)
        )),
    )
    .await?;

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

/// Grobe MIME-Erkennung anhand der Dateiendung — für Belege (PDF/Bild/XML).
pub(crate) fn guess_mime(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "pdf" => "application/pdf",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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

    #[test]
    fn guess_mime_maps_common_types() {
        assert_eq!(guess_mime("beleg.PDF"), "application/pdf");
        assert_eq!(guess_mime("scan.jpeg"), "image/jpeg");
        assert_eq!(guess_mime("rechnung.xml"), "application/xml");
        assert_eq!(guess_mime("unbekannt"), "application/octet-stream");
    }
}
