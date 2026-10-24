//! Annahme-Schleife (Plan W5.8): hyper HTTP/1.1 mit Header-Read-Timeout
//! [`HEADER_READ_TIMEOUT`] (Slowloris), Peer-Adresse als `ConnectInfo`,
//! geordnetes Herunterfahren über einen `CancellationToken`.
//!
//! hyper misst den Header-Timeout auch in der Leerlaufzeit einer
//! Keep-Alive-Verbindung zwischen zwei Anfragen; eine ruhende Verbindung
//! schließt damit nach 10 s (strenger als die im Plan genannten 60 s,
//! ADR 0039).

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::ConnectInfo;
use axum::Router;
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt as _;

pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Höchstens so lange warten offene Verbindungen beim Herunterfahren.
pub const CONNECTION_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// Bedient `listener`, bis `shutdown` ausgelöst wird; wartet dann (begrenzt)
/// auf offene Verbindungen.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    loop {
        let (stream, peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    // z. B. EMFILE: kurz durchatmen statt Busy-Loop
                    tracing::warn!("accept fehlgeschlagen: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            },
            _ = shutdown.cancelled() => break,
        };
        let app = app.clone();
        let service = hyper::service::service_fn(move |mut req: hyper::Request<Incoming>| {
            req.extensions_mut().insert(ConnectInfo::<SocketAddr>(peer));
            app.clone().oneshot(req.map(axum::body::Body::new))
        });
        let mut builder = hyper::server::conn::http1::Builder::new();
        builder
            .timer(TokioTimer::new())
            .header_read_timeout(HEADER_READ_TIMEOUT)
            .keep_alive(true);
        let conn = builder.serve_connection(TokioIo::new(stream), service);
        let watched = graceful.watch(conn);
        tokio::spawn(async move {
            if let Err(e) = watched.await {
                tracing::debug!("Verbindung {peer}: {e}");
            }
        });
    }
    drop(listener);
    if tokio::time::timeout(CONNECTION_DRAIN_TIMEOUT, graceful.shutdown())
        .await
        .is_err()
    {
        tracing::warn!("Herunterfahren: offene Verbindungen nach 30 s getrennt");
    }
    Ok(())
}
