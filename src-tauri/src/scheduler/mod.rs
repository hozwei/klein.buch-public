//! Scheduler (Phase 2B+, Dauerbetrieb seit Lauf F / ADR 0040).
//!
//! Ein periodischer Tick (alle 5 min, solange die Instanz entsperrt ist) stößt
//! Kosten-Abos, Abo-Rechnungen, Auto-AfA, Tages-Backup, Integritätsprüfung und
//! Hinweise an; seit W12 auch die opt-in Versionsprüfung ([`update_check`]).
//! Jeder Job bestimmt seine Fälligkeit aus persistierten Werten und der
//! injizierten Uhr ([`JobCtx::clock`]), nicht aus dem Prozessstart oder einem
//! Seitenaufruf.

pub mod backoff;
pub mod catch_up;
pub mod depreciation_year_close;
pub mod hold;
pub mod integrity_check_cron;
pub mod recurring;
pub mod recurring_invoice;
pub mod reminders;
pub mod tick;
pub mod update_check;

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::SqlitePool;

use crate::backup::BackupSession;
use crate::clock::Clock;
use crate::config::Paths;

/// Nie gesetzte Abbruch-Anforderung (Tests, Aufrufer ohne Scheduler).
static NEVER_STOP: AtomicBool = AtomicBool::new(false);

/// Umgebung eines Scheduler-Laufs — Tick oder manueller Auslöser
/// („Alle fälligen erstellen“, „Jetzt erstellen“). Bündelt Pool, Pfade,
/// Backup-Sitzung, Uhr und die kooperative Abbruch-Anforderung
/// (`AppCtx::scheduler_stop`, W14-027).
#[derive(Clone, Copy)]
pub struct JobCtx<'a> {
    pub pool: &'a SqlitePool,
    pub paths: &'a Paths,
    pub session: &'a BackupSession,
    pub clock: &'a dyn Clock,
    stop: &'a AtomicBool,
    /// Im Tick: Lock-Backups sammeln statt sofort anlegen (D-19); `run_jobs`
    /// legt am Ende genau eines an.
    deferred_backup: Option<&'a AtomicBool>,
    /// Manueller Auslöser über einen Command (Lauf F3a, W5-AUTOBACKUP-SYNC):
    /// Lock-Backups werden vermerkt und im Hintergrund angelegt
    /// ([`crate::backup::request_auto_backup`]), die Antwort wartet nicht.
    background_backup: bool,
    /// Erster Tick nach dem Entsperren (D-19: ohne Integritäts-Vollscan).
    first_after_unlock: bool,
}

impl<'a> JobCtx<'a> {
    /// Umgebung ohne Abbruch-Anforderung; Lock-Backups entstehen sofort.
    pub fn new(
        pool: &'a SqlitePool,
        paths: &'a Paths,
        session: &'a BackupSession,
        clock: &'a dyn Clock,
    ) -> Self {
        Self {
            pool,
            paths,
            session,
            clock,
            stop: &NEVER_STOP,
            deferred_backup: None,
            background_backup: false,
            first_after_unlock: false,
        }
    }

    /// Umgebung aus dem Anwendungskontext (Uhr, Abbruch-Flag). Lock-Backups
    /// laufen im Hintergrund (Commands); der Tick sammelt sie stattdessen
    /// ([`Self::with_deferred_backup`]).
    pub fn from_app(
        ctx: &'a crate::app_ctx::AppCtx,
        pool: &'a SqlitePool,
        clock: &'a dyn Clock,
    ) -> Self {
        Self {
            pool,
            paths: &ctx.paths,
            session: &ctx.session,
            clock,
            stop: &ctx.scheduler_stop,
            deferred_backup: None,
            background_backup: true,
            first_after_unlock: false,
        }
    }

    pub fn with_stop(mut self, stop: &'a AtomicBool) -> Self {
        self.stop = stop;
        self
    }

    /// Lock-Backups dieses Laufs in `flag` sammeln (D-19, nur der Tick).
    pub fn with_deferred_backup(mut self, flag: &'a AtomicBool) -> Self {
        self.deferred_backup = Some(flag);
        self
    }

    /// Markiert den Lauf als ersten Tick nach dem Entsperren (D-19).
    pub fn with_first_after_unlock(mut self, first: bool) -> Self {
        self.first_after_unlock = first;
        self
    }

    /// Erster Tick nach dem Entsperren?
    pub fn first_after_unlock(&self) -> bool {
        self.first_after_unlock
    }

    /// Soll der Lauf nach dem aktuellen Termin aufhören (Sperren, Passphrase-
    /// Wechsel, Herunterfahren)?
    pub fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Die Abbruch-Anforderung selbst (für lange Schleifen wie den Vollscan).
    pub fn stop_flag(&self) -> &'a AtomicBool {
        self.stop
    }

    /// Jetzt (UTC) laut injizierter Uhr.
    pub fn now(&self) -> DateTime<Utc> {
        self.clock.now_utc()
    }

    /// Heute (Europe/Berlin) laut injizierter Uhr.
    pub fn today(&self) -> NaiveDate {
        self.clock.today_berlin()
    }

    /// Backup nach festgeschriebenen Belegen (Backup-Hardline): im Tick
    /// gesammelt und am Ende des Ticks genau einmal angelegt (D-19), bei
    /// manuellen Auslösern über einen Command vermerkt und im Hintergrund
    /// angelegt (Lauf F3a, [`crate::backup::request_auto_backup`]), sonst
    /// (Tests) sofort. Fehler sind nie fatal. Beim Sammeln steht zusätzlich
    /// der Vermerk „Backup ausstehend“ in der Datenbank — fällt das Backup am
    /// Ende des Ticks wegen „Datenbank sperren“ oder Herunterfahren aus, holt
    /// es der erste Tick danach nach (F1d).
    pub async fn lock_backup(&self, event: &str) {
        if let Some(flag) = self.deferred_backup {
            flag.store(true, Ordering::SeqCst);
            if let Err(e) = crate::backup::mark_pending(self.pool, self.now()).await {
                tracing::warn!("Vermerk „Backup ausstehend“ nicht gesetzt ({event}): {e}");
            }
            return;
        }
        if self.background_backup {
            crate::backup::request_auto_backup(
                self.pool,
                self.paths,
                self.session,
                event,
                self.now(),
            )
            .await
            .ok();
            return;
        }
        crate::backup::auto_backup_at(self.pool, self.paths, self.session, event, self.now())
            .await
            .ok();
    }
}
