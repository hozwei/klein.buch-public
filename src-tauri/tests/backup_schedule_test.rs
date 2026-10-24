//! Lauf F (Teil F1b) — Backup-, Prüf- und Aufräum-Zeitlogik gegen eine echte
//! Tempfile-DB (alle Migrationen) mit injizierter Test-Uhr (D-20):
//!
//! - Tages-Backup kalenderfest zur eingestellten Uhrzeit, über beide
//!   Zeitumstellungen, nach langer Sperre genau einmal, Backoff persistiert (D-04)
//! - Rotation nach Kalender-Eimern, SFTP nie gelöscht (D-05, D-06), Ruhe
//!   während einer Pause
//! - Integritäts-Cron: Monatssperre, genau eine Meldung, gemeinsame Sperre mit
//!   „Jetzt prüfen“, Backoff, Abbruch (KB-0038, D-07)
//! - Aufräumen von data/tmp und Backup-Teildateien (D-13)
//! - Entsperren: Pflicht-Audit vor dem Einhängen (D-21), Hinweis nach langer
//!   Sperre (D-08)
//! - Herunterfahren: Fristen passen zur `stop_grace_period` (W5-STOP-GRACE)

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::archive::integrity_check;
use klein_buch_lib::backup::{self, rotation, BackupSession};
use klein_buch_lib::clock::{Clock, FixedClock};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::app_settings;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::scheduler::integrity_check_cron::{self, CronOutcome};
use klein_buch_lib::scheduler::{backoff, hold};

const PASS: &str = "passphrase-1234567890";

struct Env {
    pool: SqlitePool,
    paths: Paths,
    session: BackupSession,
    tmp: tempfile::TempDir,
}

fn paths_under(root: &Path) -> Paths {
    let data = root.join("data");
    Paths {
        data_dir: data.clone(),
        db_file: data.join("klein-buch.sqlite"),
        archive_dir: data.join("archive"),
        backups_dir: data.join("backups"),
        inputs_dir: root.join("inputs"),
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
        resource_dir: PathBuf::from("/non/existent/resources"),
        web_dir: PathBuf::from("/non/existent/web"),
        backup_targets_dir: root.join("backup-targets"),
    }
}

async fn setup() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_under(tmp.path());
    std::fs::create_dir_all(&paths.archive_dir).unwrap();
    std::fs::create_dir_all(&paths.backups_dir).unwrap();
    let url = format!("sqlite://{}", paths.db_file.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    let session = BackupSession::default();
    session.set(PASS.into());
    Env {
        pool,
        paths,
        session,
        tmp,
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn daily(env: &Env, clock: &FixedClock) -> bool {
    backup::auto_daily_if_due(&env.pool, &env.paths, &env.session, clock.now_utc())
        .await
        .unwrap()
        .is_some()
}

// ---- Tages-Backup kalenderfest (D-04, D-12) ---------------------------------

/// Genau ein Tages-Backup je Berliner Kalendertag ab 02:30 — auch am Tag der
/// Zeitumstellung (doppelte bzw. fehlende Stunde) und nach drei Tagen Sperre.
#[tokio::test]
async fn daily_backup_is_calendar_fixed_across_dst() {
    let env = setup().await;
    // Das Tages-Backup des Vortags lief.
    backup::mark_daily_done(&env.pool, d(2026, 10, 24))
        .await
        .unwrap();
    // Ende der Sommerzeit: 25.10.2026, 02:30 gibt es zweimal.
    let clock = FixedClock::at_berlin(d(2026, 10, 25), 1, 0);
    assert!(!daily(&env, &clock).await, "vor 02:30 nicht fällig");
    clock.set(Utc.with_ymd_and_hms(2026, 10, 25, 0, 30, 0).unwrap()); // 02:30 MESZ
    assert!(daily(&env, &clock).await, "02:30 (erstes Mal)");
    clock.set(Utc.with_ymd_and_hms(2026, 10, 25, 1, 30, 0).unwrap()); // 02:30 MEZ
    assert!(
        !daily(&env, &clock).await,
        "die doppelte Stunde löst nicht erneut aus"
    );
    clock.set_berlin(d(2026, 10, 25), 23, 55);
    assert!(!daily(&env, &clock).await, "derselbe Kalendertag");
    clock.set_berlin(d(2026, 10, 26), 0, 5);
    assert!(!daily(&env, &clock).await, "neuer Tag, aber vor 02:30");
    clock.set_berlin(d(2026, 10, 26), 2, 35);
    assert!(daily(&env, &clock).await, "neuer Tag nach 02:30");

    // Beginn der Sommerzeit: 29.03.2027, 02:30 gibt es nicht — ab 03:00 fällig.
    clock.set_berlin(d(2027, 3, 28), 12, 0);
    assert!(daily(&env, &clock).await);
    clock.set(Utc.with_ymd_and_hms(2027, 3, 28, 23, 50, 0).unwrap()); // 29.03. 00:50 MEZ
    assert!(!daily(&env, &clock).await);
    clock.set(Utc.with_ymd_and_hms(2027, 3, 29, 1, 5, 0).unwrap()); // 03:05 MESZ
    assert!(
        daily(&env, &clock).await,
        "übersprungene 02:30 ist ab 03:00 erreicht"
    );

    // Drei Tage gesperrt → beim ersten Tick genau eines, kein Nachholen je Tag.
    clock.set_berlin(d(2027, 4, 1), 10, 0);
    assert!(daily(&env, &clock).await);
    assert!(!daily(&env, &clock).await);
    // Monatserster: Klasse „monthly“, Zeitstempel aus der injizierten Uhr.
    let (tag, created): (String, String) = sqlx::query_as(
        "SELECT retention_tag, created_at FROM backup_history ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(tag, "monthly");
    assert_eq!(created, "2027-04-01 08:00:00");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'"
        )
        .await,
        5
    );
    assert_eq!(
        app_settings::get(&env.pool, backup::DAILY_LAST_ON_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("2027-04-01")
    );
}

/// Die eingestellte Uhrzeit gilt; Lock-Backups zählen nicht als Tages-Backup,
/// das Erst-Backup der Einrichtung schon (Vermerk, D-12).
#[tokio::test]
async fn daily_time_setting_and_marker() {
    let env = setup().await;
    app_settings::set(&env.pool, backup::DAILY_TIME_KEY, "22:15")
        .await
        .unwrap();
    backup::mark_daily_done(&env.pool, d(2026, 6, 9))
        .await
        .unwrap();
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 21, 0);
    // Ein Lock-Backup am Nachmittag ersetzt das Tages-Backup nicht.
    backup::auto_backup_at(
        &env.pool,
        &env.paths,
        &env.session,
        "invoice.lock",
        clock.now_utc(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!daily(&env, &clock).await, "vor 22:15");
    clock.set_berlin(d(2026, 6, 10), 22, 20);
    assert!(daily(&env, &clock).await, "ab 22:15 trotz Lock-Backup");
    // Vermerk (wie nach der Einrichtung): heute erledigt.
    backup::mark_daily_done(&env.pool, d(2026, 6, 11))
        .await
        .unwrap();
    clock.set_berlin(d(2026, 6, 11), 23, 0);
    assert!(!daily(&env, &clock).await);
    assert_eq!(
        backup::parse_daily_time("7:30"),
        None,
        "nur HH:MM mit zwei Stellen"
    );
    assert!(backup::parse_daily_time("24:00").is_none());
    assert!(backup::parse_daily_time("02:30").is_some());
}

/// Bestand vor Lauf F (kein Vermerk): das jüngste `auto_daily` der Historie
/// zählt, sonst entstünde nach dem Update sofort ein zweites.
#[tokio::test]
async fn daily_without_marker_reads_history() {
    let env = setup().await;
    sqlx::query(
        "INSERT INTO backup_history (id, created_at, target_path, file_hash_sha256,
            file_size_bytes, retention_tag, trigger_reason, db_schema_version, app_version)
         VALUES ('alt', '2026-06-10 01:00:00', '/x/alt.kbk', 'x', 1, 'daily', 'auto_daily', 31, 'x')",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    // 10.06. 03:00 Berliner Zeit = 01:00 UTC → heute schon erledigt
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    assert!(!daily(&env, &clock).await);
}

/// Backoff nach einem Fehlschlag steht in der DB (übersteht Neustarts): nicht
/// bei jedem Tick ein neuer Versuch, nach einer Stunde wieder.
#[tokio::test]
async fn daily_backoff_is_persisted() {
    let env = setup().await;
    // Floor nicht beschreibbar: an seiner Stelle liegt eine Datei.
    std::fs::remove_dir_all(&env.paths.backups_dir).unwrap();
    std::fs::write(&env.paths.backups_dir, b"keine Ablage").unwrap();
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    assert!(!daily(&env, &clock).await);
    let failed = "SELECT COUNT(*) FROM backup_log WHERE status = 'failed'";
    assert_eq!(count(&env.pool, failed).await, 1);
    assert!(backoff::blocked(
        &env.pool,
        backup::DAILY_BACKOFF_KEY,
        clock.now_utc(),
        Duration::hours(1)
    )
    .await
    .unwrap());
    clock.advance(Duration::minutes(30));
    assert!(!daily(&env, &clock).await);
    assert_eq!(count(&env.pool, failed).await, 1, "kein Versuch im Backoff");
    clock.advance(Duration::minutes(31));
    assert!(!daily(&env, &clock).await);
    assert_eq!(
        count(&env.pool, failed).await,
        2,
        "nach einer Stunde erneut"
    );
    // Ablage wieder da → Erfolg hebt den Backoff auf.
    std::fs::remove_file(&env.paths.backups_dir).unwrap();
    std::fs::create_dir_all(&env.paths.backups_dir).unwrap();
    clock.advance(Duration::minutes(61));
    assert!(daily(&env, &clock).await);
    assert!(app_settings::get(&env.pool, backup::DAILY_BACKOFF_KEY)
        .await
        .unwrap()
        .is_none());
}

/// F1d: Eine Uhrzeit kurz vor Mitternacht fällt zwischen zwei Ticks (alle
/// 5 min, Takt am Prozessstart). Der erste Tick nach Mitternacht holt den
/// verpassten Slot genau einmal nach.
#[tokio::test]
async fn daily_slot_before_midnight_is_caught_up_after_midnight() {
    let env = setup().await;
    app_settings::set(&env.pool, backup::DAILY_TIME_KEY, "23:58")
        .await
        .unwrap();
    backup::mark_daily_done(&env.pool, d(2026, 6, 9))
        .await
        .unwrap();
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 23, 55);
    assert!(!daily(&env, &clock).await, "23:55 vor dem Slot");
    clock.set(Utc.with_ymd_and_hms(2026, 6, 10, 22, 0, 10).unwrap()); // 11.06. 00:00:10
    assert!(daily(&env, &clock).await, "Slot 10.06. nachgeholt");
    assert_eq!(
        app_settings::get(&env.pool, backup::DAILY_LAST_ON_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("2026-06-10"),
        "gemerkt wird der Slot"
    );
    clock.advance(Duration::minutes(5));
    assert!(!daily(&env, &clock).await, "nicht doppelt");
    clock.set_berlin(d(2026, 6, 11), 23, 55);
    assert!(!daily(&env, &clock).await);
    clock.set_berlin(d(2026, 6, 12), 0, 1);
    assert!(daily(&env, &clock).await, "Slot 11.06.");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'"
        )
        .await,
        2
    );
}

/// F1d: Nach mehreren gesperrten Tagen holt eine Anmeldung um 01:00 (vor der
/// Standard-Uhrzeit 02:30) das Tages-Backup sofort nach — genau eines; um
/// 02:30 folgt das reguläre des Tages.
#[tokio::test]
async fn login_before_daily_time_after_locked_days_catches_up_once() {
    let env = setup().await;
    backup::mark_daily_done(&env.pool, d(2026, 6, 6))
        .await
        .unwrap();
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 1, 0);
    assert!(daily(&env, &clock).await, "verpasster Slot 09.06.");
    clock.advance(Duration::minutes(5));
    assert!(!daily(&env, &clock).await, "genau eines");
    clock.set_berlin(d(2026, 6, 10), 2, 35);
    assert!(daily(&env, &clock).await, "regulärer Slot 10.06.");
    assert!(!daily(&env, &clock).await);
}

/// F1d: Ein im Tick gesammeltes Lock-Backup, das wegen „Datenbank sperren“
/// ausfiel, holt der erste Tick nach dem Entsperren nach — auch wenn das
/// Tages-Backup des Tages schon lief.
#[tokio::test]
async fn deferred_lock_backup_survives_lock_and_is_caught_up() {
    let env = setup().await;
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    backup::mark_daily_done(&env.pool, d(2026, 6, 10))
        .await
        .unwrap();
    let collected = AtomicBool::new(false);
    let job = klein_buch_lib::scheduler::JobCtx::new(&env.pool, &env.paths, &env.session, &clock)
        .with_deferred_backup(&collected);
    job.lock_backup("recurring.lock").await;
    assert!(collected.load(Ordering::SeqCst));
    assert!(app_settings::get(&env.pool, backup::PENDING_KEY)
        .await
        .unwrap()
        .is_some());
    // „Datenbank sperren“ mitten im Tick: das Backup am Ende fällt aus.
    env.session.clear();
    assert!(backup::auto_backup_at(
        &env.pool,
        &env.paths,
        &env.session,
        "recurring.lock",
        clock.now_utc()
    )
    .await
    .unwrap()
    .is_none());
    assert!(app_settings::get(&env.pool, backup::PENDING_KEY)
        .await
        .unwrap()
        .is_some());
    // Entsperren, erster Tick: genau ein Backup, Vermerk weg.
    env.session.set(PASS.into());
    clock.advance(Duration::minutes(30));
    let job = klein_buch_lib::scheduler::JobCtx::new(&env.pool, &env.paths, &env.session, &clock);
    klein_buch_lib::scheduler::tick::run_jobs(&job)
        .await
        .unwrap();
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_critical'"
        )
        .await,
        1
    );
    assert!(app_settings::get(&env.pool, backup::PENDING_KEY)
        .await
        .unwrap()
        .is_none());
    klein_buch_lib::scheduler::tick::run_jobs(&job)
        .await
        .unwrap();
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM backup_history").await,
        1,
        "kein weiteres Backup"
    );
}

// ---- Rotation (D-05, D-06) -------------------------------------------------

async fn history_row(pool: &SqlitePool, id: &str, created: &str, tag: &str, path: &str) {
    sqlx::query(
        "INSERT INTO backup_history (id, created_at, target_path, file_hash_sha256,
            file_size_bytes, retention_tag, trigger_reason, db_schema_version, app_version)
         VALUES (?, ?, ?, 'x', 1, ?, 'auto_critical', 31, 'x')",
    )
    .bind(id)
    .bind(created)
    .bind(path)
    .bind(tag)
    .execute(pool)
    .await
    .unwrap();
}

/// Ein Tag mit vielen Lock-Backups verdrängt die Vortage nicht; SFTP-Zeilen
/// bleiben (Klein.Buch löscht dort nichts) und behaupten keine Löschung.
#[tokio::test]
async fn rotation_keeps_days_and_never_touches_sftp() {
    let env = setup().await;
    let floor = &env.paths.backups_dir;
    for day in 1..=10u32 {
        let p = floor.join(format!("klein-buch-tag{day:02}.kbk"));
        std::fs::write(&p, b"x").unwrap();
        history_row(
            &env.pool,
            &format!("tag{day:02}"),
            &format!("2026-05-{day:02} 01:00:00"),
            "daily",
            &p.to_string_lossy(),
        )
        .await;
    }
    for i in 0..12u32 {
        let p = floor.join(format!("klein-buch-lock{i:02}.kbk"));
        std::fs::write(&p, b"x").unwrap();
        history_row(
            &env.pool,
            &format!("lock{i:02}"),
            &format!("2026-05-11 {:02}:00:00", 6 + i),
            "daily",
            &p.to_string_lossy(),
        )
        .await;
    }
    for i in 0..40u32 {
        history_row(
            &env.pool,
            &format!("sftp{i:02}"),
            &format!("2026-04-{:02} 02:00:00", 1 + i % 28),
            "daily",
            &format!("sftp://kb@backup.example.org:22/kb/s{i:02}.kbk"),
        )
        .await;
    }
    let pruned = rotation::run(
        &env.pool,
        floor,
        &env.paths.backup_targets_dir,
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    // Ohne erfolgreiches Off-Site-Ziel gilt für den Floor die lange Policy
    // (30 Tage): 11 Tage behalten, vom vollen Tag nur das jüngste.
    assert_eq!(pruned, 11, "11 ältere Lock-Backups vom 11.05.");
    let floor_rows = count(
        &env.pool,
        "SELECT COUNT(*) FROM backup_history WHERE target_path NOT LIKE 'sftp://%'",
    )
    .await;
    assert_eq!(floor_rows, 11);
    assert!(floor.join("klein-buch-lock11.kbk").exists());
    assert!(!floor.join("klein-buch-lock00.kbk").exists());
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE target_path LIKE 'sftp://%'"
        )
        .await,
        40,
        "SFTP-Zeilen bleiben"
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.rotation.prune'"
        )
        .await,
        11
    );
}

/// Während einer Pause ruht die Rotation (Uhrsprung, Probe-Instanz).
#[tokio::test]
async fn rotation_rests_during_hold() {
    let env = setup().await;
    let floor = &env.paths.backups_dir;
    // 40 alte Tage im Floor — weit über der Grenze.
    for day in 0..40i64 {
        let created = d(2025, 1, 1) + Duration::days(day);
        let p = floor.join(format!("klein-buch-alt{day:02}.kbk"));
        std::fs::write(&p, b"x").unwrap();
        history_row(
            &env.pool,
            &format!("alt{day:02}"),
            &format!("{created} 01:00:00"),
            "daily",
            &p.to_string_lossy(),
        )
        .await;
    }
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    hold::set(&env.pool, hold::REASON_CLOCK_JUMP, clock.now_utc())
        .await
        .unwrap();
    backup::auto_backup_at(
        &env.pool,
        &env.paths,
        &env.session,
        "manual",
        clock.now_utc(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM backup_history").await,
        41,
        "keine Löschung während der Pause"
    );
    hold::release(&env.pool, clock.now_utc()).await.unwrap();
    backup::auto_backup_at(
        &env.pool,
        &env.paths,
        &env.session,
        "manual",
        clock.now_utc(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        count(&env.pool, "SELECT COUNT(*) FROM backup_history").await < 41,
        "nach der Freigabe rotiert"
    );
}

// ---- Integritäts-Cron (KB-0038, D-07) ---------------------------------------

/// Die Scan-Sperre ist prozessweit; die Tests dieses Abschnitts laufen
/// nacheinander.
static INTEGRITY_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn archive_file(env: &Env, name: &str, bytes: &[u8]) -> String {
    integrity_check_store(env, name, bytes).await
}

async fn integrity_check_store(env: &Env, name: &str, bytes: &[u8]) -> String {
    klein_buch_lib::archive::store::store_bytes(
        &env.pool,
        &env.paths.archive_dir,
        2026,
        klein_buch_lib::archive::store::ArchiveKind::InvoicePdf,
        name,
        "application/pdf",
        bytes,
    )
    .await
    .unwrap()
    .file_path
}

fn tamper(path: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(path, b"manipuliert").unwrap();
}

/// KB-0038: Der zweite Aufruf im selben Monat tut nichts; eine manipulierte
/// Datei ergibt im nächsten Monat genau eine Meldung, auch bei weiteren Läufen.
#[tokio::test]
async fn integrity_cron_runs_once_per_month_and_reports_once() {
    let _serial = INTEGRITY_SERIAL.lock().await;
    let env = setup().await;
    let never = AtomicBool::new(false);
    archive_file(&env, "RE-2026-0001.pdf", b"eins").await;
    let second = archive_file(&env, "RE-2026-0002.pdf", b"zwei").await;
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    let run = |c: &FixedClock| integrity_check_cron::run(&env.pool, c.now_utc(), &never, true);
    assert_eq!(run(&clock).await.unwrap(), CronOutcome::Scanned);
    assert_eq!(run(&clock).await.unwrap(), CronOutcome::AlreadyDone);
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM archive_integrity_checks").await,
        1
    );
    tamper(&second);
    // Monatsgrenze in Berliner Zeit: 30.06. 23:30 MESZ ist noch Juni.
    clock.set_berlin(d(2026, 6, 30), 23, 30);
    assert_eq!(run(&clock).await.unwrap(), CronOutcome::AlreadyDone);
    clock.set_berlin(d(2026, 7, 1), 0, 30);
    assert_eq!(run(&clock).await.unwrap(), CronOutcome::Scanned);
    let notices =
        "SELECT COUNT(*) FROM notifications WHERE dedup_key = 'archive_integrity_failed:2026-07'";
    assert_eq!(count(&env.pool, notices).await, 1);
    assert_eq!(run(&clock).await.unwrap(), CronOutcome::AlreadyDone);
    // Ein manueller Lauf im selben Monat meldet dieselbe Störung nicht doppelt.
    let summary = integrity_check::run_full_scan(&env.pool).await.unwrap();
    integrity_check_cron::notify_summary(&env.pool, &summary, "2026-07")
        .await
        .unwrap();
    assert_eq!(count(&env.pool, notices).await, 1);
    assert_eq!(summary.files_failed, 1);
}

/// D-07: gemeinsame Sperre — „Jetzt prüfen“ antwortet „läuft bereits“, der
/// Cron ohne Warten meldet `Busy`, zwei wartende Läufe ergeben einen Kopf.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integrity_scan_lock_is_shared() {
    let _serial = INTEGRITY_SERIAL.lock().await;
    let env = setup().await;
    let never = AtomicBool::new(false);
    archive_file(&env, "RE-2026-0001.pdf", b"eins").await;
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    {
        let _held = integrity_check::try_scan_lock().unwrap();
        let err = integrity_check::try_scan_lock().unwrap_err().to_string();
        assert!(err.contains("läuft bereits"), "{err}");
        assert_eq!(
            integrity_check_cron::run(&env.pool, clock.now_utc(), &never, false)
                .await
                .unwrap(),
            CronOutcome::Busy
        );
    }
    let (a, b) = tokio::join!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &never, true),
        integrity_check_cron::run(&env.pool, clock.now_utc(), &never, true),
    );
    let mut outcomes = vec![a.unwrap(), b.unwrap()];
    outcomes.sort_by_key(|o| format!("{o:?}"));
    assert_eq!(
        outcomes,
        vec![CronOutcome::AlreadyDone, CronOutcome::Scanned]
    );
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM archive_integrity_checks").await,
        1
    );
}

/// D-07: Ein gescheiterter Scan wird vor Ablauf des Backoffs nicht wiederholt
/// (Hinweis einmal je Monat); ein abgebrochener Scan zählt nicht.
#[tokio::test]
async fn integrity_cron_backoff_and_abort() {
    let _serial = INTEGRITY_SERIAL.lock().await;
    let env = setup().await;
    archive_file(&env, "RE-2026-0001.pdf", b"eins").await;
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    // Abbruch-Anforderung: Lauf zählt nicht, kein Backoff.
    let stop = AtomicBool::new(true);
    assert_eq!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &stop, true)
            .await
            .unwrap(),
        CronOutcome::Aborted
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM archive_integrity_checks WHERE finished_at IS NULL"
        )
        .await,
        1
    );
    stop.store(false, Ordering::SeqCst);
    // Scan scheitert (Tabelle weg) → Backoff + Hinweis.
    sqlx::query("ALTER TABLE archive_entries RENAME TO archive_entries_weg")
        .execute(&env.pool)
        .await
        .unwrap();
    assert!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &stop, true)
            .await
            .is_err()
    );
    clock.advance(Duration::hours(5));
    assert_eq!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &stop, true)
            .await
            .unwrap(),
        CronOutcome::BackedOff
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key = 'archive_integrity_scan_failed:2026-06'"
        )
        .await,
        1
    );
    sqlx::query("ALTER TABLE archive_entries_weg RENAME TO archive_entries")
        .execute(&env.pool)
        .await
        .unwrap();
    clock.advance(Duration::hours(2));
    assert_eq!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &stop, true)
            .await
            .unwrap(),
        CronOutcome::Scanned
    );
    assert!(
        app_settings::get(&env.pool, integrity_check_cron::BACKOFF_KEY)
            .await
            .unwrap()
            .is_none(),
        "Erfolg hebt den Backoff auf"
    );
}

/// F1d: Ein zukünftig datierter Lauf (Uhr stand vor, danach korrigiert) löst
/// nicht bei jedem Tick einen neuen Vollscan aus — gezählt wird ein
/// abgeschlossener Lauf im Berliner Monatsintervall.
#[tokio::test]
async fn integrity_cron_ignores_future_dated_run() {
    let _serial = INTEGRITY_SERIAL.lock().await;
    let env = setup().await;
    let never = AtomicBool::new(false);
    archive_file(&env, "RE-2026-0001.pdf", b"eins").await;
    sqlx::query(
        "INSERT INTO archive_integrity_checks (started_at, finished_at, files_checked,
            files_passed, files_failed)
         VALUES ('2027-03-05 10:00:00', '2027-03-05 10:00:05', 1, 1, 0)",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let clock = FixedClock::at_berlin(d(2026, 10, 24), 12, 0);
    assert_eq!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &never, true)
            .await
            .unwrap(),
        CronOutcome::Scanned
    );
    clock.advance(Duration::minutes(5));
    assert_eq!(
        integrity_check_cron::run(&env.pool, clock.now_utc(), &never, true)
            .await
            .unwrap(),
        CronOutcome::AlreadyDone,
        "kein zweiter Vollscan im selben Monat"
    );
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM archive_integrity_checks").await,
        2
    );
}

// ---- Aufräumen (D-13) -------------------------------------------------------

fn age(path: &Path, hours: u64) {
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(hours * 3600);
    let f = std::fs::File::open(path).unwrap();
    f.set_modified(t).unwrap();
}

#[test]
fn tmp_sweep_removes_only_old_entries() {
    use klein_buch_lib::server::startup::{sweep_partial_backups, sweep_tmp, TMP_MAX_AGE};
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let tmp = data.join("tmp");
    let old_dsgvo = tmp.join("dsgvo").join("abc");
    let downloads = tmp.join("downloads");
    std::fs::create_dir_all(&old_dsgvo).unwrap();
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(tmp.join("snapshot-alt.db"), b"x").unwrap();
    std::fs::write(tmp.join("snapshot-neu.db"), b"x").unwrap();
    std::fs::write(old_dsgvo.join("auskunft.zip"), b"x").unwrap();
    std::fs::write(downloads.join("alt.zip"), b"x").unwrap();
    age(&tmp.join("snapshot-alt.db"), 7);
    age(&old_dsgvo.join("auskunft.zip"), 8);
    age(&old_dsgvo, 8);
    age(&downloads.join("alt.zip"), 9);

    let removed = sweep_tmp(&data, TMP_MAX_AGE);
    assert!(!tmp.join("snapshot-alt.db").exists());
    assert!(tmp.join("snapshot-neu.db").exists(), "jung bleibt");
    assert!(
        !old_dsgvo.exists(),
        "altes leeres Export-Verzeichnis fällt mit"
    );
    assert!(
        downloads.join("alt.zip").exists(),
        "downloads/ hat eigene Regeln"
    );
    assert!(removed >= 3, "{removed}");

    let backups = data.join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    std::fs::write(backups.join(".klein-buch-x.kbk.1.part"), b"x").unwrap();
    std::fs::write(backups.join(".klein-buch-y.kbk.2.part"), b"x").unwrap();
    std::fs::write(backups.join("klein-buch-z.kbk"), b"x").unwrap();
    age(&backups.join(".klein-buch-x.kbk.1.part"), 7);
    age(&backups.join("klein-buch-z.kbk"), 900);
    assert_eq!(sweep_partial_backups(&backups, TMP_MAX_AGE), 1);
    assert!(backups.join(".klein-buch-y.kbk.2.part").exists());
    assert!(
        backups.join("klein-buch-z.kbk").exists(),
        "fertige Backups nie"
    );
    assert_eq!(
        sweep_partial_backups(&backups, std::time::Duration::ZERO),
        1,
        "beim Serverstart alle Teildateien"
    );
}

// ---- Entsperren (D-21, D-08) ------------------------------------------------

fn ctx_for(env: &Env) -> Arc<AppCtx> {
    AppCtx::for_tests(env.paths.clone())
}

/// D-21: Scheitert das Pflicht-Audit, bleibt die Instanz gesperrt — kein
/// Pool, keine Passphrase im RAM, kein Scheduler.
#[tokio::test]
async fn unlock_audit_failure_leaves_instance_locked() {
    let env = setup().await;
    sqlx::query(
        "CREATE TRIGGER test_no_unlock BEFORE INSERT ON audit_log
         WHEN NEW.action = 'backup.unlock.ok'
         BEGIN SELECT RAISE(ABORT, 'Audit verweigert (Test)'); END",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let ctx = ctx_for(&env);
    let err = klein_buch_lib::commands::backup::activate_unlocked(
        &ctx,
        env.pool.clone(),
        PASS.to_string(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Audit verweigert"), "{err}");
    assert!(!ctx.is_unlocked(), "kein Pool");
    assert!(ctx.session.get().is_none(), "keine Passphrase");
    assert!(ctx.begin_scheduler(), "Scheduler nicht gestartet");
    assert!(!ctx.take_fresh_unlock());
}

/// D-08: Nach fünf Tagen ohne Aktivität meldet das Entsperren die Lücke
/// genau einmal; ein Entsperren am nächsten Tag nicht.
#[tokio::test]
async fn unlock_after_long_lock_creates_gap_notice() {
    let env = setup().await;
    klein_buch_lib::db::repo::audit_log::append(&env.pool, "auth.lock", "auth", "x", None)
        .await
        .unwrap();
    let ctx = ctx_for(&env);
    let clock = Arc::new(FixedClock::at(Utc::now() + Duration::days(5)));
    ctx.set_clock(clock.clone());
    klein_buch_lib::commands::backup::activate_unlocked(&ctx, env.pool.clone(), PASS.into())
        .await
        .unwrap();
    assert!(ctx.is_unlocked());
    let gap = "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'locked_gap:%'";
    assert_eq!(count(&env.pool, gap).await, 1);
    let title: String =
        sqlx::query_scalar("SELECT title FROM notifications WHERE dedup_key LIKE 'locked_gap:%'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert!(title.contains("Tage gesperrt"), "{title}");
    // kurz danach erneut entsperrt → keine neue Meldung
    let now = clock.now_utc();
    assert!(
        !klein_buch_lib::commands::backup::locked_gap_notice(
            &env.pool,
            now - Duration::hours(20),
            now
        )
        .await
    );
    assert_eq!(count(&env.pool, gap).await, 1);
}

// ---- Herunterfahren (W5-STOP-GRACE, W13-GRACE-BACKUP) -----------------------

/// Die Fristen des Herunterfahrens passen in die `stop_grace_period` des
/// Compose-Beispiels (mit Reserve).
#[test]
fn shutdown_budget_fits_stop_grace_period() {
    use klein_buch_lib::server::http::CONNECTION_DRAIN_TIMEOUT;
    use klein_buch_lib::server::state::{
        JOB_DRAIN_TIMEOUT, POOL_CLOSE_WAIT, SCHEDULER_SHUTDOWN_WAIT,
    };
    let compose = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compose.example.yaml"),
    )
    .unwrap();
    let grace: u64 = compose
        .lines()
        .find_map(|l| l.trim().strip_prefix("stop_grace_period:"))
        .map(|v| v.trim().trim_end_matches('s').parse().unwrap())
        .expect("stop_grace_period im Compose-Beispiel");
    let budget =
        CONNECTION_DRAIN_TIMEOUT + JOB_DRAIN_TIMEOUT.max(SCHEDULER_SHUTDOWN_WAIT) + POOL_CLOSE_WAIT;
    assert!(
        budget.as_secs() + 15 <= grace,
        "Herunterfahren bis {} s, stop_grace_period {grace} s",
        budget.as_secs()
    );
}

/// Dummy, damit der Temp-Ordner bis zum Testende lebt (Pfadangaben).
#[test]
fn env_paths_are_under_tempdir() {
    let p = paths_under(Path::new("/x"));
    assert_eq!(p.backups_dir, PathBuf::from("/x/data/backups"));
}

#[allow(dead_code)]
fn keep(env: &Env) -> &Path {
    env.tmp.path()
}

// ---- Pause nach Restore (W14-023, D-23) -------------------------------------

/// W14-023/D-23: Der Grund `restore` ersetzt eine mitgebrachte Pause; während
/// er gilt, geht ein Backup nur in den Floor (kein Ordner-Ziel, Audit
/// `backup.mirror.skipped`), nach der Freigabe wieder auf das Ziel.
#[tokio::test]
async fn restore_hold_replaces_hold_and_suspends_mirror() {
    let env = setup().await;
    std::fs::create_dir_all(env.paths.backup_targets_dir.join("nas")).unwrap();
    app_settings::set(
        &env.pool,
        "backup_targets",
        r#"[{"id":"a","label":"NAS","kind":"directory","path":"nas/kb"}]"#,
    )
    .await
    .unwrap();
    let clock = FixedClock::at_berlin(d(2026, 6, 10), 12, 0);
    hold::set(&env.pool, hold::REASON_CLOCK_JUMP, clock.now_utc())
        .await
        .unwrap();
    let mut tx = env.pool.begin().await.unwrap();
    let h = hold::set_restore_in(&mut tx, clock.now_utc())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(h.reason, hold::REASON_RESTORE);
    assert_eq!(
        hold::current(&env.pool).await.unwrap().unwrap().reason,
        hold::REASON_RESTORE
    );

    let mirrored = "SELECT COUNT(*) FROM backup_log WHERE target_kind = 'directory'";
    let out = backup::auto_backup_at(
        &env.pool,
        &env.paths,
        &env.session,
        "manual",
        clock.now_utc(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(out.offsite.len(), 1);
    assert_eq!(
        out.offsite[0].error.as_deref(),
        Some(backup::MIRROR_SUSPENDED)
    );
    assert_eq!(count(&env.pool, mirrored).await, 0);
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.mirror.skipped'"
        )
        .await,
        1
    );
    assert!(
        std::fs::read_dir(env.paths.backup_targets_dir.join("nas").join("kb"))
            .map(|it| it
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().ends_with(".kbk")))
            .unwrap_or(true),
        "nichts auf dem Ziel"
    );

    hold::release(&env.pool, clock.now_utc()).await.unwrap();
    let out = backup::auto_backup_at(
        &env.pool,
        &env.paths,
        &env.session,
        "manual",
        clock.now_utc(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(out.offsite[0].error.is_none(), "{:?}", out.offsite);
    assert_eq!(count(&env.pool, mirrored).await, 1);
    // F1d: Das Ergebnis je Ziel trägt die Zeit der injizierten Uhr, nicht die
    // Systemuhr.
    let results = klein_buch_lib::backup::target::load_results(&env.pool)
        .await
        .unwrap();
    let at = chrono::DateTime::parse_from_rfc3339(&results["a"].at).unwrap();
    assert_eq!(
        at.with_timezone(&Utc),
        clock.now_utc(),
        "{}",
        results["a"].at
    );
}

/// F1d: Bleibt die Pause nach einem Restore bestehen und die Instanz danach
/// tagelang gesperrt, meldet das nächste Entsperren die Lücke; nur das
/// Entsperren, das den Restore anwendet, bleibt still.
#[tokio::test]
async fn gap_notice_also_while_restore_hold_persists() {
    let gap = "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'locked_gap:%'";
    for (days_ago, expected) in [(10, 1), (0, 0)] {
        let env = setup().await;
        let restored_at = Utc::now() - Duration::days(days_ago);
        let mut tx = env.pool.begin().await.unwrap();
        hold::set_restore_in(&mut tx, restored_at).await.unwrap();
        tx.commit().await.unwrap();
        let ctx = ctx_for(&env);
        ctx.set_clock(Arc::new(FixedClock::at(Utc::now())));
        klein_buch_lib::commands::backup::activate_unlocked(&ctx, env.pool.clone(), PASS.into())
            .await
            .unwrap();
        assert_eq!(
            count(&env.pool, gap).await,
            expected,
            "Restore vor {days_ago} Tagen"
        );
    }
}

// ---- Einrichtung (D-12) -----------------------------------------------------

/// D-12: Die Einrichtung legt genau ein Erst-Backup an — vor dem
/// Scheduler-Start und als Tages-Backup des Tages vermerkt; ein Tick am selben
/// Tag legt kein zweites an.
#[tokio::test]
async fn setup_creates_exactly_one_initial_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths_under(tmp.path());
    std::fs::create_dir_all(&paths.archive_dir).unwrap();
    std::fs::create_dir_all(&paths.backups_dir).unwrap();
    let ctx = AppCtx::for_tests(paths.clone());
    let clock = Arc::new(FixedClock::at_berlin(d(2026, 6, 10), 12, 0));
    ctx.set_clock(clock.clone());
    klein_buch_lib::commands::backup::backup_setup_passphrase(
        &ctx,
        klein_buch_lib::commands::backup::BackupSetupPassphraseArgs {
            passphrase: "einrichtung-lang-genug-2026".into(),
        },
    )
    .await
    .unwrap();
    let pool = ctx.pool().unwrap();
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM backup_history").await, 1);
    assert_eq!(
        app_settings::get(&pool, backup::DAILY_LAST_ON_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("2026-06-10")
    );
    let job =
        klein_buch_lib::scheduler::JobCtx::new(&pool, &ctx.paths, &ctx.session, clock.as_ref());
    klein_buch_lib::scheduler::tick::run_jobs(&job)
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM backup_history").await,
        1,
        "kein zweites Voll-Backup am Einrichtungstag"
    );
    assert!(
        ctx.take_fresh_unlock(),
        "erster Tick nach der Einrichtung markiert"
    );
}
