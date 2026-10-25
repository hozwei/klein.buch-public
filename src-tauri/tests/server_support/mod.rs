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
    Arc::new(AppCtx::new(paths, Arc::new(NoDesktop), "test"))
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
        // /tmp der Testumgebung ist klein; die Platzprüfung testet
        // server_uploads_test ausdrücklich.
        uploads: klein_buch_lib::uploads::UploadLimits {
            min_free_bytes: 0,
            ..Default::default()
        },
        test_ports: None,
    }
}

/// Frische Instanz im Zustand `setup` (Prüfungen wie beim echten Start).
pub fn test_server() -> TestServer {
    test_server_with(|_| {})
}

/// Wie [`test_server`], mit angepasster Konfiguration (z. B. Policy).
pub fn test_server_with(adjust: impl FnOnce(&mut ServerConfig)) -> TestServer {
    test_server_with_paths(|_| {}, adjust)
}

/// Wie [`test_server_with`], zusätzlich mit angepassten Pfaden (z. B. eine
/// Wurzel der Backup-Ziele auf einem anderen Dateisystem, Lauf F3c).
pub fn test_server_with_paths(
    adjust_paths: impl FnOnce(&mut Paths),
    adjust: impl FnOnce(&mut ServerConfig),
) -> TestServer {
    // Einmal je Prozess: `set_var` neben laufenden Threads anderer Tests ist
    // riskant (glibc-Umgebung ohne Sperre für nativen Code).
    static MOCK_ENV: std::sync::Once = std::sync::Once::new();
    MOCK_ENV.call_once(|| std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1"));
    let dir = tempfile::tempdir().unwrap();
    let mut paths = prepare_dirs(dir.path());
    adjust_paths(&mut paths);
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

/// Multipart-Upload (`POST /api/upload/{art}`, Feld `file`).
pub fn upload_request(
    art: &str,
    cookie: Option<&str>,
    file_name: &str,
    bytes: &[u8],
) -> Request<Body> {
    let boundary = "kb-test-boundary-7f3a";
    let mut body = Vec::with_capacity(bytes.len() + 256);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; \
             filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let mut b = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/upload/{art}"))
        .header(header::HOST, HOST)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(header::CONTENT_LENGTH, body.len())
        .header("x-kleinbuch-request", "1");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-kb_session={c}"));
    }
    b.body(Body::from(body)).unwrap()
}

/// Lädt eine Datei hoch und liefert die Antwort.
pub async fn upload(
    srv: &TestServer,
    cookie: &str,
    art: &str,
    file_name: &str,
    bytes: &[u8],
) -> Reply {
    send(
        &srv.app,
        upload_request(art, Some(cookie), file_name, bytes),
    )
    .await
}

/// Lädt hoch und liefert die `uploadId` (Status 200 erwartet).
pub async fn upload_id(
    srv: &TestServer,
    cookie: &str,
    art: &str,
    file_name: &str,
    bytes: &[u8],
) -> String {
    let r = upload(srv, cookie, art, file_name, bytes).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["uploadId"].as_str().unwrap().to_string()
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
    setup_reply_with(srv, PASS).await
}

/// Einrichtung mit eigener Passphrase (z. B. Disaster Recovery).
pub async fn setup_reply_with(srv: &TestServer, pass: &str) -> Reply {
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
            &serde_json::json!({ "setupToken": token, "passphrase": pass, "totpCode": code })
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

/// Einrichtung mit eigener Passphrase; liefert den Sitzungs-Cookie.
pub async fn setup_with(srv: &TestServer, pass: &str) -> String {
    let reply = setup_reply_with(srv, pass).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    session_cookie(&reply)
}

/// Neustart auf demselben Datenverzeichnis (wie die Container-Verwaltung nach
/// einem Restore): altes Herunterfahren abwarten, dann Prüfungen und
/// `prepare_filesystem` neu (wendet einen vorgemerkten Restore an). Die
/// TOTP-Uhr läuft weiter (verbrauchte Schritte bleiben verbraucht).
pub async fn restart(srv: TestServer) -> TestServer {
    let offset = srv.state.totp_now() as i64 - klein_buch_lib::server::totp::unix_now() as i64;
    if srv.state.is_stopping() {
        tokio::time::timeout(
            std::time::Duration::from_secs(60),
            srv.state.shutdown.cancelled(),
        )
        .await
        .expect("Herunterfahren nach dem Restore");
    }
    srv.state.finish_shutdown().await;
    let TestServer { state, app, dir } = srv;
    drop(app);
    drop(state);
    let paths = prepare_dirs(dir.path());
    let state = klein_buch_lib::server::prepare(ctx_for(paths), &local_config()).unwrap();
    state.shift_totp_clock_for_tests(offset);
    let app = klein_buch_lib::server::router(state.clone());
    TestServer { state, app, dir }
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

/// Re-Auth (Status 200 erwartet) und neues Sitzungs-Cookie: die Re-Auth
/// rotiert die Sitzungs-ID (W14), nur das neue Cookie trägt die Re-Auth.
pub async fn reauth_cookie(srv: &TestServer, cookie: &str) -> String {
    let r = reauth(srv, cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    session_cookie(&r)
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
    // bis 120 s: Backups laufen nacheinander (Tages-Lauf), KDF unter Last
    for _ in 0..2400 {
        let reply = send(app, get(&format!("/api/jobs/{id}"), Some(cookie))).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
        if reply.json()["state"] != "running" {
            return reply;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("Job {id} nicht fertig");
}

/// Verkäuferprofil, Kunde und ein Rechnungsentwurf (direkt über die DB).
pub async fn draft_invoice(srv: &TestServer) -> String {
    let pool = srv.state.ctx.pool().unwrap();
    klein_buch_lib::db::repo::seller_profile::upsert(
        &pool,
        &klein_buch_lib::db::repo::seller_profile::SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "kontakt@example.org".into(),
            phone: Some("+49 000 000000".into()),
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: true,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
            confirm_waive_paragraph_19: None,
        },
    )
    .await
    .unwrap();
    let contact_id = klein_buch_lib::db::repo::contacts::create(
        &pool,
        &klein_buch_lib::domain::contact::ContactInput {
            contact_type: klein_buch_lib::domain::contact::ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            street: "Hauptstr. 7".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            email: Some("kunde@example.org".into()),
            accepts_einvoice: true,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id;
    let today = klein_buch_lib::clock::today_berlin();
    let input = klein_buch_lib::domain::invoice::InvoiceInput {
        direction: klein_buch_lib::domain::invoice::InvoiceDirection::Issued,
        invoice_date: today,
        delivery_date: Some(today),
        due_date: today.checked_add_days(chrono::Days::new(14)),
        currency_code: "EUR".into(),
        items: vec![klein_buch_lib::domain::invoice::InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 2.0,
            unit_code: "HUR".into(),
            unit_price_cents: 30_000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    klein_buch_lib::commands::invoices::create_invoice_draft_from_input(
        &pool,
        &contact_id,
        &input,
        None,
    )
    .await
    .unwrap()
    .invoice
    .id
}
