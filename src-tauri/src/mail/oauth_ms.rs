//! Microsoft Graph OAuth (Block 16) — Authorization-Code-Flow mit PKCE (S256)
//! gegen die Microsoft Identity Platform v2.0 + Versand über Microsoft Graph
//! `/me/sendMail`.
//!
//! ## Schichten
//!
//! - **Functional Core** (pure, ohne I/O): [`pkce_challenge`], [`gen_pkce`],
//!   [`build_authorize_url`], [`parse_callback_query`], [`bundle_from_response`],
//!   [`is_expired`], [`build_graph_message`].
//! - **Imperative Shell** (async, Netzwerk-I/O via reqwest + tokio):
//!   [`exchange_code`], [`refresh_tokens`], [`graph_send`], [`graph_me_email`],
//!   [`bind_loopback`] / [`capture_redirect`].
//!
//! ## Modell
//!
//! Public-Client (Desktop): **kein** `client_secret` — die Sicherheit liefert
//! PKCE. Die nutzer-eigene Azure-App liefert `client_id` + `tenant_id` (in
//! `mail_accounts`, nicht geheim). Der Redirect läuft über einen lokalen
//! Loopback-Server (`http://localhost:{port}`); in der App-Registrierung wird
//! `http://localhost` als Redirect-URI der „Mobile/Desktop"-Plattform eingetragen
//! (der Port wird beim Matching ignoriert).
//!
//! ## Hard-Rule (Backup-Hardline)
//!
//! Access- und Refresh-Token werden hier nur für die Dauer eines Vorgangs
//! gehalten und ausschließlich über [`crate::mail::keyring`] persistiert —
//! niemals in DB, Log oder `audit_log`.

use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::mail::smtp::OutgoingMail;

/// Delegated-Scopes: `offline_access` (Refresh-Token), `Mail.Send` (Versand),
/// `User.Read` (Postfach-Adresse via `/me` anzeigen).
pub const GRAPH_SCOPES: &str = "offline_access Mail.Send User.Read";

const MS_LOGIN_BASE: &str = "https://login.microsoftonline.com";
const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";

// =============================================================================
// Endpoints
// =============================================================================

/// Die für einen Tenant aufgelösten OAuth-/Graph-Endpunkte. In Tests werden
/// `token`/`graph` auf einen lokalen Mock-Server gezeigt.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub graph: String,
}

impl Endpoints {
    /// Reale Microsoft-Endpunkte für einen Tenant. Leerer Tenant → `common`.
    pub fn for_tenant(tenant: &str) -> Self {
        let t = tenant.trim();
        let t = if t.is_empty() { "common" } else { t };
        Self {
            authorize: format!("{MS_LOGIN_BASE}/{t}/oauth2/v2.0/authorize"),
            token: format!("{MS_LOGIN_BASE}/{t}/oauth2/v2.0/token"),
            graph: GRAPH_BASE.to_string(),
        }
    }
}

// =============================================================================
// Functional Core — PKCE
// =============================================================================

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Kryptografisch zufälliger, URL-sicherer String aus `n_bytes` Entropie.
/// Dient als PKCE-Verifier und als CSRF-`state`.
pub fn random_urlsafe(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    b64url(&buf)
}

/// PKCE-Code-Challenge (Methode S256) aus dem Verifier — `BASE64URL(SHA256(v))`
/// ohne Padding (RFC 7636 §4.2). Pure.
pub fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    b64url(&digest)
}

/// Erzeugt ein frisches `(verifier, challenge)`-Paar. 32 Byte Entropie → 43
/// Zeichen Verifier (RFC erlaubt 43–128).
pub fn gen_pkce() -> (String, String) {
    let verifier = random_urlsafe(32);
    let challenge = pkce_challenge(&verifier);
    (verifier, challenge)
}

// =============================================================================
// Functional Core — Authorize-URL + Callback
// =============================================================================

/// Baut die Authorize-URL (pure). `scopes` ist die leerzeichengetrennte Liste.
pub fn build_authorize_url(
    authorize_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
    scopes: &str,
) -> Result<String> {
    let mut url = url::Url::parse(authorize_endpoint)
        .map_err(|e| Error::Mail(format!("Authorize-Endpoint ungültig: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", scopes)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    Ok(url.to_string())
}

/// Erfolgreich extrahierte Redirect-Parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    pub code: String,
    pub state: String,
}

/// Parst die Query eines Redirect-Aufrufs (Teil **nach** dem `?`, ohne führendes
/// `?`). Liefert `code`+`state` oder mappt einen `error`-Redirect auf einen
/// sprechenden Fehler. Pure.
pub fn parse_callback_query(query: &str) -> Result<Callback> {
    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut error_desc = None;
    for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            "error_description" => error_desc = Some(v.into_owned()),
            _ => {}
        }
    }
    if let Some(err) = error {
        let desc = error_desc.unwrap_or_default();
        return Err(Error::Mail(
            format!("Microsoft-Anmeldung abgelehnt: {err} {desc}")
                .trim()
                .to_string(),
        ));
    }
    match (code, state) {
        (Some(code), Some(state)) => Ok(Callback { code, state }),
        _ => Err(Error::Mail(
            "Redirect ohne code/state — Anmeldung unvollständig.".into(),
        )),
    }
}

// =============================================================================
// Functional Core — Token-Modell
// =============================================================================

/// Rohe Antwort des Token-Endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
}

/// Im Keychain persistiertes Token-Bundle (JSON).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenBundle {
    pub access_token: String,
    pub refresh_token: String,
    /// Ablaufzeitpunkt des Access-Tokens, UTC (RFC 3339).
    pub expires_at: String,
}

/// Baut aus einer Token-Antwort + Ausgangszeit ein Bundle (pure).
/// `fallback_refresh` greift, wenn die Antwort keinen neuen Refresh-Token
/// enthält — der Refresh-Flow liefert ihn nicht zwingend mit.
pub fn bundle_from_response(
    resp: &TokenResponse,
    now: chrono::DateTime<chrono::Utc>,
    fallback_refresh: Option<&str>,
) -> Result<TokenBundle> {
    let refresh = resp
        .refresh_token
        .clone()
        .filter(|r| !r.is_empty())
        .or_else(|| fallback_refresh.map(|s| s.to_string()))
        .ok_or_else(|| {
            Error::Mail(
                "Token-Antwort ohne Refresh-Token (fehlt der Scope offline_access?).".into(),
            )
        })?;
    let expires_at = (now + chrono::Duration::seconds(resp.expires_in.max(0))).to_rfc3339();
    Ok(TokenBundle {
        access_token: resp.access_token.clone(),
        refresh_token: refresh,
        expires_at,
    })
}

/// True, wenn der Access-Token (laut `expires_at`, RFC 3339) jetzt oder innerhalb
/// von `skew_secs` abläuft. Unparsbare/leere Zeit gilt als abgelaufen (sicher).
pub fn is_expired(expires_at: &str, now: chrono::DateTime<chrono::Utc>, skew_secs: i64) -> bool {
    match chrono::DateTime::parse_from_rfc3339(expires_at) {
        Ok(exp) => exp.with_timezone(&chrono::Utc) <= now + chrono::Duration::seconds(skew_secs),
        Err(_) => true,
    }
}

// =============================================================================
// Functional Core — Graph-Nachricht
// =============================================================================

/// Baut den Request-Body für Graph `sendMail` (pure). Anhänge als
/// `fileAttachment` mit Base64-Inhalt. `from` wird bewusst NICHT gesetzt — Graph
/// versendet aus dem angemeldeten Postfach (ein abweichendes `from` würde die
/// „Send As"-Berechtigung erfordern).
pub fn build_graph_message(mail: &OutgoingMail) -> serde_json::Value {
    let attachments: Vec<serde_json::Value> = mail
        .attachments
        .iter()
        .map(|a| {
            serde_json::json!({
                "@odata.type": "#microsoft.graph.fileAttachment",
                "name": a.filename,
                "contentType": a.mime_type,
                "contentBytes": base64::engine::general_purpose::STANDARD.encode(&a.bytes),
            })
        })
        .collect();
    serde_json::json!({
        "message": {
            "subject": mail.subject,
            "body": { "contentType": "Text", "content": mail.body_text },
            "toRecipients": [ { "emailAddress": { "address": mail.to } } ],
            "attachments": attachments,
        },
        "saveToSentItems": true
    })
}

// =============================================================================
// Imperative Shell — HTTP
// =============================================================================

fn extract_oauth_error(body: &str) -> String {
    #[derive(Deserialize)]
    struct E {
        error: Option<String>,
        error_description: Option<String>,
    }
    if let Ok(e) = serde_json::from_str::<E>(body) {
        let mut s = e.error.unwrap_or_default();
        if let Some(d) = e.error_description {
            if !d.is_empty() {
                if !s.is_empty() {
                    s.push_str(": ");
                }
                s.push_str(&d);
            }
        }
        if !s.is_empty() {
            return s
                .lines()
                .next()
                .unwrap_or(s.as_str())
                .chars()
                .take(300)
                .collect();
        }
    }
    let t = body.trim();
    if t.is_empty() {
        "unbekannter Fehler".into()
    } else {
        t.chars().take(300).collect()
    }
}

fn extract_graph_error(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(err) = v.get("error") {
            let code = err.get("code").and_then(|c| c.as_str()).unwrap_or("");
            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
            let s = format!("{code} {msg}").trim().to_string();
            if !s.is_empty() {
                return s.chars().take(300).collect();
            }
        }
    }
    let t = body.trim();
    if t.is_empty() {
        "unbekannter Fehler".into()
    } else {
        t.chars().take(300).collect()
    }
}

async fn post_token_form(token_endpoint: &str, form: &[(&str, &str)]) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let resp = client
        .post(token_endpoint)
        .form(form)
        .send()
        .await
        .map_err(|e| Error::Mail(format!("Token-Anfrage fehlgeschlagen: {e}")))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(Error::Mail(format!(
            "Token-Endpoint {}: {}",
            status.as_u16(),
            extract_oauth_error(&body)
        )));
    }
    serde_json::from_str::<TokenResponse>(&body)
        .map_err(|e| Error::Mail(format!("Token-Antwort nicht lesbar: {e}")))
}

/// Tauscht den Authorization-Code gegen Token (Public-Client + PKCE).
pub async fn exchange_code(
    token_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse> {
    post_token_form(
        token_endpoint,
        &[
            ("client_id", client_id),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
            ("scope", GRAPH_SCOPES),
        ],
    )
    .await
}

/// Erneuert den Access-Token über den Refresh-Token (silent refresh).
pub async fn refresh_tokens(
    token_endpoint: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse> {
    post_token_form(
        token_endpoint,
        &[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", GRAPH_SCOPES),
        ],
    )
    .await
}

/// Liest die Postfach-Adresse über Graph `/me` (für die Anzeige „verbunden als").
/// Best-effort: `None` bei jedem Fehler — der Connect scheitert daran nicht.
pub async fn graph_me_email(graph_base: &str, access_token: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{graph_base}/me"))
        .bearer_auth(access_token)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    v.get("mail")
        .and_then(|m| m.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| v.get("userPrincipalName").and_then(|u| u.as_str()))
        .map(|s| s.to_string())
}

/// Provider-Antwort von Graph `/me/sendMail` (Block 16b): HTTP-Status (202 bei
/// Erfolg) + `request-id`-Header (für Microsoft-Support-Fälle).
#[derive(Debug, Clone)]
pub struct GraphResponseInfo {
    pub status: u16,
    pub request_id: Option<String>,
}

/// Versendet eine Mail über Graph `/me/sendMail`. Erwartet `202 Accepted`.
pub async fn graph_send(
    graph_base: &str,
    access_token: &str,
    mail: &OutgoingMail,
) -> Result<GraphResponseInfo> {
    let message = build_graph_message(mail);
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{graph_base}/me/sendMail"))
        .bearer_auth(access_token)
        .json(&message)
        .send()
        .await
        .map_err(|e| Error::Mail(format!("Graph-Versand fehlgeschlagen: {e}")))?;
    let status = resp.status();
    // request-id VOR dem (konsumierenden) text() lesen.
    let request_id = resp
        .headers()
        .get("request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    if status.is_success() {
        Ok(GraphResponseInfo {
            status: status.as_u16(),
            request_id,
        })
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(Error::Mail(format!(
            "Graph /sendMail {}: {}",
            status.as_u16(),
            extract_graph_error(&body)
        )))
    }
}

// =============================================================================
// Imperative Shell — Loopback-Redirect
// =============================================================================

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Bindet einen lokalen Loopback-Listener auf einem flüchtigen Port. Der
/// Redirect-URI ist dann [`loopback_redirect_uri`] mit diesem Port.
pub async fn bind_loopback() -> Result<(TcpListener, u16)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| Error::Mail(format!("Loopback-Port nicht bindbar: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| Error::Mail(format!("Loopback-Adresse nicht lesbar: {e}")))?
        .port();
    Ok((listener, port))
}

/// Der Redirect-URI für einen Loopback-Port.
pub fn loopback_redirect_uri(port: u16) -> String {
    format!("http://localhost:{port}")
}

async fn accept_redirect(listener: &TcpListener) -> Result<String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| Error::Mail(format!("Loopback-Accept fehlgeschlagen: {e}")))?;
        let mut buf = [0u8; 8192];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| Error::Mail(format!("Loopback-Read fehlgeschlagen: {e}")))?;
        let req = String::from_utf8_lossy(&buf[..n]);
        // Erste Zeile: "GET /pfad?query HTTP/1.1"
        let line = req.lines().next().unwrap_or("");
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let query = target.split_once('?').map(|(_, q)| q.to_string());

        // Immer eine freundliche Seite zurückgeben (auch für /favicon.ico).
        let html = b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html lang=\"de\"><head><meta charset=\"utf-8\"><title>Klein.Buch</title></head><body style=\"font-family:sans-serif;padding:2rem;color:#1a1a1a\"><h2>Anmeldung abgeschlossen</h2><p>Du kannst dieses Fenster schlie\xc3\x9fen und zu Klein.Buch zur\xc3\xbcckkehren.</p></body></html>";
        let _ = stream.write_all(html).await;
        let _ = stream.flush().await;

        if let Some(q) = query {
            if !q.is_empty() {
                return Ok(q);
            }
        }
        // Leere/parameterlose Anfrage (z. B. favicon) → nächste Verbindung abwarten.
    }
}

/// Wartet auf genau eine Redirect-Anfrage (mit Query) und gibt deren Query
/// zurück. `timeout` begrenzt die Wartezeit.
pub async fn capture_redirect(
    listener: &TcpListener,
    timeout: std::time::Duration,
) -> Result<String> {
    match tokio::time::timeout(timeout, accept_redirect(listener)).await {
        Ok(res) => res,
        Err(_) => Err(Error::Mail(
            "Zeitüberschreitung: keine Antwort von Microsoft empfangen.".into(),
        )),
    }
}

// =============================================================================
// Tests (Functional Core)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::smtp::{MailAttachment, OutgoingMail};

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        // RFC 7636 Appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = pkce_challenge(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn gen_pkce_is_url_safe_and_long_enough() {
        let (verifier, challenge) = gen_pkce();
        assert!(verifier.len() >= 43 && verifier.len() <= 128);
        assert!(!verifier.contains('+') && !verifier.contains('/') && !verifier.contains('='));
        // Challenge ist deterministisch aus dem Verifier.
        assert_eq!(challenge, pkce_challenge(&verifier));
    }

    #[test]
    fn endpoints_default_to_common_for_blank_tenant() {
        let e = Endpoints::for_tenant("  ");
        assert!(e.authorize.contains("/common/oauth2/v2.0/authorize"));
        assert!(e.token.contains("/common/oauth2/v2.0/token"));
        let e2 = Endpoints::for_tenant("contoso.onmicrosoft.com");
        assert!(e2
            .token
            .contains("/contoso.onmicrosoft.com/oauth2/v2.0/token"));
    }

    #[test]
    fn authorize_url_carries_pkce_and_params() {
        let url = build_authorize_url(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            "client-123",
            "http://localhost:51789",
            "state-xyz",
            "challenge-abc",
            GRAPH_SCOPES,
        )
        .unwrap();
        assert!(url.contains("client_id=client-123"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge=challenge-abc"));
        assert!(url.contains("code_challenge_method=S256"));
        // redirect_uri + scope sind URL-encoded.
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A51789"));
        assert!(url.contains("Mail.Send"));
        assert!(url.contains("offline_access"));
        assert!(url.contains("state=state-xyz"));
    }

    #[test]
    fn parse_callback_extracts_code_and_state() {
        let cb = parse_callback_query("code=abc123&state=st42&session_state=foo").unwrap();
        assert_eq!(cb.code, "abc123");
        assert_eq!(cb.state, "st42");
    }

    #[test]
    fn parse_callback_maps_error_redirect() {
        let err = parse_callback_query("error=access_denied&error_description=User+declined")
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("access_denied"), "msg = {msg}");
        assert!(msg.contains("User declined"), "msg = {msg}");
    }

    #[test]
    fn parse_callback_rejects_missing_fields() {
        assert!(parse_callback_query("code=only-code").is_err());
        assert!(parse_callback_query("").is_err());
    }

    #[test]
    fn bundle_keeps_old_refresh_token_when_response_omits_it() {
        let now = chrono::Utc::now();
        let resp = TokenResponse {
            access_token: "new-access".into(),
            refresh_token: None,
            expires_in: 3600,
            scope: None,
            token_type: Some("Bearer".into()),
        };
        let bundle = bundle_from_response(&resp, now, Some("old-refresh")).unwrap();
        assert_eq!(bundle.access_token, "new-access");
        assert_eq!(bundle.refresh_token, "old-refresh");
        // expires_at ~ now + 3600s.
        let exp = chrono::DateTime::parse_from_rfc3339(&bundle.expires_at).unwrap();
        let delta = exp.with_timezone(&chrono::Utc) - now;
        assert!((3599..=3601).contains(&delta.num_seconds()));
    }

    #[test]
    fn bundle_errors_without_any_refresh_token() {
        let resp = TokenResponse {
            access_token: "a".into(),
            refresh_token: None,
            expires_in: 60,
            scope: None,
            token_type: None,
        };
        assert!(bundle_from_response(&resp, chrono::Utc::now(), None).is_err());
    }

    #[test]
    fn is_expired_respects_skew() {
        let now = chrono::Utc::now();
        let in_one_min = (now + chrono::Duration::seconds(60)).to_rfc3339();
        // Ohne Skew noch gültig …
        assert!(!is_expired(&in_one_min, now, 0));
        // … mit 120s Skew gilt es als abgelaufen.
        assert!(is_expired(&in_one_min, now, 120));
        // Müll = abgelaufen (sicher).
        assert!(is_expired("not-a-date", now, 0));
    }

    #[test]
    fn graph_message_has_recipient_body_and_base64_attachment() {
        let mail = OutgoingMail {
            from_name: "Wildbach".into(),
            from_email: "rechnung@wildbach-computerhilfe.de".into(),
            to: "kunde@example.com".into(),
            subject: "Rechnung RE-2026-0001".into(),
            body_text: "Anbei die Rechnung.".into(),
            attachments: vec![MailAttachment {
                filename: "RE-2026-0001.pdf".into(),
                mime_type: "application/pdf".into(),
                bytes: b"%PDF-1.7 fake".to_vec(),
            }],
        };
        let msg = build_graph_message(&mail);
        assert_eq!(msg["message"]["subject"], "Rechnung RE-2026-0001");
        assert_eq!(
            msg["message"]["toRecipients"][0]["emailAddress"]["address"],
            "kunde@example.com"
        );
        assert_eq!(msg["saveToSentItems"], true);
        let att = &msg["message"]["attachments"][0];
        assert_eq!(att["@odata.type"], "#microsoft.graph.fileAttachment");
        assert_eq!(att["name"], "RE-2026-0001.pdf");
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.7 fake");
        assert_eq!(att["contentBytes"], expected_b64);
        // Kein from → kein "Send As" nötig.
        assert!(msg["message"].get("from").is_none());
    }
}
