//! Datei-Auslieferung (Plan W7): Downloads und Archiv.
//!
//! - `GET /files/download/{id}`: Exporte und Vorschauen aus dem
//!   [`DownloadStore`](crate::downloads::DownloadStore), gestreamt; einmalige
//!   Exporte werden danach gelöscht.
//! - `GET /files/archive/{id}`: Archivdatei über `read_and_verify` (SHA-256,
//!   Audit `archive.read`), Pfad per `canonicalize` unter `archive_dir`.
//!   `inline` nur für selbst erzeugte Rechnungs-/Storno-/Angebots-PDFs (CSP
//!   `frame-ancestors 'self'`), alles Fremde als `attachment` mit CSP `sandbox`.
//!
//! Beide verlangen eine Sitzung; der Inhaltstyp kommt nur aus einer
//! Allowlist, `Content-Disposition` folgt RFC 6266.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use super::errors::ApiError;
use super::headers::ResponseClass;
use super::state::ServerState;
use crate::downloads::{DownloadKind, DownloadStore};
use crate::error::Error;

/// Quellen, deren PDFs der Server selbst erzeugt (inline erlaubt).
const INLINE_SOURCES: &[&str] = &["issued_invoice_pdf", "issued_quote_pdf"];

/// Inhaltstyp nur aus dieser Liste, sonst `application/octet-stream`.
pub fn allowed_content_type(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "application/pdf" => "application/pdf",
        "image/png" => "image/png",
        "image/jpeg" | "image/jpg" => "image/jpeg",
        "image/webp" => "image/webp",
        "image/gif" => "image/gif",
        "image/tiff" => "image/tiff",
        "application/xml" | "text/xml" => "application/xml",
        "application/zip" => "application/zip",
        "text/csv" | "text/csv; charset=utf-8" => "text/csv; charset=utf-8",
        "application/json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// `Content-Disposition` nach RFC 6266: ASCII-Rückfall ohne `"`, `\` und
/// Steuerzeichen plus `filename*=UTF-8''…`.
pub fn content_disposition(disposition: &str, file_name: &str) -> String {
    let cleaned: String = file_name
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    let cleaned = if cleaned.is_empty() { "datei" } else { cleaned };
    let ascii: String = cleaned
        .chars()
        .map(|c| {
            if c.is_ascii() && c != '"' && c != '%' && c != ';' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = cleaned
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("{disposition}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

fn with_headers(
    mut response: Response,
    content_type: &str,
    disposition: String,
    class: ResponseClass,
) -> Response {
    let h = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(content_type) {
        h.insert(header::CONTENT_TYPE, v);
    }
    if let Ok(v) = HeaderValue::from_str(&disposition) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    response.extensions_mut().insert(class);
    response
}

/// Löscht das Verzeichnis eines einmaligen Downloads, sobald der Stream endet
/// (auch bei Abbruch).
struct DeleteOnDrop(Option<std::path::PathBuf>);

impl Drop for DeleteOnDrop {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            DownloadStore::remove_files(&p);
        }
    }
}

/// `GET /files/download/{id}`.
pub async fn download(
    State(st): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if st.authenticate(&headers).is_none() {
        return ApiError::unauthenticated().into_response();
    }
    let Some(entry) = st.ctx.downloads.take(&id) else {
        return ApiError::not_found().into_response();
    };
    let file = match tokio::fs::File::open(&entry.path).await {
        Ok(f) => f,
        Err(_) => return ApiError::not_found().into_response(),
    };
    // Ein geliehenes Backup (Verzeichnis-Ziel, Lauf F3c) bleibt liegen.
    let guard = DeleteOnDrop(
        (entry.kind == DownloadKind::Export && entry.owned).then(|| entry.path.clone()),
    );
    let stream = tokio_util::io::ReaderStream::new(file);
    let stream = futures_util::StreamExt::map(stream, move |chunk| {
        let _keep = &guard;
        chunk
    });
    let (disposition, class) = match entry.kind {
        DownloadKind::Preview if entry.mime == "application/pdf" => {
            ("inline", ResponseClass::PdfInline)
        }
        _ => ("attachment", ResponseClass::Download),
    };
    with_headers(
        Body::from_stream(stream).into_response(),
        allowed_content_type(entry.mime),
        content_disposition(disposition, &entry.file_name),
        class,
    )
}

/// `GET /files/archive/{id}`.
pub async fn archive(
    State(st): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    match archive_inner(&st, &id, &headers).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

async fn archive_inner(
    st: &ServerState,
    id: &str,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    st.authenticate(headers)
        .ok_or_else(ApiError::unauthenticated)?;
    let pool = st.ctx.pool()?;
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT file_path, file_name, mime_type, source FROM archive_entries WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ApiError::from_error(&Error::Db(e)))?;
    let (file_path, file_name, mime, source) = row.ok_or_else(ApiError::not_found)?;
    // Pfad muss (aufgelöst) im Archiv liegen.
    let root = st
        .ctx
        .paths
        .archive_dir
        .canonicalize()
        .map_err(|e| ApiError::internal(&format!("archive_dir: {e}")))?;
    let real = std::path::Path::new(&file_path)
        .canonicalize()
        .map_err(|e| ApiError::internal(&format!("Archivdatei {id}: {e}")))?;
    if !real.starts_with(&root) {
        return Err(ApiError::internal(&format!(
            "Archivdatei {id} liegt außerhalb des Archivs"
        )));
    }
    // Hash- und Größenprüfung; bei Manipulation kein Byte ausliefern.
    let bytes = crate::archive::read_and_verify(&pool, id)
        .await
        .map_err(|e| ApiError::from_error(&e))?;
    let content_type = allowed_content_type(&mime);
    let inline = INLINE_SOURCES.contains(&source.as_str()) && content_type == "application/pdf";
    let (disposition, class) = if inline {
        ("inline", ResponseClass::PdfInline)
    } else {
        ("attachment", ResponseClass::Download)
    };
    Ok(with_headers(
        (StatusCode::OK, bytes).into_response(),
        content_type,
        content_disposition(disposition, &file_name),
        class,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition_is_rfc6266_and_safe() {
        let d = content_disposition("attachment", "Rechnung \"Müller\"\r\n;x.pdf");
        assert!(
            d.starts_with("attachment; filename=\"Rechnung _M_ller__x.pdf\""),
            "{d}"
        );
        assert!(
            d.contains("filename*=UTF-8''Rechnung%20%22M%C3%BCller%22%3Bx.pdf"),
            "{d}"
        );
        assert!(!d.contains('\r') && !d.contains('\n'));
        let d = content_disposition("attachment", "../../etc/passwd");
        assert!(d.starts_with("attachment; filename=\"etcpasswd\""), "{d}");
        assert!(!d.contains("../"), "{d}");
    }

    #[test]
    fn content_type_allowlist() {
        assert_eq!(allowed_content_type("application/pdf"), "application/pdf");
        assert_eq!(
            allowed_content_type("text/html"),
            "application/octet-stream"
        );
        assert_eq!(
            allowed_content_type("image/svg+xml"),
            "application/octet-stream"
        );
    }
}
