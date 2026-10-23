//! Konsistenter Backup-Content (Block 4).
//!
//! Baut den **unverschlüsselten** Content-ZIP, der danach von
//! [`crate::backup::encrypt`] verschlüsselt wird:
//!
//! ```text
//! klein-buch.sqlite        ← konsistenter DB-Snapshot (WAL-Checkpoint + Datei)
//! archive/<jahr>/<...>     ← write-once GoBD-Archiv (PDF/XML/Belege)
//! branding/<...>           ← hochgeladenes Logo + Unterschrift aus `data/branding`
//! ```
//!
//! Format 2 (Plan W1.5): `branding/` kommt aus `data_dir/branding` — dort legt
//! die App Logo und Unterschrift ab ([`crate::branding`]). Format 1 sicherte
//! stattdessen `inputs/branding/` (menschen-gepflegt, nicht Teil der Daten).
//!
//! DB-Snapshot (Plan W2.3): `VACUUM INTO '<data_dir>/tmp/snapshot-<uuid>.db'`
//! über eine **eigene** Verbindung (nicht über den Pool — `VACUUM INTO` darf
//! nicht in einer Transaktion laufen). SQLite liest dabei einen konsistenten
//! Stand: alle vor dem Start committeten Transaktionen, nichts Halbes, auch
//! bei parallelem Schreiber (WAL). Danach Datei lesen und löschen. Bei einer
//! verschlüsselten DB öffnet die Verbindung mit der Passphrase; liefert
//! SQLCipher eine unverschlüsselte Kopie, greift der Fallback
//! `ATTACH … KEY` + `sqlcipher_export` — ein Backup-Inhalt mit Klartext-DB
//! entsteht nie (er wird zwar zusätzlich verschlüsselt, aber die DB im
//! entschlüsselten ZIP soll wie das Original nur mit Passphrase lesbar sein).
//!
//! Temp-Dateien liegen unter `<data_dir>/tmp`, nie unter `/tmp` (im Container
//! ein RAM-tmpfs).
//!
//! Schicht: **Imperative Shell** (I/O). Reine Zip-Bytes raus, plus SHA-256.

use crate::config::Paths;
use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Ergebnis des Snapshots: die ZIP-Bytes + ihr SHA-256 (hex).
pub struct Content {
    pub zip_bytes: Vec<u8>,
    pub sha256_hex: String,
}

/// Erzeugt den Content-ZIP. Reihenfolge der Einträge ist deterministisch
/// (sortierte relative Pfade), damit Tests stabil sind.
///
/// `key`: Passphrase, wenn die DB verschlüsselt ist (`None` bei Klartext-DB,
/// z. B. in Tests). Das Packen (Lesen + Deflate) läuft in `spawn_blocking`
/// (Plan W2.6), damit es die Runtime nicht blockiert.
pub async fn build_content_zip(paths: &Paths, key: Option<&str>) -> Result<Content> {
    let db_bytes = read_db_snapshot_bytes(paths, key).await?;
    let archive_dir = paths.archive_dir.clone();
    let branding_dir = crate::branding::branding_dir(&paths.data_dir);
    tokio::task::spawn_blocking(move || zip_content(db_bytes, &archive_dir, &branding_dir))
        .await
        .map_err(|e| Error::Backup(format!("Backup-Packen abgebrochen: {e}")))?
}

fn zip_content(db_bytes: Vec<u8>, archive_dir: &Path, branding_dir: &Path) -> Result<Content> {
    let archive_files = collect_files(archive_dir)?;
    let branding_files = collect_files(branding_dir)?;

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
        zw.start_file(format!("branding/{rel}"), opts)?;
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

/// Verzeichnis für Temp-Dateien unterhalb von `data_dir` (nie `/tmp`).
pub fn tmp_dir(paths: &Paths) -> PathBuf {
    paths.data_dir.join("tmp")
}

/// Verbindung auf eine DB-Datei, ohne Statement-Logging (die Passphrase steht
/// im `PRAGMA key`, Plan W5.9).
async fn connect_file(file: &Path, key: Option<&str>) -> Result<sqlx::SqliteConnection> {
    let url = format!("sqlite://{}", file.to_string_lossy());
    let mut opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .log_statements(log::LevelFilter::Off)
        .log_slow_statements(log::LevelFilter::Off, std::time::Duration::from_secs(3600));
    if let Some(k) = key {
        opts = opts.pragma("key", crate::db::sqlcipher_key_literal(k));
    }
    Ok(opts.connect().await?)
}

fn remove_db_files(file: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut p = file.as_os_str().to_os_string();
        p.push(suffix);
        let _ = fs::remove_file(PathBuf::from(p));
    }
}

/// Konsistenter DB-Snapshot (Plan W2.3), siehe Modul-Doku.
async fn read_db_snapshot_bytes(paths: &Paths, key: Option<&str>) -> Result<Vec<u8>> {
    let dir = tmp_dir(paths);
    fs::create_dir_all(&dir)?;
    let snap = dir.join(format!("snapshot-{}.db", uuid::Uuid::now_v7().simple()));
    let result = snapshot_into(&paths.db_file, &snap, key)
        .await
        .and_then(|()| {
            fs::read(&snap).map_err(|e| {
                Error::Backup(format!(
                    "DB-Snapshot: {} konnte nicht gelesen werden: {e}",
                    snap.display()
                ))
            })
        });
    remove_db_files(&snap);
    result
}

async fn snapshot_into(db_file: &Path, snap: &Path, key: Option<&str>) -> Result<()> {
    if !db_file.is_file() {
        return Err(Error::Backup(format!(
            "DB-Snapshot: {} fehlt",
            db_file.display()
        )));
    }
    let snap_str = snap.to_string_lossy().to_string();
    let mut conn = connect_file(db_file, key).await?;
    sqlx::query("VACUUM INTO ?")
        .bind(&snap_str)
        .execute(&mut conn)
        .await?;
    conn.close().await?;

    if let Some(k) = key {
        if crate::db::db_file_is_plaintext(snap) {
            // SQLCipher hat eine Klartext-Kopie geschrieben → verwerfen und per
            // ATTACH … KEY + sqlcipher_export verschlüsselt neu erzeugen.
            remove_db_files(snap);
            let mut conn = connect_file(db_file, Some(k)).await?;
            let attach = format!(
                "ATTACH DATABASE ? AS snapshot KEY {}",
                crate::db::sqlcipher_key_literal(k)
            );
            sqlx::query(&attach)
                .bind(&snap_str)
                .execute(&mut conn)
                .await?;
            sqlx::query("SELECT sqlcipher_export('snapshot')")
                .fetch_all(&mut conn)
                .await?;
            sqlx::query("DETACH DATABASE snapshot")
                .execute(&mut conn)
                .await?;
            conn.close().await?;
            if crate::db::db_file_is_plaintext(snap) {
                return Err(Error::Backup(
                    "DB-Snapshot ist unverschlüsselt — Backup abgebrochen".into(),
                ));
            }
        }
    }
    Ok(())
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
    use sqlx::SqlitePool;
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
        // Format 2 (W1.5): Logo/Unterschrift aus data/branding, nicht aus inputs/.
        fs::create_dir_all(paths.data_dir.join("branding")).unwrap();
        fs::write(paths.data_dir.join("branding/logo.png"), b"png").unwrap();
        fs::write(paths.data_dir.join("branding/signature.png"), b"sig").unwrap();
        fs::create_dir_all(paths.inputs_dir.join("branding")).unwrap();
        fs::write(paths.inputs_dir.join("branding/logo-vorlage.png"), b"png").unwrap();

        let content = build_content_zip(&paths, None).await.unwrap();
        assert!(!content.zip_bytes.is_empty());
        assert_eq!(content.sha256_hex.len(), 64);

        // ZIP wieder öffnen und Inhalt prüfen.
        let mut archive = zip::ZipArchive::new(Cursor::new(content.zip_bytes.clone())).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"klein-buch.sqlite".to_string()));
        assert!(names.contains(&"archive/2026/invoices/pdf/RE-2026-0001.pdf".to_string()));
        assert!(names.contains(&"branding/logo.png".to_string()));
        assert!(names.contains(&"branding/signature.png".to_string()));
        assert!(
            !names.iter().any(|n| n.starts_with("inputs/")),
            "inputs/ gehört nicht ins Backup: {names:?}"
        );

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

    fn db_from_zip(zip_bytes: &[u8]) -> Vec<u8> {
        use std::io::Read;
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes.to_vec())).unwrap();
        let mut f = archive.by_name("klein-buch.sqlite").unwrap();
        let mut db = Vec::new();
        f.read_to_end(&mut db).unwrap();
        db
    }

    /// Plan W2.3: der Snapshot einer verschlüsselten DB ist selbst verschlüsselt
    /// (beginnt nicht mit `SQLite format 3`) und öffnet mit derselben Passphrase.
    /// Danach liegt nichts mehr unter `<data_dir>/tmp`.
    #[tokio::test]
    async fn snapshot_of_encrypted_db_is_encrypted_and_opens_with_same_passphrase() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        let pass = "korrekt-pferd-batterie-heftklammer";
        let pool = crate::db::open_pool(&paths.db_file, Some(pass))
            .await
            .unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t (v) VALUES ('hallo')")
            .execute(&pool)
            .await
            .unwrap();
        assert!(!crate::db::db_file_is_plaintext(&paths.db_file));

        let content = build_content_zip(&paths, Some(pass)).await.unwrap();
        let db = db_from_zip(&content.zip_bytes);
        assert!(
            !db.starts_with(b"SQLite format 3"),
            "Snapshot ist unverschlüsselt"
        );

        let restored = dir.path().join("restored.sqlite");
        fs::write(&restored, &db).unwrap();
        let check = crate::db::open_pool(&restored, Some(pass)).await.unwrap();
        let v: String = sqlx::query_scalar("SELECT v FROM t")
            .fetch_one(&check)
            .await
            .unwrap();
        assert_eq!(v, "hallo");
        check.close().await;
        let leftovers: Vec<_> = fs::read_dir(tmp_dir(&paths)).unwrap().collect();
        assert!(leftovers.is_empty(), "Temp-Reste unter data/tmp");
    }

    /// Plan W2.3: Backup bei parallelem Schreiber enthält den letzten Commit vor
    /// dem Start, aber nichts aus einer noch offenen Transaktion.
    #[tokio::test]
    async fn snapshot_with_parallel_writer_contains_committed_state_only() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        let pool = crate::db::open_pool(&paths.db_file, None).await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        for i in 0..3 {
            sqlx::query("INSERT INTO t (v) VALUES (?)")
                .bind(format!("fest-{i}"))
                .execute(&pool)
                .await
                .unwrap();
        }
        // Paralleler Schreiber mit offener Transaktion (hält die Schreibsperre).
        let mut tx = crate::db::begin_immediate(&pool).await.unwrap();
        sqlx::query("INSERT INTO t (v) VALUES ('offen')")
            .execute(&mut *tx)
            .await
            .unwrap();

        let content = build_content_zip(&paths, None).await.unwrap();
        tx.commit().await.unwrap();

        let restored = dir.path().join("restored.sqlite");
        fs::write(&restored, db_from_zip(&content.zip_bytes)).unwrap();
        let check = crate::db::open_pool(&restored, None).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&check)
            .await
            .unwrap();
        assert_eq!(n, 3, "nur die committeten Zeilen");
        check.close().await;
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
        let content = build_content_zip(&paths, None).await.unwrap();
        let archive = zip::ZipArchive::new(Cursor::new(content.zip_bytes)).unwrap();
        assert!(!archive.is_empty());
    }
}
