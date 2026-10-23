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
use klein_buch_lib::server::policy::{HttpPolicy, PublicUrl};
use klein_buch_lib::server::state::ServerState;
use tower::ServiceExt as _;

pub const PASS: &str = "korrekt-pferd-batterie-klammer";
/// Öffentliche Adresse der Test-Instanz (Host-Header aller Test-Anfragen).
pub const HOST: &str = "kb.example.org";
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
        // `oneshot` hat keine Gegenstelle (kein Loopback) → unsicheres HTTP
        // ausdrücklich erlaubt, wie in CI.
        policy: HttpPolicy {
            public_url: PublicUrl::parse(&format!("http://{HOST}")).unwrap(),
            trusted_proxies: Vec::new(),
            insecure_http: true,
        },
    }
}

/// Frische Instanz im Zustand `setup` (Prüfungen wie beim echten Start).
pub fn test_server() -> TestServer {
    test_server_with(|_| {})
}

/// Wie [`test_server`], mit angepasster Konfiguration (z. B. Policy).
pub fn test_server_with(adjust: impl FnOnce(&mut ServerConfig)) -> TestServer {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let dir = tempfile::tempdir().unwrap();
    let paths = prepare_dirs(dir.path());
    let mut cfg = local_config();
    adjust(&mut cfg);
    let state = klein_buch_lib::server::prepare(ctx_for(paths), &cfg).unwrap();
    let app = klein_buch_lib::server::router(state.clone());
    TestServer { state, app, dir }
}

/// Setzt die Gegenstelle einer Anfrage (wie die Annahme-Schleife).
pub fn from_peer(mut req: Request<Body>, peer: &str) -> Request<Body> {
    let addr: std::net::SocketAddr = peer.parse().unwrap();
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(addr));
    req
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
        .header(header::HOST, HOST)
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
    let mut b = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::HOST, HOST);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-kb_session={c}"));
    }
    b.body(Body::empty()).unwrap()
}

/// Sitzungs-Cookie aus `Set-Cookie`.
pub fn session_cookie(reply: &Reply) -> String {
    reply
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|raw| {
            raw.split(';')
                .next()
                .and_then(|c| c.strip_prefix("__Host-kb_session="))
                .map(str::to_string)
        })
        .expect("Set-Cookie der Sitzung")
}

/// TOTP-Geheimnis der Instanz (aus `/data/auth/totp.json`).
pub fn totp_secret(srv: &TestServer) -> Vec<u8> {
    let path = srv.state.ctx.paths.data_dir.join("auth").join("totp.json");
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("totp.json")).unwrap();
    klein_buch_lib::server::totp::decode_secret(file["secret_b32"].as_str().unwrap()).unwrap()
}

/// Nächster gültiger TOTP-Code: verschiebt die TOTP-Uhr der Instanz um drei
/// Schritte (jeder Schritt gilt nur einmal) und rechnet den Code aus.
pub fn next_totp(srv: &TestServer) -> String {
    srv.state.shift_totp_clock_for_tests(90);
    let secret = totp_secret(srv);
    klein_buch_lib::server::totp::code_for_step(
        &secret,
        klein_buch_lib::server::totp::step_at(srv.state.totp_now()),
    )
}

/// Serialisiert die KDF-lastigen Abläufe (Einrichtung, Anmeldung, Re-Auth)
/// innerhalb eines Test-Binaries: parallel im Debug-Build reißen Argon2 und
/// PBKDF2 sonst den 30-s-Timeout der Auth-Routen.
static KDF_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn kdf_serial() -> tokio::sync::MutexGuard<'static, ()> {
    KDF_SERIAL.lock().await
}

/// Einrichtung (Setup-Token + TOTP + [`PASS`]); liefert die Antwort.
pub async fn setup_reply(srv: &TestServer) -> Reply {
    let _serial = kdf_serial().await;
    let token = srv.state.setup_token_for_tests().expect("Setup-Token");
    let offer = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "step": "totp" }).to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(offer.status, StatusCode::OK, "{}", offer.text());
    let secret =
        klein_buch_lib::server::totp::decode_secret(offer.json()["secret"].as_str().unwrap())
            .unwrap();
    let code = klein_buch_lib::server::totp::code_for_step(
        &secret,
        klein_buch_lib::server::totp::step_at(srv.state.totp_now()),
    );
    send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "passphrase": PASS, "totpCode": code })
                .to_string(),
            None,
            None,
        ),
    )
    .await
}

/// Einrichtung; liefert den Sitzungs-Cookie.
pub async fn setup(srv: &TestServer) -> String {
    let reply = setup_reply(srv).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    session_cookie(&reply)
}

/// Anmeldung mit `pass` und dem nächsten gültigen TOTP-Code.
pub async fn login(srv: &TestServer, pass: &str) -> Reply {
    let _serial = kdf_serial().await;
    let code = next_totp(srv);
    send(
        &srv.app,
        post(
            "/api/auth/login",
            &serde_json::json!({ "passphrase": pass, "totpCode": code }).to_string(),
            None,
            None,
        ),
    )
    .await
}

/// Re-Auth der Sitzung (Passphrase + nächster TOTP-Code).
pub async fn reauth(srv: &TestServer, cookie: &str) -> Reply {
    let _serial = kdf_serial().await;
    let code = next_totp(srv);
    send(
        &srv.app,
        post(
            "/api/auth/reauth",
            &serde_json::json!({ "passphrase": PASS, "totpCode": code }).to_string(),
            Some(cookie),
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
