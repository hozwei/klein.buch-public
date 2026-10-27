//! Repository für `seller_profile` (Singleton, id = 1).
//!
//! - `get()` liefert `Option<SellerProfileRow>` — `None` solange das Profil
//!   nie gespeichert wurde (Frontend zeigt dann leeres Formular).
//! - `upsert()` macht ein INSERT-OR-UPDATE der Stammdaten. Den Steuerstatus
//!   nach §19 ändert es nicht mehr (ADR 0042 Nr. 1): Er steht in der Historie
//!   `tax_status_changes` ([`crate::db::repo::tax_status`]). Die Spalten
//!   `is_kleinunternehmer`/Verzichtsdatum bleiben als Spiegel, gepflegt nur
//!   dort; das Modell [`SellerProfileRow`] liest sie nicht.
//! - Für das Frontend ergänzt [`SellerProfileView`] den Status von heute aus
//!   der Historie (Antwortform wie bisher).

use crate::db::models::SellerProfileRow;
use crate::db::repo::tax_status;
use crate::error::{Error, Result};
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
    /// Wird ignoriert (Plan W7, Gegenprüfung): der Logo-Dateiname kommt nie
    /// vom Client, gesetzt nur über `seller_logo_set`/`seller_logo_clear`.
    #[serde(default, skip_deserializing)]
    pub logo_filename: Option<String>,
    /// Veraltet (ADR 0042 Nr. 1): Der Steuerstatus ändert sich nur noch über
    /// `tax_status_change_add`. Schickt ein Client einen Wert, der vom Status
    /// heute abweicht, lehnt `upsert` ab, statt ihn still zu übergehen.
    #[serde(default)]
    pub is_kleinunternehmer: Option<bool>,
    pub default_pdf_template: Option<String>,
    pub default_currency: Option<String>,
}

/// Profil mit dem Steuerstatus von heute aus der Historie (ADR 0042 Nr. 1) —
/// dieselbe Antwortform wie vor Migration 0033 (`isKleinunternehmer` 0/1,
/// `waivedParagraph19Since`). Maßgeblich für Belege ist der Status am Datum des
/// Belegs, nicht dieser Wert.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerProfileView {
    #[serde(flatten)]
    pub profile: SellerProfileRow,
    /// §19 heute (0/1).
    pub is_kleinunternehmer: i64,
    /// Beginn der laufenden Regelbesteuerung; leer bei §19 oder „seit Beginn“.
    #[serde(rename = "waivedParagraph19Since")]
    pub regel_since: Option<String>,
}

/// IBAN/BIC für den Vergleich: ohne Leerraum, Großbuchstaben, leer = keine.
fn normalized_account(v: Option<&str>) -> Option<String> {
    v.map(|s| {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_uppercase()
    })
    .filter(|s| !s.is_empty())
}

/// Ändert `input` gegenüber dem gespeicherten Profil etwas, das eine frische
/// Re-Auth verlangt (W14-038, Lauf F4a)? Das sind die Zahlungsdaten auf jeder
/// künftigen Rechnung (IBAN, BIC; Schreibweise mit Leerzeichen oder klein
/// zählt nicht). Der §19-Status ist seit ADR 0042 nicht mehr Teil des Profils;
/// seine Einträge verlangen immer eine frische Re-Auth (Registry). Ohne
/// gespeichertes Profil gelten die Standardwerte (keine IBAN/BIC).
pub fn reauth_relevant_change(prev: Option<&SellerProfileRow>, input: &SellerProfileInput) -> bool {
    let (iban, bic) = match prev {
        Some(p) => (
            normalized_account(p.iban.as_deref()),
            normalized_account(p.bic.as_deref()),
        ),
        None => (None, None),
    };
    iban != normalized_account(input.iban.as_deref())
        || bic != normalized_account(input.bic.as_deref())
}

const SELECT_PROFILE: &str = "SELECT id, name, legal_form, street, postal_code, city, country_code,
                tax_number, vat_id, email, phone, iban, bic, logo_filename,
                default_pdf_template, default_currency, updated_at
         FROM seller_profile WHERE id = 1";

pub async fn get(pool: &SqlitePool) -> Result<Option<SellerProfileRow>> {
    let row: Option<SellerProfileRow> = sqlx::query_as(SELECT_PROFILE).fetch_optional(pool).await?;
    Ok(row)
}

/// Wie [`get`], auf einer Verbindung bzw. in einer laufenden Transaktion.
pub async fn get_in(conn: &mut sqlx::SqliteConnection) -> Result<Option<SellerProfileRow>> {
    let row: Option<SellerProfileRow> = sqlx::query_as(SELECT_PROFILE).fetch_optional(conn).await?;
    Ok(row)
}

/// Profil mit dem Status von heute ([`SellerProfileView`]).
pub async fn get_view(pool: &SqlitePool) -> Result<Option<SellerProfileView>> {
    let Some(profile) = get(pool).await? else {
        return Ok(None);
    };
    let hist = tax_status::history(pool).await?;
    let today = crate::clock::today_berlin();
    let klein = hist.is_klein_at(today);
    let regel_since = match hist.entry_at(today) {
        Some(e) if !klein && e.effective_from != crate::domain::tax_status::anfang() => {
            Some(e.effective_from.format("%Y-%m-%d").to_string())
        }
        _ => None,
    };
    Ok(Some(SellerProfileView {
        profile,
        is_kleinunternehmer: i64::from(klein),
        regel_since,
    }))
}

/// Persistiert die Stammdaten des Profils. Der §19-Status bleibt unberührt
/// (ADR 0042 Nr. 1); ein abweichender `is_kleinunternehmer` im Input wird
/// abgelehnt. Profil und Spiegel des Status entstehen in einer Transaktion.
pub async fn upsert(pool: &SqlitePool, input: &SellerProfileInput) -> Result<SellerProfileRow> {
    validate_input(input)?;

    let today = crate::clock::today_berlin();
    let mut tx = crate::db::begin_immediate(pool).await?;
    if let Some(requested) = input.is_kleinunternehmer {
        let current = tax_status::history_in(&mut tx).await?.is_klein_at(today);
        if requested != current {
            return Err(Error::Domain(
                "Den Steuerstatus (Kleinunternehmer nach §19 oder Regelbesteuerung) änderst \
                 du unter Einstellungen → Meine Firmendaten → Steuerstatus mit \
                 „Statuswechsel eintragen …“ (Grund und Wirkungsdatum)."
                    .into(),
            ));
        }
    }

    let default_pdf_template = input
        .default_pdf_template
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let default_currency = input
        .default_currency
        .clone()
        .unwrap_or_else(|| "EUR".to_string());

    let trimmed = |v: &Option<String>| -> Option<String> {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    sqlx::query(
        "INSERT INTO seller_profile (
            id, name, legal_form, street, postal_code, city, country_code,
            tax_number, vat_id, email, phone, iban, bic, logo_filename,
            default_pdf_template, default_currency, updated_at
         ) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now','utc'))
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
            default_pdf_template = excluded.default_pdf_template,
            default_currency = excluded.default_currency,
            updated_at = datetime('now','utc')",
    )
    .bind(input.name.trim())
    .bind(trimmed(&input.legal_form))
    .bind(input.street.trim())
    .bind(input.postal_code.trim())
    .bind(input.city.trim())
    .bind(input.country_code.trim().to_uppercase())
    .bind(trimmed(&input.tax_number))
    .bind(trimmed(&input.vat_id))
    .bind(input.email.trim())
    .bind(trimmed(&input.phone))
    .bind(trimmed(&input.iban))
    .bind(trimmed(&input.bic))
    // logo_filename: beim ersten Anlegen leer, danach unverändert (nur über
    // set_logo_filename aus dem Logo-Upload).
    .bind(None::<String>)
    .bind(&default_pdf_template)
    .bind(&default_currency)
    .execute(&mut *tx)
    .await?;
    // Spiegel des Status (beim ersten Anlegen sonst der Spalten-Standard §19).
    tax_status::refresh_mirror_in(&mut tx, today).await?;
    let row = get_in(&mut tx)
        .await?
        .ok_or_else(|| Error::Domain("UPSERT ok, aber SELECT leer".into()))?;
    tx.commit().await?;
    Ok(row)
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
    use super::*;

    fn input() -> SellerProfileInput {
        SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            tax_number: None,
            vat_id: None,
            email: "kontakt@example.org".into(),
            phone: None,
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: None,
            default_pdf_template: None,
            default_currency: None,
        }
    }

    fn row(iban: Option<&str>, bic: Option<&str>) -> SellerProfileRow {
        SellerProfileRow {
            id: 1,
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            tax_number: None,
            vat_id: None,
            email: "kontakt@example.org".into(),
            phone: None,
            iban: iban.map(str::to_string),
            bic: bic.map(str::to_string),
            logo_filename: None,
            default_pdf_template: "default".into(),
            default_currency: "EUR".into(),
            updated_at: "2026-10-25 08:00:00".into(),
        }
    }

    /// W14-038 (Lauf F4a): Matrix der Re-Auth-pflichtigen Änderungen. Der
    /// §19-Status gehört seit ADR 0042 nicht mehr dazu (eigene Commands mit
    /// Re-Auth).
    #[test]
    fn reauth_relevant_change_matrix() {
        // Erstes Speichern ohne Zahlungsdaten: nichts Relevantes.
        assert!(!reauth_relevant_change(None, &input()));
        // Erstes Speichern mit IBAN: relevant.
        let mut with_iban = input();
        with_iban.iban = Some("DE02120300000000202051".into());
        assert!(reauth_relevant_change(None, &with_iban));
        let mut regular = input();
        regular.is_kleinunternehmer = Some(false);
        assert!(!reauth_relevant_change(None, &regular));

        let stored = row(Some("DE02 1203 0000 0000 2020 51"), Some("BYLADEM1001"));
        // Adresse, Name, E-Mail: nicht relevant.
        let mut same = input();
        same.iban = Some("de02120300000000202051".into());
        same.bic = Some(" BYLADEM1001 ".into());
        same.street = "Neuer Weg 2".into();
        same.email = "neu@example.org".into();
        assert!(!reauth_relevant_change(Some(&stored), &same));
        // IBAN, BIC: relevant; Leeren zählt als Änderung.
        let mut iban = same.clone();
        iban.iban = Some("DE89370400440532013000".into());
        assert!(reauth_relevant_change(Some(&stored), &iban));
        let mut bic = same.clone();
        bic.bic = Some("COBADEFFXXX".into());
        assert!(reauth_relevant_change(Some(&stored), &bic));
        let mut cleared = same.clone();
        cleared.iban = Some("  ".into());
        assert!(reauth_relevant_change(Some(&stored), &cleared));
    }
}
