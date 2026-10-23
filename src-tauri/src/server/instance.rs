//! Instanz-Marker `/data/.klein-buch-instance` (Plan W5.5).
//!
//! Der Server schreibt ihn zum Abschluss der Einrichtung. Beim Start gilt:
//! weder DB noch Marker → Zustand `setup`; DB ohne Marker oder Marker ohne DB
//! → Start verweigert (falsches oder halb geleertes Volume).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const MARKER_FILE: &str = ".klein-buch-instance";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceMarker {
    pub instance_id: String,
    pub created_at: String,
    pub app_version: String,
}

pub fn marker_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MARKER_FILE)
}

/// Schreibt den Marker atomar (tmp + fsync + rename + Verzeichnis-fsync); ein
/// vorhandener Marker bleibt unverändert.
pub fn write_marker(data_dir: &Path) -> Result<()> {
    let path = marker_path(data_dir);
    if path.exists() {
        return Ok(());
    }
    let marker = InstanceMarker {
        instance_id: uuid::Uuid::now_v7().to_string(),
        created_at: crate::clock::now_utc().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let tmp = data_dir.join(format!(
        ".{MARKER_FILE}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    let written = (|| -> Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(serde_json::to_string_pretty(&marker)?.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Ok(dir) = std::fs::File::open(data_dir) {
        let _ = dir.sync_all();
    }
    tracing::info!("Instanz-Marker angelegt ({})", marker.instance_id);
    Ok(())
}

pub fn read_marker(data_dir: &Path) -> Result<Option<InstanceMarker>> {
    let path = marker_path(data_dir);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
