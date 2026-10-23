//! Einrichten und Anmelden (Plan W5.4, vorläufig).
//!
//! W5 kennt nur die Passphrase: `POST /api/auth/setup` legt die DB an (nur im
//! Zustand `setup`), `POST /api/auth/login` entsperrt (Zustand `locked`) bzw.
//! prüft die Passphrase gegen den Verifier (Zustand `unlocked`, kein zweiter
//! Pool). Beides erzeugt eine neue Sitzung. W6 ergänzt Setup-Token, TOTP,
//! Rate-Limit, Re-Auth und die HTTP-Policy (Proxy, Host/Origin).
//!
//! Der Zustandswechsel läuft als eigener Task (Review W5): bricht der Request
//! ab (Client, 30-s-Timeout), werden DB und Marker trotzdem vollständig bzw.
//! gar nicht angelegt. Vor dem Login gibt es nur generische Meldungen;
//! Einzelheiten stehen im Log.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use zeroize::Zeroize as _;

use super::dispatch::{read_body, require_request_header};
use super::errors::ApiError;
use super::instance;
use super::limits::KDF_WAIT;
use super::session;
use super::startup::check_archive_paths;
use super::state::{InstanceState, ServerState};
use crate::commands::backup::{
    activate_unlocked, backup_setup_passphrase, ensure_no_pending_restore, open_encrypted,
    BackupSetupPassphraseArgs,
};
use crate::commands::registry::Cost;
use crate::error::Error;

/// Body-Limit der Auth-Routen.
pub const AUTH_BODY_LIMIT: usize = 16 * 1024;

#[derive(Deserialize)]
struct PassphraseBody {
    passphrase: String,
}

fn auth_failed() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "auth_failed",
        "Anmeldung fehlgeschlagen.",
    )
}

/// Interner Fehler mit generischer Meldung vor dem Login.
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

/// `GET /api/auth/status` → `setup_required` | `login_required` | `ok`.
pub async fn status(State(st): State<Arc<ServerState>>, req: Request) -> Json<serde_json::Value> {
    let state = match st.instance_state() {
        InstanceState::Setup => "setup_required",
        InstanceState::Unlocked if st.sessions.from_headers(req.headers()).is_some() => "ok",
        _ => "login_required",
    };
    Json(serde_json::json!({ "state": state }))
}

fn peer(req: &Request) -> String {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_canonical().to_string())
        .unwrap_or_else(|| "-".into())
}

/// Liest `{passphrase}`; Steuerzeichen (z. B. NUL) werden abgelehnt — sie
/// gehören in keine Passphrase und landeten sonst in SQLite-Fehlertexten.
async fn passphrase_from(req: Request) -> Result<String, ApiError> {
    let (parts, body): (_, Body) = req.into_parts();
    require_request_header(&parts.headers)?;
    let mut bytes = read_body(body, &parts.headers, AUTH_BODY_LIMIT)
        .await?
        .to_vec();
    let parsed = serde_json::from_slice::<PassphraseBody>(&bytes);
    bytes.zeroize();
    let mut passphrase = parsed
        .map(|b| b.passphrase)
        .map_err(|_| ApiError::bad_request("Ungültige Anfrage."))?;
    if passphrase.chars().any(char::is_control) {
        passphrase.zeroize();
        return Err(ApiError::bad_request("Ungültige Anfrage."));
    }
    Ok(passphrase)
}

/// Antwort mit neuer Sitzung.
fn with_new_session(st: &ServerState) -> Response {
    let token = st.sessions.create();
    let mut response = Json(serde_json::json!({ "state": "ok" })).into_response();
    if let Ok(v) = HeaderValue::from_str(&session::set_cookie_value(&token)) {
        response.headers_mut().insert(header::SET_COOKIE, v);
    }
    response
}

/// Führt den Zustandswechsel als eigenen (nachverfolgten) Task aus.
async fn detached<F>(st: &Arc<ServerState>, fut: F) -> Response
where
    F: std::future::Future<Output = Result<Response, ApiError>> + Send + 'static,
{
    match st.tasks.spawn(fut).await {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => err.into_response(),
        Err(join) => generic_failure("Anmeldung", &join).into_response(),
    }
}

/// `POST /api/auth/setup` `{passphrase}` — nur im Zustand `setup`.
pub async fn setup(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let ip = peer(&req);
    let passphrase = match passphrase_from(req).await {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let task_state = st.clone();
    detached(&st, async move {
        setup_locked(task_state, ip, passphrase).await
    })
    .await
}

/// Entfernt eine von diesem Einrichtungsversuch angelegte, unvollständige DB
/// (vorher war der Zustand `setup`, es gab also keine DB). Bliebe sie liegen,
/// könnte der erste Login sie mit beliebiger Passphrase übernehmen.
fn remove_partial_db(st: &ServerState) {
    let db = &st.ctx.paths.db_file;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = db.as_os_str().to_os_string();
        name.push(suffix);
        let path = std::path::PathBuf::from(name);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::error!("Unvollständige DB {} nicht entfernbar: {e}", path.display());
            }
        }
    }
}

async fn setup_locked(
    st: Arc<ServerState>,
    ip: String,
    passphrase: String,
) -> Result<Response, ApiError> {
    let _transition = st.transition.lock().await;
    let data_dir = &st.ctx.paths.data_dir;
    if st.instance_state() != InstanceState::Setup
        || instance::marker_path(data_dir).exists()
        || st.writes_blocked()
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "setup_unavailable",
            "Einrichtung nicht möglich.",
        ));
    }
    let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;
    let result = backup_setup_passphrase(&st.ctx, BackupSetupPassphraseArgs { passphrase }).await;
    match result {
        Ok(_) => {}
        // DB vollständig eingerichtet und entsperrt, erst danach (Erst-Backup)
        // gescheitert: Instanz gilt als eingerichtet; der erste Scheduler-Tick
        // holt das Backup nach.
        Err(e) if st.ctx.is_unlocked() => {
            tracing::error!("Einrichtung abgeschlossen, Erst-Backup fehlgeschlagen: {e}");
        }
        Err(Error::Backup(m)) if m.starts_with("Passphrase muss") => {
            remove_partial_db(&st);
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "passphrase_too_short",
                m,
            ));
        }
        Err(e) => {
            remove_partial_db(&st);
            return Err(generic_failure("Einrichtung", &e));
        }
    }
    if let Err(e) = instance::write_marker(data_dir) {
        return Err(generic_failure("Instanz-Marker", &e));
    }
    tracing::info!(target: "klein_buch_lib::auth", "auth.setup.ok ip={ip}");
    Ok(with_new_session(&st))
}

/// `POST /api/auth/login` `{passphrase}`.
pub async fn login(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let ip = peer(&req);
    let passphrase = match passphrase_from(req).await {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let task_state = st.clone();
    detached(&st, async move {
        login_locked(task_state, ip, passphrase).await
    })
    .await
}

async fn login_locked(
    st: Arc<ServerState>,
    ip: String,
    mut passphrase: String,
) -> Result<Response, ApiError> {
    let _transition = st.transition.lock().await;
    let _kdf = st.admission.admit(Cost::Kdf, KDF_WAIT).await?;
    let ok = match st.instance_state() {
        InstanceState::Setup => {
            passphrase.zeroize();
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "setup_required",
                "Instanz ist noch nicht eingerichtet.",
            ));
        }
        InstanceState::Locked => unlock(&st, passphrase).await?,
        InstanceState::Unlocked => {
            let pool = st.ctx.pool()?;
            let verified = crate::backup::verify_passphrase(&pool, &passphrase).await;
            passphrase.zeroize();
            verified.map_err(|e| generic_failure("Anmeldung", &e))?
        }
    };
    if !ok {
        tracing::warn!(target: "klein_buch_lib::auth", "auth.login.failed ip={ip} reason=passphrase");
        return Err(auth_failed());
    }
    tracing::info!(target: "klein_buch_lib::auth", "auth.login.ok ip={ip}");
    Ok(with_new_session(&st))
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
    if let Err(e) = crate::db::run_migrations(ctx, &pool).await {
        pool.close().await;
        passphrase.zeroize();
        return Err(generic_failure("Entsperren", &e));
    }
    activate_unlocked(ctx, pool, passphrase)
        .await
        .map_err(|e| generic_failure("Entsperren", &e))?;
    Ok(true)
}
