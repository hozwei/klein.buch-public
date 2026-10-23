//! Lastgrenzen im Server (Plan W5.8).
//!
//! - global höchstens [`MAX_CONCURRENT_REQUESTS`] Anfragen gleichzeitig, sonst 503
//! - Zulassung je Kostenklasse (render 2, jvm 2, kdf 1, backup/export 1);
//!   eigene Semaphoren, getrennt von denen in [`crate::limits`], die die Arbeit
//!   selbst begrenzen (sonst könnte ein Aufruf auf seine eigene Genehmigung
//!   warten)
//! - Timeout [`REQUEST_TIMEOUT`] auf `/api/*`; Längeres läuft als Job

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::errors::ApiError;
use super::state::ServerState;
use crate::commands::registry::Cost;

pub const MAX_CONCURRENT_REQUESTS: usize = 64;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// So lange wartet ein direkter Aufruf auf eine Genehmigung seiner
/// Kostenklasse, bevor er mit 503 abgelehnt wird.
pub const ADMISSION_WAIT: Duration = Duration::from_secs(5);
/// Höchstens so lange wartet die Anmeldung auf die KDF (Plan W6).
pub const KDF_WAIT: Duration = Duration::from_secs(10);

/// Plätze je Kostenklasse (Plan W5.8).
pub const RENDER_SLOTS: usize = 2;
pub const JVM_SLOTS: usize = 2;
pub const KDF_SLOTS: usize = 1;
pub const BACKUP_SLOTS: usize = 1;

/// Semaphoren je Kostenklasse und global.
pub struct Admission {
    render: Arc<Semaphore>,
    jvm: Arc<Semaphore>,
    kdf: Arc<Semaphore>,
    backup: Arc<Semaphore>,
    global: Arc<Semaphore>,
    /// Laufende Kerne je Klasse und deren Höchststand (Nachweis in Tests).
    running: [AtomicUsize; 5],
    peak: [AtomicUsize; 5],
}

/// Siehe [`Admission::running`].
pub struct RunningGuard<'a> {
    admission: &'a Admission,
    i: usize,
}

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.admission.running[self.i].fetch_sub(1, Ordering::SeqCst);
    }
}

impl Default for Admission {
    fn default() -> Self {
        Self {
            render: Arc::new(Semaphore::new(RENDER_SLOTS)),
            jvm: Arc::new(Semaphore::new(JVM_SLOTS)),
            kdf: Arc::new(Semaphore::new(KDF_SLOTS)),
            backup: Arc::new(Semaphore::new(BACKUP_SLOTS)),
            global: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            running: Default::default(),
            peak: Default::default(),
        }
    }
}

fn index(cost: Cost) -> usize {
    match cost {
        Cost::Cheap => 0,
        Cost::Render => 1,
        Cost::Jvm => 2,
        Cost::Kdf => 3,
        Cost::Backup => 4,
    }
}

impl Admission {
    fn semaphore(&self, cost: Cost) -> Option<&Arc<Semaphore>> {
        match cost {
            Cost::Cheap => None,
            Cost::Render => Some(&self.render),
            Cost::Jvm => Some(&self.jvm),
            Cost::Kdf => Some(&self.kdf),
            Cost::Backup => Some(&self.backup),
        }
    }

    /// Genehmigung für einen direkten Aufruf: höchstens `wait` warten, sonst
    /// 503 + `Retry-After`.
    pub async fn admit(
        &self,
        cost: Cost,
        wait: Duration,
    ) -> Result<Option<OwnedSemaphorePermit>, ApiError> {
        let Some(sem) = self.semaphore(cost) else {
            return Ok(None);
        };
        match tokio::time::timeout(wait, sem.clone().acquire_owned()).await {
            Ok(Ok(permit)) => Ok(Some(permit)),
            Ok(Err(_)) | Err(_) => Err(ApiError::busy(5)),
        }
    }

    /// Markiert einen laufenden Kern seiner Klasse (für den Nachweis der
    /// Grenze, Plan W5 Done-Check); zählt beim Drop wieder herunter.
    pub fn running(&self, cost: Cost) -> RunningGuard<'_> {
        let i = index(cost);
        let now = self.running[i].fetch_add(1, Ordering::SeqCst) + 1;
        self.peak[i].fetch_max(now, Ordering::SeqCst);
        RunningGuard { admission: self, i }
    }

    /// Höchststand gleichzeitig laufender Kerne einer Klasse.
    pub fn peak(&self, cost: Cost) -> usize {
        self.peak[index(cost)].load(Ordering::SeqCst)
    }

    /// Genehmigung für einen Job: wartet, bis die Klasse frei ist (die
    /// Warteschlange ist über die Job-Zahl begrenzt).
    pub async fn acquire(&self, cost: Cost) -> Option<OwnedSemaphorePermit> {
        let sem = self.semaphore(cost)?;
        sem.clone().acquire_owned().await.ok()
    }

    /// Aktuell freie Plätze einer Klasse (Tests).
    pub fn available(&self, cost: Cost) -> usize {
        self.semaphore(cost)
            .map(|s| s.available_permits())
            .unwrap_or(usize::MAX)
    }
}

/// Globale Grenze gleichzeitiger Anfragen: kein Warten, sofort 503.
pub async fn global_limit(
    State(st): State<Arc<ServerState>>,
    req: Request,
    next: Next,
) -> Response {
    // Der Healthcheck bleibt auch unter Last erreichbar (503 nur beim
    // Herunterfahren, Plan W5.10).
    if req.uri().path() == "/healthz" {
        return next.run(req).await;
    }
    match st.admission.global.clone().try_acquire_owned() {
        Ok(permit) => {
            let response = next.run(req).await;
            drop(permit);
            response
        }
        Err(_) => ApiError::busy(1).into_response(),
    }
}

/// Timeout für `/api/*` (Plan W5.8). Die Arbeit selbst läuft abgekoppelt
/// weiter (`dispatch`), nur die Antwort entfällt.
pub async fn request_timeout(req: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(req)).await {
        Ok(response) => response,
        Err(_) => ApiError::timeout().into_response(),
    }
}
