//! Scheduler-Tick (Imperative Shell) — Block 10.
//!
//! Ein Tokio-Interval (5 Minuten, PRD §6.22) stößt periodisch die fälligen
//! Aufgaben an. In Block 10 ist das nur der Recurring-Due-Check; Reminders,
//! Integrity-Check-Cron und AfA-Year-Close folgen in späteren Blöcken (15/12).
//!
//! Seit der Bootstrap-Inversion (G1-ENC Schritt 2) startet der Scheduler erst
//! NACH dem ersten erfolgreichen Entsperren (über [`ensure_started`]) — vorher
//! gibt es keinen DB-Pool. Der erste Tick feuert dann sofort; die Session ist zu
//! diesem Zeitpunkt entsperrt. Die Auto-Anlage in [`crate::scheduler::recurring`]
//! bleibt zusätzlich session-gated (defensiv). Manuell lässt sich der Lauf
//! jederzeit über `recurring_run_due_check` auslösen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::backup::BackupSession;
use crate::config::Paths;
use crate::error::Result;

/// Tick-Intervall in Sekunden (5 Minuten).
const TICK_INTERVAL_SECS: u64 = 300;

/// Einmal-Guard für den Scheduler. Seit der Bootstrap-Inversion (G1-ENC
/// Schritt 2) startet der Scheduler nicht mehr im `setup`-Closure, sondern erst
/// nach dem ersten erfolgreichen Entsperren — wenn der Pool im Tauri-State
/// liegt. [`ensure_started`] sorgt dafür, dass er genau einmal pro Prozess
/// startet, auch bei mehreren Unlock-Aufrufen.
#[derive(Default)]
pub struct SchedulerStarted(AtomicBool);

impl SchedulerStarted {
    /// `true`, wenn dieser Aufruf den Start übernehmen darf (vorher noch nicht
    /// gestartet). Folgeaufrufe liefern `false`.
    fn try_begin(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
}

/// Startet die Scheduler-Schleife genau einmal pro Prozess. Muss NACH dem
/// Entsperren aufgerufen werden (der Pool muss im Tauri-State liegen). Mehrfache
/// Aufrufe (z. B. erneutes Unlock) sind dank [`SchedulerStarted`] no-ops.
pub fn ensure_started(app: &AppHandle) {
    let flag = app.state::<SchedulerStarted>();
    if flag.try_begin() {
        start(app.clone());
    } else {
        tracing::debug!("Scheduler läuft bereits — kein erneuter Start.");
    }
}

/// Startet die Scheduler-Schleife als Hintergrund-Task. Muss NACH dem
/// Entsperren aufgerufen werden (der Pool muss im Tauri-State liegen). In der
/// Regel über [`ensure_started`] aufrufen, nicht direkt.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
        // Verpasste Ticks (z. B. Standby) nicht nachfeuern — der nächste reguläre
        // Tick deckt den Rückstand ohnehin ab (Due-Check ist idempotent über das
        // Stichtags-Raster).
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await; // erster Tick: sofort; danach alle 5 min
            if let Err(e) = run_tick(&app).await {
                tracing::warn!("Scheduler-Tick fehlgeschlagen: {e}");
            }
        }
    });
}

/// Ein Durchlauf: Recurring-Kosten (Block 10) + Auto-AfA, Integrity-Check und
/// Reminder-Regeln (Block 15). Jeder Job ist isoliert — der Fehler eines Jobs
/// bricht die anderen nicht ab.
async fn run_tick(app: &AppHandle) -> Result<()> {
    let pool = app.state::<SqlitePool>();
    let session = app.state::<BackupSession>();
    let paths = Paths::from_handle(app)?;
    let today = crate::clock::today_berlin();
    run_jobs(pool.inner(), &paths, session.inner(), Some(app), today).await
}

/// Die Jobs eines Ticks — ohne Tauri-State testbar (`app = None`).
pub async fn run_jobs(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    app: Option<&AppHandle>,
    today: chrono::NaiveDate,
) -> Result<()> {
    // 0. Tages-Backup (Plan W2.4): fällig, wenn das letzte Backup älter als 24 h
    //    ist — im laufenden Betrieb, nicht nur beim Entsperren. Fehler landen im
    //    Backup-Protokoll; der Tick läuft weiter.
    if let Err(e) = crate::backup::auto_daily_if_due(pool, paths, session).await {
        tracing::warn!("Tages-Backup fehlgeschlagen: {e}");
    }

    // 1. Recurring-Kosten (Block 10).
    match crate::scheduler::recurring::process_due(pool, paths, session, today).await {
        Ok(report) if report.created_expenses > 0 => tracing::info!(
            "Scheduler: {} Recurring-Kosten aus {} Abo(s) angelegt",
            report.created_expenses,
            report.processed_subscriptions
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Recurring-Lauf fehlgeschlagen: {e}"),
    }

    // 2. Auto-AfA zur GJ-Wende (Block 15).
    if let Err(e) =
        crate::scheduler::depreciation_year_close::run(pool, paths, session, app, today).await
    {
        tracing::warn!("Auto-AfA-Lauf fehlgeschlagen: {e}");
    }

    // 3. Monatlicher Archiv-Integrity-Check (Block 15).
    if let Err(e) = crate::scheduler::integrity_check_cron::run(pool, app, today).await {
        tracing::warn!("Integrity-Check fehlgeschlagen: {e}");
    }

    // 4. Reminder-Regeln (Block 15; backup_overdue Off-Site-bewusst seit G1-NOTIFY).
    match crate::scheduler::reminders::run(pool, app, today, &paths.backups_dir).await {
        Ok(n) if n > 0 => tracing::info!("Scheduler: {n} neue Hinweis(e) erzeugt"),
        Ok(_) => {}
        Err(e) => tracing::warn!("Reminder-Lauf fehlgeschlagen: {e}"),
    }

    // 5. Wiederkehrende Ausgangsrechnungen (Block RI-2).
    match crate::scheduler::recurring_invoice::process_due(pool, paths, session, app, today).await {
        Ok(report) if report.created_invoices > 0 => tracing::info!(
            "Scheduler: {} Abo-Rechnung(en) aus {} Vorlage(n) erzeugt",
            report.created_invoices,
            report.processed_templates
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Abo-Rechnungs-Lauf fehlgeschlagen: {e}"),
    }

    // 6. Drop-Folder fuer eingehende E-Rechnungen (Block PV1-DROP).
    // Inbox-only (R4-007-Pattern) — keine AppHandle-Bindung im Scheduler-Pfad.
    match crate::scheduler::drop_folder::run_sync(pool, paths, session, today).await {
        Ok(report) if report.imported > 0 || report.failed > 0 => tracing::info!(
            "Scheduler: Drop-Folder {} importiert / {} fehlerhaft / {} versteckt",
            report.imported,
            report.failed,
            report.ignored_hidden
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Drop-Folder-Sync fehlgeschlagen: {e}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SchedulerStarted;

    /// Plan W2.4: der Tick legt das fällige Tages-Backup selbst an — ohne
    /// Entsperr-Ereignis; ein zweiter Tick am selben Tag nicht noch einmal.
    #[tokio::test]
    async fn tick_creates_daily_backup_without_unlock_event() {
        let dir = tempfile::TempDir::new().unwrap();
        let data = dir.path().join("data");
        let paths = super::Paths {
            data_dir: data.clone(),
            db_file: data.join("klein-buch.sqlite"),
            archive_dir: data.join("archive"),
            backups_dir: data.join("backups"),
            inputs_dir: dir.path().join("inputs"),
            sidecar_dir: dir.path().join("sidecar"),
        };
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        std::fs::create_dir_all(&paths.archive_dir).unwrap();
        let pool = crate::db::open_pool(&paths.db_file, None).await.unwrap();
        crate::db::MIGRATOR.run(&pool).await.unwrap();
        let session = super::BackupSession::default();
        session.set("passphrase-1234567890".into());
        let today = crate::clock::today_berlin();

        super::run_jobs(&pool, &paths, &session, None, today)
            .await
            .unwrap();
        let daily: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(daily, 1, "Tages-Backup aus dem Tick");

        super::run_jobs(&pool, &paths, &session, None, today)
            .await
            .unwrap();
        let daily: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(daily, 1, "nicht doppelt innerhalb von 24 h");
    }

    /// Der Einmal-Guard erlaubt genau einen Start; Folgeaufrufe sind no-ops.
    #[test]
    fn scheduler_starts_only_once() {
        let flag = SchedulerStarted::default();
        assert!(flag.try_begin(), "erster Aufruf darf starten");
        assert!(
            !flag.try_begin(),
            "zweiter Aufruf darf nicht erneut starten"
        );
        assert!(!flag.try_begin(), "weitere Aufrufe bleiben no-ops");
    }
}
