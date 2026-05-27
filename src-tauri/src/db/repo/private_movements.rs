//! Repository für `private_movements` (Block 9).
//!
//! Schicht: **Imperative Shell**. Domain-Validation kommt aus
//! [`crate::domain::private_movement`]; hier nur DB-I/O.
//!
//! Privatbewegungen sind **EÜR-neutral** (Block 13 klammert sie aus) und dienen
//! nur der Vollständigkeit der Kasse. Wie Kosten werden sie bei der Erfassung
//! sofort festgeschrieben ([`create`] setzt `locked_at`); ab da greift
//! `trg_private_movements_immutable`. Es gibt **kein** Storno/Cancel (das Schema
//! hat keine Status-Spalte) — eine Fehleingabe wird durch eine Gegenbewegung
//! neutralisiert (append-only).

use crate::db::models::{PrivateMovementListItem, PrivateMovementRow};
use crate::domain::private_movement::PrivateMovementInput;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub fiscal_year: Option<i64>,
    pub movement_type: Option<String>,
}

// ---- CREATE ----------------------------------------------------------------

/// Legt eine Privatbewegung an und schreibt sie sofort fest (`locked_at=now`).
/// `movement_number` allokiert der Caller (`PV-{YYYY}-{NNNN}`).
pub async fn create(
    pool: &SqlitePool,
    input: &PrivateMovementInput,
    movement_number: &str,
    fiscal_year: i64,
    receipt_archive_id: Option<&str>,
) -> Result<PrivateMovementRow> {
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO private_movements (
            id, movement_number, fiscal_year, movement_date, movement_type,
            amount_cents, account_id, description, receipt_archive_id,
            locked_at, notes
         ) VALUES (?, ?, ?, ?, ?,  ?, ?, ?, ?,  datetime('now','utc'), ?)",
    )
    .bind(&id)
    .bind(movement_number)
    .bind(fiscal_year)
    .bind(input.movement_date.to_string())
    .bind(&input.movement_type)
    .bind(input.amount_cents)
    .bind(input.account_id.as_deref())
    .bind(input.description.trim())
    .bind(receipt_archive_id)
    .bind(input.notes.as_deref())
    .execute(pool)
    .await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create: post-INSERT SELECT leer".into()))
}

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<PrivateMovementRow>> {
    let row: Option<PrivateMovementRow> =
        sqlx::query_as("SELECT * FROM private_movements WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

pub async fn list(pool: &SqlitePool, filter: &ListFilter) -> Result<Vec<PrivateMovementListItem>> {
    let mut sql = String::from(
        "SELECT p.id, p.movement_number, p.fiscal_year, p.movement_date,
                p.movement_type, p.amount_cents, p.account_id,
                a.label AS account_label, p.description
           FROM private_movements p
           LEFT JOIN payment_accounts a ON a.id = p.account_id
          WHERE 1=1",
    );
    if filter.fiscal_year.is_some() {
        sql.push_str(" AND p.fiscal_year = ?");
    }
    if filter.movement_type.is_some() {
        sql.push_str(" AND p.movement_type = ?");
    }
    sql.push_str(" ORDER BY p.fiscal_year DESC, p.movement_number DESC");

    let mut q = sqlx::query_as::<_, PrivateMovementListItem>(&sql);
    if let Some(y) = filter.fiscal_year {
        q = q.bind(y);
    }
    if let Some(t) = filter.movement_type.as_deref() {
        q = q.bind(t);
    }
    Ok(q.fetch_all(pool).await?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
