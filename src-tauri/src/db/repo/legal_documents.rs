//! Repository für `legal_documents` + `quote_legal_documents` (Block 8,
//! Migration 0006).
//!
//! Schicht: **Imperative Shell**. Zentral versionierte Rechtsdokumente (AGB +
//! Datenschutz) als PDF-Upload pro Version; die PDF-Bytes liegen write-once in
//! `archive_entries` ([`crate::archive::store_bytes`], `ArchiveKind::LegalDocument`).
//!
//! ## GoBD-Hardline
//!
//! - **Append-only:** Versionen werden nie gelöscht (`trg_legal_documents_no_delete`).
//!   Kernfelder unveränderlich (`trg_legal_documents_immutable`); nur der
//!   Aktiv-Status (`is_active`/`activated_at`/`deactivated_at`) ändert sich.
//! - **Höchstens eine aktive Version pro doc_type** (partial unique index
//!   `uq_legal_documents_active`). [`activate`] deaktiviert die bisher aktive
//!   Version derselben Art in derselben Transaktion, bevor die neue aktiv wird.
//! - **Bindung Angebot↔Version** (`quote_legal_documents`) ist eine eigene
//!   append-only Assoziation (kein mutables Quote-Kernfeld — Angebote sind ab
//!   `sent` gelockt). [`bind_active_for_quote`] ist idempotent: eine bereits
//!   gesetzte Bindung pro (Angebot, doc_type) bleibt unverändert.

use crate::db::models::{LegalDocumentRow, QuoteLegalDocumentView};
use crate::error::{Error, Result};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

const SELECT_LEGAL: &str = "SELECT l.id, l.doc_type, l.version, l.title, l.archive_entry_id,
            l.is_active, l.created_at, l.activated_at, l.deactivated_at,
            e.file_name, e.file_size_bytes, e.mime_type
       FROM legal_documents l
       JOIN archive_entries e ON e.id = l.archive_entry_id";

/// Gültige Dokumenttypen — Reihenfolge bestimmt die Bundle-Reihenfolge.
pub const DOC_TYPES: [&str; 2] = ["agb", "privacy"];

pub fn is_valid_doc_type(doc_type: &str) -> bool {
    DOC_TYPES.contains(&doc_type)
}

// ---- CREATE ----------------------------------------------------------------

/// Legt eine neue (inaktive) Version eines Rechtsdokuments an. `version` wird
/// pro `doc_type` monoton vergeben (max+1). Das PDF muss vorher über
/// [`crate::archive::store_bytes`] archiviert sein (`archive_entry_id`).
pub async fn create_version(
    pool: &SqlitePool,
    doc_type: &str,
    archive_entry_id: &str,
    title: &str,
) -> Result<LegalDocumentRow> {
    if !is_valid_doc_type(doc_type) {
        return Err(Error::Domain(format!(
            "Unbekannter Dokumenttyp '{doc_type}' (erlaubt: agb, privacy)"
        )));
    }
    let id = Uuid::now_v7().to_string();

    let mut tx = pool.begin().await?;
    let next_version: i64 = sqlx::query(
        "SELECT COALESCE(MAX(version), 0) + 1 AS v FROM legal_documents WHERE doc_type = ?",
    )
    .bind(doc_type)
    .fetch_one(&mut *tx)
    .await?
    .try_get("v")?;

    sqlx::query(
        "INSERT INTO legal_documents
            (id, doc_type, version, title, archive_entry_id, is_active)
         VALUES (?, ?, ?, ?, ?, 0)",
    )
    .bind(&id)
    .bind(doc_type)
    .bind(next_version)
    .bind(title)
    .bind(archive_entry_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create_version: post-INSERT SELECT leer".into()))
}

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<LegalDocumentRow>> {
    let sql = format!("{SELECT_LEGAL} WHERE l.id = ?");
    let row: Option<LegalDocumentRow> = sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?;
    Ok(row)
}

/// Alle Versionen (alle Typen), neueste zuerst je Typ — für die Settings-Liste.
pub async fn list(pool: &SqlitePool) -> Result<Vec<LegalDocumentRow>> {
    let sql = format!("{SELECT_LEGAL} ORDER BY l.doc_type ASC, l.version DESC");
    let rows: Vec<LegalDocumentRow> = sqlx::query_as(&sql).fetch_all(pool).await?;
    Ok(rows)
}

/// Aktive Version eines doc_type (höchstens eine).
pub async fn get_active(pool: &SqlitePool, doc_type: &str) -> Result<Option<LegalDocumentRow>> {
    let sql = format!("{SELECT_LEGAL} WHERE l.doc_type = ? AND l.is_active = 1");
    let row: Option<LegalDocumentRow> = sqlx::query_as(&sql)
        .bind(doc_type)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

// ---- ACTIVATE / DEACTIVATE -------------------------------------------------

/// Aktiviert eine Version. Deaktiviert (in derselben Transaktion, zuerst) die
/// bisher aktive Version desselben `doc_type`, damit der partial-unique Index
/// `uq_legal_documents_active` nie verletzt wird.
pub async fn activate(pool: &SqlitePool, id: &str) -> Result<()> {
    let mut tx = pool.begin().await?;
    let doc_type: String = sqlx::query("SELECT doc_type FROM legal_documents WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechtsdokument nicht gefunden: {id}")))?
        .try_get("doc_type")?;

    // Bisher aktive Version desselben Typs deaktivieren (außer dieser).
    sqlx::query(
        "UPDATE legal_documents
            SET is_active = 0, deactivated_at = datetime('now','utc')
          WHERE doc_type = ? AND is_active = 1 AND id != ?",
    )
    .bind(&doc_type)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Diese Version aktivieren.
    let res = sqlx::query(
        "UPDATE legal_documents
            SET is_active = 1, activated_at = datetime('now','utc'), deactivated_at = NULL
          WHERE id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "activate: Dokument {id} nicht gefunden"
        )));
    }
    tx.commit().await?;
    Ok(())
}

/// Deaktiviert eine Version (kein aktives Dokument dieses Typs danach).
pub async fn deactivate(pool: &SqlitePool, id: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE legal_documents
            SET is_active = 0, deactivated_at = datetime('now','utc')
          WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "deactivate: Dokument {id} nicht gefunden"
        )));
    }
    Ok(())
}

// ---- QUOTE BINDING ---------------------------------------------------------

/// Bindet die aktuell aktiven Legal-Versionen (AGB + Datenschutz) an ein
/// Angebot — **idempotent + append-only**. Eine bereits gesetzte Bindung pro
/// (Angebot, doc_type) bleibt unverändert (rechtlicher Nachweis-Snapshot).
/// Für doc_types ohne aktive Version wird nichts gebunden.
///
/// Gibt die resultierenden Bindungen (vorhandene + neu erzeugte) zurück.
pub async fn bind_active_for_quote(
    pool: &SqlitePool,
    quote_id: &str,
) -> Result<Vec<QuoteLegalDocumentView>> {
    for doc_type in DOC_TYPES {
        // Bereits gebunden? Dann nicht anfassen (idempotent).
        let existing: Option<String> =
            sqlx::query("SELECT id FROM quote_legal_documents WHERE quote_id = ? AND doc_type = ?")
                .bind(quote_id)
                .bind(doc_type)
                .fetch_optional(pool)
                .await?
                .and_then(|r| r.try_get::<String, _>("id").ok());
        if existing.is_some() {
            continue;
        }
        // Sonst: aktive Version dieses Typs binden, falls vorhanden.
        if let Some(active) = get_active(pool, doc_type).await? {
            let id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO quote_legal_documents
                    (id, quote_id, legal_document_id, doc_type, version)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(quote_id)
            .bind(&active.id)
            .bind(doc_type)
            .bind(active.version)
            .execute(pool)
            .await?;
        }
    }
    list_for_quote(pool, quote_id).await
}

/// Liefert die fest mit einem Angebot verknüpften Legal-Versionen (Anzeige).
pub async fn list_for_quote(
    pool: &SqlitePool,
    quote_id: &str,
) -> Result<Vec<QuoteLegalDocumentView>> {
    let rows: Vec<QuoteLegalDocumentView> = sqlx::query_as(
        "SELECT q.id, q.quote_id, q.legal_document_id, q.doc_type, q.version, q.bound_at,
                l.title, l.archive_entry_id, e.file_name
           FROM quote_legal_documents q
           JOIN legal_documents l ON l.id = q.legal_document_id
           JOIN archive_entries e ON e.id = l.archive_entry_id
          WHERE q.quote_id = ?
          ORDER BY q.doc_type ASC",
    )
    .bind(quote_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
