//! Konsistenter Backup-Content (Block 4).
//!
//! Baut den **unverschlüsselten** Content-ZIP, der danach von
//! [`crate::backup::encrypt`] verschlüsselt wird:
//!
//! ```text
//! klein-buch.sqlite        ← konsistenter DB-Snapshot (WAL-Checkpoint + Datei)
//! archive/<jahr>/<...>     ← write-once GoBD-Archiv (PDF/XML/Belege)
//! inputs/branding/<...>    ← Logo/Branding (menschen-maintained, klein)
//! ```
//!
//! DB-Snapshot: zuerst `PRAGMA wal_checkpoint(TRUNCATE)` (schreibt die WAL in
//! die Haupt-Datei zurück), dann die DB-Datei direkt lesen. Das ist robust über
//! Plattformen/Treiber hinweg (`VACUUM INTO` darf nicht in einer Transaktion
//! laufen und verhielt sich über den sqlx-Pool unzuverlässig) und für eine
//! Single-User-Desktop-App mit Backups an Lock-Punkten konsistent.
//!
//! Schicht: **Imperative Shell** (I/O). Reine Zip-Bytes raus, plus SHA-256.

use crate::config::Paths;
use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

/// Ergebnis des Snapshots: die ZIP-Bytes + ihr SHA-256 (hex).
pub struct Content {
    pub zip_bytes: Vec<u8>,
    pub sha256_hex: String,
}

/// Erzeugt den Content-ZIP. Reihenfolge der Einträge ist deterministisch
/// (sortierte relative Pfade), damit Tests stabil sind.
pub async fn build_content_zip(pool: &SqlitePool, paths: &Paths) -> Result<Content> {
    let db_bytes = read_db_snapshot_bytes(pool, &paths.db_file).await?;

    let archive_files = collect_files(&paths.archive_dir)?;
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_files = collect_files(&branding_dir)?;

    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zw.start_file("klein-buch.sqlite", opts)?;
    zw.write_all(&db_bytes)?;

    for (rel, abs) in &archive_files {
        zw.start_file(format!("archive/{rel}"), opts)?;
        let bytes = fs::read(abs)?;
        zw.write_all(&bytes)?;
    }
    for (rel, abs) in &branding_files {
        zw.start_file(format!("inputs/branding/{rel}"), opts)?;
        let bytes = fs::read(abs)?;
        zw.write_all(&bytes)?;
    }

    let cursor = zw.finish()?;
    let zip_bytes = cursor.into_inner();
    let sha256_hex = sha256_hex(&zip_bytes);
    Ok(Content {
        zip_bytes,
        sha256_hex,
    })
}

/// Konsistenter DB-Snapshot: WAL in die Haupt-Datei checkpointen, dann die
/// Datei lesen. `wal_checkpoint(TRUNCATE)` ist No-op bei Rollback-Journal und
/// schadet bei `:memory:` nicht (best-effort, Fehler werden ignoriert). Das
/// Lesen einer geöffneten SQLite-Datei ist unter Shared-Read erlaubt.
async fn read_db_snapshot_bytes(pool: &SqlitePool, db_file: &Path) -> Result<Vec<u8>> {
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await;
    let bytes = fs::read(db_file).map_err(|e| {
        Error::Backup(format!(
            "DB-Snapshot: {} konnte nicht gelesen werden: {e}",
            db_file.display()
        ))
    })?;
    Ok(bytes)
}

/// Sammelt alle Dateien unter `base` rekursiv. Rückgabe: (relativer Pfad mit
/// `/`-Trenner, absoluter Pfad), sortiert. Existiert `base` nicht → leer.
fn collect_files(base: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    if !base.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|_| Error::Backup("strip_prefix beim Snapshot".into()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, path));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    async fn file_pool(db_file: &Path) -> SqlitePool {
        let opts = SqliteConnectOptions::new()
            .filename(db_file)
            .create_if_missing(true);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn snapshot_contains_db_and_archive_files() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        let pool = file_pool(&paths.db_file).await;
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t (v) VALUES ('hallo')")
            .execute(&pool)
            .await
            .unwrap();

        // Eine Archive-Datei und eine Branding-Datei anlegen.
        fs::create_dir_all(paths.archive_dir.join("2026/invoices/pdf")).unwrap();
        fs::write(
            paths.archive_dir.join("2026/invoices/pdf/RE-2026-0001.pdf"),
            b"pdf-bytes",
        )
        .unwrap();
        fs::create_dir_all(paths.inputs_dir.join("branding")).unwrap();
        fs::write(paths.inputs_dir.join("branding/logo.png"), b"png").unwrap();

        let content = build_content_zip(&pool, &paths).await.unwrap();
        assert!(!content.zip_bytes.is_empty());
        assert_eq!(content.sha256_hex.len(), 64);

        // ZIP wieder öffnen und Inhalt prüfen.
        let mut archive = zip::ZipArchive::new(Cursor::new(content.zip_bytes.clone())).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"klein-buch.sqlite".to_string()));
        assert!(names.contains(&"archive/2026/invoices/pdf/RE-2026-0001.pdf".to_string()));
        assert!(names.contains(&"inputs/branding/logo.png".to_string()));

        // Der eingebettete DB-Snapshot muss eine valide SQLite-Datei sein.
        use std::io::Read;
        let mut f = archive.by_name("klein-buch.sqlite").unwrap();
        let mut db = Vec::new();
        f.read_to_end(&mut db).unwrap();
        assert!(
            db.starts_with(b"SQLite format 3\0"),
            "Snapshot ist keine SQLite-Datei"
        );
    }

    #[tokio::test]
    async fn snapshot_works_without_archive_dir() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path()); // archive/inputs existieren nicht
        let pool = file_pool(&paths.db_file).await;
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        let content = build_content_zip(&pool, &paths).await.unwrap();
        let archive = zip::ZipArchive::new(Cursor::new(content.zip_bytes)).unwrap();
        assert!(!archive.is_empty());
    }
}
