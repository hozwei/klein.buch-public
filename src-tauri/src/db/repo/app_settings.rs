//! Schlüssel/Wert-Zugriff auf `app_settings` (Block 15).
//!
//! `app_settings` hält schema_version (Block 1) und ab Block 15 zusätzlich
//! Laufzeit-Schalter wie `depreciation_auto_year_close`. PRIMARY KEY ist `key`,
//! daher Upsert via `ON CONFLICT(key)`.

use crate::error::Result;
use sqlx::{Row, SqliteConnection, SqlitePool};

pub async fn get(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("value")))
}

/// Boolean-Setting; `"1"`/`"true"` ⇒ true. Fehlt der Schlüssel, gilt `default`.
pub async fn get_bool(pool: &SqlitePool, key: &str, default: bool) -> Result<bool> {
    Ok(get(pool, key)
        .await?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(default))
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_bool(pool: &SqlitePool, key: &str, value: bool) -> Result<()> {
    set(pool, key, if value { "1" } else { "0" }).await
}

/// Wie [`set`], innerhalb einer laufenden Transaktion.
pub async fn set_in(conn: &mut SqliteConnection, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(conn)
    .await?;
    Ok(())
}

/// Entfernt einen Laufzeit-Schlüssel (Marker, Backoff). `schema_version` und
/// andere Pflichtschlüssel nie über diesen Weg löschen.
pub async fn delete(pool: &SqlitePool, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM app_settings WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

/// Alle Schlüssel mit Präfix `prefix` (Marker je Vorlage), sortiert.
pub async fn list_prefix(pool: &SqlitePool, prefix: &str) -> Result<Vec<(String, String)>> {
    let pattern = format!("{}%", prefix.replace('%', "").replace('_', "\\_"));
    let rows = sqlx::query(
        "SELECT key, value FROM app_settings WHERE key LIKE ? ESCAPE '\\' ORDER BY key",
    )
    .bind(pattern)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
        .collect())
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
        sqlx::query(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn upsert_and_read() {
        let pool = pool().await;
        assert_eq!(get(&pool, "x").await.unwrap(), None);
        assert!(get_bool(&pool, "x", true).await.unwrap());
        set_bool(&pool, "x", false).await.unwrap();
        assert!(!get_bool(&pool, "x", true).await.unwrap());
        set(&pool, "x", "1").await.unwrap();
        assert!(get_bool(&pool, "x", false).await.unwrap());
        delete(&pool, "x").await.unwrap();
        assert_eq!(get(&pool, "x").await.unwrap(), None);
    }

    #[tokio::test]
    async fn prefix_listing_escapes_wildcards() {
        let pool = pool().await;
        set(&pool, "marker_a:1", "x").await.unwrap();
        set(&pool, "marker_a:2", "y").await.unwrap();
        set(&pool, "markerXa:3", "z").await.unwrap();
        let rows = list_prefix(&pool, "marker_a:").await.unwrap();
        assert_eq!(
            rows,
            vec![
                ("marker_a:1".to_string(), "x".to_string()),
                ("marker_a:2".to_string(), "y".to_string())
            ]
        );
    }
}
