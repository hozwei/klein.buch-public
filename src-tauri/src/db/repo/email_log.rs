//! Repository für das E-Mail-Versandprotokoll `email_log` (Block 16b).
//!
//! Schicht: **Imperative Shell**. Append-only — es gibt bewusst KEIN Update/Delete
//! (DB-Trigger erzwingen das zusätzlich). Geschrieben wird je Versandversuch genau
//! einmal über [`insert`]; gelesen für die Protokoll-Seite ([`list`]) und die
//! Versand-Historie eines Belegs ([`list_for`]).

use crate::db::models::EmailLogRow;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Eingabe für einen neuen Protokoll-Eintrag. `id` + `created_at` vergibt [`insert`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailLogEntry {
    pub account_id: Option<String>,
    pub account_label: Option<String>,
    pub channel: String,
    pub related_kind: String,
    pub related_id: Option<String>,
    pub related_number: Option<String>,
    pub from_email: String,
    pub to_email: String,
    pub subject: String,
    pub attachment_count: i64,
    pub status: String,
    pub provider_code: Option<String>,
    pub provider_message: Option<String>,
    pub request_id: Option<String>,
    pub error: Option<String>,
}

/// Schreibt einen Protokoll-Eintrag (append-only). UUIDv7 als ID, `created_at`
/// per DB-Default (UTC).
pub async fn insert(pool: &sqlx::SqlitePool, e: &EmailLogEntry) -> Result<String> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO email_log (
            id, account_id, account_label, channel, related_kind, related_id,
            related_number, from_email, to_email, subject, attachment_count,
            status, provider_code, provider_message, request_id, error
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&e.account_id)
    .bind(&e.account_label)
    .bind(&e.channel)
    .bind(&e.related_kind)
    .bind(&e.related_id)
    .bind(&e.related_number)
    .bind(&e.from_email)
    .bind(&e.to_email)
    .bind(&e.subject)
    .bind(e.attachment_count)
    .bind(&e.status)
    .bind(&e.provider_code)
    .bind(&e.provider_message)
    .bind(&e.request_id)
    .bind(&e.error)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Die jüngsten `limit` Einträge, neueste zuerst.
pub async fn list(pool: &sqlx::SqlitePool, limit: i64) -> Result<Vec<EmailLogRow>> {
    let rows = sqlx::query_as::<_, EmailLogRow>(
        "SELECT * FROM email_log ORDER BY created_at DESC, id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Filter für die Protokoll-Suche (Block 16b). Alle Felder optional (UND-verknüpft).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailLogFilter {
    /// Volltext über Empfänger, Betreff, Beleg-Nr., Konto-Label, Fehler, request-id.
    #[serde(default)]
    pub search: Option<String>,
    /// Datum von (inkl.), Format `YYYY-MM-DD`, in lokaler Zeit.
    #[serde(default)]
    pub date_from: Option<String>,
    /// Datum bis (inkl.), Format `YYYY-MM-DD`, in lokaler Zeit.
    #[serde(default)]
    pub date_to: Option<String>,
    /// 'success' | 'failed'
    #[serde(default)]
    pub status: Option<String>,
    /// 'invoice' | 'quote' | 'test'
    #[serde(default)]
    pub kind: Option<String>,
    /// 'smtp' | 'graph'
    #[serde(default)]
    pub channel: Option<String>,
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
pub async fn search(pool: &sqlx::SqlitePool, f: &EmailLogFilter) -> Result<Vec<EmailLogRow>> {
    let mut qb: sqlx::QueryBuilder<sqlx::Sqlite> =
        sqlx::QueryBuilder::new("SELECT * FROM email_log WHERE 1 = 1");

    if let Some(s) = clean(&f.search) {
        let pat = format!("%{s}%");
        qb.push(" AND (to_email LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR subject LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR related_number LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR account_label LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR error LIKE ");
        qb.push_bind(pat.clone());
        qb.push(" OR request_id LIKE ");
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
    if let Some(k) = clean(&f.kind) {
        qb.push(" AND related_kind = ");
        qb.push_bind(k);
    }
    if let Some(c) = clean(&f.channel) {
        qb.push(" AND channel = ");
        qb.push_bind(c);
    }

    qb.push(" ORDER BY created_at DESC, id DESC LIMIT ");
    qb.push_bind(f.limit.unwrap_or(500).clamp(1, 5000));

    let rows = qb.build_query_as::<EmailLogRow>().fetch_all(pool).await?;
    Ok(rows)
}

/// Versand-Historie eines konkreten Belegs (z. B. ('invoice', invoice_id)).
pub async fn list_for(
    pool: &sqlx::SqlitePool,
    related_kind: &str,
    related_id: &str,
) -> Result<Vec<EmailLogRow>> {
    let rows = sqlx::query_as::<_, EmailLogRow>(
        "SELECT * FROM email_log
         WHERE related_kind = ? AND related_id = ?
         ORDER BY created_at DESC, id DESC",
    )
    .bind(related_kind)
    .bind(related_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
