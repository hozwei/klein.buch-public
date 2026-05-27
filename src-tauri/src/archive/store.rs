//! Write-once-Archivierung von Belegen (PDF, XML, JSON-Manifeste).
//!
//! Schicht: **Imperative Shell**. Alle I/O passiert hier; das Hashing ist
//! pure (deterministisch) und wird zusammen mit Datei + DB-Eintrag in
//! einem Aufruf gekapselt.
//!
//! ## Verträge (GoBD-Hardline)
//!
//! 1. **Idempotente Hash-Berechnung** vor dem Schreiben (SHA-256 über die
//!    übergebenen Bytes).
//! 2. **Atomische Sichtbarkeit**: zuerst in eine `.tmp`-Datei schreiben,
//!    dann `rename` → erst dann existiert die finale Datei.
//! 3. **Read-only** nach Schreiben: `set_readonly(true)` (Windows: setzt
//!    `FILE_ATTRIBUTE_READONLY`; Unix: zusätzlich Mode `0o400`).
//! 4. **Eintrag in `archive_entries`** mit identem Hash, Größe und Pfad.
//!    Der Pfad ist `UNIQUE` — doppelte Schreibversuche schlagen am
//!    Index fehl, nicht am Filesystem.
//! 5. **Audit-Log-Eintrag** "archive.store" über `archive::audit`.
//!
//! ## Layout
//!
//! ```text
//! <paths.archive_dir>/<fiscal_year>/<kind>/<file_name>
//! ```
//!
//! `kind` ist ein knapper, maschinenlesbarer Slug — siehe
//! [`ArchiveKind`].

use crate::{archive::audit, error::Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};

/// Klassifizierung des Archivs — bestimmt Unterordner und Audit-Detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    InvoicePdf,
    InvoiceXml,
    QuotePdf,
    ExpenseOriginal,
    Attachment,
    ReceivedEinvoice,
    /// Versionierte Rechtsdokumente (AGB / Datenschutz), Block 8.
    LegalDocument,
}

impl ArchiveKind {
    pub fn dir_slug(self) -> &'static str {
        match self {
            ArchiveKind::InvoicePdf => "invoices/pdf",
            ArchiveKind::InvoiceXml => "invoices/xml",
            ArchiveKind::QuotePdf => "quotes/pdf",
            ArchiveKind::ExpenseOriginal => "expenses",
            ArchiveKind::Attachment => "attachments",
            ArchiveKind::ReceivedEinvoice => "received-einvoices",
            ArchiveKind::LegalDocument => "legal-documents",
        }
    }

    pub fn source_label(self) -> &'static str {
        match self {
            ArchiveKind::InvoicePdf => "issued_invoice_pdf",
            ArchiveKind::InvoiceXml => "issued_invoice_xml",
            ArchiveKind::QuotePdf => "issued_quote_pdf",
            ArchiveKind::ExpenseOriginal => "expense_attachment",
            ArchiveKind::Attachment => "user_attachment",
            ArchiveKind::ReceivedEinvoice => "received_einvoice",
            ArchiveKind::LegalDocument => "legal_document",
        }
    }
}

/// Ergebnis eines erfolgreichen Store-Vorgangs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredArchive {
    pub archive_id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_hash_sha256: String,
    pub file_size_bytes: i64,
    pub mime_type: String,
}

/// Bytes hashen, in den Archive-Tree schreiben, Datei sperren, DB-Eintrag
/// schreiben, Audit-Log-Eintrag schreiben. Liefert die Archive-ID
/// (UUIDv7) und den absoluten Pfad.
pub async fn store_bytes(
    pool: &SqlitePool,
    archive_root: &Path,
    fiscal_year: i32,
    kind: ArchiveKind,
    file_name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<StoredArchive> {
    if file_name.is_empty() {
        return Err(crate::error::Error::Domain(
            "archive::store: file_name darf nicht leer sein".into(),
        ));
    }
    if file_name.contains(['/', '\\', ':']) {
        return Err(crate::error::Error::Domain(format!(
            "archive::store: file_name enthält Pfad-Trennzeichen: {file_name}"
        )));
    }

    let hash = sha256_hex(bytes);
    let size = bytes.len() as i64;

    let abs_path = build_path(archive_root, fiscal_year, kind, file_name);

    // Parent-Verzeichnis sicherstellen.
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Wenn die Datei bereits existiert: prüfe Hash. Identisch → Idempotenz,
    // wiederverwenden statt Fehler. Unterschiedlich → harter Fehler.
    if abs_path.exists() {
        let existing = fs::read(&abs_path)?;
        let existing_hash = sha256_hex(&existing);
        if existing_hash != hash {
            return Err(crate::error::Error::Domain(format!(
                "archive::store: Datei existiert bereits mit anderem Hash: {}",
                abs_path.display()
            )));
        }
        // Wenn DB-Eintrag existiert: zurückgeben. Falls nicht: weiter zum
        // Insert, denn der ist Single Source of Truth.
        if let Some(found) = lookup_by_path(pool, &abs_path).await? {
            return Ok(found);
        }
    } else {
        // Atomar schreiben: .tmp → rename. Auf Windows ist rename über
        // existing target ein Fehler — wir haben oben aber existence
        // ausgeschlossen.
        let tmp = abs_path.with_extension(format!(
            "{}.tmp",
            abs_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("part")
        ));
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &abs_path)?;
    }

    set_readonly(&abs_path)?;

    let archive_id = uuid::Uuid::now_v7().to_string();
    let path_str = abs_path
        .to_str()
        .ok_or_else(|| {
            crate::error::Error::Domain(format!("nicht-UTF-8-Pfad: {}", abs_path.display()))
        })?
        .to_string();

    sqlx::query(
        "INSERT INTO archive_entries
            (id, file_path, file_name, file_hash_sha256, file_size_bytes, mime_type, source)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&archive_id)
    .bind(&path_str)
    .bind(file_name)
    .bind(&hash)
    .bind(size)
    .bind(mime_type)
    .bind(kind.source_label())
    .execute(pool)
    .await?;

    audit::archive_event(
        pool,
        audit::ArchiveAction::Store,
        &archive_id,
        Some(&format!(
            r#"{{"path":"{}","size":{},"hash":"{}"}}"#,
            escape_json(&path_str),
            size,
            hash
        )),
    )
    .await?;

    Ok(StoredArchive {
        archive_id,
        file_path: path_str,
        file_name: file_name.to_string(),
        file_hash_sha256: hash,
        file_size_bytes: size,
        mime_type: mime_type.to_string(),
    })
}

/// Variante ohne Audit-Spur — für den Integrity-Cron, der bereits
/// `archive.integrity_pass`/`_fail`/`_missing` emittiert. Sonst würde jeder
/// nightly Check tausende `archive.read`-Einträge pro Tag erzeugen.
pub async fn read_and_verify_silent(pool: &SqlitePool, archive_id: &str) -> Result<Vec<u8>> {
    read_and_verify_inner(pool, archive_id, false).await
}

/// Liest die Archiv-Datei und verifiziert SHA-256 gegen den DB-Eintrag.
/// Liefert die Bytes; wirft `Error::Domain` bei Tamper-Detection. Schreibt
/// einen `archive.read`-Audit-Eintrag (R2-028) — GoBD-Nachvollziehbarkeit.
pub async fn read_and_verify(pool: &SqlitePool, archive_id: &str) -> Result<Vec<u8>> {
    read_and_verify_inner(pool, archive_id, true).await
}

async fn read_and_verify_inner(
    pool: &SqlitePool,
    archive_id: &str,
    emit_audit: bool,
) -> Result<Vec<u8>> {
    let row = sqlx::query(
        "SELECT file_path, file_hash_sha256, file_size_bytes
         FROM archive_entries WHERE id = ?",
    )
    .bind(archive_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        crate::error::Error::Domain(format!("archive_entries: id {archive_id} nicht gefunden"))
    })?;

    use sqlx::Row;
    let path: String = row.try_get("file_path")?;
    let expected_hash: String = row.try_get("file_hash_sha256")?;
    let expected_size: i64 = row.try_get("file_size_bytes")?;

    let bytes = fs::read(&path)?;
    if bytes.len() as i64 != expected_size {
        return Err(crate::error::Error::Domain(format!(
            "archive tamper: size mismatch für {archive_id} (erwartet {expected_size}, gefunden {})",
            bytes.len()
        )));
    }
    let actual_hash = sha256_hex(&bytes);
    if actual_hash != expected_hash {
        return Err(crate::error::Error::Domain(format!(
            "archive tamper: hash mismatch für {archive_id} (erwartet {expected_hash}, gefunden {actual_hash})"
        )));
    }
    // R2-028: jeder erfolgreiche Lese-Vorgang aus dem fachlichen Pfad erzeugt
    // eine Audit-Spur. GoBD/§147 AO verlangen Nachvollziehbarkeit, „wer hat
    // den festgeschriebenen Beleg wann gelesen?". Integrity-Cron nutzt
    // `read_and_verify_silent`, weil er bereits eigene Events emittiert.
    if emit_audit {
        audit::archive_event(
            pool,
            audit::ArchiveAction::Read,
            archive_id,
            Some(&format!(
                r#"{{"path":"{}","bytes":{}}}"#,
                path.replace('\\', "\\\\").replace('"', "\\\""),
                bytes.len()
            )),
        )
        .await?;
    }
    Ok(bytes)
}

async fn lookup_by_path(pool: &SqlitePool, abs_path: &Path) -> Result<Option<StoredArchive>> {
    let path = abs_path
        .to_str()
        .ok_or_else(|| crate::error::Error::Domain("nicht-UTF-8-Pfad".into()))?;
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT id, file_path, file_name, file_hash_sha256, file_size_bytes, mime_type
         FROM archive_entries WHERE file_path = ?",
    )
    .bind(path)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| StoredArchive {
        archive_id: r.get("id"),
        file_path: r.get("file_path"),
        file_name: r.get("file_name"),
        file_hash_sha256: r.get("file_hash_sha256"),
        file_size_bytes: r.get("file_size_bytes"),
        mime_type: r.get("mime_type"),
    }))
}

fn build_path(root: &Path, fiscal_year: i32, kind: ArchiveKind, file_name: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    p.push(fiscal_year.to_string());
    for part in kind.dir_slug().split('/') {
        p.push(part);
    }
    p.push(file_name);
    p
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn set_readonly(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(path, perms)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = fs::metadata(path)?.permissions();
        p.set_mode(0o400);
        fs::set_permissions(path, p)?;
    }
    Ok(())
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Test-Helper: setzt readonly zurück, damit Tests Files aufräumen
/// können. Niemals in Production aufrufen.
#[cfg(test)]
pub(crate) fn unlock_for_test(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
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
        pool
    }

    #[tokio::test]
    async fn store_writes_file_and_db_row_and_sets_readonly() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let bytes = b"hello pdf";
        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            bytes,
        )
        .await
        .unwrap();

        assert!(Path::new(&res.file_path).exists());
        assert_eq!(res.file_size_bytes, bytes.len() as i64);
        // Datei muss read-only sein
        let perms = fs::metadata(&res.file_path).unwrap().permissions();
        assert!(perms.readonly(), "Datei muss read-only sein");

        // DB-Eintrag konsistent
        use sqlx::Row;
        let row =
            sqlx::query("SELECT file_hash_sha256, mime_type FROM archive_entries WHERE id = ?")
                .bind(&res.archive_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let db_hash: String = row.try_get("file_hash_sha256").unwrap();
        assert_eq!(db_hash, res.file_hash_sha256);

        // Audit-Log-Eintrag muss existieren
        let cnt: i64 =
            sqlx::query("SELECT COUNT(*) AS n FROM audit_log WHERE action = 'archive.store'")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get("n");
        assert_eq!(cnt, 1);

        // Teardown
        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }

    #[tokio::test]
    async fn store_is_idempotent_for_same_bytes() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r1 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"identical",
        )
        .await
        .unwrap();
        let r2 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"identical",
        )
        .await
        .unwrap();
        assert_eq!(r1.archive_id, r2.archive_id);
        assert_eq!(r1.file_hash_sha256, r2.file_hash_sha256);
        unlock_for_test(Path::new(&r1.file_path)).unwrap();
    }

    #[tokio::test]
    async fn store_rejects_different_bytes_for_same_path() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let r1 = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"first",
        )
        .await
        .unwrap();
        let err = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"second",
        )
        .await
        .unwrap_err();
        assert!(format!("{err}").contains("anderem Hash"));
        unlock_for_test(Path::new(&r1.file_path)).unwrap();
    }

    #[tokio::test]
    async fn read_and_verify_returns_bytes_when_intact() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoiceXml,
            "RE-2026-0001.xml",
            "application/xml",
            b"<xml/>",
        )
        .await
        .unwrap();
        let bytes = read_and_verify(&pool, &res.archive_id).await.unwrap();
        assert_eq!(bytes, b"<xml/>");

        // R2-028: erfolgreiches `read_and_verify` muss einen `archive.read`-
        // Audit-Eintrag erzeugen (GoBD-Nachvollziehbarkeit).
        use sqlx::Row;
        let cnt: i64 =
            sqlx::query("SELECT COUNT(*) AS n FROM audit_log WHERE action = 'archive.read'")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get("n");
        assert_eq!(cnt, 1, "read_and_verify muss archive.read auditieren");

        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }

    /// R2-028 — `read_and_verify_silent` emittiert KEINEN `archive.read`-Audit.
    /// Vom Integrity-Cron verwendet, der seine eigenen Events schreibt.
    #[tokio::test]
    async fn read_and_verify_silent_does_not_emit_audit() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoiceXml,
            "RE-2026-0001.xml",
            "application/xml",
            b"<xml/>",
        )
        .await
        .unwrap();
        let bytes = read_and_verify_silent(&pool, &res.archive_id)
            .await
            .unwrap();
        assert_eq!(bytes, b"<xml/>");

        use sqlx::Row;
        let cnt: i64 =
            sqlx::query("SELECT COUNT(*) AS n FROM audit_log WHERE action = 'archive.read'")
                .fetch_one(&pool)
                .await
                .unwrap()
                .get("n");
        assert_eq!(
            cnt, 0,
            "Silent-Variante darf keinen archive.read-Audit erzeugen"
        );

        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }

    #[tokio::test]
    async fn read_and_verify_detects_tamper() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoiceXml,
            "RE-2026-0001.xml",
            "application/xml",
            b"<original/>",
        )
        .await
        .unwrap();

        // Tamper: file unlock, überschreiben, wieder lock
        unlock_for_test(Path::new(&res.file_path)).unwrap();
        fs::write(&res.file_path, b"<tampered/>").unwrap();
        set_readonly(Path::new(&res.file_path)).unwrap();

        let err = read_and_verify(&pool, &res.archive_id).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("tamper"),
            "erwartete Tamper-Meldung, got: {msg}"
        );

        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }

    #[tokio::test]
    async fn store_rejects_filename_with_path_separator() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let err = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "../escape.pdf",
            "application/pdf",
            b"x",
        )
        .await
        .unwrap_err();
        assert!(format!("{err}").contains("Pfad-Trennzeichen"));
    }
}
