//! Gemeinsame Helfer der Server-Tests (Plan W5): Instanz unter einem
//! Temp-Verzeichnis, Router per `tower::ServiceExt::oneshot`.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::Router;
use klein_buch_lib::app_ctx::{AppCtx, NoDesktop};
use klein_buch_lib::config::Paths;
use klein_buch_lib::server::config::{ServerConfig, StartupOptions};
use klein_buch_lib::server::state::ServerState;
use tower::ServiceExt as _;

pub const PASS: &str = "korrekt-pferd-batterie-klammer";
pub const INDEX_HTML: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<script>window.__kb_boot = 1;</script></head><body><div id=\"app\"></div></body></html>";

/// Testvorlage mit §19-Marker (wie `cores_test.rs`).
pub const TEMPLATE_TEST: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json.decode(sys.inputs.at("data-json"))
= Rechnung #data.invoice.number
#data.kleinunternehmer.hinweis_text
"#;

pub struct TestServer {
    pub state: Arc<ServerState>,
    pub app: Router,
    pub dir: tempfile::TempDir,
}

/// Legt Ressourcen (Seed-Quelle mit Testvorlage) und Web-Verzeichnis an.
pub fn prepare_dirs(root: &Path) -> Paths {
    let paths = Paths::under(root);
    let templates = paths.resource_dir.join("inputs").join("pdf-templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(templates.join("default.typ"), TEMPLATE_TEST).unwrap();
    std::fs::create_dir_all(&paths.web_dir).unwrap();
    std::fs::write(paths.web_dir.join("index.html"), INDEX_HTML).unwrap();
    std::fs::create_dir_all(paths.web_dir.join("_app/immutable")).unwrap();
    std::fs::write(
        paths.web_dir.join("_app/immutable/app.js"),
        "console.log(1);",
    )
    .unwrap();
    std::fs::write(paths.web_dir.join("favicon.png"), b"\x89PNG").unwrap();
    paths
}

pub fn ctx_for(paths: Paths) -> Arc<AppCtx> {
    Arc::new(AppCtx::new(
        paths,
        Arc::new(klein_buch_lib::notify::InboxOnly),
        Arc::new(klein_buch_lib::secrets::MemorySecretStore::default()),
        Arc::new(NoDesktop),
        "test",
    ))
}

pub fn local_config() -> ServerConfig {
    ServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        startup: StartupOptions {
            allow_unmounted: true,
            mountinfo: None,
        },
    }
}

/// Frische Instanz im Zustand `setup` (Prüfungen wie beim echten Start).
pub fn test_server() -> TestServer {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let dir = tempfile::tempdir().unwrap();
    let paths = prepare_dirs(dir.path());
    let state = klein_buch_lib::server::prepare(ctx_for(paths), &local_config()).unwrap();
    let app = klein_buch_lib::server::router(state.clone());
    TestServer { state, app, dir }
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Reply {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub async fn send(app: &Router, req: Request<Body>) -> Reply {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    Reply {
        status,
        headers,
        body,
    }
}

/// `POST` mit Pflicht-Header, optional Sitzung und Idempotenz-Schlüssel.
pub fn post(uri: &str, body: &str, cookie: Option<&str>, key: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-kleinbuch-request", "1");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-kb_session={c}"));
    }
    if let Some(k) = key {
        b = b.header("idempotency-key", k);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

pub fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method(Method::GET).uri(uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-kb_session={c}"));
    }
    b.body(Body::empty()).unwrap()
}

/// Sitzungs-Cookie aus `Set-Cookie`.
pub fn session_cookie(reply: &Reply) -> String {
    let raw = reply
        .headers
        .get(header::SET_COOKIE)
        .expect("Set-Cookie")
        .to_str()
        .unwrap();
    raw.split(';')
        .next()
        .unwrap()
        .strip_prefix("__Host-kb_session=")
        .unwrap()
        .to_string()
}

/// Einrichtung mit [`PASS`]; liefert den Sitzungs-Cookie.
pub async fn setup(app: &Router) -> String {
    let reply = send(
        app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "passphrase": PASS }).to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    session_cookie(&reply)
}

pub async fn login(app: &Router, pass: &str) -> Reply {
    send(
        app,
        post(
            "/api/auth/login",
            &serde_json::json!({ "passphrase": pass }).to_string(),
            None,
            None,
        ),
    )
    .await
}

/// Ruft einen Command auf und holt bei 202 das Job-Ergebnis ab.
pub async fn call(app: &Router, cookie: &str, name: &str, args: serde_json::Value) -> Reply {
    let reply = send(
        app,
        post(
            &format!("/api/cmd/{name}"),
            &args.to_string(),
            Some(cookie),
            None,
        ),
    )
    .await;
    if reply.status == StatusCode::ACCEPTED {
        let id = reply.json()["jobId"].as_str().unwrap().to_string();
        return wait_job(app, cookie, &id).await;
    }
    reply
}

/// Wartet auf das Ende eines Jobs; liefert die Job-Sicht.
pub async fn wait_job(app: &Router, cookie: &str, id: &str) -> Reply {
    for _ in 0..600 {
        let reply = send(app, get(&format!("/api/jobs/{id}"), Some(cookie))).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
        if reply.json()["state"] != "running" {
            return reply;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("Job {id} nicht fertig");
}
