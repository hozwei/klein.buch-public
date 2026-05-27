//! Branding-Assets (Firmen-Logo).
//!
//! Das hochgeladene Logo ist **maschinen-geschrieben** und lebt deshalb unter
//! `data/branding/logo.<ext>` — NICHT unter `inputs/` (Hard-Rule: `inputs/` ist
//! für Maschinen tabu). `seller_profile.logo_filename` referenziert die Datei.
//!
//! Reine Datei-Helfer (kein DB-Zugriff, kein AppHandle) → einfach testbar.
//! Die Tauri-Commands (`commands::settings`) lösen das Verzeichnis aus `Paths`
//! auf und rufen diese Funktionen.

use crate::error::{Error, Result};
use base64::Engine;
use std::path::{Path, PathBuf};

/// Von Typst (`image`) + Browser-Vorschau unterstützte Logo-Formate.
const ALLOWED_EXT: &[&str] = &["png", "jpg", "jpeg", "svg", "webp", "gif"];

/// Maximale Logo-Größe (2 MB) — Logos sind klein; schützt DB/PDF/Vorschau.
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Branding-Verzeichnis unterhalb von `data/`.
pub fn branding_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("branding")
}

fn ext_of(filename: &str) -> Option<String> {
    Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

fn mime_for(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

/// Speichert Asset-Bytes als `branding/<stem>.<ext>` (z. B. `logo.png`,
/// `signature.png`). Entfernt vorher andere `<stem>.*`-Dateien (Format-Wechsel).
/// Gibt den gespeicherten Dateinamen zurück.
fn store_asset(dir: &Path, stem: &str, bytes: &[u8], original_filename: &str) -> Result<String> {
    let ext = ext_of(original_filename)
        .filter(|e| ALLOWED_EXT.contains(&e.as_str()))
        .ok_or_else(|| {
            Error::Domain(
                "Nicht unterstütztes Bildformat. Erlaubt: PNG, JPG, SVG, WEBP, GIF.".into(),
            )
        })?;
    if bytes.is_empty() {
        return Err(Error::Domain("Die Datei ist leer.".into()));
    }
    if bytes.len() > MAX_BYTES {
        return Err(Error::Domain("Die Datei ist zu groß (max. 2 MB).".into()));
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| Error::Config(format!("branding-Ordner anlegen: {e}")))?;
    remove_existing(dir, stem);
    let filename = format!("{stem}.{ext}");
    std::fs::write(dir.join(&filename), bytes)
        .map_err(|e| Error::Config(format!("Datei schreiben: {e}")))?;
    Ok(filename)
}

/// Firmen-Logo speichern (`logo.<ext>`).
pub fn store_logo(dir: &Path, bytes: &[u8], original_filename: &str) -> Result<String> {
    store_asset(dir, "logo", bytes, original_filename)
}

/// Unterschrift speichern (`signature.<ext>`) — Angebots-Signatur.
pub fn store_signature(dir: &Path, bytes: &[u8], original_filename: &str) -> Result<String> {
    store_asset(dir, "signature", bytes, original_filename)
}

fn remove_existing(dir: &Path, stem: &str) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.file_stem().and_then(|s| s.to_str()) == Some(stem) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

/// Entfernt eine Branding-Datei (idempotent — fehlende Datei ist kein Fehler).
/// Filename-basiert, funktioniert für Logo und Signatur.
pub fn clear_logo(dir: &Path, filename: &str) -> Result<()> {
    let path = dir.join(filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| Error::Config(format!("Datei löschen: {e}")))?;
    }
    Ok(())
}

/// Findet `<stem>.*` im Branding-Ordner und liest es als `(Dateiname, Bytes)`.
fn find_asset(dir: &Path, stem: &str) -> Option<(String, Vec<u8>)> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && p.file_stem().and_then(|s| s.to_str()) == Some(stem) {
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            if let (Some(name), Ok(bytes)) = (name, std::fs::read(&p)) {
                return Some((name, bytes));
            }
        }
    }
    None
}

/// Findet das gespeicherte Logo (`logo.*`) — für den PDF-Render (Logo-B).
pub fn find_logo(dir: &Path) -> Option<(String, Vec<u8>)> {
    find_asset(dir, "logo")
}

/// Findet die gespeicherte Unterschrift (`signature.*`) — für den Angebots-Render.
pub fn find_signature(dir: &Path) -> Option<(String, Vec<u8>)> {
    find_asset(dir, "signature")
}

/// Liest das Logo und gibt eine Data-URL (`data:<mime>;base64,…`) für die
/// In-App-Vorschau zurück. `None`, wenn die Datei fehlt.
pub fn read_logo_data_url(dir: &Path, filename: &str) -> Result<Option<String>> {
    let path = dir.join(filename);
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(None);
    };
    let ext = ext_of(filename).unwrap_or_default();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{};base64,{}", mime_for(&ext), b64)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "kb-branding-test-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn store_replace_read_clear() {
        let base = tmp();
        let dir = branding_dir(&base);

        let name = store_logo(&dir, &[1, 2, 3, 4], "Mein Logo.PNG").unwrap();
        assert_eq!(name, "logo.png");
        assert!(dir.join("logo.png").exists());

        // Format-Wechsel: alte logo.* wird ersetzt.
        let name2 = store_logo(&dir, &[9, 9], "brand.svg").unwrap();
        assert_eq!(name2, "logo.svg");
        assert!(!dir.join("logo.png").exists());
        assert!(dir.join("logo.svg").exists());

        let url = read_logo_data_url(&dir, "logo.svg").unwrap().unwrap();
        assert!(url.starts_with("data:image/svg+xml;base64,"));

        clear_logo(&dir, "logo.svg").unwrap();
        assert!(!dir.join("logo.svg").exists());
        // idempotent
        clear_logo(&dir, "logo.svg").unwrap();
        assert!(read_logo_data_url(&dir, "logo.svg").unwrap().is_none());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_unsupported_and_empty() {
        let dir = branding_dir(&tmp());
        assert!(store_logo(&dir, &[1], "evil.exe").is_err());
        assert!(store_logo(&dir, &[], "x.png").is_err());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
