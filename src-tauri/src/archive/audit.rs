//! Strukturierter Wrapper um [`crate::db::repo::audit_log::append`] für
//! Archive-Events.
//!
//! GoBD-Hardline: jede Archive-Operation (Store, Read, Integrity-Pass,
//! Integrity-Fail) erzeugt **genau einen** Audit-Log-Eintrag. Der zugehörige
//! `audit_log`-DB-Trigger schützt die Einträge danach gegen Update/Delete.

use crate::{db::repo::audit_log, error::Result};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveAction {
    Store,
    Read,
    IntegrityPass,
    IntegrityFail,
    /// G1-HARDEN.4: die archivierte Datei fehlt (verwaister `archive_entries`-
    /// Eintrag). Eigene Audit-Kategorie, damit eine **gelöschte/verschobene**
    /// Datei nachvollziehbar von einer **manipulierten** (Hash-Mismatch) Datei
    /// getrennt bleibt.
    IntegrityMissing,
}

impl ArchiveAction {
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiveAction::Store => "archive.store",
            ArchiveAction::Read => "archive.read",
            ArchiveAction::IntegrityPass => "archive.integrity_pass",
            ArchiveAction::IntegrityFail => "archive.integrity_fail",
            ArchiveAction::IntegrityMissing => "archive.integrity_missing",
        }
    }
}

pub async fn archive_event(
    pool: &SqlitePool,
    action: ArchiveAction,
    archive_id: &str,
    details_json: Option<&str>,
) -> Result<()> {
    audit_log::append(
        pool,
        action.as_str(),
        "archive_entry",
        archive_id,
        details_json,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Row;

    async fn pool_with_audit() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system',
                action TEXT NOT NULL,
                entity_type TEXT,
                entity_id TEXT,
                details_json TEXT
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn each_action_emits_distinct_log_string() {
        let pool = pool_with_audit().await;
        for a in [
            ArchiveAction::Store,
            ArchiveAction::Read,
            ArchiveAction::IntegrityPass,
            ArchiveAction::IntegrityFail,
            ArchiveAction::IntegrityMissing,
        ] {
            archive_event(&pool, a, "01900000-0000-7000-8000-000000000000", None)
                .await
                .unwrap();
        }
        let rows = sqlx::query("SELECT action FROM audit_log ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        let actions: Vec<String> = rows.iter().map(|r| r.get("action")).collect();
        assert_eq!(
            actions,
            vec![
                "archive.store",
                "archive.read",
                "archive.integrity_pass",
                "archive.integrity_fail",
                "archive.integrity_missing",
            ]
        );
    }
}
