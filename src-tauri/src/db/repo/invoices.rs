//! Repository für `invoices` + `invoice_items`.
//!
//! Schicht: **Imperative Shell**. Alle DB-I/O lebt hier; Domain-Logik
//! (Validation, Totals, Klausel) kommt aus [`crate::domain::invoice`].
//!
//! ## Lifecycle einer Rechnung
//!
//! ```text
//! draft ──issue──▶ issued ──send──▶ sent ──record_payment──▶ partially_paid ──▶ paid
//!   │
//!   └──cancel via Storno──▶ canceled (durch trg_invoices_immutable geschützt)
//! ```
//!
//! - `draft` ist veränderbar; `issued` und alles danach sind über
//!   `trg_invoices_immutable` (Migration 0001) gegen Änderung der
//!   Kernfelder gesperrt.
//! - **Storno** wird als **neue** Rechnung mit `is_storno_for = original_id`
//!   angelegt; die Original-Rechnung bekommt `status='canceled'`,
//!   `canceled_at`, `canceled_by_storno_id`.

use crate::db::models::{InvoiceDetail, InvoiceItemRow, InvoiceListItem, InvoiceRow};
use crate::domain::invoice::{InvoiceInput, Totals};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Snapshot der Verkäufer-Stammdaten zum Issue-Zeitpunkt. Wird mit der
/// Rechnung persistiert (siehe Schema-Felder `seller_*`) und nicht mehr
/// verändert — auch wenn das Profil später angepasst wird.
#[derive(Debug, Clone)]
pub struct SellerSnapshot<'a> {
    pub name: &'a str,
    pub street: &'a str,
    pub postal_code: &'a str,
    pub city: &'a str,
    pub tax_number: Option<&'a str>,
    pub vat_id: Option<&'a str>,
}

/// Snapshot der Empfänger-Stammdaten zum Issue-Zeitpunkt (Migration 0004).
/// Friert den Kontakt-Stand auf der Rechnung ein — unabhängig von späteren
/// Kontakt-Änderungen oder DSGVO-Anonymisierung.
#[derive(Debug, Clone)]
pub struct BuyerSnapshot<'a> {
    pub name: &'a str,
    pub street: Option<&'a str>,
    pub postal_code: Option<&'a str>,
    pub city: Option<&'a str>,
    pub country_code: &'a str,
    pub vat_id: Option<&'a str>,
    pub email: Option<&'a str>,
}

/// Payload für `create_draft`. Combines `InvoiceInput` (Domain) mit den
/// DB-fremden Feldern `contact_id` + Seller-Snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftCreatePayload {
    pub contact_id: String,
    pub fiscal_year: i64,
    pub is_kleinunternehmer: bool,
    pub input: InvoiceInput,
    /// Verknüpfung zum Ursprungsangebot bei der Konvertierung (Block 7).
    /// `None` bei direkt erstellten Rechnungen und Storno-Belegen.
    #[serde(default)]
    pub derived_from_quote_id: Option<String>,
}

/// Optionale Filter für die Listen-Query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub fiscal_year: Option<i64>,
    pub status: Option<String>,
    pub contact_id: Option<String>,
    pub include_canceled: Option<bool>,
}

// ---- CREATE ----------------------------------------------------------------

/// Legt eine Draft-Rechnung an (Status `draft`, kein `locked_at`).
///
/// Berechnet Totals aus `input.items`, persistiert in einer Transaktion.
/// Vergibt `invoice_number` über `db::numbering::next_number` — das
/// "verbraucht" eine Nummer schon im Draft. Das ist Absicht: lückenlose
/// Buchhaltung bedeutet, dass Drafts dieselbe Sequenz teilen wie
/// gelockte Rechnungen. Wenn ein Draft gelöscht wird, bleibt die Lücke;
/// das ist GoBD-dokumentierbar.
pub async fn create_draft(
    pool: &SqlitePool,
    payload: &DraftCreatePayload,
    invoice_number: &str,
    seller: &SellerSnapshot<'_>,
    buyer: &BuyerSnapshot<'_>,
    totals: &Totals,
) -> Result<InvoiceRow> {
    let id = Uuid::now_v7().to_string();
    let direction_str = match payload.input.direction {
        crate::domain::invoice::InvoiceDirection::Issued => "issued",
        crate::domain::invoice::InvoiceDirection::Received => "received",
    };

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction,
            invoice_date, delivery_date, due_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, seller_vat_id,
            net_amount_cents, tax_amount_cents, gross_amount_cents,
            currency_code, is_kleinunternehmer, pdf_template,
            status, notes, payment_note, is_storno_for, cancel_reason,
            buyer_name, buyer_street, buyer_postal_code, buyer_city,
            buyer_country_code, buyer_vat_id, buyer_email,
            derived_from_quote_id
         ) VALUES (?, ?, ?, ?,  ?, ?, ?, ?,  ?, ?, ?, ?,  ?, ?,  ?, ?, ?,  ?, ?, ?,  'draft', ?, ?, ?, ?,  ?, ?, ?, ?,  ?, ?, ?,  ?)",
    )
    .bind(&id)
    .bind(invoice_number)
    .bind(payload.fiscal_year)
    .bind(direction_str)
    .bind(payload.input.invoice_date.to_string())
    .bind(payload.input.delivery_date.map(|d| d.to_string()))
    .bind(payload.input.due_date.map(|d| d.to_string()))
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
    .bind(if payload.is_kleinunternehmer { 1i64 } else { 0i64 })
    .bind(&payload.input.pdf_template)
    .bind(payload.input.notes.as_deref())
    .bind(payload.input.payment_note.as_deref())
    .bind(payload.input.is_storno_for.as_deref())
    .bind(payload.input.cancel_reason.as_deref())
    .bind(buyer.name)
    .bind(buyer.street)
    .bind(buyer.postal_code)
    .bind(buyer.city)
    .bind(buyer.country_code)
    .bind(buyer.vat_id)
    .bind(buyer.email)
    .bind(payload.derived_from_quote_id.as_deref())
    .execute(&mut *tx)
    .await?;

    for (item, computed) in payload.input.items.iter().zip(totals.items.iter()) {
        let item_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO invoice_items (
                id, invoice_id, position, description, quantity, unit_code,
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

// ---- READ ------------------------------------------------------------------

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<InvoiceRow>> {
    let row: Option<InvoiceRow> = sqlx::query_as("SELECT * FROM invoices WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_by_number(pool: &SqlitePool, invoice_number: &str) -> Result<Option<InvoiceRow>> {
    let row: Option<InvoiceRow> = sqlx::query_as("SELECT * FROM invoices WHERE invoice_number = ?")
        .bind(invoice_number)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_items(pool: &SqlitePool, invoice_id: &str) -> Result<Vec<InvoiceItemRow>> {
    let rows: Vec<InvoiceItemRow> =
        sqlx::query_as("SELECT * FROM invoice_items WHERE invoice_id = ? ORDER BY position")
            .bind(invoice_id)
            .fetch_all(pool)
            .await?;
    Ok(rows)
}

/// Liefert Invoice + Items + Buyer-Snapshot in einem Aufruf.
pub async fn get_detail(pool: &SqlitePool, id: &str) -> Result<Option<InvoiceDetail>> {
    let Some(invoice) = get(pool, id).await? else {
        return Ok(None);
    };
    let items = get_items(pool, id).await?;
    let buyer = crate::db::repo::contacts::get(pool, &invoice.contact_id).await?;
    Ok(Some(InvoiceDetail {
        invoice,
        items,
        buyer,
    }))
}

/// Schreibt/aktualisiert den Buyer-Snapshot einer Rechnung (Block 19,
/// "Refresh bei Lock"). Nur auf nicht-festgeschriebenen Rechnungen erlaubt
/// (`locked_at IS NULL`) — gelockte Belege sind unveränderlich (GoBD).
pub async fn set_buyer_snapshot(
    pool: &SqlitePool,
    id: &str,
    buyer: &BuyerSnapshot<'_>,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE invoices SET
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
            "set_buyer_snapshot: Rechnung nicht gefunden oder bereits festgeschrieben: {id}"
        )));
    }
    Ok(())
}

/// Listen-Query mit Buyer-Name-Join. Filtert standardmäßig stornierte
/// Rechnungen NICHT aus — Buchhaltung muss sie sehen. UI kann
/// `include_canceled = false` setzen.
pub async fn list(pool: &SqlitePool, filter: &ListFilter) -> Result<Vec<InvoiceListItem>> {
    let mut sql = String::from(
        "SELECT i.id, i.invoice_number, i.fiscal_year, i.invoice_date, i.due_date,
                i.contact_id, c.name AS contact_name,
                i.gross_amount_cents, i.paid_amount_cents, i.currency_code, i.status,
                i.is_storno_for
           FROM invoices i
           JOIN contacts c ON c.id = i.contact_id
          WHERE 1=1",
    );
    if filter.fiscal_year.is_some() {
        sql.push_str(" AND i.fiscal_year = ?");
    }
    if filter.status.is_some() {
        sql.push_str(" AND i.status = ?");
    }
    if filter.contact_id.is_some() {
        sql.push_str(" AND i.contact_id = ?");
    }
    // "Stornovorgänge einschließen" = false versteckt BEIDE Seiten eines
    // Storno-Vorgangs: das aufgehobene Original (status='canceled') UND den
    // Storno-Beleg selbst (is_storno_for gesetzt). Sonst wäre die Ansicht
    // inkonsistent — Original weg, ST-Beleg bliebe stehen.
    if matches!(filter.include_canceled, Some(false)) {
        sql.push_str(" AND i.status != 'canceled' AND i.is_storno_for IS NULL");
    }
    sql.push_str(" ORDER BY i.fiscal_year DESC, i.invoice_number DESC");

    let mut q = sqlx::query_as::<_, InvoiceListItem>(&sql);
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

// ---- LOCK / ISSUE ----------------------------------------------------------

pub struct LockUpdate<'a> {
    pub validation_status: &'a str, // 'passed' | 'warning' | 'failed'
    pub validation_report: Option<&'a str>,
    pub pdf_archive_id: &'a str,
    pub xml_archive_id: &'a str,
}

/// Storno-Paar-Operation: beim Lock einer Storno-Rechnung gleichzeitig die
/// Original-Rechnung als `canceled` markieren (R1-003, v2026.5-Re-Review).
///
/// Beide UPDATEs laufen in derselben Transaktion — entweder beide oder keiner.
/// Schließt die GoBD-Lücke: stromaus zwischen `lock(storno)` und
/// `mark_canceled(original)` ließ ein gelocktes Storno + uncanceled Original
/// zurück → EÜR-Doppel-Refund-Effekt.
#[derive(Debug, Clone, Copy)]
pub struct PairCancel<'a> {
    pub original_id: &'a str,
    pub reason: Option<&'a str>,
}

/// Setzt `locked_at = now`, `status = 'issued'`, archive-IDs,
/// validation_status/_report. Schlägt fehl, wenn die Rechnung bereits
/// gelockt ist (Doppel-Lock-Schutz). Convenience-Wrapper um
/// [`lock_with_pair_cancel`] ohne Storno-Pair.
pub async fn lock(pool: &SqlitePool, id: &str, update: &LockUpdate<'_>) -> Result<()> {
    lock_with_pair_cancel(pool, id, update, None).await
}

/// Wie [`lock`], optional mit Storno-Paar-Cancel: wenn `pair` gesetzt ist,
/// wird die `original_id`-Rechnung im SELBEN Transaktions-Block als
/// `status='canceled'` markiert. Beide UPDATEs sind atomar — Stromaus zwischen
/// Lock des Storno-Belegs und Cancel des Originals lässt einen konsistenten
/// (rollback) Zustand zurück.
pub async fn lock_with_pair_cancel(
    pool: &SqlitePool,
    id: &str,
    update: &LockUpdate<'_>,
    pair: Option<PairCancel<'_>>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    // Vor-Check: schon gelockt? Lesen INNERHALB der TX, damit der WHERE-Guard
    // im UPDATE-Statement gegen TOCTOU-Races zwischen Pre-Check und UPDATE
    // robust ist (R1-S5-005 → S1-STO-3 von Subagent 10).
    let row = sqlx::query("SELECT locked_at FROM invoices WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| Error::Domain(format!("invoice nicht gefunden: {id}")))?;
    let locked_at: Option<String> = row.try_get("locked_at")?;
    if locked_at.is_some() {
        return Err(Error::Domain(format!(
            "invoice {id} ist bereits gelockt (locked_at={:?})",
            locked_at
        )));
    }

    // 1. Storno (bzw. normale Rechnung) locken.
    let res = sqlx::query(
        "UPDATE invoices SET
            locked_at = datetime('now','utc'),
            status = 'issued',
            validation_status = ?,
            validation_report = ?,
            validated_at = datetime('now','utc'),
            pdf_archive_id = ?,
            xml_archive_id = ?,
            updated_at = datetime('now','utc')
         WHERE id = ? AND locked_at IS NULL",
    )
    .bind(update.validation_status)
    .bind(update.validation_report)
    .bind(update.pdf_archive_id)
    .bind(update.xml_archive_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "lock: invoice {id} nicht gefunden oder bereits gelockt (Race)"
        )));
    }

    // 2. Optional: Original-Rechnung im SELBEN TX als 'canceled' markieren.
    if let Some(pair) = pair {
        let res = sqlx::query(
            "UPDATE invoices SET
                status = 'canceled',
                canceled_at = datetime('now','utc'),
                canceled_by_storno_id = ?,
                cancel_reason = COALESCE(?, cancel_reason),
                updated_at = datetime('now','utc')
             WHERE id = ? AND status != 'canceled'",
        )
        .bind(id) // storno-id wird canceled_by_storno_id
        .bind(pair.reason)
        .bind(pair.original_id)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() == 0 {
            return Err(Error::Domain(format!(
                "mark_canceled: invoice {} nicht gefunden oder bereits storniert",
                pair.original_id
            )));
        }
    }

    tx.commit().await?;
    Ok(())
}

// ---- PAYMENT ---------------------------------------------------------------

/// Buchung einer (Teil-)Zahlung. Aktualisiert `paid_amount_cents`,
/// `paid_at` (Cash-Basis-Eingangsdatum für EÜR), `payment_history_json`,
/// und übergibt den Status automatisch:
///
/// - `paid_amount_cents == gross_amount_cents` → `paid`
/// - `0 < paid_amount_cents < gross_amount_cents` → `partially_paid`
/// - `paid_amount_cents == 0` → keine Statusänderung
pub async fn record_payment(
    pool: &SqlitePool,
    id: &str,
    amount_cents: i64,
    paid_date_iso: &str,
    note: Option<&str>,
) -> Result<InvoiceRow> {
    if amount_cents <= 0 {
        return Err(Error::Domain(format!(
            "record_payment: amount_cents muss > 0 sein, got {amount_cents}"
        )));
    }

    let mut tx = pool.begin().await?;

    let invoice = sqlx::query(
        "SELECT gross_amount_cents, paid_amount_cents, payment_history_json, status, locked_at
         FROM invoices WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| Error::Domain(format!("invoice nicht gefunden: {id}")))?;

    let gross: i64 = invoice.try_get("gross_amount_cents")?;
    let prev_paid: i64 = invoice.try_get("paid_amount_cents")?;
    let locked: Option<String> = invoice.try_get("locked_at")?;
    let status: String = invoice.try_get("status")?;
    let history_str: Option<String> = invoice.try_get("payment_history_json")?;

    if locked.is_none() {
        return Err(Error::Domain(format!(
            "record_payment: invoice {id} ist nicht gelockt (status={status})"
        )));
    }
    if status == "canceled" {
        return Err(Error::Domain(format!(
            "record_payment: invoice {id} ist storniert"
        )));
    }

    let new_paid = prev_paid + amount_cents;
    if new_paid > gross {
        return Err(Error::Domain(format!(
            "record_payment: Summe {new_paid} > gross {gross} (Überzahlung)"
        )));
    }

    // History pflegen
    let mut history: Vec<PaymentEntry> = match history_str {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => Vec::new(),
    };
    history.push(PaymentEntry {
        amount_cents,
        paid_date: paid_date_iso.to_string(),
        note: note.map(|s| s.to_string()),
        recorded_at: chrono::Utc::now().to_rfc3339(),
    });
    let history_json = serde_json::to_string(&history)?;

    let new_status = if new_paid == gross {
        "paid"
    } else {
        "partially_paid"
    };

    sqlx::query(
        "UPDATE invoices SET
            paid_amount_cents = ?,
            paid_at = ?,
            payment_history_json = ?,
            status = ?,
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(new_paid)
    .bind(paid_date_iso)
    .bind(&history_json)
    .bind(new_status)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("record_payment: post-UPDATE SELECT leer".into()))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaymentEntry {
    pub amount_cents: i64,
    pub paid_date: String,
    pub note: Option<String>,
    pub recorded_at: String,
}

// ---- CANCEL ----------------------------------------------------------------

/// Markiert eine Original-Rechnung als storniert. Erlaubt durch
/// `trg_invoices_immutable`, weil `canceled_at` + `canceled_by_storno_id`
/// nicht in der Trigger-Whitelist sind (siehe Migration 0001).
pub async fn mark_canceled(
    pool: &SqlitePool,
    original_id: &str,
    storno_id: &str,
    reason: Option<&str>,
) -> Result<()> {
    let res = sqlx::query(
        "UPDATE invoices SET
            status = 'canceled',
            canceled_at = datetime('now','utc'),
            canceled_by_storno_id = ?,
            cancel_reason = COALESCE(?, cancel_reason),
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(storno_id)
    .bind(reason)
    .bind(original_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "mark_canceled: invoice {original_id} nicht gefunden"
        )));
    }
    Ok(())
}

// ---- SEND ------------------------------------------------------------------

/// Markiert eine gelockte Rechnung als versendet. Setzt `sent_at`
/// (Erst-Versand-Zeitpunkt; bleibt bei weiteren Sends erhalten) und hebt den
/// Status von `issued` auf `sent` an. Bezahlt/teilbezahlt/storniert bleiben
/// unverändert — ein erneuter Versand darf den Status nie zurückdrehen.
///
/// Erlaubt durch `trg_invoices_immutable`: `status` und `sent_at` stehen
/// nicht in der Immutability-Whitelist der Kernfelder.
pub async fn mark_sent(pool: &SqlitePool, id: &str, sent_at_iso: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE invoices SET
            sent_at = COALESCE(sent_at, ?),
            status = CASE WHEN status = 'issued' THEN 'sent' ELSE status END,
            updated_at = datetime('now','utc')
         WHERE id = ? AND locked_at IS NOT NULL AND status != 'canceled'",
    )
    .bind(sent_at_iso)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "mark_sent: invoice {id} nicht gefunden, nicht gelockt oder storniert"
        )));
    }
    Ok(())
}

// ---- Re-Export für Bequemlichkeit ------------------------------------------

pub use crate::db::models::{ContactRow as Buyer, InvoiceDetail as Detail};

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
