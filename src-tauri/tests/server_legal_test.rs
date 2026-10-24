//! Plan W12 — Lizenz- und Quellenseiten unter `/legal/` und
//! `GET /api/public/info` (AGPL §13): ohne Sitzung erreichbar, nur Dateien
//! direkt in `<RESOURCE_DIR>/legal` mit erlaubter Endung, kein Path-Traversal,
//! keine Verzeichnislisten, CSP ohne Skripte, Rate-Limit je Client.

mod server_support;

use axum::http::{header, StatusCode};
use klein_buch_lib::server::headers;
use klein_buch_lib::server::ratelimit::PUBLIC_PER_WINDOW;
use server_support::*;

/// Legt die Lizenzseiten der Test-Instanz an (wie die Dockerfile-Stufen).
fn write_legal(srv: &TestServer) -> std::path::PathBuf {
    let legal = srv.state.ctx.paths.resource_dir.join("legal");
    std::fs::create_dir_all(legal.join("unterordner")).unwrap();
    let files = [
        ("third-party-licenses.html", "<h1>Lizenzen Dritter</h1>"),
        ("legal.css", "body { font-family: sans-serif; }"),
        ("LICENSE.txt", "GNU AFFERO GENERAL PUBLIC LICENSE"),
        ("debian-pakete.tsv", "paket\tversion\n"),
        ("js-licenses.json", "[]"),
        ("notizen.md", "# nicht ausgeliefert"),
        (".versteckt.txt", "nein"),
    ];
    for (name, content) in files {
        std::fs::write(legal.join(name), content).unwrap();
    }
    std::fs::write(legal.join("unterordner/x.txt"), "x").unwrap();
    // neben dem Ordner: darf nie erreichbar sein
    std::fs::write(legal.join("../geheim.txt"), "geheim").unwrap();
    legal
}

#[tokio::test]
async fn legal_files_without_session_with_content_types() {
    let srv = test_server();
    write_legal(&srv);
    let cases = [
        ("/legal/", "text/html; charset=utf-8", "Lizenzen Dritter"),
        (
            "/legal/third-party-licenses.html",
            "text/html; charset=utf-8",
            "Lizenzen Dritter",
        ),
        ("/legal/legal.css", "text/css; charset=utf-8", "sans-serif"),
        ("/legal/LICENSE.txt", "text/plain; charset=utf-8", "AFFERO"),
        (
            "/legal/debian-pakete.tsv",
            "text/tab-separated-values; charset=utf-8",
            "paket",
        ),
        ("/legal/js-licenses.json", "application/json", "[]"),
    ];
    for (path, content_type, needle) in cases {
        let r = send(&srv.app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert_eq!(r.headers[header::CONTENT_TYPE], content_type, "{path}");
        assert!(r.text().contains(needle), "{path}: {}", r.text());
    }
}

#[tokio::test]
async fn index_prefers_index_html_and_redirects_without_slash() {
    let srv = test_server();
    let legal = write_legal(&srv);
    std::fs::write(legal.join("index.html"), "<h1>Übersicht</h1>").unwrap();
    let r = send(&srv.app, get("/legal/", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text().contains("Übersicht"));
    let r = send(&srv.app, get("/legal", None)).await;
    assert_eq!(r.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(r.headers[header::LOCATION], "/legal/");
}

#[tokio::test]
async fn traversal_unknown_extensions_and_listings_are_404() {
    let srv = test_server();
    write_legal(&srv);
    for path in [
        "/legal/../geheim.txt",
        "/legal/%2e%2e/geheim.txt",
        "/legal/..%2fgeheim.txt",
        "/legal/%2e%2e%2fgeheim.txt",
        "/legal/unterordner",
        "/legal/unterordner/",
        "/legal/unterordner/x.txt",
        "/legal/.versteckt.txt",
        "/legal/notizen.md",
        "/legal/fehlt.html",
        "/legal/LICENSE",
        "/legal/LICENSE.txt%00.html",
    ] {
        let r = send(&srv.app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{path}");
        assert!(!r.text().contains("geheim"), "{path}");
        assert!(!r.text().contains("__kb_boot"), "{path}: kein SPA-Fallback");
    }
}

#[tokio::test]
async fn missing_folder_is_404() {
    let srv = test_server();
    for path in ["/legal/", "/legal/third-party-licenses.html"] {
        let r = send(&srv.app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn legal_headers_forbid_scripts() {
    let srv = test_server();
    write_legal(&srv);
    let r = send(&srv.app, get("/legal/third-party-licenses.html", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    let csp = r.headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert_eq!(csp, headers::CSP_LEGAL);
    assert!(csp.contains("default-src 'none'"));
    assert!(csp.contains("style-src 'self'"));
    assert!(!csp.contains("script-src"), "{csp}");
    assert!(!csp.contains("unsafe"), "{csp}");
    assert!(csp.contains("frame-ancestors 'none'"));
    let expect = [
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cache-control", headers::CACHE_REVALIDATE),
    ];
    for (name, value) in expect {
        assert_eq!(r.headers[name], value, "{name}");
    }
    assert!(r.headers.get(header::SET_COOKIE).is_none());
    assert!(r.headers.get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn public_info_links_only_to_an_exact_state() {
    let srv = test_server();
    let r = send(&srv.app, get("/api/public/info", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    let info = r.json();
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["licenseSpdx"], "AGPL-3.0-or-later");
    let source_url = info["sourceUrl"].as_str().unwrap();
    assert!(source_url.starts_with("http"), "{info}");
    match info["commit"].as_str() {
        None => assert!(info["sourceLink"].is_null(), "dev: kein Link: {info}"),
        Some(commit) => {
            let link = info["sourceLink"].as_str().unwrap();
            assert!(link.starts_with(source_url), "{info}");
            assert!(
                link.ends_with(&format!("/tree/{commit}")) || link.contains("/releases/tag/"),
                "{info}"
            );
        }
    }
    let link = info["sourceLink"].as_str().unwrap_or_default();
    assert!(!link.contains("/main"), "nie auf einen Zweig: {info}");
}

#[tokio::test]
async fn public_routes_are_rate_limited_per_client() {
    let srv = test_server();
    write_legal(&srv);
    for i in 0..PUBLIC_PER_WINDOW {
        let path = if i % 2 == 0 {
            "/api/public/info"
        } else {
            "/legal/LICENSE.txt"
        };
        let r = send(&srv.app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::OK, "Anfrage {i}");
    }
    for path in ["/api/public/info", "/legal/LICENSE.txt"] {
        let r = send(&srv.app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{path}");
        assert!(r.headers.get(header::RETRY_AFTER).is_some(), "{path}");
        assert_eq!(r.json()["code"], "rate_limited");
    }
    // ein anderer Client (eigene Gegenstelle) ist nicht betroffen
    let req = from_peer(get("/api/public/info", None), "203.0.113.7:40000");
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::OK);
    // die Anmeldemaske bleibt erreichbar (eigenes Rate-Limit nur für Fehlversuche)
    let r = send(&srv.app, get("/api/auth/status", None)).await;
    assert_eq!(r.status, StatusCode::OK);
}
