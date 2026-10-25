//! `POST /api/cmd/{name}` (Plan W5.2, W5.6).
//!
//! Reihenfolge der Prüfungen: Pflicht-Header → Allowlist (sonst 404) →
//! Sitzung (sonst 401) → Re-Auth bei `Auth::Reauth` (sonst 403
//! `reauth_required`, W6; bei `Auth::Conditional` entscheidet der Kern,
//! Lauf F4a) → Neustart/Restore läuft (503) → Body-Limit (413) → Inhaltstyp
//! `application/json` bei nicht leerem Körper (415, Lauf F4b) → JSON (400)
//! → Render-/JVM-Kontingent der Sitzung (429). Danach je nach [`Kind`]:
//!
//! - `Read`: Zulassung der Kostenklasse, Kern als eigener Task.
//! - `Write`: wie `Read`, zusätzlich Idempotenz-Schlüssel. Der Task läuft auch
//!   weiter, wenn der Client den Request abbricht.
//! - `Job`: `202 {"jobId"}`, Ergebnis über `GET /api/jobs/{id}`.
//!
//! Bei `Write`/`Job` läuft eine registrierte Vorprüfung
//! ([`CommandSpec::precheck`], W14-039) vor der Annahme — beim Restore also vor
//! der Sperre aller Änderungen.
//!
//! DB-Commands im Zustand `locked` liefern über die Fehlerabbildung 423.

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use super::errors::ApiError;
use super::idempotency::{self, Begin, Completer, StoredResponse};
use super::limits::ADMISSION_WAIT;
use super::state::ServerState;
use crate::commands::reauth;
use crate::commands::registry::{self, Auth, CommandSpec, Cost, Kind};
use crate::db::repo::audit_log;

/// Pflicht-Header jeder ändernden API-Anfrage (Plan W5.11; W6 ergänzt die
/// Origin-/Host-Prüfung).
pub const REQUEST_HEADER: &str = "x-kleinbuch-request";

pub async fn cmd(
    State(st): State<Arc<ServerState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    match handle(st, &name, &headers, body).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

/// Prüft den Pflicht-Header.
pub fn require_request_header(headers: &HeaderMap) -> Result<(), ApiError> {
    if headers.contains_key(REQUEST_HEADER) {
        Ok(())
    } else {
        Err(ApiError::missing_request_header())
    }
}

/// Höchstdauer für das Einlesen eines (kleinen) JSON-Körpers: langsam
/// gesendete Körper belegen sonst Plätze der globalen Grenze (W14).
pub const BODY_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// ASVS V13.1.5/V13.2.5 (Lauf F4b): Ein nicht leerer Körper muss als
/// `application/json` (Parameter wie `charset` erlaubt) deklariert sein, sonst
/// 415. Leere Körper (Abmelden, Sperren, Commands ohne Argumente) brauchen
/// keinen Inhaltstyp.
pub fn require_json_content_type(headers: &HeaderMap, bytes: &[u8]) -> Result<(), ApiError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(());
    }
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"));
    if json {
        Ok(())
    } else {
        Err(ApiError::unsupported_media_type(
            "Anfragen mit Inhalt müssen als application/json gesendet werden.",
        ))
    }
}

/// Liest den Körper bis `limit` Bytes, sonst 413 (auch vorab über
/// `Content-Length`); dauert das länger als [`BODY_READ_TIMEOUT`], 408; ein
/// nicht leerer Körper ohne `application/json` ergibt 415
/// ([`require_json_content_type`]).
pub async fn read_body(body: Body, headers: &HeaderMap, limit: usize) -> Result<Bytes, ApiError> {
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|len| len > limit as u64) {
        return Err(ApiError::payload_too_large());
    }
    match tokio::time::timeout(BODY_READ_TIMEOUT, axum::body::to_bytes(body, limit)).await {
        Ok(Ok(bytes)) => {
            require_json_content_type(headers, &bytes)?;
            Ok(bytes)
        }
        Ok(Err(_)) => Err(ApiError::payload_too_large()),
        Err(_) => Err(ApiError::new(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            "body_timeout",
            "Die Anfrage wurde zu langsam übertragen.",
        )),
    }
}

async fn handle(
    st: Arc<ServerState>,
    name: &str,
    headers: &HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    require_request_header(headers)?;
    let spec = registry::find_exposed(name).ok_or_else(ApiError::not_found)?;
    let session = st
        .authenticate(headers)
        .ok_or_else(ApiError::unauthenticated)?;
    let reauth_fresh = st.sessions.reauth_fresh(session);
    if spec.auth == Auth::Reauth && !reauth_fresh {
        return Err(ApiError::reauth_required());
    }
    // W14-038 (Lauf F4a): bei bedingter Re-Auth entscheidet der Kern; als
    // frisch gilt dort auch eine Anmeldung der letzten 5 min.
    let fresh =
        reauth_fresh || (spec.auth == Auth::Conditional && st.sessions.login_fresh(session));
    // W14-033: Idempotenz-Schlüssel gelten nur innerhalb dieser Anmeldung.
    let login = st
        .sessions
        .login_seq(session)
        .ok_or_else(ApiError::unauthenticated)?;
    if st.writes_blocked() && spec.kind != Kind::Read {
        return Err(ApiError::restarting());
    }
    let bytes = read_body(body, headers, spec.body_limit).await?;
    let args: serde_json::Value = if bytes.iter().all(u8::is_ascii_whitespace) {
        serde_json::Value::Object(Default::default())
    } else {
        serde_json::from_slice(&bytes).map_err(|_| ApiError::bad_request("Ungültiges JSON."))?
    };
    if matches!(spec.cost, Cost::Render | Cost::Jvm) {
        st.sessions
            .allow_heavy(session)
            .map_err(|wait| ApiError::rate_limited(wait.as_secs().max(1)))?;
    }
    match spec.kind {
        Kind::Read => execute(st, spec, args, None, fresh).await,
        Kind::Write | Kind::Job => {
            let completer = match idempotency::key_from_headers(headers)? {
                None => None,
                Some(key) => {
                    let hash = idempotency::body_hash(spec.name, &bytes);
                    match st.idem.begin(login, spec.name, &key, hash) {
                        Begin::New(c) => Some(c),
                        Begin::Replay(stored) => return Ok(stored.into_response()),
                        Begin::Wait(rx) => {
                            return Ok(idempotency::wait_for(rx).await?.into_response())
                        }
                        Begin::Mismatch => return Err(ApiError::new(
                            StatusCode::UNPROCESSABLE_ENTITY,
                            "idempotency_mismatch",
                            "Idempotenz-Schlüssel wurde schon für eine andere Anfrage verwendet.",
                        )),
                    }
                }
            };
            // W14-039: Vorprüfung vor der Annahme (z. B. Restore: Quelle und
            // Passphrase), also vor `begin_restore` — ein ungültiger Aufruf
            // sperrt keine Änderungen. Eine Wiederholung mit demselben
            // Idempotenz-Schlüssel bekommt dieselbe Antwort.
            if let Some(precheck) = spec.precheck {
                // Vorprüfungen sehen dieselbe Frische wie der Kern (bedingte
                // Re-Auth vor der Annahme, W14-009).
                if let Err(err) =
                    reauth::with_reauth(fresh, precheck(st.ctx.clone(), args.clone())).await
                {
                    let api = ApiError::from_error(&err);
                    if let Some(c) = completer {
                        c.complete(StoredResponse::json(api.status, &api.body()));
                    }
                    return Err(api);
                }
            }
            if spec.kind == Kind::Job {
                start_job(st, spec, args, completer, fresh)
            } else {
                execute(st, spec, args, completer, fresh).await
            }
        }
    }
}

/// Ergebnis eines Kerns als gespeicherte Antwort.
fn to_stored(outcome: crate::error::Result<serde_json::Value>) -> StoredResponse {
    match outcome {
        Ok(value) => StoredResponse::json(StatusCode::OK, &value),
        Err(err) => {
            let api = ApiError::from_error(&err);
            StoredResponse::json(api.status, &api.body())
        }
    }
}

/// Direkter Aufruf: Zulassung, dann Kern als eigener Task (läuft bei Abbruch
/// des Requests zu Ende und hinterlegt sein Ergebnis beim Idempotenz-Eintrag).
async fn execute(
    st: Arc<ServerState>,
    spec: &'static CommandSpec,
    args: serde_json::Value,
    completer: Option<Completer>,
    fresh: bool,
) -> Result<Response, ApiError> {
    let permit = st.admission.admit(spec.cost, ADMISSION_WAIT).await?;
    let runner = st.clone();
    // Gleiche `ref` wie die Zeile im Zugriffsprotokoll (W14-018).
    let task = st.tasks.spawn(super::logging::with_request_id(async move {
        let _permit = permit;
        let _gate = runner.db_gate.read().await;
        let _running = runner.admission.running(spec.cost);
        // Während des Wartens kann ein Restore begonnen haben (Gegenprüfung W7).
        if runner.writes_blocked() && spec.kind != Kind::Read {
            let api = ApiError::restarting();
            let stored = StoredResponse::json(api.status, &api.body());
            if let Some(c) = completer {
                c.complete(stored.clone());
            }
            return stored;
        }
        // Akteur aller Audit-Einträge des Commands: der Inhaber (Lauf F);
        // Frische der Bestätigung für bedingte Re-Auth im Kern (Lauf F4a).
        let stored = to_stored(
            audit_log::with_actor(
                audit_log::ACTOR_OWNER,
                reauth::with_reauth(fresh, (spec.handler)(runner.ctx.clone(), args)),
            )
            .await,
        );
        if let Some(c) = completer {
            c.complete(stored.clone());
        }
        stored
    }));
    match task.await {
        Ok(stored) => Ok(stored.into_response()),
        Err(join) => Err(ApiError::internal(&format!(
            "Command {} abgebrochen: {join}",
            spec.name
        ))),
    }
}

/// Job: Platz reservieren, sofort `202 {"jobId"}`, Ablauf als eigener Task.
fn start_job(
    st: Arc<ServerState>,
    spec: &'static CommandSpec,
    args: serde_json::Value,
    completer: Option<Completer>,
    fresh: bool,
) -> Result<Response, ApiError> {
    let restore = spec.name == "backup_restore_apply";
    // Ab der Annahme keine Änderungen mehr (sie gingen beim Tausch verloren);
    // ein zweiter Restore gleichzeitig wird abgelehnt (Gegenprüfung W7).
    if restore && !st.begin_restore() {
        return Err(ApiError::restarting());
    }
    let id = match st.jobs.reserve(spec.name) {
        Ok(id) => id,
        Err(e) => {
            if restore {
                st.cancel_restore();
            }
            return Err(e);
        }
    };
    let accepted = StoredResponse::json(
        StatusCode::ACCEPTED,
        &serde_json::json!({ "jobId": id.to_string() }),
    );
    if let Some(c) = completer {
        c.complete(accepted.clone());
    }
    let runner = st.clone();
    st.tasks.spawn(super::logging::with_request_id(async move {
        if restore {
            crate::scheduler::tick::stop(&runner.ctx).await;
        }
        let permit = runner.admission.acquire(spec.cost).await;
        // Der Restore wartet exklusiv auf laufende Commands und Jobs, damit
        // ihre Änderungen im Pre-Restore-Backup stehen (Gegenprüfung W7).
        let write_gate = if restore {
            Some(runner.db_gate.write().await)
        } else {
            None
        };
        let read_gate = if restore {
            None
        } else {
            Some(runner.db_gate.read().await)
        };
        let _running = runner.admission.running(spec.cost);
        let blocked = !restore && runner.writes_blocked() && spec.kind != Kind::Read;
        let outcome = if blocked {
            Err(ApiError::restarting())
        } else {
            let ctx = runner.ctx.clone();
            let inner = tokio::spawn(audit_log::with_actor(
                audit_log::ACTOR_OWNER,
                reauth::with_reauth(fresh, async move { (spec.handler)(ctx, args).await }),
            ));
            match inner.await {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(err)) => Err(ApiError::from_error(&err)),
                Err(join) => Err(ApiError::internal(&format!(
                    "Job {} abgebrochen: {join}",
                    spec.name
                ))),
            }
        };
        drop(_running);
        drop(read_gate);
        drop(write_gate);
        drop(permit);
        let ok = outcome.is_ok();
        runner.jobs.finish(id, outcome);
        if restore {
            if ok {
                runner.request_restart();
            } else {
                runner.abort_restore().await;
            }
        }
    }));
    Ok(accepted.into_response())
}
