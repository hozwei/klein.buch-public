//! Repository für den Paket-Katalog (Block P2, Migration 0019).
//!
//! Schicht: **Imperative Shell**.
//!
//! ## GoBD-/Append-only-Hardline (legal_documents-Muster)
//! - `package_categories` + Paket-**Header** (`packages`) sind **mutable**
//!   Stammdaten (kein Beleg).
//! - `package_revisions` sind **append-only + unveränderlich** (DB-Trigger
//!   `trg_package_revisions_no_delete` / `_immutable`). „Bearbeiten" = neue
//!   Revision (`revision = max+1`). „Rollback" = neue Revision, die den Inhalt
//!   einer alten kopiert. Nie Update/Delete einer bestehenden Revision.

use crate::db::models::{PackageCategoryRow, PackageRevisionRow, PackageRow};
use crate::domain::package::PackageRevisionInput;
use crate::error::{Error, Result};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

// ===========================================================================
// Kategorien (mutable)
// ===========================================================================

pub async fn categories_list(pool: &SqlitePool) -> Result<Vec<PackageCategoryRow>> {
    let rows = sqlx::query_as::<_, PackageCategoryRow>(
        "SELECT id, name, sort_order, created_at, updated_at
           FROM package_categories ORDER BY sort_order ASC, name ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn category_get(pool: &SqlitePool, id: &str) -> Result<Option<PackageCategoryRow>> {
    let row = sqlx::query_as::<_, PackageCategoryRow>(
        "SELECT id, name, sort_order, created_at, updated_at
           FROM package_categories WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn category_create(pool: &SqlitePool, name: &str) -> Result<PackageCategoryRow> {
    let id = Uuid::now_v7().to_string();
    let next_sort: i64 =
        sqlx::query("SELECT COALESCE(MAX(sort_order), -1) + 1 AS s FROM package_categories")
            .fetch_one(pool)
            .await?
            .try_get("s")?;
    sqlx::query("INSERT INTO package_categories (id, name, sort_order) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(next_sort)
        .execute(pool)
        .await?;
    category_get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("category_create: post-INSERT SELECT leer".into()))
}

pub async fn category_update(pool: &SqlitePool, id: &str, name: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE package_categories SET name = ?, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(name)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Kategorie nicht gefunden: {id}")));
    }
    Ok(())
}

/// Setzt `sort_order` gemäß der übergebenen ID-Reihenfolge (0..n).
pub async fn categories_reorder(pool: &SqlitePool, ordered_ids: &[String]) -> Result<()> {
    let mut tx = pool.begin().await?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE package_categories SET sort_order = ?, updated_at = datetime('now','utc') WHERE id = ?",
        )
        .bind(idx as i64)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

// ===========================================================================
// Paket-Header (mutable) + Revisionen (append-only)
// ===========================================================================

const SELECT_PACKAGE: &str = "SELECT id, category_id, name, status, current_revision,
            sort_order, created_at, updated_at FROM packages";

pub async fn list(pool: &SqlitePool) -> Result<Vec<PackageRow>> {
    let rows = sqlx::query_as::<_, PackageRow>(&format!(
        "{SELECT_PACKAGE} ORDER BY sort_order ASC, name ASC"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<PackageRow>> {
    let row = sqlx::query_as::<_, PackageRow>(&format!("{SELECT_PACKAGE} WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Nächste Revisionsnummer (max+1) für ein Paket — innerhalb einer Transaktion.
async fn next_revision(tx: &mut Transaction<'_, Sqlite>, package_id: &str) -> Result<i64> {
    let next: i64 = sqlx::query(
        "SELECT COALESCE(MAX(revision), 0) + 1 AS v FROM package_revisions WHERE package_id = ?",
    )
    .bind(package_id)
    .fetch_one(&mut **tx)
    .await?
    .try_get("v")?;
    Ok(next)
}

/// Fügt eine Revision append-only ein und setzt sie als `current_revision`.
async fn insert_revision_tx(
    tx: &mut Transaction<'_, Sqlite>,
    package_id: &str,
    input: &PackageRevisionInput,
    note_override: Option<&str>,
) -> Result<i64> {
    let revision = next_revision(tx, package_id).await?;
    let rid = Uuid::now_v7().to_string();
    let note = note_override.or(input.note.as_deref());
    sqlx::query(
        "INSERT INTO package_revisions
            (id, package_id, revision, title, body_markup,
             default_unit_price_cents, unit_code, tax_category_code, note)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&rid)
    .bind(package_id)
    .bind(revision)
    .bind(&input.title)
    .bind(&input.body_markup)
    .bind(input.default_unit_price_cents)
    .bind(&input.unit_code)
    .bind(&input.tax_category_code)
    .bind(note)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE packages SET current_revision = ?, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(revision)
    .bind(package_id)
    .execute(&mut **tx)
    .await?;
    Ok(revision)
}

/// Legt einen Paket-Header + die erste Revision (revision 1) in einer
/// Transaktion an und setzt `current_revision`.
pub async fn create(
    pool: &SqlitePool,
    category_id: Option<&str>,
    name: &str,
    revision: &PackageRevisionInput,
) -> Result<PackageRow> {
    let id = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;
    let next_sort: i64 = sqlx::query("SELECT COALESCE(MAX(sort_order), -1) + 1 AS s FROM packages")
        .fetch_one(&mut *tx)
        .await?
        .try_get("s")?;
    sqlx::query(
        "INSERT INTO packages (id, category_id, name, status, current_revision, sort_order)
         VALUES (?, ?, ?, 'active', NULL, ?)",
    )
    .bind(&id)
    .bind(category_id)
    .bind(name)
    .bind(next_sort)
    .execute(&mut *tx)
    .await?;
    insert_revision_tx(&mut tx, &id, revision, None).await?;
    tx.commit().await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("package create: post-INSERT SELECT leer".into()))
}

/// „Bearbeiten" = mutablen Header (Name/Kategorie) aktualisieren UND eine neue
/// Revision append-only schreiben (revision = max+1), die zur `current_revision`
/// wird. Die bestehenden Revisionen bleiben unverändert.
pub async fn update_as_new_revision(
    pool: &SqlitePool,
    package_id: &str,
    category_id: Option<&str>,
    name: &str,
    revision: &PackageRevisionInput,
) -> Result<PackageRow> {
    let mut tx = pool.begin().await?;
    let res = sqlx::query(
        "UPDATE packages SET name = ?, category_id = ?, updated_at = datetime('now','utc')
          WHERE id = ?",
    )
    .bind(name)
    .bind(category_id)
    .bind(package_id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Paket nicht gefunden: {package_id}")));
    }
    insert_revision_tx(&mut tx, package_id, revision, None).await?;
    tx.commit().await?;
    get(pool, package_id)
        .await?
        .ok_or_else(|| Error::Domain("package update: post-UPDATE SELECT leer".into()))
}

pub async fn set_status(pool: &SqlitePool, id: &str, status: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE packages SET status = ?, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Paket nicht gefunden: {id}")));
    }
    Ok(())
}

// ===========================================================================
// Revisionen lesen + Rollback
// ===========================================================================

const SELECT_REVISION: &str = "SELECT id, package_id, revision, title, body_markup,
            default_unit_price_cents, unit_code, tax_category_code, note, created_at
       FROM package_revisions";

pub async fn revisions_list(
    pool: &SqlitePool,
    package_id: &str,
) -> Result<Vec<PackageRevisionRow>> {
    let rows = sqlx::query_as::<_, PackageRevisionRow>(&format!(
        "{SELECT_REVISION} WHERE package_id = ? ORDER BY revision DESC"
    ))
    .bind(package_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn revision_get(
    pool: &SqlitePool,
    package_id: &str,
    revision: i64,
) -> Result<Option<PackageRevisionRow>> {
    let row = sqlx::query_as::<_, PackageRevisionRow>(&format!(
        "{SELECT_REVISION} WHERE package_id = ? AND revision = ?"
    ))
    .bind(package_id)
    .bind(revision)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Liest die aktuell aktive (`current_revision`) Revision eines Pakets.
pub async fn current_revision(
    pool: &SqlitePool,
    package_id: &str,
) -> Result<Option<PackageRevisionRow>> {
    let pkg = match get(pool, package_id).await? {
        Some(p) => p,
        None => return Ok(None),
    };
    match pkg.current_revision {
        Some(rev) => revision_get(pool, package_id, rev).await,
        None => Ok(None),
    }
}

/// Rollback = **neue** Revision, die den Inhalt von `to_revision` kopiert
/// (`note = "Rollback auf Revision N"`), und sie als `current_revision` setzt.
/// Append-only bleibt gewahrt — nichts wird überschrieben.
pub async fn rollback(pool: &SqlitePool, package_id: &str, to_revision: i64) -> Result<PackageRow> {
    let src = revision_get(pool, package_id, to_revision)
        .await?
        .ok_or_else(|| {
            Error::Domain(format!(
                "Revision {to_revision} von Paket {package_id} nicht gefunden"
            ))
        })?;
    let input = PackageRevisionInput {
        title: src.title,
        body_markup: src.body_markup,
        default_unit_price_cents: src.default_unit_price_cents,
        unit_code: src.unit_code,
        tax_category_code: src.tax_category_code,
        note: None,
    };
    let note = format!("Rollback auf Revision {to_revision}");
    let mut tx = pool.begin().await?;
    insert_revision_tx(&mut tx, package_id, &input, Some(&note)).await?;
    tx.commit().await?;
    get(pool, package_id)
        .await?
        .ok_or_else(|| Error::Domain("rollback: post-INSERT SELECT leer".into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
