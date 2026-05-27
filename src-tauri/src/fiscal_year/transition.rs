//! GJ-Übergang / Carry-over (Block 15).
//!
//! Doc-Number-Counter werden lazy beim ersten Beleg eines GJ initialisiert
//! ([`crate::db::numbering`]), daher braucht der Übergang keinen expliziten
//! Reset. Was bleibt, ist die Anzeige offener Forderungen, die ins neue
//! Geschäftsjahr „mitgenommen" werden — der GJ-Abschluss-Dialog zeigt sie an,
//! damit nichts übersehen wird.

use crate::error::Result;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OpenReceivable {
    pub id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: Option<String>,
    pub fiscal_year: i64,
    pub outstanding_cents: i64,
}

/// Offene Forderungen über alle Geschäftsjahre: ausgestellte, nicht (voll)
/// bezahlte Rechnungen ohne Storno. Sortiert nach Rechnungsdatum aufsteigend.
pub async fn open_receivables(pool: &SqlitePool) -> Result<Vec<OpenReceivable>> {
    let rows = sqlx::query_as::<_, OpenReceivable>(
        "SELECT id, invoice_number, invoice_date, due_date, fiscal_year,
                (gross_amount_cents - paid_amount_cents) AS outstanding_cents
           FROM invoices
          WHERE direction = 'issued'
            AND is_storno_for IS NULL
            AND canceled_at IS NULL
            AND status IN ('issued','sent','partially_paid')
            AND gross_amount_cents > paid_amount_cents
          ORDER BY invoice_date ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
