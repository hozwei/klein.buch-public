//! Reminder-Rule-Engine (Block 15).
//!
//! Wird vom [`crate::scheduler::tick`] bei jedem Tick aufgerufen. Für jede aktive
//! Regel wird geprüft, ob ein Hinweis fällig ist; Dedup über `dedup_key` sorgt
//! dafür, dass jeder periodische Hinweis pro Periode nur **einmal** entsteht
//! (auch wenn der Tick alle 5 Minuten feuert).
//!
//! Manuel-Auswahl Block 15: monthly_doc_check, fiscal_year_lock_pending,
//! backup_overdue, invoice_overdue (alle vier aktiv).

use crate::error::Result;
use crate::fiscal_year::guard;
use crate::notify::{self, rules, NewNotification};
use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;
use std::path::Path;
use tauri::AppHandle;

/// Führt alle aktiven Reminder-Regeln aus. Liefert die Anzahl neu erzeugter
/// Hinweise (Dedup-Treffer zählen nicht).
///
/// `floor_dir` ist der lokale Backup-Floor (`paths.backups_dir`); die
/// `backup_overdue`-Regel braucht ihn, um ein konfiguriertes Off-Site-Ziel vom
/// Floor zu unterscheiden (G1-NOTIFY).
pub async fn run(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    today: NaiveDate,
    floor_dir: &Path,
) -> Result<usize> {
    let mut created = 0usize;

    if let Some(rule) = rules::get_enabled_by_type(pool, "monthly_doc_check").await? {
        created += monthly_doc_check(pool, app, &rule, today).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "fiscal_year_lock_pending").await? {
        created += fiscal_year_lock_pending(pool, app, &rule, today).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "backup_overdue").await? {
        created += backup_overdue(pool, app, &rule, today, floor_dir).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "invoice_overdue").await? {
        created += invoice_overdue(pool, app, &rule, today).await?;
    }
    Ok(created)
}

async fn emit_one(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    n: NewNotification<'_>,
) -> Result<usize> {
    Ok(notify::emit(pool, app, n).await?.map(|_| 1).unwrap_or(0))
}

/// Monatlicher Doku-Check ab Tag N (Default 10.). Dedup pro Kalendermonat.
async fn monthly_doc_check(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    rule: &rules::NotificationRule,
    today: NaiveDate,
) -> Result<usize> {
    let day = rule.config_i64("day_of_month", 10);
    if (today.day() as i64) < day {
        return Ok(0);
    }
    let key = format!("monthly_doc_check:{:04}-{:02}", today.year(), today.month());
    emit_one(
        pool,
        app,
        NewNotification {
            rule_id: Some(&rule.id),
            title: "Belege erfassen",
            body: "Monatlicher Hinweis: Sind alle Rechnungen, Kosten und Belege des Vormonats erfasst?",
            severity: "info",
            action_url: Some("/expenses"),
            dedup_key: Some(&key),
            ..Default::default()
        },
    )
    .await
}

/// GJ-Abschluss fällig ab dem Stichtag (Default 01.06.) für das Vorjahr,
/// solange es nicht abgeschlossen ist. Dedup pro Zieljahr.
async fn fiscal_year_lock_pending(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    rule: &rules::NotificationRule,
    today: NaiveDate,
) -> Result<usize> {
    let month = rule.config_i64("month", 6) as u32;
    let day = rule.config_i64("day", 1) as u32;
    let Some(threshold) = NaiveDate::from_ymd_opt(today.year(), month, day) else {
        return Ok(0);
    };
    if today < threshold {
        return Ok(0);
    }
    let target_year = (today.year() - 1) as i64;
    if guard::is_closed(pool, target_year).await? {
        return Ok(0);
    }
    let key = format!("fiscal_year_lock_pending:{target_year}");
    let body = format!(
        "Das Geschäftsjahr {target_year} ist noch nicht abgeschlossen. \
         Schließe es ab (Festschreibung) und erstelle die EÜR."
    );
    emit_one(
        pool,
        app,
        NewNotification {
            rule_id: Some(&rule.id),
            title: "Geschäftsjahr abschließen",
            body: &body,
            severity: "warning",
            action_url: Some("/fiscal-year"),
            dedup_key: Some(&key),
            ..Default::default()
        },
    )
    .await
}

/// Hinweis, wenn die maßgebliche Sicherung älter als N Tage ist (Default 7).
/// Dedup pro Tag. **Off-Site-bewusst** (G1-NOTIFY):
///
/// - Ist ein **Off-Site-Ziel** konfiguriert (Cloud-Ordner/USB/SFTP, vom Floor
///   verschieden), zählt das letzte **erfolgreiche Off-Site-Backup** aus dem
///   append-only `backup_log` (G1-LOG). Der lokale Floor ist ohnehin fast immer
///   frisch (Lock-/Tages-Backup) — die Off-Site-Kopie ist das, was ausfallen kann.
/// - Ohne Off-Site-Ziel bleibt es beim lokalen Floor (`backup_history`): warnt,
///   wenn überhaupt keine Sicherung mehr lief (App lange nicht geöffnet).
async fn backup_overdue(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    rule: &rules::NotificationRule,
    today: NaiveDate,
    floor_dir: &Path,
) -> Result<usize> {
    let max_age = rule.config_i64("max_age_days", 7);
    let has_offsite = crate::backup::target::offsite_target(pool, floor_dir)
        .await?
        .is_some();

    let (last, title, body, key): (Option<String>, &str, String, String) = if has_offsite {
        let last = sqlx::query_scalar(
            "SELECT MAX(created_at) FROM backup_log
              WHERE status = 'ok' AND target_kind IN ('directory','sftp')",
        )
        .fetch_one(pool)
        .await?;
        (
            last,
            "Off-Site-Backup überfällig",
            format!(
                "Seit über {max_age} Tagen gab es kein erfolgreiches externes Backup \
                 (oder noch keins). Prüfe dein Backup-Ziel (Cloud-Ordner, USB oder SFTP)."
            ),
            format!("offsite_backup_overdue:{}", today.format("%Y-%m-%d")),
        )
    } else {
        let last = sqlx::query_scalar("SELECT MAX(created_at) FROM backup_history")
            .fetch_one(pool)
            .await?;
        (
            last,
            "Backup überfällig",
            format!(
                "Das letzte Backup ist älter als {max_age} Tage (oder es existiert keins). \
                 Bitte jetzt eine Sicherung erstellen — am besten mit externem Ziel."
            ),
            format!("backup_overdue:{}", today.format("%Y-%m-%d")),
        )
    };

    let overdue = match last.as_deref() {
        None => true,
        Some(ts) => match ts
            .get(0..10)
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        {
            Some(d) => (today - d).num_days() > max_age,
            None => false,
        },
    };
    if !overdue {
        return Ok(0);
    }
    emit_one(
        pool,
        app,
        NewNotification {
            rule_id: Some(&rule.id),
            title,
            body: &body,
            severity: "warning",
            action_url: Some("/settings/backup"),
            dedup_key: Some(&key),
            ..Default::default()
        },
    )
    .await
}

/// Hinweis je überfälliger, festgeschriebener, unbezahlter Rechnung. Dedup pro
/// Rechnung (nur ein Hinweis, bis abgehakt).
async fn invoice_overdue(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    rule: &rules::NotificationRule,
    today: NaiveDate,
) -> Result<usize> {
    let today_str = today.format("%Y-%m-%d").to_string();
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, invoice_number, due_date
           FROM invoices
          WHERE direction = 'issued'
            AND is_storno_for IS NULL
            AND canceled_at IS NULL
            AND status IN ('issued','sent','partially_paid')
            AND gross_amount_cents > paid_amount_cents
            AND due_date IS NOT NULL
            AND due_date < ?
          ORDER BY due_date ASC",
    )
    .bind(&today_str)
    .fetch_all(pool)
    .await?;

    let mut created = 0usize;
    for (id, number, due_date) in rows {
        let key = format!("invoice_overdue:{id}");
        let body = format!("Die Rechnung {number} ist seit {due_date} überfällig.");
        let action = format!("/invoices/{id}");
        created += emit_one(
            pool,
            app,
            NewNotification {
                rule_id: Some(&rule.id),
                title: "Rechnung überfällig",
                body: &body,
                severity: "warning",
                related_entity_type: Some("invoice"),
                related_entity_id: Some(&id),
                action_url: Some(&action),
                dedup_key: Some(&key),
            },
        )
        .await?;
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for stmt in [
            "CREATE TABLE notification_rules (
                id TEXT PRIMARY KEY NOT NULL, rule_type TEXT NOT NULL, label TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1, config_json TEXT NOT NULL,
                deliver_in_app INTEGER NOT NULL DEFAULT 1, deliver_os_native INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
            "CREATE TABLE notifications (
                id TEXT PRIMARY KEY NOT NULL, rule_id TEXT, title TEXT NOT NULL, body TEXT NOT NULL,
                severity TEXT NOT NULL, related_entity_type TEXT, related_entity_id TEXT,
                triggered_at TEXT NOT NULL DEFAULT (datetime('now','utc')), dismissed_at TEXT,
                action_url TEXT, dedup_key TEXT) STRICT",
            "CREATE UNIQUE INDEX uq_notifications_dedup ON notifications(dedup_key) WHERE dedup_key IS NOT NULL",
            "CREATE TABLE fiscal_year_locks (fiscal_year INTEGER PRIMARY KEY NOT NULL,
                closed_at TEXT, income_total_cents INTEGER NOT NULL, expense_total_cents INTEGER NOT NULL,
                afa_total_cents INTEGER NOT NULL, surplus_cents INTEGER NOT NULL,
                assets_locked INTEGER NOT NULL DEFAULT 0, depreciation_entries_locked INTEGER NOT NULL DEFAULT 0,
                app_version TEXT NOT NULL, schema_version INTEGER NOT NULL, notes TEXT) STRICT",
            "INSERT INTO notification_rules (id, rule_type, label, config_json) VALUES
                ('rule_monthly_doc_check','monthly_doc_check','Doku','{\"day_of_month\":10}')",
        ] {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn monthly_doc_check_fires_once_per_month() {
        let pool = pool().await;
        let rule = rules::get_enabled_by_type(&pool, "monthly_doc_check")
            .await
            .unwrap()
            .unwrap();
        let day15 = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        // erster Lauf erzeugt einen Hinweis
        assert_eq!(
            monthly_doc_check(&pool, None, &rule, day15).await.unwrap(),
            1
        );
        // zweiter Lauf im selben Monat: Dedup
        assert_eq!(
            monthly_doc_check(&pool, None, &rule, day15).await.unwrap(),
            0
        );
        // vor dem Stichtag: nichts
        let day5 = NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();
        assert_eq!(
            monthly_doc_check(&pool, None, &rule, day5).await.unwrap(),
            0
        );
    }

    // ---- backup_overdue (G1-NOTIFY, Off-Site-bewusst) --------------------

    async fn backup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for stmt in [
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
            "CREATE TABLE notification_rules (
                id TEXT PRIMARY KEY NOT NULL, rule_type TEXT NOT NULL, label TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1, config_json TEXT NOT NULL,
                deliver_in_app INTEGER NOT NULL DEFAULT 1, deliver_os_native INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
            "CREATE TABLE notifications (
                id TEXT PRIMARY KEY NOT NULL, rule_id TEXT, title TEXT NOT NULL, body TEXT NOT NULL,
                severity TEXT NOT NULL, related_entity_type TEXT, related_entity_id TEXT,
                triggered_at TEXT NOT NULL DEFAULT (datetime('now','utc')), dismissed_at TEXT,
                action_url TEXT, dedup_key TEXT) STRICT",
            "CREATE UNIQUE INDEX uq_notifications_dedup ON notifications(dedup_key) WHERE dedup_key IS NOT NULL",
            "CREATE TABLE backup_history (id TEXT PRIMARY KEY NOT NULL, created_at TEXT NOT NULL) STRICT",
            "CREATE TABLE backup_log (id TEXT PRIMARY KEY NOT NULL, created_at TEXT NOT NULL,
                target_kind TEXT NOT NULL, status TEXT NOT NULL) STRICT",
            "INSERT INTO notification_rules (id, rule_type, label, config_json) VALUES
                ('rule_backup_overdue','backup_overdue','Backup überfällig','{\"max_age_days\":7}')",
        ] {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn set_offsite(pool: &SqlitePool) {
        sqlx::query("INSERT INTO app_settings (key, value) VALUES ('backup_target', ?)")
            .bind(r#"{"kind":"directory","path":"/mnt/onedrive/kb"}"#)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn backup_overdue_floor_path_when_no_offsite() {
        let pool = backup_pool().await;
        let rule = rules::get(&pool, "rule_backup_overdue")
            .await
            .unwrap()
            .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let floor = Path::new("/data/backups");
        // Kein Off-Site-Ziel, kein Backup → überfällig über den Floor-Pfad.
        assert_eq!(
            backup_overdue(&pool, None, &rule, today, floor)
                .await
                .unwrap(),
            1
        );
        let title: String = sqlx::query_scalar("SELECT title FROM notifications LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(title, "Backup überfällig");
    }

    #[tokio::test]
    async fn backup_overdue_offsite_fires_without_log() {
        let pool = backup_pool().await;
        set_offsite(&pool).await;
        let rule = rules::get(&pool, "rule_backup_overdue")
            .await
            .unwrap()
            .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let floor = Path::new("/data/backups");
        // Off-Site konfiguriert, aber noch kein erfolgreiches Off-Site-Backup → überfällig.
        assert_eq!(
            backup_overdue(&pool, None, &rule, today, floor)
                .await
                .unwrap(),
            1
        );
        let (title, key): (String, String) =
            sqlx::query_as("SELECT title, dedup_key FROM notifications LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(title, "Off-Site-Backup überfällig");
        assert_eq!(key, "offsite_backup_overdue:2026-06-15");
    }

    #[tokio::test]
    async fn backup_overdue_offsite_silent_with_recent_log() {
        let pool = backup_pool().await;
        set_offsite(&pool).await;
        // Frisches erfolgreiches Off-Site-Backup (heute) → nicht überfällig.
        sqlx::query(
            "INSERT INTO backup_log (id, created_at, target_kind, status)
             VALUES ('l1', '2026-06-15 08:00:00', 'directory', 'ok')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Eine ältere/fehlgeschlagene Off-Site-Zeile darf nichts ändern.
        sqlx::query(
            "INSERT INTO backup_log (id, created_at, target_kind, status)
             VALUES ('l0', '2026-05-01 08:00:00', 'sftp', 'failed')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let rule = rules::get(&pool, "rule_backup_overdue")
            .await
            .unwrap()
            .unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let floor = Path::new("/data/backups");
        assert_eq!(
            backup_overdue(&pool, None, &rule, today, floor)
                .await
                .unwrap(),
            0
        );
    }
}
