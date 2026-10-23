//! HTTP-Server `klein-buch-server` (Plan W5, ADR 0039): axum-Router, die
//! Verbindungen bedient hyper direkt.
//!
//! - [`config`]: Einstellungen aus der Umgebung (Adresse, Schalter)
//! - [`startup`]: Prüfungen vor dem Port-Bind (fail-closed) und beim Entsperren
//! - [`instance`]: Instanz-Marker `/data/.klein-buch-instance`
//! - [`state`]: gemeinsamer Zustand (Kontext, Sitzungen, Jobs, Lastgrenzen)
//! - [`auth`]: `GET /api/auth/status`, `POST /api/auth/setup`, `POST /api/auth/login`
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
pub mod headers;
pub mod health;
pub mod http;
pub mod idempotency;
pub mod instance;
pub mod jobs;
pub mod limits;
pub mod logging;
pub mod session;
pub mod startup;
pub mod state;

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

/// Router mit allen Routen und Middleware (Plan W5).
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
        .route("/jobs/{id}", get(jobs::get_job))
        .route("/public/info", get(public_info))
        .fallback(errors::not_found)
        .route_layer(middleware::from_fn(limits::request_timeout));
    Router::new()
        .nest("/api", api)
        .route("/healthz", get(health::healthz))
        // Archiv-Download folgt in W7; bis dahin kein SPA-Fallback unter /files.
        .route("/files/{*rest}", any(errors::not_found))
        .fallback_service(headers::spa_service(&state.ctx.paths.web_dir))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            limits::global_limit,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            headers::security_headers,
        ))
        .layer(middleware::from_fn(logging::access_log))
        .with_state(state)
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
        "--version" | "version" => {
            println!("klein-buch-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("Unbekannter Befehl „{other}“. Erlaubt: serve, healthcheck, version.");
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
    Ok(ServerState::new(ctx, csp, Some(lock)))
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
