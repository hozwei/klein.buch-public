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
//! - [`legal`]: Lizenz- und Quellenseiten unter `/legal/` (W12)
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
pub mod legal;
pub mod limits;
pub mod logging;
pub mod passphrase_policy;
pub mod pending;
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

/// Kennung der Anwendung (aus der Desktop-Fassung übernommen), für die Info-Anzeige.
pub const APP_IDENTIFIER: &str = "de.wildbach.kleinbuch";

/// Router mit allen Routen und Middleware (Plan W5/W6).
///
/// Öffentlich (ohne Sitzung) sind genau `GET /api/auth/status`,
/// `POST /api/auth/setup`, `POST /api/auth/login`, `POST /api/auth/login/poll`
/// (nur mit Ticket-Cookie, W14-037), `GET /api/public/info`,
/// `GET /healthz`, `GET /legal/…` (W12) und die SPA-Assets (Plan §3).
/// `/api/public/info` und `/legal/` zählen zusätzlich Anfragen je Client
/// ([`ratelimit::public_limit`]).
pub fn router(state: Arc<ServerState>) -> Router {
    layered(routes(&state), state)
}

/// Alle Routen ohne den gemeinsamen Middleware-Stapel ([`layered`]).
fn routes(state: &Arc<ServerState>) -> Router<Arc<ServerState>> {
    // Öffentliche Lese-Routen zählen Anfragen je Client (W12).
    let public_limit = middleware::from_fn_with_state(state.clone(), ratelimit::public_limit);
    let info = get(public_info).route_layer(public_limit.clone());
    let legal_index = get(legal::index).route_layer(public_limit.clone());
    let legal_file = get(legal::file).route_layer(public_limit);
    let api = Router::new()
        .route("/cmd/{name}", post(dispatch::cmd))
        .route("/auth/status", get(auth::status))
        .route("/auth/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/login/poll", post(auth::login_poll))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/lock", post(auth::lock))
        .route("/auth/reauth", post(auth::reauth))
        .route("/auth/security", get(auth::security))
        .route("/auth/sessions", get(auth::sessions))
        .route(
            "/auth/sessions/others/revoke",
            post(auth::revoke_other_sessions),
        )
        .route("/auth/totp/renew", post(auth::totp_renew))
        .route("/auth/recovery/renew", post(auth::recovery_renew))
        .route("/auth/passphrase", post(auth::change_passphrase))
        .route("/jobs/{id}", get(jobs::get_job))
        .route("/public/info", info)
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
        // Lizenz- und Quellenseiten ohne Sitzung (W12); kein SPA-Fallback.
        .route("/legal", get(legal::redirect))
        .route("/legal/", legal_index)
        .route("/legal/{*file}", legal_file)
        .fallback_service(headers::spa_service(&state.ctx.paths.web_dir))
}

/// Middleware-Stapel um `routes` (von innen nach außen): globale Grenze,
/// Panik-Abfang (ASVS 7.4.3, Lauf F4b), Host-/Origin-Policy, Sicherheits-
/// Header, Zugriffsprotokoll. Der Panik-Abfang sitzt innerhalb von Policy,
/// Headern und Protokoll, damit auch die 500 nach einer Panik HSTS, CSP,
/// nosniff und eine Protokollzeile bekommt. Öffentlich für Tests (Route, die
/// panict).
pub fn layered(routes: Router<Arc<ServerState>>, state: Arc<ServerState>) -> Router {
    routes
        .layer(middleware::from_fn_with_state(
            state.clone(),
            limits::global_limit,
        ))
        .layer(tower_http::catch_panic::CatchPanicLayer::custom(
            panic_response,
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

/// Antwort nach einer Panik in Handler oder Middleware (ASVS 7.4.3, Lauf
/// F4b): 500 im `ApiError`-Format mit `ref`, ERROR-Log nur mit der Referenz —
/// ohne Panik-Text und ohne Anfrage-Inhalt. Die Verbindung bleibt nutzbar.
pub fn panic_response(_panic: Box<dyn std::any::Any + Send + 'static>) -> axum::response::Response {
    axum::response::IntoResponse::into_response(errors::ApiError::internal(
        &"Panik bei der Bearbeitung einer Anfrage",
    ))
}

async fn head_not_allowed() -> axum::http::StatusCode {
    axum::http::StatusCode::METHOD_NOT_ALLOWED
}

/// `GET /api/public/info` (AGPL §13, Plan W12): Version, Commit, Tag,
/// Quell-URL, Link auf den exakten Quellstand (ohne Commit `null`), Lizenz.
async fn public_info() -> Json<crate::build_info::SourceInfo> {
    Json(crate::build_info::current())
}

/// Einstieg des Binaries: `serve` (Standard), `healthcheck`, `totp-reset`,
/// `version`/`--version`.
pub fn main_entry(args: Vec<OsString>) -> ExitCode {
    // W9-Fund: SQLite rechnet `datetime('now','utc')` (Standardwerte und
    // Trigger aller Tabellen) relativ zur Prozess-Zeitzone; mit
    // TZ=Europe/Berlin (E12) lägen alle Zeitstempel 1–2 h zu früh. Fachliche
    // Daten rechnen mit `crate::clock` (chrono-tz Europe/Berlin), nie mit der
    // Prozess-TZ. Gesetzt vor jedem weiteren Thread und vor dem ersten
    // Zugriff auf die Zeitzone.
    std::env::set_var("TZ", "UTC");
    crate::db::init_crypto();
    // Plan W11: kein Core-Dump und kein ptrace-Zugriff anderer Prozesse
    // desselben Nutzers auf den Speicher (Passphrase, Schlüssel).
    #[cfg(target_os = "linux")]
    if let Err(e) =
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
    {
        eprintln!("PR_SET_DUMPABLE nicht gesetzt: {e}");
    }
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
    let ctx = Arc::new(AppCtx::new(paths, Arc::new(NoDesktop), APP_IDENTIFIER));
    let state = prepare(ctx, &cfg)?;
    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .map_err(|e| Error::Config(format!("Port {} nicht verfügbar: {e}", cfg.listen)))?;
    tracing::info!("Klein.Buch lauscht auf {}", cfg.listen);

    // R3-014 (Lauf F4b): Sidecar im Hintergrund prüfen — Log und Inbox-Hinweis
    // beim nächsten Entsperren, kein Abbruch.
    let health_ctx = state.ctx.clone();
    tokio::spawn(async move {
        crate::einvoice::sidecar_health::startup_check(&health_ctx).await;
    });

    // Minütlich: abgelaufene Downloads und Uploads löschen (Plan W7: „nach 1 h
    // gelöscht“). Stündlich: alte Reste in data/tmp und abgebrochene
    // Backup-Teildateien (D-13). Alle 6 h im gesperrten Zustand ein Hinweis im
    // Log (D-19). Gesperrt nach dem Start: sofort.
    log_if_locked(&state);
    let sweeper = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        let mut minutes: u64 = 0;
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    sweeper.ctx.downloads.sweep();
                    sweeper.ctx.uploads.sweep();
                    if minutes > 0 && minutes.is_multiple_of(60) {
                        let paths = sweeper.ctx.paths.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            startup::sweep_tmp(&paths.data_dir, startup::TMP_MAX_AGE);
                            startup::sweep_partial_backups(&paths.backups_dir, startup::TMP_MAX_AGE);
                        })
                        .await;
                    }
                    if minutes > 0 && minutes.is_multiple_of(LOCKED_LOG_EVERY_MINUTES) {
                        log_if_locked(&sweeper);
                    }
                    minutes += 1;
                }
                _ = sweeper.shutdown.cancelled() => break,
            }
        }
    });

    // Signal → geordnet herunterfahren. Der Scheduler erhält die
    // Abbruch-Anforderung sofort (auch beim Neustart nach einem Restore),
    // nicht erst nach dem Abwarten der Verbindungen (W14-025b).
    let token = state.shutdown.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        tracing::info!("Signal empfangen — fahre geordnet herunter");
        token.cancel();
    });
    let stopper = state.clone();
    tokio::spawn(async move {
        stopper.shutdown.cancelled().await;
        crate::scheduler::tick::request_stop(&stopper.ctx);
    });

    // Lauf F4d: Grenze je Gegenstelle; vertraute Proxys bündeln alle Clients
    // und zählen nur gegen die globale Grenze.
    let limits = http::ConnectionLimits {
        unlimited_peers: state.policy.trusted_proxies.clone(),
        ..Default::default()
    };
    http::serve_with(
        listener,
        router(state.clone()),
        state.shutdown.clone(),
        limits,
    )
    .await?;
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
    if let Some(warning) = crate::build_info::source_url_warning() {
        tracing::warn!("{warning}");
    }
    if cfg.policy.insecure_http {
        tracing::warn!(
            "KLEIN_BUCH_INSECURE_HTTP=1: Einrichten und Anmelden auch über unverschlüsseltes \
             HTTP — nur für Tests und CI, nie im Betrieb"
        );
    }
    ctx.uploads.set_limits(cfg.uploads);
    ctx.net_tests.set_allowed_ports(cfg.test_ports.clone());
    let state = ServerState::new(ctx, csp, Some(lock), cfg.policy.clone(), ratelimit);
    if state.needs_setup_token() {
        state.ensure_setup_token();
    }
    Ok(state)
}

/// Abstand des Log-Hinweises „Instanz gesperrt“ (D-19).
const LOCKED_LOG_EVERY_MINUTES: u64 = 6 * 60;

/// Gesperrter Zustand sichtbar im Container-Log (D-19, E3: nach einem Neustart
/// läuft nichts, bis im Browser entsperrt wird).
fn log_if_locked(state: &ServerState) {
    if state.instance_state() == state::InstanceState::Locked {
        tracing::info!(
            "Instanz gesperrt — Backups, Abo-Läufe und Prüfungen ruhen bis zur Anmeldung im Browser"
        );
    }
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
