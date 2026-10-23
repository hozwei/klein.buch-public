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
    for _ in 0..50 {
        let app = srv.app.clone();
        let cookie = cookie.clone();
        tasks.push(tokio::spawn(async move {
            send(
                &app,
                post(
                    "/api/cmd/pdf_template_preview",
                    r#"{"name":"default"}"#,
                    Some(&cookie),
                    None,
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
