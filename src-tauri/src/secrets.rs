//! Zugangsdaten (SMTP-/SFTP-Passwörter, OAuth-Refresh-Token) in der
//! SQLCipher-Datenbank (Plan W8, Amendment ADR 0011/0028, Migration 0031).
//!
//! Bis W7 lagen sie im OS-Schlüsselbund (`keyring`), den es im Container nicht
//! gibt. Jetzt: Tabelle `secrets` (STRICT), verschlüsselt mit der DB
//! (Passphrase = DB-Key, ADR 0035). Werte erscheinen nie in Log, Audit,
//! Migrations- oder DSGVO-Export; der Pool muss offen sein (entsperrt).

use crate::error::Result;
use sqlx::SqlitePool;
use zeroize::Zeroizing;

/// Schlüssel des SMTP-Passworts eines Mailkontos.
pub fn smtp_key(account_id: &str) -> String {
    format!("smtp:{account_id}")
}

/// Schlüssel des OAuth-Refresh-Tokens eines Mailkontos.
pub fn oauth_key(account_id: &str) -> String {
    format!("oauth:{account_id}")
}

/// Schlüssel des SFTP-Passworts eines Backup-Ziels.
pub fn sftp_key(target_id: &str) -> String {
    format!("sftp:{target_id}")
}

/// Liest einen Eintrag. Der Wert kommt als `Zeroizing` zurück und wird beim
/// Drop überschrieben (R4-S2-ZEROIZE, Lauf F4a); Aufrufer reichen ihn nur als
/// `&str` weiter. Puffer von SQLite/sqlx und der Gegenstelle (lettre, russh)
/// liegen außerhalb dieser Kontrolle (security.md §4).
pub async fn get(pool: &SqlitePool, key: &str) -> Result<Option<Zeroizing<String>>> {
    Ok(
        sqlx::query_scalar::<_, String>("SELECT value FROM secrets WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?
            .map(Zeroizing::new),
    )
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO secrets (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                        updated_at = datetime('now','utc')",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Löscht einen Eintrag (fehlender Eintrag ist kein Fehler).
pub async fn delete(pool: &SqlitePool, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM secrets WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::MIGRATOR.run(&pool).await.unwrap();
        assert_eq!(get(&pool, &smtp_key("a")).await.unwrap(), None);
        set(&pool, &smtp_key("a"), "geheim").await.unwrap();
        set(&pool, &smtp_key("a"), "neu").await.unwrap();
        assert_eq!(
            get(&pool, &smtp_key("a"))
                .await
                .unwrap()
                .as_deref()
                .map(String::as_str),
            Some("neu")
        );
        delete(&pool, &smtp_key("a")).await.unwrap();
        delete(&pool, &smtp_key("a")).await.unwrap();
        assert_eq!(get(&pool, &smtp_key("a")).await.unwrap(), None);
        assert_eq!(sftp_key("t"), "sftp:t");
        assert_eq!(oauth_key("m"), "oauth:m");
    }
}
