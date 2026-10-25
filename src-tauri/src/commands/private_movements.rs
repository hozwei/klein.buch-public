//! Commands für Privatentnahmen / -einlagen (Block 9).
//!
//! Privatbewegungen sind EÜR-neutral (siehe domain::private_movement) und dienen
//! nur der Vollständigkeit der Kasse. Erfassung schreibt sofort fest (Lock-Event
//! → Backup-Hook). Kein Storno/Edit — Korrektur via Gegenbewegung (append-only).

use crate::app_ctx::AppCtx;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::archive::{self, ArchiveKind};
use crate::backup;
use crate::commands::attachments::sanitize_filename;
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
    /// Optionaler Beleg (z. B. Quittung) aus `POST /api/upload/beleg` (Plan W7).
    #[serde(default)]
    pub receipt_upload_id: Option<String>,
    /// Vom Command aus dem Upload gelesen, nie vom Client.
    #[serde(skip)]
    pub receipt: Option<crate::uploads::UploadedFile>,
}

/// Argumente von [`private_movements_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementsListArgs {
    #[serde(default)]
    pub filter: Option<private_movements::ListFilter>,
}

pub async fn private_movements_list(
    ctx: &Arc<AppCtx>,
    args: PrivateMovementsListArgs,
) -> Result<Vec<PrivateMovementListItem>> {
    let PrivateMovementsListArgs { filter } = args;
    let pool = ctx.pool()?;

    private_movements::list(&pool, &filter.unwrap_or_default()).await
}

/// Argumente von [`private_movements_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementsGetArgs {
    pub id: String,
}

pub async fn private_movements_get(
    ctx: &Arc<AppCtx>,
    args: PrivateMovementsGetArgs,
) -> Result<Option<PrivateMovementRow>> {
    let PrivateMovementsGetArgs { id } = args;
    let pool = ctx.pool()?;

    private_movements::get(&pool, &id).await
}

/// Argumente von [`private_movements_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementsCreateArgs {
    pub args: PrivateMovementCreateArgs,
}

pub async fn private_movements_create(
    ctx: &Arc<AppCtx>,
    args: PrivateMovementsCreateArgs,
) -> Result<PrivateMovementRow> {
    let PrivateMovementsCreateArgs { mut args } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let upload_id = args.receipt_upload_id.take().filter(|id| !id.is_empty());
    if let Some(id) = upload_id.as_deref() {
        args.receipt = Some(
            ctx.uploads
                .read(id, &[crate::uploads::UploadKind::Receipt])?,
        );
    }
    let paths = ctx.paths.clone();
    let row = create_core(&pool, &paths, session, args).await?;
    if let Some(id) = upload_id.as_deref() {
        ctx.uploads.consume(id);
    }
    Ok(row)
}

/// Testbarer Kern von [`private_movements_create`] (ohne `AppCtx`).
pub async fn create_core(
    pool: &SqlitePool,
    paths: &Paths,
    session: &backup::BackupSession,
    args: PrivateMovementCreateArgs,
) -> Result<PrivateMovementRow> {
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

    // GJ = Jahr des Belegdatums (Plan W1.2/W9, nie vom Client).
    let fiscal_year = i64::from(crate::domain::fiscal_year::fiscal_year_for(
        &input.movement_date,
    ));
    // GJ-Festschreibung (Block 15): keine neue Privatbewegung mit Datum in einem
    // abgeschlossenen Geschäftsjahr.
    let _fy = crate::fiscal_year::guard::ensure_year_open(pool, fiscal_year).await?;
    let movement_number =
        numbering::next_number(pool, DocType::PrivateMovement, fiscal_year as i32).await?;

    let receipt_archive_id = match args.receipt {
        Some(file) if !file.bytes.is_empty() => {
            let sanitized = sanitize_filename(&file.file_name);
            let archive_name = format!("{movement_number}-{sanitized}");
            let stored = archive::store_bytes(
                pool,
                &paths.archive_dir,
                fiscal_year as i32,
                ArchiveKind::Attachment,
                &archive_name,
                // Typ aus der Inhaltsprüfung, nicht aus der Endung (Plan W7).
                file.mime,
                &file.bytes,
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
        Some(serde_json::json!({ "number": row.movement_number, "type": row.movement_type, "amount": row.amount_cents })),
    )
    .await?;

    backup::auto_backup_if_unlocked(pool, paths, session, "private_movement.lock")
        .await
        .ok();

    Ok(row)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
