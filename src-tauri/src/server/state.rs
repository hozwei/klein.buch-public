//! Gemeinsamer Server-Zustand und Zustandsautomat der Instanz (Plan §3, W5.4).
//!
//! `setup` (keine DB) → `locked` (DB vorhanden, kein Pool, kein Scheduler) ↔
//! `unlocked` (Pool offen). Browser-Sitzungen sind davon getrennt; im W5-Stand
//! entsteht eine Sitzung nur zusammen mit dem Entsperren bzw. Einrichten.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::headers::Csp;
use super::idempotency::IdempotencyStore;
use super::jobs::JobRegistry;
use super::limits::Admission;
use super::session::SessionStore;
use crate::app_ctx::AppCtx;

/// Zustand der Instanz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    Setup,
    Locked,
    Unlocked,
}

impl InstanceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Locked => "locked",
            Self::Unlocked => "unlocked",
        }
    }
}

/// Wie lange der Server nach einem erfolgreichen Restore noch antwortet, damit
/// der Browser das Job-Ergebnis abholen kann, bevor er neu startet.
pub const RESTART_GRACE: Duration = Duration::from_secs(10);
/// Höchstens so lange wartet das Herunterfahren auf laufende Jobs.
pub const JOB_DRAIN_TIMEOUT: Duration = Duration::from_secs(600);

pub struct ServerState {
    pub ctx: Arc<AppCtx>,
    pub sessions: SessionStore,
    pub jobs: JobRegistry,
    pub idem: IdempotencyStore,
    pub admission: Admission,
    pub csp: Csp,
    /// Serialisiert Einrichten und Anmelden (Zustandswechsel).
    pub transition: tokio::sync::Mutex<()>,
    /// Löst das geordnete Herunterfahren aus (Signal oder Restore).
    pub shutdown: CancellationToken,
    /// Abgekoppelte Tasks (Commands, Jobs, Anmeldung); das Herunterfahren
    /// wartet auf sie.
    pub tasks: TaskTracker,
    restart_pending: AtomicBool,
    /// Ein Restore-Job läuft: keine Änderungen, Scheduler pausiert.
    restore_running: AtomicBool,
    /// Exklusive Sperre auf `/data/.klein-buch.lock` (ein Prozess je Instanz).
    _instance_lock: Option<std::fs::File>,
}

impl ServerState {
    pub fn new(ctx: Arc<AppCtx>, csp: Csp, instance_lock: Option<std::fs::File>) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            sessions: SessionStore::default(),
            jobs: JobRegistry::default(),
            idem: IdempotencyStore::default(),
            admission: Admission::default(),
            csp,
            transition: tokio::sync::Mutex::new(()),
            shutdown: CancellationToken::new(),
            tasks: TaskTracker::new(),
            restart_pending: AtomicBool::new(false),
            restore_running: AtomicBool::new(false),
            _instance_lock: instance_lock,
        })
    }

    pub fn instance_state(&self) -> InstanceState {
        if self.ctx.is_unlocked() {
            InstanceState::Unlocked
        } else if self.ctx.paths.db_file.exists() {
            InstanceState::Locked
        } else {
            InstanceState::Setup
        }
    }

    /// Neustart vorgemerkt (nach Restore) oder Herunterfahren läuft.
    pub fn is_stopping(&self) -> bool {
        self.restart_pending.load(Ordering::SeqCst) || self.shutdown.is_cancelled()
    }

    /// Keine Änderungen annehmen: Restore läuft, Neustart vorgemerkt oder
    /// Herunterfahren.
    pub fn writes_blocked(&self) -> bool {
        self.restore_running.load(Ordering::SeqCst) || self.is_stopping()
    }

    /// Restore beginnt: ab sofort keine Änderungen mehr annehmen (sie gingen
    /// beim Tausch verloren). Der Job pausiert danach den Scheduler.
    pub fn begin_restore(&self) {
        self.restore_running.store(true, Ordering::SeqCst);
    }

    /// Restore gescheitert: Sperre aufheben, Scheduler fortsetzen.
    pub async fn abort_restore(&self) {
        crate::scheduler::tick::resume(&self.ctx).await;
        self.restore_running.store(false, Ordering::SeqCst);
    }

    /// Nach einem erfolgreichen Restore: neue Änderungen ablehnen, kurz
    /// warten (Ergebnis abholbar), dann geordnet herunterfahren. Der Neustart
    /// durch die Container-Verwaltung wendet den Restore vor dem Port-Bind an.
    pub fn request_restart(self: &Arc<Self>) {
        if self.restart_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::info!("Restore vorgemerkt — Server fährt für den Neustart herunter");
        let st = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(RESTART_GRACE).await;
            st.shutdown.cancel();
        });
    }

    /// Nach dem Ende der Annahme-Schleife: abgekoppelte Tasks und Jobs
    /// abwarten, Scheduler-Tick abwarten und anhalten, Pool schließen
    /// (WAL-Checkpoint).
    pub async fn finish_shutdown(&self) {
        self.tasks.close();
        if tokio::time::timeout(JOB_DRAIN_TIMEOUT, self.tasks.wait())
            .await
            .is_err()
        {
            tracing::warn!(
                "Herunterfahren: {} Task(s), davon {} Job(s), nach {} s noch aktiv",
                self.tasks.len(),
                self.jobs.active(),
                JOB_DRAIN_TIMEOUT.as_secs()
            );
        }
        crate::scheduler::tick::stop(&self.ctx).await;
        if let Some(pool) = self.ctx.take_pool() {
            pool.close().await;
        }
        self.ctx.session.clear();
        self.sessions.clear();
    }
}
