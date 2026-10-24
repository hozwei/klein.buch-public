//! `GET /healthz` und Subcommand `healthcheck` (Plan W5.10).
//!
//! Nach außen nur 200/503 ohne Körper; über Loopback (und damit über den
//! Subcommand, Docker-`HEALTHCHECK`) mit `{"state":"setup|locked|unlocked"}`.
//! `locked` gilt als gesund — sonst entstünde nach jedem Neustart eine
//! Neustartschleife, weil erst der Browser entsperrt (E3). 503 nur, solange
//! der Server herunterfährt bzw. für den Restore neu startet.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::config::{healthcheck_addr, listen_from_env_with};
use super::state::ServerState;

pub async fn healthz(State(st): State<Arc<ServerState>>, req: Request) -> Response {
    let status = if st.is_stopping() {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    let loopback = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|c| c.0.ip().to_canonical().is_loopback());
    if loopback {
        // Plan W7: Off-Site-Status (missing | failing | ok), nur entsperrt
        // lesbar (sonst null).
        let offsite = match st.ctx.pool() {
            Ok(pool) => {
                crate::backup::target::offsite_status(&pool, &st.ctx.paths.backup_targets_dir)
                    .await
                    .ok()
            }
            Err(_) => None,
        };
        (
            status,
            Json(serde_json::json!({
                "state": st.instance_state().as_str(),
                "offsite": offsite,
            })),
        )
            .into_response()
    } else {
        status.into_response()
    }
}

/// Subcommand `healthcheck`: fragt `/healthz` über Loopback ab, gibt den
/// Körper aus; Exit 0 bei 200.
pub fn healthcheck_command() -> ExitCode {
    // Nur die Adresse — der Healthcheck braucht keine Policy-Variablen.
    let listen = match listen_from_env_with(&|name| std::env::var_os(name)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Host-Header wie die öffentliche Adresse: über Loopback ist /healthz
    // ohnehin frei, bei fest gebundener IP prüft der Server den Host (W6).
    let host = std::env::var(super::config::ENV_PUBLIC_URL)
        .ok()
        .and_then(|u| super::policy::PublicUrl::parse(&u).ok())
        .map(|u| match u.port {
            Some(p) => format!("{}:{p}", u.host),
            None => u.host,
        });
    match runtime.block_on(probe_with_host(healthcheck_addr(listen), host)) {
        Ok((200, body)) => {
            println!("{body}");
            ExitCode::SUCCESS
        }
        Ok((status, body)) => {
            eprintln!("HTTP {status} {body}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("nicht erreichbar: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Minimaler HTTP/1.1-Client für `/healthz` (Status, Körper).
pub async fn probe(addr: SocketAddr) -> std::io::Result<(u16, String)> {
    probe_with_host(addr, None).await
}

/// Wie [`probe`], mit eigenem `Host`-Header.
pub async fn probe_with_host(
    addr: SocketAddr,
    host: Option<String>,
) -> std::io::Result<(u16, String)> {
    let host = host.unwrap_or_else(|| addr.to_string());
    let fut = async {
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        let request = format!(
            "GET /healthz HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: klein-buch-healthcheck\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await?;
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| std::io::Error::other("keine HTTP-Antwort"))?;
        let body = text
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.trim().to_string())
            .unwrap_or_default();
        Ok((status, body))
    };
    tokio::time::timeout(Duration::from_secs(5), fut)
        .await
        .map_err(|_| std::io::Error::other("Zeitüberschreitung"))?
}
