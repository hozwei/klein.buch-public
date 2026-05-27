//! Repository für `quotes` + `quote_items` (Block 6).
//!
//! Schicht: **Imperative Shell**. Alle DB-I/O lebt hier; Domain-Logik
//! (Validation, Totals) kommt aus [`crate::domain::quote`].
//!
//! ## Lifecycle eines Angebots
//!
//! ```text
//! draft ──issue──▶ sent ──accept──▶ accepted ──(Block 7)──▶ converted
//!                   │
//!                   ├──reject──▶ rejected
//!                   └──cancel──▶ canceled
//! ```
//!
//! - `draft` (locked_at IS NULL) ist frei änderbar.
//! - [`issue`] ("festschreiben") setzt `locked_at` + `status='sent'`; ab da
//!   greift `trg_quotes_immutable` (Migration 0005) auf den Kernfeldern
//!   (GoBD-Hardline). PDF-Erzeugung + Mail-Versand kommen in Block 8.
//! - [`accept`] erfordert `status='sent'`; der unterschriebene Vertrag wird
//!   in der Command-Schicht als Attachment angehängt (Block 6 Feature).
//! - [`crate::db::repo::quotes::cancel`] = GoBD-konformes Zurückziehen (kein
//!   Löschen): Status `canceled` + Grund.

use crate::db::models::{QuoteDetail, QuoteItemRow, QuoteListItem, QuoteRow};
use crate::db::repo::invoices::{BuyerSnapshot, SellerSnapshot};
use crate::domain::invoice::Totals;
use crate::domain::quote::QuoteInput;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Payload für `create_draft`. Kombiniert `QuoteInput` (Domain) mit den
/// DB-fremden Feldern `contact_id` + `fiscal_year` + Kleinunternehmer-Flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftCreatePayload {
    pub contact_id: String,
    pub fiscal_year: i64,
    pub is_kleinunternehmer: bool,
    pub input: QuoteInput,
}

/// Optionale Filter für die Listen-Query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub fiscal_year: Option<i64>,
    pub status: Option<String>,
    pub contact_id: Option<String>,
    /// Storno/abgelehnte ausblenden, wenn `Some(false)`.
    pub include_inactive: Option<bool>,
}

// ---- CREATE ----------------------------------------------------------------

/// Legt ein Draft-Angebot an (Status `draft`, kein `locked_at`).
///
/// Vergibt `quote_number` über die Counter-Schicht (Caller allokiert sie);
/// wie bei Rechnungen "verbraucht" ein Draft bereits eine Nummer, damit der
/// Belegkreis lückenlos bleibt.
pub async fn create_draft(
    pool: &SqlitePool,
    payload: &DraftCreatePayload,
    quote_number: &str,
    seller: &SellerSnapshot<'_>,
    buyer: &BuyerSnapshot<'_>,
    totals: &Totals,
) -> Result<QuoteRow> {
    let id = Uuid::now_v7().to_string();

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO quotes (
            id, quote_number, fiscal_year, quote_date, valid_until, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, seller_vat_id,
            net_amount_cents, tax_amount_cents, gross_amount_cents,
            currency_code, is_kleinunternehmer, pdf_template,
            status, notes,
            buyer_name, buyer_street, buyer_postal_code, buyer_city,
            buyer_country_code, buyer_vat_id, buyer_email
         ) VALUES (?, ?, ?, ?, ?, ?,  ?, ?, ?, ?,  ?, ?,  ?, ?, ?,  ?, ?, ?,  'draft', ?,  ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(quote_number)
    .bind(payload.fiscal_year)
    .bind(payload.input.quote_date.to_string())
    .bind(payload.input.valid_until.to_string())
    .bind(&payload.contact_id)
    .bind(seller.name)
    .bind(seller.street)
    .bind(seller.postal_code)
    .bind(seller.city)
    .bind(seller.tax_number)
    .bind(seller.vat_id)
    .bind(totals.net_amount_cents)
    .bind(totals.tax_amount_cents)
    .bind(totals.gross_amount_cents)
    .bind(&payload.input.currency_code)
    .bind(if payload.is_kleinunternehmer {
        1i64
    } else {
        0i64
    })
    .bind(&payload.input.pdf_template)
    .bind(payload.input.notes.as_deref())
    .bind(buyer.name)
    .bind(buyer.street)
    .bind(buyer.postal_code)
    .bind(buyer.city)
    .bind(buyer.country_code)
    .bind(buyer.vat_id)
    .bind(buyer.email)
    .execute(&mut *tx)
    .await?;

    for (item, computed) in payload.input.items.iter().zip(totals.items.iter()) {
        let item_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO quote_items (
                id, quote_id, position, description, quantity, unit_code,
                unit_price_cents, net_amount_cents, tax_rate_percent, tax_category_code,
                description_title, description_markup, source_package_id, source_package_revision
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?,  ?, ?, ?, ?)",
        )
        .bind(&item_id)
        .bind(&id)
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
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("create_draft: post-INSERT SELECT leer".into()))
}

/// Schreibt/aktualisiert den Buyer-Snapshot eines Angebots (Block 19, Refresh
/// beim Festschreiben). Nur auf nicht-festgeschriebenen Angeboten erlaubt
/// (`locked_at IS NULL`) — gelockte Angebote sind unveränderlich (GoBD).
pub async fn set_buyer_snapshot(
    pool: &SqlitePool,
    id: &str,
    buyer: &BuyerSnapshot<'_>,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE quotes SET
            buyer_name = ?, buyer_street = ?, buyer_postal_code = ?, buyer_city = ?,
            buyer_country_code = ?, buyer_vat_id = ?, buyer_email = ?,
            updated_at = datetime('now','utc')
         WHERE id = ? AND locked_at IS NULL",
    )
    .bind(buyer.name)
    .bind(buyer.street)
    .bind(buyer.postal_code)
    .bind(buyer.city)
    .bind(buyer.country_code)
    .bind(buyer.vat_id)
    .bind(buyer.email)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_buyer_snapshot: Angebot nicht gefunden oder bereits festgeschrieben: {id}"
        )));
    }
    Ok(())
}

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<QuoteRow>> {
    let row: Option<QuoteRow> = sqlx::query_as("SELECT * FROM quotes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_by_number(pool: &SqlitePool, quote_number: &str) -> Result<Option<QuoteRow>> {
    let row: Option<QuoteRow> = sqlx::query_as("SELECT * FROM quotes WHERE quote_number = ?")
        .bind(quote_number)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_items(pool: &SqlitePool, quote_id: &str) -> Result<Vec<QuoteItemRow>> {
    let rows: Vec<QuoteItemRow> =
        sqlx::query_as("SELECT * FROM quote_items WHERE quote_id = ? ORDER BY position")
            .bind(quote_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

/// Liefert Quote + Items + Buyer-Kontakt + Anhänge in einem Aufruf.
pub async fn get_detail(pool: &SqlitePool, id: &str) -> Result<Option<QuoteDetail>> {
    let Some(quote) = get(pool, id).await? else {
        return Ok(None);
    };
    let items = get_items(pool, id).await?;
    let buyer = crate::db::repo::contacts::get(pool, &quote.contact_id).await?;
    let attachments = crate::db::repo::attachments::list_for_parent(pool, "quote", id).await?;
    Ok(Some(QuoteDetail {
        quote,
        items,
        buyer,
        attachments,
    }))
}

/// Listen-Query mit Kontakt-Name-Join.
pub async fn list(pool: &SqlitePool, filter: &ListFilter) -> Result<Vec<QuoteListItem>> {
    let mut sql = String::from(
        "SELECT q.id, q.quote_number, q.fiscal_year, q.quote_date, q.valid_until,
                q.contact_id, c.name AS contact_name,
                q.gross_amount_cents, q.currency_code, q.status
           FROM quotes q
           JOIN contacts c ON c.id = q.contact_id
          WHERE 1=1",
    );
    if filter.fiscal_year.is_some() {
        sql.push_str(" AND q.fiscal_year = ?");
    }
    if filter.status.is_some() {
        sql.push_str(" AND q.status = ?");
    }
    if filter.contact_id.is_some() {
        sql.push_str(" AND q.contact_id = ?");
    }
    if matches!(filter.include_inactive, Some(false)) {
        sql.push_str(" AND q.status NOT IN ('canceled','rejected')");
    }
    sql.push_str(" ORDER BY q.fiscal_year DESC, q.quote_number DESC");

    let mut q = sqlx::query_as::<_, QuoteListItem>(&sql);
    if let Some(y) = filter.fiscal_year {
        q = q.bind(y);
    }
    if let Some(s) = filter.status.as_deref() {
        q = q.bind(s);
    }
    if let Some(c) = filter.contact_id.as_deref() {
        q = q.bind(c);
    }
    let rows = q.fetch_all(pool).await?;
    Ok(rows)
}

// ---- STATE TRANSITIONS -----------------------------------------------------

/// Festschreiben: `locked_at = now`, `status = 'sent'`, `sent_at = now`.
/// Nur aus `draft` erlaubt (Doppel-Lock-Schutz). Macht das Angebot GoBD-
/// unveränderlich. PDF + tatsächlicher Mail-Versand: Block 8.
pub async fn issue(pool: &SqlitePool, id: &str) -> Result<()> {
    let row = sqlx::query("SELECT status, locked_at FROM quotes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {id}")))?;
    let status: String = row.try_get("status")?;
    let locked_at: Option<String> = row.try_get("locked_at")?;
    if locked_at.is_some() || status != "draft" {
        return Err(Error::Domain(format!(
            "Angebot {id} ist nicht im Entwurf (status={status}) — Festschreiben nicht möglich"
        )));
    }

    let res = sqlx::query(
        "UPDATE quotes SET
            locked_at = datetime('now','utc'),
            sent_at = datetime('now','utc'),
            status = 'sent',
            updated_at = datetime('now','utc')
         WHERE id = ? AND locked_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("issue: Angebot {id} nicht gefunden")));
    }
    Ok(())
}

/// Annahme durch den Kunden: `status='sent' → 'accepted'`, `accepted_at`.
/// `accepted_date_iso` ist das fachliche Annahmedatum (Default: Caller).
pub async fn accept(pool: &SqlitePool, id: &str, accepted_date_iso: &str) -> Result<()> {
    let status = current_status(pool, id).await?;
    if status != "sent" {
        return Err(Error::Domain(format!(
            "Annahme nur für versendete Angebote möglich (status={status})"
        )));
    }
    let res = sqlx::query(
        "UPDATE quotes SET
            status = 'accepted',
            accepted_at = ?,
            updated_at = datetime('now','utc')
         WHERE id = ? AND status = 'sent'",
    )
    .bind(accepted_date_iso)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "accept: Angebot {id} nicht aktualisiert"
        )));
    }
    Ok(())
}

/// Ablehnung durch den Kunden: `status='sent' → 'rejected'`, `rejected_at`.
pub async fn reject(pool: &SqlitePool, id: &str) -> Result<()> {
    let status = current_status(pool, id).await?;
    if status != "sent" {
        return Err(Error::Domain(format!(
            "Ablehnung nur für versendete Angebote möglich (status={status})"
        )));
    }
    let res = sqlx::query(
        "UPDATE quotes SET
            status = 'rejected',
            rejected_at = datetime('now','utc'),
            updated_at = datetime('now','utc')
         WHERE id = ? AND status = 'sent'",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "reject: Angebot {id} nicht aktualisiert"
        )));
    }
    Ok(())
}

/// Zurückziehen (GoBD-konform, kein Löschen): `status → 'canceled'`,
/// `canceled_at`, `canceled_reason`. Nicht für bereits konvertierte Angebote.
pub async fn cancel(pool: &SqlitePool, id: &str, reason: &str) -> Result<()> {
    let status = current_status(pool, id).await?;
    if status == "canceled" {
        return Err(Error::Domain(format!("Angebot {id} ist bereits storniert")));
    }
    if status == "converted" {
        return Err(Error::Domain(format!(
            "Angebot {id} wurde bereits in eine Rechnung konvertiert — Storno der Rechnung erfolgt über den Rechnungs-Storno"
        )));
    }
    let res = sqlx::query(
        "UPDATE quotes SET
            status = 'canceled',
            canceled_at = datetime('now','utc'),
            canceled_reason = ?,
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "cancel: Angebot {id} nicht gefunden"
        )));
    }
    Ok(())
}

/// Konvertierung in eine Rechnung abschließen (Block 7): `status='accepted'
/// → 'converted'`, `converted_at`, `converted_invoice_id`.
///
/// **Hard-Rule (Manuel):** Konvertierung NUR aus angenommenen Angeboten —
/// aus einem unterschriebenen/angenommenen Angebot wird die Rechnung. Andere
/// Status (draft/sent/rejected/canceled/converted) werden abgelehnt; das
/// blockt auch eine zweite Konvertierung desselben Angebots.
///
/// Erlaubt durch `trg_quotes_immutable` (Migration 0005): `status`,
/// `converted_at`, `converted_invoice_id` stehen nicht in der Immutability-
/// Whitelist der Kernfelder.
pub async fn mark_converted(pool: &SqlitePool, id: &str, invoice_id: &str) -> Result<()> {
    let status = current_status(pool, id).await?;
    if status != "accepted" {
        return Err(Error::Domain(format!(
            "Konvertierung nur für angenommene Angebote möglich (status={status})"
        )));
    }
    let res = sqlx::query(
        "UPDATE quotes SET
            status = 'converted',
            converted_at = datetime('now','utc'),
            converted_invoice_id = ?,
            updated_at = datetime('now','utc')
         WHERE id = ? AND status = 'accepted'",
    )
    .bind(invoice_id)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "mark_converted: Angebot {id} nicht aktualisiert"
        )));
    }
    Ok(())
}

/// Setzt das Angebots-PDF-Archiv (Block 8). Erlaubt durch `trg_quotes_immutable`
/// (Migration 0005): `pdf_archive_id` steht nicht in der Immutability-Whitelist
/// der Kernfelder, lässt sich also auch auf einem gelockten Angebot setzen.
pub async fn set_pdf_archive_id(pool: &SqlitePool, id: &str, archive_id: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE quotes SET pdf_archive_id = ?, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(archive_id)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "set_pdf_archive_id: Angebot {id} nicht gefunden"
        )));
    }
    Ok(())
}

async fn current_status(pool: &SqlitePool, id: &str) -> Result<String> {
    let row = sqlx::query("SELECT status FROM quotes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {id}")))?;
    Ok(row.try_get("status")?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
