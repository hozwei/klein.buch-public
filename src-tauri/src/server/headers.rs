//! Sicherheits-Header, CSP und Cache-Control (Plan W5.7) und Auslieferung der
//! SPA.
//!
//! Beim Start berechnet [`Csp::load`] die SHA-256 aller Inline-Skripte in
//! `WEB_DIR/index.html` (Bootstrap von adapter-static); `'unsafe-inline'` für
//! Skripte gibt es nicht. Auf allen Antworten: `nosniff`, `Referrer-Policy:
//! no-referrer`, COOP/CORP `same-origin`, `Permissions-Policy`. Die CSP hängt
//! von der [`ResponseClass`] ab; HSTS nur, wenn die Anfrage laut Proxy-Regel
//! HTTPS ist (W6). Kein `CorsLayer`.

use std::path::Path;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine as _;
use sha2::{Digest, Sha256};
use tower_http::services::{ServeDir, ServeFile};

use super::state::ServerState;
use crate::error::{Error, Result};

/// CSP der SPA-Seiten (mit Hashes der Inline-Skripte).
#[derive(Debug, Clone)]
pub struct Csp {
    pub spa: String,
}

/// CSP für JSON und sonstige Nicht-HTML-Antworten.
pub const CSP_NONE: &str = "default-src 'none'; frame-ancestors 'none'";
/// CSP für selbst erzeugte PDFs in der In-App-Ansicht (W7).
pub const CSP_PDF_INLINE: &str = "default-src 'none'; frame-ancestors 'self'";
/// CSP für Downloads fremder Inhalte (W7).
pub const CSP_DOWNLOAD: &str = "sandbox; default-src 'none'; frame-ancestors 'none'";
/// CSP der Lizenz- und Quellenseiten unter `/legal/` (W12): keine Skripte,
/// Stil nur aus `/legal/legal.css`.
pub const CSP_LEGAL: &str = "default-src 'none'; style-src 'self'; img-src 'self'; \
                             base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
pub const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=(), payment=()";
pub const CACHE_NO_STORE: &str = "no-store";
pub const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";
pub const CACHE_REVALIDATE: &str = "no-cache";

impl Csp {
    pub fn from_index_html(html: &str) -> Self {
        let hashes = inline_script_hashes(html)
            .into_iter()
            .map(|h| format!(" '{h}'"))
            .collect::<String>();
        Self {
            spa: format!(
                "default-src 'self'; script-src 'self'{hashes}; style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data: blob:; connect-src 'self'; frame-src 'self' blob:; \
                 object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
            ),
        }
    }

    /// Liest `index.html` aus dem Web-Verzeichnis; fehlt sie, bricht der
    /// Start ab.
    pub fn load(web_dir: &Path) -> Result<Self> {
        let index = web_dir.join("index.html");
        let html = std::fs::read_to_string(&index).map_err(|e| {
            Error::Config(format!(
                "{} nicht lesbar ({e}) — Frontend fehlt. Start abgebrochen.",
                index.display()
            ))
        })?;
        Ok(Self::from_index_html(&html))
    }
}

/// `sha256-…` je Inline-Skript (Inhalt zwischen `<script …>` ohne `src` und
/// `</script>`).
pub fn inline_script_hashes(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<script") {
        let tag_start = pos + start;
        let Some(tag_len) = lower[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_len + 1;
        let Some(close) = lower[tag_end..].find("</script") else {
            break;
        };
        let content_end = tag_end + close;
        let tag = &lower[tag_start..tag_end];
        if !tag.contains(" src=") && !tag.contains(" src =") {
            // Wie der HTML-Parser: CRLF und einzelnes CR werden zu LF.
            let text = html[tag_end..content_end]
                .replace("\r\n", "\n")
                .replace('\r', "\n");
            let digest = Sha256::digest(text.as_bytes());
            out.push(format!(
                "sha256-{}",
                base64::engine::general_purpose::STANDARD.encode(digest)
            ));
        }
        pos = content_end;
    }
    out
}

/// Art der Antwort für Header-Auswahl. Handler können sie als Extension
/// setzen (z. B. PDF inline in W7); sonst entscheidet der Pfad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseClass {
    Spa,
    Api,
    PdfInline,
    Download,
    ImmutableAsset,
    Asset,
    /// `/legal/` (W12)
    Legal,
}

fn classify(path: &str, response: &Response) -> ResponseClass {
    if let Some(class) = response.extensions().get::<ResponseClass>() {
        return *class;
    }
    if path.starts_with("/api/") || path == "/api" || path == "/healthz" {
        return ResponseClass::Api;
    }
    if path == "/legal" || path.starts_with("/legal/") {
        return ResponseClass::Legal;
    }
    if path.starts_with("/files/") {
        return ResponseClass::Download;
    }
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    // Nur erfolgreiche Antworten dürfen ein Jahr im Cache liegen (W14): eine
    // 503/421 unter /_app/immutable/ würde sonst den Browser dauerhaft brechen.
    let cacheable = matches!(response.status().as_u16(), 200 | 304);
    if is_html {
        ResponseClass::Spa
    } else if path.starts_with("/_app/immutable/") && cacheable {
        ResponseClass::ImmutableAsset
    } else if !cacheable {
        ResponseClass::Api
    } else {
        ResponseClass::Asset
    }
}

/// Setzt die Header einer Klasse (überschreibt vorhandene).
pub fn apply(class: ResponseClass, headers: &mut HeaderMap, csp: &Csp, https: bool) {
    let mut set = |name: HeaderName, value: &str| {
        if let Ok(v) = HeaderValue::from_str(value) {
            headers.insert(name, v);
        }
    };
    set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(header::REFERRER_POLICY, "no-referrer");
    set(
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    );
    set(
        HeaderName::from_static("cross-origin-resource-policy"),
        "same-origin",
    );
    set(
        HeaderName::from_static("permissions-policy"),
        PERMISSIONS_POLICY,
    );
    if https {
        set(
            header::STRICT_TRANSPORT_SECURITY,
            // ohne includeSubDomains: die Instanz kennt die Subdomains des
            // Betreibers nicht (Review W6)
            "max-age=31536000",
        );
    }
    let (policy, cache) = match class {
        ResponseClass::Spa => (csp.spa.as_str(), CACHE_NO_STORE),
        ResponseClass::Api => (CSP_NONE, CACHE_NO_STORE),
        ResponseClass::PdfInline => (CSP_PDF_INLINE, CACHE_NO_STORE),
        ResponseClass::Download => (CSP_DOWNLOAD, CACHE_NO_STORE),
        ResponseClass::ImmutableAsset => (CSP_NONE, CACHE_IMMUTABLE),
        ResponseClass::Asset => (CSP_NONE, CACHE_REVALIDATE),
        ResponseClass::Legal => (CSP_LEGAL, CACHE_REVALIDATE),
    };
    set(header::CONTENT_SECURITY_POLICY, policy);
    set(header::CACHE_CONTROL, cache);
}

/// Middleware für alle Antworten. HSTS nur, wenn die Anfrage laut
/// Proxy-Regel HTTPS war (`ClientInfo` der Policy-Middleware, W6).
pub async fn security_headers(
    State(st): State<Arc<ServerState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;
    let https = response
        .extensions()
        .get::<super::policy::ClientInfo>()
        .is_some_and(|c| c.https);
    let class = classify(&path, &response);
    apply(class, response.headers_mut(), &st.csp, https);
    response
}

/// Statische Auslieferung der SPA; unbekannte Pfade bekommen `index.html`
/// (Client-Routing).
pub fn spa_service(web_dir: &Path) -> ServeDir<ServeFile> {
    ServeDir::new(web_dir).fallback(ServeFile::new(web_dir.join("index.html")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_only_inline_scripts() {
        let html = r#"<html><head><script type="module" src="/_app/a.js"></script>
<SCRIPT>console.log(1)</SCRIPT></head><body><script>
  start();
</script></body></html>"#;
        let hashes = inline_script_hashes(html);
        assert_eq!(hashes.len(), 2);
        let expected =
            base64::engine::general_purpose::STANDARD.encode(Sha256::digest(b"console.log(1)"));
        assert_eq!(hashes[0], format!("sha256-{expected}"));
        let csp = Csp::from_index_html(html);
        assert!(csp.spa.contains(&format!("'sha256-{expected}'")));
        let script_src = csp
            .spa
            .split(';')
            .find(|d| d.trim_start().starts_with("script-src"))
            .unwrap();
        assert!(!script_src.contains("unsafe"), "{script_src}");
    }

    #[test]
    fn only_successful_immutable_assets_are_cached_for_a_year() {
        use axum::http::StatusCode;
        let with_status = |status: StatusCode| {
            let mut r = Response::new(axum::body::Body::empty());
            *r.status_mut() = status;
            r.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/javascript"),
            );
            r
        };
        let path = "/_app/immutable/chunks/app.js";
        assert_eq!(
            classify(path, &with_status(StatusCode::OK)),
            ResponseClass::ImmutableAsset
        );
        for status in [
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::MISDIRECTED_REQUEST,
        ] {
            assert_eq!(
                classify(path, &with_status(status)),
                ResponseClass::Api,
                "{status}"
            );
        }
    }
}
