//! Schlüssel/Wert-Zugriff auf `app_settings` (Block 15).
//!
//! `app_settings` hält schema_version (Block 1) und ab Block 15 zusätzlich
//! Laufzeit-Schalter wie `depreciation_auto_year_close`. PRIMARY KEY ist `key`,
//! daher Upsert via `ON CONFLICT(key)`.

use crate::error::Result;
use sqlx::{Row, SqlitePool};

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
    }
}
