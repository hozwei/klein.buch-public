//! Repository für das Zahlungsbuch `invoice_payments` (Migration 0034,
//! ADR 0042 Nr. 7–9).
//!
//! Schicht: **Imperative Shell**. Zahlungen auf Ausgangsrechnungen sind
//! append-only Zeilen: eine Zahlung (`payment`, Betrag > 0) oder ihre
//! Gegenbuchung (`reversal`, „Zahlung stornieren“: Betrag, Datum und Konto der
//! Zahlung mit umgekehrtem Vorzeichen, Grund Pflicht, höchstens eine je
//! Zahlung). `paid_amount_cents`, `paid_at` und `status` der Rechnung sind ein
//! Cache des Buchs; [`refresh_cache_in`] setzt ihn nach jeder Buchung in
//! derselben Transaktion neu, die Trigger aus 0034 lassen nichts anderes zu.
//!
//! Die Gegenbuchung wirkt am ursprünglichen Zahldatum: eine nie geflossene
//! Zahlung war kein Zufluss nach § 11 EStG. Echte Rückzahlungen an den Kunden
//! bleiben der Storno mit Erstattung (am Tag der Rückzahlung).

use crate::db::models::{InvoicePaymentRow, InvoicePaymentView, InvoiceRow};
use crate::error::{Error, Result};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

/// Art einer Zahlung.
pub const KIND_PAYMENT: &str = "payment";
/// Art einer Gegenbuchung.
pub const KIND_REVERSAL: &str = "reversal";

/// Mindest- und Höchstlänge des Grundes einer Gegenbuchung (wie beim Verwerfen
/// eines Entwurfs; Grenzen auch als CHECK in 0034).
pub const REASON_MIN_CHARS: usize = 10;
pub const REASON_MAX_CHARS: usize = 500;

/// Bereinigt den Grund einer Gegenbuchung (getrimmt, ohne Steuerzeichen,
/// höchstens [`REASON_MAX_CHARS`]) und prüft die Mindestlänge.
pub fn clean_reason(reason: &str) -> Result<String> {
    let reason: String = reason
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(REASON_MAX_CHARS)
        .collect();
    let reason = reason.trim().to_string();
    if reason.chars().count() < REASON_MIN_CHARS {
        return Err(Error::BadRequest(
            "Bitte einen Grund angeben (mindestens 10 Zeichen) — er steht an der Gegenbuchung \
             und im Audit-Log."
                .into(),
        ));
    }
    Ok(reason)
}

/// Alle Buchungen einer Rechnung in der Reihenfolge der Erfassung.
pub async fn list_in(
    conn: &mut SqliteConnection,
    invoice_id: &str,
) -> Result<Vec<InvoicePaymentRow>> {
    Ok(
        sqlx::query_as("SELECT * FROM invoice_payments WHERE invoice_id = ? ORDER BY seq")
            .bind(invoice_id)
            .fetch_all(conn)
            .await?,
    )
}

/// Wie [`list_in`], auf dem Pool.
pub async fn list(pool: &SqlitePool, invoice_id: &str) -> Result<Vec<InvoicePaymentRow>> {
    let mut conn = pool.acquire().await?;
    list_in(&mut conn, invoice_id).await
}

/// Eine Buchung.
pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<InvoicePaymentRow>> {
    Ok(
        sqlx::query_as("SELECT * FROM invoice_payments WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// Zahlungen (`payment`), die nicht durch eine Gegenbuchung storniert sind.
pub fn effective_payments(rows: &[InvoicePaymentRow]) -> impl Iterator<Item = &InvoicePaymentRow> {
    rows.iter().filter(move |p| {
        p.kind == KIND_PAYMENT
            && !rows
                .iter()
                .any(|r| r.reverses_payment_id.as_deref() == Some(p.id.as_str()))
    })
}

/// Jüngstes Zahldatum der nicht stornierten Zahlungen (`None` ohne solche).
pub fn last_effective_paid_date(rows: &[InvoicePaymentRow]) -> Option<&str> {
    effective_payments(rows).map(|p| p.paid_date.as_str()).max()
}

/// Setzt den Cache der Rechnung aus dem Buch (ADR 0042 Nr. 7):
/// `paid_amount_cents` = Summe, `paid_at` = jüngstes Zahldatum der nicht
/// stornierten Zahlungen, `status` aus der Summe (0 → `sent` bzw. `issued`,
/// darunter → `partially_paid`, voll → `paid`). Eine stornierte Rechnung
/// behält ihren Status. Liefert den neuen Status.
pub async fn refresh_cache_in(conn: &mut SqliteConnection, invoice_id: &str) -> Result<String> {
    let rows = list_in(&mut *conn, invoice_id).await?;
    let (gross, status, sent_at): (i64, String, Option<String>) =
        sqlx::query_as("SELECT gross_amount_cents, status, sent_at FROM invoices WHERE id = ?")
            .bind(invoice_id)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {invoice_id}")))?;
    let sum = rows
        .iter()
        .try_fold(0i64, |acc, r| acc.checked_add(r.amount_cents))
        .ok_or_else(|| Error::Domain(crate::domain::money::amount_out_of_range_message()))?;
    let paid_at = last_effective_paid_date(&rows).map(str::to_string);
    let new_status = if status == "canceled" || status == "draft" {
        status
    } else if sum == 0 {
        if sent_at.is_some() { "sent" } else { "issued" }.to_string()
    } else if sum >= gross {
        "paid".to_string()
    } else {
        "partially_paid".to_string()
    };
    sqlx::query(
        "UPDATE invoices SET
            paid_amount_cents = ?,
            paid_at = ?,
            status = ?,
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(sum)
    .bind(&paid_at)
    .bind(&new_status)
    .bind(invoice_id)
    .execute(&mut *conn)
    .await?;
    Ok(new_status)
}

/// Nächste laufende Nummer der Rechnung.
async fn next_seq(conn: &mut SqliteConnection, invoice_id: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM invoice_payments WHERE invoice_id = ?",
    )
    .bind(invoice_id)
    .fetch_one(conn)
    .await?)
}

/// Prüft, dass auf die Rechnung gebucht werden darf: festgeschrieben, nicht
/// verworfen, nicht storniert, kein Storno-Beleg, Ausgangsrechnung, kein
/// festgeschriebener Storno (die DB prüft dasselbe als zweite Linie).
fn ensure_bookable(invoice: &InvoiceRow, has_locked_storno: bool) -> Result<()> {
    if invoice.locked_at.is_none() || invoice.voided_at.is_some() {
        return Err(Error::Domain(
            "Zahlungen lassen sich nur auf festgeschriebene Rechnungen buchen.".into(),
        ));
    }
    if invoice.status == "canceled" || has_locked_storno {
        return Err(Error::Domain("Die Rechnung ist storniert.".into()));
    }
    if invoice.is_storno_for.is_some() {
        return Err(Error::Domain(
            "Auf einen Storno-Beleg wird keine Zahlung gebucht.".into(),
        ));
    }
    if invoice.direction != "issued" {
        return Err(Error::Domain(
            "Zahlungen werden nur auf Ausgangsrechnungen gebucht.".into(),
        ));
    }
    Ok(())
}

async fn load_invoice(conn: &mut SqliteConnection, invoice_id: &str) -> Result<(InvoiceRow, bool)> {
    let invoice: InvoiceRow = sqlx::query_as("SELECT * FROM invoices WHERE id = ?")
        .bind(invoice_id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {invoice_id}")))?;
    let has_locked_storno: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM invoices WHERE is_storno_for = ? AND locked_at IS NOT NULL)",
    )
    .bind(invoice_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok((invoice, has_locked_storno))
}

/// Eine zu buchende Zahlung.
#[derive(Debug, Clone)]
pub struct NewPayment<'a> {
    pub amount_cents: i64,
    /// Zahldatum `JJJJ-MM-TT`.
    pub paid_date: &'a str,
    pub note: Option<&'a str>,
    pub account_id: Option<&'a str>,
    /// Zeitpunkt der Erfassung (UTC, DB-Format).
    pub created_at: &'a str,
}

/// Bucht eine (Teil-)Zahlung in einer laufenden Transaktion: Buchzeile, Cache
/// der Rechnung und Audit `invoice.payment_recorded` (ADR 0042 Nr. 7).
/// Betrag > 0, Konto (falls angegeben) aktiv, keine Überzahlung.
pub async fn record_in(
    conn: &mut SqliteConnection,
    invoice_id: &str,
    p: &NewPayment<'_>,
) -> Result<InvoicePaymentRow> {
    if p.amount_cents <= 0 {
        return Err(Error::Domain(format!(
            "record_payment: amount_cents muss > 0 sein, got {}",
            p.amount_cents
        )));
    }
    // F5-fix: Domänengrenze statt Überlauf (overflow-checks im Release-Profil).
    if !crate::domain::money::amount_in_range(p.amount_cents) {
        return Err(Error::Domain(
            crate::domain::money::amount_out_of_range_message(),
        ));
    }
    let (invoice, has_locked_storno) = load_invoice(&mut *conn, invoice_id).await?;
    if let Some(acc) = p.account_id {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM payment_accounts WHERE id = ? AND active = 1")
                .bind(acc)
                .fetch_one(&mut *conn)
                .await?;
        if n == 0 {
            return Err(Error::Domain(
                "Das gewählte Zahlungskonto gibt es nicht oder es ist deaktiviert.".into(),
            ));
        }
    }
    ensure_bookable(&invoice, has_locked_storno)?;
    let new_paid = invoice.paid_amount_cents.saturating_add(p.amount_cents);
    if new_paid > invoice.gross_amount_cents {
        return Err(Error::Domain(format!(
            "record_payment: Summe {new_paid} > gross {} (Überzahlung)",
            invoice.gross_amount_cents
        )));
    }
    let id = Uuid::now_v7().to_string();
    let seq = next_seq(&mut *conn, invoice_id).await?;
    let row: InvoicePaymentRow = sqlx::query_as(
        "INSERT INTO invoice_payments
            (id, invoice_id, seq, kind, amount_cents, paid_date, account_id, note, created_at)
         VALUES (?, ?, ?, 'payment', ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(&id)
    .bind(invoice_id)
    .bind(seq)
    .bind(p.amount_cents)
    .bind(p.paid_date)
    .bind(p.account_id)
    .bind(p.note)
    .bind(p.created_at)
    .fetch_one(&mut *conn)
    .await?;
    let status = refresh_cache_in(&mut *conn, invoice_id).await?;
    crate::db::repo::audit_log::append_in(
        &mut *conn,
        "invoice.payment_recorded",
        "invoice",
        invoice_id,
        Some(serde_json::json!({
            "payment_id": row.id,
            "amount": row.amount_cents,
            "paid_date": row.paid_date,
            "status": status,
            "account": row.account_id,
        })),
    )
    .await?;
    Ok(row)
}

/// Warum eine Zahlung sich nicht stornieren lässt (`None`: möglich). Ohne die
/// Prüfung des Geschäftsjahres (das prüft der Aufrufer unter der
/// Buchungserlaubnis, die Detailansicht über [`views`]).
fn reversal_blocker(
    invoice: &InvoiceRow,
    has_locked_storno: bool,
    has_open_storno_draft: bool,
    rows: &[InvoicePaymentRow],
    payment: &InvoicePaymentRow,
) -> Option<String> {
    if payment.kind != KIND_PAYMENT {
        return Some("Eine Gegenbuchung lässt sich nicht stornieren.".into());
    }
    if rows
        .iter()
        .any(|r| r.reverses_payment_id.as_deref() == Some(payment.id.as_str()))
    {
        return Some("Die Zahlung ist bereits storniert.".into());
    }
    if invoice.is_storno_for.is_some() {
        return Some("Zahlungen auf einem Storno-Beleg lassen sich nicht stornieren.".into());
    }
    if invoice.status == "canceled" || has_locked_storno {
        return Some(
            "Die Rechnung ist storniert; die Erstattung im Storno beruht auf den gebuchten \
             Zahlungen."
                .into(),
        );
    }
    if has_open_storno_draft {
        return Some(
            "Zur Rechnung gibt es einen offenen Storno-Entwurf. Erst den Storno abschließen \
             oder den Entwurf verwerfen."
                .into(),
        );
    }
    if invoice.locked_at.is_none() || invoice.voided_at.is_some() {
        return Some("Die Rechnung ist nicht festgeschrieben.".into());
    }
    None
}

async fn has_open_storno_draft(conn: &mut SqliteConnection, invoice_id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM invoices
                         WHERE is_storno_for = ? AND locked_at IS NULL AND voided_at IS NULL)",
    )
    .bind(invoice_id)
    .fetch_one(conn)
    .await?)
}

/// Meldung, wenn das Geschäftsjahr des Zahldatums abgeschlossen ist (ADR 0042
/// Nr. 8): keine Korrektur im abgeschlossenen Jahr. Berichtigungspflicht nach
/// § 153 Abs. 1 Satz 1 Nr. 1 AO nur, wenn es zu einer Steuerverkürzung kommen
/// kann; zu hoch erfasste Einnahmen ändert das Finanzamt nur auf Antrag
/// (§ 164 Abs. 2, § 172 Abs. 1 Satz 1 Nr. 2 Buchst. a, § 173 Abs. 1 Nr. 2 AO).
pub fn closed_year_message(year: i64) -> String {
    format!(
        "Das Geschäftsjahr {year} ist abgeschlossen. Eine Zahlung mit Zahldatum in diesem Jahr \
         lässt sich nicht mehr stornieren. Ist die Erklärung schon abgegeben und führt die \
         falsche Zahlung zu zu niedriger Steuer, ist die Erklärung zu berichtigen (§ 153 AO); \
         zu hoch erfasste Einnahmen lassen sich auf Antrag ändern (§ 173 Abs. 1 Nr. 2, § 164 \
         Abs. 2 AO) — bitte mit der Steuerberatung klären."
    )
}

/// Ergebnis einer Gegenbuchung.
#[derive(Debug, Clone)]
pub struct Reversed {
    pub original: InvoicePaymentRow,
    pub reversal: InvoicePaymentRow,
    pub status: String,
}

/// Storniert eine Zahlung in einer laufenden Transaktion (ADR 0042 Nr. 8):
/// Gegenbuchung mit Betrag, Datum und Konto der Zahlung und umgekehrtem
/// Vorzeichen, Cache der Rechnung und Audit `invoice.payment_reversed`. Nicht
/// bei Storno-Belegen, stornierten Rechnungen, offenem Storno-Entwurf oder
/// bereits stornierter Zahlung. `reason` muss über [`clean_reason`] gelaufen
/// sein. Das Geschäftsjahr prüft der Aufrufer (Buchungserlaubnis); die DB
/// lehnt ein abgeschlossenes Jahr zusätzlich ab.
pub async fn reverse_in(
    conn: &mut SqliteConnection,
    invoice_id: &str,
    payment_id: &str,
    reason: &str,
    created_at: &str,
) -> Result<Reversed> {
    let reason = clean_reason(reason)?;
    let (invoice, has_locked_storno) = load_invoice(&mut *conn, invoice_id).await?;
    let rows = list_in(&mut *conn, invoice_id).await?;
    let original = rows
        .iter()
        .find(|r| r.id == payment_id)
        .cloned()
        .ok_or_else(|| Error::Domain("Die Zahlung gibt es zu dieser Rechnung nicht.".into()))?;
    let open_draft = has_open_storno_draft(&mut *conn, invoice_id).await?;
    if let Some(why) = reversal_blocker(&invoice, has_locked_storno, open_draft, &rows, &original) {
        return Err(Error::Domain(why));
    }
    let id = Uuid::now_v7().to_string();
    let seq = next_seq(&mut *conn, invoice_id).await?;
    let reversal: InvoicePaymentRow = sqlx::query_as(
        "INSERT INTO invoice_payments
            (id, invoice_id, seq, kind, amount_cents, paid_date, account_id,
             reverses_payment_id, reason, created_at)
         VALUES (?, ?, ?, 'reversal', ?, ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(&id)
    .bind(invoice_id)
    .bind(seq)
    .bind(-original.amount_cents)
    .bind(&original.paid_date)
    .bind(&original.account_id)
    .bind(&original.id)
    .bind(&reason)
    .bind(created_at)
    .fetch_one(&mut *conn)
    .await?;
    let status = refresh_cache_in(&mut *conn, invoice_id).await?;
    crate::db::repo::audit_log::append_in(
        &mut *conn,
        "invoice.payment_reversed",
        "invoice",
        invoice_id,
        Some(serde_json::json!({
            "number": invoice.invoice_number,
            "payment_id": original.id,
            "reversal_id": reversal.id,
            "amount": original.amount_cents,
            "paid_date": original.paid_date,
            "account": original.account_id,
            "reason": reason,
            "status": status,
        })),
    )
    .await?;
    Ok(Reversed {
        original,
        reversal,
        status,
    })
}

/// Lesemodell des Zahlungsbuchs einer Rechnung für die Detailansicht: jede
/// Buchung mit Verweis auf ihre Gegenbuchung und der Angabe, ob „Zahlung
/// stornieren“ möglich ist — einschließlich des Geschäftsjahres des
/// Zahldatums (abgeschlossen → nicht möglich, Hinweis auf § 153 AO).
pub async fn views(pool: &SqlitePool, invoice: &InvoiceRow) -> Result<Vec<InvoicePaymentView>> {
    let mut conn = pool.acquire().await?;
    let rows = list_in(&mut conn, &invoice.id).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let has_locked_storno: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM invoices WHERE is_storno_for = ? AND locked_at IS NOT NULL)",
    )
    .bind(&invoice.id)
    .fetch_one(&mut *conn)
    .await?;
    let open_draft = has_open_storno_draft(&mut conn, &invoice.id).await?;
    let closed: std::collections::HashSet<i64> =
        sqlx::query_scalar::<_, i64>("SELECT fiscal_year FROM fiscal_year_locks")
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    Ok(rows
        .iter()
        .map(|r| {
            let reversed_by_id = rows
                .iter()
                .find(|x| x.reverses_payment_id.as_deref() == Some(r.id.as_str()))
                .map(|x| x.id.clone());
            let not_reversible_reason = if r.kind == KIND_PAYMENT && reversed_by_id.is_none() {
                reversal_blocker(invoice, has_locked_storno, open_draft, &rows, r).or_else(|| {
                    r.paid_date
                        .get(..4)
                        .and_then(|y| y.parse::<i64>().ok())
                        .filter(|y| closed.contains(y))
                        .map(closed_year_message)
                })
            } else {
                None
            };
            InvoicePaymentView {
                row: r.clone(),
                reversible: r.kind == KIND_PAYMENT
                    && reversed_by_id.is_none()
                    && not_reversible_reason.is_none(),
                reversed_by_id,
                not_reversible_reason,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        id: &str,
        kind: &str,
        amount: i64,
        date: &str,
        reverses: Option<&str>,
    ) -> InvoicePaymentRow {
        InvoicePaymentRow {
            id: id.into(),
            invoice_id: "i".into(),
            seq: 1,
            kind: kind.into(),
            amount_cents: amount,
            paid_date: date.into(),
            account_id: None,
            note: None,
            reverses_payment_id: reverses.map(str::to_string),
            reason: reverses.map(|_| "Fehlbuchung erfasst".to_string()),
            created_at: "2026-01-01 00:00:00".into(),
        }
    }

    #[test]
    fn last_effective_date_ignores_reversed_pairs() {
        let rows = vec![
            row("a", KIND_PAYMENT, 100, "2026-03-01", None),
            row("b", KIND_PAYMENT, 200, "2026-05-01", None),
            row("c", KIND_REVERSAL, -200, "2026-05-01", Some("b")),
        ];
        assert_eq!(last_effective_paid_date(&rows), Some("2026-03-01"));
        let ids: Vec<&str> = effective_payments(&rows).map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["a"]);
        let all_reversed = vec![
            row("a", KIND_PAYMENT, 100, "2026-03-01", None),
            row("c", KIND_REVERSAL, -100, "2026-03-01", Some("a")),
        ];
        assert_eq!(last_effective_paid_date(&all_reversed), None);
    }

    #[test]
    fn reason_needs_ten_characters() {
        assert!(clean_reason("  kurz  ").is_err());
        assert!(clean_reason("\u{7}\u{7}\u{7}\u{7}\u{7}\u{7}\u{7}\u{7}\u{7}\u{7}abc").is_err());
        assert_eq!(
            clean_reason("  Betrag falsch erfasst ").unwrap(),
            "Betrag falsch erfasst"
        );
        assert_eq!(clean_reason(&"x".repeat(600)).unwrap().chars().count(), 500);
    }
}
