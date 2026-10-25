//! Einrichten, Anmelden und Sitzungsverwaltung (Plan W6, E3/E5).
//!
//! - `GET /api/auth/status` → `{state, insecureHttp}` mit `state` =
//!   `setup_required` | `totp_setup_required` | `login_required` | `ok`.
//! - `POST /api/auth/setup` zweistufig über dieselbe Route: `{setupToken,
//!   step:"totp"}` liefert ein neues TOTP-Geheimnis (QR als SVG);
//!   `{setupToken, passphrase, totpCode}` richtet atomar ein — erst Token und
//!   TOTP-Bestätigung, dann die DB (bzw. bei fehlender `totp.json` die
//!   Neueinrichtung des zweiten Faktors mit der bestehenden Passphrase).
//! - `POST /api/auth/login` `{passphrase, totpCode | recoveryCode}`: TOTP
//!   **vor** der KDF, identische Antwort bei falschem TOTP und falscher
//!   Passphrase, Rate-Limit je Gerät bzw. Client-IP.
//! - `POST /api/auth/login/poll`: Ergebnis einer Anmeldung bzw. Einrichtung,
//!   die länger als [`pending::LOGIN_SYNC_WAIT`] dauerte (202 + Ticket-Cookie,
//!   W14-037, Lauf F4a).
//! - Mit Sitzung: `logout`, `lock` („Datenbank sperren“), `reauth`,
//!   `security`, `sessions` (Übersicht, W14-005); mit Sitzung + frischer
//!   Re-Auth: `totp/renew`, `recovery/renew`, `passphrase`,
//!   `sessions/others/revoke`.
//!
//! Zustandswechsel laufen als eigener Task (abbruchsicher). Vor dem Login gibt
//! es nur generische Meldungen; Fehlversuche stehen als
//! `WARN auth.login.failed ip=… reason=…` im Log (fail2ban-tauglich; `reason`
//! = `totp`, `totp_replay`, `passphrase`, `rate_limited`, `totp_locked`), nicht
//! im `audit_log` — dort nur `auth.login.ok` / `auth.reauth.ok` ohne IP.
//! Neue Passphrasen prüft [`super::passphrase_policy`] (Sperrliste, Muster,
//! Kontext; ASVS V2.1.7).

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use zeroize::Zeroize as _;
use zeroize::Zeroizing;

use super::dispatch::{read_body, require_request_header};
use super::errors::ApiError;
use super::instance;
use super::limits::KDF_WAIT;
use super::passphrase_policy::{self, Weakness};
use super::pending::{self, AuthOutcome, AuthSuccess, Taken};
use super::policy::ClientInfo;
use super::ratelimit::{self, Streak, DEVICE_COOKIE, SUSPECT_FACTOR_AFTER, TOTP_LOCK_AFTER};
use super::session::{self, SessionId};
use super::startup::check_archive_paths;
use super::state::{InstanceState, ServerState, DB_GATE_WAIT, SCHEDULER_STOP_WAIT};
use super::totp::{self, Check};
use crate::commands::backup::{
    activate_unlocked, backup_setup_passphrase, ensure_no_pending_restore, open_encrypted,
    BackupSetupPassphraseArgs,
};
use crate::commands::registry::Cost;
use crate::error::Error;

/// Body-Limit der Auth-Routen.
pub const AUTH_BODY_LIMIT: usize = 16 * 1024;
/// Mindestlänge der Passphrase (R5-005, ADR 0035).
pub const MIN_PASSPHRASE: usize = 16;

/// Körper aller Auth-Routen; Geheimnisse werden beim Drop überschrieben.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AuthBody {
    passphrase: Option<String>,
    totp_code: Option<String>,
    recovery_code: Option<String>,
    setup_token: Option<String>,
    step: Option<String>,
    new_passphrase: Option<String>,
}

impl Drop for AuthBody {
    fn drop(&mut self) {
        for field in [
            &mut self.passphrase,
            &mut self.totp_code,
            &mut self.recovery_code,
            &mut self.setup_token,
            &mut self.new_passphrase,
        ] {
            if let Some(s) = field.as_mut() {
                s.zeroize();
            }
        }
    }
}

impl AuthBody {
    /// Nimmt die Passphrase aus dem Körper (ohne Kopie); sie wird beim Drop
    /// des Rückgabewerts überschrieben (W14-015).
    fn take_passphrase(&mut self) -> Zeroizing<String> {
        Zeroizing::new(self.passphrase.take().unwrap_or_default())
    }

    /// Wie [`Self::take_passphrase`] für die neue Passphrase.
    fn take_new_passphrase(&mut self) -> Zeroizing<String> {
        Zeroizing::new(self.new_passphrase.take().unwrap_or_default())
    }
}

/// Gelesene Anfrage einer Auth-Route.
struct AuthRequest {
    info: ClientInfo,
    headers: HeaderMap,
    body: AuthBody,
}

fn auth_failed() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "auth_failed",
        "Anmeldung fehlgeschlagen.",
    )
}

fn insecure_transport() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "insecure_transport",
        "Anmeldung nur über HTTPS (Reverse-Proxy) oder lokal möglich.",
    )
}

fn rate_limited(wait: std::time::Duration) -> ApiError {
    let secs = wait.as_secs().max(1);
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        format!("Anmeldung fehlgeschlagen. Zu viele Fehlversuche — bitte {secs} s warten."),
    )
    .with_retry_after(secs)
}

/// W14-004: Anmeldung mit TOTP ohne Gerätecookie gesperrt (keine Wartezeit,
/// sondern bis zu einer Anmeldung mit Einmal-Code bzw. von einem bekannten
/// Gerät).
fn totp_locked_error() -> ApiError {
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        "totp_locked",
        format!(
            "Anmeldung mit App-Code von neuen Geräten gesperrt ({TOTP_LOCK_AFTER} Fehlversuche in \
             Folge). Bitte mit einem Einmal-Code oder von einem bereits benutzten Gerät anmelden."
        ),
    )
}

/// Interner Fehler mit generischer Meldung (Details nur im Log).
fn generic_failure(what: &str, detail: &dyn std::fmt::Display) -> ApiError {
    let api = ApiError::internal(&format!("{what}: {detail}"));
    ApiError {
        message: format!(
            "{what} fehlgeschlagen (Ref {}).",
            api.reference.as_deref().unwrap_or("-")
        ),
        ..api
    }
}

fn client_info(req: &Request) -> ClientInfo {
    req.extensions()
        .get::<ClientInfo>()
        .copied()
        .unwrap_or(ClientInfo {
            ip: std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            loopback: false,
            via_trusted_proxy: false,
            https: false,
        })
}

/// Liest Pflicht-Header und Körper; Steuerzeichen in Geheimnissen werden
/// abgelehnt (sie gehören in keine Passphrase und landeten sonst in
/// SQLite-Fehlertexten).
async fn read_request(req: Request) -> Result<AuthRequest, ApiError> {
    let info = client_info(&req);
    let (parts, body): (_, Body) = req.into_parts();
    require_request_header(&parts.headers)?;
    let mut bytes = read_body(body, &parts.headers, AUTH_BODY_LIMIT)
        .await?
        .to_vec();
    let parsed = if bytes.iter().all(u8::is_ascii_whitespace) {
        Ok(AuthBody::default())
    } else {
        serde_json::from_slice::<AuthBody>(&bytes)
    };
    bytes.zeroize();
    let body = parsed.map_err(|_| ApiError::bad_request("Ungültige Anfrage."))?;
    let has_control = [
        &body.passphrase,
        &body.totp_code,
        &body.recovery_code,
        &body.setup_token,
        &body.new_passphrase,
    ]
    .iter()
    .any(|f| {
        f.as_deref()
            .is_some_and(|s| s.chars().any(char::is_control))
    });
    if has_control {
        return Err(ApiError::bad_request("Ungültige Anfrage."));
    }
    Ok(AuthRequest {
        info,
        headers: parts.headers,
        body,
    })
}

/// Schlüssel für das Rate-Limit (gültiges Gerätecookie, sonst Client-IP),
/// „hat Gerätecookie“ und dessen Wert (zum Erneuern nach Erfolg, W14).
fn rate_key(
    st: &ServerState,
    headers: &HeaderMap,
    info: &ClientInfo,
) -> (String, bool, Option<String>) {
    let cookie = session::cookie_from_headers(headers, DEVICE_COOKIE);
    let device = cookie.as_deref().and_then(|v| st.ratelimit.device_id(v));
    let has_device = device.is_some();
    let value = if has_device { cookie } else { None };
    (
        ratelimit::key_for(device.as_deref(), info.ip),
        has_device,
        value,
    )
}

/// `Set-Cookie` für das Gerätecookie: vorhandenen gültigen Wert mit frischer
/// Laufzeit erneuern (gleitend, W14), sonst einen neuen ausstellen.
fn device_cookie(st: &ServerState, existing: Option<&str>) -> Option<HeaderValue> {
    let value = existing
        .map(str::to_string)
        .unwrap_or_else(|| st.ratelimit.new_device_cookie());
    HeaderValue::from_str(&ratelimit::device_cookie_header(&value)).ok()
}

/// Antwort mit neuer Sitzung und erneuertem bzw. neuem Gerätecookie.
fn with_new_session(
    st: &ServerState,
    mut body: serde_json::Value,
    device: Option<&str>,
) -> Response {
    let token = st.sessions.create();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("state".into(), "ok".into());
    }
    let mut response = Json(body).into_response();
    if let Ok(v) = HeaderValue::from_str(&session::set_cookie_value(&token)) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    if let Some(v) = device_cookie(st, device) {
        response.headers_mut().append(header::SET_COOKIE, v);
    }
    response
}

/// Führt einen Zustandswechsel als eigenen (nachverfolgten) Task aus.
async fn detached<F>(st: &Arc<ServerState>, fut: F) -> Response
where
    F: std::future::Future<Output = Result<Response, ApiError>> + Send + 'static,
{
    // Audit-Einträge eines Zustandswechsels der Anmeldung schreibt der
    // Inhaber (Lauf F, R1-S3-ACTOR); fehlgeschlagene Versuche auditieren nicht.
    let fut = crate::db::repo::audit_log::with_actor(crate::db::repo::audit_log::ACTOR_OWNER, fut);
    match st.tasks.spawn(super::logging::with_request_id(fut)).await {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => err.into_response(),
        Err(join) => generic_failure("Anmeldung", &join).into_response(),
    }
}

fn into_response(result: Result<Response, ApiError>) -> Response {
    match result {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

/// Antwort auf eine abgeschlossene Anmeldung bzw. Einrichtung: bei Erfolg
/// erst jetzt die neue Sitzung (W14-037: auch beim späten Abholen).
fn issue(st: &ServerState, outcome: AuthOutcome) -> Response {
    match outcome {
        Ok(AuthSuccess { body, device }) => with_new_session(st, body, device.as_deref()),
        Err(e) => e.into_response(),
    }
}

fn joined(result: Result<AuthOutcome, tokio::task::JoinError>) -> AuthOutcome {
    result.unwrap_or_else(|join| Err(generic_failure("Anmeldung", &join)))
}

/// Anmeldung bzw. Einrichtung als eigener Task (W14-037, Lauf F4a): fertig
/// innerhalb von [`ServerState::login_sync_wait`] → Antwort wie bisher; sonst
/// `202 {"state":"pending"}` mit Ticket-Cookie, der Task läuft zu Ende und
/// hinterlegt sein Ergebnis für `POST /api/auth/login/poll`.
async fn detached_auth<F>(st: &Arc<ServerState>, fut: F) -> Response
where
    F: std::future::Future<Output = AuthOutcome> + Send + 'static,
{
    let fut = crate::db::repo::audit_log::with_actor(crate::db::repo::audit_log::ACTOR_OWNER, fut);
    let mut handle = st.tasks.spawn(super::logging::with_request_id(fut));
    match tokio::time::timeout(st.login_sync_wait(), &mut handle).await {
        Ok(result) => issue(st, joined(result)),
        Err(_) => {
            let Some((ticket, key)) = st.pending_auth.park() else {
                // Zu viele offene Abläufe: wie bisher Zeitüberschreitung; der
                // Ablauf läuft trotzdem zu Ende.
                return ApiError::timeout().into_response();
            };
            let runner = st.clone();
            st.tasks.spawn(async move {
                let outcome = joined(handle.await);
                runner.pending_auth.finish(key, outcome);
            });
            tracing::info!(
                target: "klein_buch_lib::auth",
                "Anmeldung dauert länger (z. B. Umstellung der Datenbank) — Ergebnis per Abholung"
            );
            pending_response(Some(&ticket))
        }
    }
}

/// `202 {"state":"pending"}`, beim ersten Mal mit Ticket-Cookie.
fn pending_response(ticket: Option<&str>) -> Response {
    let mut response = (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "state": "pending", "pollAfterMs": 1000 })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    if let Some(t) = ticket {
        if let Ok(v) = HeaderValue::from_str(&pending::ticket_cookie(t)) {
            response.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    response
}

/// Audit ohne IP (Plan W6); ein Fehler wird geloggt, nicht gemeldet. Nur
/// erfolgreiche Schritte des Inhabers (Akteur `owner`, Lauf F).
async fn audit(st: &ServerState, action: &str) {
    audit_details(st, action, None).await;
}

/// Wie [`audit`], mit Details (nie Geheimnisse, nie IP).
async fn audit_details(st: &ServerState, action: &str, details: Option<serde_json::Value>) {
    use crate::db::repo::audit_log;
    if let Ok(pool) = st.ctx.pool() {
        if let Err(e) = audit_log::with_actor(
            audit_log::ACTOR_OWNER,
            audit_log::append(&pool, action, "auth", "session", details),
        )
        .await
        {
            tracing::error!("Audit {action} nicht geschrieben: {e}");
        }
    }
}

/// Inbox-Hinweis zur Sperre der TOTP-Anmeldung ohne Gerätecookie (W14-004);
/// nur bei offener Datenbank (sonst steht die Sperre im Log und der Hinweis
/// „aufgehoben“ folgt nach der nächsten Anmeldung).
async fn totp_lock_notice(st: &ServerState, released: bool) {
    let Ok(pool) = st.ctx.pool() else {
        return;
    };
    let (title, body, severity) = if released {
        (
            "Sperre der App-Code-Anmeldung aufgehoben",
            format!(
                "Die Anmeldung mit App-Code von neuen Geräten war nach {TOTP_LOCK_AFTER} \
                 Fehlversuchen in Folge gesperrt und ist mit dieser Anmeldung wieder frei. \
                 Die Fehlversuche zeigen massenhaftes Durchprobieren; schon falsche App-Codes \
                 zählen mit, das Daten-Passwort ist damit nicht verraten. Prüfe unter \
                 Einstellungen → Sicherheit die Sitzungen und die Zahl der restlichen \
                 Einmal-Codes."
            ),
            "warning",
        )
    } else {
        (
            "Anmeldung mit App-Code gesperrt",
            format!(
                "Nach {TOTP_LOCK_AFTER} Fehlversuchen in Folge ist die Anmeldung mit App-Code von \
                 neuen Geräten gesperrt. Das zeigt massenhaftes Durchprobieren: Schon falsche \
                 App-Codes zählen mit, das Daten-Passwort ist damit nicht verraten. Bereits \
                 benutzte Geräte funktionieren weiter; von einem neuen Gerät hebt eine Anmeldung \
                 mit Einmal-Code die Sperre auf. Jeder Einmal-Code gilt nur einmal — neue \
                 entstehen beim Neueinrichten des App-Codes unter Einstellungen → Sicherheit."
            ),
            "urgent",
        )
    };
    let dedup = format!(
        "totp_lock:{}:{}",
        if released { "released" } else { "locked" },
        st.ctx.clock().now_utc().timestamp()
    );
    if let Err(e) = crate::notify::store::create(
        &pool,
        crate::notify::store::NewNotification {
            title,
            body: &body,
            severity,
            action_url: Some("/settings/security"),
            dedup_key: Some(&dedup),
            ..Default::default()
        },
    )
    .await
    {
        tracing::error!("Hinweis zur TOTP-Sperre nicht geschrieben: {e}");
    }
}

/// Falsche Passphrase nach **gültigem** zweitem Faktor (Lauf F4d): Anders als
/// die Obergrenze in Folge setzt das einen gültigen App-Code bzw. Einmal-Code
/// voraus. Ab [`SUSPECT_FACTOR_AFTER`] in Folge (ohne erfolgreiche Anmeldung
/// bzw. Re-Auth dazwischen) einmal je Folge WARN-Log und dringender Hinweis:
/// Der zweite Faktor ist womöglich bekannt, empfohlen ist dessen
/// Neueinrichtung (neues Geheimnis, neue Einmal-Codes).
async fn factor_suspect(st: &ServerState, ip: std::net::IpAddr) {
    let (count, streak) = st.ratelimit.factor_suspect_failure();
    if count != SUSPECT_FACTOR_AFTER {
        return;
    }
    tracing::warn!(
        target: "klein_buch_lib::auth",
        "auth.factor_suspect ip={ip} failures={SUSPECT_FACTOR_AFTER} — gültiger zweiter Faktor, \
         falsches Daten-Passwort"
    );
    factor_suspect_notice(st, &streak).await;
}

/// Erfolgreiche Anmeldung bzw. Re-Auth beendet die Folge. Reichte sie bis zur
/// Grenze, wird der Hinweis jetzt (erneut) geschrieben: Bei gesperrter
/// Instanz (nach einem Neustart) gab es beim Erreichen keine offene DB; die
/// Kennung der Folge im `dedup_key` verhindert einen doppelten Hinweis.
async fn factor_suspect_end(st: &ServerState) {
    let (count, streak) = st.ratelimit.factor_suspect_clear();
    if count >= SUSPECT_FACTOR_AFTER {
        factor_suspect_notice(st, &streak).await;
    }
}

async fn factor_suspect_notice(st: &ServerState, streak: &str) {
    let Ok(pool) = st.ctx.pool() else {
        return;
    };
    let body = format!(
        "{SUSPECT_FACTOR_AFTER} Anmeldeversuche in Folge hatten einen gültigen App-Code bzw. \
         Einmal-Code, aber ein falsches Daten-Passwort. Warst du das nicht, kennt jemand \
         womöglich das Geheimnis deiner Authenticator-App oder einen Einmal-Code: Richte den \
         App-Code unter Einstellungen → Sicherheit neu ein (dabei entstehen neue Einmal-Codes) \
         und prüfe die Sitzungen. Das Daten-Passwort selbst war bei diesen Versuchen falsch."
    );
    let dedup = format!("factor_suspect:{streak}");
    if let Err(e) = crate::notify::store::create(
        &pool,
        crate::notify::store::NewNotification {
            title: "Gültiger App-Code mit falschem Daten-Passwort",
            body: &body,
            severity: "urgent",
            action_url: Some("/settings/security"),
            dedup_key: Some(&dedup),
            ..Default::default()
        },
    )
    .await
    {
        tracing::error!("Hinweis zum zweiten Faktor nicht geschrieben: {e}");
    }
}

/// Zählt einen Fehlversuch ohne Gerätecookie für die Obergrenze in Folge
/// (W14-004); beim Erreichen WARN-Log und Hinweis. Zählt bewusst auch falsche
/// App-Codes ohne jede Kenntnis der Passphrase: Die Grenze begrenzt das
/// Durchprobieren des zweiten Faktors (NIST SP 800-63B §5.2.2); dass sie
/// dadurch von jedem auslösbar ist, steht in security.md (V2.2.1).
async fn count_streak(st: &ServerState, ip: std::net::IpAddr) {
    if st.ratelimit.streak_failure() == Streak::Tripped {
        tracing::warn!(
            target: "klein_buch_lib::auth",
            "auth.totp_locked ip={ip} failures={TOTP_LOCK_AFTER} — Anmeldung mit TOTP ohne \
             Gerätecookie gesperrt bis zu einer Anmeldung mit Einmal-Code bzw. bekanntem Gerät"
        );
        totp_lock_notice(st, false).await;
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// `GET /api/auth/status`.
pub async fn status(State(st): State<Arc<ServerState>>, req: Request) -> Json<serde_json::Value> {
    let state = match st.instance_state() {
        InstanceState::Setup => "setup_required",
        _ if !st.totp.exists() => {
            // Nach `totp-reset` im laufenden Betrieb: Token für die
            // Neueinrichtung bereitstellen (einmal, im Container-Log).
            st.ensure_setup_token();
            st.sessions.clear();
            "totp_setup_required"
        }
        InstanceState::Unlocked if st.authenticate(req.headers()).is_some() => "ok",
        _ => "login_required",
    };
    Json(serde_json::json!({
        "state": state,
        "insecureHttp": st.policy.insecure_http,
    }))
}

// ---------------------------------------------------------------------------
// Zweiter Faktor
// ---------------------------------------------------------------------------

/// Prüft TOTP-Code bzw. Einmal-Code (vor jeder KDF).
fn check_second_factor(st: &ServerState, body: &AuthBody) -> Result<(Check, bool), ApiError> {
    let result = if let Some(code) = body.totp_code.as_deref().filter(|c| !c.trim().is_empty()) {
        st.totp.verify_code(code, st.totp_now()).map(|c| (c, false))
    } else if let Some(code) = body
        .recovery_code
        .as_deref()
        .filter(|c| !c.trim().is_empty())
    {
        st.totp.use_recovery(code).map(|c| (c, true))
    } else {
        Ok((Check::Invalid, false))
    };
    result.map_err(|e| generic_failure("Anmeldung", &e))
}

/// Code gegen ein ausstehendes (noch nicht gespeichertes) Geheimnis prüfen;
/// liefert den passenden Zeitschritt (er gilt danach als verbraucht).
fn code_matches_pending(secret: &[u8], code: &str, now_unix: u64) -> Option<u64> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let now = totp::step_at(now_unix);
    let mut matched = None;
    for step in [now.saturating_sub(1), now, now + 1] {
        let expected = totp::code_for_step(secret, step);
        if bool::from(expected.as_bytes().ct_eq(code.as_bytes())) && matched.is_none() {
            matched = Some(step);
        }
    }
    matched.filter(|_| code.len() == totp::DIGITS as usize)
}

fn totp_offer(st: &ServerState, secret: &[u8]) -> Result<Response, ApiError> {
    let uri = totp::otpauth_uri(secret, &st.policy.public_url.host);
    let svg = totp::qr_svg(&uri).map_err(|e| generic_failure("QR-Code", &e))?;
    Ok(Json(serde_json::json!({
        "secret": totp::encode_secret(secret),
        "otpauthUri": uri,
        "qrSvg": svg,
    }))
    .into_response())
}

/// Prüft eine **neue** Passphrase (Einrichtung, Wechsel): Mindestlänge, dann
/// [`passphrase_policy::check`] mit Kontextbegriffen (z. B. Firmenname und
/// E-Mail aus dem Profil). Bestehende Passphrasen prüft niemand nach.
fn check_new_passphrase(p: &str, context: &[String]) -> Result<(), ApiError> {
    if p.chars().count() < MIN_PASSPHRASE {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "passphrase_too_short",
            "Passphrase muss mindestens 16 Zeichen haben (Tipp: 3–4 zufällige Wörter)",
        ));
    }
    match passphrase_policy::check(p, context) {
        Ok(()) => Ok(()),
        Err(Weakness::TooLong) => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "passphrase_too_long",
            format!(
                "Passphrase darf höchstens {} Zeichen haben.",
                passphrase_policy::MAX_CHARS
            ),
        )),
        Err(_) => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "passphrase_common",
            "Diese Passphrase ist zu leicht zu erraten (bekannter Satz, einfaches Muster oder \
             Firmenname/E-Mail). Tipp: 3–4 zufällige Wörter.",
        )),
    }
}

/// Kontextbegriffe für eine neue Passphrase aus dem Verkäuferprofil (bei
/// offener Datenbank; sonst keine).
async fn passphrase_context(st: &ServerState) -> Vec<String> {
    let Ok(pool) = st.ctx.pool() else {
        return Vec::new();
    };
    match crate::db::repo::seller_profile::get(&pool).await {
        Ok(Some(p)) => passphrase_policy::profile_context(&p.name, &p.email),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Einrichtung
// ---------------------------------------------------------------------------

/// `POST /api/auth/setup` (zweistufig, siehe Moduldoku).
pub async fn setup(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let request = match read_request(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if !st.policy.auth_transport_ok(&request.info) {
        return insecure_transport().into_response();
    }
    let (key, has_device, device) = rate_key(&st, &request.headers, &request.info);
    if let Err(wait) = st.ratelimit.check(&key, has_device) {
        return rate_limited(wait).into_response();
    }
    let token_ok = request
        .body
        .setup_token
        .as_deref()
        .is_some_and(|t| st.setup_token_matches(t));
    if !token_ok {
        st.ratelimit.failure(&key, has_device);
        tracing::warn!(target: "klein_buch_lib::auth", "auth.setup.failed ip={} reason=token", request.info.ip);
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "setup_token_invalid",
            "Einrichtung nicht möglich (Setup-Token aus dem Container-Log prüfen).",
        )
        .into_response();
    }
    if request.body.step.as_deref() == Some("totp") {
        // Lauf F4a: Bei der Ersteinrichtung kann das Frontend die neue
        // Passphrase schon hier prüfen lassen (vor dem QR-Code korrigierbar);
        // verbindlich bleibt die Prüfung im letzten Schritt.
        if let Some(p) = request.body.passphrase.as_deref() {
            if st.instance_state() == InstanceState::Setup {
                if let Err(e) = check_new_passphrase(p, &[]) {
                    return e.into_response();
                }
            }
        }
        let secret = totp::generate_secret();
        st.set_pending_setup(secret.clone());
        return into_response(totp_offer(&st, &secret));
    }
    let task_state = st.clone();
    detached_auth(&st, async move {
        setup_final(task_state, request, key, has_device, device).await
    })
    .await
}

/// Entfernt eine von diesem Einrichtungsversuch angelegte, unvollständige DB
/// (vorher war der Zustand `setup`, es gab also keine DB). Bliebe sie liegen,
/// könnte der erste Login sie mit beliebiger Passphrase übernehmen.
fn remove_partial_db(st: &ServerState) {
    remove_db_files(&st.ctx.paths.db_file);
}

/// Löscht die DB-Datei samt `-wal`/`-shm`/`-journal` ohne `exists()` vorab
/// (Lauf F2c, LE-CI-FLAKE): SQLite entfernt `-wal`/`-shm` beim Schließen des
/// Pools selbst; eine dazwischen verschwundene Datei ist kein Fehler und
/// erzeugt keinen Fehler-Logeintrag mehr. Liefert die Zahl der Dateien, die
/// sich nicht entfernen ließen.
fn remove_db_files(db: &std::path::Path) -> usize {
    let mut failed = 0;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = db.as_os_str().to_os_string();
        name.push(suffix);
        let path = std::path::PathBuf::from(name);
        if let Err(e) = crate::db::remove_file_if_exists(&path) {
            tracing::error!("Unvollständige DB {} nicht entfernbar: {e}", path.display());
            failed += 1;
        }
    }
    failed
}

async fn setup_final(
    st: Arc<ServerState>,
    mut request: AuthRequest,
    key: String,
    has_device: bool,
    device: Option<String>,
) -> AuthOutcome {
    let _transition = st.transition.lock().await;
    // Erneut unter der Sperre: parallele Versuche bekommen nicht mehr Anläufe.
    if let Err(wait) = st.ratelimit.check(&key, has_device) {
        return Err(rate_limited(wait));
    }
    let data_dir = st.ctx.paths.data_dir.clone();
    let fresh = st.instance_state() == InstanceState::Setup;
    let resetup = !fresh && !st.totp.exists();
    if (!fresh && !resetup)
        || (fresh && instance::marker_path(&data_dir).exists())
        || st.writes_blocked()
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "setup_unavailable",
            "Einrichtung nicht möglich.",
        ));
    }
    let secret = st.pending_setup().ok_or_else(|| {
        ApiError::new(
            StatusCode::CONFLICT,
            "totp_offer_missing",
            "Bitte zuerst den QR-Code für den zweiten Faktor anfordern.",
        )
    })?;
    let code = request.body.totp_code.clone().unwrap_or_default();
    let Some(confirmed_step) = code_matches_pending(&secret, &code, st.totp_now()) else {
        st.ratelimit.failure(&key, has_device);
        tracing::warn!(target: "klein_buch_lib::auth", "auth.setup.failed ip={} reason=totp", request.info.ip);
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "totp_invalid",
            "Der TOTP-Code stimmt nicht — Uhrzeit des Geräts prüfen und den aktuellen Code eingeben.",
        ));
    };
    let mut passphrase = request.body.take_passphrase();
    if fresh {
        check_new_passphrase(&passphrase, &[])?;
    }
    let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;

    if fresh {
        // Ohne Kopie in den Kern (er überschreibt sie beim Verlassen).
        let result = backup_setup_passphrase(
            &st.ctx,
            BackupSetupPassphraseArgs {
                passphrase: std::mem::take(&mut *passphrase),
            },
        )
        .await;
        match result {
            Ok(_) => {}
            // DB vollständig eingerichtet, erst danach (Erst-Backup) gescheitert:
            // der erste Scheduler-Tick holt das Backup nach.
            Err(e) if st.ctx.is_unlocked() => {
                tracing::error!("Einrichtung abgeschlossen, Erst-Backup fehlgeschlagen: {e}");
            }
            Err(e) => {
                remove_partial_db(&st);
                return Err(generic_failure("Einrichtung", &e));
            }
        }
        instance::write_marker(&data_dir).map_err(|e| generic_failure("Instanz-Marker", &e))?;
    } else {
        // Neueinrichtung des zweiten Faktors: Passphrase gegen die DB prüfen.
        let ok = match st.instance_state() {
            InstanceState::Locked => unlock(&st, passphrase).await?,
            _ => {
                let pool = st.ctx.pool()?;
                crate::backup::verify_passphrase(&pool, &passphrase)
                    .await
                    .map_err(|e| generic_failure("Einrichtung", &e))?
            }
        };
        if !ok {
            st.ratelimit.failure(&key, has_device);
            tracing::warn!(target: "klein_buch_lib::auth", "auth.setup.failed ip={} reason=passphrase", request.info.ip);
            return Err(auth_failed());
        }
    }

    let codes = st
        .totp
        .install(&secret, confirmed_step)
        .map_err(|e| generic_failure("TOTP", &e))?;
    st.finish_setup();
    st.sessions.clear();
    st.ratelimit.clear(&key);
    // Einrichtung mit Setup-Token (Container-Zugriff) und Passphrase hebt eine
    // Sperre der TOTP-Anmeldung auf (W14-004).
    let was_locked = st.ratelimit.streak_success() >= TOTP_LOCK_AFTER;
    audit(
        &st,
        if fresh {
            "auth.setup.ok"
        } else {
            "auth.totp.setup"
        },
    )
    .await;
    if was_locked {
        totp_lock_notice(&st, true).await;
    }
    tracing::info!(target: "klein_buch_lib::auth", "auth.setup.ok ip={}", request.info.ip);
    Ok(AuthSuccess {
        body: serde_json::json!({ "recoveryCodes": codes }),
        device,
    })
}

// ---------------------------------------------------------------------------
// Anmeldung
// ---------------------------------------------------------------------------

/// `POST /api/auth/login`.
pub async fn login(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let request = match read_request(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if !st.policy.auth_transport_ok(&request.info) {
        return insecure_transport().into_response();
    }
    let (key, has_device, device) = rate_key(&st, &request.headers, &request.info);
    // W14: Ein Login mit Einmal-Code (10 Codes zu 50 Bit, Raten aussichtslos)
    // zählt nur unter dem eigenen Schlüssel, nicht im globalen Budget — sonst
    // könnte ein Angreifer das Budget dauerhaft ausschöpfen und den Inhaber auf
    // jedem neuen Gerät aussperren. Die Passphrase bleibt Pflicht.
    let recovery_only = request.body.recovery_code.is_some() && request.body.totp_code.is_none();
    let exempt = has_device || recovery_only;
    if let Err(wait) = st.ratelimit.check(&key, exempt) {
        tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={} reason=rate_limited", request.info.ip);
        return rate_limited(wait).into_response();
    }
    // W14-004: nach TOTP_LOCK_AFTER Fehlversuchen in Folge keine Anmeldung mit
    // TOTP ohne Gerätecookie mehr (Einmal-Code und bekannte Geräte bleiben).
    if !exempt && st.ratelimit.totp_locked() {
        tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={} reason=totp_locked", request.info.ip);
        return totp_locked_error().into_response();
    }
    let task_state = st.clone();
    detached_auth(&st, async move {
        login_inner(task_state, request, key, exempt, device).await
    })
    .await
}

/// `POST /api/auth/login/poll` (W14-037): Ergebnis einer Anmeldung bzw.
/// Einrichtung, die mit 202 beantwortet wurde. 202 solange sie läuft; danach
/// einmal das Ergebnis (200 mit Sitzung bzw. Fehler); unbekannt → 410.
pub async fn login_poll(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let request = match read_request(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if !st.policy.auth_transport_ok(&request.info) {
        return insecure_transport().into_response();
    }
    let clear_ticket = |mut response: Response| {
        if let Ok(v) = HeaderValue::from_str(&pending::clear_ticket_cookie()) {
            response.headers_mut().append(header::SET_COOKIE, v);
        }
        response
    };
    let taken = session::cookie_from_headers(&request.headers, pending::TICKET_COOKIE)
        .map_or(Taken::Unknown, |t| st.pending_auth.take(&t));
    match taken {
        Taken::Running => pending_response(None),
        Taken::Done(outcome) => clear_ticket(issue(&st, outcome)),
        Taken::Unknown => clear_ticket(
            ApiError::new(
                StatusCode::GONE,
                "login_expired",
                "Anmeldung abgelaufen oder unbekannt — bitte erneut anmelden.",
            )
            .into_response(),
        ),
    }
}

fn totp_missing(st: &ServerState) -> ApiError {
    st.ensure_setup_token();
    st.sessions.clear();
    ApiError::new(
        StatusCode::FORBIDDEN,
        "totp_setup_required",
        "Zweiter Faktor fehlt — Neueinrichtung mit dem Setup-Token aus dem Container-Log.",
    )
}

/// Anmeldung unter der Zustandssperre. `exempt`: Gerätecookie bzw. reiner
/// Einmal-Code-Login (zählt nicht im globalen Budget und nicht für die
/// Obergrenze in Folge, W14-004).
async fn login_inner(
    st: Arc<ServerState>,
    mut request: AuthRequest,
    key: String,
    exempt: bool,
    device: Option<String>,
) -> AuthOutcome {
    let _transition = st.transition.lock().await;
    let ip = request.info.ip;
    // Erneut unter der Sperre: parallele Versuche bekommen nicht mehr Anläufe.
    if let Err(wait) = st.ratelimit.check(&key, exempt) {
        return Err(rate_limited(wait));
    }
    if !exempt && st.ratelimit.totp_locked() {
        tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={ip} reason=totp_locked");
        return Err(totp_locked_error());
    }
    if st.instance_state() == InstanceState::Setup {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "setup_required",
            "Instanz ist noch nicht eingerichtet.",
        ));
    }
    if !st.totp.exists() {
        return Err(totp_missing(&st));
    }
    // Zweiter Faktor vor der KDF; ein gültiger Code gilt als verbraucht.
    let (check, used_recovery) = check_second_factor(&st, &request.body)?;
    match check {
        Check::Valid => {}
        Check::Missing => return Err(totp_missing(&st)),
        Check::Invalid | Check::Replay => {
            st.ratelimit.failure(&key, exempt);
            if !exempt {
                count_streak(&st, ip).await;
            }
            // ASVS V2.8.5: Wiederholung eines verbrauchten Codes eigens im Log
            // (die Antwort bleibt dieselbe).
            let reason = if check == Check::Replay {
                "totp_replay"
            } else {
                "totp"
            };
            tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={ip} reason={reason}");
            return Err(auth_failed());
        }
    }
    let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;
    let passphrase = request.body.take_passphrase();
    let ok = match st.instance_state() {
        InstanceState::Locked => unlock(&st, passphrase).await?,
        _ => {
            let pool = st.ctx.pool()?;
            crate::backup::verify_passphrase(&pool, &passphrase)
                .await
                .map_err(|e| generic_failure("Anmeldung", &e))?
        }
    };
    if !ok {
        st.ratelimit.failure(&key, exempt);
        if !exempt {
            count_streak(&st, ip).await;
        }
        tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={ip} reason=passphrase");
        factor_suspect(&st, ip).await;
        return Err(auth_failed());
    }
    factor_suspect_end(&st).await;
    let failed = st.ratelimit.success_login(&key);
    // W14-004: eine erfolgreiche Anmeldung beendet die Folge; war die
    // TOTP-Anmeldung gesperrt, hebt diese (Gerätecookie bzw. Einmal-Code) sie auf.
    let was_locked = st.ratelimit.streak_success() >= TOTP_LOCK_AFTER;
    audit(&st, "auth.login.ok").await;
    if was_locked {
        totp_lock_notice(&st, true).await;
        tracing::warn!(target: "klein_buch_lib::auth", "auth.totp_unlocked ip={ip}");
    }
    tracing::info!(target: "klein_buch_lib::auth", "auth.login.ok ip={ip}");
    Ok(AuthSuccess {
        body: serde_json::json!({
            "failedAttempts": failed,
            "usedRecoveryCode": used_recovery,
            "recoveryLeft": st.totp.recovery_left(),
        }),
        device,
    })
}

/// Entsperren: DB öffnen und prüfen, Archivpfad-Stichprobe, erst dann
/// migrieren und aktivieren (Pool, Scheduler, Audit). Passt die Stichprobe
/// nicht, bleibt die Instanz gesperrt, ohne dass etwas geschrieben wurde.
async fn unlock(
    st: &Arc<ServerState>,
    mut passphrase: Zeroizing<String>,
) -> Result<bool, ApiError> {
    // `passphrase` wird auf jedem Rückweg beim Drop überschrieben (W14-015).
    let ctx = &st.ctx;
    if let Err(e) = ensure_no_pending_restore(&ctx.paths) {
        return Err(generic_failure("Entsperren", &e));
    }
    let pool = match open_encrypted(&ctx.paths, &passphrase).await {
        Ok(Some(pool)) => pool,
        Ok(None) => return Ok(false),
        Err(e) => return Err(generic_failure("Entsperren", &e)),
    };
    if let Err(e) = check_archive_paths(&pool, &ctx.paths).await {
        tracing::error!("{e}");
        pool.close().await;
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "unlock_refused",
            "Entsperren verweigert — Einzelheiten im Server-Log.",
        ));
    }
    if let Err(e) = crate::commands::backup::migrate_with_backup(ctx, &pool, &passphrase).await {
        pool.close().await;
        let msg = e.to_string();
        if msg.contains(crate::commands::backup::PRE_MIGRATION_FAILED) {
            // Plan W7: klare Meldung, Einzelheiten im Log (dort schon protokolliert).
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "pre_migration_backup_failed",
                "Entsperren abgebrochen: Vor der Datenbank-Umstellung ließ sich kein Backup \
                 schreiben. Es wurde nichts verändert — Einzelheiten im Server-Log.",
            ));
        }
        if msg.contains(crate::db::INTEGRITY_REFUSED)
            && msg.contains(crate::db::INTEGRITY_AFTER_MIGRATION)
        {
            // Lauf F2c: Die Prüfung schlug erst nach der gespeicherten
            // Umstellung an — „nichts verändert“ stimmt dann nicht.
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "db_integrity_failed",
                "Entsperren abgebrochen: Nach der Umstellung der Datenbank hat die Prüfung \
                 Unstimmigkeiten gefunden. Die Umstellung ist gespeichert, vorher wurde ein \
                 Backup angelegt — Rückweg und Einzelheiten im Server-Log (Handbuch: Fehlersuche \
                 im Container).",
            ));
        }
        if msg.contains(crate::db::INTEGRITY_REFUSED) {
            // R1-012 / Vorab-Prüfung vor der Umstellung: Einzelheiten stehen
            // schon im Log; die Datenbank ist unverändert.
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "db_integrity_failed",
                "Entsperren abgebrochen: Die Prüfung der Datenbank hat Unstimmigkeiten gefunden. \
                 Daten wurden nicht verändert — Einzelheiten im Server-Log (Handbuch: Fehlersuche \
                 im Container).",
            ));
        }
        return Err(generic_failure("Entsperren", &e));
    }
    // Ohne Kopie in die Sitzung (der leere Rest wird überschrieben).
    activate_unlocked(ctx, pool, std::mem::take(&mut *passphrase))
        .await
        .map_err(|e| generic_failure("Entsperren", &e))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Mit Sitzung
// ---------------------------------------------------------------------------

fn session_of(st: &ServerState, headers: &HeaderMap) -> Result<SessionId, ApiError> {
    st.authenticate(headers)
        .ok_or_else(ApiError::unauthenticated)
}

fn require_reauth(st: &ServerState, id: SessionId) -> Result<(), ApiError> {
    if st.sessions.reauth_fresh(id) {
        Ok(())
    } else {
        Err(ApiError::reauth_required())
    }
}

fn ok_clearing_cookie() -> Response {
    let mut response = Json(serde_json::json!({ "state": "ok" })).into_response();
    if let Ok(v) = HeaderValue::from_str(&session::clear_cookie_value()) {
        response.headers_mut().insert(header::SET_COOKIE, v);
    }
    response
}

/// `POST /api/auth/logout`.
pub async fn logout(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    into_response(
        async {
            let request = read_request(req).await?;
            let id = session_of(&st, &request.headers)?;
            st.sessions.remove(id);
            Ok(ok_clearing_cookie())
        }
        .await,
    )
}

/// `POST /api/auth/lock` — „Datenbank sperren“: beendet sofort alle Sitzungen,
/// bittet den Scheduler um Abbruch nach dem laufenden Termin, wartet auf
/// laufende Aktionen (höchstens [`DB_GATE_WAIT`]) und den Tick (höchstens
/// [`SCHEDULER_STOP_WAIT`]), schreibt ein ausstehendes Lock-Backup (Lauf F3c),
/// schließt den Pool und überschreibt die Passphrase im RAM (W14-027).
pub async fn lock(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let request = match read_request(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = session_of(&st, &request.headers) {
        return e.into_response();
    }
    let task_state = st.clone();
    detached(&st, async move {
        lock_database(&task_state)
            .await
            .map(|()| ok_clearing_cookie())
    })
    .await
}

/// Sperrt die Datenbank (auch für Tests aufrufbar).
///
/// Reihenfolge (W14-027, ADR 0040): Sitzungen und Klartext-Downloads zuerst
/// (eine fremde Sitzung ist sofort draußen), dann die kooperative
/// Abbruch-Anforderung an den Scheduler, dann die Schreibsperre. Läuft der
/// Tick nach [`SCHEDULER_STOP_WAIT`] noch (hängender Sidecar, SMTP-Timeout),
/// wird trotzdem gesperrt: Pool und Passphrase sind weg, der Tick scheitert am
/// nächsten Datenbankzugriff und weitere Ticks laufen leer. Ein ausstehendes
/// Lock-Backup wird vorher noch geschrieben (höchstens [`DB_GATE_WAIT`],
/// Lauf F3c, ADR 0040 Nr. 57).
pub async fn lock_database(st: &Arc<ServerState>) -> Result<(), ApiError> {
    let _transition = st.transition.lock().await;
    st.sessions.clear();
    // Exporte (Klartext!) und Uploads nicht über das Sperren hinaus liegen
    // lassen (Gegenprüfung W7). Dazu (W14-015, Lauf F4a): offene
    // Anmelde-Ergebnisse und ein ausstehendes TOTP-Geheimnis.
    st.ctx.downloads.clear();
    st.ctx.uploads.clear();
    st.pending_auth.clear();
    st.clear_pending_renew();
    crate::scheduler::tick::request_stop(&st.ctx);
    let _gate = match tokio::time::timeout(DB_GATE_WAIT, st.db_gate.write()).await {
        Ok(gate) => gate,
        Err(_) => {
            // Gesperrt wird nicht; der Scheduler läuft weiter. Die Sitzungen
            // bleiben beendet (Anmeldung nötig, dann erneut sperren).
            crate::scheduler::tick::withdraw_stop(&st.ctx);
            return Err(ApiError::busy(10));
        }
    };
    audit(st, "auth.lock").await;
    if !crate::scheduler::tick::stop_within(&st.ctx, SCHEDULER_STOP_WAIT).await {
        tracing::warn!(
            target: "klein_buch_lib::auth",
            "Datenbank sperren: Scheduler-Tick läuft nach {} s noch — Sperre ohne Abwarten",
            SCHEDULER_STOP_WAIT.as_secs()
        );
    }
    // Lauf F3c: ein ausstehendes Lock-Backup (eben festgeschriebener Beleg)
    // vor dem Schließen des Pools schreiben, höchstens DB_GATE_WAIT.
    crate::scheduler::tick::flush_pending_backup(&st.ctx, DB_GATE_WAIT).await;
    if let Some(pool) = st.ctx.take_pool() {
        // Neue Zugriffe scheitern ab sofort; auf ausgeliehene Verbindungen
        // höchstens DB_GATE_WAIT warten.
        if tokio::time::timeout(DB_GATE_WAIT, pool.close())
            .await
            .is_err()
        {
            tracing::warn!(target: "klein_buch_lib::auth", "Datenbank sperren: Pool schließt verzögert");
        }
    }
    st.ctx.session.clear();
    st.sessions.clear();
    // W14-015/W14-033 (Lauf F4a): gespeicherte Antworten (Idempotenz) und
    // Ergebnisse beendeter Jobs überdauern das Sperren nicht. Laufende Aktionen
    // gibt es hier nicht mehr (Schreibsperre `db_gate`).
    st.idem.clear();
    st.jobs.clear_finished();
    tracing::info!(target: "klein_buch_lib::auth", "Datenbank gesperrt");
    Ok(())
}

/// `POST /api/auth/reauth` `{passphrase, totpCode | recoveryCode}`.
pub async fn reauth(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let request = match read_request(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let id = match session_of(&st, &request.headers) {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };
    if !st.policy.auth_transport_ok(&request.info) {
        return insecure_transport().into_response();
    }
    let (key, has_device, device) = rate_key(&st, &request.headers, &request.info);
    if let Err(wait) = st.ratelimit.check(&key, has_device) {
        return rate_limited(wait).into_response();
    }
    // W14-004: auch die Re-Auth mit TOTP ohne Gerätecookie ist gesperrt.
    if !has_device && st.ratelimit.totp_locked() {
        tracing::warn!(target: "klein_buch_lib::auth", "auth.reauth.failed ip={} reason=totp_locked", request.info.ip);
        return totp_locked_error().into_response();
    }
    let task_state = st.clone();
    detached(&st, async move {
        let st = task_state;
        let mut request = request;
        let ip = request.info.ip;
        let _transition = st.transition.lock().await;
        if let Err(wait) = st.ratelimit.check(&key, has_device) {
            return Err(rate_limited(wait));
        }
        if !has_device && st.ratelimit.totp_locked() {
            return Err(totp_locked_error());
        }
        let pool = st.ctx.pool()?;
        let (check, _) = check_second_factor(&st, &request.body)?;
        if check != Check::Valid {
            st.ratelimit.failure(&key, has_device);
            if !has_device {
                count_streak(&st, ip).await;
            }
            let reason = if check == Check::Replay {
                "totp_replay"
            } else {
                "totp"
            };
            tracing::warn!(target: "klein_buch_lib::auth", "auth.reauth.failed ip={ip} reason={reason}");
            return Err(auth_failed());
        }
        let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;
        let passphrase = request.body.take_passphrase();
        let verified = crate::backup::verify_passphrase(&pool, &passphrase).await;
        drop(passphrase);
        if !verified.map_err(|e| generic_failure("Re-Auth", &e))? {
            st.ratelimit.failure(&key, has_device);
            if !has_device {
                count_streak(&st, ip).await;
            }
            tracing::warn!(target: "klein_buch_lib::auth", "auth.reauth.failed ip={ip} reason=passphrase");
            factor_suspect(&st, ip).await;
            return Err(auth_failed());
        }
        factor_suspect_end(&st).await;
        st.ratelimit.clear(&key);
        // W14: neue Sitzungs-ID mit frischer Re-Auth; die alte (womöglich
        // gestohlene) verliert das Re-Auth-Recht und läuft kurz danach ab.
        let Some(token) = st.sessions.rotate_with_reauth(id) else {
            return Err(ApiError::unauthenticated());
        };
        audit(&st, "auth.reauth.ok").await;
        // ASVS 7.2.1 (Lauf F4c): auch die erfolgreiche Re-Auth mit IP im Log.
        tracing::info!(target: "klein_buch_lib::auth", "auth.reauth.ok ip={ip}");
        let mut response = Json(serde_json::json!({
            "state": "ok",
            "reauthSeconds": session::REAUTH_WINDOW.as_secs(),
        }))
        .into_response();
        if let Ok(v) = HeaderValue::from_str(&session::set_cookie_value(&token)) {
            response.headers_mut().append(header::SET_COOKIE, v);
        }
        if let Some(v) = device_cookie(&st, device.as_deref()) {
            response.headers_mut().append(header::SET_COOKIE, v);
        }
        Ok(response)
    })
    .await
}

/// `GET /api/auth/security` — Stand des zweiten Faktors.
pub async fn security(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    if let Err(e) = session_of(&st, req.headers()) {
        return e.into_response();
    }
    Json(serde_json::json!({
        "recoveryLeft": st.totp.recovery_left(),
        "insecureHttp": st.policy.insecure_http,
    }))
    .into_response()
}

/// `GET /api/auth/sessions` (W14-005): gültige Sitzungen mit Anmeldezeit,
/// letzter Nutzung (UTC wie DB-Zeitstempel), gekürzter Kennung und „diese
/// Sitzung“. Keine IP, kein Gerätename (nicht gespeichert).
pub async fn sessions(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let id = match session_of(&st, req.headers()) {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };
    let now = st.ctx.clock().now_utc();
    let at = |ago: std::time::Duration| {
        let ago = chrono::Duration::from_std(ago).unwrap_or_else(|_| chrono::Duration::zero());
        crate::clock::to_db_utc(now - ago)
    };
    let list: Vec<serde_json::Value> = st
        .sessions
        .overview(id)
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.short_id,
                "loginAt": at(s.since_login),
                "lastSeenAt": at(s.since_seen),
                "current": s.current,
                "reauthActive": s.reauth_active,
            })
        })
        .collect();
    Json(serde_json::json!({ "count": list.len(), "sessions": list })).into_response()
}

/// `POST /api/auth/sessions/others/revoke` (Re-Auth, W14-005): beendet alle
/// anderen Sitzungen; Audit `auth.sessions_revoked` mit der Anzahl.
pub async fn revoke_other_sessions(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    into_response(
        async {
            let request = read_request(req).await?;
            let id = session_of(&st, &request.headers)?;
            require_reauth(&st, id)?;
            let revoked = st.sessions.retain_only(id);
            audit_details(
                &st,
                "auth.sessions_revoked",
                Some(serde_json::json!({ "count": revoked })),
            )
            .await;
            tracing::info!(target: "klein_buch_lib::auth", "auth.sessions_revoked count={revoked}");
            Ok(Json(serde_json::json!({ "state": "ok", "revoked": revoked })).into_response())
        }
        .await,
    )
}

/// `POST /api/auth/totp/renew` (Re-Auth): `{step:"totp"}` → neues Geheimnis;
/// `{totpCode}` → ersetzt Geheimnis und Einmal-Codes.
pub async fn totp_renew(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    into_response(
        async {
            let request = read_request(req).await?;
            if !st.policy.auth_transport_ok(&request.info) {
                return Err(insecure_transport());
            }
            let id = session_of(&st, &request.headers)?;
            require_reauth(&st, id)?;
            if request.body.step.as_deref() == Some("totp") {
                let secret = totp::generate_secret();
                st.set_pending_renew(id, secret.clone());
                return totp_offer(&st, &secret);
            }
            let secret = st.take_pending_renew(id).ok_or_else(|| {
                ApiError::new(
                    StatusCode::CONFLICT,
                    "totp_offer_missing",
                    "Bitte zuerst den neuen QR-Code anfordern.",
                )
            })?;
            let code = request.body.totp_code.clone().unwrap_or_default();
            let Some(confirmed_step) = code_matches_pending(&secret, &code, st.totp_now()) else {
                // Angebot bleibt gültig für einen neuen Versuch.
                st.set_pending_renew(id, secret);
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "totp_invalid",
                    "Der TOTP-Code stimmt nicht.",
                ));
            };
            let codes = st
                .totp
                .install(&secret, confirmed_step)
                .map_err(|e| generic_failure("TOTP", &e))?;
            // W14: Andere Sitzungen enden (Verdacht auf Kompromittierung).
            st.sessions.retain_only(id);
            audit(&st, "auth.totp.renewed").await;
            Ok(Json(serde_json::json!({ "state": "ok", "recoveryCodes": codes })).into_response())
        }
        .await,
    )
}

/// `POST /api/auth/recovery/renew` (Re-Auth) — neue Einmal-Codes.
pub async fn recovery_renew(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    into_response(
        async {
            let request = read_request(req).await?;
            if !st.policy.auth_transport_ok(&request.info) {
                return Err(insecure_transport());
            }
            let id = session_of(&st, &request.headers)?;
            require_reauth(&st, id)?;
            let codes = st
                .totp
                .renew_recovery()
                .map_err(|e| generic_failure("Einmal-Codes", &e))?;
            st.sessions.retain_only(id);
            audit(&st, "auth.recovery.renewed").await;
            Ok(Json(serde_json::json!({ "state": "ok", "recoveryCodes": codes })).into_response())
        }
        .await,
    )
}

/// `POST /api/auth/passphrase` `{newPassphrase}` (Re-Auth). Die neue
/// Passphrase prüft [`check_new_passphrase`] samt Firmenname und E-Mail aus
/// dem Profil (ASVS V2.1.7, Lauf F4a).
pub async fn change_passphrase(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let mut request = match read_request(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if !st.policy.auth_transport_ok(&request.info) {
        return insecure_transport().into_response();
    }
    let id = match session_of(&st, &request.headers).and_then(|id| {
        require_reauth(&st, id)?;
        Ok(id)
    }) {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };
    let new = request.body.take_new_passphrase();
    let context = passphrase_context(&st).await;
    if let Err(e) = check_new_passphrase(&new, &context) {
        return e.into_response();
    }
    let task_state = st.clone();
    detached(&st, async move { rekey(task_state, id, new).await }).await
}

/// Passphrase wechseln: Schreibsperre + Scheduler-Pause → Pool zu → neu
/// verschlüsseln (Temp + rename) → Pool mit neuem Key, Verifier neu →
/// Sitzungs-Passphrase tauschen → andere Sitzungen ungültig → Antwort; das
/// Backup mit der neuen Passphrase läuft danach als eigener Task (sonst
/// überschritte der Wechsel leicht den 30-s-Timeout, Review W6).
///
/// Fehlerfälle: Welche Passphrase danach gilt, entscheidet nicht die
/// Annahme, sondern ein Probe-Öffnen (erst alt, dann neu). Lässt sich der
/// Verifier nicht schreiben, wird gesperrt — das Entsperren prüft dann über
/// SQLCipher selbst, also nur mit der neuen Passphrase.
///
/// Alte und neue Passphrase liegen als `Zeroizing` vor und werden auf jedem
/// Rückweg beim Drop überschrieben (W14-015).
async fn rekey(
    st: Arc<ServerState>,
    id: SessionId,
    new: Zeroizing<String>,
) -> Result<Response, ApiError> {
    let _transition = st.transition.lock().await;
    let ctx = st.ctx.clone();
    if st.writes_blocked() {
        return Err(ApiError::restarting());
    }
    let Some(old) = ctx.session.get() else {
        return Err(ApiError::from_error(&Error::Locked));
    };
    if bool::from(old.as_bytes().ct_eq(new.as_bytes())) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "passphrase_unchanged",
            "Die neue Passphrase ist dieselbe wie die bisherige.",
        ));
    }
    // W14-027: Scheduler zuerst um Abbruch bitten, dann Schreibsperre; auf den
    // Tick höchstens SCHEDULER_STOP_WAIT warten (der Wechsel braucht die Datei
    // exklusiv — läuft der Tick noch, lieber 503 als unbegrenzt warten).
    crate::scheduler::tick::request_stop(&ctx);
    let gate = match tokio::time::timeout(DB_GATE_WAIT, st.db_gate.write()).await {
        Ok(g) => g,
        Err(_) => {
            crate::scheduler::tick::withdraw_stop(&ctx);
            return Err(ApiError::busy(10));
        }
    };
    if !crate::scheduler::tick::stop_within(&ctx, SCHEDULER_STOP_WAIT).await {
        crate::scheduler::tick::withdraw_stop(&ctx);
        drop(gate);
        return Err(ApiError::busy(30));
    }
    if let Some(pool) = ctx.take_pool() {
        pool.close().await;
    }
    let outcome = crate::db::rekey_db_file(&ctx.paths.db_file, &old, &new).await;
    let new_pool = match outcome {
        Ok(()) => open_encrypted(&ctx.paths, &new).await.ok().flatten(),
        Err(_) => None,
    };
    let Some(new_pool) = new_pool else {
        // Probe-Öffnen: welche Passphrase passt jetzt?
        let reason = match outcome {
            Err(e) => e.to_string(),
            Ok(()) => "neue Passphrase öffnet die neu verschlüsselte Datenbank nicht".into(),
        };
        let message = if let Ok(Some(pool)) = open_encrypted(&ctx.paths, &old).await {
            ctx.set_pool(pool);
            crate::scheduler::tick::resume(&ctx).await;
            "Passphrase nicht geändert — die bisherige gilt weiter."
        } else {
            // Weder sicher alt noch neu offen: gesperrt lassen, Anmeldung
            // entscheidet (SQLCipher prüft die Passphrase selbst).
            ctx.session.clear();
            st.sessions.clear();
            "Passphrase-Wechsel unklar abgebrochen — bitte neu anmelden (zuerst mit der neuen, \
             sonst mit der bisherigen Passphrase)."
        };
        drop(gate);
        let api = generic_failure("Passphrase ändern", &reason);
        return Err(ApiError {
            message: format!(
                "{message} (Ref {})",
                api.reference.as_deref().unwrap_or("-")
            ),
            ..api
        });
    };
    if let Err(e) = crate::backup::store_verifier(&new_pool, &new).await {
        // Verifier veraltet: nicht entsperrt weiterlaufen (sonst gälte die alte
        // Passphrase für Re-Auth), sondern sperren.
        new_pool.close().await;
        ctx.session.clear();
        st.sessions.clear();
        drop(gate);
        let api = generic_failure("Passphrase ändern", &e);
        return Err(ApiError {
            message: format!(
                "Datenbank neu verschlüsselt, Verifier nicht geschrieben — bitte mit der neuen \
                 Passphrase neu anmelden (Ref {}).",
                api.reference.as_deref().unwrap_or("-")
            ),
            ..api
        });
    }
    drop(old);
    ctx.set_pool(new_pool.clone());
    // Die Sitzung hält eine eigene, beim Ersetzen überschriebene Kopie.
    ctx.session.set(new.to_string());
    st.sessions.retain_only(id);
    crate::scheduler::tick::resume(&ctx).await;
    if let Err(e) = crate::db::repo::audit_log::with_actor(
        crate::db::repo::audit_log::ACTOR_OWNER,
        crate::db::repo::audit_log::append(
            &new_pool,
            "backup.passphrase.changed",
            "backup",
            "passphrase",
            None,
        ),
    )
    .await
    {
        tracing::error!("Audit backup.passphrase.changed nicht geschrieben: {e}");
    }
    drop(gate);
    // Sofort ein Backup mit der neuen Passphrase (ältere bleiben mit der alten
    // einspielbar) — nach der Antwort, das Herunterfahren wartet darauf.
    let paths = ctx.paths.clone();
    st.tasks.spawn(async move {
        match crate::backup::create_now(&new_pool, &paths, &new, "manual").await {
            Ok(o) => tracing::info!("Backup nach Passphrase-Wechsel: {}", o.file_path),
            Err(e) => tracing::error!("Backup nach Passphrase-Wechsel fehlgeschlagen: {e}"),
        }
        // `new` wird hier beim Drop überschrieben.
    });
    tracing::info!(target: "klein_buch_lib::auth", "Passphrase geändert");
    Ok(Json(serde_json::json!({ "state": "ok", "backupStarted": true })).into_response())
}

// ---------------------------------------------------------------------------
// Subcommand
// ---------------------------------------------------------------------------

/// `klein-buch-server totp-reset` (per `docker exec`): löscht `totp.json`
/// unter `flock`. Der laufende Server beendet daraufhin alle Sitzungen; der
/// nächste Login gibt einen neuen Setup-Token aus.
pub fn totp_reset_command() -> std::process::ExitCode {
    let Some(data_dir) = std::env::var_os(crate::config::ENV_DATA_DIR).filter(|v| !v.is_empty())
    else {
        eprintln!("{} fehlt.", crate::config::ENV_DATA_DIR);
        return std::process::ExitCode::FAILURE;
    };
    let store = totp::TotpStore::new(std::path::Path::new(&data_dir));
    match store.reset() {
        Ok(true) => {
            println!(
                "TOTP-Geheimnis gelöscht. Alle Sitzungen enden; beim nächsten Login steht ein \
                 neuer Setup-Token im Container-Log. Neueinrichtung mit Setup-Token + Passphrase."
            );
            std::process::ExitCode::SUCCESS
        }
        Ok(false) => {
            println!("Kein TOTP-Geheimnis vorhanden — nichts zu tun.");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("totp-reset fehlgeschlagen: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lauf F2c (LE-CI-FLAKE): fehlende Begleitdateien (`-wal`/`-shm`/`-journal`,
    /// die SQLite beim Schließen schon entfernt hat) sind kein Fehler; vorhandene
    /// werden gelöscht.
    #[test]
    fn remove_db_files_ignores_vanished_companions() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("klein-buch.db");
        assert_eq!(remove_db_files(&db), 0, "nichts da: kein Fehler");
        std::fs::write(&db, b"x").unwrap();
        std::fs::write(dir.path().join("klein-buch.db-wal"), b"x").unwrap();
        assert_eq!(remove_db_files(&db), 0);
        assert!(!db.exists());
        assert!(!dir.path().join("klein-buch.db-wal").exists());
    }
}
