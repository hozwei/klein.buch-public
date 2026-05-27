//! Append-only Audit-Log-Repository.
//!
//! Insert-only Interface — der DB-Trigger `trg_audit_no_update` /
//! `trg_audit_no_delete` schützt zusätzlich gegen versehentliche
//! Modifikationen. Niemals Passphrasen, Tokens, Klartext-Credentials
//! in `details_json` schreiben.

use crate::error::Result;
use sqlx::SqlitePool;

/// Hängt einen Eintrag an. `actor` wird als "system" gesetzt (Single-User-App).
pub async fn append(
    pool: &SqlitePool,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    details_json: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_log (actor, action, entity_type, entity_id, details_json)
         VALUES ('system', ?, ?, ?, ?)",
    )
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(details_json)
    .execute(pool)
    .await?;
    Ok(())
}

/// Liefert die jüngsten Audit-Einträge (Phase 2D baut ein vollständiges
/// Read-Model auf; Block 2 nutzt das nur für Tests).
pub async fn recent(pool: &SqlitePool, limit: i64) -> Result<Vec<AuditEntry>> {
    let rows: Vec<AuditEntry> = sqlx::query_as(
        "SELECT id, timestamp_utc, actor, action, entity_type, entity_id, details_json
         FROM audit_log
         ORDER BY id DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub timestamp_utc: String,
    pub actor: String,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub details_json: Option<String>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
