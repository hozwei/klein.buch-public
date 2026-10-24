//! Seed von `/inputs` aus dem Image mit Manifest (Plan W11 Punkt 6, ADR 0039).
//!
//! `/inputs/.seed-manifest.json` merkt sich je Datei den SHA-256 der zuletzt
//! geseedeten Fassung. Beim Start gilt je Datei der Image-Quelle:
//! - fehlt im Ziel → kopieren;
//! - im Ziel unverändert (Hash = Manifest) und das Image bringt eine neue
//!   Fassung → ersetzen, Hinweis in die Inbox;
//! - vom Nutzer geändert (Hash weder Manifest noch neue Fassung) → nie
//!   anfassen; die neue Fassung landet als `<name>.neu`, Hinweis in die Inbox.
//!
//! Die Hinweise stehen bis zum nächsten Entsperren im Manifest
//! (`pending_notices`), weil die Datenbank beim Start noch zu ist.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;

pub const MANIFEST: &str = ".seed-manifest.json";
/// Endung für neue Fassungen vom Nutzer geänderter Dateien.
pub const NEW_SUFFIX: &str = ".neu";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    #[serde(default)]
    pub image_version: String,
    /// Relativer Pfad (mit `/`) → SHA-256 (hex) der zuletzt geseedeten Fassung.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub pending_notices: Vec<Notice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

/// Was der Seed je Datei getan hat (für Tests und Log).
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub copied: Vec<String>,
    pub updated: Vec<String>,
    pub kept_with_new: Vec<String>,
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(std::fs::read(path)?)))
}

/// Schreibt über eine Temp-Datei im selben Ordner und benennt dann um.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.seed-tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn collect(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect(&path, base, out)?;
        } else {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            if rel == MANIFEST || rel.ends_with(NEW_SUFFIX) {
                continue;
            }
            out.push((rel, path));
        }
    }
    Ok(())
}

pub fn read_manifest(target: &Path) -> Manifest {
    std::fs::read(target.join(MANIFEST))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_manifest(target: &Path, m: &Manifest) -> Result<()> {
    write_atomic(&target.join(MANIFEST), &serde_json::to_vec_pretty(m)?)?;
    Ok(())
}

/// Seedet `bundled` nach `target` (siehe Moduldoku).
pub fn seed(bundled: &Path, target: &Path, image_version: &str) -> Result<Report> {
    std::fs::create_dir_all(target)?;
    let mut manifest = read_manifest(target);
    let before = manifest.clone();
    let mut files = Vec::new();
    collect(bundled, bundled, &mut files)?;
    files.sort();
    let mut report = Report::default();
    for (rel, src) in files {
        let new_hash = sha256_file(&src)?;
        let dst = target.join(&rel);
        if !dst.exists() {
            write_atomic(&dst, &std::fs::read(&src)?)?;
            manifest.files.insert(rel.clone(), new_hash);
            report.copied.push(rel);
            continue;
        }
        let dst_hash = sha256_file(&dst)?;
        if dst_hash == new_hash {
            manifest.files.insert(rel, new_hash);
            continue;
        }
        if manifest.files.get(&rel) == Some(&dst_hash) {
            // unverändert seit dem letzten Seed → neue Fassung übernehmen
            write_atomic(&dst, &std::fs::read(&src)?)?;
            manifest.files.insert(rel.clone(), new_hash);
            manifest.pending_notices.push(Notice {
                title: "Vorlage aktualisiert".into(),
                body: format!(
                    "Die mitgelieferte Datei „{rel}“ in /inputs wurde mit der neuen Fassung \
                     aus Klein.Buch {image_version} ersetzt (sie war unverändert)."
                ),
            });
            report.updated.push(rel);
            continue;
        }
        // vom Nutzer geändert → nie anfassen, neue Fassung daneben
        let neu = target.join(format!("{rel}{NEW_SUFFIX}"));
        let neu_hash = sha256_file(&neu).ok();
        if neu_hash.as_deref() != Some(new_hash.as_str()) {
            write_atomic(&neu, &std::fs::read(&src)?)?;
            manifest.pending_notices.push(Notice {
                title: "Neue Fassung einer geänderten Vorlage".into(),
                body: format!(
                    "Du hast „{rel}“ in /inputs geändert; sie bleibt unverändert. Die neue \
                     Fassung aus Klein.Buch {image_version} liegt daneben als „{rel}{NEW_SUFFIX}“ \
                     — bitte vergleichen und bei Bedarf übernehmen."
                ),
            });
            report.kept_with_new.push(rel);
        }
    }
    manifest.image_version = image_version.to_string();
    if manifest != before {
        write_manifest(target, &manifest)?;
    }
    Ok(report)
}

/// Offene Hinweise aus dem Manifest in die Inbox übernehmen (beim Entsperren)
/// und danach aus dem Manifest entfernen. Fehler sind nicht fatal.
pub async fn deliver_notices(pool: &sqlx::SqlitePool, target: &Path) -> Result<usize> {
    let mut manifest = read_manifest(target);
    if manifest.pending_notices.is_empty() {
        return Ok(0);
    }
    let notices = std::mem::take(&mut manifest.pending_notices);
    for n in &notices {
        crate::notify::store::create(
            pool,
            crate::notify::store::NewNotification {
                title: &n.title,
                body: &n.body,
                severity: "info",
                action_url: Some("/settings/pdf-templates"),
                ..Default::default()
            },
        )
        .await?;
    }
    write_manifest(target, &manifest)?;
    Ok(notices.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn read(dir: &Path, rel: &str) -> String {
        std::fs::read_to_string(dir.join(rel)).unwrap()
    }

    #[test]
    fn missing_files_are_copied_and_recorded() {
        let t = tempfile::tempdir().unwrap();
        let (src, dst) = (t.path().join("img"), t.path().join("inputs"));
        put(&src, "pdf-templates/default.typ", "v1");
        put(&src, "afa/tabelle.json", "{}");
        let r = seed(&src, &dst, "2026.6.0").unwrap();
        assert_eq!(r.copied.len(), 2);
        assert_eq!(read(&dst, "pdf-templates/default.typ"), "v1");
        let m = read_manifest(&dst);
        assert_eq!(m.image_version, "2026.6.0");
        assert_eq!(m.files.len(), 2);
        assert!(m.pending_notices.is_empty());
        // zweiter Lauf ohne Änderung: nichts zu tun
        assert_eq!(seed(&src, &dst, "2026.6.0").unwrap(), Report::default());
    }

    #[test]
    fn unchanged_file_is_replaced_by_new_version() {
        let t = tempfile::tempdir().unwrap();
        let (src, dst) = (t.path().join("img"), t.path().join("inputs"));
        put(&src, "pdf-templates/default.typ", "v1");
        seed(&src, &dst, "2026.6.0").unwrap();
        put(&src, "pdf-templates/default.typ", "v2");
        let r = seed(&src, &dst, "2026.7.0").unwrap();
        assert_eq!(r.updated, vec!["pdf-templates/default.typ".to_string()]);
        assert_eq!(read(&dst, "pdf-templates/default.typ"), "v2");
        assert!(!dst.join("pdf-templates/default.typ.neu").exists());
        let m = read_manifest(&dst);
        assert_eq!(m.pending_notices.len(), 1);
        assert_eq!(m.pending_notices[0].title, "Vorlage aktualisiert");
    }

    #[test]
    fn user_changed_file_is_never_touched() {
        let t = tempfile::tempdir().unwrap();
        let (src, dst) = (t.path().join("img"), t.path().join("inputs"));
        put(&src, "pdf-templates/default.typ", "v1");
        seed(&src, &dst, "2026.6.0").unwrap();
        put(&dst, "pdf-templates/default.typ", "meine Fassung");
        put(&src, "pdf-templates/default.typ", "v2");
        let r = seed(&src, &dst, "2026.7.0").unwrap();
        assert_eq!(
            r.kept_with_new,
            vec!["pdf-templates/default.typ".to_string()]
        );
        assert_eq!(read(&dst, "pdf-templates/default.typ"), "meine Fassung");
        assert_eq!(read(&dst, "pdf-templates/default.typ.neu"), "v2");
        assert_eq!(read_manifest(&dst).pending_notices.len(), 1);
        // erneuter Start: kein zweiter Hinweis für dieselbe neue Fassung
        let r = seed(&src, &dst, "2026.7.0").unwrap();
        assert!(r.kept_with_new.is_empty());
        assert_eq!(read_manifest(&dst).pending_notices.len(), 1);
        // Datei ohne Manifest-Eintrag, die vom Image abweicht, gilt als eigene
        put(&dst, "pdf-templates/eigene.typ", "x");
        put(&src, "pdf-templates/eigene.typ", "y");
        seed(&src, &dst, "2026.7.0").unwrap();
        assert_eq!(read(&dst, "pdf-templates/eigene.typ"), "x");
        assert_eq!(read(&dst, "pdf-templates/eigene.typ.neu"), "y");
    }
}
