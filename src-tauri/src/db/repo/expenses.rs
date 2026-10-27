//! Repository für `expenses` (Block 9).
//!
//! Schicht: **Imperative Shell**. Domain-Validation kommt aus
//! [`crate::domain::expense`]; hier nur DB-I/O.
//!
//! ## Lebenszyklus (GoBD-Hardline)
//!
//! Kosten kennen keinen Draft-Zustand (das Schema hat nur `status` in
//! `('recorded','canceled')`). Sie werden bei der Erfassung **sofort
//! festgeschrieben**: [`create`] setzt `status='recorded'` UND `locked_at`.
//! Ab da greift `trg_expenses_immutable` auf den Kernfeldern. Eine fehlerhafte
//! Kosten-Position wird **storniert** ([`cancel`] → `status='canceled'`) und
//! durch eine neue, korrigierte Position ersetzt — nie editiert oder gelöscht.
//!
//! Der primäre Beleg (`receipt_archive_id`) wird beim Anlegen gesetzt; er steht
//! NICHT in der Immutability-Whitelist und ließe sich bei Bedarf auch
//! nachträglich setzen (analog `quotes::set_pdf_archive_id`).

use crate::db::models::{ExpenseDetail, ExpenseListItem, ExpenseRow};
use crate::domain::expense::ExpenseInput;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub fiscal_year: Option<i64>,
    pub status: Option<String>,
    pub category: Option<String>,
    /// Stornierte ausblenden, wenn `Some(false)`.
    pub include_canceled: Option<bool>,
}

// ---- CREATE ----------------------------------------------------------------

/// Legt eine Kosten-Position an und schreibt sie **sofort fest**
/// (`status='recorded'`, `locked_at=now`). `expense_number` allokiert der
/// Caller über die Counter-Schicht. `vendor_name_snapshot` = `input.vendor_name`.
pub async fn create(
    pool: &SqlitePool,
    input: &ExpenseInput,
    expense_number: &str,
    fiscal_year: i64,
    receipt_archive_id: Option<&str>,
) -> Result<ExpenseRow> {
    let mut conn = pool.acquire().await?;
    let id = insert_in(
        &mut conn,
        input,
        expense_number,
        fiscal_year,
        receipt_archive_id,
        None,
    )
    .await?;
    drop(conn);

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create: post-INSERT SELECT leer".into()))
}

/// INSERT einer sofort festgeschriebenen Kosten-Position innerhalb einer
/// laufenden Transaktion (Abo-Lauf: Nummer, Beleg, Abo-Bezug, Audit und
/// Vorrücken gemeinsam, Lauf F W14-022). Liefert die neue ID.
pub async fn insert_in(
    conn: &mut SqliteConnection,
    input: &ExpenseInput,
    expense_number: &str,
    fiscal_year: i64,
    receipt_archive_id: Option<&str>,
    recurring_subscription_id: Option<&str>,
) -> Result<String> {
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO expenses (
            id, expense_number, fiscal_year, expense_date, paid_date,
            paid_from_account_id, vendor_contact_id, vendor_name_snapshot,
            vendor_invoice_number, category, description,
            net_amount_cents, tax_amount_cents, gross_amount_cents, currency_code,
            reverse_charge_13b, receipt_archive_id, recurring_subscription_id,
            status, locked_at, notes
         ) VALUES (?, ?, ?, ?, ?,  ?, ?, ?,  ?, ?, ?,  ?, ?, ?, ?,  ?, ?, ?,
                   'recorded', datetime('now','utc'), ?)",
    )
    .bind(&id)
    .bind(expense_number)
    .bind(fiscal_year)
    .bind(input.expense_date.to_string())
    .bind(input.paid_date.map(|d| d.to_string()))
    .bind(input.paid_from_account_id.as_deref())
    .bind(input.vendor_contact_id.as_deref())
    .bind(input.vendor_name.trim())
    .bind(input.vendor_invoice_number.as_deref())
    .bind(&input.category)
    .bind(input.description.trim())
    .bind(input.net_amount_cents)
    .bind(input.tax_amount_cents)
    .bind(input.gross_amount_cents)
    .bind(&input.currency_code)
    .bind(if input.reverse_charge_13b { 1i64 } else { 0i64 })
    .bind(receipt_archive_id)
    .bind(recurring_subscription_id)
    .bind(input.notes.as_deref())
    .execute(conn)
    .await?;
    Ok(id)
}

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<ExpenseRow>> {
    let row: Option<ExpenseRow> = sqlx::query_as("SELECT * FROM expenses WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Wie [`get`], auf einer Verbindung bzw. in einer laufenden Transaktion.
pub async fn get_in(conn: &mut SqliteConnection, id: &str) -> Result<Option<ExpenseRow>> {
    let row: Option<ExpenseRow> = sqlx::query_as("SELECT * FROM expenses WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await?;
    Ok(row)
}

/// Expense + Lieferanten-Kontakt (optional) + zusätzliche Anhänge
/// (`attachments` mit `parent_type='expense'`).
pub async fn get_detail(pool: &SqlitePool, id: &str) -> Result<Option<ExpenseDetail>> {
    let Some(expense) = get(pool, id).await? else {
        return Ok(None);
    };
    let vendor = match expense.vendor_contact_id.as_deref() {
        Some(cid) => crate::db::repo::contacts::get(pool, cid).await?,
        None => None,
    };
    let attachments = crate::db::repo::attachments::list_for_parent(pool, "expense", id).await?;
    let source_format =
        detect_receipt_source_format(pool, expense.receipt_archive_id.as_deref()).await;
    Ok(Some(ExpenseDetail {
        expense,
        vendor,
        attachments,
        source_format,
    }))
}

/// PV1-A5: grobe Format-Erkennung für den „Roh-XML anzeigen"-Button. Schaut
/// in `archive_entries`, ob der primäre Beleg eine empfangene E-Rechnung ist,
/// und liefert dann `"zugferd"` (PDF/A-3) oder `"xrechnung-cii"` (XML). Die
/// feine CII/UBL-Unterscheidung passiert erst im Viewer-Command, der die
/// Bytes ohnehin liest. Fehler werden bewusst als `None` geschluckt — die
/// Funktion ist rein UI-relevant und darf den Detail-Load nicht abreißen.
async fn detect_receipt_source_format(
    pool: &SqlitePool,
    archive_id: Option<&str>,
) -> Option<String> {
    let id = archive_id?;
    let row = sqlx::query("SELECT mime_type, source FROM archive_entries WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()?;
    use sqlx::Row;
    let source: String = row.try_get("source").ok()?;
    if source != "received_einvoice" {
        return None;
    }
    let mime: String = row.try_get("mime_type").ok()?;
    match mime.as_str() {
        "application/pdf" => Some("zugferd".to_string()),
        "application/xml" => Some("xrechnung-cii".to_string()),
        _ => None,
    }
}

pub async fn list(pool: &SqlitePool, filter: &ListFilter) -> Result<Vec<ExpenseListItem>> {
    let mut sql = String::from(
        "SELECT id, expense_number, fiscal_year, expense_date, paid_date,
                vendor_name_snapshot, vendor_invoice_number, category, description,
                gross_amount_cents, currency_code,
                status, reverse_charge_13b, receipt_archive_id,
                recurring_subscription_id
           FROM expenses
          WHERE 1=1",
    );
    if filter.fiscal_year.is_some() {
        sql.push_str(" AND fiscal_year = ?");
    }
    if filter.status.is_some() {
        sql.push_str(" AND status = ?");
    }
    if filter.category.is_some() {
        sql.push_str(" AND category = ?");
    }
    if matches!(filter.include_canceled, Some(false)) {
        sql.push_str(" AND status != 'canceled'");
    }
    sql.push_str(" ORDER BY fiscal_year DESC, expense_number DESC");

    let mut q = sqlx::query_as::<_, ExpenseListItem>(&sql);
    if let Some(y) = filter.fiscal_year {
        q = q.bind(y);
    }
    if let Some(s) = filter.status.as_deref() {
        q = q.bind(s);
    }
    if let Some(c) = filter.category.as_deref() {
        q = q.bind(c);
    }
    Ok(q.fetch_all(pool).await?)
}

// ---- CANCEL (Storno statt Löschung) ----------------------------------------

/// GoBD-konformes Stornieren: `status='canceled'` + Grund. Kein Update der
/// Kernfelder (durch `trg_expenses_immutable` ohnehin geschützt).
pub async fn cancel(pool: &SqlitePool, id: &str, reason: &str) -> Result<()> {
    let status = current_status(pool, id).await?;
    if status == "canceled" {
        return Err(Error::Domain(format!(
            "Kosten {id} sind bereits storniert."
        )));
    }
    let res = sqlx::query(
        "UPDATE expenses SET
            status = 'canceled',
            canceled_at = datetime('now','utc'),
            canceled_reason = ?
         WHERE id = ? AND status = 'recorded' AND capitalized_as_asset_id IS NULL",
    )
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        // Lauf F8: auch eine inzwischen aktivierte Kostenposition bleibt stehen
        // (Bedingung im UPDATE gegen ein gleichzeitiges Aktivieren).
        return Err(Error::Domain(format!(
            "Die Kosten {id} lassen sich nicht stornieren: nicht gefunden, schon \
             storniert oder als Anlage aktiviert."
        )));
    }
    Ok(())
}

/// Setzt das Zahlungs-Datum (Cash-Basis) + optional das Zahl-Konto nach dem
/// Anlegen — z. B. um „noch nicht bezahlt" später als bezahlt zu markieren.
/// `paid_date = None` setzt es wieder auf „offen" (Fehl-Markierung korrigieren).
///
/// Erlaubt durch `trg_expenses_immutable`: `paid_date`/`paid_from_account_id`
/// sind keine Beleg-Kernfelder. Nur für nicht-stornierte Kosten. Jede Änderung
/// wird (im Command) im Audit-Log protokolliert — das ist der GoBD-Nachweis.
pub async fn set_payment(
    pool: &SqlitePool,
    id: &str,
    paid_date: Option<&str>,
    paid_from_account_id: Option<&str>,
) -> Result<ExpenseRow> {
    let status = current_status(pool, id).await?;
    if status == "canceled" {
        return Err(Error::Domain(format!(
            "Kosten {id} sind storniert — keine Zahlungs-Markierung möglich."
        )));
    }
    let res =
        sqlx::query("UPDATE expenses SET paid_date = ?, paid_from_account_id = ? WHERE id = ?")
            .bind(paid_date)
            .bind(paid_from_account_id)
            .bind(id)
            .execute(pool)
            .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_payment: Kosten {id} nicht gefunden"
        )));
    }
    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("set_payment: post-UPDATE SELECT leer".into()))
}

/// Wie [`set_payment`], in einer laufenden Transaktion (Lauf F8c: Prüfung
/// aktivierter Kosten und Änderung unter derselben Sperre). Nur für
/// nicht-stornierte Kosten (Bedingung im UPDATE).
pub async fn set_payment_in(
    conn: &mut SqliteConnection,
    id: &str,
    paid_date: Option<&str>,
    paid_from_account_id: Option<&str>,
) -> Result<ExpenseRow> {
    let res = sqlx::query(
        "UPDATE expenses SET paid_date = ?, paid_from_account_id = ?
          WHERE id = ? AND status = 'recorded'",
    )
    .bind(paid_date)
    .bind(paid_from_account_id)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "Kosten {id} nicht gefunden oder storniert — keine Zahlungs-Markierung möglich."
        )));
    }
    get_in(&mut *conn, id)
        .await?
        .ok_or_else(|| Error::Domain("set_payment: post-UPDATE SELECT leer".into()))
}

/// Setzt/ersetzt den primären Beleg nach dem Anlegen. Erlaubt durch
/// `trg_expenses_immutable` (receipt_archive_id ist kein Kernfeld).
pub async fn set_receipt_archive_id(pool: &SqlitePool, id: &str, archive_id: &str) -> Result<()> {
    let res = sqlx::query("UPDATE expenses SET receipt_archive_id = ? WHERE id = ?")
        .bind(archive_id)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_receipt_archive_id: Kosten {id} nicht gefunden"
        )));
    }
    Ok(())
}

/// Persistiert das KoSIT-Validierungsergebnis einer **empfangenen** E-Rechnung
/// (Block 11). `status` ist `passed`|`warning`|`failed`, `report_json` die
/// verdichtete [`crate::einvoice::types::ValidationSummary`] als JSON.
///
/// Erlaubt durch `trg_expenses_immutable`: beide Spalten sind keine
/// Beleg-Kernfelder. Die Validierung ist beratend — sie blockiert den Import
/// nie (eine eingegangene Rechnung muss fürs EÜR unabhängig vom Formal-Befund
/// erfasst werden); der Befund wird hier nur revisionssicher dokumentiert.
pub async fn set_einvoice_validation(
    pool: &SqlitePool,
    id: &str,
    status: Option<&str>,
    report_json: Option<&str>,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE expenses SET einvoice_validation_status = ?, einvoice_validation_report = ? WHERE id = ?",
    )
    .bind(status)
    .bind(report_json)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_einvoice_validation: Kosten {id} nicht gefunden"
        )));
    }
    Ok(())
}

/// Verknüpft eine Kosten-Position mit dem auslösenden Abo (Block 10). Erlaubt
/// durch `trg_expenses_immutable` (recurring_subscription_id ist kein Kernfeld).
pub async fn set_recurring_subscription_id(
    pool: &SqlitePool,
    id: &str,
    recurring_subscription_id: &str,
) -> Result<()> {
    let res = sqlx::query("UPDATE expenses SET recurring_subscription_id = ? WHERE id = ?")
        .bind(recurring_subscription_id)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_recurring_subscription_id: Kosten {id} nicht gefunden"
        )));
    }
    Ok(())
}

/// Verknüpft eine Kosten-Position mit der daraus aktivierten Anlage (Block 12).
/// Erlaubt durch `trg_expenses_immutable` (capitalized_as_asset_id ist kein
/// Kernfeld). So lässt sich vom Kosten-Beleg auf die Anlage springen und doppelte
/// Aktivierung im UI verhindern.
pub async fn set_capitalized_asset_id(pool: &SqlitePool, id: &str, asset_id: &str) -> Result<()> {
    let res = sqlx::query("UPDATE expenses SET capitalized_as_asset_id = ? WHERE id = ?")
        .bind(asset_id)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_capitalized_asset_id: Kosten {id} nicht gefunden"
        )));
    }
    Ok(())
}

async fn current_status(pool: &SqlitePool, id: &str) -> Result<String> {
    let row = sqlx::query("SELECT status FROM expenses WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| Error::Domain(format!("Kosten nicht gefunden: {id}")))?;
    Ok(row.try_get("status")?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
