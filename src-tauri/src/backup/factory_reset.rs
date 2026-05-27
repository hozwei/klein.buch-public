//! Factory Reset — Datei-Nuke-Phase (G1-RESET, ADR 0036), Imperative Shell.
//!
//! Zweiphasig wie der Restore (race-frei unter Windows-File-Lock):
//!
//! - **Phase A** ([`request`], App läuft): nur einen **Marker** ins `data_dir`
//!   schreiben (mit den zu löschenden Keychain-Service-IDs) und Neustart
//!   anfordern. Es wird NICHTS gelöscht und der DB-Pool NICHT geschlossen — sonst
//!   liefen gleichzeitige/nachfolgende Commands in „attempted to acquire a
//!   connection on a closed pool".
//! - **Phase B** ([`apply_pending`], App-Start, **vor** dem Pool-Open): den
//!   gesamten `data_dir`-Inhalt löschen, Kern-Verzeichnisse leer neu anlegen und
//!   die app-eigenen Keychain-Geheimnisse entfernen. Danach startet die App leer
//!   → Onboarding.
//!
//! Der Marker liegt im Dateisystem (nicht in der DB!), weil die DB selbst
//! gelöscht wird. `inputs/` liegt außerhalb von `data_dir` und bleibt unberührt.

use crate::config::Paths;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs;

/// Name der Factory-Reset-Marker-Datei im `data_dir`.
pub const RESET_MARKER: &str = "FACTORY_RESET_PENDING.json";

/// Inhalt des Reset-Markers: alles, was die Nuke-Phase nach dem DB-Löschen noch
/// braucht (die Keychain-Service-IDs sind danach nicht mehr ermittelbar).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResetMarker {
    /// Keychain-Service-IDs der Mail-Konten, deren Secrets (SMTP + OAuth) beim
    /// Reset gelöscht werden sollen.
    #[serde(default)]
    pub mail_services: Vec<String>,
}

/// Phase A: Marker schreiben (App läuft). Löscht **nichts**, schließt **nichts**.
pub fn request(paths: &Paths, mail_services: &[String]) -> Result<()> {
    fs::create_dir_all(&paths.data_dir)?;
    let marker = ResetMarker {
        mail_services: mail_services.to_vec(),
    };
    fs::write(
        paths.data_dir.join(RESET_MARKER),
        serde_json::to_vec_pretty(&marker)?,
    )?;
    Ok(())
}

/// Phase B: einen vorgemerkten Reset anwenden (App-Start, vor Pool-Open).
/// `Ok(true)`, wenn gelöscht wurde; `Ok(false)`, wenn kein Marker vorlag.
pub fn apply_pending(paths: &Paths) -> Result<bool> {
    let marker_path = paths.data_dir.join(RESET_MARKER);
    if !marker_path.exists() {
        return Ok(false);
    }
    // Marker lesen, BEVOR `data_dir` geleert wird (er liegt selbst darin). Ein
    // beschädigter Marker darf den Reset nicht blockieren → Default (leer).
    let marker: ResetMarker = fs::read(&marker_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();

    // `data_dir` vollständig leeren (DB+WAL/SHM, Archiv inkl. read-only `0o400`,
    // Floor-Backups, Branding, Exporte, Restore-Staging, der Marker selbst) und
    // die Kern-Verzeichnisse leer neu anlegen.
    crate::backup::restore::clear_dir_force(&paths.data_dir)?;
    fs::create_dir_all(&paths.data_dir)?;
    fs::create_dir_all(&paths.archive_dir)?;
    fs::create_dir_all(&paths.backups_dir)?;

    // Keychain best-effort (Geräte-Weitergabe): Mail-SMTP/OAuth je Konto +
    // SFTP-Backup-Passwort. Ein Fehler hier darf den Reset nicht scheitern lassen.
    for svc in &marker.mail_services {
        let _ = crate::mail::keyring::delete_password(svc);
        let _ = crate::mail::keyring::delete_oauth_tokens(svc);
    }
    let _ = crate::backup::sftp::delete_password();

    tracing::warn!("Factory Reset angewandt — lokale Instanz geleert (Onboarding).");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use std::path::Path;
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

    #[test]
    fn apply_pending_noop_without_marker() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.data_dir).unwrap();
        assert!(!apply_pending(&paths).unwrap());
    }

    #[test]
    fn request_then_apply_wipes_data_keeps_offsite() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(paths.archive_dir.join("2026")).unwrap();
        fs::create_dir_all(&paths.backups_dir).unwrap();
        fs::write(&paths.db_file, b"DB").unwrap();
        fs::write(paths.archive_dir.join("2026/beleg.pdf"), b"x").unwrap();
        // read-only Archiv-Datei (wie 0o400 in Produktion).
        let ro = paths.archive_dir.join("2026/locked.xml");
        fs::write(&ro, b"<xml/>").unwrap();
        let mut perms = fs::metadata(&ro).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&ro, perms).unwrap();
        fs::write(paths.backups_dir.join("floor.kbk"), b"f").unwrap();
        // Off-Site außerhalb von data_dir.
        let off = dir.path().parent().unwrap().join("fr-offsite");
        fs::create_dir_all(&off).unwrap();
        fs::write(off.join("off.kbk"), b"o").unwrap();

        // Phase A: Marker (keine Mail-Services → kein echter Keychain-Zugriff).
        request(&paths, &[]).unwrap();
        assert!(paths.data_dir.join(RESET_MARKER).exists());
        assert!(paths.db_file.exists(), "Phase A löscht nichts");

        // Phase B.
        assert!(apply_pending(&paths).unwrap());
        assert!(!paths.db_file.exists());
        assert!(!paths.archive_dir.join("2026/beleg.pdf").exists());
        assert!(
            !paths.archive_dir.join("2026/locked.xml").exists(),
            "auch read-only Archiv-Dateien müssen weg sein"
        );
        assert!(!paths.backups_dir.join("floor.kbk").exists());
        assert!(!paths.data_dir.join(RESET_MARKER).exists());
        assert!(paths.archive_dir.is_dir() && paths.backups_dir.is_dir());
        assert!(off.join("off.kbk").is_file(), "Off-Site bleibt bestehen");

        // Idempotent: zweiter Lauf ohne Marker = No-op.
        assert!(!apply_pending(&paths).unwrap());
        let _ = fs::remove_dir_all(&off);
    }
}
