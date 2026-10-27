//! Lizenz- und Quellenseiten unter `/legal/` (Plan W12, ADR 0014/0039).
//!
//! Ohne Sitzung erreichbar (Plan §3), mit dem Rate-Limit der öffentlichen
//! Routen. Ausgeliefert werden nur Dateien **direkt** in
//! `<RESOURCE_DIR>/legal` (im Image `/opt/klein-buch/legal`, erzeugt in den
//! Dockerfile-Stufen): Name nur aus `A-Za-z0-9._-`, kein führender Punkt,
//! Endung aus [`content_type_for`]; der Pfad muss nach `canonicalize` im
//! Ordner selbst liegen (kein Path-Traversal, auch nicht über Symlinks).
//! Keine Verzeichnislisten. `/legal/` liefert die erste vorhandene Datei aus
//! [`INDEX_FILES`], `/legal` leitet dorthin um. Header setzt
//! [`super::headers`] mit [`ResponseClass::Legal`] (CSP ohne Skripte).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use super::errors::ApiError;
use super::headers::ResponseClass;
use super::state::ServerState;

/// Unterordner von `RESOURCE_DIR`.
pub const LEGAL_DIR: &str = "legal";
/// Startseite von `/legal/` (die erste vorhandene).
pub const INDEX_FILES: &[&str] = &["index.html", "third-party-licenses.html"];
/// Größte ausgelieferte Datei (die Lizenzseite mit allen Texten ist einige MB).
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Inhaltstyp nach Endung; andere Endungen werden nicht ausgeliefert.
pub fn content_type_for(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "html" => Some("text/html; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "txt" => Some("text/plain; charset=utf-8"),
        "tsv" => Some("text/tab-separated-values; charset=utf-8"),
        "json" => Some("application/json"),
        _ => None,
    }
}

/// Ein einzelnes Pfadsegment ohne Sonderzeichen und ohne führenden Punkt.
fn valid_name(name: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    !name.is_empty() && name.len() <= 128 && !name.starts_with('.') && name.chars().all(allowed)
}

/// Datei direkt in `legal_dir` samt Inhaltstyp, sonst `None`.
pub fn resolve(legal_dir: &Path, name: &str) -> Option<(PathBuf, &'static str)> {
    if !valid_name(name) {
        return None;
    }
    content_type_for(name)?;
    let base = legal_dir.canonicalize().ok()?;
    let path = base.join(name).canonicalize().ok()?;
    if !path.starts_with(&base) || path.parent() != Some(base.as_path()) {
        return None;
    }
    // Inhaltstyp nach dem tatsächlichen Ziel (ein Symlink im Ordner darf
    // keine andere Endung vortäuschen).
    let target_name = path.file_name()?.to_str()?;
    let content_type = content_type_for(target_name)?;
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    Some((path, content_type))
}

fn legal_dir(st: &ServerState) -> PathBuf {
    st.ctx.paths.resource_dir.join(LEGAL_DIR)
}

async fn serve(dir: &Path, name: &str) -> Option<Response> {
    let (path, content_type) = resolve(dir, name)?;
    // Lauf F4b (W12-LEGAL-TIMEOUTS): gestreamt statt je Anfrage komplett in den
    // Speicher gelesen (die Lizenzseite ist einige MB groß); langsame Leser
    // begrenzt der Schreib-Leerlauf-Timeout der Verbindung (`http`).
    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("/legal/{name} nicht lesbar: {e}");
            return None;
        }
    };
    let length = file.metadata().await.ok().map(|m| m.len());
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(file));
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Some(len) = length {
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    }
    response.extensions_mut().insert(ResponseClass::Legal);
    Some(response)
}

/// `GET /legal` → `/legal/` (relative Verweise der Seiten).
pub async fn redirect() -> Response {
    (
        StatusCode::PERMANENT_REDIRECT,
        [(header::LOCATION, "/legal/")],
    )
        .into_response()
}

/// `GET /legal/`: Startseite.
pub async fn index(State(st): State<Arc<ServerState>>) -> Response {
    let dir = legal_dir(&st);
    for name in INDEX_FILES {
        if let Some(response) = serve(&dir, name).await {
            return response;
        }
    }
    ApiError::not_found().into_response()
}

/// `GET /legal/{datei}`.
pub async fn file(State(st): State<Arc<ServerState>>, UrlPath(name): UrlPath<String>) -> Response {
    match serve(&legal_dir(&st), &name).await {
        Some(response) => response,
        None => ApiError::not_found().into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_types() {
        let html = content_type_for("a.HTML");
        assert_eq!(html, Some("text/html; charset=utf-8"));
        let tsv = content_type_for("debian-pakete.tsv").unwrap_or_default();
        assert!(tsv.starts_with("text/tab-separated-values"), "{tsv}");
        for bad in ["LICENSE", "x.md", "x.js", "x.svg", "x.html.bak"] {
            assert_eq!(content_type_for(bad), None, "{bad}");
        }
        for bad in ["", ".x.txt", "a/b.txt", "..", "a\\b.txt", "a b.txt"] {
            assert!(!valid_name(bad), "{bad}");
        }
        assert!(!valid_name("ü.txt"));
        assert!(valid_name("third-party-licenses.html"));
    }

    #[test]
    fn resolve_stays_inside_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(LEGAL_DIR);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("LICENSE.txt"), "AGPL").unwrap();
        std::fs::write(dir.join("sub/x.txt"), "x").unwrap();
        let outside = root.path().join("secret.txt");
        std::fs::write(&outside, "geheim").unwrap();
        assert!(resolve(&dir, "LICENSE.txt").is_some());
        assert!(resolve(&dir, "sub").is_none(), "kein Verzeichnis");
        assert!(resolve(&dir, "fehlt.txt").is_none());
        #[cfg(unix)]
        {
            let linked = std::os::unix::fs::symlink(&outside, dir.join("link.txt"));
            assert!(linked.is_ok());
            assert!(resolve(&dir, "link.txt").is_none(), "Symlink nach außen");
        }
        assert!(resolve(&root.path().join("fehlt"), "LICENSE.txt").is_none());
    }
}
