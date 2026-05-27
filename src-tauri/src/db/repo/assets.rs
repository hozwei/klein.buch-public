//! Repository für `assets` (Block 12).
//!
//! Schicht: **Imperative Shell**. Domain-Validation/-Rechnung kommen aus
//! [`crate::domain::asset`] / [`crate::domain::depreciation`]; hier nur DB-I/O.
//!
//! ## Lebenszyklus (GoBD-Hardline)
//!
//! Eine Anlage wird **unlocked** angelegt — sie ist bis zur ersten AfA-Buchung
//! korrigierbar ([`update`]). Mit der ersten AfA-Buchung
//! ([`crate::depreciation::accrue_yearly`]) wird sie über [`lock`] festgeschrieben;
//! ab dann sperrt `trg_assets_immutable` die Kernfelder. Eine Anlage wird **nie
//! gelöscht** — sie wird höchstens veräußert/verschrottet ([`dispose`]).

use crate::db::models::{AssetDetail, AssetListItem, AssetRow};
use crate::domain::asset::AssetInput;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub fiscal_year: Option<i64>,
    /// `Some(true)` → nur veräußerte; `Some(false)` → nur aktive; `None` → alle.
    pub disposed: Option<bool>,
    pub afa_category: Option<String>,
}

// ---- CREATE ----------------------------------------------------------------

/// Legt eine Anlage an (unlocked). `book_value_cents` ist der betriebliche
/// Start-Restbuchwert, `useful_life_years` die effektive Nutzungsdauer (vom
/// Caller je nach Methode gesetzt: linear = Eingabe, computer_special = 1, GWG =
/// None). `asset_number` allokiert der Caller über die Counter-Schicht.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &SqlitePool,
    input: &AssetInput,
    asset_number: &str,
    fiscal_year: i64,
    method_db: &str,
    useful_life_years: Option<f64>,
    book_value_cents: i64,
) -> Result<AssetRow> {
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO assets (
            id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, expense_id, vendor_contact_id,
            depreciation_method, useful_life_years, afa_category,
            business_share_percent, book_value_cents, notes
         ) VALUES (?, ?, ?, ?, ?,  ?, ?, ?,  ?, ?, ?,  ?, ?, ?)",
    )
    .bind(&id)
    .bind(asset_number)
    .bind(input.label.trim())
    .bind(input.acquisition_date.to_string())
    .bind(input.acquisition_cost_cents)
    .bind(fiscal_year)
    .bind(input.expense_id.as_deref())
    .bind(input.vendor_contact_id.as_deref())
    .bind(method_db)
    .bind(useful_life_years)
    .bind(input.afa_category.as_deref())
    .bind(input.business_share_percent)
    .bind(book_value_cents)
    .bind(input.notes.as_deref())
    .execute(pool)
    .await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create: post-INSERT SELECT leer".into()))
}

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<AssetRow>> {
    let row: Option<AssetRow> = sqlx::query_as("SELECT * FROM assets WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Anlage + Lieferanten-Kontakt (optional) + AfA-Historie + (falls aktiviert)
/// die Belegnummer der Quell-Kosten.
pub async fn get_detail(pool: &SqlitePool, id: &str) -> Result<Option<AssetDetail>> {
    let Some(asset) = get(pool, id).await? else {
        return Ok(None);
    };
    let vendor = match asset.vendor_contact_id.as_deref() {
        Some(cid) => crate::db::repo::contacts::get(pool, cid).await?,
        None => None,
    };
    let depreciation_entries = crate::db::repo::depreciation::list_for_asset(pool, id).await?;
    let source_expense_number = match asset.expense_id.as_deref() {
        Some(eid) => sqlx::query("SELECT expense_number FROM expenses WHERE id = ?")
            .bind(eid)
            .fetch_optional(pool)
            .await?
            .map(|r| r.get::<String, _>("expense_number")),
        None => None,
    };
    Ok(Some(AssetDetail {
        asset,
        vendor,
        depreciation_entries,
        source_expense_number,
    }))
}

pub async fn list(pool: &SqlitePool, filter: &ListFilter) -> Result<Vec<AssetListItem>> {
    let mut sql = String::from(
        "SELECT id, asset_number, label, acquisition_date, acquisition_fiscal_year,
                acquisition_cost_cents, depreciation_method, business_share_percent,
                book_value_cents, last_depreciation_year, disposed, disposal_date, locked_at
           FROM assets
          WHERE 1=1",
    );
    if filter.fiscal_year.is_some() {
        sql.push_str(" AND acquisition_fiscal_year = ?");
    }
    match filter.disposed {
        Some(true) => sql.push_str(" AND disposed = 1"),
        Some(false) => sql.push_str(" AND disposed = 0"),
        None => {}
    }
    if filter.afa_category.is_some() {
        sql.push_str(" AND afa_category = ?");
    }
    sql.push_str(" ORDER BY acquisition_fiscal_year DESC, asset_number DESC");

    let mut q = sqlx::query_as::<_, AssetListItem>(&sql);
    if let Some(y) = filter.fiscal_year {
        q = q.bind(y);
    }
    if let Some(c) = filter.afa_category.as_deref() {
        q = q.bind(c);
    }
    Ok(q.fetch_all(pool).await?)
}

/// Aktive (nicht veräußerte) Anlagen, deren Anschaffungsjahr `<= fiscal_year`
/// liegt — Grundmenge für den AfA-Buchungslauf eines Geschäftsjahres.
pub async fn list_active_for_year(pool: &SqlitePool, fiscal_year: i64) -> Result<Vec<AssetRow>> {
    let rows: Vec<AssetRow> = sqlx::query_as(
        "SELECT * FROM assets
          WHERE disposed = 0 AND acquisition_fiscal_year <= ?
          ORDER BY asset_number ASC",
    )
    .bind(fiscal_year)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---- UPDATE (nur solange unlocked) -----------------------------------------

/// Korrigiert eine **noch nicht festgeschriebene** Anlage (vor der ersten
/// AfA-Buchung). Nach dem Lock schlägt der Aufruf mit einer klaren Meldung fehl
/// (die `trg_assets_immutable`-Trigger würden Kernfeld-Änderungen ohnehin
/// abbrechen). `business_share_percent`/Methode/Nutzungsdauer/Kosten dürfen sich
/// pre-lock ändern, daher wird auch der Start-Restbuchwert neu gesetzt.
#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: &AssetInput,
    method_db: &str,
    useful_life_years: Option<f64>,
    book_value_cents: i64,
) -> Result<AssetRow> {
    let existing = get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {id}")))?;
    if existing.locked_at.is_some() {
        return Err(Error::Domain(
            "Anlage ist festgeschrieben (AfA bereits gebucht) — eine Korrektur ist nicht mehr möglich."
                .into(),
        ));
    }
    if existing.disposed == 1 {
        return Err(Error::Domain(
            "Anlage ist bereits veräußert — keine Stammdaten-Änderung möglich.".into(),
        ));
    }

    sqlx::query(
        "UPDATE assets SET
            label = ?, acquisition_date = ?, acquisition_cost_cents = ?,
            expense_id = ?, vendor_contact_id = ?, depreciation_method = ?,
            useful_life_years = ?, afa_category = ?, business_share_percent = ?,
            book_value_cents = ?, notes = ?, updated_at = datetime('now','utc')
          WHERE id = ?",
    )
    .bind(input.label.trim())
    .bind(input.acquisition_date.to_string())
    .bind(input.acquisition_cost_cents)
    .bind(input.expense_id.as_deref())
    .bind(input.vendor_contact_id.as_deref())
    .bind(method_db)
    .bind(useful_life_years)
    .bind(input.afa_category.as_deref())
    .bind(input.business_share_percent)
    .bind(book_value_cents)
    .bind(input.notes.as_deref())
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("update: post-UPDATE SELECT leer".into()))
}

// ---- LOCK / AfA-Fortschreibung ---------------------------------------------

/// Schreibt die Anlage fest (erste AfA-Buchung). Idempotent — ein erneuter
/// Aufruf bei bereits gesetztem `locked_at` ändert nichts (kein Kernfeld).
pub async fn lock(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE assets SET locked_at = datetime('now','utc') WHERE id = ? AND locked_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Schreibt Restbuchwert + zuletzt gebuchtes GJ fort (durch den AfA-Lauf oder das
/// Zurücksetzen). `book_value_cents`/`last_depreciation_year` sind keine
/// Immutability-Kernfelder. `last_depreciation_year = None` setzt das Feld auf NULL
/// zurück (z. B. wenn alle Buchungen zurückgesetzt wurden).
pub async fn set_book_value(
    pool: &SqlitePool,
    id: &str,
    book_value_cents: i64,
    last_depreciation_year: Option<i64>,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE assets SET book_value_cents = ?, last_depreciation_year = ?,
                updated_at = datetime('now','utc')
          WHERE id = ?",
    )
    .bind(book_value_cents)
    .bind(last_depreciation_year)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_book_value: Anlage {id} nicht gefunden"
        )));
    }
    Ok(())
}

// ---- DISPOSE (Veräußerung/Verschrottung, kein Löschen) ---------------------

/// Veräußert/verschrottet eine Anlage. `residual_book_value_cents` ist der
/// Restbuchwert-Snapshot zum Disposal. Disposal-Felder stehen NICHT in der
/// Immutability-Whitelist — auch eine festgeschriebene Anlage darf veräußert
/// werden. Doppel-Disposal wird verhindert.
pub async fn dispose(
    pool: &SqlitePool,
    id: &str,
    disposal_date: &str,
    disposal_type_db: &str,
    proceeds_cents: i64,
    residual_book_value_cents: i64,
) -> Result<AssetRow> {
    let existing = get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {id}")))?;
    if existing.disposed == 1 {
        return Err(Error::Domain("Anlage ist bereits veräußert.".into()));
    }

    sqlx::query(
        "UPDATE assets SET
            disposed = 1, disposal_date = ?, disposal_type = ?,
            disposal_proceeds_cents = ?, disposal_residual_book_value_cents = ?,
            updated_at = datetime('now','utc')
          WHERE id = ?",
    )
    .bind(disposal_date)
    .bind(disposal_type_db)
    .bind(proceeds_cents)
    .bind(residual_book_value_cents)
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("dispose: post-UPDATE SELECT leer".into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
