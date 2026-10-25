//! Anwendungskontext (Plan W4): Zustand der Command-Kerne (`crate::commands`)
//! — früher `State`/`AppHandle` der Desktop-Shell. Der HTTP-Server
//! (`crate::server`, W5) hält genau eine Instanz in einem `Arc`.

use crate::backup::BackupSession;
use crate::clock::{Clock, SystemClock};
use crate::config::Paths;
use crate::db::PendingRestoreAudit;
use crate::error::{Error, Result};
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

/// Desktop-Nebenwirkungen (Datei öffnen, im Ordner zeigen, Browser, Neustart).
/// Im Server gibt es sie nicht ([`NoDesktop`]); die betroffenen Commands sind
/// nicht freigegeben (W5) oder liefern ab W7 Bytes. Tests zeichnen Aufrufe auf.
pub trait DesktopActions: Send + Sync {
    fn open_path(&self, path: &Path) -> Result<()>;
    fn reveal(&self, path: &Path) -> Result<()>;
    fn open_url(&self, url: &str) -> Result<()>;
    fn restart(&self) -> Result<()>;
}

/// Keine Desktop-Nebenwirkungen (Server, Tests).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoDesktop;

impl NoDesktop {
    fn unavailable() -> Error {
        Error::Domain("Im Server-Betrieb nicht verfügbar.".into())
    }
}

impl DesktopActions for NoDesktop {
    fn open_path(&self, _path: &Path) -> Result<()> {
        Err(Self::unavailable())
    }
    fn reveal(&self, _path: &Path) -> Result<()> {
        Err(Self::unavailable())
    }
    fn open_url(&self, _url: &str) -> Result<()> {
        Err(Self::unavailable())
    }
    fn restart(&self) -> Result<()> {
        Err(Self::unavailable())
    }
}

pub struct AppCtx {
    /// Verzeichnisse (Daten, Archiv, Backups, Inputs, Sidecar, Ressourcen, Web).
    pub paths: Paths,
    /// DB-Pool — `None`, solange die Instanz gesperrt ist (Plan §3).
    pool: RwLock<Option<SqlitePool>>,
    /// Passphrase der laufenden Sitzung (nur im RAM, ADR 0035).
    pub session: BackupSession,
    /// Beim Serverstart angewendeter Restore, dessen Audit nach dem Entsperren folgt.
    pub pending_restore: PendingRestoreAudit,
    /// Verbindungstests: Kontingent und SFTP-Probe-Speicher (W8). Zugangsdaten
    /// selbst liegen in der DB (`crate::secrets`).
    pub net_tests: crate::nettest::NetTests,
    /// Desktop-Nebenwirkungen (im Server [`NoDesktop`]).
    pub desktop: Arc<dyn DesktopActions>,
    /// Einmal-Guard für den Scheduler (genau ein Start pro Prozess).
    scheduler_started: AtomicBool,
    /// Wird während eines Scheduler-Ticks gehalten; `scheduler::tick::stop`
    /// und `stop_within` warten darüber auf das Ende des laufenden Ticks.
    pub scheduler_gate: tokio::sync::Mutex<()>,
    /// Scheduler angehalten (Restore, „Datenbank sperren“, Passphrase-Wechsel,
    /// Herunterfahren). Der Tick prüft es vor jedem Lauf; `tick::resume`
    /// setzt es ohne die Sperre zurück — ein noch laufender alter Tick hält das
    /// Entsperren nicht auf (F1d).
    pub scheduler_paused: AtomicBool,
    /// Kooperativer Abbruch (W14-027, ADR 0040): gesetzt von
    /// `scheduler::tick::request_stop`; laufende Abo-Läufe beenden den
    /// gerade bearbeiteten Termin und hören dann auf.
    pub scheduler_stop: AtomicBool,
    /// Zeitpunkt des letzten Entsperrens (D-19): der Tick wartet danach
    /// `scheduler::tick::UNLOCK_SETTLE`, bevor er fällige Jobs abarbeitet.
    unlocked_at: Mutex<Option<std::time::Instant>>,
    /// Erster Tick nach einem Entsperren steht aus (D-19: ohne Vollscan).
    fresh_unlock: AtomicBool,
    /// Uhr für zeitabhängige Abläufe (D-20, ADR 0040); in Tests austauschbar.
    clock: RwLock<Arc<dyn Clock>>,
    /// Kennung der Anwendung für die Info-Anzeige (z. B. `de.wildbach.kleinbuch`).
    pub app_identifier: String,
    /// Exporte und Vorschauen zum Abruf über `/files/download/{id}` (W7).
    pub downloads: crate::downloads::DownloadStore,
    /// Uploads aus `POST /api/upload/{art}` bis zum Verbrauch (W7).
    pub uploads: crate::uploads::UploadStore,
    /// Hinweise aus Prüfungen im gesperrten Zustand (Serverstart), beim nächsten
    /// Entsperren in die Inbox (Lauf F4b, R3-014).
    pub deferred_notices: crate::notify::deferred::DeferredNotices,
}

impl AppCtx {
    pub fn new(
        paths: Paths,
        desktop: Arc<dyn DesktopActions>,
        app_identifier: impl Into<String>,
    ) -> Self {
        Self {
            downloads: crate::downloads::DownloadStore::new(&paths.data_dir),
            uploads: crate::uploads::UploadStore::new(&paths.data_dir),
            deferred_notices: Default::default(),
            paths,
            pool: RwLock::new(None),
            session: BackupSession::default(),
            pending_restore: PendingRestoreAudit::default(),
            net_tests: crate::nettest::NetTests::default(),
            desktop,
            scheduler_started: AtomicBool::new(false),
            scheduler_gate: tokio::sync::Mutex::new(()),
            scheduler_paused: AtomicBool::new(false),
            scheduler_stop: AtomicBool::new(false),
            unlocked_at: Mutex::new(None),
            fresh_unlock: AtomicBool::new(false),
            clock: RwLock::new(Arc::new(SystemClock)),
            app_identifier: app_identifier.into(),
        }
    }

    /// Offener DB-Pool oder [`Error::Locked`], solange nicht entsperrt.
    pub fn pool(&self) -> Result<SqlitePool> {
        self.pool
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(Error::Locked)
    }

    /// Hinterlegt den Pool nach erfolgreichem Entsperren/Einrichten.
    pub fn set_pool(&self, pool: SqlitePool) {
        *self.pool.write().unwrap_or_else(PoisonError::into_inner) = Some(pool);
    }

    /// Nimmt den Pool heraus (Datenbank sperren, W6).
    pub fn take_pool(&self) -> Option<SqlitePool> {
        self.pool
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    pub fn is_unlocked(&self) -> bool {
        self.pool
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Uhr der Anwendung (im Betrieb die Systemuhr).
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Ersetzt die Uhr (Tests: `clock::FixedClock`).
    pub fn set_clock(&self, clock: Arc<dyn Clock>) {
        *self.clock.write().unwrap_or_else(PoisonError::into_inner) = clock;
    }

    pub fn open_path(&self, path: impl AsRef<Path>) -> Result<()> {
        self.desktop.open_path(path.as_ref())
    }

    pub fn reveal(&self, path: impl AsRef<Path>) -> Result<()> {
        self.desktop.reveal(path.as_ref())
    }

    pub fn open_url(&self, url: &str) -> Result<()> {
        self.desktop.open_url(url)
    }

    /// Vermerkt ein Entsperren bzw. die Einrichtung (D-19).
    pub fn mark_unlocked(&self) {
        *self
            .unlocked_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(std::time::Instant::now());
        self.fresh_unlock.store(true, Ordering::SeqCst);
    }

    /// Restliche Wartezeit bis `settle` nach dem letzten Entsperren (`None` =
    /// keine Wartezeit mehr).
    pub fn unlock_settle_remaining(
        &self,
        settle: std::time::Duration,
    ) -> Option<std::time::Duration> {
        let at = *self
            .unlocked_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        at.and_then(|t| settle.checked_sub(t.elapsed()))
            .filter(|d| !d.is_zero())
    }

    /// `true` genau einmal je Entsperren (erster Tick danach).
    pub fn take_fresh_unlock(&self) -> bool {
        self.fresh_unlock.swap(false, Ordering::SeqCst)
    }

    /// `true` genau für den ersten Aufruf (Scheduler-Start).
    pub fn begin_scheduler(&self) -> bool {
        !self.scheduler_started.swap(true, Ordering::SeqCst)
    }

    /// Kontext für Tests: Pfade unter `dir`, Zugangsdaten nur im RAM.
    #[doc(hidden)]
    pub fn for_tests(paths: Paths) -> Arc<Self> {
        Arc::new(Self::new(paths, Arc::new(NoDesktop), "test"))
    }
}
