//! Lauf F4b — HTTP-Server, Eingaben, Exporte, Sidecar: Inhaltstyp-Pflicht
//! (ASVS V13.1.5/V13.2.5), `/healthz` hinter einem Proxy auf demselben Host
//! (W14-006), Panik-Abfang (ASVS 7.4.3), `/legal/` gestreamt, Verbindungs-
//! obergrenze und Schreib-Leerlauf-Timeout (W12-LEGAL-TIMEOUTS, echte Sockets
//! nur auf 127.0.0.1), Hinweise vom Serverstart in der Inbox (R3-014).

mod server_support;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::get as get_route;
use axum::Router;
use klein_buch_lib::server::http::{serve_with, ConnectionLimits};
use klein_buch_lib::server::state::ServerState;
use server_support::*;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio_util::sync::CancellationToken;

/// `POST` mit Pflicht-Header und Sitzung, Inhaltstyp nach Wahl.
fn post_typed(
    uri: &str,
    body: &str,
    cookie: Option<&str>,
    content_type: Option<&str>,
) -> Request<Body> {
    let mut b = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::HOST, HOST)
        .header("x-kleinbuch-request", "1");
    if let Some(ct) = content_type {
        b = b.header(header::CONTENT_TYPE, ct);
    }
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-kb_session={c}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_body_requires_json_content_type() {
    let srv = test_server();
    let app = &srv.app;
    let cookie = setup(&srv).await;
    let url = "/api/cmd/contacts_list";
    for ct in [
        Some("text/plain"),
        Some("application/x-www-form-urlencoded"),
        None,
    ] {
        let r = send(app, post_typed(url, "{}", Some(&cookie), ct)).await;
        assert_eq!(
            r.status,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{ct:?}: {}",
            r.text()
        );
        assert_eq!(r.json()["code"], "unsupported_media_type");
    }
    for ct in [
        "application/json",
        "application/json; charset=utf-8",
        "Application/JSON",
    ] {
        let r = send(app, post_typed(url, "{}", Some(&cookie), Some(ct))).await;
        assert_eq!(r.status, StatusCode::OK, "{ct}: {}", r.text());
    }
    // leerer Körper braucht keinen Inhaltstyp
    let r = send(app, post_typed(url, "", Some(&cookie), None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // Auth-Routen ebenso
    let body = format!(r#"{{"passphrase":"{PASS}","totpCode":"123456"}}"#);
    let r = send(
        app,
        post_typed("/api/auth/reauth", &body, Some(&cookie), Some("text/plain")),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{}", r.text());
    // Abmelden ohne Körper und ohne Inhaltstyp bleibt erlaubt
    let r = send(app, post_typed("/api/auth/logout", "", Some(&cookie), None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

/// W14-006: Ein Reverse-Proxy auf demselben Host (Gegenstelle 127.0.0.1 mit
/// Weiterleitungs-Headern) bekommt nur den Status, keinen Zustand.
#[tokio::test]
async fn healthz_details_only_for_direct_loopback() {
    let srv = test_server();
    let r = send(&srv.app, from_peer(get("/healthz", None), "127.0.0.1:5000")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["state"], "setup", "{}", r.text());
    for (name, value) in [
        ("x-forwarded-for", "203.0.113.5"),
        ("x-forwarded-proto", "https"),
        ("forwarded", "for=203.0.113.5"),
    ] {
        let mut req = from_peer(get("/healthz", None), "127.0.0.1:5000");
        req.headers_mut().insert(name, value.parse().unwrap());
        let r = send(&srv.app, req).await;
        assert_eq!(r.status, StatusCode::OK, "{name}");
        assert!(r.body.is_empty(), "{name}: {}", r.text());
    }
}

/// ASVS 7.4.3: Eine Panik im Handler ergibt 500 im API-Format mit `ref` und
/// den Sicherheits-Headern, ohne Panik-Text; der Dienst bleibt nutzbar.
#[tokio::test]
async fn panic_in_handler_returns_500_with_reference() {
    let srv = test_server();
    let routes = Router::<Arc<ServerState>>::new().route(
        "/api/panik",
        get_route(|| async {
            if std::hint::black_box(true) {
                panic!("geheimer-inhalt-4711");
            }
            "nie"
        }),
    );
    let app = klein_buch_lib::server::layered(routes, srv.state.clone());
    for _ in 0..2 {
        let r = send(&app, get("/api/panik", None)).await;
        assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR);
        let body = r.json();
        assert_eq!(body["code"], "internal", "{}", r.text());
        assert!(
            body["ref"].as_str().is_some_and(|s| !s.is_empty()),
            "{}",
            r.text()
        );
        assert!(!r.text().contains("geheimer-inhalt"), "{}", r.text());
        assert_eq!(
            r.headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
            "nosniff"
        );
        assert_eq!(
            r.headers.get(header::CONTENT_SECURITY_POLICY).unwrap(),
            klein_buch_lib::server::headers::CSP_NONE
        );
    }
}

/// W12-LEGAL-TIMEOUTS: `/legal/` liefert gestreamt mit Länge und Zeichensatz.
#[tokio::test]
async fn legal_file_is_streamed_with_length() {
    let srv = test_server();
    let legal = srv.state.ctx.paths.resource_dir.join("legal");
    std::fs::create_dir_all(&legal).unwrap();
    let text = "AGPL-3.0 ".repeat(30_000);
    std::fs::write(legal.join("LICENSE.txt"), &text).unwrap();
    let r = send(&srv.app, get("/legal/LICENSE.txt", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body.as_ref(), text.as_bytes());
    assert_eq!(
        r.headers.get(header::CONTENT_LENGTH).unwrap(),
        &text.len().to_string()
    );
    assert_eq!(
        r.headers.get(header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
}

/// Startet die Annahme-Schleife auf 127.0.0.1 mit einem kleinen Router.
async fn local_server(limits: ConnectionLimits) -> (std::net::SocketAddr, CancellationToken) {
    let app = Router::new()
        .route("/klein", get_route(|| async { "ok" }))
        .route("/gross", get_route(|| async { vec![b'x'; 32 << 20] }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = CancellationToken::new();
    let stop = token.clone();
    tokio::spawn(async move {
        let _ = serve_with(listener, app, stop, limits).await;
    });
    (addr, token)
}

/// W12-LEGAL-TIMEOUTS: Eine Gegenstelle, die nicht mehr liest, verliert die
/// Verbindung nach dem Schreib-Leerlauf-Timeout; ein normaler Leser bekommt
/// alles.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_reader_is_disconnected_after_write_idle_timeout() {
    let (addr, token) = local_server(ConnectionLimits {
        max_connections: 8,
        write_idle_timeout: Duration::from_secs(1),
        ..Default::default()
    })
    .await;
    let request = b"GET /gross HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    let full = 32usize << 20;

    // Normaler Leser: vollständige Antwort.
    let mut reader = tokio::net::TcpStream::connect(addr).await.unwrap();
    reader.write_all(request).await.unwrap();
    let mut all = Vec::new();
    tokio::time::timeout(Duration::from_secs(60), reader.read_to_end(&mut all))
        .await
        .expect("Leser bekommt die Antwort")
        .unwrap();
    assert!(all.len() > full, "vollständig: {}", all.len());

    // Langsamer Leser: kleiner Empfangspuffer, liest 3 s lang nichts.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4096).unwrap();
    let mut slow = socket.connect(addr).await.unwrap();
    slow.write_all(request).await.unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let mut got = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let read_all = async {
        loop {
            match slow.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => got.extend_from_slice(&buf[..n]),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(30), read_all)
        .await
        .expect("Server beendet die Verbindung");
    assert!(
        got.len() < full,
        "Verbindung vor dem Ende getrennt: {} von {full} Bytes",
        got.len()
    );
    token.cancel();
}

/// W12-LEGAL-TIMEOUTS: Bei voller Verbindungsgrenze nimmt der Server erst
/// wieder an, wenn eine Verbindung endet.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_limit_defers_accept() {
    let (addr, token) = local_server(ConnectionLimits {
        max_connections: 1,
        write_idle_timeout: Duration::from_secs(5),
        ..Default::default()
    })
    .await;
    // A belegt den einzigen Platz (ruhend, ohne Anfrage).
    let idle = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    // B wartet im Backlog: keine Antwort, solange A offen ist.
    let mut waiting = tokio::net::TcpStream::connect(addr).await.unwrap();
    waiting
        .write_all(b"GET /klein HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut buf = vec![0u8; 1024];
    let early = tokio::time::timeout(Duration::from_millis(800), waiting.read(&mut buf)).await;
    assert!(early.is_err(), "keine Antwort bei voller Grenze");
    // A endet → B wird angenommen und beantwortet.
    drop(idle);
    let mut answer = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), waiting.read_to_end(&mut answer))
        .await
        .expect("Antwort nach freiem Platz")
        .unwrap();
    let text = String::from_utf8_lossy(&answer);
    assert!(text.starts_with("HTTP/1.1 200"), "{text}");
    token.cancel();
}

/// Lauf F4d (Befund F4 S3): Eine Gegenstelle bekommt höchstens
/// `max_per_peer` Verbindungen — weitere werden angenommen und sofort
/// geschlossen, statt die globale Grenze zu belegen. Endet eine ihrer
/// Verbindungen, ist der Platz wieder frei. Vertraute Proxys
/// (`unlimited_peers`) zählen nur gegen die globale Grenze.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_peer_limit_closes_excess_connections() {
    const REQ: &[u8] = b"GET /klein HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    async fn answered(addr: std::net::SocketAddr) -> bool {
        let Ok(mut s) = tokio::net::TcpStream::connect(addr).await else {
            return false;
        };
        if s.write_all(REQ).await.is_err() {
            return false;
        }
        let mut out = Vec::new();
        match tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await {
            Ok(Ok(_)) => String::from_utf8_lossy(&out).starts_with("HTTP/1.1 200"),
            _ => false,
        }
    }

    let (addr, token) = local_server(ConnectionLimits {
        max_connections: 8,
        write_idle_timeout: Duration::from_secs(5),
        max_per_peer: 2,
        unlimited_peers: Vec::new(),
    })
    .await;
    // Zwei ruhende Verbindungen belegen den Anteil von 127.0.0.1.
    let idle_a = tokio::net::TcpStream::connect(addr).await.unwrap();
    let _idle_b = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!answered(addr).await, "dritte Verbindung geschlossen");
    // A endet → der Platz wird frei (der Server bemerkt das Ende kurz danach).
    drop(idle_a);
    let mut ok = false;
    for _ in 0..50 {
        if answered(addr).await {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ok, "nach dem Ende einer Verbindung wieder angenommen");
    token.cancel();

    // Vertrauter Proxy: nur die globale Grenze.
    let (addr, token) = local_server(ConnectionLimits {
        max_connections: 8,
        write_idle_timeout: Duration::from_secs(5),
        max_per_peer: 2,
        unlimited_peers: vec!["127.0.0.1/32".parse().unwrap()],
    })
    .await;
    let _idle_a = tokio::net::TcpStream::connect(addr).await.unwrap();
    let _idle_b = tokio::net::TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(answered(addr).await, "Proxy ohne Anteil je Gegenstelle");
    token.cancel();
}

/// R3-014: Ein Problem des Sidecars beim Start landet beim Entsperren als
/// Hinweis in der Inbox — genau einmal (Dedup je Version und Tag).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sidecar_problem_reaches_the_inbox_on_unlock() {
    use klein_buch_lib::einvoice::sidecar_health;
    let srv = test_server();
    let empty = tempfile::tempdir().unwrap();
    let problems =
        sidecar_health::check_with(empty.path(), std::path::Path::new("/nonexistent/java")).await;
    assert!(!problems.is_empty());
    sidecar_health::record(&srv.state.ctx, &problems);
    assert_eq!(srv.state.ctx.deferred_notices.pending().len(), 1);

    setup(&srv).await;
    assert!(srv.state.ctx.deferred_notices.pending().is_empty());
    let pool = srv.state.ctx.pool().unwrap();
    let count = || async {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT COUNT(*), COALESCE(MAX(body), '') FROM notifications \
             WHERE dedup_key LIKE 'sidecar_health:%' AND severity = 'urgent'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let (n, body) = count().await;
    assert_eq!(n, 1);
    assert!(body.contains("mustang.jar fehlt"), "{body}");
    assert!(
        !body.contains(&*empty.path().to_string_lossy()),
        "kein Serverpfad: {body}"
    );

    // Gleicher Tag, gleiche Version: kein zweiter Hinweis.
    sidecar_health::record(&srv.state.ctx, &problems);
    srv.state.ctx.deferred_notices.deliver(&pool).await.unwrap();
    assert_eq!(count().await.0, 1);
}
