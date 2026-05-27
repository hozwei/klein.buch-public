//! Repository für `recurring_invoices` (+ `recurring_invoice_items`) — Block RI-1.
//!
//! Schicht: **Imperative Shell**. Domain-Validation kommt aus
//! [`crate::domain::recurring_invoice`]; die Stichtags-Mathematik aus
//! [`crate::domain::recurring`]. Hier nur DB-I/O.
//!
//! ## Lebenszyklus
//!
//! Eine Abo-Rechnungs-Vorlage ist ein **Stammdatum/Template**, kein GoBD-Beleg
//! — es gibt keinen Immutability-Trigger. Vorlagen werden editiert
//! ([`update`], ersetzt die Positionen) und pausiert ([`set_active`]); ein
//! Hard-Delete entfällt bewusst.
//!
//! Die Materialisierung am Stichtag (Block RI-2) liegt im Scheduler; [`advance`]
//! schreibt danach `next_due_date` + `last_*` fort.

use crate::db::models::{RecurringInvoiceDetail, RecurringInvoiceItemRow, RecurringInvoiceRow};
use crate::domain::invoice::compute_totals;
use crate::domain::recurring_invoice::RecurringInvoiceInput;
use crate::error::{Error, Result};
use sqlx::SqlitePool;
use uuid::Uuid;

// ---- CREATE ----------------------------------------------------------------

/// Legt eine Vorlage + Positionen an (transaktional). Pro Position wird das
/// Netto wie bei echten Rechnungen über [`compute_totals`] (kaufmännische
/// Rundung) berechnet und gespeichert.
pub async fn create(
    pool: &SqlitePool,
    input: &RecurringInvoiceInput,
) -> Result<RecurringInvoiceDetail> {
    let id = Uuid::now_v7().to_string();
    let totals = compute_totals(&input.items);

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO recurring_invoices (
            id, label, contact_id, frequency, day_of_period, next_due_date,
            start_date, end_date, auto_mode, payment_terms_days, pdf_template,
            service_period_note, notes
         ) VALUES (?, ?, ?, ?, ?, ?,  ?, ?, ?, ?, ?,  ?, ?)",
    )
    .bind(&id)
    .bind(input.label.trim())
    .bind(&input.contact_id)
    .bind(&input.frequency)
    .bind(input.day_of_period)
    .bind(input.next_due_date.to_string())
    .bind(input.start_date.map(|d| d.to_string()))
    .bind(input.end_date.map(|d| d.to_string()))
    .bind(&input.auto_mode)
    .bind(input.payment_terms_days)
    .bind(&input.pdf_template)
    .bind(if input.service_period_note {
        1i64
    } else {
        0i64
    })
    .bind(input.notes.as_deref())
    .execute(&mut *tx)
    .await?;

    insert_items(&mut tx, &id, input, &totals).await?;

    tx.commit().await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create: post-INSERT SELECT leer".into()))
}

// ---- UPDATE (Vorlage ist Stammdatum, editierbar) ---------------------------

/// Aktualisiert Kopf + Positionen. Positionen werden ersetzt (delete + insert).
/// `active`/`last_*`/`next_due_date` bleiben unberührt (dafür [`set_active`] /
/// [`advance`]).
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: &RecurringInvoiceInput,
) -> Result<RecurringInvoiceDetail> {
    let totals = compute_totals(&input.items);

    let mut tx = pool.begin().await?;

    let res = sqlx::query(
        "UPDATE recurring_invoices SET
            label = ?, contact_id = ?, frequency = ?, day_of_period = ?,
            start_date = ?, end_date = ?, auto_mode = ?, payment_terms_days = ?,
            pdf_template = ?, service_period_note = ?, notes = ?,
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(input.label.trim())
    .bind(&input.contact_id)
    .bind(&input.frequency)
    .bind(input.day_of_period)
    .bind(input.start_date.map(|d| d.to_string()))
    .bind(input.end_date.map(|d| d.to_string()))
    .bind(&input.auto_mode)
    .bind(input.payment_terms_days)
    .bind(&input.pdf_template)
    .bind(if input.service_period_note {
        1i64
    } else {
        0i64
    })
    .bind(input.notes.as_deref())
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Vorlage nicht gefunden: {id}")));
    }

    sqlx::query("DELETE FROM recurring_invoice_items WHERE recurring_invoice_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    insert_items(&mut tx, id, input, &totals).await?;

    tx.commit().await?;

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("update: post-UPDATE SELECT leer".into()))
}

/// Aktiviert/pausiert eine Vorlage. Pausierte Vorlagen ignoriert der Scheduler.
pub async fn set_active(pool: &SqlitePool, id: &str, active: bool) -> Result<()> {
    let res = sqlx::query(
        "UPDATE recurring_invoices SET active = ?, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(if active { 1i64 } else { 0i64 })
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Vorlage nicht gefunden: {id}")));
    }
    Ok(())
}

// ---- READ ------------------------------------------------------------------

/// Nur der Kopf (für Scheduler/Advance, ohne Positionen).
pub async fn get_row(pool: &SqlitePool, id: &str) -> Result<Option<RecurringInvoiceRow>> {
    let row: Option<RecurringInvoiceRow> =
        sqlx::query_as("SELECT * FROM recurring_invoices WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// Kopf + Positionen.
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<RecurringInvoiceDetail>> {
    let Some(template) = get_row(pool, id).await? else {
        return Ok(None);
    };
    let items = items_for(pool, id).await?;
    Ok(Some(RecurringInvoiceDetail { template, items }))
}

/// Positionen einer Vorlage, nach `position` sortiert.
pub async fn items_for(
    pool: &SqlitePool,
    recurring_invoice_id: &str,
) -> Result<Vec<RecurringInvoiceItemRow>> {
    let rows: Vec<RecurringInvoiceItemRow> = sqlx::query_as(
        "SELECT * FROM recurring_invoice_items
          WHERE recurring_invoice_id = ? ORDER BY position",
    )
    .bind(recurring_invoice_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list(pool: &SqlitePool, include_inactive: bool) -> Result<Vec<RecurringInvoiceRow>> {
    let sql = if include_inactive {
        "SELECT * FROM recurring_invoices
         ORDER BY active DESC, next_due_date ASC, label COLLATE NOCASE ASC"
    } else {
        "SELECT * FROM recurring_invoices WHERE active = 1
         ORDER BY next_due_date ASC, label COLLATE NOCASE ASC"
    };
    let rows: Vec<RecurringInvoiceRow> = sqlx::query_as(sql).fetch_all(pool).await?;
    Ok(rows)
}

/// Aktive Vorlagen, deren Stichtag erreicht/überschritten ist
/// (`next_due_date <= today`) und die noch nicht über ihr Laufzeit-Ende hinaus
/// sind. `today` als `YYYY-MM-DD` (Europe/Berlin), damit der ISO-String-Vergleich
/// korrekt sortiert. Liefert ALLE fälligen Vorlagen (auch `auto_mode='draft'`);
/// die Stufe wertet der Scheduler je Zeile aus.
pub async fn list_due(pool: &SqlitePool, today: &str) -> Result<Vec<RecurringInvoiceRow>> {
    let rows: Vec<RecurringInvoiceRow> = sqlx::query_as(
        "SELECT * FROM recurring_invoices
          WHERE active = 1
            AND next_due_date <= ?
            AND (end_date IS NULL OR next_due_date <= end_date)
          ORDER BY next_due_date ASC",
    )
    .bind(today)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---- ADVANCE (nach Materialisierung durch den Scheduler) -------------------

/// Schreibt nach einer erzeugten Rechnung den Fortschritt fort:
/// `last_executed_at`, `last_invoice_id` und den nächsten `next_due_date`.
pub async fn advance(
    pool: &SqlitePool,
    id: &str,
    last_invoice_id: &str,
    next_due_date: &str,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE recurring_invoices SET
            last_executed_at = datetime('now','utc'),
            last_invoice_id = ?,
            next_due_date = ?,
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(last_invoice_id)
    .bind(next_due_date)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "advance: Vorlage {id} nicht gefunden"
        )));
    }
    Ok(())
}

// ---- intern ----------------------------------------------------------------

/// Fügt die Positionen einer Vorlage ein. `net_amount_cents` kommt aus den
/// vorab berechneten `totals` (kaufmännische Rundung, identisch zur Rechnung).
async fn insert_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    recurring_invoice_id: &str,
    input: &RecurringInvoiceInput,
    totals: &crate::domain::invoice::Totals,
) -> Result<()> {
    for (item, computed) in input.items.iter().zip(totals.items.iter()) {
        let item_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO recurring_invoice_items (
                id, recurring_invoice_id, position, description, quantity, unit_code,
                unit_price_cents, net_amount_cents, tax_rate_percent, tax_category_code,
                description_title, description_markup, source_package_id, source_package_revision
             ) VALUES (?, ?, ?, ?, ?, ?,  ?, ?, ?, ?,  ?, ?, ?, ?)",
        )
        .bind(&item_id)
        .bind(recurring_invoice_id)
        .bind(item.position as i64)
        .bind(&item.description)
        .bind(item.quantity)
        .bind(&item.unit_code)
        .bind(item.unit_price_cents)
        .bind(computed.net_amount_cents)
        .bind(item.tax_rate_percent)
        .bind(&item.tax_category_code)
        .bind(item.description_title.as_deref())
        .bind(item.description_markup.as_deref())
        .bind(item.source_package_id.as_deref())
        .bind(item.source_package_revision)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
