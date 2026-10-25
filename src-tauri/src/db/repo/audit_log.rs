//! Append-only Audit-Log-Repository.
//!
//! Insert-only Interface — der DB-Trigger `trg_audit_no_update` /
//! `trg_audit_no_delete` schützt zusätzlich gegen versehentliche
//! Modifikationen. Niemals Passphrasen, Tokens, Klartext-Credentials
//! in `details_json` schreiben.
//!
//! **Details** (Lauf F, W14-010/R2-S2-AUDITJSON) nimmt die Schnittstelle nur
//! als [`serde_json::Value`] entgegen und serialisiert sie selbst: Anführungs-
//! und Steuerzeichen in Namen, Gründen oder Pfaden ergeben immer gültiges JSON
//! (kein handgebautes `format!`, keine Log-Injektion über `details_json`).
//!
//! **Akteur** (Lauf F, R1-S3-ACTOR): `owner` für Aufrufe über die Sitzung des
//! Inhabers (Commands und Jobs, gesetzt vom Dispatcher), `scheduler` für die
//! Abläufe des Schedulers, sonst `system` (Einrichtung, Entsperren, Start,
//! Restore-Nacharbeit). Gesetzt über [`with_actor`] als Task-lokaler Wert; ein
//! eigener `tokio::spawn` innerhalb eines Ablaufs fällt auf `system` zurück.

use crate::error::Result;
use sqlx::{SqliteConnection, SqlitePool};
use std::future::Future;

/// Akteur: Aufruf über die angemeldete Sitzung des Inhabers.
pub const ACTOR_OWNER: &str = "owner";
/// Akteur: automatischer Ablauf des Schedulers (Tick).
pub const ACTOR_SCHEDULER: &str = "scheduler";
/// Akteur: alles übrige (Einrichtung, Entsperren, Start, Restore-Nacharbeit).
pub const ACTOR_SYSTEM: &str = "system";

tokio::task_local! {
    static ACTOR: &'static str;
}

/// Führt `fut` mit `actor` als Akteur aller darin geschriebenen
/// Audit-Einträge aus.
pub async fn with_actor<F: Future>(actor: &'static str, fut: F) -> F::Output {
    ACTOR.scope(actor, fut).await
}

/// Akteur des laufenden Tasks (Standard [`ACTOR_SYSTEM`]).
pub fn current_actor() -> &'static str {
    ACTOR.try_with(|a| *a).unwrap_or(ACTOR_SYSTEM)
}

/// Hängt einen Eintrag an; `actor` aus [`current_actor`].
pub async fn append(
    pool: &SqlitePool,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    details: Option<serde_json::Value>,
) -> Result<()> {
    let details = details.map(|d| d.to_string());
    sqlx::query(
        "INSERT INTO audit_log (actor, action, entity_type, entity_id, details_json)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(current_actor())
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

/// Wie [`append`], innerhalb einer laufenden Transaktion (Beleg und Audit
/// gemeinsam committet, Lauf F).
pub async fn append_in(
    conn: &mut SqliteConnection,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    details: Option<serde_json::Value>,
) -> Result<()> {
    let details = details.map(|d| d.to_string());
    sqlx::query(
        "INSERT INTO audit_log (actor, action, entity_type, entity_id, details_json)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(current_actor())
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(details)
    .execute(conn)
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
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
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

    /// Lauf F (R1-S3-ACTOR): Akteur aus dem Task-Kontext, sonst `system`.
    #[tokio::test]
    async fn actor_follows_the_task_context() {
        let pool = pool().await;
        append(&pool, "a.plain", "x", "1", None).await.unwrap();
        with_actor(ACTOR_OWNER, append(&pool, "a.owner", "x", "1", None))
            .await
            .unwrap();
        with_actor(ACTOR_SCHEDULER, async {
            let mut conn = pool.acquire().await.unwrap();
            append_in(&mut conn, "a.scheduler", "x", "1", None).await
        })
        .await
        .unwrap();
        // Ein eigener Task erbt den Akteur nicht.
        let p = pool.clone();
        with_actor(ACTOR_OWNER, async move {
            tokio::spawn(async move { append(&p, "a.spawned", "x", "1", None).await })
                .await
                .unwrap()
        })
        .await
        .unwrap();
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT action, actor FROM audit_log ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                ("a.plain".into(), "system".into()),
                ("a.owner".into(), "owner".into()),
                ("a.scheduler".into(), "scheduler".into()),
                ("a.spawned".into(), "system".into()),
            ]
        );
    }

    /// Lauf F (W14-010): Anführungs-, Steuer- und Unicode-Zeichen ergeben
    /// gültiges JSON mit unverändertem Inhalt.
    #[tokio::test]
    async fn details_are_serialized_as_valid_json() {
        let pool = pool().await;
        let odd = "Grund \"zitiert\"\n\tTab \\ Backslash \u{1} Größe ✓";
        append(
            &pool,
            "a.odd",
            "x",
            "1",
            Some(serde_json::json!({ "reason": odd, "n": 3 })),
        )
        .await
        .unwrap();
        let (details, valid): (String, i64) = sqlx::query_as(
            "SELECT details_json, json_valid(details_json) FROM audit_log WHERE action = 'a.odd'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(valid, 1, "{details}");
        let v: serde_json::Value = serde_json::from_str(&details).unwrap();
        assert_eq!(v["reason"], odd);
        assert_eq!(v["n"], 3);
    }
}
