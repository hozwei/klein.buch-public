//! Repository für `seller_profile` (Singleton, id = 1).
//!
//! - `get()` liefert `Option<SellerProfileRow>` — `None` solange das Profil
//!   nie gespeichert wurde (Frontend zeigt dann leeres Formular).
//! - `upsert()` macht ein INSERT-OR-REPLACE und kümmert sich um die §19-
//!   Verzicht-Logik: beim Wechsel `is_kleinunternehmer 1 → 0` wird
//!   `waived_paragraph_19_since` auf heute gesetzt; ein Audit-Log-Eintrag
//!   wird automatisch geschrieben.
//! - Beim Wechsel zurück (`0 → 1`) prüfen wir die 5-Jahres-Bindung. Falls
//!   verletzt, gibt `upsert` einen Domain-Error zurück (UI muss vorher
//!   warnen — das ist der finale Riegel).

use crate::db::models::SellerProfileRow;
use crate::db::repo::audit_log;
use crate::domain::kleinunternehmer::{waiver_deadline, KleinunternehmerStatus};
use crate::error::{Error, Result};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerProfileInput {
    pub name: String,
    pub legal_form: Option<String>,
    pub street: String,
    pub postal_code: String,
    pub city: String,
    pub country_code: String,
    pub tax_number: Option<String>,
    pub vat_id: Option<String>,
    pub email: String,
    pub phone: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub logo_filename: Option<String>,
    pub is_kleinunternehmer: bool,
    pub default_pdf_template: Option<String>,
    pub default_currency: Option<String>,
    /// Wenn `true` und der bestehende Status §19-aktiv war, signalisiert
    /// das Frontend, dass der Warn-Dialog ("5-Jahres-Bindung verstanden")
    /// bestätigt wurde. Pflicht für Wechsel 1 → 0.
    pub confirm_waive_paragraph_19: Option<bool>,
}

pub async fn get(pool: &SqlitePool) -> Result<Option<SellerProfileRow>> {
    let row: Option<SellerProfileRow> = sqlx::query_as(
        "SELECT id, name, legal_form, street, postal_code, city, country_code,
                tax_number, vat_id, email, phone, iban, bic, logo_filename,
                is_kleinunternehmer, waived_paragraph_19_since,
                default_pdf_template, default_currency, updated_at
         FROM seller_profile WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Persistiert das Profil. §19-Wechsel werden geprüft und auditiert.
pub async fn upsert(pool: &SqlitePool, input: &SellerProfileInput) -> Result<SellerProfileRow> {
    validate_input(input)?;

    let prev = get(pool).await?;
    let prev_status = prev.as_ref().map(|p| KleinunternehmerStatus {
        is_kleinunternehmer: p.is_kleinunternehmer == 1,
        waived_since: p
            .waived_paragraph_19_since
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
    });

    let new_is_klein = input.is_kleinunternehmer;
    let prev_is_klein = prev_status.map(|s| s.is_kleinunternehmer);
    let today = Utc::now().date_naive();

    // Wechsel 1 → 0 (Verzicht auf §19): UI-Bestätigung Pflicht, Datum setzen,
    // Audit-Log.
    let waived_since_new: Option<String> = match (prev_is_klein, new_is_klein) {
        (Some(true), false) => {
            if !input.confirm_waive_paragraph_19.unwrap_or(false) {
                return Err(Error::Domain(
                    "Verzicht auf §19 erfordert explizite Bestätigung der 5-Jahres-Bindung.".into(),
                ));
            }
            audit_log::append(
                pool,
                "seller_profile_waive_paragraph_19",
                "seller_profile",
                "1",
                Some(&format!(
                    "{{\"since\":\"{}\",\"deadline\":\"{}\"}}",
                    today,
                    waiver_deadline(today)
                )),
            )
            .await?;
            Some(today.to_string())
        }
        // Wechsel 0 → 1 (Rückkehr zu §19): 5-Jahres-Bindung prüfen.
        (Some(false), true) => {
            if let Some(prev_waived) = prev
                .as_ref()
                .and_then(|p| p.waived_paragraph_19_since.as_deref())
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            {
                let deadline = waiver_deadline(prev_waived);
                if today < deadline {
                    return Err(Error::Domain(format!(
                        "Rückkehr zu §19 erst ab {deadline} möglich (5-Jahres-Bindung)."
                    )));
                }
            }
            audit_log::append(
                pool,
                "seller_profile_return_to_paragraph_19",
                "seller_profile",
                "1",
                Some(&format!("{{\"since\":\"{today}\"}}")),
            )
            .await?;
            // waived_paragraph_19_since wird auf NULL zurückgesetzt — Bindung ist abgelaufen.
            None
        }
        // Keine Statusänderung: Bestand übernehmen.
        _ => prev
            .as_ref()
            .and_then(|p| p.waived_paragraph_19_since.clone()),
    };

    let default_pdf_template = input
        .default_pdf_template
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let default_currency = input
        .default_currency
        .clone()
        .unwrap_or_else(|| "EUR".to_string());

    sqlx::query(
        "INSERT INTO seller_profile (
            id, name, legal_form, street, postal_code, city, country_code,
            tax_number, vat_id, email, phone, iban, bic, logo_filename,
            is_kleinunternehmer, waived_paragraph_19_since,
            default_pdf_template, default_currency, updated_at
         ) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now','utc'))
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            legal_form = excluded.legal_form,
            street = excluded.street,
            postal_code = excluded.postal_code,
            city = excluded.city,
            country_code = excluded.country_code,
            tax_number = excluded.tax_number,
            vat_id = excluded.vat_id,
            email = excluded.email,
            phone = excluded.phone,
            iban = excluded.iban,
            bic = excluded.bic,
            logo_filename = excluded.logo_filename,
            is_kleinunternehmer = excluded.is_kleinunternehmer,
            waived_paragraph_19_since = excluded.waived_paragraph_19_since,
            default_pdf_template = excluded.default_pdf_template,
            default_currency = excluded.default_currency,
            updated_at = datetime('now','utc')",
    )
    .bind(input.name.trim())
    .bind(
        input
            .legal_form
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
            .tax_number
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
    .bind(input.email.trim())
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
    .bind(
        input
            .logo_filename
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()),
    )
    .bind(if new_is_klein { 1i64 } else { 0i64 })
    .bind(waived_since_new.as_deref())
    .bind(&default_pdf_template)
    .bind(&default_currency)
    .execute(pool)
    .await?;

    get(pool)
        .await?
        .ok_or_else(|| Error::Domain("UPSERT ok, aber SELECT leer".into()))
}

/// Setzt nur den Logo-Dateinamen (ohne §19-Logik). Das Profil muss existieren —
/// sonst Domain-Fehler (Logo-Upload vor erstem Profil-Save ist nicht sinnvoll).
pub async fn set_logo_filename(pool: &SqlitePool, filename: Option<&str>) -> Result<()> {
    let res = sqlx::query(
        "UPDATE seller_profile SET logo_filename = ?, updated_at = datetime('now','utc') WHERE id = 1",
    )
    .bind(filename)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(
            "Bitte zuerst die Firmendaten speichern, bevor du ein Logo hochlädst.".into(),
        ));
    }
    Ok(())
}

/// Setzt nur die globale Standard-PDF-Vorlage (Block 17a). Profil muss
/// existieren. Der Name wird im Command gegen die verfügbaren Vorlagen +
/// §19-Kompatibilität geprüft — hier nur der reine UPDATE.
pub async fn set_default_pdf_template(pool: &SqlitePool, name: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE seller_profile SET default_pdf_template = ?, updated_at = datetime('now','utc') WHERE id = 1",
    )
    .bind(name)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::Domain(
            "Bitte zuerst die Firmendaten speichern, bevor du eine Vorlage wählst.".into(),
        ));
    }
    Ok(())
}

fn validate_input(input: &SellerProfileInput) -> Result<()> {
    let mut missing: Vec<&str> = Vec::new();
    if input.name.trim().is_empty() {
        missing.push("Name");
    }
    if input.street.trim().is_empty() {
        missing.push("Straße");
    }
    if input.postal_code.trim().is_empty() {
        missing.push("PLZ");
    }
    if input.city.trim().is_empty() {
        missing.push("Stadt");
    }
    if input.country_code.trim().is_empty() {
        missing.push("Land");
    }
    // Steuernummer ist absichtlich NICHT pflicht beim Profil-Save —
    // Kleinunternehmer-Onboarding läuft oft vor STNr-Erteilung durchs FA.
    // Der §14-UStG-Pflichtangaben-Check (Steuernummer ODER USt-IdNr.)
    // wird in Block 3 beim Rechnungs-Issue erzwungen.
    if input.email.trim().is_empty() {
        missing.push("E-Mail");
    }
    if !missing.is_empty() {
        return Err(Error::Domain(format!(
            "Pflichtfeld(er) fehlen: {}",
            missing.join(", ")
        )));
    }
    // §14c-Schutz: Kleinunternehmer dürfen keine USt-IdNr-only-Rechnungen
    // ausweisen — die VAT-ID darf trotzdem gepflegt sein (Auslandsumsätze
    // brauchen sie). Hier also kein Verbot, nur Hinweis im UI.
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
