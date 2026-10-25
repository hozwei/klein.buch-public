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
    error::{Error, Result},
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
    /// Plan W2.2: Dateien, die vorhanden, aber nicht lesbar sind (z. B.
    /// fehlende Rechte). **Nicht** in `files_failed` — kein Manipulationsbefund,
    /// aber ein Betriebsproblem, das gemeldet wird.
    pub files_unreadable: i64,
    pub unreadable_archive_ids: Vec<String>,
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
    /// Plan W2.2: Datei vorhanden, aber nicht lesbar (z. B. `PermissionDenied`).
    /// Kein Tamper; eigener Audit-Eintrag (`archive.integrity_unreadable`).
    Unreadable,
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
    // entsteht — getrennt von Tamper (Hash-Mismatch). Review W2 S2-1: nur
    // `NotFound` heißt „fehlt“; ein Rechteproblem auf dem Ordner (`exists()`
    // lieferte dann `false`) ist „nicht lesbar“, kein Missing.
    let meta = std::fs::metadata(&path);
    if let Err(e) = &meta {
        if e.kind() != std::io::ErrorKind::NotFound {
            audit::archive_event(
                pool,
                audit::ArchiveAction::IntegrityUnreadable,
                archive_id,
                Some(serde_json::json!({
                    "path": &path,
                    "error_kind": format!("{:?}", e.kind()),
                })),
            )
            .await?;
            return Ok(VerifyOutcome::Unreadable);
        }
    }
    if meta.is_err() {
        audit::archive_event(
            pool,
            audit::ArchiveAction::IntegrityMissing,
            archive_id,
            Some(serde_json::json!({ "path": &path })),
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
        // Plan W2.2: Lese-/Rechtefehler sind keine Manipulation.
        Err(Error::Io(e)) => {
            audit::archive_event(
                pool,
                audit::ArchiveAction::IntegrityUnreadable,
                archive_id,
                Some(serde_json::json!({
                    "path": &path,
                    "error_kind": format!("{:?}", e.kind()),
                })),
            )
            .await?;
            Ok(VerifyOutcome::Unreadable)
        }
        // DB-Fehler brechen den Scan ab, statt als Tamper zu zählen.
        Err(e @ Error::Db(_)) => Err(e),
        Err(e) => {
            let msg = format!("{e}");
            audit::archive_event(
                pool,
                audit::ArchiveAction::IntegrityFail,
                archive_id,
                Some(serde_json::json!({ "error": &msg })),
            )
            .await?;
            Ok(VerifyOutcome::Tampered)
        }
    }
}

/// Gemeinsame Sperre für den Vollscan (Lauf F, D-07): Monats-Cron und „Jetzt
/// prüfen“ laufen nie gleichzeitig. Der Cron wartet, der manuelle Lauf
/// antwortet bei belegter Sperre „läuft bereits“ ([`try_scan_lock`]).
pub static SCAN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Fehltext, wenn schon ein Vollscan läuft.
pub const SCAN_RUNNING: &str =
    "Die Integritätsprüfung läuft bereits — das Ergebnis erscheint in der Historie.";

/// Sperre für einen manuellen Vollscan oder Fehler „läuft bereits“.
pub fn try_scan_lock() -> Result<tokio::sync::MutexGuard<'static, ()>> {
    SCAN_LOCK
        .try_lock()
        .map_err(|_| Error::Domain(SCAN_RUNNING.into()))
}

/// Vollständiger Scan über alle `archive_entries`. Schreibt einen
/// Eintrag in `archive_integrity_checks` mit aggregiertem Ergebnis.
/// Aufrufer halten [`SCAN_LOCK`].
pub async fn run_full_scan(pool: &SqlitePool) -> Result<IntegrityCheckSummary> {
    run_full_scan_at(pool, crate::clock::now_utc()).await
}

/// Wie [`run_full_scan`], Beginn (`started_at`) aus der injizierten Uhr des
/// Aufrufers — dieselbe Zeitbasis, mit der der Monats-Cron „schon gelaufen“
/// prüft (Lauf F, D-20).
pub async fn run_full_scan_at(
    pool: &SqlitePool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<IntegrityCheckSummary> {
    static NEVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    run_full_scan_until(pool, &NEVER, now)
        .await?
        .ok_or_else(|| Error::Domain("Integritätsprüfung abgebrochen".into()))
}

/// Wie [`run_full_scan_at`], prüft vor jeder Datei die Abbruch-Anforderung
/// (`stop`, Herunterfahren/Sperren; W14-025b). `Ok(None)` = abgebrochen — der
/// Kopf bleibt ohne `finished_at` und zählt nicht als Monatslauf.
pub async fn run_full_scan_until(
    pool: &SqlitePool,
    stop: &std::sync::atomic::AtomicBool,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Option<IntegrityCheckSummary>> {
    // Kopf in archive_integrity_checks anlegen
    let check_id =
        sqlx::query("INSERT INTO archive_integrity_checks (started_at) VALUES (?) RETURNING id")
            .bind(crate::clock::to_db_utc(now))
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
    let mut unreadable_ids: Vec<String> = Vec::new();
    for id in &ids {
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::info!("Integritätsprüfung auf Anforderung abgebrochen");
            return Ok(None);
        }
        match verify_one(pool, id).await? {
            VerifyOutcome::Pass => passed += 1,
            VerifyOutcome::Tampered => tampered_ids.push(id.clone()),
            VerifyOutcome::Missing => missing_ids.push(id.clone()),
            VerifyOutcome::Unreadable => unreadable_ids.push(id.clone()),
        }
    }

    // `files_failed` = ALLE Integritätsverletzungen (Tamper + verwaist) — der
    // ehrliche Gesamtwert für die Historie/UI. `failed_archive_ids` listet beide;
    // `missing_archive_ids` ist die Teilmenge der verwaisten Einträge.
    let mut failed_ids = tampered_ids;
    failed_ids.extend(missing_ids.iter().cloned());
    let files_failed = failed_ids.len() as i64;

    // W2-INTEGRITY-UNREADABLE (Migration 0032): „fehlt“ und „nicht lesbar“
    // stehen mit in der Historie; die ID-Listen per serde_json.
    let failed_json = serde_json::to_string(&failed_ids)?;
    let missing_json = serde_json::to_string(&missing_ids)?;
    let unreadable_json = serde_json::to_string(&unreadable_ids)?;
    sqlx::query(
        "UPDATE archive_integrity_checks
            SET finished_at = datetime('now','utc'),
                files_checked = ?,
                files_passed = ?,
                files_failed = ?,
                failed_archive_ids_json = ?,
                files_missing = ?,
                files_unreadable = ?,
                missing_archive_ids_json = ?,
                unreadable_archive_ids_json = ?
          WHERE id = ?",
    )
    .bind(ids.len() as i64)
    .bind(passed)
    .bind(files_failed)
    .bind(&failed_json)
    .bind(missing_ids.len() as i64)
    .bind(unreadable_ids.len() as i64)
    .bind(&missing_json)
    .bind(&unreadable_json)
    .bind(check_id)
    .execute(pool)
    .await?;

    Ok(Some(IntegrityCheckSummary {
        check_id,
        files_checked: ids.len() as i64,
        files_passed: passed,
        files_failed,
        files_missing: missing_ids.len() as i64,
        failed_archive_ids: failed_ids,
        missing_archive_ids: missing_ids,
        files_unreadable: unreadable_ids.len() as i64,
        unreadable_archive_ids: unreadable_ids,
    }))
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
                failed_archive_ids_json TEXT,
                files_missing INTEGER,
                files_unreadable INTEGER,
                missing_archive_ids_json TEXT,
                unreadable_archive_ids_json TEXT
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

    /// Plan W2.2: eine nicht lesbare Datei (fehlende Rechte) ist **kein**
    /// Tamper-Befund, sondern `Unreadable` mit eigenem Audit-Eintrag.
    #[cfg(unix)]
    #[tokio::test]
    async fn permission_denied_is_unreadable_not_tampered() {
        use std::os::unix::fs::PermissionsExt;
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"intakt",
        )
        .await
        .unwrap();
        fs::set_permissions(&r.file_path, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&r.file_path).is_ok() {
            // Läuft als root (Rechte greifen nicht) — Szenario nicht herstellbar.
            fs::set_permissions(&r.file_path, fs::Permissions::from_mode(0o600)).unwrap();
            return;
        }

        assert_eq!(
            verify_one(&pool, &r.archive_id).await.unwrap(),
            VerifyOutcome::Unreadable
        );
        let sum = run_full_scan(&pool).await.unwrap();
        assert_eq!(sum.files_failed, 0, "kein Tamper-/Missing-Befund");
        assert_eq!(sum.files_unreadable, 1);
        assert_eq!(sum.unreadable_archive_ids, vec![r.archive_id.clone()]);
        let fails: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'archive.integrity_fail'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(fails, 0);
        // Migration 0032: „nicht lesbar“ steht in der Historie
        let (unreadable, ids): (Option<i64>, Option<String>) = sqlx::query_as(
            "SELECT files_unreadable, unreadable_archive_ids_json FROM archive_integrity_checks
              ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unreadable, Some(1));
        let ids: Vec<String> = serde_json::from_str(&ids.unwrap()).unwrap();
        assert_eq!(ids, vec![r.archive_id.clone()]);

        fs::set_permissions(&r.file_path, fs::Permissions::from_mode(0o600)).unwrap();
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

    /// KB-0038: Eine reine Größenabweichung (Eintrag passt nicht zur Datei,
    /// Inhalt sonst unverändert) zählt als Manipulation, mit Audit-Detail.
    #[tokio::test]
    async fn size_mismatch_alone_is_tampered() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"intakt",
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE archive_entries SET file_size_bytes = file_size_bytes + 1 WHERE id = ?",
        )
        .bind(&r.archive_id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            verify_one(&pool, &r.archive_id).await.unwrap(),
            VerifyOutcome::Tampered
        );
        let detail: String = sqlx::query_scalar(
            "SELECT details_json FROM audit_log WHERE action = 'archive.integrity_fail'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(detail.contains("size mismatch"), "{detail}");
        unlock_for_test(Path::new(&r.file_path)).unwrap();
    }

    /// D-07: Abbruch-Anforderung vor der ersten Datei → `None`, der Kopf
    /// bleibt ohne Abschluss (zählt nicht als Monatslauf).
    #[tokio::test]
    async fn stop_request_aborts_scan() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"x",
        )
        .await
        .unwrap();
        let stop = std::sync::atomic::AtomicBool::new(true);
        assert!(run_full_scan_until(&pool, &stop, chrono::Utc::now())
            .await
            .unwrap()
            .is_none());
        let open: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM archive_integrity_checks WHERE finished_at IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(open, 1);
        unlock_for_test(Path::new(&r.file_path)).unwrap();
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
        // Migration 0032: „fehlt“ steht in der Historie
        let (missing, ids, unreadable): (Option<i64>, Option<String>, Option<i64>) =
            sqlx::query_as(
                "SELECT files_missing, missing_archive_ids_json, files_unreadable
                   FROM archive_integrity_checks ORDER BY id DESC LIMIT 1",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(missing, Some(1));
        assert_eq!(unreadable, Some(0));
        let ids: Vec<String> = serde_json::from_str(&ids.unwrap()).unwrap();
        assert_eq!(ids, vec![r2.archive_id.clone()]);

        unlock_for_test(Path::new(&r1.file_path)).unwrap();
    }
}
