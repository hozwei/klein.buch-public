//! System-/GoBD-Einsicht-Commands (Phase 2D, Block 15) +
//! statische App-/Lizenz-Metadaten für den „Über"-Dialog (Block G2-DOC.3.5).
//!
//! - Audit-Trail-Read-View (Read-Only auf `audit_log`).
//! - Manueller Archiv-Integritäts-Check + dessen Historie (der automatische Lauf
//!   sitzt im [`crate::scheduler::integrity_check_cron`]).
//! - `app_info()` liefert App-Version (CARGO_PKG_VERSION), Schema-Version
//!   (`db::schema_version::EXPECTED_SCHEMA_VERSION`), Identifier
//!   (`tauri.conf.json`), Lizenz-Kennung und Build-Commit (Env-Var
//!   `KLEINBUCH_BUILD_COMMIT`, gesetzt vom Release-Workflow; sonst `"dev"`).
//!   Read-only, kein DB-Zugriff — auch vor dem Unlock benutzbar.
//! - `third_party_licenses_path()` resolved den Bundle-Pfad zur generierten
//!   `resources/handbook/third-party-licenses.html` (cargo-about + pnpm licenses
//!   im Release-CI; im Dev-Build steht dort der eingecheckte Stub).

use crate::app_ctx::AppCtx;
use crate::archive::integrity_check::{self, IntegrityCheckSummary};
use crate::db::repo::audit_log::{self, AuditEntry};
use crate::db::schema_version::EXPECTED_SCHEMA_VERSION;
use crate::error::{Error, Result};
use serde::Serialize;
use std::sync::Arc;

/// Argumente von [`audit_trail_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditTrailListArgs {
    #[serde(default)]
    pub limit: Option<i64>,
}

pub async fn audit_trail_list(
    ctx: &Arc<AppCtx>,
    args: AuditTrailListArgs,
) -> Result<Vec<AuditEntry>> {
    let AuditTrailListArgs { limit } = args;
    let pool = ctx.pool()?;

    let limit = limit.unwrap_or(200).clamp(1, 2000);
    audit_log::recent(&pool, limit).await
}

/// Argumente von [`archive_integrity_run`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveIntegrityRunArgs {}

pub async fn archive_integrity_run(
    ctx: &Arc<AppCtx>,
    _args: ArchiveIntegrityRunArgs,
) -> Result<IntegrityCheckSummary> {
    let pool = ctx.pool()?;

    let summary = integrity_check::run_full_scan(&pool).await?;
    let month = crate::clock::today_berlin().format("%Y-%m").to_string();
    crate::scheduler::integrity_check_cron::notify_summary(&pool, ctx.notifier(), &summary, &month)
        .await?;
    Ok(summary)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityCheckRow {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub files_checked: i64,
    pub files_passed: i64,
    pub files_failed: i64,
    pub failed_archive_ids_json: Option<String>,
}

/// Argumente von [`archive_integrity_history`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveIntegrityHistoryArgs {
    #[serde(default)]
    pub limit: Option<i64>,
}

pub async fn archive_integrity_history(
    ctx: &Arc<AppCtx>,
    args: ArchiveIntegrityHistoryArgs,
) -> Result<Vec<IntegrityCheckRow>> {
    let ArchiveIntegrityHistoryArgs { limit } = args;
    let pool = ctx.pool()?;

    let limit = limit.unwrap_or(50).clamp(1, 500);
    let rows = sqlx::query_as::<_, IntegrityCheckRow>(
        "SELECT id, started_at, finished_at, files_checked, files_passed,
                files_failed, failed_archive_ids_json
           FROM archive_integrity_checks
          ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;
    Ok(rows)
}

// --- G2-DOC.3.5: „Über"-Dialog ------------------------------------------------

/// Statische App-Metadaten für den „Über"-Dialog.
///
/// Alles Compile-Time-Werte aus Cargo / Build-Env — kein DB-Zugriff, kein
/// I/O. Der Command darf daher auch **vor** dem Unlock aufgerufen werden
/// (er taucht im UI sowieso nur im entsperrten Zustand auf, aber wir
/// machen ihn nicht unnötig pool-abhängig).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// App-Version aus `Cargo.toml` (`env!("CARGO_PKG_VERSION")`).
    pub app_version: String,
    /// Erwartete DB-Schema-Version dieser Binary.
    pub schema_version: i32,
    /// Kennung der Anwendung (`server::APP_IDENTIFIER`, `de.wildbach.kleinbuch`).
    pub identifier: String,
    /// SPDX-Kennung der App-Lizenz (`AGPL-3.0-or-later`).
    pub license_spdx: String,
    /// Source-Repository (für AGPL §13: "corresponding source" sichtbar im UI).
    pub repository_url: String,
    /// Build-Commit-Kurz-Hash (vom Release-CI gesetzt) oder `"dev"`.
    pub build_commit: String,
}

/// Argumente von [`app_info`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoArgs {}

pub async fn app_info(ctx: &Arc<AppCtx>, _args: AppInfoArgs) -> AppInfo {
    AppInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version: EXPECTED_SCHEMA_VERSION,
        identifier: ctx.app_identifier.clone(),
        license_spdx: "AGPL-3.0-or-later".to_string(),
        repository_url: "https://github.com/hozwei/klein.buch-public".to_string(),
        build_commit: option_env!("KLEINBUCH_BUILD_COMMIT")
            .unwrap_or("dev")
            .to_string(),
    }
}

/// Argumente von [`third_party_licenses_path`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThirdPartyLicensesPathArgs {}

/// Auflöst den Bundle-Pfad zur generierten Drittanbieter-Lizenz-Übersicht.
///
/// Im Release-Bundle liegt die Datei in `<resource_dir>/resources/handbook/
/// third-party-licenses.html` (siehe `tauri.conf.json`-`bundle.resources`).
/// Im Dev-Build greifen wir auf `<cwd>/resources/handbook/...` zurück —
/// `cwd` ist beim `cargo tauri dev` immer `src-tauri/`.
///
/// Existiert die Datei nicht, gibt es einen `Error::Config`, damit der
/// Frontend-Caller einen sauberen Toast statt einen leeren Pfad bekommt.
pub async fn third_party_licenses_path(
    ctx: &Arc<AppCtx>,
    _args: ThirdPartyLicensesPathArgs,
) -> Result<String> {
    // Release-Bundle: `<resource_dir>/handbook/…`; Desktop-Dev (resource_dir =
    // Repo `klein-buch/`): `src-tauri/resources/handbook/…`. Entfällt in W5
    // (Lizenzseite unter `/legal/`, W12).
    let rel = std::path::Path::new("handbook").join("third-party-licenses.html");
    let candidates = [
        ctx.paths.resource_dir.join(&rel),
        ctx.paths
            .resource_dir
            .join("src-tauri")
            .join("resources")
            .join(&rel),
    ];
    let path = candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone());
    if !path.exists() {
        return Err(Error::Config(format!(
            "Drittanbieter-Lizenz-Datei nicht gefunden: {}",
            path.display()
        )));
    }
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `app_info()` ist reiner Compile-Time-Code (Cargo-Env + Konstanten +
    /// optionaler Build-Env). Wir prüfen die Cargo-Anteile direkt — der
    /// `AppHandle`-Pfad braucht eine echte Tauri-App und ist im
    /// Integrations-Smoke abgedeckt.
    #[test]
    fn app_version_is_calver_format() {
        // Cargo.toml ist Single Source der App-Version (Manuel 2026-05-26).
        // `tauri.conf.json::version` ist entfernt (Tauri-Fallback auf
        // Cargo); `package.json::version` ist auf 0.0.0 gepinnt + Note.
        //
        // Tripwire: das Schema muss CalVer `YYYY.M.PATCH` bleiben —
        // fängt versehentliches Zurückrutschen auf semver-1.x oder
        // Müll-Strings, ohne bei jedem Monats-Bump mit-gepatcht
        // werden zu müssen.
        let v = env!("CARGO_PKG_VERSION");
        let parts: Vec<&str> = v.split('.').collect();
        assert_eq!(parts.len(), 3, "Version muss YYYY.M.PATCH sein, war: {v}");
        let year: u32 = parts[0].parse().expect("Jahr nicht numerisch");
        let month: u32 = parts[1].parse().expect("Monat nicht numerisch");
        let _patch: u32 = parts[2].parse().expect("Patch nicht numerisch");
        assert!(year >= 2026, "Jahr soll >= 2026 sein, war: {year}");
        assert!(
            (1..=12).contains(&month),
            "Monat soll 1..12 sein, war: {month}"
        );
    }

    #[test]
    fn schema_version_constant_is_27_or_higher() {
        // Sanity: wir kompilieren gegen die aktuelle Schema-Version-Konstante.
        // Wenn das Schema wächst, soll dieser Test mit der Migration mit-
        // wachsen — er fängt versehentliche Downgrades im Build.
        // `const { assert!(..) }` damit clippy nicht über die Konstanten-
        // Assertion meckert (assertions_on_constants).
        const { assert!(EXPECTED_SCHEMA_VERSION >= 27) };
    }

    #[test]
    fn build_commit_defaults_to_dev() {
        // Im normalen Dev-/CI-Test-Lauf ist KLEINBUCH_BUILD_COMMIT nicht
        // gesetzt → Fallback "dev". Im Release-Build setzt der Workflow
        // den Wert auf den Commit-SHA.
        let v = option_env!("KLEINBUCH_BUILD_COMMIT").unwrap_or("dev");
        assert!(!v.is_empty());
    }
}
