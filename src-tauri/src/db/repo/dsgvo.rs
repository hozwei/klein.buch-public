//! Read-only Sammel-Queries für die **DSGVO-Auskunft nach Art. 15** (Block 18).
//!
//! Schicht: **Imperative Shell** — nur SELECTs, keine Mutation, keine Migration.
//! Trägt zu **genau einem Kontakt** alle personenbezogenen Datenbestände
//! zusammen: Rechnungen (beide Richtungen) + Positionen, Angebote + Positionen,
//! Kosten (Kontakt als Lieferant), archivierte Dokumente (PDF/XML/Belege/
//! Anhänge — Metadaten), Versandprotokoll-Bezüge und Audit-Log-Bezüge.
//!
//! Bewusst **nicht** enthalten: `private_movements` — die Tabelle hat keinen
//! `contact_id`, also keinen Personenbezug zu einem Kontakt.
//!
//! Die reine Aufbereitung in den Report passiert im Functional Core
//! [`crate::domain::dsgvo`].

use crate::db::models::{
    ContactRow, EmailLogRow, ExpenseRow, InvoiceItemRow, InvoiceRow, QuoteItemRow, QuoteRow,
};
use crate::domain::dsgvo::{RawAudit, RawDocument};
use crate::error::Result;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Alles, was zu einem Kontakt gesammelt wurde. Owned — der Command baut daraus
/// die Borrow-Sicht [`crate::domain::dsgvo::RawData`].
pub struct Gathered {
    pub contact: ContactRow,
    pub invoices: Vec<(InvoiceRow, Vec<InvoiceItemRow>)>,
    pub quotes: Vec<(QuoteRow, Vec<QuoteItemRow>)>,
    pub expenses: Vec<ExpenseRow>,
    pub documents: Vec<RawDocument>,
    pub emails: Vec<EmailLogRow>,
    pub audit: Vec<RawAudit>,
}

/// Sammelt alle kontaktbezogenen Daten. `Ok(None)`, wenn der Kontakt nicht
/// existiert.
pub async fn gather(pool: &SqlitePool, contact_id: &str) -> Result<Option<Gathered>> {
    let contact: Option<ContactRow> = sqlx::query_as("SELECT * FROM contacts WHERE id = ?")
        .bind(contact_id)
        .fetch_optional(pool)
        .await?;
    let Some(contact) = contact else {
        return Ok(None);
    };

    // ---- Rechnungen (beide Richtungen) + Positionen ----
    let invoice_rows: Vec<InvoiceRow> = sqlx::query_as(
        "SELECT * FROM invoices WHERE contact_id = ? ORDER BY invoice_date ASC, invoice_number ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let mut invoices = Vec::with_capacity(invoice_rows.len());
    for inv in invoice_rows {
        let items: Vec<InvoiceItemRow> = sqlx::query_as(
            "SELECT * FROM invoice_items WHERE invoice_id = ? ORDER BY position ASC",
        )
        .bind(&inv.id)
        .fetch_all(pool)
        .await?;
        invoices.push((inv, items));
    }

    // ---- Angebote + Positionen ----
    let quote_rows: Vec<QuoteRow> = sqlx::query_as(
        "SELECT * FROM quotes WHERE contact_id = ? ORDER BY quote_date ASC, quote_number ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let mut quotes = Vec::with_capacity(quote_rows.len());
    for q in quote_rows {
        let items: Vec<QuoteItemRow> =
            sqlx::query_as("SELECT * FROM quote_items WHERE quote_id = ? ORDER BY position ASC")
                .bind(&q.id)
                .fetch_all(pool)
                .await?;
        quotes.push((q, items));
    }

    // ---- Kosten, bei denen der Kontakt Lieferant ist ----
    let expenses: Vec<ExpenseRow> = sqlx::query_as(
        "SELECT * FROM expenses WHERE vendor_contact_id = ? ORDER BY expense_date ASC, expense_number ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;

    // ---- Dokument-Referenzen einsammeln (kind, label, archive_id) ----
    let mut doc_refs: Vec<(String, Option<String>, String)> = Vec::new();
    for (inv, _) in &invoices {
        if let Some(a) = &inv.pdf_archive_id {
            doc_refs.push((
                "Rechnung (PDF)".into(),
                Some(inv.invoice_number.clone()),
                a.clone(),
            ));
        }
        if let Some(a) = &inv.xml_archive_id {
            doc_refs.push((
                "Rechnung (XML/E-Rechnung)".into(),
                Some(inv.invoice_number.clone()),
                a.clone(),
            ));
        }
    }
    for (q, _) in &quotes {
        if let Some(a) = &q.pdf_archive_id {
            doc_refs.push((
                "Angebot (PDF)".into(),
                Some(q.quote_number.clone()),
                a.clone(),
            ));
        }
    }
    for e in &expenses {
        if let Some(a) = &e.receipt_archive_id {
            doc_refs.push((
                "Beleg / Eingangsrechnung".into(),
                Some(e.expense_number.clone()),
                a.clone(),
            ));
        }
    }
    // Anhänge (direkt am Kontakt + an seinen Belegen).
    let mut attach_parents: Vec<(&str, String, String)> = Vec::new(); // (parent_type, parent_id, label)
    attach_parents.push(("contact", contact.id.clone(), contact.name.clone()));
    for (inv, _) in &invoices {
        attach_parents.push(("invoice", inv.id.clone(), inv.invoice_number.clone()));
    }
    for (q, _) in &quotes {
        attach_parents.push(("quote", q.id.clone(), q.quote_number.clone()));
    }
    for e in &expenses {
        attach_parents.push(("expense", e.id.clone(), e.expense_number.clone()));
    }
    for (ptype, pid, label) in &attach_parents {
        let atts = crate::db::repo::attachments::list_for_parent(pool, ptype, pid.as_str()).await?;
        for a in atts {
            let kind = match a.label.as_deref().filter(|s| !s.is_empty()) {
                Some(l) => format!("Anhang ({l})"),
                None => "Anhang".to_string(),
            };
            doc_refs.push((kind, Some(label.clone()), a.archive_entry_id));
        }
    }

    // Metadaten je Archive-Eintrag nachladen, nach archive_id deduplizieren.
    let mut documents: Vec<RawDocument> = Vec::new();
    let mut seen_archive: HashSet<String> = HashSet::new();
    for (kind, label, archive_id) in doc_refs {
        if !seen_archive.insert(archive_id.clone()) {
            continue;
        }
        let meta = sqlx::query(
            "SELECT file_name, mime_type, file_size_bytes, file_hash_sha256, received_at
             FROM archive_entries WHERE id = ?",
        )
        .bind(&archive_id)
        .fetch_optional(pool)
        .await?;
        if let Some(row) = meta {
            documents.push(RawDocument {
                kind,
                related_label: label,
                archive_id,
                file_name: row.try_get("file_name")?,
                mime_type: row.try_get("mime_type")?,
                size_bytes: row.try_get("file_size_bytes")?,
                sha256: row.try_get("file_hash_sha256")?,
                created_at: row.try_get("received_at")?,
            });
        }
    }

    // ---- Beleg-IDs für E-Mail-/Audit-Filter ----
    let mut doc_ids: Vec<String> = Vec::new();
    for (inv, _) in &invoices {
        doc_ids.push(inv.id.clone());
    }
    for (q, _) in &quotes {
        doc_ids.push(q.id.clone());
    }

    // ---- Versandprotokoll (email_log): Bezug über Beleg-ID oder Empfänger ----
    let emails = gather_emails(pool, &doc_ids, contact.email.as_deref()).await?;

    // ---- Audit-Log-Bezüge: Kontakt + seine Belege ----
    let mut audit_ids: Vec<String> = doc_ids.clone();
    for e in &expenses {
        audit_ids.push(e.id.clone());
    }
    audit_ids.push(contact.id.clone());
    let audit = gather_audit(pool, &audit_ids).await?;

    Ok(Some(Gathered {
        contact,
        invoices,
        quotes,
        expenses,
        documents,
        emails,
        audit,
    }))
}

fn placeholders(n: usize) -> String {
    std::iter::repeat_n("?", n).collect::<Vec<_>>().join(",")
}

async fn gather_emails(
    pool: &SqlitePool,
    doc_ids: &[String],
    email: Option<&str>,
) -> Result<Vec<EmailLogRow>> {
    if doc_ids.is_empty() && email.is_none() {
        return Ok(vec![]);
    }
    let mut sql = String::from("SELECT * FROM email_log WHERE 0 = 1");
    if !doc_ids.is_empty() {
        sql.push_str(&format!(
            " OR related_id IN ({})",
            placeholders(doc_ids.len())
        ));
    }
    if email.is_some() {
        sql.push_str(" OR to_email = ?");
    }
    sql.push_str(" ORDER BY created_at ASC");

    let mut q = sqlx::query_as::<_, EmailLogRow>(&sql);
    for id in doc_ids {
        q = q.bind(id);
    }
    if let Some(e) = email {
        q = q.bind(e);
    }
    let mut rows = q.fetch_all(pool).await?;

    // Dedup nach id (ein per Beleg-ID gefundener Eintrag kann via to_email
    // erneut matchen).
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(r.id.clone()));
    Ok(rows)
}

async fn gather_audit(pool: &SqlitePool, entity_ids: &[String]) -> Result<Vec<RawAudit>> {
    if entity_ids.is_empty() {
        return Ok(vec![]);
    }
    let sql = format!(
        "SELECT timestamp_utc, action, entity_type, entity_id
         FROM audit_log WHERE entity_id IN ({}) ORDER BY id ASC",
        placeholders(entity_ids.len())
    );
    let mut q = sqlx::query(&sql);
    for id in entity_ids {
        q = q.bind(id);
    }
    let rows = q.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| RawAudit {
            timestamp_utc: r.get("timestamp_utc"),
            action: r.get("action"),
            entity_type: r.get("entity_type"),
            entity_id: r.get("entity_id"),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
