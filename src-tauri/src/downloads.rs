//! Downloads ohne Pfad vom Client (Plan W7, E10).
//!
//! Exporte (EÜR, DATEV, ELSTER, Steuerberater, Migration, DSGVO) und Vorschauen
//! (Vorlage, Paket, Broschüre, Angebots-Bundle) entstehen in
//! `<data>/tmp/downloads/<uuid>/<Dateiname>`; der Kern gibt ein
//! [`DownloadTicket`] zurück, der Browser holt die Datei über
//! `GET /files/download/{id}`. Exporte sind einmalig abrufbar und verfallen
//! nach [`EXPORT_TTL`], Vorschauen sind mehrfach abrufbar (PDF-Ansicht lädt
//! nach) und verfallen nach [`PREVIEW_TTL`]. Abgelaufene werden bei jedem
//! Registrieren und Abrufen gelöscht, Reste beim Serverstart
//! und durch den stündlichen Sweeper (`<data>/tmp`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use uuid::Uuid;

use crate::error::{Error, Result};

pub const EXPORT_TTL: Duration = Duration::from_secs(3600);
pub const PREVIEW_TTL: Duration = Duration::from_secs(600);
const DIR: &str = "downloads";

/// Was der Kern dem Browser zurückgibt.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTicket {
    pub download_id: String,
    pub file_name: String,
    pub size_bytes: u64,
}

/// Art des Downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadKind {
    /// Export: einmalig, Anhang, [`EXPORT_TTL`].
    Export,
    /// Vorschau (selbst erzeugtes PDF): mehrfach, inline, [`PREVIEW_TTL`].
    Preview,
}

/// Ein registrierter Download.
#[derive(Debug, Clone)]
pub struct DownloadEntry {
    pub path: PathBuf,
    pub file_name: String,
    pub mime: &'static str,
    pub kind: DownloadKind,
    expires: Instant,
}

/// Ablage der Downloads (im `AppCtx`).
pub struct DownloadStore {
    root: PathBuf,
    entries: Mutex<HashMap<Uuid, DownloadEntry>>,
}

/// Dateiname ohne Pfadteile, Steuerzeichen und für Windows ungültige Zeichen.
pub fn safe_file_name(name: &str, fallback: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(150).collect()
    }
}

/// MIME-Typ nach Endung (nur für selbst erzeugte Dateien).
pub fn mime_for(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        "application/pdf"
    } else if lower.ends_with(".zip") {
        "application/zip"
    } else if lower.ends_with(".csv") {
        "text/csv; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

impl DownloadStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("tmp").join(DIR),
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, DownloadEntry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Legt das Verzeichnis eines neuen Downloads an und liefert ID und den
    /// Pfad, unter den der Kern die Datei schreibt.
    pub fn prepare(&self, file_name: &str) -> Result<(Uuid, PathBuf)> {
        self.sweep();
        let id = Uuid::new_v4();
        let dir = self.root.join(id.simple().to_string());
        std::fs::create_dir_all(&dir)?;
        let name = safe_file_name(file_name, "download");
        Ok((id, dir.join(name)))
    }

    /// Registriert die geschriebene Datei.
    pub fn register(&self, id: Uuid, path: &Path, kind: DownloadKind) -> Result<DownloadTicket> {
        let meta = std::fs::metadata(path)?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| Error::Other(anyhow::anyhow!("Download ohne Dateinamen")))?;
        let ttl = match kind {
            DownloadKind::Export => EXPORT_TTL,
            DownloadKind::Preview => PREVIEW_TTL,
        };
        let entry = DownloadEntry {
            path: path.to_path_buf(),
            mime: mime_for(&file_name),
            file_name: file_name.clone(),
            kind,
            expires: Instant::now() + ttl,
        };
        self.lock().insert(id, entry);
        Ok(DownloadTicket {
            download_id: id.to_string(),
            file_name,
            size_bytes: meta.len(),
        })
    }

    /// Schreibt Bytes als neuen Download (Vorschauen).
    pub fn put_bytes(
        &self,
        file_name: &str,
        bytes: &[u8],
        kind: DownloadKind,
    ) -> Result<DownloadTicket> {
        let (id, path) = self.prepare(file_name)?;
        std::fs::write(&path, bytes)?;
        self.register(id, &path, kind)
    }

    /// Eintrag zum Abruf; einmalige Downloads werden dabei entnommen (der
    /// Aufrufer löscht die Datei nach dem Ausliefern über [`Self::remove_files`]).
    pub fn take(&self, id: &str) -> Option<DownloadEntry> {
        self.sweep();
        let id = Uuid::parse_str(id).ok()?;
        let mut map = self.lock();
        let entry = map.get(&id)?.clone();
        if entry.kind == DownloadKind::Export {
            map.remove(&id);
        }
        Some(entry)
    }

    /// Löscht das Verzeichnis eines Downloads.
    pub fn remove_files(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Entfernt abgelaufene Einträge samt Dateien, dazu nicht registrierte
    /// Verzeichnisse (abgebrochene Vorgänge), die älter als [`EXPORT_TTL`] sind.
    pub fn sweep(&self) {
        let now = Instant::now();
        let (expired, known): (Vec<PathBuf>, Vec<String>) = {
            let mut map = self.lock();
            let gone: Vec<Uuid> = map
                .iter()
                .filter(|(_, e)| e.expires <= now)
                .map(|(id, _)| *id)
                .collect();
            let expired = gone
                .iter()
                .filter_map(|id| map.remove(id).map(|e| e.path))
                .collect();
            (
                expired,
                map.keys().map(|id| id.simple().to_string()).collect(),
            )
        };
        for path in expired {
            Self::remove_files(&path);
        }
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > EXPORT_TTL);
            if old && !known.contains(&name) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// Alle Einträge und Dateien löschen (beim Sperren der Datenbank).
    pub fn clear(&self) {
        self.lock().clear();
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// Anzahl aktiver Einträge (Tests).
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_single_use_preview_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let store = DownloadStore::new(dir.path());
        let ticket = store
            .put_bytes("../../etc/EÜR 2026.pdf", b"%PDF-1.7", DownloadKind::Export)
            .unwrap();
        assert_eq!(ticket.file_name, "EÜR 2026.pdf");
        assert_eq!(ticket.size_bytes, 8);
        let entry = store.take(&ticket.download_id).unwrap();
        assert!(entry
            .path
            .starts_with(dir.path().join("tmp").join("downloads")));
        assert!(store.take(&ticket.download_id).is_none(), "einmalig");
        DownloadStore::remove_files(&entry.path);
        assert!(!entry.path.exists());

        let preview = store
            .put_bytes("vorschau.pdf", b"%PDF", DownloadKind::Preview)
            .unwrap();
        assert!(store.take(&preview.download_id).is_some());
        assert!(store.take(&preview.download_id).is_some(), "mehrfach");
        assert!(store.take("kein-uuid").is_none());
    }

    #[test]
    fn file_names_are_cleaned() {
        assert_eq!(safe_file_name("a\"b\r\n.pdf", "x"), "ab.pdf");
        assert_eq!(safe_file_name("..", "x"), "x");
        assert_eq!(safe_file_name("C:\\temp\\x.zip", "y"), "x.zip");
    }
}
