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

use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;

use crate::app_ctx::AppCtx;
use crate::backup::BackupSession;
use crate::config::Paths;
use crate::error::Result;
use crate::notify::Notifier;

/// Tick-Intervall in Sekunden (5 Minuten).
const TICK_INTERVAL_SECS: u64 = 300;

/// Wartet einen laufenden Tick ab und pausiert weitere (Herunterfahren,
/// Restore; Plan W5.6).
pub async fn stop(ctx: &AppCtx) {
    *ctx.scheduler_gate.lock().await = true;
}

/// Hebt eine Pause wieder auf (z. B. nach gescheitertem Restore).
pub async fn resume(ctx: &AppCtx) {
    *ctx.scheduler_gate.lock().await = false;
}

/// Startet die Scheduler-Schleife genau einmal pro Prozess. Muss NACH dem
/// Entsperren aufgerufen werden (der Pool liegt dann im [`AppCtx`]). Mehrfache
/// Aufrufe (z. B. erneutes Entsperren) sind no-ops.
pub fn ensure_started(ctx: &Arc<AppCtx>) {
    if ctx.begin_scheduler() {
        start(ctx.clone());
    } else {
        tracing::debug!("Scheduler läuft bereits — kein erneuter Start.");
    }
}

/// Startet die Scheduler-Schleife als Hintergrund-Task. In der Regel über
/// [`ensure_started`] aufrufen, nicht direkt.
pub fn start(ctx: Arc<AppCtx>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
        // Verpasste Ticks (z. B. Standby) nicht nachfeuern — der nächste reguläre
        // Tick deckt den Rückstand ohnehin ab (Due-Check ist idempotent über das
        // Stichtags-Raster).
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await; // erster Tick: sofort; danach alle 5 min
            let gate = ctx.scheduler_gate.lock().await;
            if *gate {
                tracing::debug!("Scheduler pausiert — Tick ausgelassen");
                continue;
            }
            if let Err(e) = run_tick(&ctx).await {
                tracing::warn!("Scheduler-Tick fehlgeschlagen: {e}");
            }
            drop(gate);
        }
    });
}

/// Ein Durchlauf: Recurring-Kosten (Block 10) + Auto-AfA, Integrity-Check und
/// Reminder-Regeln (Block 15). Jeder Job ist isoliert — der Fehler eines Jobs
/// bricht die anderen nicht ab. Gesperrte Instanz → nichts zu tun.
async fn run_tick(ctx: &Arc<AppCtx>) -> Result<()> {
    let Ok(pool) = ctx.pool() else {
        return Ok(());
    };
    let today = crate::clock::today_berlin();
    run_jobs(&pool, &ctx.paths, &ctx.session, ctx.notifier(), today).await
}

/// Die Jobs eines Ticks — ohne Tauri-State testbar (`app = None`).
pub async fn run_jobs(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    notifier: Option<&dyn Notifier>,
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
        crate::scheduler::depreciation_year_close::run(pool, paths, session, notifier, today).await
    {
        tracing::warn!("Auto-AfA-Lauf fehlgeschlagen: {e}");
    }

    // 3. Monatlicher Archiv-Integrity-Check (Block 15).
    if let Err(e) = crate::scheduler::integrity_check_cron::run(pool, notifier, today).await {
        tracing::warn!("Integrity-Check fehlgeschlagen: {e}");
    }

    // 4. Reminder-Regeln (Block 15; backup_overdue Off-Site-bewusst seit G1-NOTIFY).
    match crate::scheduler::reminders::run(pool, notifier, today, &paths.backup_targets_dir).await {
        Ok(n) if n > 0 => tracing::info!("Scheduler: {n} neue Hinweis(e) erzeugt"),
        Ok(_) => {}
        Err(e) => tracing::warn!("Reminder-Lauf fehlgeschlagen: {e}"),
    }

    // 5. Wiederkehrende Ausgangsrechnungen (Block RI-2).
    match crate::scheduler::recurring_invoice::process_due(pool, paths, session, notifier, today)
        .await
    {
        Ok(report) if report.created_invoices > 0 => tracing::info!(
            "Scheduler: {} Abo-Rechnung(en) aus {} Vorlage(n) erzeugt",
            report.created_invoices,
            report.processed_templates
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Abo-Rechnungs-Lauf fehlgeschlagen: {e}"),
    }

    // Der Drop-Folder (Block PV1-DROP) läuft im Server nicht (E11, Plan W5.2);
    // Rechnungseingang per Upload (W7).
    Ok(())
}

#[cfg(test)]
mod tests {

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
            resource_dir: std::path::PathBuf::from("/non/existent/resources"),
            web_dir: std::path::PathBuf::from("/non/existent/web"),
            backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
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
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = crate::app_ctx::AppCtx::for_tests(crate::config::Paths::under(dir.path()));
        assert!(ctx.begin_scheduler(), "erster Aufruf darf starten");
        assert!(
            !ctx.begin_scheduler(),
            "zweiter Aufruf darf nicht erneut starten"
        );
        assert!(!ctx.begin_scheduler(), "weitere Aufrufe bleiben no-ops");
    }
}
