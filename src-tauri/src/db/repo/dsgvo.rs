//! Read-only Sammel-Queries für die **DSGVO-Auskunft nach Art. 15** (Block 18).
//!
//! Schicht: **Imperative Shell** — nur SELECTs, keine Mutation, keine Migration.
//! Trägt zu **genau einem Kontakt** alle personenbezogenen Datenbestände
//! zusammen: Rechnungen (beide Richtungen) + Positionen, Angebote + Positionen,
//! Kosten, Kosten-Abos und Anlagen (Kontakt als Lieferant), Abo-Rechnungs-
//! Vorlagen samt Positionen (Lauf F2c), archivierte Dokumente (PDF/XML/Belege/
//! Anhänge — Metadaten), Versandprotokoll-Bezüge und Audit-Log-Bezüge.
//!
//! Bewusst **nicht** enthalten: `private_movements` — die Tabelle hat keinen
//! `contact_id`, also keinen Personenbezug zu einem Kontakt.
//!
//! Die reine Aufbereitung in den Report passiert im Functional Core
//! [`crate::domain::dsgvo`].

use crate::db::models::{
    AssetRow, ContactRow, EmailLogRow, ExpenseRow, InvoiceItemRow, InvoiceRow, QuoteItemRow,
    QuoteRow, RecurringInvoiceItemRow, RecurringInvoiceRow, RecurringSubscriptionRow,
};
use crate::domain::dsgvo::{RawAudit, RawDocument};
use crate::error::Result;
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Alles, was zu einem Kontakt gesammelt wurde. Owned — der Command baut daraus
/// die Borrow-Sicht [`crate::domain::dsgvo::RawData`].
pub struct Gathered {
    pub contact: ContactRow,
    pub invoices: Vec<(InvoiceRow, Vec<InvoiceItemRow>)>,
    pub quotes: Vec<(QuoteRow, Vec<QuoteItemRow>)>,
    pub expenses: Vec<ExpenseRow>,
    /// Abo-Rechnungs-Vorlagen an den Kontakt samt Positionen (Lauf F2c).
    pub recurring_invoices: Vec<(RecurringInvoiceRow, Vec<RecurringInvoiceItemRow>)>,
    /// Kosten-Abos mit dem Kontakt als Lieferant (Lauf F2c).
    pub subscriptions: Vec<RecurringSubscriptionRow>,
    /// Anlagen mit dem Kontakt als Lieferant (Lauf F2c).
    pub assets: Vec<AssetRow>,
    pub documents: Vec<RawDocument>,
    pub emails: Vec<EmailLogRow>,
    pub audit: Vec<RawAudit>,
}

/// JSON-Array der Werte als **ein** Bind-Parameter (`json_each(?)`): keine
/// Obergrenze der SQLite-Variablen, keine Abfrage je Element (Lauf F,
/// R1-S2-NPLUS1).
fn json_list<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    serde_json::Value::from(values.into_iter().collect::<Vec<_>>()).to_string()
}

/// Normalform einer Adresse für den Abgleich: getrimmt, ASCII-Kleinbuchstaben
/// — dieselbe Faltung wie SQLites `lower()` (die Domain ist ohnehin
/// unabhängig von der Schreibweise; Abweichungen im lokalen Teil gelten bei
/// gängigen Anbietern als dieselbe Adresse).
fn normalize_email(e: &str) -> String {
    e.trim().to_ascii_lowercase()
}

/// Sammelt alle kontaktbezogenen Daten. `Ok(None)`, wenn der Kontakt nicht
/// existiert. Eine feste Zahl von Abfragen, unabhängig von der Zahl der Belege
/// (Lauf F, R1-S2-NPLUS1).
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
    let invoice_items: Vec<InvoiceItemRow> = sqlx::query_as(
        "SELECT ii.* FROM invoice_items ii
           JOIN invoices i ON i.id = ii.invoice_id
          WHERE i.contact_id = ?
          ORDER BY ii.invoice_id, ii.position ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let mut items_by_invoice: HashMap<String, Vec<InvoiceItemRow>> = HashMap::new();
    for it in invoice_items {
        items_by_invoice
            .entry(it.invoice_id.clone())
            .or_default()
            .push(it);
    }
    let invoices: Vec<(InvoiceRow, Vec<InvoiceItemRow>)> = invoice_rows
        .into_iter()
        .map(|inv| {
            let items = items_by_invoice.remove(&inv.id).unwrap_or_default();
            (inv, items)
        })
        .collect();

    // ---- Angebote + Positionen ----
    let quote_rows: Vec<QuoteRow> = sqlx::query_as(
        "SELECT * FROM quotes WHERE contact_id = ? ORDER BY quote_date ASC, quote_number ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let quote_items: Vec<QuoteItemRow> = sqlx::query_as(
        "SELECT qi.* FROM quote_items qi
           JOIN quotes q ON q.id = qi.quote_id
          WHERE q.contact_id = ?
          ORDER BY qi.quote_id, qi.position ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let mut items_by_quote: HashMap<String, Vec<QuoteItemRow>> = HashMap::new();
    for it in quote_items {
        items_by_quote
            .entry(it.quote_id.clone())
            .or_default()
            .push(it);
    }
    let quotes: Vec<(QuoteRow, Vec<QuoteItemRow>)> = quote_rows
        .into_iter()
        .map(|q| {
            let items = items_by_quote.remove(&q.id).unwrap_or_default();
            (q, items)
        })
        .collect();

    // ---- Kosten, bei denen der Kontakt Lieferant ist ----
    let expenses: Vec<ExpenseRow> = sqlx::query_as(
        "SELECT * FROM expenses WHERE vendor_contact_id = ? ORDER BY expense_date ASC, expense_number ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;

    // ---- Abo-Rechnungs-Vorlagen an den Kontakt + Positionen (Lauf F2c) ----
    // Eine Vorlage hält Leistungen, Beträge, Rhythmus und Laufzeit einer
    // Vertragsbeziehung mit der Person — eigene Verarbeitung (Art. 15 Abs. 1).
    let template_rows: Vec<RecurringInvoiceRow> = sqlx::query_as(
        "SELECT * FROM recurring_invoices WHERE contact_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let template_items: Vec<RecurringInvoiceItemRow> = sqlx::query_as(
        "SELECT ri.* FROM recurring_invoice_items ri
           JOIN recurring_invoices r ON r.id = ri.recurring_invoice_id
          WHERE r.contact_id = ?
          ORDER BY ri.recurring_invoice_id, ri.position ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let mut items_by_template: HashMap<String, Vec<RecurringInvoiceItemRow>> = HashMap::new();
    for it in template_items {
        items_by_template
            .entry(it.recurring_invoice_id.clone())
            .or_default()
            .push(it);
    }
    let recurring_invoices: Vec<(RecurringInvoiceRow, Vec<RecurringInvoiceItemRow>)> =
        template_rows
            .into_iter()
            .map(|t| {
                let items = items_by_template.remove(&t.id).unwrap_or_default();
                (t, items)
            })
            .collect();

    // ---- Kosten-Abos und Anlagen mit dem Kontakt als Lieferant (Lauf F2c) ----
    let subscriptions: Vec<RecurringSubscriptionRow> = sqlx::query_as(
        "SELECT * FROM recurring_subscriptions WHERE vendor_contact_id = ?
          ORDER BY created_at ASC, id ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    let assets: Vec<AssetRow> = sqlx::query_as(
        "SELECT * FROM assets WHERE vendor_contact_id = ? ORDER BY asset_number ASC",
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

    // Anhänge (direkt am Kontakt + an seinen Belegen) in einer Abfrage; die
    // Beschriftung kommt aus dem jeweiligen Eltern-Beleg.
    let mut parent_labels: HashMap<(&str, &str), &str> = HashMap::new();
    parent_labels.insert(("contact", contact.id.as_str()), contact.name.as_str());
    for (inv, _) in &invoices {
        parent_labels.insert(("invoice", inv.id.as_str()), inv.invoice_number.as_str());
    }
    for (q, _) in &quotes {
        parent_labels.insert(("quote", q.id.as_str()), q.quote_number.as_str());
    }
    for e in &expenses {
        parent_labels.insert(("expense", e.id.as_str()), e.expense_number.as_str());
    }
    for s in &subscriptions {
        parent_labels.insert(("recurring", s.id.as_str()), s.label.as_str());
    }
    for a in &assets {
        parent_labels.insert(("asset", a.id.as_str()), a.asset_number.as_str());
    }
    let attachments: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT parent_type, parent_id, archive_entry_id, label FROM attachments
          WHERE (parent_type = 'contact' AND parent_id = ?1)
             OR (parent_type = 'invoice'
                 AND parent_id IN (SELECT id FROM invoices WHERE contact_id = ?1))
             OR (parent_type = 'quote'
                 AND parent_id IN (SELECT id FROM quotes WHERE contact_id = ?1))
             OR (parent_type = 'expense'
                 AND parent_id IN (SELECT id FROM expenses WHERE vendor_contact_id = ?1))
             OR (parent_type = 'recurring'
                 AND parent_id IN (SELECT id FROM recurring_subscriptions
                                    WHERE vendor_contact_id = ?1))
             OR (parent_type = 'asset'
                 AND parent_id IN (SELECT id FROM assets WHERE vendor_contact_id = ?1))
          ORDER BY CASE parent_type WHEN 'contact' THEN 0 WHEN 'invoice' THEN 1
                                    WHEN 'quote' THEN 2 WHEN 'expense' THEN 3
                                    WHEN 'recurring' THEN 4 ELSE 5 END,
                   parent_id, sort_order ASC, created_at ASC",
    )
    .bind(contact_id)
    .fetch_all(pool)
    .await?;
    for (ptype, pid, archive_id, label) in attachments {
        let kind = match label.as_deref().filter(|s| !s.is_empty()) {
            Some(l) => format!("Anhang ({l})"),
            None => "Anhang".to_string(),
        };
        let parent = parent_labels
            .get(&(ptype.as_str(), pid.as_str()))
            .map(|l| l.to_string());
        doc_refs.push((kind, parent, archive_id));
    }

    // Metadaten aller Archive-Einträge in einer Abfrage, nach archive_id
    // dedupliziert (Reihenfolge wie die Referenzen).
    let mut seen_archive: HashSet<&str> = HashSet::new();
    let unique_refs: Vec<&(String, Option<String>, String)> = doc_refs
        .iter()
        .filter(|(_, _, id)| seen_archive.insert(id.as_str()))
        .collect();
    let meta_rows = sqlx::query(
        "SELECT id, file_name, mime_type, file_size_bytes, file_hash_sha256, received_at
           FROM archive_entries WHERE id IN (SELECT value FROM json_each(?))",
    )
    .bind(json_list(unique_refs.iter().map(|(_, _, id)| id.as_str())))
    .fetch_all(pool)
    .await?;
    let mut meta: HashMap<String, sqlx::sqlite::SqliteRow> = HashMap::new();
    for row in meta_rows {
        meta.insert(row.try_get("id")?, row);
    }
    let mut documents: Vec<RawDocument> = Vec::new();
    for (kind, label, archive_id) in unique_refs {
        if let Some(row) = meta.get(archive_id) {
            documents.push(RawDocument {
                kind: kind.clone(),
                related_label: label.clone(),
                archive_id: archive_id.clone(),
                file_name: row.try_get("file_name")?,
                mime_type: row.try_get("mime_type")?,
                size_bytes: row.try_get("file_size_bytes")?,
                sha256: row.try_get("file_hash_sha256")?,
                created_at: row.try_get("received_at")?,
            });
        }
    }

    // ---- Versandprotokoll (email_log) ----
    // Bezug über Beleg-ID (Rechnung/Angebot), über den Kontakt selbst
    // (Katalogversand) oder über die Empfängeradresse — die aktuelle des
    // Kontakts und jede frühere aus den Empfänger-Snapshots seiner Belege,
    // ohne Rücksicht auf Groß-/Kleinschreibung (Lauf F, KB-0049).
    let mut related_ids: Vec<&str> = vec![contact.id.as_str()];
    related_ids.extend(invoices.iter().map(|(inv, _)| inv.id.as_str()));
    related_ids.extend(quotes.iter().map(|(q, _)| q.id.as_str()));
    let mut addresses: BTreeSet<String> = BTreeSet::new();
    let snapshot_emails = invoices
        .iter()
        .filter_map(|(inv, _)| inv.buyer_email.as_deref())
        .chain(quotes.iter().filter_map(|(q, _)| q.buyer_email.as_deref()));
    for e in contact.email.as_deref().into_iter().chain(snapshot_emails) {
        let n = normalize_email(e);
        if !n.is_empty() {
            addresses.insert(n);
        }
    }
    let emails = gather_emails(pool, &related_ids, &addresses).await?;

    // ---- Audit-Log-Bezüge: Kontakt + seine Belege, je mit Entitätstyp ----
    let audit = gather_audit(
        pool,
        &AuditScope {
            contact_id: &contact.id,
            invoices: invoices.iter().map(|(i, _)| i.id.as_str()).collect(),
            quotes: quotes.iter().map(|(q, _)| q.id.as_str()).collect(),
            expenses: expenses.iter().map(|e| e.id.as_str()).collect(),
            templates: recurring_invoices
                .iter()
                .map(|(t, _)| t.id.as_str())
                .collect(),
            subscriptions: subscriptions.iter().map(|s| s.id.as_str()).collect(),
            assets: assets.iter().map(|a| a.id.as_str()).collect(),
        },
    )
    .await?;

    Ok(Some(Gathered {
        contact,
        invoices,
        quotes,
        expenses,
        recurring_invoices,
        subscriptions,
        assets,
        documents,
        emails,
        audit,
    }))
}

async fn gather_emails(
    pool: &SqlitePool,
    related_ids: &[&str],
    addresses: &BTreeSet<String>,
) -> Result<Vec<EmailLogRow>> {
    let rows: Vec<EmailLogRow> = sqlx::query_as(
        "SELECT * FROM email_log
          WHERE related_id IN (SELECT value FROM json_each(?1))
             OR lower(trim(to_email)) IN (SELECT value FROM json_each(?2))
          ORDER BY created_at ASC, id ASC",
    )
    .bind(json_list(related_ids.iter().copied()))
    .bind(json_list(addresses.iter().map(String::as_str)))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// IDs je Entitätstyp, auf die sich Audit-Einträge der Auskunft beziehen.
struct AuditScope<'a> {
    contact_id: &'a str,
    invoices: Vec<&'a str>,
    quotes: Vec<&'a str>,
    expenses: Vec<&'a str>,
    /// Abo-Rechnungs-Vorlagen (`entity_type = 'recurring_invoice'`).
    templates: Vec<&'a str>,
    /// Kosten-Abos (`entity_type = 'recurring'`).
    subscriptions: Vec<&'a str>,
    assets: Vec<&'a str>,
}

/// Audit-Einträge zum Kontakt und zu seinen Belegen, Vorlagen, Kosten-Abos und
/// Anlagen — gefiltert über Entitätstyp **und** ID (Lauf F, R2-S2-DSGVO; F2c
/// um Vorlagen, Kosten-Abos und Anlagen ergänzt): ein Eintrag zählt nur, wenn
/// er sich ausdrücklich auf eines dieser Objekte bezieht.
async fn gather_audit(pool: &SqlitePool, scope: &AuditScope<'_>) -> Result<Vec<RawAudit>> {
    let rows = sqlx::query(
        "SELECT timestamp_utc, action, entity_type, entity_id
           FROM audit_log
          WHERE (entity_type = 'contact' AND entity_id = ?1)
             OR (entity_type = 'invoice' AND entity_id IN (SELECT value FROM json_each(?2)))
             OR (entity_type = 'quote' AND entity_id IN (SELECT value FROM json_each(?3)))
             OR (entity_type = 'expense' AND entity_id IN (SELECT value FROM json_each(?4)))
             OR (entity_type = 'recurring_invoice'
                 AND entity_id IN (SELECT value FROM json_each(?5)))
             OR (entity_type = 'recurring' AND entity_id IN (SELECT value FROM json_each(?6)))
             OR (entity_type = 'asset' AND entity_id IN (SELECT value FROM json_each(?7)))
          ORDER BY id ASC",
    )
    .bind(scope.contact_id)
    .bind(json_list(scope.invoices.iter().copied()))
    .bind(json_list(scope.quotes.iter().copied()))
    .bind(json_list(scope.expenses.iter().copied()))
    .bind(json_list(scope.templates.iter().copied()))
    .bind(json_list(scope.subscriptions.iter().copied()))
    .bind(json_list(scope.assets.iter().copied()))
    .fetch_all(pool)
    .await?;
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
