//! Migrations-Export-Orchestrierung (Block 4).
//!
//! Erzeugt ein **unverschlüsseltes** ZIP für den Worst-Case (Klein.Buch wird
//! eingestellt / Tool-Wechsel). Inhalt:
//!
//! ```text
//! tables/<table>.json   ← ein JSON-Array pro Tabelle (alle Zeilen)
//! archive/<jahr>/<...>  ← alle Original-Belege (PDF/XML)
//! schema/NNNN_*.sql     ← alle Migrationen (aus dem eingebetteten MIGRATOR)
//! schema/erd.md         ← Tabellen + CREATE-Statements
//! manifest.json         ← App-/Schema-Version, Zeitstempel, Counts, Hashes
//! README.md             ← Erklärung
//! read_export.py        ← Standalone-Reader (nur Python-Stdlib)
//! ```
//!
//! Im Gegensatz zum Backup ist dieser Export bewusst **offen lesbar** — sein
//! Zweck ist Daten-Portabilität, nicht Vertraulichkeit.

use crate::config::Paths;
use crate::db::repo::audit_log;
use crate::error::{Error, Result};
use crate::migration_export::json_dump;
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub zip_path: String,
    pub table_count: usize,
    pub total_rows: i64,
    pub archive_file_count: usize,
    /// R4-004: Anzahl der Archive-Dateien, deren neu berechneter SHA-256 von
    /// dem in `archive_entries.file_hash_sha256` gespeicherten Soll-Hash
    /// abweicht (Bitrot / Tamper). Pro Treffer wird zusätzlich ein
    /// `migration.export.tamper_detected`-Audit-Event geschrieben. **0** bei
    /// einem sauberen Export — von außen sichtbar als „integrity check passed".
    pub archive_tamper_count: usize,
    pub zip_size_bytes: i64,
}

/// Manifest-Eintrag pro Archive-Datei (R4-005).
#[derive(Debug, Clone)]
struct ArchiveFileEntry {
    rel_path: String,
    sha256: String,
    /// `Some(true)` = DB-Hash existiert + weicht ab (Tamper);
    /// `Some(false)` = DB-Hash existiert + matched (clean);
    /// `None` = kein DB-Eintrag für diesen Pfad (z. B. legacy-Datei,
    /// vor `archive_entries`-Migration im Filesystem hinterlassen).
    tamper: Option<bool>,
}

/// Standalone-Reader (Python-Stdlib, kein pip nötig).
const READER_PY: &str = r#"#!/usr/bin/env python3
"""Standalone-Reader für den Klein.Buch-Migrations-Export.

Nutzung:
    python3 read_export.py                # listet Tabellen + Zeilenzahlen
    python3 read_export.py <tabelle>      # gibt die Zeilen einer Tabelle aus

Benötigt nur die Python-Standardbibliothek. Die Archive-Dateien liegen
unverändert unter archive/. Das Schema (DDL) liegt unter schema/.
"""
import json, sys, pathlib

BASE = pathlib.Path(__file__).resolve().parent
TABLES = BASE / "tables"

def load(table):
    with open(TABLES / f"{table}.json", encoding="utf-8") as f:
        return json.load(f)

def main():
    if len(sys.argv) == 1:
        for p in sorted(TABLES.glob("*.json")):
            rows = json.load(open(p, encoding="utf-8"))
            print(f"{p.stem:30s} {len(rows):8d} rows")
        return
    table = sys.argv[1]
    rows = load(table)
    print(json.dumps(rows, indent=2, ensure_ascii=False))

if __name__ == "__main__":
    main()
"#;

/// Vollständigen Export erzeugen.
///
/// **R4-Hardening (Block R4-Review 2026-05-27):**
/// - **R4-004:** Archive-Dateien werden gegen ihren in `archive_entries`
///   gespeicherten Soll-SHA-256 verifiziert. Bei Bitrot/Tamper wird ein
///   `migration.export.tamper_detected`-Audit-Event geschrieben (Export
///   läuft trotzdem weiter, Tamper-Flag im Manifest — best-effort-Recovery
///   für den Steuerprüfer, keine stille Lücke).
/// - **R4-005:** Manifest enthält jetzt `archive_file_hashes` (pro Datei).
/// - **R4-006:** Alle Tabellen-Dumps + ERD laufen innerhalb **einer**
///   `BEGIN IMMEDIATE`-Transaktion → konsistenter Point-in-Time-Snapshot.
pub async fn export_all(
    pool: &sqlx::SqlitePool,
    paths: &Paths,
    target_zip_path: &Path,
) -> Result<ExportReport> {
    if let Some(parent) = target_zip_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(target_zip_path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // ----- R4-006: TX-Snapshot über alle DB-Reads -----
    // Eine eigene Connection für den gesamten Snapshot; `BEGIN IMMEDIATE`
    // fixiert den Read-Stand gegen parallele Writer (WAL-Modus erlaubt
    // gleichzeitige Reader, aber ohne TX würden wir pro Tabelle eigene
    // Snapshots erhalten → potenzielle Inkonsistenz zwischen z. B.
    // `invoices` und `invoice_items`).
    let mut conn = pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    // 1. Tabellen.
    let tables = json_dump::list_tables_in_conn(&mut conn).await?;
    let mut table_counts: Vec<(String, i64)> = Vec::new();
    let mut table_hashes: Vec<(String, String)> = Vec::new();
    let mut total_rows: i64 = 0;
    for table in &tables {
        let json_str = json_dump::dump_table_json_in_conn(&mut conn, table).await?;
        let count = json_dump::row_count_in_conn(&mut conn, table).await?;
        total_rows += count;
        table_counts.push((table.clone(), count));
        table_hashes.push((table.clone(), sha256_hex(json_str.as_bytes())));
        zw.start_file(format!("tables/{table}.json"), opts)?;
        zw.write_all(json_str.as_bytes())?;
    }

    // 2. Archive-Dateien — R4-004 + R4-005: pro Datei Hash bilden + ggf.
    //    gegen DB-Hash verifizieren + ins Manifest aufnehmen.
    let archive_files = collect_files(&paths.archive_dir)?;
    let mut archive_entries: Vec<ArchiveFileEntry> = Vec::with_capacity(archive_files.len());
    let mut tamper_details: Vec<(String, String, String)> = Vec::new(); // (path, expected, actual)
    for (rel, abs) in &archive_files {
        let bytes = std::fs::read(abs)?;
        let actual_hash = sha256_hex(&bytes);
        let abs_str = abs.to_string_lossy().to_string();
        let expected_hash = json_dump::archive_hash_for_path(&mut conn, &abs_str).await?;
        let tamper = match expected_hash.as_deref() {
            Some(h) if h == actual_hash => Some(false),
            Some(h) => {
                tamper_details.push((rel.clone(), h.to_string(), actual_hash.clone()));
                Some(true)
            }
            None => None,
        };
        zw.start_file(format!("archive/{rel}"), opts)?;
        zw.write_all(&bytes)?;
        archive_entries.push(ArchiveFileEntry {
            rel_path: rel.clone(),
            sha256: actual_hash,
            tamper,
        });
    }
    let archive_tamper_count = tamper_details.len();

    // 3. Schema (Migrationen aus dem eingebetteten MIGRATOR — read-only,
    //    nicht aus der DB; bleibt außerhalb der TX-Snapshot-Logik).
    for m in crate::db::MIGRATOR.iter() {
        let fname = format!(
            "schema/{:04}_{}.sql",
            m.version,
            slugify(m.description.as_ref())
        );
        zw.start_file(fname, opts)?;
        zw.write_all(m.sql.as_bytes())?;
    }

    // 4. ERD-Markdown (innerhalb des TX-Snapshots).
    let erd = build_erd_in_conn(&mut conn).await?;
    zw.start_file("schema/erd.md", opts)?;
    zw.write_all(erd.as_bytes())?;

    // TX-Snapshot abschließen — alle DB-Reads sind durch.
    sqlx::query("COMMIT").execute(&mut *conn).await?;
    drop(conn);

    // 5. Manifest — R4-005: `archive_file_hashes` zusätzlich aufgenommen.
    let exported_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let manifest = json!({
        "tool": "Klein.Buch",
        "app_version": env!("CARGO_PKG_VERSION"),
        "schema_version": crate::db::schema_version::EXPECTED_SCHEMA_VERSION,
        "exported_at": exported_at,
        "tables": table_counts.iter().map(|(n, c)| json!({"name": n, "rows": c})).collect::<Vec<_>>(),
        "table_hashes_sha256": table_hashes.iter().map(|(n, h)| json!({"name": n, "sha256": h})).collect::<Vec<_>>(),
        "archive_file_count": archive_files.len(),
        "archive_file_hashes": archive_entries.iter().map(|e| {
            let mut obj = json!({"path": format!("archive/{}", e.rel_path), "sha256": e.sha256});
            if let Some(tamper) = e.tamper {
                obj["tamper"] = json!(tamper);
            }
            obj
        }).collect::<Vec<_>>(),
        "archive_tamper_count": archive_tamper_count,
    });
    zw.start_file("manifest.json", opts)?;
    zw.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    // 6. README.
    zw.start_file("README.md", opts)?;
    zw.write_all(build_readme(&exported_at).as_bytes())?;

    // 7. Reader-Script.
    zw.start_file("read_export.py", opts)?;
    zw.write_all(READER_PY.as_bytes())?;

    let file = zw.finish()?;
    let zip_size = file.metadata()?.len() as i64;

    // R4-004: Tamper-Audit-Events NACH COMMIT (TX war read-only Snapshot).
    // GoBD-Audit-Trail: pro Tamper ein Event mit Pfad + erwartetem/gefundenem
    // Hash. Audit-Append-Fehler propagieren (R1-010-Pattern).
    for (rel, expected, actual) in &tamper_details {
        audit_log::append(
            pool,
            "migration.export.tamper_detected",
            "archive",
            rel,
            Some(&format!(
                r#"{{"expected":"{expected}","actual":"{actual}"}}"#
            )),
        )
        .await?;
    }

    Ok(ExportReport {
        zip_path: target_zip_path.to_string_lossy().to_string(),
        table_count: tables.len(),
        total_rows,
        archive_file_count: archive_files.len(),
        archive_tamper_count,
        zip_size_bytes: zip_size,
    })
}

async fn build_erd_in_conn(conn: &mut sqlx::sqlite::SqliteConnection) -> Result<String> {
    let stmts = json_dump::create_statements_in_conn(conn).await?;
    let mut s = String::new();
    s.push_str("# Klein.Buch — Schema (ERD)\n\n");
    s.push_str("Tabellen und ihre `CREATE`-Statements. Fremdschlüssel sind in den\n");
    s.push_str("`REFERENCES`-Klauseln der jeweiligen Spalten dokumentiert.\n\n");
    for (name, sql) in &stmts {
        s.push_str(&format!("## {name}\n\n```sql\n{sql}\n```\n\n"));
    }
    Ok(s)
}

fn build_readme(exported_at: &str) -> String {
    format!(
        "# Klein.Buch — Datenexport\n\n\
        Erstellt: {exported_at}\n\n\
        Dieser Export enthält **alle** in Klein.Buch gespeicherten Daten in offenen,\n\
        werkzeugunabhängigen Formaten.\n\n\
        ## Inhalt\n\n\
        - `tables/<tabelle>.json` — eine JSON-Datei pro Datenbanktabelle (Array von Zeilen-Objekten).\n\
        - `archive/` — alle Original-Belege (PDF/XML), unverändert (GoBD-Original).\n\
        - `schema/NNNN_*.sql` — alle Datenbank-Migrationen (DDL).\n\
        - `schema/erd.md` — Tabellenübersicht mit CREATE-Statements.\n\
        - `manifest.json` — App-/Schema-Version, Zeitstempel, Zeilenzahlen, Tabellen-Hashes.\n\
        - `read_export.py` — Standalone-Reader (nur Python-Standardbibliothek).\n\n\
        ## Lesen\n\n\
        ```sh\n\
        python3 read_export.py            # Tabellen + Zeilenzahlen\n\
        python3 read_export.py invoices   # Inhalt einer Tabelle\n\
        ```\n\n\
        Beträge sind in **Cent** (Integer) gespeichert. Datums-/Zeitfelder sind\n\
        ISO-8601. BLOB-Werte (falls vorhanden) erscheinen als `hex:<...>`.\n"
    )
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn collect_files(base: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    if !base.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .map_err(|_| Error::Backup("strip_prefix beim Export".into()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, path));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;
    use std::io::Read;
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

    async fn demo_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE contacts (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO contacts (id, name) VALUES ('c1','ACME')")
            .execute(&pool)
            .await
            .unwrap();
        // R4-004: `archive_entries` für Tamper-Lookup beim Export.
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
        // audit_log für Tamper-Events.
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
        pool
    }

    #[tokio::test]
    async fn export_contains_tables_archive_and_meta() {
        let pool = demo_pool().await;
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(paths.archive_dir.join("2026/invoices/pdf")).unwrap();
        std::fs::write(
            paths.archive_dir.join("2026/invoices/pdf/RE-2026-0001.pdf"),
            b"pdf",
        )
        .unwrap();

        let zip_path = dir.path().join("export.zip");
        let report = export_all(&pool, &paths, &zip_path).await.unwrap();
        // Drei Tabellen: contacts, archive_entries, audit_log.
        assert_eq!(report.table_count, 3);
        assert!(report.total_rows >= 1);
        assert_eq!(report.archive_file_count, 1);
        // R4-004: keine `archive_entries`-Zeile für die Datei → tamper = None,
        // archive_tamper_count = 0.
        assert_eq!(report.archive_tamper_count, 0);
        assert!(zip_path.is_file());

        let mut archive = zip::ZipArchive::new(File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"tables/contacts.json".to_string()));
        assert!(names.contains(&"archive/2026/invoices/pdf/RE-2026-0001.pdf".to_string()));
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"README.md".to_string()));
        assert!(names.contains(&"read_export.py".to_string()));
        assert!(names.iter().any(|n| n == "schema/erd.md"));

        // tables/contacts.json muss die Zeile enthalten.
        let mut f = archive.by_name("tables/contacts.json").unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert!(s.contains("ACME"));
    }
}
