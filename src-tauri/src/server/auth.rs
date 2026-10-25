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
//! - Mit Sitzung: `logout`, `lock` („Datenbank sperren“), `reauth`,
//!   `security`; mit Sitzung + frischer Re-Auth: `totp/renew`,
//!   `recovery/renew`, `passphrase`.
//!
//! Zustandswechsel laufen als eigener Task (abbruchsicher). Vor dem Login gibt
//! es nur generische Meldungen; Fehlversuche stehen als
//! `WARN auth.login.failed ip=… reason=…` im Log (fail2ban-tauglich), nicht im
//! `audit_log` — dort nur `auth.login.ok` / `auth.reauth.ok` ohne IP.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use zeroize::Zeroize as _;

use super::dispatch::{read_body, require_request_header};
use super::errors::ApiError;
use super::instance;
use super::limits::KDF_WAIT;
use super::policy::ClientInfo;
use super::ratelimit::{self, DEVICE_COOKIE};
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
    fn passphrase(&self) -> String {
        self.passphrase.clone().unwrap_or_default()
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
    match st.tasks.spawn(fut).await {
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

/// Audit ohne IP (Plan W6); ein Fehler wird geloggt, nicht gemeldet. Nur
/// erfolgreiche Schritte des Inhabers (Akteur `owner`, Lauf F).
async fn audit(st: &ServerState, action: &str) {
    use crate::db::repo::audit_log;
    if let Ok(pool) = st.ctx.pool() {
        if let Err(e) = audit_log::with_actor(
            audit_log::ACTOR_OWNER,
            audit_log::append(&pool, action, "auth", "session", None),
        )
        .await
        {
            tracing::error!("Audit {action} nicht geschrieben: {e}");
        }
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

fn check_new_passphrase(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < MIN_PASSPHRASE {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "passphrase_too_short",
            "Passphrase muss mindestens 16 Zeichen haben (Tipp: 3–4 zufällige Wörter)",
        ));
    }
    Ok(())
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
        let secret = totp::generate_secret();
        st.set_pending_setup(secret.clone());
        return into_response(totp_offer(&st, &secret));
    }
    let task_state = st.clone();
    detached(&st, async move {
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
    request: AuthRequest,
    key: String,
    has_device: bool,
    device: Option<String>,
) -> Result<Response, ApiError> {
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
    let passphrase = request.body.passphrase();
    if fresh {
        check_new_passphrase(&passphrase)?;
    }
    let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;

    if fresh {
        let result =
            backup_setup_passphrase(&st.ctx, BackupSetupPassphraseArgs { passphrase }).await;
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
                let mut p = passphrase;
                let verified = crate::backup::verify_passphrase(&pool, &p).await;
                p.zeroize();
                verified.map_err(|e| generic_failure("Einrichtung", &e))?
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
    audit(
        &st,
        if fresh {
            "auth.setup.ok"
        } else {
            "auth.totp.setup"
        },
    )
    .await;
    tracing::info!(target: "klein_buch_lib::auth", "auth.setup.ok ip={}", request.info.ip);
    Ok(with_new_session(
        &st,
        serde_json::json!({ "recoveryCodes": codes }),
        device.as_deref(),
    ))
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
    let task_state = st.clone();
    detached(&st, async move {
        login_inner(task_state, request, key, exempt, device).await
    })
    .await
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

async fn login_inner(
    st: Arc<ServerState>,
    request: AuthRequest,
    key: String,
    has_device: bool,
    device: Option<String>,
) -> Result<Response, ApiError> {
    let _transition = st.transition.lock().await;
    let ip = request.info.ip;
    // Erneut unter der Sperre: parallele Versuche bekommen nicht mehr Anläufe.
    if let Err(wait) = st.ratelimit.check(&key, has_device) {
        return Err(rate_limited(wait));
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
        Check::Invalid => {
            st.ratelimit.failure(&key, has_device);
            tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={ip} reason=totp");
            return Err(auth_failed());
        }
    }
    let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;
    let passphrase = request.body.passphrase();
    let ok = match st.instance_state() {
        InstanceState::Locked => unlock(&st, passphrase).await?,
        _ => {
            let pool = st.ctx.pool()?;
            let mut p = passphrase;
            let verified = crate::backup::verify_passphrase(&pool, &p).await;
            p.zeroize();
            verified.map_err(|e| generic_failure("Anmeldung", &e))?
        }
    };
    if !ok {
        st.ratelimit.failure(&key, has_device);
        tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={ip} reason=passphrase");
        return Err(auth_failed());
    }
    let failed = st.ratelimit.success_login(&key);
    audit(&st, "auth.login.ok").await;
    tracing::info!(target: "klein_buch_lib::auth", "auth.login.ok ip={ip}");
    Ok(with_new_session(
        &st,
        serde_json::json!({
            "failedAttempts": failed,
            "usedRecoveryCode": used_recovery,
            "recoveryLeft": st.totp.recovery_left(),
        }),
        device.as_deref(),
    ))
}

/// Entsperren: DB öffnen und prüfen, Archivpfad-Stichprobe, erst dann
/// migrieren und aktivieren (Pool, Scheduler, Audit). Passt die Stichprobe
/// nicht, bleibt die Instanz gesperrt, ohne dass etwas geschrieben wurde.
async fn unlock(st: &Arc<ServerState>, mut passphrase: String) -> Result<bool, ApiError> {
    let ctx = &st.ctx;
    if let Err(e) = ensure_no_pending_restore(&ctx.paths) {
        passphrase.zeroize();
        return Err(generic_failure("Entsperren", &e));
    }
    let pool = match open_encrypted(&ctx.paths, &passphrase).await {
        Ok(Some(pool)) => pool,
        Ok(None) => {
            passphrase.zeroize();
            return Ok(false);
        }
        Err(e) => {
            passphrase.zeroize();
            return Err(generic_failure("Entsperren", &e));
        }
    };
    if let Err(e) = check_archive_paths(&pool, &ctx.paths).await {
        tracing::error!("{e}");
        pool.close().await;
        passphrase.zeroize();
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "unlock_refused",
            "Entsperren verweigert — Einzelheiten im Server-Log.",
        ));
    }
    if let Err(e) = crate::commands::backup::migrate_with_backup(ctx, &pool, &passphrase).await {
        pool.close().await;
        passphrase.zeroize();
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
    activate_unlocked(ctx, pool, passphrase)
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
    // lassen (Gegenprüfung W7).
    st.ctx.downloads.clear();
    st.ctx.uploads.clear();
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
    let task_state = st.clone();
    detached(&st, async move {
        let st = task_state;
        let ip = request.info.ip;
        let _transition = st.transition.lock().await;
        if let Err(wait) = st.ratelimit.check(&key, has_device) {
            return Err(rate_limited(wait));
        }
        let pool = st.ctx.pool()?;
        let (check, _) = check_second_factor(&st, &request.body)?;
        if check != Check::Valid {
            st.ratelimit.failure(&key, has_device);
            tracing::warn!(target: "klein_buch_lib::auth", "auth.reauth.failed ip={ip} reason=totp");
            return Err(auth_failed());
        }
        let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;
        let mut passphrase = request.body.passphrase();
        let verified = crate::backup::verify_passphrase(&pool, &passphrase).await;
        passphrase.zeroize();
        if !verified.map_err(|e| generic_failure("Re-Auth", &e))? {
            st.ratelimit.failure(&key, has_device);
            tracing::warn!(target: "klein_buch_lib::auth", "auth.reauth.failed ip={ip} reason=passphrase");
            return Err(auth_failed());
        }
        st.ratelimit.clear(&key);
        // W14: neue Sitzungs-ID mit frischer Re-Auth; die alte (womöglich
        // gestohlene) verliert das Re-Auth-Recht und läuft kurz danach ab.
        let Some(token) = st.sessions.rotate_with_reauth(id) else {
            return Err(ApiError::unauthenticated());
        };
        audit(&st, "auth.reauth.ok").await;
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

/// `POST /api/auth/passphrase` `{newPassphrase}` (Re-Auth).
pub async fn change_passphrase(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let request = match read_request(req).await {
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
    let new = request.body.new_passphrase.clone().unwrap_or_default();
    if let Err(e) = check_new_passphrase(&new) {
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
async fn rekey(st: Arc<ServerState>, id: SessionId, mut new: String) -> Result<Response, ApiError> {
    let _transition = st.transition.lock().await;
    let ctx = st.ctx.clone();
    if st.writes_blocked() {
        new.zeroize();
        return Err(ApiError::restarting());
    }
    let Some(mut old) = ctx.session.get() else {
        new.zeroize();
        return Err(ApiError::from_error(&Error::Locked));
    };
    if bool::from(old.as_bytes().ct_eq(new.as_bytes())) {
        old.zeroize();
        new.zeroize();
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
            old.zeroize();
            new.zeroize();
            return Err(ApiError::busy(10));
        }
    };
    if !crate::scheduler::tick::stop_within(&ctx, SCHEDULER_STOP_WAIT).await {
        crate::scheduler::tick::withdraw_stop(&ctx);
        drop(gate);
        old.zeroize();
        new.zeroize();
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
        old.zeroize();
        new.zeroize();
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
        old.zeroize();
        new.zeroize();
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
    ctx.set_pool(new_pool.clone());
    ctx.session.set(new.clone());
    old.zeroize();
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
        new.zeroize();
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
