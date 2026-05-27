//! Persistente In-App-Inbox (Block 15).
//!
//! Die Inbox ist die Quelle der Wahrheit für Hinweise. OS-native Pushes sind nur
//! ein zusätzlicher Kanal (siehe [`crate::notify::os_native`]). Einträge werden
//! nicht hart gelöscht, sondern als „gelesen" (`dismissed_at`) markiert.
//!
//! Dedup: über den optionalen `dedup_key` (Unique-Index) wird verhindert, dass
//! derselbe periodische Reminder mehrfach erzeugt wird (z. B.
//! `"monthly_doc_check:2026-06"`).

use crate::error::Result;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub rule_id: Option<String>,
    pub title: String,
    pub body: String,
    pub severity: String,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<String>,
    pub triggered_at: String,
    pub dismissed_at: Option<String>,
    pub action_url: Option<String>,
    pub dedup_key: Option<String>,
}

/// Eingabe für eine neue Notification. Referenzen, damit Aufrufer nichts klonen.
#[derive(Debug, Clone)]
pub struct NewNotification<'a> {
    pub rule_id: Option<&'a str>,
    pub title: &'a str,
    pub body: &'a str,
    /// 'info' | 'warning' | 'urgent'
    pub severity: &'a str,
    pub related_entity_type: Option<&'a str>,
    pub related_entity_id: Option<&'a str>,
    pub action_url: Option<&'a str>,
    pub dedup_key: Option<&'a str>,
}

// Manuelles Default (kein derive): `&str` als Feld-Default ist über String-Literale
// garantiert; `severity` defaultet auf "info".
impl<'a> Default for NewNotification<'a> {
    fn default() -> Self {
        Self {
            rule_id: None,
            title: "",
            body: "",
            severity: "info",
            related_entity_type: None,
            related_entity_id: None,
            action_url: None,
            dedup_key: None,
        }
    }
}

/// Legt eine Notification an. Mit `dedup_key` idempotent: existiert bereits eine
/// Zeile mit gleichem Schlüssel, wird nichts angelegt und `Ok(None)` geliefert.
/// Ohne `dedup_key` wird immer eingefügt.
pub async fn create(pool: &SqlitePool, n: NewNotification<'_>) -> Result<Option<Notification>> {
    let id = Uuid::now_v7().to_string();
    let res = sqlx::query(
        "INSERT OR IGNORE INTO notifications
            (id, rule_id, title, body, severity, related_entity_type,
             related_entity_id, action_url, dedup_key)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(n.rule_id)
    .bind(n.title)
    .bind(n.body)
    .bind(n.severity)
    .bind(n.related_entity_type)
    .bind(n.related_entity_id)
    .bind(n.action_url)
    .bind(n.dedup_key)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Ok(None); // Dedup-Treffer
    }
    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Notification>> {
    let row = sqlx::query_as::<_, Notification>("SELECT * FROM notifications WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Liste der Hinweise, neueste zuerst. `include_dismissed=false` blendet
/// abgehakte aus (Standard-Inbox-Ansicht).
pub async fn list(pool: &SqlitePool, include_dismissed: bool) -> Result<Vec<Notification>> {
    let sql = if include_dismissed {
        "SELECT * FROM notifications ORDER BY triggered_at DESC, id DESC LIMIT 500"
    } else {
        "SELECT * FROM notifications WHERE dismissed_at IS NULL
         ORDER BY triggered_at DESC, id DESC LIMIT 500"
    };
    let rows = sqlx::query_as::<_, Notification>(sql)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Anzahl ungelesener (nicht abgehakter) Hinweise — für das Badge.
pub async fn count_unread(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM notifications WHERE dismissed_at IS NULL")
        .fetch_one(pool)
        .await?;
    Ok(row.try_get::<i64, _>("n")?)
}

pub async fn dismiss(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE notifications SET dismissed_at = datetime('now','utc')
         WHERE id = ? AND dismissed_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn dismiss_all(pool: &SqlitePool) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE notifications SET dismissed_at = datetime('now','utc')
         WHERE dismissed_at IS NULL",
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE notifications (
                id TEXT PRIMARY KEY NOT NULL,
                rule_id TEXT,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                severity TEXT NOT NULL,
                related_entity_type TEXT,
                related_entity_id TEXT,
                triggered_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
                dismissed_at TEXT,
                action_url TEXT,
                dedup_key TEXT
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE UNIQUE INDEX uq_notifications_dedup
             ON notifications(dedup_key) WHERE dedup_key IS NOT NULL",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn sample<'a>(title: &'a str, dedup: Option<&'a str>) -> NewNotification<'a> {
        NewNotification {
            rule_id: None,
            title,
            body: "Test-Hinweis",
            severity: "info",
            related_entity_type: None,
            related_entity_id: None,
            action_url: None,
            dedup_key: dedup,
        }
    }

    #[tokio::test]
    async fn create_and_count_unread() {
        let pool = fresh_pool().await;
        let n = create(&pool, sample("Erster", None)).await.unwrap();
        assert!(n.is_some());
        assert_eq!(count_unread(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn dedup_key_prevents_duplicates() {
        let pool = fresh_pool().await;
        let first = create(&pool, sample("Doku", Some("monthly:2026-06")))
            .await
            .unwrap();
        assert!(first.is_some(), "erste Anlage erfolgreich");
        let second = create(&pool, sample("Doku", Some("monthly:2026-06")))
            .await
            .unwrap();
        assert!(second.is_none(), "zweite Anlage per Dedup unterdrückt");
        assert_eq!(count_unread(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn dismiss_reduces_unread_but_keeps_row() {
        let pool = fresh_pool().await;
        let n = create(&pool, sample("Weg", None)).await.unwrap().unwrap();
        dismiss(&pool, &n.id).await.unwrap();
        assert_eq!(count_unread(&pool).await.unwrap(), 0);
        // Zeile bleibt erhalten (Historie), taucht nur in include_dismissed auf.
        assert_eq!(list(&pool, false).await.unwrap().len(), 0);
        assert_eq!(list(&pool, true).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn dismiss_all_marks_everything_read() {
        let pool = fresh_pool().await;
        create(&pool, sample("A", None)).await.unwrap();
        create(&pool, sample("B", None)).await.unwrap();
        let n = dismiss_all(&pool).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(count_unread(&pool).await.unwrap(), 0);
    }
}
