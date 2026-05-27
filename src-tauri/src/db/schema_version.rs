//! Schema-Version-Check.
//!
//! `EXPECTED_SCHEMA_VERSION` ist in der Binary einkompiliert. Bei jedem App-Start
//! liest `check_compatible()` den aktuellen Wert aus `app_settings` und vergleicht.
//!
//! - Wert gleich → ok
//! - Wert kleiner → Migration-Runner hat eine Migration ausgelassen (sollte nicht passieren) → Error
//! - Wert größer → DB stammt von einer neueren App-Version → Error mit Update-Hint
//!
//! Bei jedem neuen Migrations-File MUSS die `INSERT INTO app_settings` für
//! `schema_version` aktualisiert werden, und `EXPECTED_SCHEMA_VERSION` hochgesetzt.

use crate::error::{Error, Result};
use sqlx::{Row, SqlitePool};

pub const EXPECTED_SCHEMA_VERSION: i32 = 30;

pub async fn check_compatible(pool: &SqlitePool) -> Result<()> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'schema_version'")
        .fetch_one(pool)
        .await?;
    let value: String = row.try_get("value")?;
    let found: i32 = value
        .parse()
        .map_err(|_| Error::Config(format!("schema_version ist kein Integer: '{value}'")))?;

    if found == EXPECTED_SCHEMA_VERSION {
        tracing::info!("Schema-Version {} ok.", found);
        Ok(())
    } else if found > EXPECTED_SCHEMA_VERSION {
        Err(Error::SchemaMismatch {
            expected: EXPECTED_SCHEMA_VERSION,
            found,
            hint: "Diese Klein.Buch-Version ist zu alt für deine Datenbank. \
                   Bitte aktualisiere die App."
                .into(),
        })
    } else {
        Err(Error::SchemaMismatch {
            expected: EXPECTED_SCHEMA_VERSION,
            found,
            hint: "Datenbank hat eine niedrigere Schema-Version als erwartet. \
                   Migration übersprungen? Prüfe Migrationen."
                .into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn check_passes_on_matching_version() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(&format!(
            "INSERT INTO app_settings (key, value) VALUES ('schema_version', '{}')",
            EXPECTED_SCHEMA_VERSION
        ))
        .execute(&pool)
        .await
        .unwrap();
        check_compatible(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn check_fails_on_too_new_db() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO app_settings (key, value) VALUES ('schema_version', '99')")
            .execute(&pool)
            .await
            .unwrap();
        let err = check_compatible(&pool).await.unwrap_err();
        match err {
            Error::SchemaMismatch {
                expected, found, ..
            } => {
                assert_eq!(expected, EXPECTED_SCHEMA_VERSION);
                assert_eq!(found, 99);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn it_compiles() {}
}
