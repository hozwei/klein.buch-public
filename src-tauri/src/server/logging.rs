//! Logging ohne Geheimnisse (Plan W5.9).
//!
//! sqlx führt `PRAGMA key = '<Passphrase>'` als normales Statement aus und
//! loggt Statements auf DEBUG bzw. langsame auf WARN. `db::open_pool` schaltet
//! das Statement-Logging ab; zusätzlich hängt [`filter`] fest
//! `sqlx::query=off` an, das auch ein gesetztes `RUST_LOG` nicht aufhebt.
//! Das Zugriffsprotokoll nennt nur Methode, Pfad ohne Query, Status, Dauer.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing_subscriber::EnvFilter;

/// Standard-Filter ohne `RUST_LOG`.
pub const DEFAULT_FILTER: &str = "info";
/// Wird immer angehängt (überschreibt eine gleichnamige Direktive aus `RUST_LOG`).
pub const SQLX_QUERY_OFF: &str = "sqlx::query=off";

/// Filter aus `RUST_LOG` (falls gültig) bzw. [`DEFAULT_FILTER`], immer mit
/// [`SQLX_QUERY_OFF`].
pub fn filter(rust_log: Option<&str>) -> EnvFilter {
    let base = rust_log
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_FILTER));
    match SQLX_QUERY_OFF.parse() {
        Ok(directive) => base.add_directive(directive),
        // statisch gültig; ohne Direktive bleibt das Statement-Logging in
        // `db::open_pool` trotzdem aus
        Err(_) => base,
    }
}

/// Initialisiert den globalen Subscriber (einmal pro Prozess).
pub fn init() {
    let rust_log = std::env::var("RUST_LOG").ok();
    // Farbcodes nur im Terminal, nicht im Container-Log.
    let ansi = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter(rust_log.as_deref()))
        .with_ansi(ansi)
        .try_init();
}

/// Zugriffsprotokoll: Methode, Pfad ohne Query, Status, Dauer.
pub async fn access_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let started = std::time::Instant::now();
    let response = next.run(req).await;
    tracing::info!(
        target: "klein_buch_lib::http",
        "{} {} {} {} ms",
        method,
        path,
        response.status().as_u16(),
        started.elapsed().as_millis()
    );
    response
}
