//! Klein.Buch Build-Script.
//!
//! Spiegelt `klein-buch/inputs/{specs,pdf-templates,mail-templates,branding}`
//! nach `src-tauri/resources/inputs/...`, damit der Tauri-Bundler den
//! gebundleten Default-Inputs-Tree als Resource einsammeln kann. Die echte
//! menschen-maintained Quelle bleibt `klein-buch/inputs/`; das Mirror unter
//! `src-tauri/resources/inputs/` ist read-only Build-Artefakt
//! (in `.gitignore` ausgenommen). Zur Laufzeit kopiert
//! [`klein_buch_lib::config::ensure_inputs_seeded`] vom Mirror in den
//! user-editierbaren `app_local_data_dir/inputs/` (R7-INPUTS).
//!
//! Hintergrund: bis V2026.5 fehlte `inputs/` komplett im Bundle, die App
//! versuchte `resource_dir().join("inputs")` zu lesen und crashte beim
//! Öffnen des AfA-Formulars (`afa-tabellen.json` not found). Die Re-Reviews
//! R1–R6 haben Code geprüft, nicht das Installer-Artefakt — Bug hat es
//! deshalb bis ins Release geschafft.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let inputs_src = manifest_dir
        .parent()
        .expect("CARGO_MANIFEST_DIR ohne Eltern-Verzeichnis")
        .join("inputs");
    let inputs_dst = manifest_dir.join("resources").join("inputs");

    // Subdirs, die ins Bundle wandern. `examples/` ist Repo-only (Test-Fixtures)
    // und wird ausgespart.
    const BUNDLED_SUBDIRS: &[&str] = &["specs", "pdf-templates", "mail-templates", "branding"];

    // Mirror jedes Mal neu aufbauen, damit gelöschte Source-Files auch im
    // Bundle verschwinden.
    if inputs_dst.exists() {
        fs::remove_dir_all(&inputs_dst).expect("resources/inputs/ konnte nicht entfernt werden");
    }
    fs::create_dir_all(&inputs_dst).expect("resources/inputs/ konnte nicht angelegt werden");

    for sub in BUNDLED_SUBDIRS {
        let src = inputs_src.join(sub);
        let dst = inputs_dst.join(sub);
        if src.is_dir() {
            copy_dir_recursive(&src, &dst).unwrap_or_else(|e| {
                panic!(
                    "Konnte {} nicht nach {} spiegeln: {e}",
                    src.display(),
                    dst.display()
                )
            });
        } else {
            // Subdir fehlt im Repo — wir legen das Ziel leer an, damit der
            // First-Run-Copy ueberhaupt etwas zu sehen bekommt (statt eines
            // fehlenden Verzeichnisses).
            fs::create_dir_all(&dst).expect("Bundle-Subdir konnte nicht angelegt werden");
        }
        // Cargo bei Aenderungen im Source neu bauen.
        println!("cargo:rerun-if-changed={}", src.display());
    }

    println!("cargo:rerun-if-changed={}", inputs_src.display());

    tauri_build::build();
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}
