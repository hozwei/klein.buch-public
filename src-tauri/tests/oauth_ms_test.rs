//! Integration-Tests für den OAuth-Microsoft-Pfad (Block 16).
//!
//! Verifiziert die Netzwerk-Schicht (Token-Exchange, Refresh, Graph /sendMail,
//! Graph /me) gegen einen **In-Process-Mock-HTTP-Server** — deterministisch und
//! CI-tauglich, ohne echte Microsoft-Credentials. Der echte Round-Trip gegen ein
//! M365-Postfach ist ein manueller Smoke (siehe Block-16-Report).
//!
//! Zusätzlich: Repo-Test, dass ein OAuth-Account seine Spalten + Session-Metadaten
//! korrekt persistiert (Migration 0014).

use klein_buch_lib::db::repo::mail_accounts::{self, MailAccountInput};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::mail::oauth_ms;
use klein_buch_lib::mail::smtp::{MailAttachment, OutgoingMail};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// =============================================================================
// Mini-Mock-HTTP-Server
// =============================================================================

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Liest eine vollständige HTTP-Request (Header + ggf. Content-Length-Body).
async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        data.extend_from_slice(&buf[..n]);
        if let Some(pos) = find_subslice(&data, b"\r\n\r\n") {
            let header_str = String::from_utf8_lossy(&data[..pos]).to_lowercase();
            let content_length = header_str.lines().find_map(|l| {
                l.strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            });
            match content_length {
                Some(cl) => {
                    let body_len = data.len() - (pos + 4);
                    if body_len >= cl {
                        break;
                    }
                }
                None => break,
            }
        }
    }
    data
}

async fn respond(stream: &mut TcpStream, status_line: &str, content_type: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.flush().await;
}

async fn bind() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, format!("http://127.0.0.1:{port}"))
}

// =============================================================================
// DB-Helper
// =============================================================================

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

// =============================================================================
// Tests — Token-Endpoint (Exchange + Refresh)
// =============================================================================

#[tokio::test]
async fn exchange_and_refresh_against_mock_token_server() {
    let (listener, base) = bind().await;
    let token_url = format!("{base}/token");

    // Server bedient zwei Anfragen: erst authorization_code, dann refresh_token.
    let server = tokio::spawn(async move {
        // 1) Exchange
        let (mut s, _) = listener.accept().await.unwrap();
        let req = read_request(&mut s).await;
        let body = String::from_utf8_lossy(&req);
        assert!(
            body.contains("grant_type=authorization_code"),
            "body={body}"
        );
        assert!(body.contains("code=code-xyz"), "body={body}");
        assert!(body.contains("code_verifier=the-verifier"), "body={body}");
        assert!(body.contains("client_id=client-123"), "body={body}");
        respond(
            &mut s,
            "200 OK",
            "application/json",
            r#"{"access_token":"acc1","refresh_token":"ref1","expires_in":3600,"token_type":"Bearer","scope":"Mail.Send offline_access User.Read"}"#,
        )
        .await;

        // 2) Refresh
        let (mut s2, _) = listener.accept().await.unwrap();
        let req2 = read_request(&mut s2).await;
        let body2 = String::from_utf8_lossy(&req2);
        assert!(body2.contains("grant_type=refresh_token"), "body2={body2}");
        assert!(body2.contains("refresh_token=ref1"), "body2={body2}");
        // Refresh-Antwort OHNE neuen Refresh-Token (häufiger Fall).
        respond(
            &mut s2,
            "200 OK",
            "application/json",
            r#"{"access_token":"acc2","expires_in":3600,"token_type":"Bearer"}"#,
        )
        .await;
    });

    // Exchange
    let resp1 = oauth_ms::exchange_code(
        &token_url,
        "client-123",
        "http://localhost:1234",
        "code-xyz",
        "the-verifier",
    )
    .await
    .expect("exchange ok");
    assert_eq!(resp1.access_token, "acc1");
    let bundle = oauth_ms::bundle_from_response(&resp1, chrono::Utc::now(), None).unwrap();
    assert_eq!(bundle.refresh_token, "ref1");
    assert!(!oauth_ms::is_expired(
        &bundle.expires_at,
        chrono::Utc::now(),
        0
    ));

    // Refresh — neuer Access-Token, alter Refresh-Token bleibt erhalten.
    let resp2 = oauth_ms::refresh_tokens(&token_url, "client-123", "ref1")
        .await
        .expect("refresh ok");
    assert_eq!(resp2.access_token, "acc2");
    let bundle2 = oauth_ms::bundle_from_response(
        &resp2,
        chrono::Utc::now(),
        Some(bundle.refresh_token.as_str()),
    )
    .unwrap();
    assert_eq!(bundle2.refresh_token, "ref1");
    assert_eq!(bundle2.access_token, "acc2");

    server.await.unwrap();
}

#[tokio::test]
async fn token_endpoint_error_is_mapped() {
    let (listener, base) = bind().await;
    let token_url = format!("{base}/token");
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let _ = read_request(&mut s).await;
        respond(
            &mut s,
            "400 Bad Request",
            "application/json",
            r#"{"error":"invalid_grant","error_description":"AADSTS70008: refresh token expired"}"#,
        )
        .await;
    });

    let err = oauth_ms::refresh_tokens(&token_url, "client-123", "stale")
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid_grant"), "msg={msg}");
    server.await.unwrap();
}

// =============================================================================
// Tests — Graph
// =============================================================================

fn sample_mail() -> OutgoingMail {
    OutgoingMail {
        from_name: "Wildbach Computerhilfe".into(),
        from_email: "rechnung@wildbach-computerhilfe.de".into(),
        to: "kunde@example.com".into(),
        subject: "Rechnung RE-2026-0001".into(),
        body_text: "Anbei die Rechnung.".into(),
        attachments: vec![MailAttachment {
            filename: "RE-2026-0001.pdf".into(),
            mime_type: "application/pdf".into(),
            bytes: b"%PDF-1.7 fake".to_vec(),
        }],
    }
}

#[tokio::test]
async fn graph_send_posts_message_with_bearer_and_accepts_202() {
    let (listener, base) = bind().await;
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let req = read_request(&mut s).await;
        let raw = String::from_utf8_lossy(&req);
        assert!(raw.contains("POST /me/sendMail"), "raw={raw}");
        assert!(
            raw.to_lowercase()
                .contains("authorization: bearer acc-token"),
            "kein Bearer-Header"
        );
        assert!(
            raw.contains("Rechnung RE-2026-0001"),
            "Subject fehlt im Body"
        );
        assert!(raw.contains("kunde@example.com"), "Empfänger fehlt");
        // 202 Accepted = Erfolg bei sendMail.
        respond(&mut s, "202 Accepted", "text/plain", "").await;
    });

    let info = oauth_ms::graph_send(&base, "acc-token", &sample_mail())
        .await
        .expect("graph_send ok");
    assert_eq!(info.status, 202);
    server.await.unwrap();
}

#[tokio::test]
async fn graph_send_maps_error_response() {
    let (listener, base) = bind().await;
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let _ = read_request(&mut s).await;
        respond(
            &mut s,
            "403 Forbidden",
            "application/json",
            r#"{"error":{"code":"ErrorAccessDenied","message":"Access is denied."}}"#,
        )
        .await;
    });

    let err = oauth_ms::graph_send(&base, "acc-token", &sample_mail())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ErrorAccessDenied"), "msg={msg}");
    server.await.unwrap();
}

#[tokio::test]
async fn graph_me_email_reads_mailbox() {
    let (listener, base) = bind().await;
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let req = read_request(&mut s).await;
        let raw = String::from_utf8_lossy(&req);
        assert!(raw.contains("GET /me"), "raw={raw}");
        respond(
            &mut s,
            "200 OK",
            "application/json",
            r#"{"mail":"manuel@wildbach.onmicrosoft.com","userPrincipalName":"manuel@wildbach.onmicrosoft.com"}"#,
        )
        .await;
    });

    let email = oauth_ms::graph_me_email(&base, "acc-token").await;
    assert_eq!(email.as_deref(), Some("manuel@wildbach.onmicrosoft.com"));
    server.await.unwrap();
}

// =============================================================================
// Tests — Repo (Migration 0014)
// =============================================================================

#[tokio::test]
async fn oauth_account_persists_columns_and_session() {
    let (pool, _dir) = setup_pool().await;

    let input = MailAccountInput {
        label: "Wildbach M365".into(),
        auth_type: "oauth_microsoft".into(),
        smtp_host: None,
        smtp_port: None,
        smtp_user: None,
        smtp_use_tls: true,
        from_email: "rechnung@wildbach-computerhilfe.de".into(),
        from_name: "Wildbach Computerhilfe".into(),
        is_default: true,
        oauth_tenant_id: Some("contoso.onmicrosoft.com".into()),
        oauth_client_id: Some("11111111-2222-3333-4444-555555555555".into()),
    };
    let row = mail_accounts::create(&pool, &input).await.unwrap();
    assert_eq!(row.auth_type, "oauth_microsoft");
    assert_eq!(
        row.oauth_tenant_id.as_deref(),
        Some("contoso.onmicrosoft.com")
    );
    assert_eq!(
        row.oauth_client_id.as_deref(),
        Some("11111111-2222-3333-4444-555555555555")
    );
    // Keychain-Service-ID wird jetzt auch für OAuth gesetzt.
    assert!(row.keychain_service_id.is_some());
    // Vor Connect noch keine Session.
    assert!(row.oauth_account_email.is_none());
    assert!(row.oauth_token_expires_at.is_none());

    // Connect schreibt Session-Metadaten.
    mail_accounts::set_oauth_session(
        &pool,
        &row.id,
        Some("manuel@contoso.onmicrosoft.com"),
        Some("Mail.Send offline_access User.Read"),
        Some("2026-05-22T10:00:00+00:00"),
    )
    .await
    .unwrap();
    let got = mail_accounts::get(&pool, &row.id).await.unwrap().unwrap();
    assert_eq!(
        got.oauth_account_email.as_deref(),
        Some("manuel@contoso.onmicrosoft.com")
    );
    assert_eq!(
        got.oauth_token_expires_at.as_deref(),
        Some("2026-05-22T10:00:00+00:00")
    );

    // Reiner Ablauf-Update.
    mail_accounts::set_oauth_token_expiry(&pool, &row.id, "2026-06-01T00:00:00+00:00")
        .await
        .unwrap();
    let got2 = mail_accounts::get(&pool, &row.id).await.unwrap().unwrap();
    assert_eq!(
        got2.oauth_token_expires_at.as_deref(),
        Some("2026-06-01T00:00:00+00:00")
    );
    // Email bleibt erhalten.
    assert_eq!(
        got2.oauth_account_email.as_deref(),
        Some("manuel@contoso.onmicrosoft.com")
    );

    // Disconnect räumt die Session.
    mail_accounts::clear_oauth_session(&pool, &row.id)
        .await
        .unwrap();
    let cleared = mail_accounts::get(&pool, &row.id).await.unwrap().unwrap();
    assert!(cleared.oauth_account_email.is_none());
    assert!(cleared.oauth_scopes.is_none());
    assert!(cleared.oauth_token_expires_at.is_none());
    // tenant/client_id (Konfiguration) bleiben bestehen.
    assert_eq!(
        cleared.oauth_client_id.as_deref(),
        Some("11111111-2222-3333-4444-555555555555")
    );
}
