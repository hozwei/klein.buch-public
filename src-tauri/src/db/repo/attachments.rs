//! Repository für `attachments` (Block 6).
//!
//! Schicht: **Imperative Shell**. Ein `attachments`-Eintrag verknüpft eine
//! Eltern-Entität (`parent_type` + `parent_id`) mit einem write-once
//! archivierten File (`archive_entries`). Erst-Anwendung: der unterschriebene
//! Vertrag beim Angebots-Annahme-Workflow (`parent_type='quote'`).
//!
//! Die Datei selbst wird über [`crate::archive::store_bytes`] GoBD-konform
//! abgelegt (SHA-256, read-only); hier wird nur die Verknüpfung persistiert.

use crate::db::models::AttachmentView;
use crate::error::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Verknüpft ein bereits archiviertes File mit einer Eltern-Entität.
///
/// `parent_type` muss einem der CHECK-Werte aus `0001_init.sql` entsprechen
/// (`invoice`/`quote`/`expense`/`asset`/`contact`/`recurring`). `sort_order`
/// bestimmt die Reihenfolge im Bundle (Block 8).
pub async fn create(
    pool: &SqlitePool,
    parent_type: &str,
    parent_id: &str,
    archive_entry_id: &str,
    label: Option<&str>,
    sort_order: i64,
) -> Result<String> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO attachments
            (id, parent_type, parent_id, archive_entry_id, label, sort_order)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(parent_type)
    .bind(parent_id)
    .bind(archive_entry_id)
    .bind(label)
    .bind(sort_order)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Liefert alle Anhänge einer Eltern-Entität als Anzeige-Sicht
/// (JOIN auf `archive_entries` für Dateiname/Größe/MIME), sortiert nach
/// `sort_order` und Erstell-Zeitpunkt.
pub async fn list_for_parent(
    pool: &SqlitePool,
    parent_type: &str,
    parent_id: &str,
) -> Result<Vec<AttachmentView>> {
    let rows: Vec<AttachmentView> = sqlx::query_as(
        "SELECT a.id, a.parent_type, a.parent_id, a.archive_entry_id,
                a.label, a.sort_order, a.created_at,
                e.file_name, e.file_size_bytes, e.mime_type
           FROM attachments a
           JOIN archive_entries e ON e.id = a.archive_entry_id
          WHERE a.parent_type = ? AND a.parent_id = ?
          ORDER BY a.sort_order ASC, a.created_at ASC",
    )
    .bind(parent_type)
    .bind(parent_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Zählt die Anhänge einer Eltern-Entität — für `sort_order`-Vergabe.
pub async fn count_for_parent(
    pool: &SqlitePool,
    parent_type: &str,
    parent_id: &str,
) -> Result<i64> {
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM attachments WHERE parent_type = ? AND parent_id = ?",
    )
    .bind(parent_type)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("n")?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
