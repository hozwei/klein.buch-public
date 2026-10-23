//! `POST /api/upload/{art}` (Plan W7): multipart, gestreamt nach
//! `<data>/tmp/uploads/<uuid>/datei`.
//!
//! Reihenfolge: Pflicht-Header → Sitzung → Art → `Content-Length` gegen das
//! Limit der Art (413) → freier Platz (`KLEIN_BUCH_MIN_FREE_BYTES`, 507) →
//! genau ein Feld `file` lesen, Bytes zählen (über dem Limit 413, Teil wird
//! gelöscht), je Datenblock höchstens [`IDLE_TIMEOUT`] warten → Typ per Magic
//! Bytes gegen die Allowlist der Art (415) → [`UploadTicket`].

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{FromRequest, Multipart, Path, Request, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::Json;
use tokio::io::AsyncWriteExt;

use super::dispatch::require_request_header;
use super::errors::ApiError;
use super::state::ServerState;
use crate::uploads::{free_bytes, sniff, UploadKind, UploadStore, UploadTicket};

/// Höchstens so lange ohne neue Daten, dann Abbruch (Schutz vor Slowloris).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// Zuschlag für Multipart-Rahmen und Header gegenüber dem Dateilimit.
const MULTIPART_OVERHEAD: u64 = 64 * 1024;
/// XML/SVG werden zur UTF-8-Prüfung ganz gelesen (alle XML-Arten ≤ 20 MiB).
const SNIFF_WHOLE_MAX: u64 = 32 * 1024 * 1024;

/// Löscht einen angefangenen Upload, falls er nicht registriert wird.
struct Partial(Option<std::path::PathBuf>);

impl Drop for Partial {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            UploadStore::remove_files(&p);
        }
    }
}

pub async fn upload(
    State(st): State<Arc<ServerState>>,
    Path(art): Path<String>,
    req: Request,
) -> Response {
    match upload_inner(&st, &art, req).await {
        Ok(ticket) => Json(ticket).into_response(),
        Err(e) => e.into_response(),
    }
}

fn declared_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
}

async fn upload_inner(st: &ServerState, art: &str, req: Request) -> Result<UploadTicket, ApiError> {
    let headers = req.headers().clone();
    require_request_header(&headers)?;
    st.authenticate(&headers)
        .ok_or_else(ApiError::unauthenticated)?;
    let kind = UploadKind::from_slug(art).ok_or_else(ApiError::not_found)?;
    if st.writes_blocked() {
        return Err(ApiError::restarting());
    }
    let store = &st.ctx.uploads;
    let limits = store.limits();
    let max = kind.max_bytes(&limits);
    let declared = declared_length(&headers);
    if declared.is_some_and(|len| len > max.saturating_add(MULTIPART_OVERHEAD)) {
        return Err(ApiError::payload_too_large());
    }
    let free = free_bytes(&st.ctx.paths.data_dir).map_err(|e| ApiError::internal(&e))?;
    let needed = limits
        .min_free_bytes
        .saturating_add(declared.unwrap_or(max).min(max));
    if free < needed {
        tracing::warn!(
            "Upload abgelehnt: {free} Bytes frei, {needed} nötig (KLEIN_BUCH_MIN_FREE_BYTES)"
        );
        return Err(ApiError::insufficient_storage());
    }

    let mut multipart = Multipart::from_request(req, &())
        .await
        .map_err(|_| ApiError::bad_request("Erwartet multipart/form-data mit dem Feld „file“."))?;
    let mut field = loop {
        let next = tokio::time::timeout(IDLE_TIMEOUT, multipart.next_field())
            .await
            .map_err(|_| ApiError::bad_request("Upload abgebrochen (keine Daten)."))?
            .map_err(|_| ApiError::bad_request("Multipart-Daten fehlerhaft."))?;
        match next {
            Some(f) if f.name() == Some("file") => break f,
            Some(_) => continue,
            None => return Err(ApiError::bad_request("Feld „file“ fehlt.")),
        }
    };
    let original_name = field.file_name().unwrap_or("").to_string();

    let (id, path) = store.begin().map_err(|e| ApiError::from_error(&e))?;
    let mut partial = Partial(Some(path.clone()));
    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|e| ApiError::internal(&format!("Upload-Datei anlegen: {e}")))?;
    let mut size: u64 = 0;
    let mut head: Vec<u8> = Vec::with_capacity(512);
    loop {
        let chunk = tokio::time::timeout(IDLE_TIMEOUT, field.chunk())
            .await
            .map_err(|_| ApiError::bad_request("Upload abgebrochen (Zeitüberschreitung)."))?
            .map_err(|_| ApiError::bad_request("Upload abgebrochen."))?;
        let Some(chunk) = chunk else { break };
        size = size.saturating_add(chunk.len() as u64);
        if size > max {
            return Err(ApiError::payload_too_large());
        }
        if head.len() < 512 {
            let take = (512 - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| ApiError::internal(&format!("Upload schreiben: {e}")))?;
    }
    file.flush()
        .await
        .map_err(|e| ApiError::internal(&format!("Upload schreiben: {e}")))?;
    drop(file);
    if size == 0 {
        return Err(ApiError::bad_request("Die Datei ist leer."));
    }

    let mut file_type = sniff(&head, None);
    let mut whole = None;
    if file_type.is_none() && size <= SNIFF_WHOLE_MAX {
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| ApiError::internal(&format!("Upload lesen: {e}")))?;
        file_type = sniff(&head, Some(&bytes));
        whole = Some(bytes);
    }
    let file_type = file_type
        .filter(|t| kind.allowed().contains(t))
        .ok_or_else(|| ApiError::unsupported_media_type(kind.type_error(whole.as_deref())))?;

    partial.0 = None;
    Ok(store.register(id, &path, kind, &original_name, file_type, size))
}
