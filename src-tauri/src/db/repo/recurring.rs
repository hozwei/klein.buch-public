//! Repository für `recurring_subscriptions` (Block 10).
//!
//! Schicht: **Imperative Shell**. Domain-Validation kommt aus
//! [`crate::domain::recurring`]; hier nur DB-I/O.
//!
//! ## Lebenszyklus
//!
//! Ein Abo ist ein **Stammdatum/Template**, kein GoBD-Beleg — es gibt keinen
//! Immutability-Trigger. Abos werden editiert ([`update`]) und pausiert
//! ([`set_active`]); ein Hard-Delete entfällt bewusst (FK von `expenses`
//! über `recurring_subscription_id` ist plain TEXT, aber `last_expense_id`
//! referenziert `expenses(id)`).
//!
//! Die Auto-Anlage der Kosten am Stichtag (Catch-up) liegt in
//! [`crate::scheduler::recurring`]; [`advance`] schreibt danach den nächsten
//! Stichtag + `last_*` fort.

use crate::db::models::RecurringSubscriptionRow;
use crate::domain::recurring::RecurringInput;
use crate::error::{Error, Result};
use sqlx::SqlitePool;
use uuid::Uuid;

// ---- CREATE ----------------------------------------------------------------

/// Legt ein Abo an. `next_due_date` wird als `YYYY-MM-DD` gespeichert.
pub async fn create(pool: &SqlitePool, input: &RecurringInput) -> Result<RecurringSubscriptionRow> {
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO recurring_subscriptions (
            id, label, vendor_contact_id, frequency, day_of_period, next_due_date,
            expected_amount_cents, category, description_template,
            auto_create_expense, reverse_charge_13b_default, active
         ) VALUES (?, ?, ?, ?, ?, ?,  ?, ?, ?,  ?, ?, 1)",
    )
    .bind(&id)
    .bind(input.label.trim())
    .bind(input.vendor_contact_id.as_deref())
    .bind(&input.frequency)
    .bind(input.day_of_period)
    .bind(input.next_due_date.to_string())
    .bind(input.expected_amount_cents)
    .bind(&input.category)
    .bind(input.description_template.trim())
    .bind(if input.auto_create_expense {
        1i64
    } else {
        0i64
    })
    .bind(if input.reverse_charge_13b_default {
        1i64
    } else {
        0i64
    })
    .execute(pool)
    .await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create: post-INSERT SELECT leer".into()))
}

// ---- UPDATE (Abo ist Stammdatum, editierbar) -------------------------------

/// Aktualisiert die Konfiguration eines Abos. `active` bleibt unberührt
/// (dafür [`set_active`]); `last_*` werden vom Scheduler über [`advance`]
/// gepflegt und hier nicht angefasst.
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: &RecurringInput,
) -> Result<RecurringSubscriptionRow> {
    let res = sqlx::query(
        "UPDATE recurring_subscriptions SET
            label = ?, vendor_contact_id = ?, frequency = ?, day_of_period = ?,
            next_due_date = ?, expected_amount_cents = ?, category = ?,
            description_template = ?, auto_create_expense = ?, reverse_charge_13b_default = ?
         WHERE id = ?",
    )
    .bind(input.label.trim())
    .bind(input.vendor_contact_id.as_deref())
    .bind(&input.frequency)
    .bind(input.day_of_period)
    .bind(input.next_due_date.to_string())
    .bind(input.expected_amount_cents)
    .bind(&input.category)
    .bind(input.description_template.trim())
    .bind(if input.auto_create_expense {
        1i64
    } else {
        0i64
    })
    .bind(if input.reverse_charge_13b_default {
        1i64
    } else {
        0i64
    })
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Abo nicht gefunden: {id}")));
    }
    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("update: post-UPDATE SELECT leer".into()))
}

/// Aktiviert/pausiert ein Abo. Pausierte Abos werden vom Scheduler ignoriert
/// und in der Liste als „pausiert" markiert (kein Hard-Delete).
pub async fn set_active(pool: &SqlitePool, id: &str, active: bool) -> Result<()> {
    let res = sqlx::query("UPDATE recurring_subscriptions SET active = ? WHERE id = ?")
        .bind(if active { 1i64 } else { 0i64 })
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Abo nicht gefunden: {id}")));
    }
    Ok(())
}

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<RecurringSubscriptionRow>> {
    let row: Option<RecurringSubscriptionRow> =
        sqlx::query_as("SELECT * FROM recurring_subscriptions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

pub async fn list(
    pool: &SqlitePool,
    include_inactive: bool,
) -> Result<Vec<RecurringSubscriptionRow>> {
    let sql = if include_inactive {
        "SELECT * FROM recurring_subscriptions
         ORDER BY active DESC, next_due_date ASC, label COLLATE NOCASE ASC"
    } else {
        "SELECT * FROM recurring_subscriptions WHERE active = 1
         ORDER BY next_due_date ASC, label COLLATE NOCASE ASC"
    };
    let rows: Vec<RecurringSubscriptionRow> = sqlx::query_as(sql).fetch_all(pool).await?;
    Ok(rows)
}

/// Aktive Abos mit Auto-Anlage, deren Stichtag erreicht/überschritten ist
/// (`next_due_date <= today`). `today` als `YYYY-MM-DD` — der Scheduler injiziert
/// den lokalen Tag (Europe/Berlin), damit der String-Vergleich (ISO-Datum) korrekt
/// sortiert.
pub async fn list_due_auto(
    pool: &SqlitePool,
    today: &str,
) -> Result<Vec<RecurringSubscriptionRow>> {
    let rows: Vec<RecurringSubscriptionRow> = sqlx::query_as(
        "SELECT * FROM recurring_subscriptions
          WHERE active = 1 AND auto_create_expense = 1 AND next_due_date <= ?
          ORDER BY next_due_date ASC",
    )
    .bind(today)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---- ADVANCE (nach Auto-/Manuell-Anlage) -----------------------------------

/// Schreibt nach einer erzeugten Kosten-Position den Fortschritt fort:
/// `last_executed_at`, `last_expense_id` und den nächsten `next_due_date`.
pub async fn advance(
    pool: &SqlitePool,
    id: &str,
    last_expense_id: &str,
    next_due_date: &str,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE recurring_subscriptions SET
            last_executed_at = datetime('now','utc'),
            last_expense_id = ?,
            next_due_date = ?
         WHERE id = ?",
    )
    .bind(last_expense_id)
    .bind(next_due_date)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("advance: Abo {id} nicht gefunden")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
