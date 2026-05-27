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

use chrono::Datelike;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::archive::{self, ArchiveKind};
use crate::config::Paths;
use crate::db::models::LegalDocumentRow;
use crate::db::repo::{audit_log, legal_documents};
use crate::error::{Error, Result};

#[tauri::command]
pub async fn legal_documents_list(pool: State<'_, SqlitePool>) -> Result<Vec<LegalDocumentRow>> {
    legal_documents::list(pool.inner()).await
}

/// Lädt eine neue Version eines Rechtsdokuments hoch. Das PDF wird write-once
/// archiviert; die neue Version ist zunächst **inaktiv** (separat aktivieren).
#[tauri::command]
pub async fn legal_documents_upload(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    doc_type: String,
    title: Option<String>,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<LegalDocumentRow> {
    let pool = pool.inner();
    if !legal_documents::is_valid_doc_type(&doc_type) {
        return Err(Error::Domain(format!(
            "Unbekannter Dokumenttyp '{doc_type}' (erlaubt: agb, privacy)"
        )));
    }
    if file_bytes.is_empty() {
        return Err(Error::Domain("Datei ist leer.".into()));
    }
    if !file_bytes.starts_with(b"%PDF") {
        return Err(Error::Domain(
            "Es werden nur PDF-Dateien akzeptiert (die Datei beginnt nicht mit %PDF).".into(),
        ));
    }

    let paths = Paths::from_handle(&app)?;
    let sanitized = sanitize_filename(&file_name);
    // Write-once-Pfad muss eindeutig sein → kurzer UUID-Token als Präfix
    // (mehrere Versionen mit gleichem Original-Namen kollisionsfrei).
    let token = &Uuid::now_v7().to_string()[..8];
    let archive_name = format!("{doc_type}-{token}-{sanitized}");
    let fiscal_year = chrono::Local::now().year();

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

    Ok(row)
}

/// Aktiviert eine Version (deaktiviert die bisher aktive desselben Typs).
#[tauri::command]
pub async fn legal_documents_activate(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    let pool = pool.inner();
    legal_documents::activate(pool, &id).await?;
    audit_log::append(pool, "legal_document.activate", "legal_document", &id, None).await?;
    Ok(())
}

/// Deaktiviert eine Version (danach kein aktives Dokument dieses Typs).
#[tauri::command]
pub async fn legal_documents_deactivate(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    let pool = pool.inner();
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
