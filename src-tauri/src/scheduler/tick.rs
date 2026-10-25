//! Scheduler-Tick (Imperative Shell) — Block 10, Dauerbetrieb seit Lauf F.
//!
//! Ein Tokio-Interval (5 Minuten, PRD §6.22) stößt die fälligen Aufgaben an.
//! Reihenfolge eines Ticks (ADR 0040): Kosten-Abos → Abo-Rechnungen →
//! Auto-AfA → Backup (Tages-Backup, sonst ein gesammeltes Lock-Backup) →
//! Integritätsprüfung → Hinweise; danach die opt-in Versionsprüfung. Jeder Job
//! ist isoliert — der Fehler eines Jobs bricht die übrigen nicht ab (KB-0045).
//!
//! Der Scheduler startet nach dem ersten Entsperren bzw. der Einrichtung
//! ([`ensure_started`]); vorher gibt es keinen DB-Pool. Im gesperrten Zustand
//! läuft der Tick leer. Zeit kommt aus der Uhr des `AppCtx` (D-20).
//!
//! **Gestaffelt nach dem Entsperren (D-19):** Der erste Tick wartet
//! [`UNLOCK_SETTLE`] nach dem Entsperren (die Anmeldung soll nicht mit Backup
//! und Abo-Läufen konkurrieren) und lässt den Integritäts-Vollscan aus; die
//! Lock-Backups der Abo-Läufe eines Ticks werden zu genau einem Backup
//! zusammengefasst — ein fälliges Tages-Backup deckt sie mit ab.
//!
//! **Lock-Backups außerhalb der Antwort (Lauf F3a, W5-AUTOBACKUP-SYNC):**
//! Festschreibende Commands vermerken das Backup und wecken die Schleife
//! ([`crate::backup::request_auto_backup`]); nach [`BACKUP_COALESCE`] legt sie
//! unter derselben Sperre wie der Tick genau ein Backup an. Pausen und
//! Abbruch-Anforderung gelten auch hier; „Datenbank sperren“ und das
//! Herunterfahren holen ein so ausgefallenes Backup vor dem Schließen des Pools
//! nach ([`flush_pending_backup`], Lauf F3c), sonst der nächste Tick über den
//! Vermerk.
//!
//! **Anhalten (W14-027, W14-025b, ADR 0040):** [`request_stop`] setzt die
//! kooperative Abbruch-Anforderung (`AppCtx::scheduler_stop`); laufende
//! Abo-Läufe beenden den gerade bearbeiteten Termin, der Vollscan die gerade
//! geprüfte Datei, und der Tick hört auf (ein gesammeltes Lock-Backup legt er
//! noch an). [`stop`] wartet danach unbegrenzt auf das Ende des Ticks
//! (Restore), [`stop_within`] höchstens eine Frist („Datenbank sperren“,
//! „Passphrase ändern“, Herunterfahren). [`resume`] hebt beides auf.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::app_ctx::AppCtx;
use crate::error::Result;
use crate::scheduler::update_check::{self, Outcome};
use crate::scheduler::JobCtx;

/// Tick-Intervall in Sekunden (5 Minuten).
const TICK_INTERVAL_SECS: u64 = 300;

/// Wartezeit nach dem Entsperren, bevor der Tick fällige Jobs abarbeitet (D-19).
pub const UNLOCK_SETTLE: Duration = Duration::from_secs(60);

/// Bittet laufende Abo-Läufe, nach dem aktuellen Termin aufzuhören, und lässt
/// weitere Ticks aus, bis [`resume`] bzw. [`withdraw_stop`] folgt.
pub fn request_stop(ctx: &AppCtx) {
    ctx.scheduler_stop.store(true, Ordering::SeqCst);
}

/// Nimmt eine Abbruch-Anforderung zurück, ohne auf den Tick zu warten (z. B.
/// wenn „Passphrase ändern“ wegen einer Frist abbricht).
pub fn withdraw_stop(ctx: &AppCtx) {
    ctx.scheduler_stop.store(false, Ordering::SeqCst);
}

/// Fordert den Abbruch an, wartet einen laufenden Tick ab und pausiert weitere
/// (Restore; Plan W5.6). Ohne Frist — das Herunterfahren nutzt [`stop_within`].
pub async fn stop(ctx: &AppCtx) {
    request_stop(ctx);
    let _gate = ctx.scheduler_gate.lock().await;
    ctx.scheduler_paused.store(true, Ordering::SeqCst);
}

/// Wie [`stop`], aber höchstens `wait` lang. `false` = der Tick läuft noch; die
/// Abbruch-Anforderung bleibt gesetzt (der Tick hört nach dem laufenden Termin
/// auf, weitere Ticks lassen aus), bis [`resume`] oder [`withdraw_stop`].
pub async fn stop_within(ctx: &AppCtx, wait: Duration) -> bool {
    request_stop(ctx);
    match tokio::time::timeout(wait, ctx.scheduler_gate.lock()).await {
        Ok(_gate) => {
            ctx.scheduler_paused.store(true, Ordering::SeqCst);
            true
        }
        Err(_) => false,
    }
}

/// Holt ein ausstehendes Lock-Backup nach, bevor „Datenbank sperren“ oder das
/// Herunterfahren den Pool schließt (Lauf F3c, ADR 0040 Nr. 57): Steht der
/// Vermerk ([`crate::backup::PENDING_KEY`]) und ist die Sitzung entsperrt,
/// wartet es auf die Tick-Sperre (ein laufendes Backup endet zuerst),
/// pausiert den Scheduler und legt das Backup unter der Sperre an — alles
/// zusammen höchstens `wait`. Ohne Vermerk kehrt es sofort zurück. Der
/// geweckte Lauf verwirft das Backup bei einer Abbruch-Anforderung; ohne
/// dieses Nachholen läge ein eben festgeschriebener Beleg bis zum nächsten
/// Entsperren nur in der Live-DB. `true` = nichts steht mehr aus.
pub async fn flush_pending_backup(ctx: &Arc<AppCtx>, wait: Duration) -> bool {
    let Ok(pool) = ctx.pool() else {
        return true;
    };
    if !ctx.session.is_unlocked() {
        return true;
    }
    match crate::backup::is_backup_pending(&pool).await {
        Ok(false) => return true,
        Ok(true) => {}
        Err(e) => {
            tracing::warn!("Vermerk „Backup ausstehend“ nicht lesbar: {e}");
            return false;
        }
    }
    let run = async {
        let _gate = ctx.scheduler_gate.lock().await;
        ctx.scheduler_paused.store(true, Ordering::SeqCst);
        run_requested_backup(ctx).await;
    };
    if tokio::time::timeout(wait, run).await.is_err() {
        tracing::warn!(
            "Ausstehendes Lock-Backup nach {} s nicht fertig — das erste Entsperren holt es nach",
            wait.as_secs()
        );
        return false;
    }
    !crate::backup::is_backup_pending(&pool)
        .await
        .unwrap_or(true)
}

/// Hebt eine Pause wieder auf (Entsperren, gescheiterter Restore, Ende des
/// Passphrase-Wechsels). Wartet nicht auf die Tick-Sperre (F1d): Läuft beim
/// Entsperren noch ein alter Tick (hängender Sidecar oder SMTP nach „Datenbank
/// sperren“), scheitert er am geschlossenen Pool und endet; die Anmeldung
/// hängt nicht an ihm.
pub async fn resume(ctx: &AppCtx) {
    ctx.scheduler_paused.store(false, Ordering::SeqCst);
    withdraw_stop(ctx);
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
        // Tick deckt den Rückstand ab (Fälligkeit aus persistierten Werten).
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            // Erster Tick sofort, danach alle 5 min; dazwischen weckt ein
            // Lock-Ereignis den Lauf für sein Backup (Lauf F3a).
            let wake = tokio::select! {
                _ = interval.tick() => Wake::Tick,
                _ = ctx.session.backup_wake_requested() => Wake::Backup,
            };
            if wake == Wake::Backup {
                requested_backup_cycle(&ctx).await;
                continue;
            }
            // D-19: frühestens UNLOCK_SETTLE nach dem Entsperren — ohne die
            // Sperre zu halten (Sperren und Herunterfahren warten nicht darauf).
            // Ein Lock-Ereignis in dieser Zeit bekommt sein Backup trotzdem
            // gleich (der Inhaber arbeitet schon, Lauf F3a).
            while let Some(wait) = ctx.unlock_settle_remaining(UNLOCK_SETTLE) {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = ctx.session.backup_wake_requested() => {
                        requested_backup_cycle(&ctx).await;
                    }
                }
            }
            let gate = ctx.scheduler_gate.lock().await;
            if ctx.scheduler_paused.load(Ordering::SeqCst)
                || ctx.scheduler_stop.load(Ordering::SeqCst)
            {
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

/// Anlass eines Schleifendurchlaufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wake {
    /// Regulärer Tick (Intervall).
    Tick,
    /// Lock-Ereignis mit Backup-Anforderung ([`crate::backup::request_auto_backup`]).
    Backup,
}

/// Wartezeit nach dem Wecken, bevor das angeforderte Backup beginnt: weitere
/// Festschreibungen in dieser Zeit (Serie, Storno-Paar) fallen in dasselbe
/// Backup.
pub const BACKUP_COALESCE: Duration = Duration::from_secs(2);

/// Ein geweckter Durchlauf: [`BACKUP_COALESCE`] warten (Serien zusammenfassen),
/// dann unter der Tick-Sperre das angeforderte Backup — außer der Scheduler
/// ist pausiert oder soll anhalten (dann bleibt der Vermerk: Sperren und
/// Herunterfahren holen es über [`flush_pending_backup`] nach, sonst der erste
/// Tick nach `resume`).
async fn requested_backup_cycle(ctx: &Arc<AppCtx>) {
    tokio::time::sleep(BACKUP_COALESCE).await;
    let _gate = ctx.scheduler_gate.lock().await;
    if ctx.scheduler_paused.load(Ordering::SeqCst) || ctx.scheduler_stop.load(Ordering::SeqCst) {
        tracing::debug!("Scheduler pausiert — angefordertes Backup wartet");
        return;
    }
    run_requested_backup(ctx).await;
}

/// Angefordertes Backup nach einem Lock-Ereignis (Lauf F3a, W5-AUTOBACKUP-SYNC)
/// unter der Tick-Sperre: Passphrase-Wechsel und Restore warten darauf wie auf
/// einen Tick, „Datenbank sperren“ und Herunterfahren bis zu ihrer Frist für
/// das Nachholen ([`flush_pending_backup`]). Akteur `scheduler`.
async fn run_requested_backup(ctx: &Arc<AppCtx>) {
    let Ok(pool) = ctx.pool() else {
        return;
    };
    let now = ctx.clock().now_utc();
    let outcome = crate::db::repo::audit_log::with_actor(
        crate::db::repo::audit_log::ACTOR_SCHEDULER,
        crate::backup::requested_backup_now(&pool, &ctx.paths, &ctx.session, now),
    )
    .await;
    if let Err(e) = outcome {
        tracing::warn!("Angefordertes Backup fehlgeschlagen: {e}");
    }
}

/// Ein Durchlauf mit der Uhr des Kontexts. Gesperrte Instanz → nichts zu tun.
async fn run_tick(ctx: &Arc<AppCtx>) -> Result<()> {
    let Ok(pool) = ctx.pool() else {
        return Ok(());
    };
    let clock = ctx.clock();
    let job = JobCtx::from_app(ctx, &pool, clock.as_ref())
        .with_first_after_unlock(ctx.take_fresh_unlock());
    let result = run_jobs(&job).await;

    // Opt-in Versionsprüfung (Plan W12, E14): ohne Opt-in keine Verbindung,
    // sonst höchstens täglich. Außerhalb von `run_jobs`, damit die DB-Jobs
    // (und ihre Tests) nie ins Netz gehen.
    if !job.stop_requested() {
        let url = update_check::endpoint();
        match update_check::run_if_due(&pool, &url, job.now()).await {
            Ok(Outcome::NewVersion { tag, created: true }) => {
                tracing::info!("Versionsprüfung: neue Version {tag} — Hinweis angelegt");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("{e}"),
        }
    }
    result
}

/// Die Jobs eines Ticks. Jeder Job ist isoliert: ein Fehler wird protokolliert,
/// die folgenden Jobs laufen trotzdem (KB-0045). Eine Abbruch-Anforderung
/// beendet den Tick vor dem nächsten Job; ein gesammeltes Lock-Backup entsteht
/// trotzdem (Backup-Hardline).
pub async fn run_jobs(job: &JobCtx<'_>) -> Result<()> {
    // Akteur aller Audit-Einträge des Ticks (Lauf F, R1-S3-ACTOR).
    crate::db::repo::audit_log::with_actor(
        crate::db::repo::audit_log::ACTOR_SCHEDULER,
        run_jobs_inner(job),
    )
    .await
}

async fn run_jobs_inner(job: &JobCtx<'_>) -> Result<()> {
    let lock_backup_due = AtomicBool::new(false);
    let deferred = job.with_deferred_backup(&lock_backup_due);
    let stopped = run_booking_jobs(&deferred).await;

    // 3. Backup (Plan W2.4, D-04, D-19): das Tages-Backup (kalenderfest)
    //    deckt die Lock-Ereignisse dieses Ticks mit ab; sonst genau ein
    //    Lock-Backup, falls Abo-Läufe Belege festgeschrieben haben. Fehler
    //    landen im Backup-Protokoll; der Tick läuft weiter.
    let daily = if stopped {
        None
    } else {
        match crate::backup::auto_daily_if_due(job.pool, job.paths, job.session, job.now()).await {
            Ok(outcome) => outcome,
            Err(e) => {
                tracing::warn!("Tages-Backup fehlgeschlagen: {e}");
                None
            }
        }
    };
    if daily.is_none() {
        if lock_backup_due.load(Ordering::SeqCst) {
            crate::backup::auto_backup_at(
                job.pool,
                job.paths,
                job.session,
                "recurring.lock",
                job.now(),
            )
            .await
            .ok();
        } else if !stopped {
            // F1d: Ein Lock-Backup, das bei „Datenbank sperren“ oder beim
            // Herunterfahren ausfiel (Vermerk `backup_pending_since`), holt
            // der erste Tick danach nach — unabhängig vom Tages-Vermerk.
            if let Err(e) =
                crate::backup::pending_backup_if_due(job.pool, job.paths, job.session, job.now())
                    .await
            {
                tracing::warn!("Ausstehendes Backup nicht nachgeholt: {e}");
            }
        }
    }
    if stopped || job.stop_requested() {
        return Ok(());
    }

    // 4. Monatlicher Archiv-Integrity-Check (Block 15, D-07) — nicht im ersten
    //    Tick nach dem Entsperren (D-19).
    if job.first_after_unlock() {
        tracing::debug!("Integritätsprüfung im ersten Tick nach dem Entsperren ausgelassen");
    } else if let Err(e) =
        crate::scheduler::integrity_check_cron::run(job.pool, job.now(), job.stop_flag(), true)
            .await
    {
        tracing::warn!("Integrity-Check fehlgeschlagen: {e}");
    }
    if job.stop_requested() {
        return Ok(());
    }

    // 5. Reminder-Regeln (Block 15; backup_overdue Off-Site-bewusst seit G1-NOTIFY;
    //    „Abo fällig“ im Modus Erinnerung seit Lauf F) — nach dem Backup, damit
    //    ein gerade nachgeholtes Tages-Backup nicht als überfällig gilt.
    match crate::scheduler::reminders::run(job.pool, job.today(), &job.paths.backup_targets_dir)
        .await
    {
        Ok(n) if n > 0 => tracing::info!("Scheduler: {n} neue Hinweis(e) erzeugt"),
        Ok(_) => {}
        Err(e) => tracing::warn!("Reminder-Lauf fehlgeschlagen: {e}"),
    }

    // Der Drop-Folder (Block PV1-DROP) läuft im Server nicht (E11, Plan W5.2);
    // Rechnungseingang per Upload (W7).
    Ok(())
}

/// Belegerzeugende Jobs (Lock-Backups gesammelt). `true` = auf Anforderung
/// vorzeitig beendet.
async fn run_booking_jobs(job: &JobCtx<'_>) -> bool {
    // 1. Kosten-Abos (Block 10, Lauf F).
    match crate::scheduler::recurring::process_due(job).await {
        Ok(report) if report.created_expenses > 0 => tracing::info!(
            "Scheduler: {} Abo-Kosten aus {} Abo(s) gebucht",
            report.created_expenses,
            report.processed_subscriptions
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Abo-Kosten-Lauf fehlgeschlagen: {e}"),
    }
    if job.stop_requested() {
        return true;
    }

    // 2. Wiederkehrende Ausgangsrechnungen (Block RI-2, Lauf F).
    match crate::scheduler::recurring_invoice::process_due(job).await {
        Ok(report) if report.created_invoices > 0 => tracing::info!(
            "Scheduler: {} Abo-Rechnung(en) aus {} Vorlage(n) erzeugt",
            report.created_invoices,
            report.processed_templates
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Abo-Rechnungs-Lauf fehlgeschlagen: {e}"),
    }
    if job.stop_requested() {
        return true;
    }

    // 2b. Auto-AfA zur GJ-Wende (Block 15; legt sein Backup selbst an, einmal
    //     je Jahr).
    if let Err(e) = crate::scheduler::depreciation_year_close::run(job).await {
        tracing::warn!("Auto-AfA-Lauf fehlgeschlagen: {e}");
    }
    job.stop_requested()
}

/// Restliche Wartezeit nach dem Entsperren (pure, für Tests): `None`, sobald
/// `settle` seit `elapsed` vergangen ist.
pub fn settle_remaining(elapsed: Duration, settle: Duration) -> Option<Duration> {
    settle.checked_sub(elapsed).filter(|d| !d.is_zero())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    fn paths(dir: &tempfile::TempDir) -> crate::config::Paths {
        let data = dir.path().join("data");
        crate::config::Paths {
            data_dir: data.clone(),
            db_file: data.join("klein-buch.sqlite"),
            archive_dir: data.join("archive"),
            backups_dir: data.join("backups"),
            inputs_dir: dir.path().join("inputs"),
            sidecar_dir: dir.path().join("sidecar"),
            resource_dir: std::path::PathBuf::from("/non/existent/resources"),
            web_dir: std::path::PathBuf::from("/non/existent/web"),
            backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
        }
    }

    /// Plan W2.4 / D-04: der Tick legt das fällige Tages-Backup selbst an —
    /// ohne Entsperr-Ereignis; ein zweiter Tick am selben Tag nicht noch einmal.
    #[tokio::test]
    async fn tick_creates_daily_backup_without_unlock_event() {
        let dir = tempfile::TempDir::new().unwrap();
        let paths = paths(&dir);
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        std::fs::create_dir_all(&paths.archive_dir).unwrap();
        let pool = crate::db::open_pool(&paths.db_file, None).await.unwrap();
        crate::db::MIGRATOR.run(&pool).await.unwrap();
        let session = crate::backup::BackupSession::default();
        session.set("passphrase-1234567890".into());
        let clock = crate::clock::FixedClock::at_berlin(
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
            12,
            0,
        );
        let job = crate::scheduler::JobCtx::new(&pool, &paths, &session, &clock);

        super::run_jobs(&job).await.unwrap();
        let daily: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(daily, 1, "Tages-Backup aus dem Tick");
        // Lauf F (R1-S3-ACTOR): Audit-Einträge des Ticks tragen den Akteur
        // `scheduler`.
        let actors: Vec<String> = sqlx::query_scalar("SELECT DISTINCT actor FROM audit_log")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(actors, vec!["scheduler".to_string()], "{actors:?}");

        super::run_jobs(&job).await.unwrap();
        let daily: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(daily, 1, "nicht doppelt am selben Tag");
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

    /// D-19: Nach dem Entsperren wartet der Tick, der erste Tick danach ist
    /// genau einmal als solcher markiert.
    #[test]
    fn unlock_settle_and_fresh_flag() {
        use std::time::Duration;
        assert_eq!(
            super::settle_remaining(Duration::from_secs(10), Duration::from_secs(60)),
            Some(Duration::from_secs(50))
        );
        assert_eq!(
            super::settle_remaining(Duration::from_secs(60), Duration::from_secs(60)),
            None
        );
        assert_eq!(
            super::settle_remaining(Duration::from_secs(90), Duration::from_secs(60)),
            None
        );
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = crate::app_ctx::AppCtx::for_tests(crate::config::Paths::under(dir.path()));
        assert!(ctx.unlock_settle_remaining(super::UNLOCK_SETTLE).is_none());
        assert!(!ctx.take_fresh_unlock());
        ctx.mark_unlocked();
        let wait = ctx.unlock_settle_remaining(super::UNLOCK_SETTLE).unwrap();
        assert!(wait > Duration::from_secs(55) && wait <= super::UNLOCK_SETTLE);
        assert!(ctx.take_fresh_unlock());
        assert!(!ctx.take_fresh_unlock(), "nur der erste Tick");
    }
}
