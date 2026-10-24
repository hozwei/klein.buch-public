//! Plan W12 — opt-in Versionsprüfung (ADR 0039 E14): ohne Opt-in baut die
//! Prüfung keine Verbindung auf; mit Opt-in fragt sie höchstens täglich einen
//! (hier lokalen) Release-Endpunkt ab, sendet nur den festen User-Agent und
//! legt für eine neuere Version genau einen Inbox-Hinweis an (nur Text).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{Duration, TimeZone, Utc};
use klein_buch_lib::db::repo::app_settings;
use klein_buch_lib::scheduler::update_check::{
    run_if_due, Outcome, SETTING_ENABLED, SETTING_LAST_CHECK, USER_AGENT,
};
use sqlx::SqlitePool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Lokaler Release-Endpunkt: zählt Verbindungen, merkt sich die Anfragen.
struct MockReleases {
    url: String,
    connections: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
}

async fn mock_releases(body: &'static str) -> MockReleases {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let connections = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (count, seen) = (connections.clone(), requests.clone());
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
            seen.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf).into_owned());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    MockReleases {
        url: format!("http://{addr}/repos/example/fork/releases/latest"),
        connections,
        requests,
    }
}

async fn pool() -> (tempfile::TempDir, SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let pool = klein_buch_lib::db::open_pool(&dir.path().join("kb.sqlite"), None)
        .await
        .unwrap();
    klein_buch_lib::db::MIGRATOR.run(&pool).await.unwrap();
    (dir, pool)
}

async fn update_hints(pool: &SqlitePool) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT title, body, action_url FROM notifications
          WHERE dedup_key LIKE 'update_available:%'",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

const NEWER: &str = r#"{"tag_name":"v2099.1.0","name":"Klein.Buch 2099.1 <b>fett</b>",
"html_url":"https://example.org/releases/tag/v2099.1.0","draft":false,"prerelease":false}"#;
const SAME: &str = r#"{"tag_name":"v2026.1.0","name":"alt","html_url":"https://example.org/x"}"#;

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 24, 9, 0, 0).unwrap()
}

#[tokio::test]
async fn without_opt_in_no_connection_is_made() {
    let mock = mock_releases(NEWER).await;
    let (_dir, pool) = pool().await;

    // Standard: aus
    let outcome = run_if_due(&pool, &mock.url, now()).await.unwrap();
    assert_eq!(outcome, Outcome::Disabled);
    // ausdrücklich aus
    app_settings::set_bool(&pool, SETTING_ENABLED, false)
        .await
        .unwrap();
    let outcome = run_if_due(&pool, &mock.url, now()).await.unwrap();
    assert_eq!(outcome, Outcome::Disabled);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let connections = mock.connections.load(Ordering::SeqCst);
    assert_eq!(connections, 0, "keine Verbindung");
    assert!(update_hints(&pool).await.is_empty());
    let last = app_settings::get(&pool, SETTING_LAST_CHECK).await.unwrap();
    assert_eq!(last, None, "ohne Opt-in auch kein Zeitstempel");
}

#[tokio::test]
async fn with_opt_in_newer_version_creates_one_text_hint() {
    let mock = mock_releases(NEWER).await;
    let (_dir, pool) = pool().await;
    app_settings::set_bool(&pool, SETTING_ENABLED, true)
        .await
        .unwrap();

    let outcome = run_if_due(&pool, &mock.url, now()).await.unwrap();
    assert_eq!(
        outcome,
        Outcome::NewVersion {
            tag: "v2099.1.0".into(),
            created: true,
        }
    );
    assert_eq!(mock.connections.load(Ordering::SeqCst), 1);

    // nur GET mit festem User-Agent, keine weiteren Daten
    let request = mock.requests.lock().unwrap()[0].to_ascii_lowercase();
    assert!(
        request.starts_with("get /repos/example/fork/releases/latest http/1.1\r\n"),
        "{request}"
    );
    let ua = format!("user-agent: {}\r\n", USER_AGENT.to_ascii_lowercase());
    assert!(request.contains(&ua), "{request}");
    for forbidden in ["cookie:", "authorization:", "content-length:", "2026."] {
        assert!(!request.contains(forbidden), "{forbidden} in {request}");
    }

    // Hinweis: Name und Link als Text, kein Link-Ziel für die Oberfläche
    let hints = update_hints(&pool).await;
    assert_eq!(hints.len(), 1);
    let (title, body, action_url) = &hints[0];
    assert!(title.contains("2099.1.0"), "{title}");
    assert!(
        body.contains("https://example.org/releases/tag/v2099.1.0"),
        "{body}"
    );
    assert!(
        body.contains("Klein.Buch 2099.1 <b>fett</b>"),
        "als Text: {body}"
    );
    assert!(action_url.is_none(), "kein Link-Ziel: {action_url:?}");

    // am selben Tag keine zweite Abfrage
    let later = now() + Duration::hours(23);
    let outcome = run_if_due(&pool, &mock.url, later).await.unwrap();
    assert_eq!(outcome, Outcome::NotDue);
    assert_eq!(mock.connections.load(Ordering::SeqCst), 1);

    // am nächsten Tag wieder, aber derselbe Hinweis nur einmal (je Version)
    let next_day = now() + Duration::hours(25);
    let outcome = run_if_due(&pool, &mock.url, next_day).await.unwrap();
    assert_eq!(
        outcome,
        Outcome::NewVersion {
            tag: "v2099.1.0".into(),
            created: false,
        }
    );
    assert_eq!(mock.connections.load(Ordering::SeqCst), 2);
    assert_eq!(update_hints(&pool).await.len(), 1);
}

#[tokio::test]
async fn same_or_older_version_creates_no_hint() {
    let mock = mock_releases(SAME).await;
    let (_dir, pool) = pool().await;
    app_settings::set_bool(&pool, SETTING_ENABLED, true)
        .await
        .unwrap();
    let outcome = run_if_due(&pool, &mock.url, now()).await.unwrap();
    assert_eq!(
        outcome,
        Outcome::UpToDate {
            latest: "v2026.1.0".into(),
        }
    );
    assert!(update_hints(&pool).await.is_empty());
}

#[tokio::test]
async fn unreachable_endpoint_counts_as_daily_attempt() {
    // Port eines geschlossenen Listeners: Verbindung schlägt fehl.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let url = format!("http://{addr}/releases/latest");
    let (_dir, pool) = pool().await;
    app_settings::set_bool(&pool, SETTING_ENABLED, true)
        .await
        .unwrap();
    assert!(run_if_due(&pool, &url, now()).await.is_err());
    let outcome = run_if_due(&pool, &url, now()).await.unwrap();
    assert_eq!(outcome, Outcome::NotDue, "höchstens einmal täglich");
}
