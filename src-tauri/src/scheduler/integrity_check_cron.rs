//! Monatlicher Archiv-Integritäts-Check (Block 15, Dauerbetrieb seit Lauf F).
//!
//! Re-hasht alle `archive_entries` (Bit-Rot-/Tamper-Detection über 10 Jahre,
//! D-50). Höchstens **einmal pro Kalendermonat** (Europe/Berlin) automatisch —
//! gesteuert über das jüngste *abgeschlossene* `archive_integrity_checks`. Bei
//! Befunden entsteht ein **dringender** Hinweis (Severity `urgent`).
//!
//! Lauf F (D-07, ADR 0040):
//! - **Gemeinsame Sperre** mit „Jetzt prüfen“
//!   ([`integrity_check::SCAN_LOCK`]): der Cron wartet, der manuelle Lauf
//!   antwortet „läuft bereits“; nach dem Warten prüft der Cron erneut, ob der
//!   Monat inzwischen erledigt ist.
//! - **Persistierter Backoff:** Scheitert der Scan (z. B. DB-Fehler), versucht
//!   der Cron es erst nach [`FAILURE_BACKOFF_HOURS`] Stunden erneut (nicht bei
//!   jedem Tick), mit einem Hinweis je Monat.
//! - **Abbruch je Datei:** Sperren und Herunterfahren beenden den Scan vor der
//!   nächsten Datei; der Lauf zählt dann nicht und wird beim nächsten Tick
//!   wiederholt.
//!
//! Der eigentliche Scan liegt in [`crate::archive::integrity_check`]; hier nur
//! die Cron-Steuerung + Notification.

use crate::archive::integrity_check;
use crate::error::Result;
use crate::notify::{self, rules, NewNotification};
use crate::scheduler::backoff;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::sync::atomic::AtomicBool;

/// Schlüssel des Backoffs nach einem gescheiterten Monats-Scan.
pub const BACKOFF_KEY: &str = "integrity_last_failure_at";
/// Wartezeit nach einem Fehlschlag.
pub const FAILURE_BACKOFF_HOURS: i64 = 6;

/// Ergebnis eines Cron-Aufrufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CronOutcome {
    /// In diesem Monat schon gelaufen.
    AlreadyDone,
    /// Nach einem Fehlschlag noch im Backoff.
    BackedOff,
    /// Ein manueller Lauf hält die Sperre (nur ohne Warten).
    Busy,
    /// Auf Anforderung abgebrochen (zählt nicht).
    Aborted,
    /// Scan gelaufen.
    Scanned,
}

/// Führt den Monats-Scan aus, falls in diesem Kalendermonat noch keiner
/// abgeschlossen wurde. `wait = false` (manueller Auslöser „Prüfungen
/// ausführen“) überspringt den Scan, wenn gerade einer läuft.
pub async fn run(
    pool: &SqlitePool,
    now: DateTime<Utc>,
    stop: &AtomicBool,
    wait: bool,
) -> Result<CronOutcome> {
    let today = crate::clock::berlin_date(now);
    let this_month = today.format("%Y-%m").to_string();
    if ran_this_month(pool, today).await? {
        return Ok(CronOutcome::AlreadyDone);
    }
    if backoff::blocked(
        pool,
        BACKOFF_KEY,
        now,
        chrono::Duration::hours(FAILURE_BACKOFF_HOURS),
    )
    .await?
    {
        return Ok(CronOutcome::BackedOff);
    }
    let _scan = if wait {
        integrity_check::SCAN_LOCK.lock().await
    } else {
        match integrity_check::SCAN_LOCK.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(CronOutcome::Busy),
        }
    };
    // Während des Wartens kann „Jetzt prüfen“ den Monat erledigt haben.
    if ran_this_month(pool, today).await? {
        return Ok(CronOutcome::AlreadyDone);
    }

    let summary = match integrity_check::run_full_scan_until(pool, stop, now).await {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(CronOutcome::Aborted),
        Err(e) => {
            backoff::record_failure(pool, BACKOFF_KEY, now).await?;
            let body = format!(
                "Die monatliche Prüfung des Archivs ist abgebrochen ({e}). Klein.Buch versucht es \
                 in {FAILURE_BACKOFF_HOURS} Stunden erneut. Bleibt der Fehler, das Server-Log \
                 prüfen oder die Prüfung unter Einstellungen → Audit-Trail von Hand starten."
            );
            notify::emit(
                pool,
                NewNotification {
                    title: "Archiv-Prüfung fehlgeschlagen",
                    body: &body,
                    severity: "warning",
                    action_url: Some("/settings/audit-trail"),
                    dedup_key: Some(&format!("archive_integrity_scan_failed:{this_month}")),
                    ..Default::default()
                },
            )
            .await?;
            return Err(e);
        }
    };
    backoff::clear(pool, BACKOFF_KEY).await?;
    tracing::info!(
        "Integrity-Check {this_month}: {} geprüft, {} ok, {} Fehler",
        summary.files_checked,
        summary.files_passed,
        summary.files_failed
    );
    notify_summary(pool, &summary, &this_month).await?;
    Ok(CronOutcome::Scanned)
}

/// Lief im Berliner Kalendermonat von `today` schon ein abgeschlossener Scan?
///
/// Geprüft wird das Monatsintervall (F1d), nicht der jüngste Lauf: Ein Lauf
/// mit Datum in der Zukunft (Uhr stand vor, danach korrigiert) zählte sonst nie
/// als „dieser Monat“, und jeder Tick hashte das ganze Archiv neu.
async fn ran_this_month(pool: &SqlitePool, today: chrono::NaiveDate) -> Result<bool> {
    use chrono::Datelike;
    let first = today.with_day(1).unwrap_or(today);
    let next = first
        .checked_add_months(chrono::Months::new(1))
        .unwrap_or(first);
    // `started_at` ist UTC im DB-Format; die Grenzen sind die Berliner
    // Monatsanfänge in UTC (Plan W1.8). Review W2 S2-2: nur abgeschlossene
    // Läufe zählen — ein abgebrochener Scan (DB-Fehler, Herunterfahren) darf
    // den Monat nicht als erledigt markieren.
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM archive_integrity_checks
          WHERE finished_at IS NOT NULL
            AND replace(substr(started_at, 1, 19), 'T', ' ') >= ?1
            AND replace(substr(started_at, 1, 19), 'T', ' ') < ?2
          LIMIT 1",
    )
    .bind(crate::clock::berlin_day_start_db_utc(first))
    .bind(crate::clock::berlin_day_start_db_utc(next))
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

/// Hinweise zum Ergebnis eines Scans — vom Monats-Cron und vom manuellen Lauf
/// (Review W2 S1-2: sonst erschien „nicht lesbar“ in einem Monat mit manuellem
/// Scan gar nicht).
pub async fn notify_summary(
    pool: &SqlitePool,
    summary: &integrity_check::IntegrityCheckSummary,
    this_month: &str,
) -> Result<()> {
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

    // Plan W2.2: nicht lesbare Dateien (Rechte) sind kein Tamper, aber ein
    // Betriebsproblem — eigener Hinweis, damit sie nicht untergehen.
    if summary.files_unreadable > 0 {
        let key = format!("archive_integrity_unreadable:{this_month}");
        let body = format!(
            "{} Archiv-Datei(en) konnten nicht gelesen werden (z. B. fehlende \
             Dateirechte). Das ist kein Manipulationsbefund, verhindert aber die \
             Prüfung. Bitte die Rechte im Datenverzeichnis prüfen.",
            summary.files_unreadable
        );
        notify::emit(
            pool,
            NewNotification {
                title: "Archiv-Dateien nicht lesbar",
                body: &body,
                severity: "warning",
                action_url: Some("/settings/audit-trail"),
                dedup_key: Some(&key),
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}
