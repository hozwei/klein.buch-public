//! AfA-Buchungs-Shell (Imperative Shell) — Block 12.
//!
//! Erzeugt für ein Geschäftsjahr die Jahres-AfA-Buchungen (`depreciation_entries`)
//! aller aktiven Anlagen und schreibt den Restbuchwert in `assets` fort. Die reine
//! Rechnung liegt in [`crate::domain::depreciation`]; hier die DB-/Backup-Orchestrierung.
//!
//! ## Trigger
//!
//! - **Manuell** (Block 12): „AfA für Jahr X jetzt buchen" → [`accrue_for_year`].
//!   Das ist der Erstanwendungs-Pfad (z. B. AfA 2026 nachbuchen).
//! - **Automatisch zur GJ-Wende** (01.01., PRD §6.18): der Scheduler-Cron
//!   `scheduler::depreciation_year_close` wird erst in **Block 15** verdrahtet —
//!   bewusste Scope-Grenze (vetobar). Diese Shell ist dann ohne Änderung nutzbar.
//!
//! ## Verhalten (mit Manuel-Defaults aus Block 10 konsistent)
//!
//! - **Catch-up:** Pro Anlage werden alle noch nicht gebuchten Jahre vom
//!   Anschaffungsjahr bis `fiscal_year` nachgeholt (sequenziell, Restbuchwert
//!   durchgereicht). So ist die AfA-Reihe auch dann konsistent, wenn ein Jahr
//!   übersprungen wurde.
//! - **Idempotenz:** Bereits gebuchte (Anlage, GJ) werden übersprungen
//!   (`UNIQUE` + Vor-Prüfung). Ein zweiter Lauf ändert nichts.
//! - **Festschreibung:** erfolgt NICHT beim Buchen, sondern zum **GJ-Abschluss**
//!   (Block 15) — die Unveränderbarkeit (§146 Abs. 4 AO/GoBD) greift erst mit der
//!   Periodenfestschreibung. Im offenen GJ bleibt die AfA über [`reset_asset`]
//!   korrigierbar (jede Korrektur revisionssicher im Audit-Log).
//! - **Unlock-Gate:** Jede Buchung ändert Daten und braucht ein Backup. Bei
//!   gesperrter Backup-Session bucht der Lauf nichts.
//! - **Ein Backup pro Lauf** statt je Buchung.
//!
//! Veräußerte Anlagen (`disposed=1`) werden ausgelassen — ihr Restbuchwert geht
//! über die Disposal-Gewinn/Verlust-Rechnung in die EÜR (Block 13), nicht über AfA.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::backup::{self, BackupSession};
use crate::config::Paths;
use crate::db::models::AssetRow;
use crate::db::repo::{assets, audit_log, depreciation as depreciation_repo};
use crate::domain::asset::{business_book_value_start_cents, DepreciationMethod};
use crate::domain::depreciation::{compute_yearly, DepreciationAsset};
use crate::error::{Error, Result};

/// Ergebnis eines AfA-Buchungslaufs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccrueReport {
    /// Lauf übersprungen, weil die Backup-Session gesperrt war.
    pub skipped_locked: bool,
    /// Ziel-Geschäftsjahr.
    pub fiscal_year: i64,
    /// Anzahl Anlagen, für die mindestens eine Buchung erzeugt wurde.
    pub processed_assets: usize,
    /// Gesamtzahl erzeugter AfA-Buchungen (inkl. nachgeholter Jahre).
    pub booked_entries: usize,
    /// Summe der gebuchten AfA in Cent (über alle erzeugten Buchungen).
    pub total_depreciation_cents: i64,
}

/// Bucht die AfA bis einschließlich `fiscal_year` für alle aktiven Anlagen.
/// `now` kommt aus der Uhr des Aufrufers (`AppCtx`/`JobCtx`, D-20): das
/// Berliner Datum für die Plausibilität, der Zeitpunkt für den Backup-Vermerk.
pub async fn accrue_for_year(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    fiscal_year: i64,
    now: DateTime<Utc>,
) -> Result<AccrueReport> {
    let today = crate::clock::berlin_date(now);
    let mut report = AccrueReport {
        fiscal_year,
        ..Default::default()
    };

    // Plausibilität: keine AfA für ein zukünftiges Geschäftsjahr buchen.
    if fiscal_year > today.year() as i64 {
        return Err(Error::Domain(format!(
            "AfA für {fiscal_year} kann nicht gebucht werden — das Geschäftsjahr liegt in der Zukunft (heute {})."
            , today.year()
        )));
    }

    if !session.is_unlocked() {
        report.skipped_locked = true;
        tracing::debug!("AfA-Lauf übersprungen: Backup-Session gesperrt");
        return Ok(report);
    }

    let candidates = assets::list_active_for_year(pool, fiscal_year).await?;

    for asset in &candidates {
        let booked = accrue_asset(pool, asset, fiscal_year).await?;
        if booked.entries > 0 {
            report.processed_assets += 1;
            report.booked_entries += booked.entries;
            report.total_depreciation_cents += booked.total_cents;
        }
    }

    if report.booked_entries > 0 {
        backup::request_auto_backup(pool, paths, session, "depreciation.lock", now)
            .await
            .ok();
    }
    Ok(report)
}

/// Setzt die noch nicht festgeschriebene (offenes GJ) AfA einer Anlage zurück:
/// löscht die ungelockten Buchungen, stellt den Restbuchwert wieder her und
/// protokolliert die gelöschten Werte revisionssicher (GoBD-Nachvollziehbarkeit).
/// Festgeschriebene (gelockte) Buchungen bleiben unangetastet. Danach ist die
/// Anlage wieder editierbar (sofern keine gelockten Buchungen mehr übrig sind).
/// `now` (Backup-Vermerk) aus der Uhr des Aufrufers (D-20).
pub async fn reset_asset(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    asset_id: &str,
    now: DateTime<Utc>,
) -> Result<AssetRow> {
    let asset = assets::get(pool, asset_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {asset_id}")))?;
    // Lauf F2c: Nach dem Abgang bleibt die AfA stehen — der AfA-Lauf bucht
    // veräußerte Anlagen nicht mehr, die AfA des Abgangsjahres fehlte sonst
    // endgültig in der EÜR (DB-Backstop
    // `trg_depreciation_entries_no_delete_disposed`).
    if asset.disposed == 1 {
        return Err(Error::Domain(format!(
            "Anlage {} ist veräußert — ihre Abschreibung lässt sich nicht mehr zurücksetzen.",
            asset.asset_number
        )));
    }

    let entries = depreciation_repo::list_for_asset(pool, asset_id).await?; // year asc
    if !entries.iter().any(|e| e.locked_at.is_none()) {
        return Err(Error::Domain(
            "Keine zurücksetzbare Abschreibung vorhanden (nichts gebucht oder bereits festgeschrieben)."
                .into(),
        ));
    }

    let removed = depreciation_repo::reset_unlocked_for_asset(pool, asset_id).await?;

    // Restbuchwert + zuletzt gebuchtes Jahr aus den verbleibenden (gelockten)
    // Buchungen ableiten; sonst zurück auf den betrieblichen Start-Restbuchwert.
    let remaining_locked: Vec<_> = entries.iter().filter(|e| e.locked_at.is_some()).collect();
    let (book_value, last_year) = match remaining_locked.last() {
        Some(e) => (e.book_value_after_cents, Some(e.fiscal_year)),
        None => (
            business_book_value_start_cents(
                asset.acquisition_cost_cents,
                asset.business_share_percent,
            ),
            None,
        ),
    };
    assets::set_book_value(pool, asset_id, book_value, last_year).await?;

    // Gelöschte Werte revisionssicher protokollieren (ursprünglicher Inhalt feststellbar).
    let removed_json: Vec<serde_json::Value> = removed
        .iter()
        .map(|e| {
            serde_json::json!({
                "fiscal_year": e.fiscal_year,
                "amount": e.depreciation_amount_cents,
            })
        })
        .collect();
    audit_log::append(
        pool,
        "depreciation.reset",
        "asset",
        asset_id,
        Some(serde_json::json!({
            "asset_number": &asset.asset_number,
            "removed": removed_json,
            "book_value_restored": book_value,
        })),
    )
    .await?;

    backup::request_auto_backup(pool, paths, session, "depreciation.reset", now)
        .await
        .ok();

    assets::get(pool, asset_id)
        .await?
        .ok_or_else(|| Error::Domain("reset: post-update SELECT leer".into()))
}

struct AssetAccrual {
    entries: usize,
    total_cents: i64,
}

/// Holt für **eine** Anlage die noch nicht gebuchte AfA bis einschließlich
/// `target_year` in der laufenden Schreib-Transaktion des Aufrufers nach
/// (abgeschlossene Jahre ausgenommen) und liefert die Zahl der neuen
/// Buchungen. Lauf F2c/F3b: `assets_dispose` ruft das vor der anteiligen AfA
/// des Abgangsjahres für die Vorjahre — in derselben Transaktion wie den
/// Abgang; danach bucht der AfA-Lauf die veräußerte Anlage nicht mehr.
pub(crate) async fn accrue_asset_through_in(
    conn: &mut sqlx::SqliteConnection,
    asset: &AssetRow,
    target_year: i64,
) -> Result<usize> {
    let da = depreciation_asset(asset)?;
    let mut entries = 0;
    for year in asset.acquisition_fiscal_year..=target_year {
        match accrue_year_in(conn, asset, &da, year).await? {
            YearOutcome::Exhausted => break,
            YearOutcome::Skipped => {}
            YearOutcome::Booked(_) => entries += 1,
        }
    }
    Ok(entries)
}

/// Ergebnis von [`accrue_year_in`] für ein Jahr.
enum YearOutcome {
    /// Restbuchwert 0 — keine weiteren Jahre.
    Exhausted,
    /// Jahr abgeschlossen, schon gebucht oder ohne AfA.
    Skipped,
    /// Gebucht (Betrag in Cent).
    Booked(i64),
}

/// AfA-Grundlage einer Anlage für die reine Rechnung.
fn depreciation_asset(asset: &AssetRow) -> Result<DepreciationAsset> {
    let method = DepreciationMethod::from_db(&asset.depreciation_method).ok_or_else(|| {
        Error::Domain(format!(
            "Anlage {} hat ungültige AfA-Methode '{}'",
            asset.asset_number, asset.depreciation_method
        ))
    })?;
    Ok(DepreciationAsset {
        depreciation_method: method,
        acquisition_date: parse_date(&asset.acquisition_date)?,
        acquisition_cost_cents: asset.acquisition_cost_cents,
        business_share_percent: asset.business_share_percent,
        useful_life_years: asset.useful_life_years,
    })
}

/// Bucht die AfA **eines** Jahres in einer laufenden Schreib-Transaktion
/// (Lauf F, D-03): Restbuchwert frisch aus der Zeile lesen, Eintrag anlegen,
/// Restbuchwert fortschreiben und auditieren — zusammen oder gar nicht. Ein
/// Abbruch dazwischen ließe sonst einen veralteten Restbuchwert stehen, aus
/// dem das Folgejahr eine zu hohe AfA berechnete.
async fn accrue_year_in(
    conn: &mut sqlx::SqliteConnection,
    asset: &AssetRow,
    da: &DepreciationAsset,
    year: i64,
) -> Result<YearOutcome> {
    let running: i64 = sqlx::query_scalar("SELECT book_value_cents FROM assets WHERE id = ?")
        .bind(&asset.id)
        .fetch_one(&mut *conn)
        .await?;
    if running <= 0 {
        return Ok(YearOutcome::Exhausted);
    }
    // Lauf F (R2-S2-GJLOCK): nie in ein abgeschlossenes Geschäftsjahr
    // buchen — geprüft in der Schreib-Transaktion (der Abschluss bucht die
    // AfA vor seinem Eintrag in `fiscal_year_locks`).
    let closed: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM fiscal_year_locks WHERE fiscal_year = ?")
            .bind(year)
            .fetch_optional(&mut *conn)
            .await?;
    if closed.is_some() {
        return Ok(YearOutcome::Skipped);
    }
    // Bereits gebucht? → überspringen (idempotent), Restbuchwert bleibt.
    let booked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM depreciation_entries WHERE asset_id = ? AND fiscal_year = ?",
    )
    .bind(&asset.id)
    .bind(year)
    .fetch_one(&mut *conn)
    .await?;
    if booked > 0 {
        return Ok(YearOutcome::Skipped);
    }

    let calc = compute_yearly(da, year as i32, running);
    if calc.is_noop() {
        return Ok(YearOutcome::Skipped);
    }

    depreciation_repo::book_entry_in(&mut *conn, &asset.id, year, &calc).await?;
    // Restbuchwert fortschreiben. KEIN Lock — die Festschreibung erfolgt erst
    // zum GJ-Abschluss (Block 15), bis dahin ist die AfA korrigierbar.
    assets::set_book_value_in(
        &mut *conn,
        &asset.id,
        calc.book_value_after_cents,
        Some(year),
    )
    .await?;
    audit_log::append_in(
        &mut *conn,
        "depreciation.accrue",
        "asset",
        &asset.id,
        Some(serde_json::json!({
            "asset_number": asset.asset_number,
            "fiscal_year": year,
            "amount": calc.depreciation_amount_cents,
            "book_value_after": calc.book_value_after_cents,
            "full_writeoff": calc.is_full_writeoff,
        })),
    )
    .await?;
    Ok(YearOutcome::Booked(calc.depreciation_amount_cents))
}

/// Holt für eine Anlage alle noch nicht gebuchten Jahre vom Anschaffungsjahr bis
/// `target_year` nach — je Jahr eine Schreib-Transaktion; die Schreibsperre
/// schließt zugleich parallele Läufe aus.
async fn accrue_asset(
    pool: &SqlitePool,
    asset: &AssetRow,
    target_year: i64,
) -> Result<AssetAccrual> {
    let mut result = AssetAccrual {
        entries: 0,
        total_cents: 0,
    };

    // Fertig abgeschrieben → nichts zu tun.
    if asset.book_value_cents <= 0 {
        return Ok(result);
    }

    let da = depreciation_asset(asset)?;
    for year in asset.acquisition_fiscal_year..=target_year {
        let mut tx = crate::db::begin_immediate(pool).await?;
        match accrue_year_in(&mut tx, asset, &da, year).await? {
            YearOutcome::Exhausted => break,
            YearOutcome::Skipped => continue,
            YearOutcome::Booked(amount) => {
                tx.commit().await?;
                result.entries += 1;
                result.total_cents += amount;
            }
        }
    }

    Ok(result)
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("Ungültiges Datum '{s}': {e}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
