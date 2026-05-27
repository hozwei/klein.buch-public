//! Repository für `contacts`.
//!
//! - UUIDv7 als PK (zeitsortiert → natürliche Reihenfolge in List-Queries).
//! - **Kein DELETE.** Statt `delete` wird `archived = 1` gesetzt.
//! - Suche ist case-insensitive über `name`, `email`, `vat_id`.

use crate::db::models::ContactRow;
use crate::domain::anonymize as anonymize_domain;
use crate::domain::contact::{validate, ContactInput};
use crate::error::{Error, Result};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Liefert Kontakte sortiert nach Name. `include_archived = false` filtert
/// archivierte Einträge weg (Default für UI-Listen).
pub async fn list(pool: &SqlitePool, include_archived: bool) -> Result<Vec<ContactRow>> {
    let sql = if include_archived {
        "SELECT id, contact_type, name, legal_form, vat_id, tax_number,
                street, postal_code, city, country_code,
                email, phone, iban, bic,
                accepts_einvoice, archived, notes, created_at, updated_at,
                anonymized_at
         FROM contacts
         ORDER BY name COLLATE NOCASE ASC"
    } else {
        "SELECT id, contact_type, name, legal_form, vat_id, tax_number,
                street, postal_code, city, country_code,
                email, phone, iban, bic,
                accepts_einvoice, archived, notes, created_at, updated_at,
                anonymized_at
         FROM contacts
         WHERE archived = 0
         ORDER BY name COLLATE NOCASE ASC"
    };
    let rows: Vec<ContactRow> = sqlx::query_as(sql).fetch_all(pool).await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<ContactRow>> {
    let row: Option<ContactRow> = sqlx::query_as(
        "SELECT id, contact_type, name, legal_form, vat_id, tax_number,
                street, postal_code, city, country_code,
                email, phone, iban, bic,
                accepts_einvoice, archived, notes, created_at, updated_at,
                anonymized_at
         FROM contacts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Validiert + persistiert. Gibt den frisch angelegten Row zurück.
pub async fn create(pool: &SqlitePool, input: &ContactInput) -> Result<ContactRow> {
    validate(input).map_err(|errs| Error::Domain(format_errs(&errs)))?;
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO contacts (
            id, contact_type, name, legal_form, vat_id, tax_number,
            street, postal_code, city, country_code,
            email, phone, iban, bic,
            accepts_einvoice, archived, notes
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(&id)
    .bind(input.contact_type.as_db_str())
    .bind(input.name.trim())
    .bind(
        input
            .legal_form
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .vat_id
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .tax_number
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(input.street.trim())
    .bind(input.postal_code.trim())
    .bind(input.city.trim())
    .bind(input.country_code.trim().to_uppercase())
    .bind(
        input
            .email
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .phone
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .iban
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .bic
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(if input.accepts_einvoice { 1i64 } else { 0i64 })
    .bind(
        input
            .notes
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .execute(pool)
    .await?;

    get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain("INSERT erfolgreich, aber SELECT lieferte None".into()))
}

pub async fn update(pool: &SqlitePool, id: &str, input: &ContactInput) -> Result<ContactRow> {
    validate(input).map_err(|errs| Error::Domain(format_errs(&errs)))?;

    let res = sqlx::query(
        "UPDATE contacts SET
            contact_type = ?, name = ?, legal_form = ?, vat_id = ?, tax_number = ?,
            street = ?, postal_code = ?, city = ?, country_code = ?,
            email = ?, phone = ?, iban = ?, bic = ?,
            accepts_einvoice = ?, notes = ?,
            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(input.contact_type.as_db_str())
    .bind(input.name.trim())
    .bind(
        input
            .legal_form
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .vat_id
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .tax_number
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(input.street.trim())
    .bind(input.postal_code.trim())
    .bind(input.city.trim())
    .bind(input.country_code.trim().to_uppercase())
    .bind(
        input
            .email
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .phone
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .iban
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .bic
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(if input.accepts_einvoice { 1i64 } else { 0i64 })
    .bind(
        input
            .notes
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(id)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Kontakt nicht gefunden: {id}")));
    }

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("UPDATE ok, aber SELECT leer".into()))
}

pub async fn archive(pool: &SqlitePool, id: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE contacts SET archived = 1, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Kontakt nicht gefunden: {id}")));
    }
    Ok(())
}

pub async fn unarchive(pool: &SqlitePool, id: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE contacts SET archived = 0, updated_at = datetime('now','utc') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!("Kontakt nicht gefunden: {id}")));
    }
    Ok(())
}

/// Zählt offene (nicht festgeschriebene) Belege eines Kontakts:
/// Rechnungs- und Angebots-Entwürfe (`locked_at IS NULL`). Steuert die
/// Anonymisierungs-Sperre (Block 19, Manuel-Entscheidung "Nur ohne offene
/// Entwürfe"): solange offene Entwürfe existieren, ist die Anonymisierung
/// blockiert (Entwürfe nutzen den Live-Kontakt, nicht den Snapshot).
/// Rückgabe: `(offene_rechnungs_entwuerfe, offene_angebots_entwuerfe)`.
pub async fn count_open_drafts(pool: &SqlitePool, contact_id: &str) -> Result<(i64, i64)> {
    let invoice_drafts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices WHERE contact_id = ? AND locked_at IS NULL",
    )
    .bind(contact_id)
    .fetch_one(pool)
    .await?;
    let quote_drafts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM quotes WHERE contact_id = ? AND locked_at IS NULL",
    )
    .bind(contact_id)
    .fetch_one(pool)
    .await?;
    Ok((invoice_drafts, quote_drafts))
}

/// Zählt festgeschriebene (aufbewahrungspflichtige) Belege eines Kontakts —
/// für Audit-Detail + UI-Aufklärung, was bei der Anonymisierung über den
/// Buyer-Snapshot erhalten bleibt. Rückgabe: `(rechnungen, angebote)`.
pub async fn count_locked_documents(pool: &SqlitePool, contact_id: &str) -> Result<(i64, i64)> {
    let invoices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices WHERE contact_id = ? AND locked_at IS NOT NULL",
    )
    .bind(contact_id)
    .fetch_one(pool)
    .await?;
    let quotes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM quotes WHERE contact_id = ? AND locked_at IS NOT NULL",
    )
    .bind(contact_id)
    .fetch_one(pool)
    .await?;
    Ok((invoices, quotes))
}

/// DSGVO Art. 17 (Block 19): anonymisiert einen Kontakt. Überschreibt die
/// personenbezogenen Stammdaten (`name` → Platzhalter `"Anonymisiert #<id>"`,
/// alle übrigen Personenfelder NULL), setzt `anonymized_at` + `archived = 1`.
/// `country_code`, `contact_type`, `accepts_einvoice` bleiben (NOT NULL bzw.
/// nicht personenbezogen).
///
/// **Kein DELETE** — die rechnungsgebundenen Daten bleiben über den
/// Buyer-Snapshot (invoices/quotes) 10 Jahre erhalten (§147 AO / GoBD).
///
/// Blockiert (Err), solange offene Entwürfe existieren (siehe
/// [`crate::domain::anonymize::anonymization_blocker`]) oder der Kontakt bereits
/// anonymisiert ist.
pub async fn anonymize(pool: &SqlitePool, id: &str) -> Result<ContactRow> {
    let existing = get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Kontakt nicht gefunden: {id}")))?;
    if existing.anonymized_at.is_some() {
        return Err(Error::Domain("Kontakt ist bereits anonymisiert.".into()));
    }

    let (inv_drafts, quote_drafts) = count_open_drafts(pool, id).await?;
    if let Some(msg) = anonymize_domain::anonymization_blocker(inv_drafts, quote_drafts) {
        return Err(Error::Domain(msg));
    }

    let placeholder = anonymize_domain::anonymized_name(id);
    let res = sqlx::query(
        "UPDATE contacts SET
            name = ?,
            legal_form = NULL, vat_id = NULL, tax_number = NULL,
            street = NULL, postal_code = NULL, city = NULL,
            email = NULL, phone = NULL, iban = NULL, bic = NULL,
            notes = NULL,
            archived = 1,
            anonymized_at = datetime('now','utc'),
            updated_at = datetime('now','utc')
         WHERE id = ? AND anonymized_at IS NULL",
    )
    .bind(&placeholder)
    .bind(id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(format!(
            "Anonymisierung fehlgeschlagen (Kontakt nicht gefunden oder bereits anonymisiert): {id}"
        )));
    }

    get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain("anonymize: post-UPDATE SELECT leer".into()))
}

/// Case-insensitive Substring-Suche über `name`, `email`, `vat_id`,
/// `city`, `postal_code`. Leere Query → leere Liste (verhindert Vollscan via "%%").
pub async fn search(
    pool: &SqlitePool,
    query: &str,
    include_archived: bool,
) -> Result<Vec<ContactRow>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    // Block-3a-Hotfix für Block-2-Bug: SQLite's `LIKE … COLLATE NOCASE`
    // ist ASCII-only — "MÜNCHEN" matched nicht "München". Rust's
    // `str::to_lowercase()` macht Unicode-Case-Folding korrekt. Bei
    // <5000 Kontakten (siehe Block-2-Notes-Scoping) ist der Vollscan
    // ohnehin akzeptabel; FTS5 später als Optimierung.
    let needle = q.to_lowercase();
    let all = list(pool, include_archived).await?;
    let matches: Vec<ContactRow> = all
        .into_iter()
        .filter(|c| {
            let in_field = |s: &str| s.to_lowercase().contains(&needle);
            in_field(&c.name)
                || c.email.as_deref().is_some_and(in_field)
                || c.vat_id.as_deref().is_some_and(in_field)
                || c.city.as_deref().is_some_and(in_field)
                || c.postal_code.as_deref().is_some_and(in_field)
        })
        .collect();
    Ok(matches)
}

fn format_errs(errs: &[crate::domain::contact::ValidationError]) -> String {
    errs.iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
