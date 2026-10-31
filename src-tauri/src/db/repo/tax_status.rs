//! Repository der §19-Statushistorie (`tax_status_changes`, Migration 0033,
//! ADR 0042 Nr. 1–6).
//!
//! - Lesen: [`history`] bzw. [`history_in`] liefern die gültigen Einträge als
//!   [`TaxStatusHistory`]; alle Lesestellen fragen den Status am maßgeblichen
//!   Datum des Belegs ab (ADR 0042 Nr. 3), nicht mehr das Verkäuferprofil.
//! - Schreiben: [`insert_in`] und [`revoke_in`] laufen in der Transaktion des
//!   Aufrufers, zusammen mit dem Audit (`tax_status.changed` bzw.
//!   `tax_status.revoked`) und dem Spiegel im Profil ([`refresh_mirror_in`]).
//! - Die Spalten `seller_profile.is_kleinunternehmer` und
//!   `waived_paragraph_19_since` schreibt nur noch [`refresh_mirror_in`]; kein
//!   Code liest sie (Test `mirror_columns_are_written_only_here`).

use crate::db::repo::audit_log;
use crate::domain::tax_status::{
    CapitalizedInputTax, ChangeReason, TaxStatus, TaxStatusEntry, TaxStatusHistory, UmsatzPosten,
    YearStatus, ANFANG,
};
use crate::error::{Error, Result};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};

/// Ein Eintrag der Historie, wie gespeichert (auch widerrufene).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusChangeRow {
    pub id: String,
    /// JJJJ-MM-TT; „0001-01-01“ = seit Beginn.
    pub effective_from: String,
    pub status: String,
    pub reason: String,
    pub binding_until: Option<String>,
    pub trigger_document: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub revoke_reason: Option<String>,
}

const SELECT: &str = "SELECT id, effective_from, status, reason, binding_until,
                             trigger_document, note, created_at, revoked_at, revoke_reason
                        FROM tax_status_changes
                       ORDER BY effective_from, created_at, id";

/// Alle Einträge (auch widerrufene), nach Wirkungsdatum.
pub async fn list(pool: &SqlitePool) -> Result<Vec<TaxStatusChangeRow>> {
    Ok(sqlx::query_as(SELECT).fetch_all(pool).await?)
}

/// Wie [`list`], in einer laufenden Transaktion.
pub async fn list_in(conn: &mut SqliteConnection) -> Result<Vec<TaxStatusChangeRow>> {
    Ok(sqlx::query_as(SELECT).fetch_all(conn).await?)
}

fn parse_day(s: &str, what: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("Steuerstatus: ungültiges {what} '{s}': {e}")))
}

fn to_entry(r: &TaxStatusChangeRow) -> Result<TaxStatusEntry> {
    Ok(TaxStatusEntry {
        id: r.id.clone(),
        effective_from: parse_day(&r.effective_from, "Wirkungsdatum")?,
        status: TaxStatus::parse(&r.status)
            .ok_or_else(|| Error::Domain(format!("Steuerstatus: unbekannt '{}'", r.status)))?,
        reason: ChangeReason::parse(&r.reason).ok_or_else(|| {
            Error::Domain(format!("Steuerstatus: unbekannter Grund '{}'", r.reason))
        })?,
        binding_until: r
            .binding_until
            .as_deref()
            .map(|s| parse_day(s, "Bindungsende"))
            .transpose()?,
        revoked: r.revoked_at.is_some(),
    })
}

/// Historie aus gespeicherten Einträgen.
pub fn history_of(rows: &[TaxStatusChangeRow]) -> Result<TaxStatusHistory> {
    Ok(TaxStatusHistory::new(
        rows.iter().map(to_entry).collect::<Result<Vec<_>>>()?,
    ))
}

/// Gültige Historie.
pub async fn history(pool: &SqlitePool) -> Result<TaxStatusHistory> {
    history_of(&list(pool).await?)
}

/// Wie [`history`], in einer laufenden Transaktion.
pub async fn history_in(conn: &mut SqliteConnection) -> Result<TaxStatusHistory> {
    history_of(&list_in(conn).await?)
}

/// §19 am `date`?
pub async fn is_klein_at(pool: &SqlitePool, date: NaiveDate) -> Result<bool> {
    Ok(history(pool).await?.is_klein_at(date))
}

/// Status des Geschäftsjahres (dreiwertig).
pub async fn year_status(pool: &SqlitePool, year: i32) -> Result<YearStatus> {
    Ok(history(pool).await?.year_status(year))
}

/// Tagesdatum aus einem Belegdatum (Zeitanteil wird ignoriert).
pub fn parse_document_date(value: &str, what: &str) -> Result<NaiveDate> {
    let head = value.get(..10).unwrap_or(value);
    NaiveDate::parse_from_str(head, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("{what}: ungültiges Datum '{value}': {e}")))
}

/// §19-Status, unter dem eine Kostenposition steuerlich zu behandeln ist
/// (Lauf F8d, ADR 0039 Nr. 148; datumsgenau seit ADR 0042 Nr. 3): maßgeblich
/// ist der Status bei **Leistungsbezug** (§ 15 Abs. 1 Satz 1 Nr. 1 UStG;
/// Abschn. 19.5 UStAE). Kosten haben kein eigenes Leistungsdatum; es gilt das
/// Belegdatum (`expense_date`, ein festgeschriebenes Kernfeld). Die zeitliche
/// Zuordnung der Ausgabe (Zahlungsjahr, § 11 Abs. 2 EStG) bleibt unberührt.
pub fn is_klein_for_expense(hist: &TaxStatusHistory, expense_date: &str) -> Result<bool> {
    Ok(hist.is_klein_at(parse_document_date(expense_date, "Kosten: Belegdatum")?))
}

/// Abgeschlossene Geschäftsjahre.
pub async fn closed_years_in(conn: &mut SqliteConnection) -> Result<Vec<i32>> {
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT fiscal_year FROM fiscal_year_locks")
        .fetch_all(conn)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(y,)| i32::try_from(y).ok())
        .collect())
}

/// Aktivierte Kosten mit Vorsteuer, deren Zahlung in einem der `closed_years`
/// liegt (ADR 0042 Nr. 4): Belegdatum und Zahljahr, wie sie
/// `repo::euer::capitalized_input_tax` für den Jahres-Snapshot liest.
pub async fn capitalized_input_tax_in(
    conn: &mut SqliteConnection,
    closed_years: &[i32],
) -> Result<Vec<CapitalizedInputTax>> {
    if closed_years.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT expense_date, paid_date
           FROM expenses
          WHERE status = 'recorded'
            AND paid_date IS NOT NULL
            AND capitalized_as_asset_id IS NOT NULL
            AND tax_amount_cents > 0",
    )
    .fetch_all(conn)
    .await?;
    let mut out = Vec::new();
    for (expense_date, paid_date) in rows {
        let paid_year = parse_document_date(&paid_date, "Kosten: Zahldatum")?.year();
        if !closed_years.contains(&paid_year) {
            continue;
        }
        out.push(CapitalizedInputTax {
            expense_date: parse_document_date(&expense_date, "Kosten: Belegdatum")?,
            paid_year,
        });
    }
    Ok(out)
}

/// Ein neuer Eintrag (geprüft vom Aufrufer mit
/// [`crate::domain::tax_status::validate_change`]).
pub struct NewEntry<'a> {
    pub effective_from: NaiveDate,
    pub status: TaxStatus,
    pub reason: ChangeReason,
    pub binding_until: Option<NaiveDate>,
    pub trigger_document: Option<&'a str>,
    pub note: Option<&'a str>,
}

fn day_str(d: NaiveDate) -> String {
    if d == crate::domain::tax_status::anfang() {
        ANFANG.to_string()
    } else {
        d.format("%Y-%m-%d").to_string()
    }
}

/// Legt einen Eintrag an, mit Audit `tax_status.changed` in derselben
/// Transaktion. Liefert die ID (UUIDv7).
pub async fn insert_in(conn: &mut SqliteConnection, e: &NewEntry<'_>) -> Result<String> {
    let id = uuid::Uuid::now_v7().to_string();
    let effective = day_str(e.effective_from);
    let binding = e.binding_until.map(day_str);
    sqlx::query(
        "INSERT INTO tax_status_changes
            (id, effective_from, status, reason, binding_until, trigger_document, note)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&effective)
    .bind(e.status.as_str())
    .bind(e.reason.as_str())
    .bind(binding.as_deref())
    .bind(e.trigger_document)
    .bind(e.note)
    .execute(&mut *conn)
    .await?;
    audit_log::append_in(
        conn,
        "tax_status.changed",
        "tax_status_change",
        &id,
        Some(serde_json::json!({
            "effective_from": effective,
            "status": e.status.as_str(),
            "reason": e.reason.as_str(),
            "binding_until": binding,
            "trigger_document": e.trigger_document,
        })),
    )
    .await?;
    Ok(id)
}

/// Wie [`insert_in`] samt Spiegel, in einer eigenen Transaktion — für interne
/// Aufrufer und Tests, die einen schon geprüften Eintrag anlegen. Der Command
/// `tax_status_change_add` prüft vorher mit
/// [`crate::domain::tax_status::validate_change`].
pub async fn insert(pool: &SqlitePool, e: &NewEntry<'_>, today: NaiveDate) -> Result<String> {
    let mut tx = crate::db::begin_immediate(pool).await?;
    let id = insert_in(&mut tx, e).await?;
    refresh_mirror_in(&mut tx, today).await?;
    tx.commit().await?;
    Ok(id)
}
/// Widerruft einen Eintrag (einmalig, mit Grund), mit Audit
/// `tax_status.revoked` in derselben Transaktion.
pub async fn revoke_in(conn: &mut SqliteConnection, id: &str, reason: &str) -> Result<()> {
    let res = sqlx::query(
        "UPDATE tax_status_changes
            SET revoked_at = datetime('now','utc'), revoke_reason = ?
          WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(reason)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    if res.rows_affected() != 1 {
        return Err(Error::Domain(crate::domain::tax_status::message(
            &crate::domain::tax_status::ChangeError::NotFound,
        )));
    }
    audit_log::append_in(
        conn,
        "tax_status.revoked",
        "tax_status_change",
        id,
        Some(serde_json::json!({ "reason": reason })),
    )
    .await?;
    Ok(())
}

/// Spiegel im Verkäuferprofil auf den Stand von `today` setzen (ADR 0042 Nr. 1):
/// `is_kleinunternehmer` = Status heute, `waived_paragraph_19_since` = Beginn
/// der laufenden Regelbesteuerung (leer seit Beginn bzw. bei §19). Nur
/// informativ für ältere Leser; maßgeblich ist die Historie. Ohne Profil ein
/// No-Op.
pub async fn refresh_mirror_in(conn: &mut SqliteConnection, today: NaiveDate) -> Result<()> {
    let hist = history_in(conn).await?;
    let entry = hist.entry_at(today);
    let klein = hist.is_klein_at(today);
    let since = match entry {
        Some(e) if !klein && e.effective_from != crate::domain::tax_status::anfang() => {
            Some(day_str(e.effective_from))
        }
        _ => None,
    };
    sqlx::query(
        "UPDATE seller_profile SET is_kleinunternehmer = ?, waived_paragraph_19_since = ?
          WHERE id = 1",
    )
    .bind(i64::from(klein))
    .bind(since)
    .execute(conn)
    .await?;
    Ok(())
}

/// Vereinnahmte Entgelte (netto) der Jahre `years` für die Grenzprüfung (ADR
/// 0042 Nr. 5): Zahlungseingänge auf festgeschriebene Ausgangsrechnungen
/// (Netto-Anteil je Steuergruppe), Storno-Erstattungen negativ. Erlöse aus dem
/// Abgang von Anlagen erfasst Klein.Buch nicht als Rechnung; sie bleiben
/// außen vor (§ 19 Abs. 2 UStG).
pub async fn turnover_items(pool: &SqlitePool, years: &[i32]) -> Result<Vec<UmsatzPosten>> {
    let dates: std::collections::HashMap<String, (Option<String>, String)> =
        sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT invoice_number, COALESCE(delivery_date, service_period_end), invoice_date
               FROM invoices
              WHERE direction = 'issued' AND locked_at IS NOT NULL",
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(n, d, i)| (n, (d, i)))
        .collect();
    let doc_date = |number: &str, fallback: &str| -> Result<NaiveDate> {
        let raw = dates
            .get(number)
            .map(|(d, i)| d.clone().unwrap_or_else(|| i.clone()))
            .unwrap_or_else(|| fallback.to_string());
        parse_document_date(&raw, "Umsatz")
    };
    let mut out = Vec::new();
    for &y in years {
        // ADR 0042 Nr. 9: Eine stornierte Zahlung war kein Zufluss — das Paar
        // aus Zahlung und Gegenbuchung bleibt ganz außen vor (auch als
        // auslösender Beleg einer Grenze).
        for i in crate::db::repo::euer::income_detail(pool, y)
            .await?
            .into_iter()
            .filter(|i| !i.is_reversal && !i.reversed)
        {
            let net: i64 = if i.shares.is_empty() {
                i.amount_cents
            } else {
                i.shares.iter().map(|s| s.net_cents).sum()
            };
            out.push(UmsatzPosten {
                paid_date: parse_document_date(&i.paid_date, "Zahlung")?,
                net_cents: net,
                document_date: doc_date(&i.invoice_number, &i.paid_date)?,
                document: i.invoice_number,
            });
        }
        for s in crate::db::repo::euer::storno_detail(pool, y).await? {
            let net: i64 = if s.shares.is_empty() {
                s.refunded_cents
            } else {
                s.shares.iter().map(|x| x.net_cents).sum()
            };
            let date = parse_document_date(&s.storno_date, "Storno")?;
            out.push(UmsatzPosten {
                paid_date: date,
                net_cents: -net,
                document_date: date,
                document: s.storno_number,
            });
        }
    }
    Ok(out)
}

/// Gibt es Zahlungseingänge vor dem Jahr `year`? (Gründungsjahr-Vermutung,
/// ADR 0042 Nr. 5.)
pub async fn has_turnover_before(pool: &SqlitePool, year: i32) -> Result<bool> {
    let years = crate::db::repo::euer::available_years(pool).await?;
    for y in years.into_iter().filter(|y| *y < year) {
        if crate::db::repo::euer::income_detail(pool, y)
            .await?
            .iter()
            .any(|i| !i.is_reversal && !i.reversed)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                rs_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// ADR 0042 Nr. 1: Die Spiegelspalten im Verkäuferprofil sind nicht mehr
    /// maßgeblich. Außer diesem Modul (das sie schreibt) nennt kein Quelltext
    /// `waived_paragraph_19_since`, und kein SQL-Text verbindet
    /// `seller_profile` mit `is_kleinunternehmer`. Das Rust-Modell
    /// `SellerProfileRow` hat beide Felder nicht mehr; der Compiler fängt jeden
    /// Lesezugriff über das Modell, dieser Test die SQL-Texte.
    #[test]
    fn mirror_columns_are_written_only_here() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        rs_files(&root.join("src"), &mut files);
        rs_files(&root.join("tests"), &mut files);
        let me = root.join("src/db/repo/tax_status.rs");
        // Migrations- und Restore-Test legen den Stand vor 0033 an (Schalter und
        // Datum im Profil).
        let pre_0033 = [
            root.join("tests/migration_0033_test.rs"),
            root.join("tests/server_restore_test.rs"),
        ];
        let mut hits = Vec::new();
        for f in files.iter().filter(|f| **f != me && !pre_0033.contains(f)) {
            let text = std::fs::read_to_string(f).unwrap();
            if text.contains("waived_paragraph_19_since") {
                hits.push(format!("{}: waived_paragraph_19_since", f.display()));
            }
            // SQL-Texte: Abschnitte zwischen Anführungszeichen.
            for lit in text.split('"').skip(1).step_by(2) {
                if lit.contains("seller_profile") && lit.contains("is_kleinunternehmer") {
                    hits.push(format!("{}: {}", f.display(), lit.trim()));
                }
            }
        }
        assert!(
            hits.is_empty(),
            "Lesestellen der Spiegelspalten (nur db::repo::tax_status darf sie schreiben):\n{}",
            hits.join("\n")
        );
    }
}
