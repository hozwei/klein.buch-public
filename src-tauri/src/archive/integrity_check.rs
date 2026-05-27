//! Periodische SHA-256-Re-Verifizierung aller `archive_entries`.
//!
//! In Phase 1 nur als On-Demand-Funktion benutzt (Manueller Smoke-Test +
//! Restore-Workflow). In Phase 2D wird das hier vom Scheduler getriggert
//! und das Ergebnis in `archive_integrity_checks` festgehalten.
//!
//! GoBD-Hardline: jeder Pass und Fail erzeugt einen Audit-Log-Eintrag
//! ("archive.integrity_pass" / "archive.integrity_fail") via
//! [`crate::archive::audit`].

use crate::{
    archive::{audit, store},
    error::Result,
};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityCheckSummary {
    pub check_id: i64,
    pub files_checked: i64,
    pub files_passed: i64,
    /// Gesamtzahl der Einträge, die NICHT intakt verifiziert wurden — Tamper
    /// (Hash-/Größen-Mismatch) **und** verwaiste/fehlende Dateien zusammen.
    pub files_failed: i64,
    /// Teilmenge von `files_failed`: Einträge, deren Datei fehlt (G1-HARDEN.4).
    pub files_missing: i64,
    /// Alle nicht-intakten Einträge (Tamper + verwaist).
    pub failed_archive_ids: Vec<String>,
    /// Nur die verwaisten Einträge (Datei nicht mehr vorhanden).
    pub missing_archive_ids: Vec<String>,
}

/// Ergebnis der Verifikation eines einzelnen Archiv-Eintrags (G1-HARDEN.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// Datei vorhanden, Hash + Größe stimmen.
    Pass,
    /// Datei vorhanden, aber Hash/Größe weichen ab (Manipulation/Bit-Rot).
    Tampered,
    /// Datei fehlt — verwaister `archive_entries`-Eintrag. **Sauberes Skip**
    /// statt Lese-Fehler; eigener Audit-Eintrag (`archive.integrity_missing`).
    Missing,
}

/// Verifiziert einen einzelnen Archive-Eintrag und **klassifiziert** das
/// Ergebnis (G1-HARDEN.4). In allen drei Fällen (Pass/Tamper/Missing) entsteht
/// **genau ein** Audit-Log-Eintrag. `Err` ist nur Infrastruktur-Fehlern (DB)
/// vorbehalten — eine fehlende Datei ist kein harter Fehler, sondern wird sauber
/// als [`VerifyOutcome::Missing`] gemeldet.
pub async fn verify_one(pool: &SqlitePool, archive_id: &str) -> Result<VerifyOutcome> {
    let path: Option<String> =
        sqlx::query_scalar("SELECT file_path FROM archive_entries WHERE id = ?")
            .bind(archive_id)
            .fetch_optional(pool)
            .await?;
    let Some(path) = path else {
        // Kein DB-Eintrag (im Scan über vorhandene ids unerwartet) — defensiv als
        // verwaist behandeln, damit der Scan nicht hart abbricht.
        audit::archive_event(
            pool,
            audit::ArchiveAction::IntegrityMissing,
            archive_id,
            None,
        )
        .await?;
        return Ok(VerifyOutcome::Missing);
    };

    // Verwaiste Datei (gelöscht/verschoben) sauber abfangen, BEVOR ein Lese-Fehler
    // entsteht — getrennt von Tamper (Hash-Mismatch).
    if !std::path::Path::new(&path).exists() {
        audit::archive_event(
            pool,
            audit::ArchiveAction::IntegrityMissing,
            archive_id,
            Some(&format!(r#"{{"path":"{}"}}"#, escape(&path))),
        )
        .await?;
        return Ok(VerifyOutcome::Missing);
    }

    // Silent-Variante: der Integrity-Cron emittiert bereits eigene Audit-
    // Events (Pass/Fail/Missing) — der `archive.read`-Audit aus R2-028 ist
    // hier unnötiger Lärm.
    match store::read_and_verify_silent(pool, archive_id).await {
        Ok(_) => {
            audit::archive_event(pool, audit::ArchiveAction::IntegrityPass, archive_id, None)
                .await?;
            Ok(VerifyOutcome::Pass)
        }
        Err(e) => {
            let msg = format!("{e}");
            audit::archive_event(
                pool,
                audit::ArchiveAction::IntegrityFail,
                archive_id,
                Some(&format!(r#"{{"error":"{}"}}"#, escape(&msg))),
            )
            .await?;
            Ok(VerifyOutcome::Tampered)
        }
    }
}

/// Vollständiger Scan über alle `archive_entries`. Schreibt einen
/// Eintrag in `archive_integrity_checks` mit aggregiertem Ergebnis.
pub async fn run_full_scan(pool: &SqlitePool) -> Result<IntegrityCheckSummary> {
    // Kopf in archive_integrity_checks anlegen
    let check_id = sqlx::query("INSERT INTO archive_integrity_checks DEFAULT VALUES RETURNING id")
        .fetch_one(pool)
        .await?;
    use sqlx::Row;
    let check_id: i64 = check_id.try_get("id")?;

    let ids: Vec<String> = sqlx::query("SELECT id FROM archive_entries ORDER BY received_at")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| r.get::<String, _>("id"))
        .collect();

    let mut passed = 0i64;
    let mut tampered_ids: Vec<String> = Vec::new();
    let mut missing_ids: Vec<String> = Vec::new();
    for id in &ids {
        match verify_one(pool, id).await? {
            VerifyOutcome::Pass => passed += 1,
            VerifyOutcome::Tampered => tampered_ids.push(id.clone()),
            VerifyOutcome::Missing => missing_ids.push(id.clone()),
        }
    }

    // `files_failed` = ALLE Integritätsverletzungen (Tamper + verwaist) — der
    // ehrliche Gesamtwert für die Historie/UI. `failed_archive_ids` listet beide;
    // `missing_archive_ids` ist die Teilmenge der verwaisten Einträge.
    let mut failed_ids = tampered_ids;
    failed_ids.extend(missing_ids.iter().cloned());
    let files_failed = failed_ids.len() as i64;

    let failed_json = serde_json::to_string(&failed_ids).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "UPDATE archive_integrity_checks
            SET finished_at = datetime('now','utc'),
                files_checked = ?,
                files_passed = ?,
                files_failed = ?,
                failed_archive_ids_json = ?
          WHERE id = ?",
    )
    .bind(ids.len() as i64)
    .bind(passed)
    .bind(files_failed)
    .bind(&failed_json)
    .bind(check_id)
    .execute(pool)
    .await?;

    Ok(IntegrityCheckSummary {
        check_id,
        files_checked: ids.len() as i64,
        files_passed: passed,
        files_failed,
        files_missing: missing_ids.len() as i64,
        failed_archive_ids: failed_ids,
        missing_archive_ids: missing_ids,
    })
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::store::{store_bytes, unlock_for_test, ArchiveKind};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE archive_entries (
                id TEXT PRIMARY KEY NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                file_hash_sha256 TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                source TEXT NOT NULL,
                received_at TEXT NOT NULL DEFAULT (datetime('now','utc'))
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system',
                action TEXT NOT NULL,
                entity_type TEXT,
                entity_id TEXT,
                details_json TEXT
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE archive_integrity_checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
                finished_at TEXT,
                files_checked INTEGER NOT NULL DEFAULT 0,
                files_passed INTEGER NOT NULL DEFAULT 0,
                files_failed INTEGER NOT NULL DEFAULT 0,
                failed_archive_ids_json TEXT
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn full_scan_passes_when_no_tampering() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        for i in 1..=3 {
            let r = store_bytes(
                &pool,
                dir.path(),
                2026,
                ArchiveKind::InvoicePdf,
                &format!("RE-2026-{i:04}.pdf"),
                "application/pdf",
                format!("doc {i}").as_bytes(),
            )
            .await
            .unwrap();
            unlock_for_test(Path::new(&r.file_path)).unwrap();
        }

        let sum = run_full_scan(&pool).await.unwrap();
        assert_eq!(sum.files_checked, 3);
        assert_eq!(sum.files_passed, 3);
        assert_eq!(sum.files_failed, 0);
        assert!(sum.failed_archive_ids.is_empty());
    }

    #[tokio::test]
    async fn full_scan_detects_tampered_file() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r1 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"good",
        )
        .await
        .unwrap();
        let r2 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0002.pdf",
            "application/pdf",
            b"also good",
        )
        .await
        .unwrap();

        // Tamper r2
        unlock_for_test(Path::new(&r2.file_path)).unwrap();
        fs::write(&r2.file_path, b"tampered!").unwrap();

        let sum = run_full_scan(&pool).await.unwrap();
        assert_eq!(sum.files_checked, 2);
        assert_eq!(sum.files_passed, 1);
        assert_eq!(sum.files_failed, 1);
        assert_eq!(sum.files_missing, 0);
        assert_eq!(sum.failed_archive_ids, vec![r2.archive_id]);

        unlock_for_test(Path::new(&r1.file_path)).unwrap();
        unlock_for_test(Path::new(&r2.file_path)).unwrap();
    }

    /// G1-HARDEN.4: eine **fehlende** Archiv-Datei (verwaister Eintrag) wird
    /// sauber als „missing" gemeldet — getrennt von Tamper — und der Scan läuft
    /// trotzdem komplett durch.
    #[tokio::test]
    async fn full_scan_reports_missing_file_as_orphan() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r1 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"intact",
        )
        .await
        .unwrap();
        let r2 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0002.pdf",
            "application/pdf",
            b"will vanish",
        )
        .await
        .unwrap();

        // r2 von der Platte entfernen → verwaister DB-Eintrag.
        unlock_for_test(Path::new(&r2.file_path)).unwrap();
        fs::remove_file(&r2.file_path).unwrap();

        let sum = run_full_scan(&pool).await.unwrap();
        assert_eq!(sum.files_checked, 2);
        assert_eq!(sum.files_passed, 1);
        // Verwaist zählt als Integritätsproblem (files_failed) UND wird als
        // missing aufgeschlüsselt.
        assert_eq!(sum.files_failed, 1);
        assert_eq!(sum.files_missing, 1);
        assert_eq!(sum.missing_archive_ids, vec![r2.archive_id.clone()]);
        assert!(sum.failed_archive_ids.contains(&r2.archive_id));

        // Eigener Audit-Eintrag für die fehlende Datei.
        use sqlx::Row;
        let cnt: i64 = sqlx::query(
            "SELECT COUNT(*) AS n FROM audit_log WHERE action = 'archive.integrity_missing'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
        assert_eq!(cnt, 1);

        unlock_for_test(Path::new(&r1.file_path)).unwrap();
    }
}
