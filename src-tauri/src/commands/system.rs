//! System-/GoBD-Einsicht-Commands (Phase 2D, Block 15) +
//! statische App-/Lizenz-Metadaten für den „Über"-Dialog (Block G2-DOC.3.5).
//!
//! - Audit-Trail-Read-View (Read-Only auf `audit_log`).
//! - Manueller Archiv-Integritäts-Check + dessen Historie (der automatische Lauf
//!   sitzt im [`crate::scheduler::integrity_check_cron`]).
//! - `app_info()` liefert App-Version (CARGO_PKG_VERSION), Schema-Version
//!   (`db::schema_version::EXPECTED_SCHEMA_VERSION`), Identifier, Lizenz-Kennung
//!   und die Quellangaben aus [`crate::build_info`] (Commit, Tag, Quell-URL,
//!   Link auf den exakten Stand — dieselbe Quelle wie `GET /api/public/info`,
//!   Plan W12). Read-only, kein DB-Zugriff.
//! - `third_party_licenses_path()` ist im Server entfallen (Route::Dropped);
//!   die Lizenzseiten liefert `GET /legal/` aus (Plan W12).
//! - `scheduler_hold_status` / `scheduler_release_hold`: Pause der automatischen
//!   Belegerzeugung anzeigen bzw. (mit Re-Auth) aufheben (Lauf F, ADR 0040).

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

/// Argumente von [`scheduler_hold_status`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerHoldStatusArgs {}

/// Besteht eine Pause der automatischen Belegerzeugung (Uhrsprung, Restore)?
pub async fn scheduler_hold_status(
    ctx: &Arc<AppCtx>,
    _args: SchedulerHoldStatusArgs,
) -> Result<crate::scheduler::hold::HoldStatus> {
    let pool = ctx.pool()?;
    crate::scheduler::hold::status(&pool).await
}

/// Argumente von [`scheduler_release_hold`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerReleaseHoldArgs {}

/// Hebt die Pause auf (Re-Auth, Audit `scheduler.hold_released`). Der nächste
/// Tick arbeitet wieder; Rückstände folgen den Nachhol-Regeln (ADR 0040).
pub async fn scheduler_release_hold(
    ctx: &Arc<AppCtx>,
    _args: SchedulerReleaseHoldArgs,
) -> Result<crate::scheduler::hold::HoldStatus> {
    let pool = ctx.pool()?;
    let now = ctx.clock().now_utc();
    if let Some(previous) = crate::scheduler::hold::release(&pool, now).await? {
        tracing::info!(
            "Pause der automatischen Belegerzeugung aufgehoben (Grund war {})",
            previous.reason
        );
    }
    crate::scheduler::hold::status(&pool).await
}

/// Nummernkreis für die Anzeige (Lauf F, D-24).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingCounterView {
    pub doc_type: String,
    pub label: String,
    pub fiscal_year: i64,
    pub last_value: i64,
    /// Zuletzt vergebene Nummer (z. B. `RE-2026-0012`), `None` bei Stand 0.
    pub last_number: Option<String>,
}

/// Argumente von [`numbering_counters_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingCountersListArgs {}

/// Stände aller Nummernkreise (zuletzt vergebene Nummer je Art und Jahr).
pub async fn numbering_counters_list(
    ctx: &Arc<AppCtx>,
    _args: NumberingCountersListArgs,
) -> Result<Vec<NumberingCounterView>> {
    let pool = ctx.pool()?;
    Ok(crate::db::numbering::snapshot(&pool)
        .await?
        .into_iter()
        .map(|c| {
            let last_number = doc_type_of(&c.doc_type)
                .filter(|_| c.last_value > 0)
                .and_then(|dt| {
                    let year = i32::try_from(c.fiscal_year).ok()?;
                    let seq = u32::try_from(c.last_value).ok()?;
                    Some(crate::domain::numbering::format(dt, year, seq))
                });
            NumberingCounterView {
                label: crate::db::label(&c.doc_type).to_string(),
                doc_type: c.doc_type,
                fiscal_year: c.fiscal_year,
                last_value: c.last_value,
                last_number,
            }
        })
        .collect())
}

fn doc_type_of(slug: &str) -> Option<crate::domain::numbering::DocType> {
    use crate::domain::numbering::DocType;
    [
        DocType::Quote,
        DocType::Invoice,
        DocType::StornoInvoice,
        DocType::Expense,
        DocType::PrivateMovement,
        DocType::Asset,
    ]
    .into_iter()
    .find(|d| d.db_slug() == slug)
}

/// Höchster zulässiger Zählerstand beim Anheben (Schutz vor Tippfehlern).
pub const NUMBERING_RAISE_MAX: i64 = 999_999;

/// Argumente von [`numbering_raise`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingRaiseArgs {
    /// Slug (`invoice`, `storno_invoice`, `expense`, `quote`, `private_movement`, `asset`).
    pub doc_type: String,
    pub fiscal_year: i64,
    /// Zuletzt vergebene Nummer (die nächste ist `lastValue + 1`).
    pub last_value: i64,
    /// Begründung (Pflicht, steht im Audit-Log).
    pub reason: String,
}

/// Hebt einen Nummernkreis an (Lauf F, D-24; Re-Auth): nur aufwärts, Audit
/// `numbering.raised` mit altem und neuem Stand und Begründung. Für den Fall,
/// dass nach dem Stand eines eingespielten Backups außerhalb dieser Instanz
/// weitere Nummern vergeben wurden (Disaster Recovery) — bereits vergebene
/// Nummern dürfen nicht erneut vergeben werden (§14 Abs. 4 Nr. 4 UStG); die
/// Lücke ist dokumentiert. Belege werden nicht verändert.
pub async fn numbering_raise(
    ctx: &Arc<AppCtx>,
    args: NumberingRaiseArgs,
) -> Result<Vec<NumberingCounterView>> {
    let NumberingRaiseArgs {
        doc_type,
        fiscal_year,
        last_value,
        reason,
    } = args;
    let pool = ctx.pool()?;
    if doc_type_of(&doc_type).is_none() {
        return Err(Error::BadRequest(format!(
            "Unbekannter Nummernkreis: {doc_type}"
        )));
    }
    if !(2000..=2100).contains(&fiscal_year) {
        return Err(Error::BadRequest(format!(
            "Ungültiges Geschäftsjahr: {fiscal_year}"
        )));
    }
    if !(1..=NUMBERING_RAISE_MAX).contains(&last_value) {
        return Err(Error::BadRequest(format!(
            "Zählerstand muss zwischen 1 und {NUMBERING_RAISE_MAX} liegen."
        )));
    }
    let reason: String = reason
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(500)
        .collect();
    if reason.chars().count() < 10 {
        return Err(Error::BadRequest(
            "Bitte eine Begründung angeben (mindestens 10 Zeichen) — sie steht im Audit-Log."
                .into(),
        ));
    }
    let mut tx = crate::db::begin_immediate(&pool).await?;
    let Some(raised) =
        crate::db::numbering::raise_in(&mut tx, &doc_type, fiscal_year, last_value).await?
    else {
        return Err(Error::Domain(
            "Der Nummernkreis steht schon auf diesem oder einem höheren Wert — absenken ist \
             nicht zulässig (bereits vergebene Nummern dürfen nicht erneut vergeben werden)."
                .into(),
        ));
    };
    audit_log::append_in(
        &mut tx,
        "numbering.raised",
        "numbering",
        &format!("{doc_type}:{fiscal_year}"),
        Some(serde_json::json!({
            "doc_type": raised.doc_type,
            "fiscal_year": raised.fiscal_year,
            "from": raised.from,
            "to": raised.to,
            "reason": reason,
        })),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        "Nummernkreis {doc_type} {fiscal_year} angehoben: {} → {}",
        raised.from,
        raised.to
    );
    numbering_counters_list(ctx, NumberingCountersListArgs {}).await
}

/// Argumente von [`numbering_restored_drafts_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingRestoredDraftsListArgs {}

/// Wiederhergestellte Entwürfe, deren Nummer vor dem Restore schon
/// festgeschrieben war (F1d) — sie lassen sich erst nach Bestätigung
/// festschreiben.
pub async fn numbering_restored_drafts_list(
    ctx: &Arc<AppCtx>,
    _args: NumberingRestoredDraftsListArgs,
) -> Result<Vec<crate::backup::after_restore::Conflict>> {
    let pool = ctx.pool()?;
    crate::backup::after_restore::list_conflicts(&pool).await
}

/// Argumente von [`numbering_restored_draft_confirm`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingRestoredDraftConfirmArgs {
    /// `invoice` (auch Storno) oder `quote`.
    pub kind: String,
    pub id: String,
    /// Begründung (Pflicht, steht im Audit-Log), z. B. „an das versandte
    /// Original laut Mail-Ausgang angepasst“.
    pub reason: String,
}

/// Bestätigt (Re-Auth), dass ein vorgemerkter Entwurf dem versandten Original
/// mit derselben Nummer entspricht (F1d, ADR 0040). Audit
/// `numbering.restored_draft_confirmed` mit Begründung; danach lässt sich der
/// Entwurf wie gewohnt festschreiben. Der Beleg selbst wird nicht verändert.
pub async fn numbering_restored_draft_confirm(
    ctx: &Arc<AppCtx>,
    args: NumberingRestoredDraftConfirmArgs,
) -> Result<Vec<crate::backup::after_restore::Conflict>> {
    let NumberingRestoredDraftConfirmArgs { kind, id, reason } = args;
    let pool = ctx.pool()?;
    if !matches!(kind.as_str(), "invoice" | "quote") {
        return Err(Error::BadRequest(format!("Unbekannte Belegart: {kind}")));
    }
    let reason: String = reason
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(500)
        .collect();
    if reason.chars().count() < 10 {
        return Err(Error::BadRequest(
            "Bitte eine Begründung angeben (mindestens 10 Zeichen) — sie steht im Audit-Log."
                .into(),
        ));
    }
    crate::backup::after_restore::confirm_conflict(&pool, &kind, &id, &reason).await?;
    crate::backup::after_restore::list_conflicts(&pool).await
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

    // D-07: nie parallel zum Monats-Cron (oder einem zweiten Klick).
    let _scan = integrity_check::try_scan_lock()?;
    let now = ctx.clock().now_utc();
    let summary = integrity_check::run_full_scan_at(&pool, now).await?;
    let month = crate::clock::berlin_date(now).format("%Y-%m").to_string();
    crate::scheduler::integrity_check_cron::notify_summary(&pool, &summary, &month).await?;
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
    /// Teilmenge von `files_failed`: fehlende Dateien. `None` = Lauf vor
    /// Migration 0032 (nicht erfasst).
    pub files_missing: Option<i64>,
    /// Vorhanden, aber nicht lesbar (kein Manipulationsbefund). `None` = vor 0032.
    pub files_unreadable: Option<i64>,
    pub missing_archive_ids_json: Option<String>,
    pub unreadable_archive_ids_json: Option<String>,
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
                files_failed, failed_archive_ids_json, files_missing, files_unreadable,
                missing_archive_ids_json, unreadable_archive_ids_json
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
/// Alles Compile-Time-Werte aus Cargo / Build-Env (plus der Laufzeit-Override
/// der Quell-URL) — kein DB-Zugriff. Seit Plan W5 braucht der Command eine
/// Sitzung; die Quellangaben gibt es ohne Sitzung über `/api/public/info`.
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
    /// Wirksame Quell-URL (AGPL §13; `KLEIN_BUCH_SOURCE_URL`, sonst Build-Wert).
    pub repository_url: String,
    /// Voller Commit-SHA des Builds; `None` im Entwicklungs-Build.
    pub build_commit: Option<String>,
    /// Release-Tag des Builds; `None` außerhalb von Release-Builds.
    pub build_tag: Option<String>,
    /// Link auf den exakten Quellstand; `None` ohne bekannten Commit.
    pub source_link: Option<String>,
}

/// Argumente von [`app_info`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoArgs {}

pub async fn app_info(ctx: &Arc<AppCtx>, _args: AppInfoArgs) -> AppInfo {
    let source = crate::build_info::current();
    AppInfo {
        app_version: source.version,
        schema_version: EXPECTED_SCHEMA_VERSION,
        identifier: ctx.app_identifier.clone(),
        license_spdx: source.license_spdx,
        repository_url: source.source_url,
        build_commit: source.commit,
        build_tag: source.tag,
        source_link: source.source_link,
    }
}

/// Argumente von [`third_party_licenses_path`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThirdPartyLicensesPathArgs {}

/// Pfad der generierten Drittanbieter-Lizenz-Übersicht (Altbestand).
///
/// Im Server ist der Command gesperrt (`Route::Dropped`); die Lizenzseiten
/// liefert `GET /legal/` aus (Plan W12). Gesucht wird unter
/// `<resource_dir>/handbook/third-party-licenses.html`, ersatzweise im
/// Ressourcen-Ordner des Quellbaums (Ausführung aus `klein-buch/`). Die
/// Funktion samt Lizenzseite entfernt Block F6 (W13-ALTBESTAND-LICENSES,
/// ADR 0040 Entscheidung 30).
///
/// Existiert die Datei nicht, gibt es einen `Error::Config` statt eines leeren
/// Pfads.
pub async fn third_party_licenses_path(
    ctx: &Arc<AppCtx>,
    _args: ThirdPartyLicensesPathArgs,
) -> Result<String> {
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
    /// optionaler Build-Env). Wir prüfen die Cargo-Anteile direkt; der
    /// Command selbst ist im Server-Vertragstest abgedeckt.
    #[test]
    fn app_version_is_calver_format() {
        // Cargo.toml ist Single Source der App-Version (Manuel 2026-05-26).
        // Eine zweite Versionsquelle (frühere App-Shell) gibt es nicht mehr;
        // `package.json::version` ist auf 0.0.0 gepinnt + Note.
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

    /// `app_info` und `/api/public/info` lesen dieselbe Quelle (Plan W12);
    /// keine feste Repository-URL mehr im Code.
    #[tokio::test]
    async fn app_info_uses_build_info() {
        let dir = tempfile::TempDir::new().unwrap();
        let ctx = crate::app_ctx::AppCtx::for_tests(crate::config::Paths::under(dir.path()));
        let info = app_info(&ctx, AppInfoArgs {}).await;
        let source = crate::build_info::current();
        assert_eq!(info.repository_url, source.source_url);
        assert_eq!(info.build_commit, source.commit);
        assert_eq!(info.build_tag, source.tag);
        assert_eq!(info.source_link, source.source_link);
        assert_eq!(info.license_spdx, crate::build_info::LICENSE_SPDX);
        if info.build_commit.is_none() {
            assert!(info.source_link.is_none(), "ohne Commit kein Link");
        }
    }
}
