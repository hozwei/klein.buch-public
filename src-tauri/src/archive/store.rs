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
//! 2. **Atomische Sichtbarkeit**: zuerst in eine Temp-Datei mit eindeutigem
//!    Namen schreiben (`.<name>.<uuid>.tmp`, fsync), schreibschützen, dann
//!    ohne Überschreiben an den Zielnamen bringen (Plan W2.1) → erst dann
//!    existiert die finale Datei, und sie ist von Anfang an read-only.
//!    Eine Datei am Zielpfad **ohne** `archive_entries`-Eintrag ist der Rest
//!    eines abgebrochenen Laufs (nie Teil der Aufbewahrung) und wird ersetzt;
//!    mit Eintrag bleibt ein anderer Inhalt ein harter Fehler.
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

use crate::hash::sha256_hex;
use crate::{archive::audit, error::Result};
use serde::Serialize;
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
///
/// Liegt am Zielpfad schon ein **registrierter** Eintrag mit anderem Inhalt,
/// ist das ein harter Fehler („existiert bereits mit anderem Hash“).
pub async fn store_bytes(
    pool: &SqlitePool,
    archive_root: &Path,
    fiscal_year: i32,
    kind: ArchiveKind,
    file_name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<StoredArchive> {
    match store_bytes_inner(
        pool,
        archive_root,
        fiscal_year,
        kind,
        file_name,
        mime_type,
        bytes,
    )
    .await?
    {
        StoreAttempt::Stored(s) => Ok(s),
        StoreAttempt::Conflict(path) => Err(path_error(
            "Datei existiert bereits mit anderem Hash",
            &path,
        )),
    }
}

/// Wie [`store_bytes`], weicht aber bei einem registrierten Eintrag mit anderem
/// Inhalt auf eine neue Dateiversion aus (`RE-2026-0001-2.pdf`, `-3`, …).
///
/// Für Belege, die nach einem Abbruch neu erzeugt werden (Review W2 S1-1): ist
/// die PDF schon archiviert, die Rechnung aber noch nicht festgeschrieben
/// (Fehler beim XML, Absturz), erzeugt jeder neue Versuch andere Bytes
/// (Zeitstempel in den PDF-Metadaten). Der alte Eintrag bleibt write-once im
/// Archiv (nicht referenziert), der Beleg verweist auf die neue Version.
#[allow(clippy::too_many_arguments)]
pub async fn store_bytes_new_version(
    pool: &SqlitePool,
    archive_root: &Path,
    fiscal_year: i32,
    kind: ArchiveKind,
    file_name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<StoredArchive> {
    const MAX_VERSIONS: u32 = 50;
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (file_name.to_string(), String::new()),
    };
    for version in 1..=MAX_VERSIONS {
        let name = if version == 1 {
            file_name.to_string()
        } else {
            format!("{stem}-{version}{ext}")
        };
        match store_bytes_inner(
            pool,
            archive_root,
            fiscal_year,
            kind,
            &name,
            mime_type,
            bytes,
        )
        .await?
        {
            StoreAttempt::Stored(s) => return Ok(s),
            StoreAttempt::Conflict(_) => continue,
        }
    }
    Err(crate::error::Error::Domain(format!(
        "archive::store: mehr als {MAX_VERSIONS} Versionen von {file_name}"
    )))
}

enum StoreAttempt {
    Stored(StoredArchive),
    /// Registrierter Eintrag am Zielpfad mit anderem Inhalt.
    Conflict(PathBuf),
}

#[allow(clippy::too_many_arguments)]
async fn store_bytes_inner(
    pool: &SqlitePool,
    archive_root: &Path,
    fiscal_year: i32,
    kind: ArchiveKind,
    file_name: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<StoreAttempt> {
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

    // Review W2 S0-1: Prüfen, Schreiben und INSERT je Zielpfad am Stück. Ohne
    // Sperre könnte ein paralleler Lauf eine gerade geschriebene, noch nicht
    // registrierte Datei für verwaist halten. Schlüssel in Kleinbuchstaben, weil
    // manche Dateisysteme Groß-/Kleinschreibung nicht unterscheiden.
    let _path_guard = crate::locks::ARCHIVE_PATHS
        .lock(&abs_path.to_string_lossy().to_lowercase())
        .await;

    // Parent-Verzeichnis sicherstellen.
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Wenn die Datei bereits existiert: prüfe Hash. Identisch → Idempotenz,
    // wiederverwenden statt Fehler. Unterschiedlich → mit DB-Eintrag ein
    // Konflikt; ohne Eintrag ein Rest eines abgebrochenen Laufs (W2.1), der
    // **beiseitegelegt** (nicht gelöscht) wird.
    let mut replaced_orphan: Option<(String, String)> = None;
    if abs_path.exists() {
        let existing = fs::read(&abs_path)?;
        let existing_hash = sha256_hex(&existing);
        let registered = lookup_by_path(pool, &abs_path).await?;
        if existing_hash == hash {
            // DB-Eintrag vorhanden → zurückgeben; sonst weiter zum Insert,
            // denn der ist Single Source of Truth.
            if let Some(found) = registered {
                return Ok(StoreAttempt::Stored(found));
            }
        } else if registered.is_some() {
            return Ok(StoreAttempt::Conflict(abs_path));
        } else {
            let kept = keep_orphan_aside(&abs_path)?;
            tracing::warn!(
                "archive::store: Datei ohne archive_entries-Eintrag beiseitegelegt ({}) und neu geschrieben: {}",
                kept,
                abs_path.display()
            );
            write_new_readonly(&abs_path, bytes)?;
            replaced_orphan = Some((existing_hash, kept));
        }
    } else {
        write_new_readonly(&abs_path, bytes)?;
    }

    set_readonly(&abs_path)?;

    let archive_id = uuid::Uuid::now_v7().to_string();
    let path_str = abs_path
        .to_str()
        .ok_or_else(|| path_error("Pfad ist kein gültiges UTF-8", &abs_path))?
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

    let mut details = serde_json::json!({ "path": &path_str, "size": size, "hash": &hash });
    if let Some((orphan_hash, kept)) = replaced_orphan {
        details["replaced_orphan_hash"] = serde_json::Value::from(orphan_hash);
        details["orphan_kept_as"] = serde_json::Value::from(kept);
    }
    audit::archive_event(
        pool,
        audit::ArchiveAction::Store,
        &archive_id,
        Some(details),
    )
    .await?;

    Ok(StoreAttempt::Stored(StoredArchive {
        archive_id,
        file_path: path_str,
        file_name: file_name.to_string(),
        file_hash_sha256: hash,
        file_size_bytes: size,
        mime_type: mime_type.to_string(),
    }))
}

/// Legt eine nicht registrierte Datei am Zielpfad unter einem versteckten Namen
/// beiseite (`.<name>.orphan-<uuid>`, Schreibschutz bleibt) und liefert den
/// neuen Dateinamen. Nichts wird gelöscht (Review W2 S0-1/S2-5): sollte die
/// Waisen-Erkennung je irren (Pfad-Schreibweise, Umzug), bleibt die Datei da.
fn keep_orphan_aside(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kept = format!(".{name}.orphan-{}", uuid::Uuid::now_v7().simple());
    fs::rename(path, path.with_file_name(&kept))?;
    Ok(kept)
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
            Some(serde_json::json!({ "path": &path, "bytes": bytes.len() })),
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

/// Fachlicher Fehler mit Archivpfad (W5-ERR-PATHS, Lauf F4c): Die Meldung an
/// den Client nennt nur den Dateinamen; der volle Serverpfad steht nur im Log.
fn path_error(what: &str, path: &Path) -> crate::error::Error {
    tracing::warn!("archive::store: {what}: {}", path.display());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    crate::error::Error::Domain(format!("Archiv: {what} ({name})."))
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

/// Schreibt `bytes` als neue, schreibgeschützte Datei an `final_path` (Plan
/// W2.1): eindeutiger Temp-Name im selben Verzeichnis (keine Kollision mit
/// Resten früherer Läufe oder parallelen Schreibern), `fsync`, Rücklese-Prüfung
/// (Lauf F, KB-0040: die Temp-Datei wird gelesen und gegen den SHA-256 des
/// Puffers geprüft, bevor sie sichtbar wird), Schreibschutz **vor** dem
/// Sichtbarwerden, dann ohne Überschreiben an den Zielnamen (`hard_link`
/// scheitert, wenn das Ziel schon existiert; ohne Hardlinks `renameat2` mit
/// `RENAME_NOREPLACE`, Lauf F W2-S2-4). Fehler räumen die Temp-Datei weg.
fn write_new_readonly(final_path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let name = final_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            crate::error::Error::Domain("archive::store: ungültiger Dateiname".into())
        })?;
    let tmp = final_path.with_file_name(format!(".{name}.{}.tmp", uuid::Uuid::now_v7().simple()));
    let result = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        // KB-0040: Rücklese-Prüfung vor dem Sichtbarwerden.
        if sha256_hex(&fs::read(&tmp)?) != sha256_hex(bytes) {
            return Err(path_error(
                "Rücklese-Prüfung fehlgeschlagen (Inhalt weicht ab)",
                final_path,
            ));
        }
        set_readonly(&tmp)?;
        match fs::hard_link(&tmp, final_path) {
            // Der Hardlink teilt den Inode: die Temp-Datei nur entfernen, ihre
            // Rechte NICHT ändern (sonst wäre auch die Zieldatei beschreibbar).
            // Unter Linux braucht das Löschen nur Schreibrecht aufs Verzeichnis.
            Ok(()) => fs::remove_file(&tmp)?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(path_error("Datei wurde parallel angelegt", final_path));
            }
            // Dateisystem ohne Hardlinks: umbenennen, ohne zu überschreiben.
            Err(_) => rename_no_clobber(&tmp, final_path)?,
        }
        if let Some(parent) = final_path.parent() {
            // Verzeichniseintrag dauerhaft machen (best-effort, nicht überall möglich).
            if let Ok(dir) = fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    })();
    if result.is_err() {
        // Fehlt die Temp-Datei schon (NotFound), ist nichts zu tun.
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Umbenennen ohne Überschreiben (Lauf F, W2-S2-4): unter Linux `renameat2`
/// mit `RENAME_NOREPLACE` (atomar; existiert das Ziel, `AlreadyExists`).
/// Kennt das Dateisystem das Flag nicht (`EINVAL`/`ENOSYS`/`EOPNOTSUPP`), wird
/// das Ziel unmittelbar vorher geprüft und normal umbenannt — gegen Schreiber
/// im selben Prozess schützt die Pfad-Sperre, ein fremder Schreiber auf dem
/// Archiv-Volume ist ausgeschlossen (Betriebsdoku: DB und Archiv nicht auf
/// CIFS/exFAT).
fn rename_no_clobber(from: &Path, to: &Path) -> Result<()> {
    let exists = || -> Result<()> { Err(path_error("Datei wurde parallel angelegt", to)) };
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{renameat_with, RenameFlags, CWD};
        use rustix::io::Errno;
        match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
            Ok(()) => return Ok(()),
            Err(Errno::EXIST) => return exists(),
            Err(Errno::INVAL | Errno::NOSYS | Errno::OPNOTSUPP) => {}
            Err(e) => return Err(std::io::Error::from(e).into()),
        }
    }
    match fs::symlink_metadata(to) {
        Ok(_) => exists(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(fs::rename(from, to)?),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn set_readonly(path: &Path) -> Result<()> {
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

    /// Lauf F6b (LE-HASH-CRATES): Archiv-Hashes bleiben über Crate-Updates
    /// bit-genau (FIPS 180-2, Anhang B: SHA-256 von "abc", leer, 448 Bit).
    #[test]
    fn sha256_hex_known_answers() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::TempDir;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        create_tables(&pool).await;
        pool
    }

    /// Datei-DB im Produktionsmodus (WAL, busy_timeout, mehrere Verbindungen).
    async fn file_pool(dir: &Path) -> SqlitePool {
        let pool = crate::db::open_pool(&dir.join("t.sqlite"), None)
            .await
            .unwrap();
        create_tables(&pool).await;
        pool
    }

    async fn create_tables(pool: &SqlitePool) {
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
        .execute(pool)
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
        .execute(pool)
        .await
        .unwrap();
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

    /// Plan W2.1: ein liegengebliebener Temp-Rest (früher fester Name
    /// `<name>.tmp`, hier sogar schreibgeschützt) blockiert das Archivieren nicht,
    /// und nach dem Schreiben bleibt keine Temp-Datei zurück.
    #[tokio::test]
    async fn store_ignores_stale_temp_file_and_leaves_no_temp() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let target_dir = dir
            .path()
            .join("2026")
            .join(ArchiveKind::InvoicePdf.dir_slug());
        fs::create_dir_all(&target_dir).unwrap();
        let stale = target_dir.join("RE-2026-0001.pdf.tmp");
        fs::write(&stale, b"rest").unwrap();
        set_readonly(&stale).unwrap();

        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0001.pdf",
            "application/pdf",
            b"neu",
        )
        .await
        .unwrap();
        assert_eq!(fs::read(&res.file_path).unwrap(), b"neu");
        let leftovers: Vec<String> = fs::read_dir(&target_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.') && n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "Temp-Reste: {leftovers:?}");
        unlock_for_test(&stale).unwrap();
        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }

    /// Plan W2.1: Abbruch nach dem Schreiben, vor dem DB-Eintrag → Wiederholung
    /// mit anderen Bytes (z. B. neu gerenderte PDF) ersetzt die verwaiste Datei
    /// statt dauerhaft an „existiert mit anderem Hash“ zu scheitern.
    #[tokio::test]
    async fn store_replaces_orphan_file_without_db_entry() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let path = build_path(
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0002.pdf",
        );
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_new_readonly(&path, b"erster Versuch").unwrap();
        assert!(fs::metadata(&path).unwrap().permissions().readonly());

        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0002.pdf",
            "application/pdf",
            b"zweiter Versuch",
        )
        .await
        .unwrap();
        assert_eq!(fs::read(&res.file_path).unwrap(), b"zweiter Versuch");
        assert!(fs::metadata(&res.file_path)
            .unwrap()
            .permissions()
            .readonly());
        use sqlx::Row;
        let details: String = sqlx::query(
            "SELECT details_json FROM audit_log WHERE action = 'archive.store' AND entity_id = ?",
        )
        .bind(&res.archive_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("details_json");
        assert!(details.contains("replaced_orphan_hash"), "{details}");
        // Review W2 S0-1: die Waise wird beiseitegelegt, nicht gelöscht.
        let kept: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().contains(".orphan-"))
            .collect();
        assert_eq!(kept.len(), 1, "Waise beiseitegelegt");
        assert_eq!(fs::read(&kept[0]).unwrap(), b"erster Versuch");
        unlock_for_test(&kept[0]).unwrap();
        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }

    /// Review W2 S0-1: zwei parallele Schreiber mit verschiedenen Bytes auf
    /// denselben Pfad — genau einer wird registriert, der andere bekommt den
    /// Konflikt, und die registrierte Datei passt zu ihrem Hash.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parallel_writers_on_same_path_never_break_registered_file() {
        let dir = TempDir::new().unwrap();
        let pool = file_pool(dir.path()).await;
        let mut set = tokio::task::JoinSet::new();
        for i in 0..6 {
            let pool = pool.clone();
            let root = dir.path().to_path_buf();
            set.spawn(async move {
                store_bytes(
                    &pool,
                    &root,
                    2026,
                    ArchiveKind::InvoicePdf,
                    "RE-2026-0009.pdf",
                    "application/pdf",
                    format!("Version {i}").as_bytes(),
                )
                .await
            });
        }
        let mut ok = Vec::new();
        while let Some(r) = set.join_next().await {
            match r.unwrap() {
                Ok(s) => ok.push(s),
                Err(e) => assert!(e.to_string().contains("anderem Hash"), "{e}"),
            }
        }
        assert_eq!(ok.len(), 1, "genau ein registrierter Eintrag");
        let bytes = fs::read(&ok[0].file_path).unwrap();
        assert_eq!(sha256_hex(&bytes), ok[0].file_hash_sha256);
        unlock_for_test(Path::new(&ok[0].file_path)).unwrap();
    }

    /// Review W2 S1-1: Wiederholung nach Abbruch mit neuen Bytes weicht auf eine
    /// neue Dateiversion aus statt dauerhaft zu scheitern.
    #[tokio::test]
    async fn new_version_on_registered_conflict() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let a = store_bytes_new_version(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0010.pdf",
            "application/pdf",
            b"erster",
        )
        .await
        .unwrap();
        let same = store_bytes_new_version(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0010.pdf",
            "application/pdf",
            b"erster",
        )
        .await
        .unwrap();
        assert_eq!(a.archive_id, same.archive_id, "gleiche Bytes → idempotent");
        let b = store_bytes_new_version(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            "RE-2026-0010.pdf",
            "application/pdf",
            b"zweiter",
        )
        .await
        .unwrap();
        assert_eq!(b.file_name, "RE-2026-0010-2.pdf");
        assert_eq!(
            fs::read(&a.file_path).unwrap(),
            b"erster",
            "alter Eintrag unverändert"
        );
        unlock_for_test(Path::new(&a.file_path)).unwrap();
        unlock_for_test(Path::new(&b.file_path)).unwrap();
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
        // W5-ERR-PATHS (Lauf F4c): Die Meldung an den Client nennt den
        // Dateinamen, aber keinen Serverpfad; die HTTP-Antwort ebenso.
        let root = dir.path().to_string_lossy().into_owned();
        assert!(format!("{err}").contains("RE-2026-0001.pdf"), "{err}");
        assert!(!format!("{err}").contains(&root), "{err}");
        let body = crate::server::errors::ApiError::from_error(&err)
            .body()
            .to_string();
        assert!(!body.contains(&root), "{body}");
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
    /// Lauf F (W2-S2-4): der Rückfall ohne Hardlinks überschreibt nie ein
    /// vorhandenes Ziel.
    #[test]
    fn rename_no_clobber_never_overwrites() {
        let dir = TempDir::new().unwrap();
        let from = dir.path().join("a.tmp");
        let to = dir.path().join("a.pdf");
        fs::write(&from, b"neu").unwrap();
        fs::write(&to, b"alt").unwrap();
        let err = rename_no_clobber(&from, &to).unwrap_err();
        assert!(format!("{err}").contains("parallel angelegt"), "{err}");
        assert_eq!(fs::read(&to).unwrap(), b"alt", "Ziel unverändert");
        assert_eq!(fs::read(&from).unwrap(), b"neu", "Quelle bleibt");

        let to2 = dir.path().join("b.pdf");
        rename_no_clobber(&from, &to2).unwrap();
        assert_eq!(fs::read(&to2).unwrap(), b"neu");
        assert!(!from.exists());
    }

    /// Lauf F (W14-010): Anführungs- und Steuerzeichen im Dateinamen ergeben
    /// gültige Audit-Details.
    #[tokio::test]
    async fn store_audit_details_are_valid_json_for_odd_names() {
        let pool = fresh_pool().await;
        let dir = TempDir::new().unwrap();
        let name = "Beleg \"Q3\"\tTab.pdf";
        let res = store_bytes(
            &pool,
            dir.path(),
            2026,
            ArchiveKind::InvoicePdf,
            name,
            "application/pdf",
            b"inhalt",
        )
        .await
        .unwrap();
        use sqlx::Row;
        let details: String = sqlx::query(
            "SELECT details_json FROM audit_log WHERE action = 'archive.store' AND entity_id = ?",
        )
        .bind(&res.archive_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("details_json");
        let v: serde_json::Value = serde_json::from_str(&details).expect("gültiges JSON");
        assert_eq!(v["path"], res.file_path.as_str());
        assert_eq!(v["size"], 6);
        unlock_for_test(Path::new(&res.file_path)).unwrap();
    }
}
