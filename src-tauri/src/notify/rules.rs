//! Reminder-Regel-Konfiguration (Block 15).
//!
//! Regeln werden deterministisch per Migration `0012_notifications.sql` geseedet.
//! Der User kann sie im UI ein-/ausschalten; die Schedule-Parameter liegen als
//! JSON in `config_json` (z. B. `{"day_of_month":10}`). Der Scheduler liest die
//! aktiven Regeln und erzeugt daraus Notifications (siehe
//! [`crate::scheduler::reminders`]).

use crate::error::Result;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRule {
    pub id: String,
    pub rule_type: String,
    pub label: String,
    pub enabled: i64,
    pub config_json: String,
    pub deliver_in_app: i64,
    pub deliver_os_native: i64,
    pub created_at: String,
}

impl NotificationRule {
    pub fn is_enabled(&self) -> bool {
        self.enabled != 0
    }
    /// Liest einen i64-Parameter aus `config_json` mit Fallback.
    pub fn config_i64(&self, key: &str, default: i64) -> i64 {
        serde_json::from_str::<serde_json::Value>(&self.config_json)
            .ok()
            .and_then(|v| v.get(key).and_then(|x| x.as_i64()))
            .unwrap_or(default)
    }
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<NotificationRule>> {
    let rows = sqlx::query_as::<_, NotificationRule>(
        "SELECT * FROM notification_rules ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<NotificationRule>> {
    let row =
        sqlx::query_as::<_, NotificationRule>("SELECT * FROM notification_rules WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// Holt die (erste) aktive Regel eines Typs. `None`, wenn keine existiert oder
/// die Regel deaktiviert ist.
pub async fn get_enabled_by_type(
    pool: &SqlitePool,
    rule_type: &str,
) -> Result<Option<NotificationRule>> {
    let row = sqlx::query_as::<_, NotificationRule>(
        "SELECT * FROM notification_rules
          WHERE rule_type = ? AND enabled = 1
          ORDER BY id ASC LIMIT 1",
    )
    .bind(rule_type)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> Result<()> {
    sqlx::query("UPDATE notification_rules SET enabled = ? WHERE id = ?")
        .bind(i64::from(enabled))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool_with_rule() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE notification_rules (
                id TEXT PRIMARY KEY NOT NULL,
                rule_type TEXT NOT NULL,
                label TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config_json TEXT NOT NULL,
                deliver_in_app INTEGER NOT NULL DEFAULT 1,
                deliver_os_native INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO notification_rules (id, rule_type, label, enabled, config_json)
             VALUES ('rule_monthly_doc_check','monthly_doc_check','Monats-Doku-Check',1,'{\"day_of_month\":10}')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn reads_config_param_with_fallback() {
        let pool = pool_with_rule().await;
        let r = get(&pool, "rule_monthly_doc_check").await.unwrap().unwrap();
        assert_eq!(r.config_i64("day_of_month", 1), 10);
        assert_eq!(r.config_i64("missing", 42), 42);
        assert!(r.is_enabled());
    }

    #[tokio::test]
    async fn disabling_hides_from_enabled_lookup() {
        let pool = pool_with_rule().await;
        set_enabled(&pool, "rule_monthly_doc_check", false)
            .await
            .unwrap();
        let r = get_enabled_by_type(&pool, "monthly_doc_check")
            .await
            .unwrap();
        assert!(r.is_none());
    }
}
