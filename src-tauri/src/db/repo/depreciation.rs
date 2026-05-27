//! Repository für `depreciation_entries` (Block 12).
//!
//! Schicht: **Imperative Shell**. Die Rechnung kommt aus
//! [`crate::domain::depreciation`]; hier nur DB-I/O. Eindeutig pro Anlage/GJ über
//! `UNIQUE(asset_id, fiscal_year)` — der Buchungslauf ist damit idempotent.
//!
//! ## Festschreibung (GoBD / §146 Abs. 4 AO)
//!
//! AfA ist eine **Jahres-Größe** der Gewinnermittlung (EÜR, §4 Abs. 3 EStG). Die
//! Unveränderbarkeit greift erst mit der **Festschreibung zum Geschäftsjahr-
//! Abschluss** (Block 15: `fiscal_year::lock` setzt `locked_at` auf den Einträgen),
//! nicht im Moment der Buchung. Eine Buchung wird daher **ungelockt** angelegt
//! (`locked_at = NULL`) und bleibt im offenen GJ korrigierbar
//! ([`reset_unlocked_for_asset`]) — jede Korrektur wird im Audit-Log mit den
//! ursprünglichen Werten protokolliert (Nachvollziehbarkeit). Nach dem GJ-Abschluss
//! sperrt `trg_depreciation_immutable` jede Änderung.

use crate::db::models::DepreciationEntryRow;
use crate::domain::depreciation::DepreciationCalc;
use crate::error::{Error, Result};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Bucht eine Jahres-AfA für eine Anlage — **ungelockt** (Festschreibung erst zum
/// GJ-Abschluss). Schlägt fehl, wenn für (asset, fiscal_year) bereits eine Buchung
/// existiert (UNIQUE-Constraint); der Aufrufer prüft das vorher via
/// [`get_for_asset_year`] (idempotenter Lauf).
pub async fn book_entry(
    pool: &SqlitePool,
    asset_id: &str,
    fiscal_year: i64,
    calc: &DepreciationCalc,
) -> Result<DepreciationEntryRow> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO depreciation_entries (
            id, asset_id, fiscal_year, depreciation_amount_cents, months_in_year,
            book_value_before_cents, book_value_after_cents, is_full_writeoff
         ) VALUES (?, ?, ?, ?, ?,  ?, ?, ?)",
    )
    .bind(&id)
    .bind(asset_id)
    .bind(fiscal_year)
    .bind(calc.depreciation_amount_cents)
    .bind(calc.months_in_year as i64)
    .bind(calc.book_value_before_cents)
    .bind(calc.book_value_after_cents)
    .bind(if calc.is_full_writeoff { 1i64 } else { 0i64 })
    .execute(pool)
    .await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("book_entry: post-INSERT SELECT leer".into()))
}

/// Zahl der AfA-Buchungen einer Anlage (egal ob gelockt). Gate fürs Bearbeiten
/// der Stammdaten: solange Buchungen existieren, muss erst zurückgesetzt werden.
pub async fn count_for_asset(pool: &SqlitePool, asset_id: &str) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM depreciation_entries WHERE asset_id = ?")
        .bind(asset_id)
        .fetch_one(pool)
        .await?;
    Ok(row.try_get::<i64, _>("n")?)
}

/// Löscht alle **ungelockten** (= noch nicht festgeschriebenen, offenes GJ)
/// AfA-Buchungen einer Anlage und liefert sie zurück — der Aufrufer protokolliert
/// die gelöschten Werte revisionssicher im Audit-Log (GoBD-Nachvollziehbarkeit)
/// und stellt den Restbuchwert wieder her. Gelockte (festgeschriebene) Einträge
/// bleiben unangetastet.
pub async fn reset_unlocked_for_asset(
    pool: &SqlitePool,
    asset_id: &str,
) -> Result<Vec<DepreciationEntryRow>> {
    let removed: Vec<DepreciationEntryRow> = sqlx::query_as(
        "SELECT * FROM depreciation_entries WHERE asset_id = ? AND locked_at IS NULL",
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await?;
    sqlx::query("DELETE FROM depreciation_entries WHERE asset_id = ? AND locked_at IS NULL")
        .bind(asset_id)
        .execute(pool)
        .await?;
    Ok(removed)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<DepreciationEntryRow>> {
    let row: Option<DepreciationEntryRow> =
        sqlx::query_as("SELECT * FROM depreciation_entries WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// Buchung für (Anlage, GJ) — `None`, wenn noch nicht gebucht. Idempotenz-Check
/// für den AfA-Lauf.
pub async fn get_for_asset_year(
    pool: &SqlitePool,
    asset_id: &str,
    fiscal_year: i64,
) -> Result<Option<DepreciationEntryRow>> {
    let row: Option<DepreciationEntryRow> =
        sqlx::query_as("SELECT * FROM depreciation_entries WHERE asset_id = ? AND fiscal_year = ?")
            .bind(asset_id)
            .bind(fiscal_year)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// AfA-Historie einer Anlage (aufsteigend nach GJ) — für die Detail-Ansicht.
pub async fn list_for_asset(
    pool: &SqlitePool,
    asset_id: &str,
) -> Result<Vec<DepreciationEntryRow>> {
    let rows: Vec<DepreciationEntryRow> = sqlx::query_as(
        "SELECT * FROM depreciation_entries WHERE asset_id = ? ORDER BY fiscal_year ASC",
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Alle AfA-Buchungen eines Geschäftsjahres — Grundlage der EÜR-AfA-Position
/// (Block 13).
pub async fn list_for_year(
    pool: &SqlitePool,
    fiscal_year: i64,
) -> Result<Vec<DepreciationEntryRow>> {
    let rows: Vec<DepreciationEntryRow> = sqlx::query_as(
        "SELECT * FROM depreciation_entries WHERE fiscal_year = ? ORDER BY asset_id ASC",
    )
    .bind(fiscal_year)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Festschreibung zum GJ-Abschluss (Block 15): setzt `locked_at` auf alle noch
/// ungelockten AfA-Buchungen des Geschäftsjahres. Liefert die Anzahl der nun
/// festgeschriebenen Einträge. Ab hier verhindert `trg_depreciation_immutable`
/// jede weitere Änderung. Idempotent — bereits gelockte Einträge bleiben unberührt.
pub async fn lock_for_year(pool: &SqlitePool, fiscal_year: i64) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE depreciation_entries
            SET locked_at = datetime('now','utc')
          WHERE fiscal_year = ? AND locked_at IS NULL",
    )
    .bind(fiscal_year)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
