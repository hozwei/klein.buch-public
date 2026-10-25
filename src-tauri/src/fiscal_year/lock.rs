//! GJ-Abschluss / Festschreibung (Block 15).
//!
//! [`close_year`] schließt ein abgelaufenes Geschäftsjahr prüfungssicher ab:
//! 1. AfA des Jahres sicher buchen (idempotent, holt fehlende Jahre nach),
//! 2. AfA-Buchungen + betroffene Anlagen festschreiben (`locked_at`),
//! 3. EÜR-Eckwerte als Snapshot ins Festschreibungsprotokoll (`fiscal_year_locks`),
//! 4. Audit-Log-Eintrag `fiscal_year.close` mit Belegzahlen,
//! 5. Auto-Critical-Backup (§6.15).
//!
//! Lauf F (R2-S2-GJLOCK, W14-034): Der Abschluss nimmt die exklusive Sperre
//! [`guard::closing`] — er beginnt erst, wenn keine Buchung mehr läuft, und
//! solange er läuft, beginnt keine (jede Buchung hält ab ihrer Jahresprüfung
//! eine [`guard::BookingPermit`]). Schritte 2–4 laufen in **einer**
//! Schreib-Transaktion: Snapshot und Protokoll sehen denselben Stand, und ein
//! Abbruch lässt keine festgeschriebene AfA ohne Abschluss zurück. Offene
//! Entwürfe des Jahres blockieren den Abschluss nicht (sie sind nicht Teil der
//! EÜR und lassen sich weiter verwerfen); ihre Zahl steht in der Übersicht,
//! in der Antwort und im Audit.
//!
//! Der Abschluss ist **unumkehrbar** (kein Entsperren-UI; DB-Trigger verbieten
//! Update/Delete auf `fiscal_year_locks`). Ab dem Abschluss verhindert der
//! [`crate::fiscal_year::guard`] jede neue Buchung mit Datum im geschlossenen Jahr.

use crate::backup::{self, BackupSession};
use crate::config::Paths;
use crate::db::repo::{audit_log, euer as euer_repo};
use crate::error::{Error, Result};
use crate::euer::aggregate::aggregate;
use crate::fiscal_year::guard;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FiscalYearLock {
    pub fiscal_year: i64,
    pub closed_at: String,
    pub income_total_cents: i64,
    pub expense_total_cents: i64,
    pub afa_total_cents: i64,
    pub surplus_cents: i64,
    pub assets_locked: i64,
    pub depreciation_entries_locked: i64,
    pub app_version: String,
    pub schema_version: i64,
    pub notes: Option<String>,
    /// Offene Entwürfe des Jahres zum Zeitpunkt des Abschlusses — nur in der
    /// Antwort von [`close_year`], nicht gespeichert (Lauf F).
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_drafts: Option<OpenDrafts>,
}

/// Offene (nicht festgeschriebene, nicht verworfene) Entwürfe eines Jahres.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDrafts {
    /// Rechnungs- und Storno-Entwürfe (`voided_at IS NULL`).
    pub invoices: i64,
    /// Angebots-Entwürfe (`status = 'draft'`; zurückgezogene zählen nicht).
    pub quotes: i64,
}

impl OpenDrafts {
    pub fn total(&self) -> i64 {
        self.invoices + self.quotes
    }
}

/// Zählt die offenen Entwürfe des Geschäftsjahres `year`.
pub async fn open_drafts_in(conn: &mut SqliteConnection, year: i64) -> Result<OpenDrafts> {
    let invoices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices
          WHERE fiscal_year = ? AND locked_at IS NULL AND voided_at IS NULL",
    )
    .bind(year)
    .fetch_one(&mut *conn)
    .await?;
    let quotes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM quotes
          WHERE fiscal_year = ? AND locked_at IS NULL AND status = 'draft'",
    )
    .bind(year)
    .fetch_one(&mut *conn)
    .await?;
    Ok(OpenDrafts { invoices, quotes })
}

/// Wie [`open_drafts_in`], über den Pool.
pub async fn open_drafts(pool: &SqlitePool, year: i64) -> Result<OpenDrafts> {
    let mut conn = pool.acquire().await?;
    open_drafts_in(&mut conn, year).await
}

/// Belegzahlen des Jahres fürs Audit (nur Zählwerte, keine Inhalte).
async fn document_counts_in(conn: &mut SqliteConnection, year: i64) -> Result<serde_json::Value> {
    let count = |sql: &'static str| sqlx::query_scalar::<_, i64>(sql).bind(year);
    let invoices = count(
        "SELECT COUNT(*) FROM invoices WHERE direction = 'issued' AND fiscal_year = ?
            AND locked_at IS NOT NULL AND is_storno_for IS NULL",
    )
    .fetch_one(&mut *conn)
    .await?;
    let stornos = count(
        "SELECT COUNT(*) FROM invoices WHERE fiscal_year = ?
            AND locked_at IS NOT NULL AND is_storno_for IS NOT NULL",
    )
    .fetch_one(&mut *conn)
    .await?;
    let expenses =
        count("SELECT COUNT(*) FROM expenses WHERE fiscal_year = ? AND locked_at IS NOT NULL")
            .fetch_one(&mut *conn)
            .await?;
    let private_movements = count(
        "SELECT COUNT(*) FROM private_movements WHERE fiscal_year = ? AND locked_at IS NOT NULL",
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(serde_json::json!({
        "invoices": invoices,
        "stornos": stornos,
        "expenses": expenses,
        "private_movements": private_movements,
    }))
}

/// Schließt das Geschäftsjahr `year` ab. `today` wird injiziert (Testbarkeit).
pub async fn close_year(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    year: i64,
    today: NaiveDate,
) -> Result<FiscalYearLock> {
    let current = today.year() as i64;

    // 1. Plausibilität — nur abgelaufene Geschäftsjahre schließen.
    if year >= current {
        return Err(Error::Domain(format!(
            "Das Geschäftsjahr {year} kann erst nach seinem Ablauf abgeschlossen werden \
             (aktuell läuft das Geschäftsjahr {current})."
        )));
    }
    // 2. Doppel-Abschluss verhindern (früh; in der Transaktion erneut geprüft).
    if guard::is_closed(pool, year).await? {
        return Err(already_closed(year));
    }
    // 3. Backup-Session muss entsperrt sein — der Abschluss bucht AfA und erzeugt
    //    eine Sicherung; beides braucht die Passphrase.
    if !session.is_unlocked() {
        return Err(Error::Domain(
            "Bitte zuerst das Backup entsperren (Passphrase eingeben). \
             Der Geschäftsjahres-Abschluss erzeugt eine Sicherung."
                .into(),
        ));
    }

    // Lauf F: exklusiv gegen alle Buchungen (wartet auf laufende).
    let closing = guard::closing().await;

    // 4. AfA des Jahres sicher buchen (idempotent; bereits gebuchte bleiben).
    crate::depreciation::accrue_yearly::accrue_for_year(pool, paths, session, year, today).await?;

    // 5.–8. Festschreiben, Snapshot, Protokoll und Audit in einer Transaktion.
    let mut tx = crate::db::begin_immediate(pool).await?;
    let closed: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM fiscal_year_locks WHERE fiscal_year = ?")
            .bind(year)
            .fetch_optional(&mut *tx)
            .await?;
    if closed.is_some() {
        return Err(already_closed(year));
    }
    let dep_locked = sqlx::query(
        "UPDATE depreciation_entries
            SET locked_at = datetime('now','utc')
          WHERE fiscal_year = ? AND locked_at IS NULL",
    )
    .bind(year)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    // Betroffene (noch offene) Anlagen: alle bis zum Jahr angeschafften — auch
    // veräußerte (Lauf F2c). Eine im Jahr (oder später) veräußerte Anlage steht
    // mit AfA und Abgang im Anlageverzeichnis des Jahres; ohne `locked_at`
    // schützte `trg_assets_immutable` ihre Stammdaten nie.
    let assets_locked = sqlx::query(
        "UPDATE assets SET locked_at = datetime('now','utc')
          WHERE acquisition_fiscal_year <= ? AND locked_at IS NULL",
    )
    .bind(year)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let inputs = euer_repo::load_inputs_in(&mut tx).await?;
    let report = aggregate(year as i32, &inputs);
    let drafts = open_drafts_in(&mut tx, year).await?;
    let documents = document_counts_in(&mut tx, year).await?;

    let app_version = env!("CARGO_PKG_VERSION");
    sqlx::query(
        "INSERT INTO fiscal_year_locks
            (fiscal_year, income_total_cents, expense_total_cents, afa_total_cents,
             surplus_cents, assets_locked, depreciation_entries_locked,
             app_version, schema_version)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(year)
    .bind(report.total_income_cents)
    .bind(report.total_expenses_cents)
    .bind(report.depreciation_total_cents)
    .bind(report.surplus_cents)
    .bind(assets_locked as i64)
    .bind(dep_locked as i64)
    .bind(app_version)
    .bind(crate::db::schema_version::EXPECTED_SCHEMA_VERSION as i64)
    .execute(&mut *tx)
    .await?;

    audit_log::append_in(
        &mut tx,
        "fiscal_year.close",
        "fiscal_year",
        &year.to_string(),
        Some(serde_json::json!({
            "income": report.total_income_cents,
            "expenses": report.total_expenses_cents,
            "afa": report.depreciation_total_cents,
            "surplus": report.surplus_cents,
            "assets_locked": assets_locked,
            "depreciation_locked": dep_locked,
            "documents": documents,
            "open_drafts": { "invoices": drafts.invoices, "quotes": drafts.quotes },
        })),
    )
    .await?;
    tx.commit().await?;
    drop(closing);

    // 9. Auto-Critical-Backup (§6.15: nach fiscal_year.close).
    backup::auto_backup_if_unlocked(pool, paths, session, "fiscal_year.close")
        .await
        .ok();

    // 10. Protokoll zurückgeben.
    let mut lock = get(pool, year)
        .await?
        .ok_or_else(|| Error::Domain("Abschluss-Protokoll konnte nicht gelesen werden.".into()))?;
    lock.open_drafts = Some(drafts);
    Ok(lock)
}

fn already_closed(year: i64) -> Error {
    Error::Domain(format!(
        "Das Geschäftsjahr {year} ist bereits abgeschlossen."
    ))
}

pub async fn get(pool: &SqlitePool, year: i64) -> Result<Option<FiscalYearLock>> {
    let row = sqlx::query_as::<_, FiscalYearLock>(
        "SELECT * FROM fiscal_year_locks WHERE fiscal_year = ?",
    )
    .bind(year)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_closed(pool: &SqlitePool) -> Result<Vec<FiscalYearLock>> {
    let rows = sqlx::query_as::<_, FiscalYearLock>(
        "SELECT * FROM fiscal_year_locks ORDER BY fiscal_year DESC",
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
