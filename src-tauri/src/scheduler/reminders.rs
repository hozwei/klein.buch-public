//! Reminder-Rule-Engine (Block 15).
//!
//! Wird vom [`crate::scheduler::tick`] bei jedem Tick aufgerufen. Für jede aktive
//! Regel wird geprüft, ob ein Hinweis fällig ist; Dedup über `dedup_key` sorgt
//! dafür, dass jeder periodische Hinweis pro Periode nur **einmal** entsteht
//! (auch wenn der Tick alle 5 Minuten feuert).
//!
//! Manuel-Auswahl Block 15: monthly_doc_check, fiscal_year_lock_pending,
//! backup_overdue, invoice_overdue (alle vier aktiv).
//!
//! Lauf F (D-01, ADR 0040): Kosten-Abos im Modus „Erinnerung“
//! (`auto_create_expense = 0`) erzeugen am Stichtag einen Hinweis „Abo fällig“
//! (einmal je Abo und Stichtag). Die abschaltbare Regel `rule_recurring_due`
//! (rule_type `recurring_due`) seedet Migration 0032.

use crate::error::Result;
use crate::fiscal_year::guard;
use crate::notify::{self, rules, NewNotification};
use chrono::{Datelike, NaiveDate};
use sqlx::SqlitePool;
use std::path::Path;

/// Führt alle aktiven Reminder-Regeln aus. Liefert die Anzahl neu erzeugter
/// Hinweise (Dedup-Treffer zählen nicht).
///
/// `targets_root` ist die Wurzel der Verzeichnis-Ziele
/// (`paths.backup_targets_dir`); die `backup_overdue`-Regel braucht sie, um die
/// konfigurierten Off-Site-Ziele zu lesen (G1-NOTIFY, Plan W7).
pub async fn run(pool: &SqlitePool, today: NaiveDate, targets_root: &Path) -> Result<usize> {
    let mut created = 0usize;

    if let Some(rule) = rules::get_enabled_by_type(pool, "monthly_doc_check").await? {
        created += monthly_doc_check(pool, &rule, today).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "fiscal_year_lock_pending").await? {
        created += fiscal_year_lock_pending(pool, &rule, today).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "backup_overdue").await? {
        created += backup_overdue(pool, &rule, today, targets_root).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "invoice_overdue").await? {
        created += invoice_overdue(pool, &rule, today).await?;
    }
    if let Some(rule) = rules::get_enabled_by_type(pool, "recurring_due").await? {
        created += recurring_due(pool, &rule, today).await?;
    }
    // ADR 0042 Nr. 5: Umsatzgrenzen nach §19 (Regel `rule_tax_threshold`, prüft
    // selbst, ob sie aktiv ist). Ein Fehler hier hält die übrigen Hinweise nicht auf.
    match crate::commands::tax_status::notify_thresholds(pool, today).await {
        Ok(n) => created += n,
        Err(e) => tracing::warn!("Grenzprüfung nach §19 fehlgeschlagen: {e}"),
    }
    Ok(created)
}

/// „Abo fällig“ für Kosten-Abos im Modus Erinnerung. Dedup je Abo und Stichtag;
/// der Stichtag rückt erst mit „Buchen“ (`recurring_run_now`) vor.
async fn recurring_due(
    pool: &SqlitePool,
    rule: &rules::NotificationRule,
    today: NaiveDate,
) -> Result<usize> {
    let due = crate::db::repo::recurring::list_due_reminders(pool, &today.to_string()).await?;
    let mut created = 0usize;
    for sub in &due {
        let date = NaiveDate::parse_from_str(&sub.next_due_date, "%Y-%m-%d")
            .map(|d| d.format("%d.%m.%Y").to_string())
            .unwrap_or_else(|_| sub.next_due_date.clone());
        let body = format!(
            "„{}“ ist seit {date} fällig (erwartet {}). Die Kosten-Position legst du unter \
             Kosten → Wiederkehrende Abos mit „Buchen“ an.",
            sub.label,
            euro(sub.expected_amount_cents)
        );
        let key = format!("recurring_due:{}:{}", sub.id, sub.next_due_date);
        created += emit_one(
            pool,
            NewNotification {
                rule_id: Some(&rule.id),
                title: "Abo fällig",
                body: &body,
                severity: "info",
                related_entity_type: Some("recurring"),
                related_entity_id: Some(&sub.id),
                action_url: Some("/expenses/recurring"),
                dedup_key: Some(&key),
            },
        )
        .await?;
    }
    Ok(created)
}

/// Cent → „1.234,56 €“.
fn euro(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let mut euros = (abs / 100).to_string();
    let mut grouped = String::new();
    while euros.len() > 3 {
        let split = euros.len() - 3;
        grouped = format!(".{}{grouped}", &euros[split..]);
        euros.truncate(split);
    }
    format!(
        "{}{euros}{grouped},{:02} €",
        if neg { "-" } else { "" },
        abs % 100
    )
}

async fn emit_one(pool: &SqlitePool, n: NewNotification<'_>) -> Result<usize> {
    Ok(notify::emit(pool, n).await?.map(|_| 1).unwrap_or(0))
}

/// Monatlicher Doku-Check ab Tag N (Default 10.). Dedup pro Kalendermonat.
async fn monthly_doc_check(
    pool: &SqlitePool,
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
/// - Ist ein **Off-Site-Ziel** konfiguriert (Ordner unter `/backup-targets` oder SFTP, vom Floor
///   verschieden), zählt das letzte **erfolgreiche Off-Site-Backup** aus dem
///   append-only `backup_log` (G1-LOG). Der lokale Floor ist ohnehin fast immer
///   frisch (Lock-/Tages-Backup) — die Off-Site-Kopie ist das, was ausfallen kann.
/// - Ohne Off-Site-Ziel bleibt es beim lokalen Floor (`backup_history`): warnt,
///   wenn überhaupt keine Sicherung mehr lief (z. B. Tages-Backup scheitert).
///
/// Das Alter rechnet in Europe/Berlin (Lauf F, D-08): der UTC-Zeitstempel
/// wird erst umgerechnet, sonst lag ein Backup zwischen 0 und 2 Uhr einen Tag
/// zu früh. Eine Lücke durch lange Sperre meldet das Entsperren selbst
/// (`commands::backup::locked_gap_notice`).
async fn backup_overdue(
    pool: &SqlitePool,
    rule: &rules::NotificationRule,
    today: NaiveDate,
    targets_root: &Path,
) -> Result<usize> {
    let max_age = rule.config_i64("max_age_days", 7);
    let has_offsite = !crate::backup::target::list_targets(pool, targets_root)
        .await?
        .is_empty();

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
                 (oder noch keins). Prüfe deine Backup-Ziele (Ordner unter /backup-targets oder SFTP)."
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
        Some(ts) => match crate::clock::db_utc_to_berlin(ts) {
            Some(b) => (today - b.date_naive()).num_days() > max_age,
            None => false,
        },
    };
    if !overdue {
        return Ok(0);
    }
    emit_one(
        pool,
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
        assert_eq!(monthly_doc_check(&pool, &rule, day15).await.unwrap(), 1);
        // zweiter Lauf im selben Monat: Dedup
        assert_eq!(monthly_doc_check(&pool, &rule, day15).await.unwrap(), 0);
        // vor dem Stichtag: nichts
        let day5 = NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();
        assert_eq!(monthly_doc_check(&pool, &rule, day5).await.unwrap(), 0);
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
        sqlx::query("INSERT INTO app_settings (key, value) VALUES ('backup_targets', ?)")
            .bind(r#"[{"id":"a","label":"NAS","kind":"directory","path":"nas/kb"}]"#)
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
        let floor = Path::new("/backup-targets");
        // Kein Off-Site-Ziel, kein Backup → überfällig über den Floor-Pfad.
        assert_eq!(backup_overdue(&pool, &rule, today, floor).await.unwrap(), 1);
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
        let floor = Path::new("/backup-targets");
        // Off-Site konfiguriert, aber noch kein erfolgreiches Off-Site-Backup → überfällig.
        assert_eq!(backup_overdue(&pool, &rule, today, floor).await.unwrap(), 1);
        let (title, key): (String, String) =
            sqlx::query_as("SELECT title, dedup_key FROM notifications LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(title, "Off-Site-Backup überfällig");
        assert_eq!(key, "offsite_backup_overdue:2026-06-15");
    }

    /// D-08: Ein Backup um 23:30 UTC gehört zum Berliner Folgetag. Am
    /// 15.06. mit Grenze 7 Tage ist ein Backup vom 07.06. 23:30 UTC
    /// (= 08.06. 01:30 Berlin) noch nicht überfällig.
    #[tokio::test]
    async fn backup_overdue_uses_berlin_date_of_utc_timestamp() {
        let pool = backup_pool().await;
        sqlx::query(
            "INSERT INTO backup_history (id, created_at) VALUES ('b1', '2026-06-07 23:30:00')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let rule = rules::get(&pool, "rule_backup_overdue")
            .await
            .unwrap()
            .unwrap();
        let floor = Path::new("/backup-targets");
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        assert_eq!(
            backup_overdue(&pool, &rule, today, floor).await.unwrap(),
            0,
            "Berliner Tag 08.06. → 7 Tage, nicht überfällig"
        );
        let next = NaiveDate::from_ymd_opt(2026, 6, 16).unwrap();
        assert_eq!(backup_overdue(&pool, &rule, next, floor).await.unwrap(), 1);
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
        let floor = Path::new("/backup-targets");
        assert_eq!(backup_overdue(&pool, &rule, today, floor).await.unwrap(), 0);
    }
}
