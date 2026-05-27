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

use chrono::{Datelike, NaiveDate};
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
pub async fn accrue_for_year(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    fiscal_year: i64,
    today: NaiveDate,
) -> Result<AccrueReport> {
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
        backup::auto_backup_if_unlocked(pool, paths, session, "depreciation.lock")
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
pub async fn reset_asset(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    asset_id: &str,
) -> Result<AssetRow> {
    let asset = assets::get(pool, asset_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {asset_id}")))?;

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
    let removed_json = removed
        .iter()
        .map(|e| {
            format!(
                r#"{{"fiscal_year":{},"amount":{}}}"#,
                e.fiscal_year, e.depreciation_amount_cents
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    audit_log::append(
        pool,
        "depreciation.reset",
        "asset",
        asset_id,
        Some(&format!(
            r#"{{"asset_number":"{}","removed":[{}],"book_value_restored":{}}}"#,
            esc(&asset.asset_number),
            removed_json,
            book_value
        )),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, paths, session, "depreciation.reset")
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

/// Holt für eine Anlage alle noch nicht gebuchten Jahre vom Anschaffungsjahr bis
/// `target_year` nach.
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

    let method = DepreciationMethod::from_db(&asset.depreciation_method).ok_or_else(|| {
        Error::Domain(format!(
            "Anlage {} hat ungültige AfA-Methode '{}'",
            asset.asset_number, asset.depreciation_method
        ))
    })?;
    let acquisition_date = parse_date(&asset.acquisition_date)?;
    let da = DepreciationAsset {
        depreciation_method: method,
        acquisition_date,
        acquisition_cost_cents: asset.acquisition_cost_cents,
        business_share_percent: asset.business_share_percent,
        useful_life_years: asset.useful_life_years,
    };

    // Restbuchwert, der in das nächste zu buchende Jahr geht. Da Buchungen
    // sequenziell + lückenlos erfolgen, spiegelt der gespeicherte Restbuchwert
    // den Stand nach dem zuletzt gebuchten Jahr.
    let mut running = asset.book_value_cents;

    for year in asset.acquisition_fiscal_year..=target_year {
        if running <= 0 {
            break;
        }
        // Bereits gebucht? → überspringen (idempotent), Restbuchwert bleibt.
        if depreciation_repo::get_for_asset_year(pool, &asset.id, year)
            .await?
            .is_some()
        {
            continue;
        }

        let calc = compute_yearly(&da, year as i32, running);
        if calc.is_noop() {
            continue;
        }

        depreciation_repo::book_entry(pool, &asset.id, year, &calc).await?;
        running = calc.book_value_after_cents;

        // Restbuchwert fortschreiben. KEIN Lock — die Festschreibung erfolgt erst
        // zum GJ-Abschluss (Block 15), bis dahin ist die AfA korrigierbar.
        assets::set_book_value(pool, &asset.id, running, Some(year)).await?;

        audit_log::append(
            pool,
            "depreciation.accrue",
            "asset",
            &asset.id,
            Some(&format!(
                r#"{{"asset_number":"{}","fiscal_year":{},"amount":{},"book_value_after":{},"full_writeoff":{}}}"#,
                esc(&asset.asset_number),
                year,
                calc.depreciation_amount_cents,
                calc.book_value_after_cents,
                calc.is_full_writeoff
            )),
        )
        .await?;

        result.entries += 1;
        result.total_cents += calc.depreciation_amount_cents;
    }

    Ok(result)
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("Ungültiges Datum '{s}': {e}")))
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
