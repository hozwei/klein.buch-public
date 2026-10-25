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
use chrono::NaiveDate;
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
    pub is_kleinunternehmer: bool,
    pub default_pdf_template: Option<String>,
    pub default_currency: Option<String>,
    /// Wenn `true` und der bestehende Status §19-aktiv war, signalisiert
    /// das Frontend, dass der Warn-Dialog ("5-Jahres-Bindung verstanden")
    /// bestätigt wurde. Pflicht für Wechsel 1 → 0.
    pub confirm_waive_paragraph_19: Option<bool>,
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
/// zählt nicht) und der §19-Status samt Verzicht (§14c-Risiko). Ohne
/// gespeichertes Profil gelten die Standardwerte (keine IBAN/BIC, §19 aktiv).
pub fn reauth_relevant_change(prev: Option<&SellerProfileRow>, input: &SellerProfileInput) -> bool {
    let (iban, bic, klein) = match prev {
        Some(p) => (
            normalized_account(p.iban.as_deref()),
            normalized_account(p.bic.as_deref()),
            p.is_kleinunternehmer == 1,
        ),
        None => (None, None, true),
    };
    iban != normalized_account(input.iban.as_deref())
        || bic != normalized_account(input.bic.as_deref())
        || klein != input.is_kleinunternehmer
}

/// Period-aware §19-Status eines Geschäftsjahres aus dem Profil (R2-002): für
/// Jahre vor dem Verzichts-Stichtag gilt §19 weiter; ohne Profil gilt §19
/// (Zielgruppe von Klein.Buch).
pub fn is_klein_for_year(profile: Option<&SellerProfileRow>, fiscal_year: i32) -> bool {
    let status = profile
        .map(|p| KleinunternehmerStatus {
            is_kleinunternehmer: p.is_kleinunternehmer == 1,
            waived_since: p
                .waived_paragraph_19_since
                .as_deref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        })
        .unwrap_or(KleinunternehmerStatus {
            is_kleinunternehmer: true,
            waived_since: None,
        });
    crate::domain::kleinunternehmer::is_active_for_year(&status, fiscal_year)
}

/// §19-Status, unter dem eine Kostenposition steuerlich zu behandeln ist
/// (Lauf F8d, ADR 0039 Nr. 148): maßgeblich ist der Status bei
/// **Leistungsbezug**, nicht im Zahlungsjahr (§ 15 Abs. 1 Satz 1 Nr. 1 UStG;
/// Abschn. 19.5 UStAE zum Wechsel der Besteuerungsform). Das bestimmt die
/// Aufteilung netto/Vorsteuer in EÜR-Ausfüllhilfe und DATEV-Stapel und die
/// Anschaffungskosten aktivierter Kosten (§ 9b Abs. 1 EStG). Kosten haben
/// kein eigenes Leistungsdatum; es gilt das Belegdatum (`expense_date`, ein
/// festgeschriebenes Kernfeld). Die zeitliche Zuordnung der Ausgabe
/// (Zahlungsjahr, § 11 Abs. 2 EStG) bleibt davon unberührt.
pub fn is_klein_for_expense(
    profile: Option<&SellerProfileRow>,
    expense_date: &str,
) -> Result<bool> {
    let head = expense_date.get(..10).unwrap_or(expense_date);
    let date = NaiveDate::parse_from_str(head, "%Y-%m-%d").map_err(|e| {
        Error::Domain(format!(
            "Kosten: ungültiges Belegdatum '{expense_date}': {e}"
        ))
    })?;
    Ok(is_klein_for_year(profile, chrono::Datelike::year(&date)))
}

const SELECT_PROFILE: &str = "SELECT id, name, legal_form, street, postal_code, city, country_code,
                tax_number, vat_id, email, phone, iban, bic, logo_filename,
                is_kleinunternehmer, waived_paragraph_19_since,
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
    let today = crate::clock::today_berlin();

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
                Some(serde_json::json!({
                    "since": today.to_string(),
                    "deadline": waiver_deadline(today).to_string(),
                })),
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
                Some(serde_json::json!({ "since": today.to_string() })),
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
    // logo_filename: beim ersten Anlegen leer, danach unverändert (nur über
    // set_logo_filename aus dem Logo-Upload).
    .bind(None::<String>)
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
            is_kleinunternehmer: true,
            default_pdf_template: None,
            default_currency: None,
            confirm_waive_paragraph_19: None,
        }
    }

    fn row(iban: Option<&str>, bic: Option<&str>, klein: bool) -> SellerProfileRow {
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
            is_kleinunternehmer: i64::from(klein),
            waived_paragraph_19_since: None,
            default_pdf_template: "default".into(),
            default_currency: "EUR".into(),
            updated_at: "2026-10-25 08:00:00".into(),
        }
    }

    /// W14-038 (Lauf F4a): Matrix der Re-Auth-pflichtigen Änderungen.
    #[test]
    fn reauth_relevant_change_matrix() {
        // Erstes Speichern ohne Zahlungsdaten, §19 aktiv: nichts Relevantes.
        assert!(!reauth_relevant_change(None, &input()));
        // Erstes Speichern mit IBAN bzw. ohne §19: relevant.
        let mut with_iban = input();
        with_iban.iban = Some("DE02120300000000202051".into());
        assert!(reauth_relevant_change(None, &with_iban));
        let mut regular = input();
        regular.is_kleinunternehmer = false;
        assert!(reauth_relevant_change(None, &regular));

        let stored = row(
            Some("DE02 1203 0000 0000 2020 51"),
            Some("BYLADEM1001"),
            true,
        );
        // Adresse, Name, E-Mail: nicht relevant.
        let mut same = input();
        same.iban = Some("de02120300000000202051".into());
        same.bic = Some(" BYLADEM1001 ".into());
        same.street = "Neuer Weg 2".into();
        same.email = "neu@example.org".into();
        assert!(!reauth_relevant_change(Some(&stored), &same));
        // IBAN, BIC, §19-Status: relevant; Leeren zählt als Änderung.
        let mut iban = same.clone();
        iban.iban = Some("DE89370400440532013000".into());
        assert!(reauth_relevant_change(Some(&stored), &iban));
        let mut bic = same.clone();
        bic.bic = Some("COBADEFFXXX".into());
        assert!(reauth_relevant_change(Some(&stored), &bic));
        let mut cleared = same.clone();
        cleared.iban = Some("  ".into());
        assert!(reauth_relevant_change(Some(&stored), &cleared));
        let mut waive = same.clone();
        waive.is_kleinunternehmer = false;
        waive.confirm_waive_paragraph_19 = Some(true);
        assert!(reauth_relevant_change(Some(&stored), &waive));
        let back = row(None, None, false);
        assert!(reauth_relevant_change(Some(&back), &input()));
    }

    /// Lauf F8d (ADR 0039 Nr. 148): Der Status von Kosten folgt dem Jahr des
    /// Belegdatums (Leistungsbezug), nicht dem Zahlungsjahr. Verzicht 2027:
    /// Kosten mit Belegdatum Dezember 2026 bleiben §19, ab 2027 Regel.
    #[test]
    fn expense_status_follows_the_document_date() {
        let mut waived = row(None, None, false);
        waived.waived_paragraph_19_since = Some("2027-03-15".into());
        assert!(is_klein_for_expense(Some(&waived), "2026-12-31").unwrap());
        assert!(!is_klein_for_expense(Some(&waived), "2027-01-01").unwrap());
        // Zeitanteil wird ignoriert; ohne Profil gilt §19.
        assert!(!is_klein_for_expense(Some(&waived), "2027-06-01 10:00:00").unwrap());
        assert!(is_klein_for_expense(None, "2030-01-01").unwrap());
        assert!(is_klein_for_expense(Some(&waived), "31.12.2026").is_err());
    }
}
