//! App-Konfiguration: Pfade zu DB, Archive, Backups, Inputs.
//!
//! Resolution-Reihenfolge:
//! 1. Env-Var (für Tests).
//! 2. Tauri AppHandle::path() (Production).
//! 3. Falls weder noch: relative Pfade vom CWD (cargo test outside Tauri).
//!
//! ## R7-INPUTS — `inputs/` Zwei-Welten-Modell
//!
//! - **Source-of-truth (read-only, im Bundle):**
//!   `resource_dir/inputs/` — wird zur Build-Zeit von [`build.rs`] aus
//!   `klein-buch/inputs/{specs,pdf-templates,mail-templates,branding}`
//!   gespiegelt und von Tauri-Bundler als Resource eingesammelt.
//! - **User-editable (read/write):** `inputs_dir` = `app_local_data_dir/inputs/`.
//!   Beim App-Start kopiert [`ensure_inputs_seeded`] fehlende Dateien aus dem
//!   Bundle dorthin — idempotent, ohne Überschreiben (`inputs/`-Hardline:
//!   User-Edits sind heilig).
//!
//! Dev-Build (`cfg!(debug_assertions)`): beide Pfade zeigen aufs Repo-`inputs/`,
//! Seeding ist no-op. Production: getrennte Pfade, Seeding läuft beim Start.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub struct Paths {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub archive_dir: PathBuf,
    pub backups_dir: PathBuf,
    /// User-editable inputs-Verzeichnis. Im Dev = Repo-`inputs/`, in Production =
    /// `app_local_data_dir/inputs/` (wird per [`ensure_inputs_seeded`] vom
    /// gebundelten Default-Mirror befüllt).
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

        // R7-INPUTS: in Production wandert `inputs_dir` in den
        // user-editierbaren `app_local_data_dir/inputs/`. Der gebundelte
        // Default-Mirror unter `resource_dir/inputs/` wird beim Start einmalig
        // dorthin gespiegelt (siehe `ensure_inputs_seeded`).
        let inputs_dir = if cfg!(debug_assertions) {
            std::env::current_dir()?.join("..").join("inputs")
        } else {
            app.path()
                .app_local_data_dir()
                .map_err(|e| crate::error::Error::Config(format!("app_local_data_dir: {e}")))?
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

/// R7-INPUTS: First-Run-Copy vom gebundelten Default-Mirror nach `inputs_dir`.
///
/// - **Idempotent:** mehrere Aufrufe sind ein No-op, wenn alle Dateien schon da
///   sind.
/// - **No-Overwrite:** existierende Dateien werden NIE ueberschrieben — die
///   `inputs/`-Hardline aus `CLAUDE.md` schuetzt User-Edits (BMF-Updates der
///   AfA-Tabellen, eigene PDF-Vorlagen, custom Mail-Templates).
/// - **Dev-Build:** no-op, weil `paths.inputs_dir` direkt aufs Repo-`inputs/`
///   zeigt (Source == Ziel).
///
/// Wird in `db::prepare_filesystem` aufgerufen, vor jedem Pool-Open.
pub fn ensure_inputs_seeded(app: &AppHandle, paths: &Paths) -> crate::error::Result<()> {
    if cfg!(debug_assertions) {
        // Dev: inputs_dir IS das Repo-`inputs/`. Kein Seeding.
        return Ok(());
    }

    let bundled = app
        .path()
        .resource_dir()
        .map_err(|e| crate::error::Error::Config(format!("resource_dir: {e}")))?
        .join("inputs");

    if !bundled.exists() {
        // Kein Bundle-Mirror — entweder beschädigte Installation oder ein
        // hypothetisches Vor-R7-INPUTS-Release. Hart loggen, App weiterlaufen
        // lassen; afa_tabellen::load wirft dann den eigentlichen Fehler.
        tracing::warn!(
            target: "klein_buch_lib::config",
            "Bundle-inputs-Mirror fehlt unter {}; First-Run-Copy übersprungen",
            bundled.display()
        );
        return Ok(());
    }

    std::fs::create_dir_all(&paths.inputs_dir)?;
    seed_dir_recursive(&bundled, &paths.inputs_dir)?;
    tracing::info!(
        target: "klein_buch_lib::config",
        "inputs/ aus Bundle gespiegelt: {} -> {}",
        bundled.display(),
        paths.inputs_dir.display()
    );
    Ok(())
}

/// Rekursive Helfer-Funktion fuer `ensure_inputs_seeded`. **Existierende Dateien
/// werden NICHT ueberschrieben.** Fehlende Subdirs werden angelegt.
fn seed_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if entry.file_type()?.is_dir() {
            seed_dir_recursive(&src_path, &dst_path)?;
        } else if !dst_path.exists() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Test-only: macht [`seed_dir_recursive`] für Integrations-Tests aufrufbar,
/// ohne `AppHandle`. Nicht für Production gedacht.
#[doc(hidden)]
pub fn seed_inputs_for_test(bundled: &Path, target: &Path) -> std::io::Result<()> {
    seed_dir_recursive(bundled, target)
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
