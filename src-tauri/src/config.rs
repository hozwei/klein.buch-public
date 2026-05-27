//! App-Konfiguration: Pfade zu DB, Archive, Backups, Inputs.
//!
//! Resolution-Reihenfolge:
//! 1. Env-Var (für Tests).
//! 2. Tauri AppHandle::path() (Production).
//! 3. Falls weder noch: relative Pfade vom CWD (cargo test outside Tauri).

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub struct Paths {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub archive_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub inputs_dir: PathBuf,
    pub sidecar_dir: PathBuf,
}

impl Paths {
    pub fn from_handle(app: &AppHandle) -> crate::error::Result<Self> {
        // In Dev/Test landet alles unter klein-buch/data/. Production:
        // App-Data-Roaming (Windows: %APPDATA%\de.wildbach.kleinbuch\).
        let data_dir = if cfg!(debug_assertions) {
            std::env::current_dir()?.join("..").join("data")
        } else {
            app.path()
                .app_data_dir()
                .map_err(|e| crate::error::Error::Config(format!("app_data_dir: {e}")))?
        };

        let archive_dir = data_dir.join("archive");
        let backups_dir = data_dir.join("backups");
        let db_file = data_dir.join("klein-buch.sqlite");

        let inputs_dir = if cfg!(debug_assertions) {
            std::env::current_dir()?.join("..").join("inputs")
        } else {
            app.path()
                .resource_dir()
                .map_err(|e| crate::error::Error::Config(format!("resource_dir: {e}")))?
                .join("inputs")
        };

        let sidecar_triple = sidecar_target_triple();
        let sidecar_dir = if cfg!(debug_assertions) {
            std::env::current_dir()?
                .join("binaries")
                .join(format!("klein-buch-java-{sidecar_triple}"))
        } else {
            app.path()
                .resource_dir()
                .map_err(|e| crate::error::Error::Config(format!("resource_dir: {e}")))?
                .join("binaries")
                .join(format!("klein-buch-java-{sidecar_triple}"))
        };

        Ok(Self {
            data_dir,
            db_file,
            archive_dir,
            backups_dir,
            inputs_dir,
            sidecar_dir,
        })
    }
}

/// Target-Triple für die mitgelieferte Sidecar-Bundle-Struktur.
/// Block 0 baut nur Windows-x86_64; macOS/Linux folgen in Block 17 via CI-Matrix.
pub fn sidecar_target_triple() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        // Fallback — wird beim Sidecar-Resolve einen Fehler werfen.
        "unknown-target"
    }
}
