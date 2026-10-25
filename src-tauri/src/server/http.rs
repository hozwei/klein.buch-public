//! Annahme-Schleife (Plan W5.8): hyper HTTP/1.1 mit Header-Read-Timeout
//! [`HEADER_READ_TIMEOUT`] (Slowloris), Peer-Adresse als `ConnectInfo`,
//! geordnetes Herunterfahren über einen `CancellationToken`.
//!
//! hyper misst den Header-Timeout auch in der Leerlaufzeit einer
//! Keep-Alive-Verbindung zwischen zwei Anfragen; eine ruhende Verbindung
//! schließt damit nach 10 s (strenger als die im Plan genannten 60 s,
//! ADR 0039).
//!
//! Lauf F4b (W12-LEGAL-TIMEOUTS): höchstens [`MAX_CONNECTIONS`] offene
//! Verbindungen — ist die Grenze erreicht, nimmt die Schleife erst wieder an,
//! wenn eine Verbindung endet (die übrigen warten im Backlog des Kernels).
//! Jede Verbindung schließt, wenn ein Schreibvorgang [`WRITE_IDLE_TIMEOUT`]
//! lang nicht vorankommt (Gegenstelle liest nicht mehr). Lesen ist schon
//! begrenzt: Header über [`HEADER_READ_TIMEOUT`], JSON-Körper über
//! `dispatch::BODY_READ_TIMEOUT`, Uploads über den Leerlauf-Timeout je
//! Datenblock.
//!
//! Lauf F4d: Zusätzlich höchstens [`MAX_CONNECTIONS_PER_PEER`] Verbindungen je
//! Gegenstelle (IPv4-Adresse bzw. IPv6-/64). Eine Gegenstelle allein kann die
//! globale Grenze damit nicht ausschöpfen (langsame Leser, die alle ~25 s ein
//! paar Bytes abholen und so den Schreib-Leerlauf immer wieder zurücksetzen).
//! Weitere Verbindungen derselben Gegenstelle werden angenommen und sofort
//! geschlossen. Vertraute Proxys (`TRUSTED_PROXIES`) zählen nur gegen die
//! globale Grenze: Hinter ihnen teilen sich alle Clients eine Peer-Adresse.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use ipnet::IpNet;

use axum::extract::ConnectInfo;
use axum::Router;
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::time::Sleep;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt as _;

pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Höchstens so lange warten offene Verbindungen beim Herunterfahren.
pub const CONNECTION_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
/// Obergrenze gleichzeitig offener Verbindungen (Lauf F4b). Ein Inhaber mit
/// wenigen Browser-Tabs braucht nur einige; die Grenze schützt Dateideskriptoren
/// und Speicher vor vielen ruhenden oder langsamen Gegenstellen.
pub const MAX_CONNECTIONS: usize = 256;
/// Schreib-Leerlauf (Lauf F4b): kommt ein Schreibvorgang so lange nicht voran,
/// endet die Verbindung.
pub const WRITE_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Obergrenze offener Verbindungen je Gegenstelle (Lauf F4d): ein Browser
/// öffnet je Host höchstens 6, ein Reverse-Proxy ohne `TRUSTED_PROXIES` für
/// einen Inhaber nur wenige mehr.
pub const MAX_CONNECTIONS_PER_PEER: usize = 32;

/// Grenzen der Annahme-Schleife (in Tests verkleinert).
#[derive(Debug, Clone)]
pub struct ConnectionLimits {
    pub max_connections: usize,
    pub write_idle_timeout: Duration,
    /// Höchstens so viele Verbindungen je Gegenstelle (IPv4 bzw. IPv6-/64).
    pub max_per_peer: usize,
    /// Gegenstellen ohne eigene Grenze (vertraute Proxys, `TRUSTED_PROXIES`):
    /// nur die globale Grenze gilt.
    pub unlimited_peers: Vec<IpNet>,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            max_connections: MAX_CONNECTIONS,
            write_idle_timeout: WRITE_IDLE_TIMEOUT,
            max_per_peer: MAX_CONNECTIONS_PER_PEER,
            unlimited_peers: Vec::new(),
        }
    }
}

/// Schlüssel der Grenze je Gegenstelle: IPv4-Adresse (auch IPv4-mapped),
/// IPv6 zusammengefasst je /64 (wie das Rate-Limit der Anmeldung).
fn peer_key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::V6(std::net::Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0))
        }
        v4 => v4,
    }
}

type PeerCounts = Arc<Mutex<HashMap<IpAddr, usize>>>;

/// Belegter Platz einer Gegenstelle; gibt ihn beim Drop zurück.
struct PeerSlot {
    counts: PeerCounts,
    key: IpAddr,
}

impl PeerSlot {
    fn try_acquire(counts: &PeerCounts, key: IpAddr, max: usize) -> Option<Self> {
        let mut map = counts.lock().unwrap_or_else(PoisonError::into_inner);
        let n = map.entry(key).or_insert(0);
        if *n >= max.max(1) {
            return None;
        }
        *n += 1;
        Some(Self {
            counts: counts.clone(),
            key,
        })
    }
}

impl Drop for PeerSlot {
    fn drop(&mut self) {
        let mut map = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(n) = map.get_mut(&self.key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(&self.key);
            }
        }
    }
}

/// Bedient `listener`, bis `shutdown` ausgelöst wird; wartet dann (begrenzt)
/// auf offene Verbindungen.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    serve_with(listener, app, shutdown, ConnectionLimits::default()).await
}

/// Wie [`serve`], mit eigenen Grenzen.
pub async fn serve_with(
    listener: TcpListener,
    app: Router,
    shutdown: CancellationToken,
    limits: ConnectionLimits,
) -> std::io::Result<()> {
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let slots = Arc::new(Semaphore::new(limits.max_connections.max(1)));
    let per_peer: PeerCounts = Arc::new(Mutex::new(HashMap::new()));
    // Abgewiesene Verbindungen höchstens einmal je Minute im Log (kein
    // Log-Fluten durch den Angreifer).
    let mut last_refusal_log: Option<Instant> = None;
    loop {
        // Erst ein freier Platz, dann `accept` — bei voller Grenze bleiben neue
        // Verbindungen im Backlog, statt angenommen und ruhend gehalten zu werden.
        let permit = tokio::select! {
            permit = slots.clone().acquire_owned() => match permit {
                Ok(p) => p,
                Err(_) => break,
            },
            _ = shutdown.cancelled() => break,
        };
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
        let peer_slot = if limits
            .unlimited_peers
            .iter()
            .any(|net| net.contains(&peer.ip().to_canonical()))
        {
            None
        } else {
            match PeerSlot::try_acquire(&per_peer, peer_key(peer.ip()), limits.max_per_peer) {
                Some(slot) => Some(slot),
                None => {
                    if last_refusal_log.is_none_or(|t| t.elapsed() >= Duration::from_secs(60)) {
                        last_refusal_log = Some(Instant::now());
                        tracing::warn!(
                            "Verbindung von {} abgewiesen: mehr als {} offene Verbindungen dieser \
                             Gegenstelle",
                            peer.ip(),
                            limits.max_per_peer
                        );
                    }
                    drop(stream);
                    continue;
                }
            }
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
        let io = WriteIdleTimeout::new(stream, limits.write_idle_timeout);
        let conn = builder.serve_connection(TokioIo::new(io), service);
        let watched = graceful.watch(conn);
        tokio::spawn(async move {
            let _slot = permit;
            let _peer_slot = peer_slot;
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

/// Strom mit Schreib-Leerlauf-Timeout: Bleibt ein Schreibvorgang (`write`,
/// `flush`, `shutdown`) länger als `timeout` ohne Fortschritt hängen, liefert er
/// `TimedOut` — hyper beendet daraufhin die Verbindung. Jeder Fortschritt
/// setzt die Frist zurück. Lesen reicht der Strom unverändert durch.
pub struct WriteIdleTimeout<S> {
    inner: S,
    timeout: Duration,
    deadline: Option<Pin<Box<Sleep>>>,
}

impl<S> WriteIdleTimeout<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            deadline: None,
        }
    }

    /// Wertet das Ergebnis eines Schreibschritts aus: Fortschritt löscht die
    /// Frist, `Pending` startet sie bzw. prüft, ob sie abgelaufen ist.
    fn track<T>(&mut self, cx: &mut Context<'_>, poll: Poll<io::Result<T>>) -> Poll<io::Result<T>> {
        match poll {
            Poll::Ready(result) => {
                self.deadline = None;
                Poll::Ready(result)
            }
            Poll::Pending => {
                let timeout = self.timeout;
                let deadline = self
                    .deadline
                    .get_or_insert_with(|| Box::pin(tokio::time::sleep(timeout)));
                match deadline.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        self.deadline = None;
                        Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Gegenstelle nimmt keine Daten mehr an",
                        )))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for WriteIdleTimeout<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for WriteIdleTimeout<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let poll = Pin::new(&mut self.inner).poll_write(cx, buf);
        self.track(cx, poll)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let poll = Pin::new(&mut self.inner).poll_write_vectored(cx, bufs);
        self.track(cx, poll)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let poll = Pin::new(&mut self.inner).poll_flush(cx);
        self.track(cx, poll)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let poll = Pin::new(&mut self.inner).poll_shutdown(cx);
        self.track(cx, poll)
    }
}
