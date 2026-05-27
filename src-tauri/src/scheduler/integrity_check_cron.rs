//! Monatlicher Archiv-Integritäts-Check (Block 15).
//!
//! Re-hasht alle `archive_entries` (Bit-Rot-/Tamper-Detection über 10 Jahre,
//! D-50). Höchstens **einmal pro Kalendermonat** automatisch — gesteuert über
//! das jüngste `archive_integrity_checks.started_at`. Bei Fehlern entsteht ein
//! **dringender** Hinweis (Severity `urgent`), gegen den Verlust eines stillen
//! Datenschadens.
//!
//! Der eigentliche Scan liegt in [`crate::archive::integrity_check`] (bereits in
//! Phase 1 implementiert + getestet); hier nur die Cron-Steuerung + Notification.

use crate::archive::integrity_check;
use crate::error::Result;
use crate::notify::{self, rules, NewNotification};
use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::AppHandle;

/// Führt den Monats-Scan aus, falls in diesem Kalendermonat noch keiner lief.
pub async fn run(pool: &SqlitePool, app: Option<&AppHandle>, today: NaiveDate) -> Result<()> {
    let this_month = today.format("%Y-%m").to_string();
    let last: Option<String> =
        sqlx::query_scalar("SELECT MAX(started_at) FROM archive_integrity_checks")
            .fetch_one(pool)
            .await?;
    let ran_this_month = last
        .as_deref()
        .and_then(|ts| ts.get(0..7))
        .map(|m| m == this_month)
        .unwrap_or(false);
    if ran_this_month {
        return Ok(());
    }

    let summary = integrity_check::run_full_scan(pool).await?;
    tracing::info!(
        "Integrity-Check {this_month}: {} geprüft, {} ok, {} Fehler",
        summary.files_checked,
        summary.files_passed,
        summary.files_failed
    );

    if summary.files_failed > 0 {
        if let Some(rule) = rules::get_enabled_by_type(pool, "archive_integrity_failed").await? {
            let key = format!("archive_integrity_failed:{this_month}");
            // G1-HARDEN.4: Tamper (Hash-Mismatch) und verwaiste/fehlende Dateien
            // getrennt benennen — der Nutzer braucht beide für die Diagnose.
            let tampered = summary.files_failed - summary.files_missing;
            let body = format!(
                "{} von {} Archiv-Dateien sind nicht mehr in Ordnung: {} verändert \
                 (Hash-Mismatch), {} fehlen (Datei nicht mehr vorhanden). Das ist \
                 kritisch (GoBD). Bitte ein sauberes Backup zurückspielen.",
                summary.files_failed, summary.files_checked, tampered, summary.files_missing
            );
            notify::emit(
                pool,
                app,
                NewNotification {
                    rule_id: Some(&rule.id),
                    title: "Archiv-Integrität gestört",
                    body: &body,
                    severity: "urgent",
                    action_url: Some("/settings/audit-trail"),
                    dedup_key: Some(&key),
                    ..Default::default()
                },
            )
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
