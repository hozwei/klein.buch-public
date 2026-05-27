//! Restore-Wizard (Block 4) — verify-then-apply, zweiphasig.
//!
//! Warum zweiphasig? Während die App läuft, hält der sqlx-Pool die SQLite-Datei
//! (inkl. WAL) offen. Unter Windows lässt sich eine geöffnete DB-Datei nicht
//! zuverlässig ersetzen. Daher:
//!
//! 1. **Phase A — [`apply`]** (App läuft): Pre-Restore-Backup erstellen,
//!    Backup-Datei entschlüsseln + Hash verifizieren, Inhalt in ein
//!    Staging-Verzeichnis entpacken, eine **Marker-Datei** schreiben und einen
//!    Neustart anfordern.
//! 2. **Phase B — [`apply_pending`]** (App-Start, **vor** dem Öffnen des Pools):
//!    Staging-DB über die echte DB legen, Archive ersetzen, Marker löschen.
//!
//! Die Marker-Datei liegt im Dateisystem (nicht in der DB!), weil die DB selbst
//! ausgetauscht wird.
//!
//! GoBD/§-Hinweise:
//! - **`inputs/` bleibt tabu** — beim Restore werden nur maschinen-verwaltete
//!   Daten ersetzt (DB + `archive/`). Branding aus `inputs/` wird **nicht**
//!   überschrieben (es ist menschen-maintained und liegt ohnehin lokal vor).
//! - **Pre-Restore-Backup ist Pflicht** und nutzt die *aktuelle* Session-
//!   Passphrase, sodass der Vor-Restore-Stand jederzeit wiederherstellbar ist.

use crate::backup::{create_now, encrypt, manifest, snapshot, BackupSession};
use crate::config::Paths;
use crate::db::schema_version::EXPECTED_SCHEMA_VERSION;
use crate::error::{Error, Result};
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

/// Name der Restore-Marker-Datei im `data_dir`.
pub const RESTORE_MARKER: &str = "RESTORE_PENDING.json";
/// Name des Staging-Verzeichnisses im `data_dir`.
pub const RESTORE_STAGING: &str = "restore-staging";

/// Anzeige-Vorschau eines Backup-Files (ohne Passphrase).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub file_path: String,
    pub created_at: String,
    pub app_version: String,
    pub format_version: u32,
    pub schema_version: i32,
    pub current_schema_version: i32,
    pub compatible: bool,
    pub content_size_bytes: u64,
}

/// Ergebnis von Phase A.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub requires_restart: bool,
    pub pre_restore_backup_path: String,
    pub source_file: String,
    pub staged_at: String,
}

/// Liest den Plain-Header eines Backup-Files für die Anzeige im Wizard.
pub fn preview(file_path: &Path) -> Result<RestorePreview> {
    let bytes = fs::read(file_path)?;
    let m = manifest::read_manifest_only(&bytes)?;
    Ok(RestorePreview {
        file_path: file_path.to_string_lossy().to_string(),
        created_at: m.created_at,
        app_version: m.app_version,
        format_version: m.format_version,
        schema_version: m.schema_version,
        current_schema_version: EXPECTED_SCHEMA_VERSION,
        compatible: m.schema_version == EXPECTED_SCHEMA_VERSION,
        content_size_bytes: m.content_size_bytes,
    })
}

/// Entschlüsselt + verifiziert ein Backup-File mit der gegebenen Passphrase.
/// Liefert (Manifest, Content-ZIP-Bytes). Schlägt fehl bei falscher Passphrase
/// (GCM-Auth) oder bei Content-Hash-Mismatch (Manipulation).
pub fn decrypt_backup(
    file_bytes: &[u8],
    passphrase: &str,
) -> Result<(manifest::Manifest, Vec<u8>)> {
    let (m, ct) = manifest::unframe(file_bytes)?;
    let salt = manifest::from_hex(&m.salt_hex)?;
    let nonce = encrypt::nonce_array(&manifest::from_hex(&m.nonce_hex)?)?;
    let key = encrypt::derive_key(passphrase.as_bytes(), &salt)?;
    let content = encrypt::decrypt(&key, &nonce, &ct)?;
    let actual = snapshot::sha256_hex(&content);
    if actual != m.content_sha256 {
        return Err(Error::Backup(
            "Content-Hash stimmt nicht — Backup beschädigt".into(),
        ));
    }
    Ok((m, content))
}

/// Phase A: validieren, Pre-Restore-Backup, entschlüsseln, ins Staging entpacken,
/// Marker schreiben. Verlangt eine entsperrte Session (für das Pre-Restore-
/// Backup mit der *aktuellen* Passphrase).
pub async fn apply(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    file_path: &Path,
    passphrase: &str,
) -> Result<RestoreReport> {
    let current = session.get().ok_or_else(|| {
        Error::Backup("Vor dem Restore bitte die App entsperren (aktuelle Passphrase).".into())
    })?;

    let file_bytes = fs::read(file_path)?;
    let header = manifest::read_manifest_only(&file_bytes)?;
    if header.schema_version != EXPECTED_SCHEMA_VERSION {
        return Err(Error::Backup(format!(
            "Backup hat Schema-Version {}, die App erwartet {}. Restore abgebrochen — bitte App-Version angleichen.",
            header.schema_version, EXPECTED_SCHEMA_VERSION
        )));
    }

    // Pre-Restore-Backup (Pflicht) mit der aktuellen Session-Passphrase.
    let pre = create_now(pool, paths, &current, "pre_restore").await?;

    // Entschlüsseln + Content-Hash prüfen.
    let (_m, content_zip) = decrypt_backup(&file_bytes, passphrase)?;

    // Ins Staging entpacken.
    let staging = paths.data_dir.join(RESTORE_STAGING);
    reset_dir(&staging)?;
    extract_zip(&content_zip, &staging)?;
    if !staging.join("klein-buch.sqlite").is_file() {
        return Err(Error::Backup(
            "Backup enthält keine klein-buch.sqlite — Restore abgebrochen".into(),
        ));
    }

    // Marker schreiben (Dateisystem, nicht DB).
    let staged_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let marker = paths.data_dir.join(RESTORE_MARKER);
    let info = serde_json::json!({
        "staging": staging.to_string_lossy(),
        "source": file_path.to_string_lossy(),
        "staged_at": staged_at,
        "pre_restore_backup": pre.file_path,
    });
    fs::write(&marker, serde_json::to_vec_pretty(&info)?)?;

    // R2-026: Pflicht-Audit (GoBD: Restore-Vorgang muss lückenlos
    // dokumentiert sein). Fehler propagieren statt schlucken.
    crate::db::repo::audit_log::append(
        pool,
        "backup.restore.staged",
        "backup",
        "restore",
        Some(&format!(
            r#"{{"source":"{}","pre_restore":"{}"}}"#,
            file_path.to_string_lossy().replace('"', "'"),
            pre.file_path.replace('"', "'")
        )),
    )
    .await?;

    Ok(RestoreReport {
        requires_restart: true,
        pre_restore_backup_path: pre.file_path,
        source_file: file_path.to_string_lossy().to_string(),
        staged_at,
    })
}

/// Info über einen angewandten Restore (für nachträgliches Audit nach Pool-Open).
#[derive(Debug, Clone)]
pub struct AppliedInfo {
    pub source: String,
    pub staged_at: String,
}

/// Suffix für die zur-Seite-bewegten Originale beim Phase-B-Swap (R4-001).
const DB_ROLLBACK_SUFFIX: &str = ".rollback";
/// Verzeichnis-Name für das zur-Seite-bewegte alte Archive.
const ARCHIVE_ROLLBACK_DIR: &str = "archive.rollback";

/// **R4-001:** Bereinigt Rollback-Reste aus einem unvollständigen Vorlauf-Swap.
///
/// Wird zu Beginn von `apply_pending` aufgerufen — und damit **vor** jedem
/// Pool-Open in `db::prepare_filesystem`. Drei Szenarien:
///
/// 1. **Sauberer Zustand** (keine `.rollback`-Reste) → no-op.
/// 2. **Vorheriger Swap erfolgreich, aber Cleanup unterbrochen** (DB + .rollback
///    beide vorhanden) → .rollback ist ein verwaister Schatten der alten DB,
///    sicher löschen.
/// 3. **Vorheriger Swap mittendrin gecrasht** (DB fehlt, .rollback existiert) →
///    Original wiederherstellen (rename zurück). Marker bleibt für Re-Run.
///
/// `pub`, weil das Integrations-Test-File `tests/r4_review_test.rs` die Funktion
/// direkt aufruft (verifiziert Crash-Recovery-Szenarien ohne den Marker-Pfad).
pub fn recover_pending_rollback(paths: &Paths) -> Result<()> {
    let rb_db = with_suffix(&paths.db_file, DB_ROLLBACK_SUFFIX);
    let rb_archive = paths.data_dir.join(ARCHIVE_ROLLBACK_DIR);

    // DB-Seite.
    match (paths.db_file.exists(), rb_db.exists()) {
        (true, true) => {
            // Beide da → vorheriger Swap fertig, Cleanup nicht abgeschlossen.
            let _ = remove_if_exists(&rb_db);
        }
        (false, true) => {
            // DB fehlt, .rollback da → Crash zwischen Rename und Re-Insert.
            // Original wiederherstellen.
            if let Some(parent) = paths.db_file.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&rb_db, &paths.db_file)?;
        }
        _ => {}
    }
    // WAL/SHM von der wiederhergestellten Original-DB können hierbleiben —
    // SQLite spielt sie beim nächsten Open korrekt zurück.

    // Archive-Seite.
    match (paths.archive_dir.is_dir(), rb_archive.is_dir()) {
        (true, true) => {
            // Beide da → verwaister Schatten, löschen.
            let _ = clear_dir_force(&rb_archive);
            let _ = fs::remove_dir(&rb_archive);
        }
        (false, true) => {
            // Archive fehlt, .rollback da → wiederherstellen.
            fs::rename(&rb_archive, &paths.archive_dir)?;
        }
        _ => {}
    }
    Ok(())
}

/// Phase B: wird beim App-Start **vor** dem Öffnen des Pools aufgerufen. Wenn ein
/// Marker existiert, wird die Staging-DB über die echte DB gelegt und das Archive
/// ersetzt. `inputs/` bleibt unangetastet (Hardline).
///
/// **R4-001:** Atomare Swap-Versicherung — alte DB und altes Archive werden
/// vor dem Replace per `rename` zur Seite gelegt (`.rollback`-Suffix bzw.
/// `archive.rollback`-Verzeichnis). Bei Fehler im Swap werden sie zurück-
/// gerename'd, der Marker bleibt für einen späteren Re-Run. Bei Erfolg werden
/// die `.rollback`-Reste gelöscht. Vor jedem Lauf räumt `recover_pending_rollback`
/// Reste aus einem mittendrin gecrashten Vorgänger auf — Re-Run-fähig.
pub fn apply_pending(paths: &Paths) -> Result<Option<AppliedInfo>> {
    // R4-001: Reste aus einem evtl. mittendrin gecrashten Vorgänger zuerst
    // aufräumen / zurückspielen.
    recover_pending_rollback(paths)?;

    let marker = paths.data_dir.join(RESTORE_MARKER);
    if !marker.exists() {
        return Ok(None);
    }

    let info: serde_json::Value = serde_json::from_slice(&fs::read(&marker)?)?;
    let staging = PathBuf::from(
        info.get("staging")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Backup("Restore-Marker ohne staging-Pfad".into()))?,
    );
    let source = info
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let staged_at = info
        .get("staged_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let staged_db = staging.join("klein-buch.sqlite");
    if !staged_db.is_file() {
        return Err(Error::Backup(
            "Restore-Staging unvollständig (keine DB) — Marker bleibt für Diagnose".into(),
        ));
    }

    // R4-001: Stage 1 — alte DB + altes Archive per atomic rename zur Seite.
    let rb_db = with_suffix(&paths.db_file, DB_ROLLBACK_SUFFIX);
    let rb_archive = paths.data_dir.join(ARCHIVE_ROLLBACK_DIR);

    // DB-Side: WAL/SHM gehören zur alten DB → mit ihnen kann SQLite nicht ohne
    // sie zurück, also löschen wir sie sofort. Die .rollback-DB ist ohne WAL/SHM
    // bei einem evtl. Rollback ein sauberer (wenn ggf. älterer) Zustand.
    remove_if_exists(&with_suffix(&paths.db_file, "-wal"))?;
    remove_if_exists(&with_suffix(&paths.db_file, "-shm"))?;
    if paths.db_file.exists() {
        // Bestehende `.rollback` (sollte nach `recover_pending_rollback` nicht mehr
        // existieren, aber defensiv) wegputzen.
        let _ = remove_if_exists(&rb_db);
        fs::rename(&paths.db_file, &rb_db)?;
    }
    if paths.archive_dir.is_dir() {
        let _ = clear_dir_force(&rb_archive);
        let _ = fs::remove_dir(&rb_archive);
        fs::rename(&paths.archive_dir, &rb_archive)?;
    }

    // R4-001: Stage 2 — eigentlicher Swap. Bei jedem Fehler darin: Rollback,
    // Marker bleibt für Re-Run.
    let swap_result: Result<()> = (|| {
        if let Some(parent) = paths.db_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&staged_db, &paths.db_file)?;

        let staged_archive = staging.join("archive");
        if staged_archive.is_dir() {
            fs::create_dir_all(&paths.archive_dir)?;
            copy_dir_recursive(&staged_archive, &paths.archive_dir)?;
        }
        Ok(())
    })();

    if let Err(e) = swap_result {
        // Rollback: angefangene neue DB/Archive zurücknehmen, Originale
        // wiederherstellen.
        let _ = remove_if_exists(&paths.db_file);
        if rb_db.exists() {
            let _ = fs::rename(&rb_db, &paths.db_file);
        }
        if paths.archive_dir.is_dir() {
            let _ = clear_dir_force(&paths.archive_dir);
            let _ = fs::remove_dir(&paths.archive_dir);
        }
        if rb_archive.is_dir() {
            let _ = fs::rename(&rb_archive, &paths.archive_dir);
        }
        // Marker + Staging bleiben — Re-Run kann später durchgereicht werden.
        return Err(e);
    }

    // R4-001: Stage 3 — Cleanup der `.rollback`-Reste.
    let _ = remove_if_exists(&rb_db);
    if rb_archive.is_dir() {
        let _ = clear_dir_force(&rb_archive);
        let _ = fs::remove_dir(&rb_archive);
    }

    // Marker + Staging aufräumen.
    let _ = fs::remove_file(&marker);
    let _ = clear_dir_force(&staging);

    Ok(Some(AppliedInfo { source, staged_at }))
}

// ---------------------------------------------------------------------------
// Datei-Helfer
// ---------------------------------------------------------------------------

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        make_writable(path)?;
        fs::remove_file(path)?;
    }
    Ok(())
}

fn make_writable(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(path, perms);
    }
    Ok(())
}

/// Leert ein Verzeichnis rekursiv, inkl. read-only-Dateien (Archive-Files sind
/// `0o400`/read-only). Entfernt das Verzeichnis selbst nicht.
///
/// `pub(crate)`, weil der Factory Reset (G1-RESET, ADR 0036) denselben
/// force-rekursiven Lösch-Mechanismus für den `data_dir`-Nuke nutzt.
pub(crate) fn clear_dir_force(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut stack = vec![dir.to_path_buf()];
    let mut dirs_to_remove = Vec::new();
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p.clone());
                dirs_to_remove.push(p);
            } else {
                make_writable(&p)?;
                fs::remove_file(&p)?;
            }
        }
    }
    // Tiefste zuerst entfernen.
    dirs_to_remove.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs_to_remove {
        let _ = fs::remove_dir(&d);
    }
    Ok(())
}

fn reset_dir(dir: &Path) -> Result<()> {
    clear_dir_force(dir)?;
    let _ = fs::remove_dir(dir);
    fs::create_dir_all(dir)?;
    Ok(())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            fs::create_dir_all(&dst)?;
            copy_dir_recursive(&src, &dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Entpackt einen Content-ZIP nach `dest`. Wehrt Path-Traversal ab.
fn extract_zip(zip_bytes: &[u8], dest: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            return Err(Error::Backup(format!(
                "unsicherer Pfad im Backup-ZIP: {name}"
            )));
        }
        let out_path = dest.join(&name);
        if name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        fs::write(&out_path, &buf)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tempfile::TempDir;

    fn paths_for(dir: &Path) -> Paths {
        Paths {
            data_dir: dir.to_path_buf(),
            db_file: dir.join("klein-buch.sqlite"),
            archive_dir: dir.join("archive"),
            backups_dir: dir.join("backups"),
            inputs_dir: dir.join("inputs"),
            sidecar_dir: dir.join("sidecar"),
        }
    }

    async fn pool_for_backup(db_file: &Path) -> SqlitePool {
        let opts = SqliteConnectOptions::new()
            .filename(db_file)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        for ddl in [
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
            "CREATE TABLE audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system', action TEXT NOT NULL,
                entity_type TEXT, entity_id TEXT, details_json TEXT) STRICT",
            "CREATE TABLE backup_history (id TEXT PRIMARY KEY NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
                target_path TEXT NOT NULL, file_hash_sha256 TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL, is_encrypted INTEGER NOT NULL DEFAULT 1,
                retention_tag TEXT NOT NULL, trigger_reason TEXT NOT NULL,
                db_schema_version INTEGER NOT NULL, app_version TEXT NOT NULL,
                verified_at TEXT) STRICT",
            "CREATE TABLE marker (id INTEGER PRIMARY KEY, v TEXT) STRICT",
            "INSERT INTO marker (v) VALUES ('original-data')",
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn backup_then_decrypt_roundtrip() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let outcome = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();
        let file_bytes = std::fs::read(&outcome.file_path).unwrap();

        // Korrekte Passphrase → Content entschlüsselbar, enthält DB.
        let (_m, content) = decrypt_backup(&file_bytes, "passphrase-1234567890").unwrap();
        let staging = dir.path().join("stg");
        extract_zip(&content, &staging).unwrap();
        assert!(staging.join("klein-buch.sqlite").is_file());

        // Falsche Passphrase → Fehler.
        assert!(decrypt_backup(&file_bytes, "falsch").is_err());
    }

    #[tokio::test]
    async fn tampered_backup_fails() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let outcome = backup::create_now(&pool, &paths, "pw-abcdef", "manual")
            .await
            .unwrap();
        let mut file_bytes = std::fs::read(&outcome.file_path).unwrap();
        // Letztes Byte (im Ciphertext-Bereich) kippen → GCM-Auth schlägt an.
        let last = file_bytes.len() - 1;
        file_bytes[last] ^= 0x01;
        assert!(decrypt_backup(&file_bytes, "pw-abcdef").is_err());
    }

    #[test]
    fn extract_zip_rejects_traversal() {
        // Manuell einen ZIP mit bösartigem Pfad bauen.
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = zip::write::SimpleFileOptions::default();
        use std::io::Write;
        zw.start_file("../evil.txt", opts).unwrap();
        zw.write_all(b"x").unwrap();
        let bytes = zw.finish().unwrap().into_inner();
        let dir = TempDir::new().unwrap();
        assert!(extract_zip(&bytes, dir.path()).is_err());
    }

    #[test]
    fn apply_pending_swaps_db_and_archive() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::create_dir_all(paths.archive_dir.join("2025")).unwrap();
        // Alte DB + altes Archive.
        fs::write(&paths.db_file, b"OLD-DB").unwrap();
        fs::write(paths.archive_dir.join("2025/old.pdf"), b"old").unwrap();

        // Staging mit neuer DB + neuem Archive.
        let staging = paths.data_dir.join(RESTORE_STAGING);
        fs::create_dir_all(staging.join("archive/2026")).unwrap();
        fs::write(staging.join("klein-buch.sqlite"), b"NEW-DB").unwrap();
        fs::write(staging.join("archive/2026/new.pdf"), b"new").unwrap();

        // Marker.
        let marker = paths.data_dir.join(RESTORE_MARKER);
        let info = serde_json::json!({"staging": staging.to_string_lossy(), "source":"x.kbk", "staged_at":"t"});
        fs::write(&marker, serde_json::to_vec(&info).unwrap()).unwrap();

        let applied = apply_pending(&paths).unwrap();
        assert!(applied.is_some());
        assert_eq!(fs::read(&paths.db_file).unwrap(), b"NEW-DB");
        assert!(paths.archive_dir.join("2026/new.pdf").is_file());
        assert!(!paths.archive_dir.join("2025/old.pdf").exists());
        assert!(!marker.exists());
    }

    #[test]
    fn apply_pending_noop_without_marker() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.data_dir).unwrap();
        assert!(apply_pending(&paths).unwrap().is_none());
    }

    /// G1-HARDEN.1: voller Backup→Restore-Roundtrip über die echte Pipeline
    /// (`apply` → Neustart-Simulation → `apply_pending`) mit nicht-trivialem
    /// Datenvolumen (1000 Zeilen). Verifiziert: (a) der **identische** Stand
    /// wird wiederhergestellt, (b) das **Pre-Restore-Backup wird erzwungen**
    /// erstellt.
    #[tokio::test]
    async fn full_restore_roundtrip_restores_identical_state() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        // Nicht-triviales Volumen.
        sqlx::query("CREATE TABLE bulk (id INTEGER PRIMARY KEY, payload TEXT NOT NULL) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        for i in 0..1000i64 {
            sqlx::query("INSERT INTO bulk (id, payload) VALUES (?, ?)")
                .bind(i)
                .bind(format!("row-{i}"))
                .execute(&pool)
                .await
                .unwrap();
        }

        // Original-Stand sichern (manuelles Backup).
        let original = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();

        // Stand verändern — genau das soll der Restore zurückdrehen.
        sqlx::query("UPDATE marker SET v = 'mutated'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM bulk WHERE id >= 500")
            .execute(&pool)
            .await
            .unwrap();

        // Restore Phase A — verlangt entsperrte Session (für das Pre-Restore-Backup).
        let session = backup::BackupSession::default();
        session.set("passphrase-1234567890".to_string());
        let report = apply(
            &pool,
            &paths,
            &session,
            Path::new(&original.file_path),
            "passphrase-1234567890",
        )
        .await
        .unwrap();

        // Pre-Restore-Backup wurde erzwungen erstellt + Staging/Marker liegen vor.
        assert!(report.requires_restart);
        assert!(
            Path::new(&report.pre_restore_backup_path).is_file(),
            "Pre-Restore-Backup muss als Datei existieren"
        );
        assert!(paths.data_dir.join(RESTORE_MARKER).exists());
        assert!(paths
            .data_dir
            .join(RESTORE_STAGING)
            .join("klein-buch.sqlite")
            .is_file());

        // Neustart simulieren: Pool schließen, dann Phase B (vor Pool-Open).
        pool.close().await;
        drop(pool);
        assert!(apply_pending(&paths).unwrap().is_some());

        // DB neu öffnen (ohne DDL) → Stand muss IDENTISCH zum Original sein.
        let opts = SqliteConnectOptions::new()
            .filename(&paths.db_file)
            .create_if_missing(false);
        let pool2 = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        let v: String = sqlx::query_scalar("SELECT v FROM marker")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(
            v, "original-data",
            "Restore muss den Original-Stand herstellen"
        );
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bulk")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(n, 1000, "alle 1000 Zeilen müssen wiederhergestellt sein");
    }

    /// G1-HARDEN.1: ohne entsperrte Session bricht der Restore ab — das
    /// Pre-Restore-Backup (mit der aktuellen Passphrase) ist nicht erstellbar,
    /// also darf nicht restauriert werden.
    #[tokio::test]
    async fn apply_rejects_when_session_locked() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let original = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();

        // Session NICHT entsperrt.
        let session = backup::BackupSession::default();
        let err = apply(
            &pool,
            &paths,
            &session,
            Path::new(&original.file_path),
            "passphrase-1234567890",
        )
        .await
        .unwrap_err();
        assert!(
            format!("{err}").contains("entsperr"),
            "erwartete Entsperr-Hinweis, got: {err}"
        );
        // Kein Marker geschrieben — der Restore wurde sauber abgebrochen.
        assert!(!paths.data_dir.join(RESTORE_MARKER).exists());
    }
}
