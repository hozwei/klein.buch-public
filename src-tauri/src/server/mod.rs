//! HTTP-Server `klein-buch-server` (Plan W5/W6, ADR 0039): axum-Router, die
//! Verbindungen bedient hyper direkt.
//!
//! - [`config`]: Einstellungen aus der Umgebung (Adresse, Schalter, Policy)
//! - [`policy`]: öffentliche URL, vertraute Proxys, Host/Origin-Prüfung
//! - [`totp`], [`ratelimit`]: zweiter Faktor, Wartezeiten und Gerätecookie
//! - [`startup`]: Prüfungen vor dem Port-Bind (fail-closed) und beim Entsperren
//! - [`instance`]: Instanz-Marker `/data/.klein-buch-instance`
//! - [`state`]: gemeinsamer Zustand (Kontext, Sitzungen, Jobs, Lastgrenzen)
//! - [`auth`]: Status, Einrichtung, Anmeldung, Abmelden, Sperren, Re-Auth,
//!   TOTP/Einmal-Codes erneuern, Passphrase ändern
//! - [`dispatch`]: `POST /api/cmd/{name}` mit Allowlist aus der Registry
//! - [`jobs`], [`idempotency`]: abgekoppelte Abläufe, Wiederholung ohne Doppelwirkung
//! - [`errors`]: Fehlerabbildung auf HTTP-Status und Antwort
//! - [`headers`]: Sicherheits-Header, CSP mit Skript-Hashes, Cache-Control, SPA
//! - [`limits`]: Semaphoren je Kostenklasse, globale Grenze, Timeout
//! - [`http`]: Annahme-Schleife mit Header-Timeout, geordnetes Herunterfahren
//! - [`health`]: `GET /healthz` und Subcommand `healthcheck`
//! - [`logging`]: Log-Filter ohne Geheimnisse, Zugriffsprotokoll

pub mod auth;
pub mod config;
pub mod dispatch;
pub mod errors;
pub mod files;
pub mod headers;
pub mod health;
pub mod http;
pub mod idempotency;
pub mod instance;
pub mod jobs;
pub mod limits;
pub mod logging;
pub mod policy;
pub mod ratelimit;
pub mod session;
pub mod startup;
pub mod state;
pub mod totp;
pub mod uploads;

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;

use axum::routing::{any, get, post};
use axum::{middleware, Json, Router};

use crate::app_ctx::{AppCtx, NoDesktop};
use crate::config::Paths;
use crate::error::{Error, Result};
use state::ServerState;

/// Kennung der Anwendung (früher Tauri-Identifier), für die Info-Anzeige.
pub const APP_IDENTIFIER: &str = "de.wildbach.kleinbuch";

/// Router mit allen Routen und Middleware (Plan W5/W6).
///
/// Öffentlich (ohne Sitzung) sind genau `GET /api/auth/status`,
/// `POST /api/auth/setup`, `POST /api/auth/login`, `GET /api/public/info`,
/// `GET /healthz` und die SPA-Assets (Plan §3); `/legal/` folgt in W12.
pub fn router(state: Arc<ServerState>) -> Router {
    let api = Router::new()
        .route("/cmd/{name}", post(dispatch::cmd))
        .route("/auth/status", get(auth::status))
        .route("/auth/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/lock", post(auth::lock))
        .route("/auth/reauth", post(auth::reauth))
        .route("/auth/security", get(auth::security))
        .route("/auth/totp/renew", post(auth::totp_renew))
        .route("/auth/recovery/renew", post(auth::recovery_renew))
        .route("/auth/passphrase", post(auth::change_passphrase))
        .route("/jobs/{id}", get(jobs::get_job))
        .route("/public/info", get(public_info))
        .fallback(errors::not_found)
        .route_layer(middleware::from_fn(limits::request_timeout));
    // Uploads ohne Gesamt-Timeout und ohne axum-Körperlimit: Größe je Art und
    // Leerlauf-Timeout je Datenblock prüft der Handler (Plan W7).
    let upload = Router::new()
        .route(
            "/api/upload/{art}",
            post(uploads::upload).fallback(errors::not_found),
        )
        .layer(axum::extract::DefaultBodyLimit::disable());
    Router::new()
        .merge(upload)
        .nest("/api", api)
        .route("/healthz", get(health::healthz))
        // HEAD würde einen einmaligen Export verbrauchen bzw. `archive.read`
        // auditieren, ohne auszuliefern (Gegenprüfung W7) → 405.
        .route(
            "/files/download/{id}",
            get(files::download).head(head_not_allowed),
        )
        .route(
            "/files/archive/{id}",
            get(files::archive).head(head_not_allowed),
        )
        // sonst kein SPA-Fallback unter /files
        .route("/files/{*rest}", any(errors::not_found))
        .fallback_service(headers::spa_service(&state.ctx.paths.web_dir))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            limits::global_limit,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            policy::http_policy,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            headers::security_headers,
        ))
        .layer(middleware::from_fn(logging::access_log))
        .with_state(state)
}

async fn head_not_allowed() -> axum::http::StatusCode {
    axum::http::StatusCode::METHOD_NOT_ALLOWED
}

/// `GET /api/public/info` (AGPL §13): Version, Commit, Quelle, Lizenz.
async fn public_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "commit": option_env!("KLEIN_BUCH_COMMIT").unwrap_or("unbekannt"),
        "sourceUrl": config::source_url(),
        "licenseSpdx": "AGPL-3.0-or-later",
    }))
}

/// Einstieg des Binaries: `serve` (Standard) oder `healthcheck`.
pub fn main_entry(args: Vec<OsString>) -> ExitCode {
    let sub = args.first().and_then(|a| a.to_str()).unwrap_or("serve");
    match sub {
        "serve" => run_server(),
        "healthcheck" => health::healthcheck_command(),
        "totp-reset" => auth::totp_reset_command(),
        "--version" | "version" => {
            println!("klein-buch-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!(
                "Unbekannter Befehl „{other}“. Erlaubt: serve, healthcheck, totp-reset, version."
            );
            ExitCode::from(2)
        }
    }
}

fn run_server() -> ExitCode {
    logging::init();
    tracing::info!("Klein.Buch v{} startet", env!("CARGO_PKG_VERSION"));

    // Plan W1.3: Sidecar-Mock nur in Debug-/Test-Builds; ein Release-Build mit
    // gesetztem Schalter verweigert den Start.
    let mock_env = std::env::var(crate::einvoice::SIDECAR_MOCK_ENV).ok();
    if crate::einvoice::mock_policy(cfg!(debug_assertions), mock_env.as_deref())
        == crate::einvoice::MockPolicy::RefuseStart
    {
        tracing::error!(
            "{} ist in Release-Builds nicht erlaubt — Start abgebrochen.",
            crate::einvoice::SIDECAR_MOCK_ENV
        );
        return ExitCode::FAILURE;
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("Tokio-Runtime nicht startbar: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve_main()) {
        Ok(()) => {
            tracing::info!("Klein.Buch beendet");
            ExitCode::SUCCESS
        }
        Err(e) => {
            tracing::error!("Start abgebrochen: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Baut Kontext und Zustand aus der Umgebung, prüft fail-closed und lauscht.
async fn serve_main() -> Result<()> {
    let cfg = config::ServerConfig::from_env()?;
    let paths = Paths::from_env()?;
    let ctx = Arc::new(AppCtx::new(
        paths,
        Arc::new(crate::notify::InboxOnly),
        Arc::new(crate::secrets::KeyringSecretStore),
        Arc::new(NoDesktop),
        APP_IDENTIFIER,
    ));
    let state = prepare(ctx, &cfg)?;
    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .map_err(|e| Error::Config(format!("Port {} nicht verfügbar: {e}", cfg.listen)))?;
    tracing::info!("Klein.Buch lauscht auf {}", cfg.listen);

    // Abgelaufene Downloads und Uploads unabhängig von Anfragen löschen
    // (Plan W7: „nach 1 h gelöscht“, Gegenprüfung).
    let sweeper = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    sweeper.ctx.downloads.sweep();
                    sweeper.ctx.uploads.sweep();
                }
                _ = sweeper.shutdown.cancelled() => break,
            }
        }
    });

    let token = state.shutdown.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        tracing::info!("Signal empfangen — fahre geordnet herunter");
        token.cancel();
    });

    http::serve(listener, router(state.clone()), state.shutdown.clone()).await?;
    state.finish_shutdown().await;
    Ok(())
}

/// Prüfungen vor dem Port-Bind (fail-closed, Plan W5.5) und Aufbau des
/// Zustands. Für Tests getrennt aufrufbar.
pub fn prepare(ctx: Arc<AppCtx>, cfg: &config::ServerConfig) -> Result<Arc<ServerState>> {
    let csp = headers::Csp::load(&ctx.paths.web_dir)?;
    let lock = startup::run_checks(&ctx, &cfg.startup)?;
    let ratelimit = ratelimit::RateLimiter::load_or_create(&ctx.paths.data_dir)?;
    if cfg
        .policy
        .trusted_proxies
        .iter()
        .any(|net| net.prefix_len() == 0)
    {
        tracing::warn!(
            "KLEIN_BUCH_TRUSTED_PROXIES enthält 0.0.0.0/0 bzw. ::/0 — dann vertraut der Server den \
             Weiterleitungs-Headern jedes Clients (Rate-Limit und HTTPS-Regel umgehbar)"
        );
    }
    if cfg.policy.insecure_http {
        tracing::warn!(
            "KLEIN_BUCH_INSECURE_HTTP=1: Einrichten und Anmelden auch über unverschlüsseltes \
             HTTP — nur für Tests und CI, nie im Betrieb"
        );
    }
    ctx.uploads.set_limits(cfg.uploads);
    let state = ServerState::new(ctx, csp, Some(lock), cfg.policy.clone(), ratelimit);
    if state.needs_setup_token() {
        state.ensure_setup_token();
    }
    Ok(state)
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
