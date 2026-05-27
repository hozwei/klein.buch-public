//! Repository für `payment_accounts` (Block 9).
//!
//! Schicht: **Imperative Shell**. Zahlungs-Konten (Bank/Bargeld/PayPal/…) sind
//! Stammdaten — sie sind NICHT GoBD-gelockt, werden aber von `expenses`,
//! `invoices` und `private_movements` per FK referenziert. Daher **kein
//! Hard-Delete** (würde FKs brechen); stattdessen `active = 0` (deaktivieren).
//!
//! ## is_default
//!
//! Höchstens ein Konto ist `is_default = 1`. Beim Setzen wird in einer
//! Transaktion zuerst bei allen anderen `is_default = 0` gesetzt.

use crate::db::models::PaymentAccountRow;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Erlaubte Konto-Typen — synchron zum CHECK in `0007_expenses.sql`.
pub const VALID_TYPES: &[&str] = &["bank", "cash", "paypal", "stripe", "other"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentAccountInput {
    pub label: String,
    pub account_type: String,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub is_default: bool,
    #[serde(default)]
    pub show_on_invoice: bool,
    #[serde(default)]
    pub details: Option<String>,
}

fn validate(input: &PaymentAccountInput) -> Result<()> {
    if input.label.trim().is_empty() {
        return Err(Error::Domain(
            "Konto-Bezeichnung darf nicht leer sein.".into(),
        ));
    }
    if !VALID_TYPES.contains(&input.account_type.as_str()) {
        return Err(Error::Domain(format!(
            "Ungültiger Konto-Typ '{}' (erlaubt: {}).",
            input.account_type,
            VALID_TYPES.join(", ")
        )));
    }
    Ok(())
}

fn clean(opt: Option<&str>) -> Option<String> {
    opt.map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Erzwingt die Default-Invariante: ein **inaktives** Konto ist nie Standard,
/// und solange mindestens **ein aktives** Konto existiert, ist **genau eines**
/// davon Standard. Wird nach jeder Mutation (create/update/set_active) gerufen.
///
/// - inaktive Defaults werden geräumt;
/// - bei 0 aktiven Defaults wird das älteste aktive Konto (UUIDv7-PK = zeit-
///   sortiert) zum Standard befördert — so geht beim Deaktivieren oder
///   Ent-Haken des Standard-Kontos der Default nicht verloren;
/// - bei >1 aktiven Defaults bleibt nur das älteste, der Rest wird geräumt
///   (defensiv; sollte durch die clear-others-Logik nicht vorkommen).
async fn ensure_one_default(pool: &SqlitePool) -> Result<()> {
    use sqlx::Row;

    // Inaktive Konten dürfen nicht Standard sein.
    sqlx::query("UPDATE payment_accounts SET is_default = 0 WHERE active = 0 AND is_default = 1")
        .execute(pool)
        .await?;

    let n: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM payment_accounts WHERE active = 1 AND is_default = 1",
    )
    .fetch_one(pool)
    .await?
    .try_get("n")?;

    if n == 0 {
        // Ältestes aktives Konto zum Standard machen (falls es eines gibt).
        sqlx::query(
            "UPDATE payment_accounts SET is_default = 1
              WHERE id = (SELECT id FROM payment_accounts WHERE active = 1
                          ORDER BY id ASC LIMIT 1)",
        )
        .execute(pool)
        .await?;
    } else if n > 1 {
        sqlx::query(
            "UPDATE payment_accounts SET is_default = 0
              WHERE active = 1 AND is_default = 1
                AND id != (SELECT id FROM payment_accounts WHERE active = 1 AND is_default = 1
                           ORDER BY id ASC LIMIT 1)",
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

// ---- READ ------------------------------------------------------------------

/// Spaltenliste mit `"type" AS account_type` (DB-Spalte `type` ist SQL-Keyword).
const COLS: &str = "id, label, \"type\" AS account_type, iban, bic, is_default, active, show_on_invoice, details, created_at";

pub async fn list(pool: &SqlitePool, include_inactive: bool) -> Result<Vec<PaymentAccountRow>> {
    let sql = if include_inactive {
        format!("SELECT {COLS} FROM payment_accounts ORDER BY is_default DESC, label COLLATE NOCASE ASC")
    } else {
        format!(
            "SELECT {COLS} FROM payment_accounts WHERE active = 1
             ORDER BY is_default DESC, label COLLATE NOCASE ASC"
        )
    };
    let rows: Vec<PaymentAccountRow> = sqlx::query_as(&sql).fetch_all(pool).await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<PaymentAccountRow>> {
    let row: Option<PaymentAccountRow> =
        sqlx::query_as(&format!("SELECT {COLS} FROM payment_accounts WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

/// Alle **aktiven** Konten, die auf Belegen erscheinen sollen (mehrere möglich).
/// Standard-Konto zuerst, dann alphabetisch. Quelle der Bankdaten fürs PDF + XML.
pub async fn invoice_accounts(pool: &SqlitePool) -> Result<Vec<PaymentAccountRow>> {
    let rows: Vec<PaymentAccountRow> = sqlx::query_as(&format!(
        "SELECT {COLS} FROM payment_accounts
         WHERE active = 1 AND show_on_invoice = 1
         ORDER BY is_default DESC, label COLLATE NOCASE ASC"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---- WRITE -----------------------------------------------------------------

pub async fn create(pool: &SqlitePool, input: &PaymentAccountInput) -> Result<PaymentAccountRow> {
    validate(input)?;
    let id = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await?;

    if input.is_default {
        sqlx::query("UPDATE payment_accounts SET is_default = 0")
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(
        "INSERT INTO payment_accounts (id, label, \"type\", iban, bic, is_default, active, show_on_invoice, details)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(input.label.trim())
    .bind(&input.account_type)
    .bind(clean(input.iban.as_deref()))
    .bind(clean(input.bic.as_deref()))
    .bind(if input.is_default { 1i64 } else { 0i64 })
    .bind(if input.show_on_invoice { 1i64 } else { 0i64 })
    .bind(clean(input.details.as_deref()))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    ensure_one_default(pool).await?;
    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create: post-INSERT SELECT leer".into()))
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: &PaymentAccountInput,
) -> Result<PaymentAccountRow> {
    validate(input)?;
    let mut tx = pool.begin().await?;

    if input.is_default {
        sqlx::query("UPDATE payment_accounts SET is_default = 0 WHERE id != ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    let res = sqlx::query(
        "UPDATE payment_accounts
            SET label = ?, \"type\" = ?, iban = ?, bic = ?, is_default = ?, show_on_invoice = ?, details = ?
          WHERE id = ?",
    )
    .bind(input.label.trim())
    .bind(&input.account_type)
    .bind(clean(input.iban.as_deref()))
    .bind(clean(input.bic.as_deref()))
    .bind(if input.is_default { 1i64 } else { 0i64 })
    .bind(if input.show_on_invoice { 1i64 } else { 0i64 })
    .bind(clean(input.details.as_deref()))
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Konto nicht gefunden: {id}")));
    }

    tx.commit().await?;
    ensure_one_default(pool).await?;
    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("update: post-UPDATE SELECT leer".into()))
}

/// Aktiviert/deaktiviert ein Konto (kein Hard-Delete wegen FK-Referenzen).
/// Beim Deaktivieren des Standard-Kontos wird automatisch ein anderes aktives
/// Konto zum Standard befördert ([`ensure_one_default`]).
pub async fn set_active(pool: &SqlitePool, id: &str, active: bool) -> Result<()> {
    let res = sqlx::query("UPDATE payment_accounts SET active = ? WHERE id = ?")
        .bind(if active { 1i64 } else { 0i64 })
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Konto nicht gefunden: {id}")));
    }
    ensure_one_default(pool).await?;
    Ok(())
}

/// Idempotenter Seed: legt — falls noch GAR kein Konto existiert — die zwei
/// Standard-Konten "Hauptkonto" (bank, default) und "Bargeld" (cash) an.
/// Liefert `true`, wenn geseedet wurde.
pub async fn ensure_defaults(pool: &SqlitePool) -> Result<bool> {
    use sqlx::Row;
    let n: i64 = sqlx::query("SELECT COUNT(*) AS n FROM payment_accounts")
        .fetch_one(pool)
        .await?
        .try_get("n")?;
    if n > 0 {
        return Ok(false);
    }
    create(
        pool,
        &PaymentAccountInput {
            label: "Hauptkonto".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: true,
            show_on_invoice: false,
            details: None,
        },
    )
    .await?;
    create(
        pool,
        &PaymentAccountInput {
            label: "Bargeld".into(),
            account_type: "cash".into(),
            iban: None,
            bic: None,
            is_default: false,
            show_on_invoice: false,
            details: None,
        },
    )
    .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
