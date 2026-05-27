//! Migrations-Export-Commands (Block 4).

use crate::config::Paths;
use crate::db::repo::audit_log;
use crate::error::Result;
use crate::migration_export::export::{self, ExportReport};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

/// Exportiert alle Daten als offenes ZIP an den gewählten Pfad.
/// `target_path` ist der vom Nutzer (File-Dialog) gewählte Ziel-Dateipfad.
///
/// R2-027: schreibt nach erfolgreichem Export einen `migration.export`-Audit-
/// Eintrag (DSGVO Art. 5(2) Rechenschaftspflicht + GoBD-Audit-Trail über
/// Daten-Auslieferungen, analog zu `dsgvo.export`).
#[tauri::command]
pub async fn migration_export_run(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    target_path: String,
) -> Result<ExportReport> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
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
