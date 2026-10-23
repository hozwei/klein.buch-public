//! Anwendungskontext (Plan W4): ersetzt Tauri-`State`/`AppHandle` in den
//! Command-Kernen (`crate::commands`). Die Tauri-Hülle (bis W5) und der
//! HTTP-Server (ab W5) halten genau eine Instanz in einem `Arc`.

use crate::backup::BackupSession;
use crate::config::Paths;
use crate::db::PendingRestoreAudit;
use crate::error::{Error, Result};
use crate::notify::Notifier;
use crate::secrets::SecretStore;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

/// Desktop-Nebenwirkungen (Datei öffnen, im Ordner zeigen, Browser, Neustart).
/// Die Tauri-Hülle setzt sie um; im Server gibt es sie nicht ([`NoDesktop`]) —
/// die betroffenen Commands entfallen dort oder liefern Bytes (W5/W7).
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
    /// Beim Start angewendeter Restore, dessen Audit nach dem Entsperren folgt.
    pub pending_restore: PendingRestoreAudit,
    /// OS-Benachrichtigungen (Desktop) bzw. keine (Server, Tests).
    pub notifier: Arc<dyn Notifier>,
    /// Ablage für Zugangsdaten (bis W8 Keychain, danach SQLCipher-Tabelle).
    pub secrets: Arc<dyn SecretStore>,
    /// Desktop-Nebenwirkungen (nur Tauri-Hülle).
    pub desktop: Arc<dyn DesktopActions>,
    /// Einmal-Guard für den Scheduler (genau ein Start pro Prozess).
    scheduler_started: AtomicBool,
    /// Kennung der Anwendung für die Info-Anzeige (z. B. `de.wildbach.kleinbuch`).
    pub app_identifier: String,
}

impl AppCtx {
    pub fn new(
        paths: Paths,
        notifier: Arc<dyn Notifier>,
        secrets: Arc<dyn SecretStore>,
        desktop: Arc<dyn DesktopActions>,
        app_identifier: impl Into<String>,
    ) -> Self {
        Self {
            paths,
            pool: RwLock::new(None),
            session: BackupSession::default(),
            pending_restore: PendingRestoreAudit::default(),
            notifier,
            secrets,
            desktop,
            scheduler_started: AtomicBool::new(false),
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

    /// Notifier für die `notify`-/Scheduler-Funktionen.
    pub fn notifier(&self) -> Option<&dyn Notifier> {
        Some(self.notifier.as_ref())
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

    /// `true` genau für den ersten Aufruf (Scheduler-Start).
    pub fn begin_scheduler(&self) -> bool {
        !self.scheduler_started.swap(true, Ordering::SeqCst)
    }

    /// Kontext für Tests: Pfade unter `dir`, ohne OS-Benachrichtigungen,
    /// Zugangsdaten nur im RAM.
    #[doc(hidden)]
    pub fn for_tests(paths: Paths) -> Arc<Self> {
        Arc::new(Self::new(
            paths,
            Arc::new(crate::notify::InboxOnly),
            Arc::new(crate::secrets::MemorySecretStore::default()),
            Arc::new(NoDesktop),
            "test",
        ))
    }
}
