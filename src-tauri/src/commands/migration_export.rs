//! Migrations-Export-Commands (Block 4).

use crate::app_ctx::AppCtx;
use crate::db::repo::audit_log;
use crate::error::Result;
use crate::migration_export::export::{self, ExportReport};
use std::sync::Arc;

/// Argumente von [`migration_export_run`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationExportRunArgs {
    pub target_path: String,
}

/// Exportiert alle Daten als offenes ZIP an den gewählten Pfad.
/// `target_path` ist der vom Nutzer (File-Dialog) gewählte Ziel-Dateipfad.
///
/// R2-027: schreibt nach erfolgreichem Export einen `migration.export`-Audit-
/// Eintrag (DSGVO Art. 5(2) Rechenschaftspflicht + GoBD-Audit-Trail über
/// Daten-Auslieferungen, analog zu `dsgvo.export`).
pub async fn migration_export_run(
    ctx: &Arc<AppCtx>,
    args: MigrationExportRunArgs,
) -> Result<ExportReport> {
    let MigrationExportRunArgs { target_path } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let paths = ctx.paths.clone();
    let report = export::export_all(pool, &paths, std::path::Path::new(&target_path)).await?;
    audit_log::append(
        pool,
        "migration.export",
        "migration",
        &report.zip_path,
        Some(&format!(
            r#"{{"tables":{},"rows":{},"archiveFiles":{}}}"#,
            report.table_count, report.total_rows, report.archive_file_count
        )),
    )
    .await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
