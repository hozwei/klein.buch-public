//! Plan W5.8 — Header-Read-Timeout und Lastgrenzen.
//!
//! - Ein Client, der die Header Byte für Byte schickt, wird nach 10 s getrennt.
//! - 50 parallele `pdf_template_preview`: höchstens 2 gleichzeitig, der Rest
//!   503 (Zulassung) bzw. 429 (Kontingent der Sitzung).

mod server_support;

use std::time::{Duration, Instant};

use axum::http::StatusCode;
use klein_buch_lib::commands::registry::Cost;
use klein_buch_lib::server::http::{serve, HEADER_READ_TIMEOUT};
use server_support::*;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn byte_by_byte_headers_are_cut_after_timeout() {
    let srv = test_server();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = CancellationToken::new();
    tokio::spawn(serve(listener, srv.app.clone(), token.clone()));

    // Normale Anfrage geht durch.
    let (status, _) = klein_buch_lib::server::health::probe(addr).await.unwrap();
    assert_eq!(status, 200);

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut rd, mut wr) = stream.into_split();
    let started = Instant::now();
    let reader = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = rd.read_to_end(&mut buf).await;
        (started.elapsed(), buf)
    });
    let request =
        b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Langsam: abcdefghijklmnop\r\n\r\n";
    for byte in request {
        if reader.is_finished() || wr.write_all(&[*byte]).await.is_err() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let (closed_after, response) = reader.await.unwrap();
    assert!(
        closed_after >= HEADER_READ_TIMEOUT - Duration::from_millis(500)
            && closed_after < HEADER_READ_TIMEOUT + Duration::from_secs(3),
        "getrennt nach {closed_after:?}"
    );
    assert!(
        !String::from_utf8_lossy(&response).contains(" 200 "),
        "keine Antwort auf die unvollständige Anfrage"
    );
    token.cancel();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_previews_are_limited() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let mut tasks = Vec::new();
    for i in 0..50 {
        let app = srv.app.clone();
        let cookie = cookie.clone();
        // verschiedene Adressen: die Grenze je Client (W14) soll hier nicht greifen
        let peer = format!("198.51.100.{}:4000", i + 1);
        tasks.push(tokio::spawn(async move {
            send(
                &app,
                from_peer(
                    post(
                        "/api/cmd/pdf_template_preview",
                        r#"{"name":"default"}"#,
                        Some(&cookie),
                        None,
                    ),
                    &peer,
                ),
            )
            .await
        }));
    }
    let mut by_status = std::collections::BTreeMap::<u16, usize>::new();
    for t in tasks {
        let r = t.await.unwrap();
        *by_status.entry(r.status.as_u16()).or_default() += 1;
    }
    let peak = srv.state.admission.peak(Cost::Render);
    assert!(peak <= 2, "höchstens 2 gleichzeitig, waren {peak}");
    assert!(peak >= 1);
    // Kontingent: 30 je Minute und Sitzung
    assert_eq!(
        by_status.get(&429).copied().unwrap_or(0),
        20,
        "{by_status:?}"
    );
    for status in by_status.keys() {
        // 200: Download-Ticket (W7); 503: keine Zulassung binnen 5 s
        assert!(
            [200u16, 429, 503].contains(status),
            "unerwarteter Status {status}: {by_status:?}"
        );
    }
    assert_eq!(by_status.values().sum::<usize>(), 50);
    let _ = StatusCode::OK;
}

/// W14: Ein Körper, der nie ankommt, belegt seinen Platz in der globalen Grenze
/// höchstens `BODY_READ_TIMEOUT` lang (dann 408), auch ohne Anmeldung.
#[tokio::test(start_paused = true)]
async fn stalled_request_body_times_out() {
    use axum::body::Body;
    use axum::http::{header, Method, Request};
    use klein_buch_lib::server::dispatch::BODY_READ_TIMEOUT;

    let srv = test_server();
    let stalled =
        futures_util::stream::pending::<std::result::Result<axum::body::Bytes, std::io::Error>>();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/login")
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_LENGTH, "16000")
        .header("x-kleinbuch-request", "1")
        .body(Body::from_stream(stalled))
        .unwrap();
    let started = tokio::time::Instant::now();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::REQUEST_TIMEOUT, "{}", r.text());
    assert_eq!(r.json()["code"], "body_timeout");
    assert!(started.elapsed() >= BODY_READ_TIMEOUT);
}

/// W14: Höchstens `MAX_CONCURRENT_POSTS_PER_CLIENT` gleichzeitige POSTs je
/// Adresse — langsam gesendete Körper eines Clients füllen die globale Grenze
/// nicht; andere Clients bleiben bedienbar.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_posts_are_limited_per_client() {
    use axum::body::Body;
    use axum::http::{header, Method, Request};
    use klein_buch_lib::server::limits::MAX_CONCURRENT_POSTS_PER_CLIENT;

    let srv = test_server();
    let stalled_from = |peer: &str| {
        let body = futures_util::stream::pending::<
            std::result::Result<axum::body::Bytes, std::io::Error>,
        >();
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::HOST, HOST)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, "16000")
            .header("x-kleinbuch-request", "1")
            .body(Body::from_stream(body))
            .unwrap();
        from_peer(req, peer)
    };
    let mut held = Vec::new();
    for _ in 0..MAX_CONCURRENT_POSTS_PER_CLIENT {
        let app = srv.app.clone();
        let req = stalled_from("203.0.113.9:4000");
        held.push(tokio::spawn(async move { send(&app, req).await }));
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let r = send(&srv.app, stalled_from("203.0.113.9:4001")).await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{}", r.text());
    assert_eq!(r.json()["code"], "busy");
    // ein anderer Client wird weiter bedient (hier: Anmeldung ohne Daten → 4xx, kein 503)
    let other = from_peer(
        post("/api/auth/login", "{}", None, None),
        "198.51.100.99:4000",
    );
    let r = send(&srv.app, other).await;
    assert_ne!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{}", r.text());
    for t in held {
        let r = t.await.unwrap();
        assert_eq!(r.status, StatusCode::REQUEST_TIMEOUT);
    }
}
