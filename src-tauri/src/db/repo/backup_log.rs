//! Repository für das Backup-Protokoll `backup_log` (G1-LOG, ADR 0034).
//!
//! Schicht: **Imperative Shell**. Append-only — es gibt bewusst KEIN Update/Delete
//! (DB-Trigger erzwingen das zusätzlich). Geschrieben wird je Sicherungs-Ziel
//! genau einmal über [`insert`] (Floor + ggf. Off-Site-Spiegelung, jeweils
//! Erfolg/Fehler); gelesen für die Protokoll-Seite ([`list`] / [`search`]).
//!
//! Hard-Line: **niemals** die Passphrase oder ein anderes Geheimnis — `detail`
//! trägt nur Fehlertext.

use crate::db::models::BackupLogRow;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Eingabe für einen neuen Protokoll-Eintrag. `id` + `created_at` vergibt [`insert`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupLogEntry {
    /// Auslöser des Laufs: 'manual' | 'auto_daily' | 'auto_critical' | 'pre_restore'.
    pub trigger: String,
    /// Ziel-Typ: 'local' (Floor) | 'directory' (Off-Site-Ordner) | 'sftp'.
    pub target_kind: String,
    /// Optionaler Anzeigename (z. B. SFTP-Host); NULL für lokal/Ordner.
    pub target_label: Option<String>,
    pub file_name: String,
    /// Vollständiger Pfad bzw. `sftp://…`-URI.
    pub full_path: String,
    pub size_bytes: i64,
    /// 'ok' | 'failed'.
    pub status: String,
    /// Fehlertext bei `status='failed'`. **Niemals** ein Geheimnis.
    pub detail: Option<String>,
}

/// Schreibt einen Protokoll-Eintrag (append-only). UUIDv7 als ID, `created_at`
/// per DB-Default (UTC).
pub async fn insert(pool: &sqlx::SqlitePool, e: &BackupLogEntry) -> Result<String> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO backup_log (
            id, trigger, target_kind, target_label, file_name, full_path,
            size_bytes, status, detail
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&e.trigger)
    .bind(&e.target_kind)
    .bind(&e.target_label)
    .bind(&e.file_name)
    .bind(&e.full_path)
    .bind(e.size_bytes)
    .bind(&e.status)
    .bind(&e.detail)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Die jüngsten `limit` Einträge, neueste zuerst.
pub async fn list(pool: &sqlx::SqlitePool, limit: i64) -> Result<Vec<BackupLogRow>> {
    let rows = sqlx::query_as::<_, BackupLogRow>(
        "SELECT * FROM backup_log ORDER BY created_at DESC, id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Filter für die Protokoll-Suche. Alle Felder optional (UND-verknüpft).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupLogFilter {
    /// Volltext über Dateiname, Pfad, Ziel-Label, Fehlertext.
    #[serde(default)]
    pub search: Option<String>,
    /// Datum von (inkl.), Format `YYYY-MM-DD`, in lokaler Zeit.
    #[serde(default)]
    pub date_from: Option<String>,
    /// Datum bis (inkl.), Format `YYYY-MM-DD`, in lokaler Zeit.
    #[serde(default)]
    pub date_to: Option<String>,
    /// 'ok' | 'failed'
    #[serde(default)]
    pub status: Option<String>,
    /// 'manual' | 'auto_daily' | 'auto_critical' | 'pre_restore'
    #[serde(default)]
    pub trigger: Option<String>,
    /// 'local' | 'directory' | 'sftp'
    #[serde(default)]
    pub target_kind: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

fn clean(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Serverseitige Suche/Filterung über das Protokoll. Dynamische WHERE-Klausel via
/// `QueryBuilder` (parametrisiert, kein SQL-Injection-Risiko). Datumsgrenzen
/// werden gegen die LOKALE Zeit verglichen (`date(created_at, 'localtime')`),
/// passend zur Anzeige.
pub async fn search(pool: &sqlx::SqlitePool, f: &BackupLogFilter) -> Result<Vec<BackupLogRow>> {
    let mut qb: sqlx::QueryBuilder<sqlx::Sqlite> =
        sqlx::QueryBuilder::new("SELECT * FROM backup_log WHERE 1 = 1");

    if let Some(s) = clean(&f.search) {
        let pat = format!("%{s}%");
        qb.push(" AND (file_name LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR full_path LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR target_label LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR detail LIKE ");
        qb.push_bind(pat);
        qb.push(")");
    }
    if let Some(d) = clean(&f.date_from) {
        qb.push(" AND date(created_at, 'localtime') >= ");
        qb.push_bind(d);
    }
    if let Some(d) = clean(&f.date_to) {
        qb.push(" AND date(created_at, 'localtime') <= ");
        qb.push_bind(d);
    }
    if let Some(s) = clean(&f.status) {
        qb.push(" AND status = ");
        qb.push_bind(s);
    }
    if let Some(t) = clean(&f.trigger) {
        qb.push(" AND trigger = ");
        qb.push_bind(t);
    }
    if let Some(k) = clean(&f.target_kind) {
        qb.push(" AND target_kind = ");
        qb.push_bind(k);
    }

    qb.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    qb.push_bind(f.limit.unwrap_or(500).clamp(1, 5000));

    let rows = qb.build_query_as::<BackupLogRow>().fetch_all(pool).await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
