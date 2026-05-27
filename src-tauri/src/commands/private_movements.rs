//! Tauri-Commands für Privatentnahmen / -einlagen (Block 9).
//!
//! Privatbewegungen sind EÜR-neutral (siehe domain::private_movement) und dienen
//! nur der Vollständigkeit der Kasse. Erfassung schreibt sofort fest (Lock-Event
//! → Backup-Hook). Kein Storno/Edit — Korrektur via Gegenbewegung (append-only).

use chrono::Datelike;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::archive::{self, ArchiveKind};
use crate::backup;
use crate::commands::attachments::{guess_mime, sanitize_filename};
use crate::config::Paths;
use crate::db::models::{PrivateMovementListItem, PrivateMovementRow};
use crate::db::numbering;
use crate::db::repo::{audit_log, private_movements};
use crate::domain::numbering::DocType;
use crate::domain::private_movement::{self, PrivateMovementInput};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementCreateArgs {
    pub input: PrivateMovementInput,
    /// Optional — sonst aus dem Bewegungs-Datum (Kalenderjahr) abgeleitet.
    pub fiscal_year: Option<i64>,
    /// Optionaler Beleg (z. B. Quittung). `None` → ohne Beleg.
    pub receipt_bytes: Option<Vec<u8>>,
    pub receipt_filename: Option<String>,
}

#[tauri::command]
pub async fn private_movements_list(
    pool: State<'_, SqlitePool>,
    filter: Option<private_movements::ListFilter>,
) -> Result<Vec<PrivateMovementListItem>> {
    private_movements::list(pool.inner(), &filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn private_movements_get(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<PrivateMovementRow>> {
    private_movements::get(pool.inner(), &id).await
}

#[tauri::command]
pub async fn private_movements_create(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    args: PrivateMovementCreateArgs,
) -> Result<PrivateMovementRow> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    let input = args.input;

    if let Err(errs) = private_movement::validate_private_movement(&input) {
        return Err(Error::Domain(format!(
            "Privatbewegung kann nicht erfasst werden: {}",
            errs.iter()
                .map(private_movement::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    let fiscal_year = args
        .fiscal_year
        .unwrap_or_else(|| input.movement_date.year() as i64);
    // GJ-Festschreibung (Block 15): keine neue Privatbewegung mit Datum in einem
    // abgeschlossenen Geschäftsjahr.
    crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;
    let movement_number =
        numbering::next_number(pool, DocType::PrivateMovement, fiscal_year as i32).await?;

    let receipt_archive_id = match args.receipt_bytes {
        Some(bytes) if !bytes.is_empty() => {
            let sanitized = sanitize_filename(args.receipt_filename.as_deref().unwrap_or("beleg"));
            let archive_name = format!("{movement_number}-{sanitized}");
            let mime = guess_mime(&sanitized);
            let stored = archive::store_bytes(
                pool,
                &paths.archive_dir,
                fiscal_year as i32,
                ArchiveKind::Attachment,
                &archive_name,
                mime,
                &bytes,
            )
            .await?;
            Some(stored.archive_id)
        }
        _ => None,
    };

    let row = private_movements::create(
        pool,
        &input,
        &movement_number,
        fiscal_year,
        receipt_archive_id.as_deref(),
    )
    .await?;

    audit_log::append(
        pool,
        "private_movement.create",
        "private_movement",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","type":"{}","amount":{}}}"#,
            row.movement_number, row.movement_type, row.amount_cents
        )),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, &paths, session.inner(), "private_movement.lock")
        .await
        .ok();

    Ok(row)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
