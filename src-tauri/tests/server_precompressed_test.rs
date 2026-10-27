//! Lauf 2026.11 U4 (ADR 0041 Nr. 212): vorkomprimierte SPA-Dateien.
//!
//! Der Frontend-Build legt neben Text-Dateien `<datei>.br` und `<datei>.gz` ab
//! (`vite.config.js`, Plugin `klein-buch-precompress`; die Rundreise prüft
//! `src/precompress.test.ts`). Der Server wählt sie nach `Accept-Encoding`,
//! sonst roh, und setzt `Vary: Accept-Encoding`. API, `/files/`, `/healthz`
//! und `/legal/` werden nie komprimiert — auch dann nicht, wenn im
//! Web-Verzeichnis gleichnamige Dateien mit Varianten lägen.

mod server_support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use klein_buch_lib::server::headers;
use server_support::*;

/// Asset unter `/_app/immutable/` (über 1 KiB wie die echten Chunks).
const ASSET: &str = "/_app/immutable/chunks/kb.js";

fn js() -> String {
    "console.log(\"klein-buch\");\n".repeat(40)
}

// Erzeugt mit node:zlib aus `js()`, Einstellungen wie in `vite.config.js`:
// brotliCompressSync(MODE_TEXT, QUALITY 11, SIZE_HINT) bzw. gzipSync(level 9).
const JS_BR: &[u8] = &[
    0x1b, 0x37, 0x04, 0xf8, 0x8d, 0x93, 0x5c, 0xf5, 0x65, 0xd8, 0x37, 0xb6, 0xbe, 0xc5, 0xe0, 0x0e,
    0x12, 0x22, 0x96, 0x52, 0xd8, 0xa9, 0x42, 0x2a, 0x2c, 0x15, 0x78, 0xf1, 0xea, 0xba, 0xcc, 0x05,
    0x42, 0x8f, 0xd7, 0xb2, 0xbe, 0x25, 0x50, 0x00,
];
const JS_GZ: &[u8] = &[
    0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0x4b, 0xce, 0xcf, 0x2b, 0xce, 0xcf,
    0x49, 0xd5, 0xcb, 0xc9, 0x4f, 0xd7, 0x50, 0xca, 0xce, 0x49, 0xcd, 0xcc, 0xd3, 0x4d, 0x2a, 0x4d,
    0xce, 0x50, 0xd2, 0xb4, 0xe6, 0x4a, 0x1e, 0x95, 0x1a, 0x95, 0x1a, 0x95, 0x1a, 0x95, 0xc2, 0x26,
    0x05, 0x00, 0x68, 0x69, 0xbf, 0x75, 0x38, 0x04, 0x00, 0x00,
];
// Brotli von `INDEX_HTML` (wie oben).
const INDEX_BR: &[u8] = &[
    0x1b, 0x85, 0x00, 0x10, 0x44, 0x27, 0x47, 0x7a, 0xd0, 0xbc, 0x22, 0xfd, 0xde, 0x8a, 0x92, 0x3b,
    0xe5, 0xc0, 0xa9, 0x2c, 0xe0, 0x3c, 0xa1, 0x96, 0xd0, 0xe7, 0xd7, 0xc9, 0xe1, 0x41, 0x6e, 0xe7,
    0xd3, 0xad, 0xb5, 0x34, 0x4a, 0xb8, 0x45, 0x4a, 0x3c, 0x4e, 0xdb, 0x9a, 0xcd, 0xe0, 0x86, 0xfc,
    0xe1, 0x15, 0x6f, 0x21, 0xa6, 0x75, 0xa8, 0xd6, 0xb3, 0xec, 0x87, 0x66, 0x94, 0x01,
];

/// CRC-32 (IEEE) wie im gzip-Trailer.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn get_encoded(uri: &str, accept_encoding: &str) -> Request<Body> {
    let mut req = get(uri, None);
    req.headers_mut()
        .insert(header::ACCEPT_ENCODING, accept_encoding.parse().unwrap());
    req
}

/// Instanz mit dem Asset samt Varianten und `index.html.br`.
fn server_with_variants() -> TestServer {
    let srv = test_server();
    let web = &srv.state.ctx.paths.web_dir;
    let chunks = web.join("_app/immutable/chunks");
    std::fs::create_dir_all(&chunks).unwrap();
    std::fs::write(chunks.join("kb.js"), js()).unwrap();
    std::fs::write(chunks.join("kb.js.br"), JS_BR).unwrap();
    std::fs::write(chunks.join("kb.js.gz"), JS_GZ).unwrap();
    std::fs::write(web.join("index.html.br"), INDEX_BR).unwrap();
    srv
}

fn encoding(r: &Reply) -> Option<&str> {
    r.headers
        .get(header::CONTENT_ENCODING)
        .map(|v| v.to_str().unwrap())
}

fn assert_vary(r: &Reply, what: &str) {
    assert_eq!(
        r.headers.get(header::VARY).map(|v| v.to_str().unwrap()),
        Some("accept-encoding"),
        "{what}"
    );
}

#[tokio::test]
async fn asset_variant_follows_accept_encoding() {
    let srv = server_with_variants();
    let raw = js();
    // Fixture passt zum Inhalt: gzip-Trailer (CRC-32, Länge) der Rohdatei.
    let trailer = &JS_GZ[JS_GZ.len() - 8..];
    assert_eq!(trailer[..4], crc32(raw.as_bytes()).to_le_bytes());
    assert_eq!(trailer[4..], (raw.len() as u32).to_le_bytes());

    for (accept, want, body) in [
        ("br", Some("br"), JS_BR),
        ("gzip", Some("gzip"), JS_GZ),
        // Chrome/Firefox: Brotli vor gzip
        ("gzip, deflate, br, zstd", Some("br"), JS_BR),
        ("br;q=0, gzip", Some("gzip"), JS_GZ),
        ("gzip;q=0.5, br;q=0.4", Some("gzip"), JS_GZ),
        ("identity", None, raw.as_bytes()),
        ("deflate", None, raw.as_bytes()),
    ] {
        let r = send(&srv.app, get_encoded(ASSET, accept)).await;
        assert_eq!(r.status, StatusCode::OK, "{accept}");
        assert_eq!(encoding(&r), want, "{accept}");
        assert_eq!(r.body.as_ref(), body, "{accept}");
        assert_eq!(
            r.headers.get(header::CONTENT_LENGTH).unwrap(),
            &body.len().to_string(),
            "{accept}"
        );
        // Inhaltstyp und Header-Klasse wie bei der Rohdatei
        assert_eq!(
            r.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/javascript; charset=utf-8",
            "{accept}"
        );
        assert_eq!(
            r.headers.get(header::CACHE_CONTROL).unwrap(),
            headers::CACHE_IMMUTABLE,
            "{accept}"
        );
        assert_eq!(
            r.headers.get(header::CONTENT_SECURITY_POLICY).unwrap(),
            headers::CSP_NONE,
            "{accept}"
        );
        assert_vary(&r, accept);
    }

    // ohne Accept-Encoding: roh, aber ebenfalls mit Vary
    let r = send(&srv.app, get(ASSET, None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(encoding(&r), None);
    assert_eq!(r.body.as_ref(), raw.as_bytes());
    assert_vary(&r, "ohne Accept-Encoding");

    // Datei ohne Varianten (unter 1 KiB): roh
    let r = send(&srv.app, get_encoded("/_app/immutable/app.js", "gzip, br")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(encoding(&r), None);
    assert_eq!(r.text(), "console.log(1);");
    assert_vary(&r, "ohne Varianten");
}

/// `/` und der `index.html`-Fallback des Client-Routings nehmen die Variante;
/// CSP (Hashes aus der Rohdatei) und `no-store` bleiben.
#[tokio::test]
async fn spa_index_and_fallback_use_variant() {
    let srv = server_with_variants();
    for path in ["/", "/index.html", "/invoices", "/settings/backup"] {
        let r = send(&srv.app, get_encoded(path, "gzip, deflate, br")).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert_eq!(encoding(&r), Some("br"), "{path}");
        assert_eq!(r.body.as_ref(), INDEX_BR, "{path}");
        assert_eq!(
            r.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8",
            "{path}"
        );
        assert_eq!(
            r.headers.get(header::CONTENT_SECURITY_POLICY).unwrap(),
            srv.state.csp.spa.as_str(),
            "{path}"
        );
        assert_eq!(
            r.headers.get(header::CACHE_CONTROL).unwrap(),
            headers::CACHE_NO_STORE,
            "{path}"
        );
        assert_vary(&r, path);

        // nur gzip angenommen, aber keine index.html.gz: roh
        let r = send(&srv.app, get_encoded(path, "gzip")).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert_eq!(encoding(&r), None, "{path}");
        assert_eq!(r.text(), INDEX_HTML, "{path}");
        assert_vary(&r, path);
    }
}

/// API, Downloads/Archiv, Healthcheck und `/legal/` laufen nicht über die
/// statische Auslieferung: nie `Content-Encoding`, kein `Vary` — auch mit
/// gleichnamigen Dateien samt Varianten im Web-Verzeichnis.
#[tokio::test]
async fn api_files_and_legal_are_never_compressed() {
    let srv = server_with_variants();
    let web = &srv.state.ctx.paths.web_dir;
    for trap in [
        "api/public/info",
        "api/auth/status",
        "files/download/x",
        "files/archive/x",
        "files/x",
        "healthz",
        "legal/index.html",
    ] {
        let file = web.join(trap);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        for suffix in ["", ".br", ".gz"] {
            let mut name = file.clone().into_os_string();
            name.push(suffix);
            std::fs::write(name, "falle").unwrap();
        }
    }
    let accept = "gzip, deflate, br, zstd";
    for path in [
        "/api/public/info",
        "/api/auth/status",
        "/api/gibt-es-nicht",
        "/files/download/x",
        "/files/archive/x",
        "/files/x",
        "/healthz",
        "/legal/",
        "/legal/index.html",
    ] {
        let r = send(&srv.app, get_encoded(path, accept)).await;
        assert_eq!(encoding(&r), None, "{path}");
        assert!(r.headers.get(header::VARY).is_none(), "{path}");
        assert!(!r.text().contains("falle"), "{path}: {}", r.text());
    }
    // die öffentliche Info kommt als lesbares JSON
    let r = send(&srv.app, get_encoded("/api/public/info", accept)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.json()["version"].is_string(), "{}", r.text());
}
