//! Backup-Ziel-Abstraktion (Block 4 → G1-BKP, ADR 0034).
//!
//! Ein Backup-Ziel ist user-konfigurierbar (Settings-UI) und wird als JSON in
//! `app_settings` unter `backup_target` gehalten. Heute gibt es genau eine
//! Variante — [`BackupTarget::Directory`]: ein lokaler bzw. gemounteter Ordner
//! (USB, NAS, oder ein von OneDrive/iCloud/Dropbox/Nextcloud synchronisierter
//! Ordner). Klein.Buch schreibt dorthin nur eine **verschlüsselte** Datei —
//! keine Cloud-API. Liegt das Ziel in einem Sync-Ordner, ergibt sich die
//! Off-site-Replikation kostenlos; der Cloud-Provider sieht nur Blobs.
//!
//! Seit **G1-BKP.3** gibt es zusätzlich [`BackupTarget::Sftp`]: ein eigener
//! Server über SSH/SFTP (`russh`/`russh-sftp`). Auch dorthin schreibt Klein.Buch
//! nur die **verschlüsselte** `.kbk`-Datei. Das SFTP-Passwort liegt
//! ausschließlich im OS-Keychain (siehe [`super::sftp`]) — nie in DB/Log/audit.
//! Der Server-Host-Key wird über einen **gepinnten SHA-256-Fingerprint**
//! verifiziert (MITM-Schutz); Uploads ohne passenden Pin werden abgelehnt.
//!
//! Ist kein Ziel konfiguriert, wird `paths.backups_dir` (`<data_dir>/backups`)
//! als sicherer interner Default genutzt (Floor-Tier).

use crate::backup::{get_setting, set_setting};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Neuer, strukturierter Ziel-Key (JSON-serialisiertes [`BackupTarget`]).
pub const TARGET_KEY: &str = "backup_target";
/// Alt-Key (reiner Verzeichnis-Pfad als String) — nur noch **Lese-Fallback** für
/// Bestands-Installationen vor G1-BKP. Wird nicht mehr geschrieben.
pub const TARGET_KEY_DIR_LEGACY: &str = "backup_target_path";

/// Unterordner, den Default-Vorschlag und Auto-Detect unter einem Root anhängen.
const BACKUP_SUBDIR: &str = "Klein.Buch-Backups";

/// Konfiguriertes Backup-Ziel. Persistiert als JSON.
///
/// - Directory: `{"kind":"directory","path":…}`
/// - Sftp: `{"kind":"sftp","host":…,"port":22,"user":…,"remote_dir":…,"host_fingerprint":…}`
///
/// Das `rename_all = "snake_case"` greift nur auf die **Varianten-Namen**
/// (`Directory` → `directory`); die Struct-Felder sind bereits snake_case und
/// bleiben unverändert. Das SFTP-**Passwort** ist bewusst NICHT Teil dieses Enums
/// — es lebt im OS-Keychain (siehe [`super::sftp`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackupTarget {
    /// Lokaler/gemounteter Ordner (USB, NAS, Cloud-Sync-Ordner).
    Directory { path: String },
    /// Eigener SSH/SFTP-Server. `host_fingerprint` ist der gepinnte
    /// SHA-256-Host-Key-Fingerprint (`SHA256:…`); ohne ihn lehnt der Upload ab.
    Sftp {
        host: String,
        port: u16,
        user: String,
        remote_dir: String,
        host_fingerprint: Option<String>,
    },
}

/// Vom Auto-Detect gefundener Cloud-Ordner-Vorschlag fürs UI (1-Klick).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTarget {
    /// Anzeigename, z. B. „OneDrive" / „iCloud Drive".
    pub label: String,
    /// Vollständiger, vorgeschlagener Backup-Pfad (Cloud-Root + Unterordner).
    pub path: String,
}

// ---------------------------------------------------------------------------
// Persistenz
// ---------------------------------------------------------------------------

/// Liefert das konfigurierte Ziel, falls gesetzt. Liest zuerst den neuen
/// JSON-Key, fällt sonst auf den Alt-String-Key (Directory) zurück.
pub async fn get_target(pool: &SqlitePool) -> Result<Option<BackupTarget>> {
    if let Some(json) = get_setting(pool, TARGET_KEY).await? {
        if !json.trim().is_empty() {
            let t = serde_json::from_str::<BackupTarget>(&json)
                .map_err(|e| Error::Backup(format!("Backup-Ziel ungültig: {e}")))?;
            return Ok(Some(t));
        }
    }
    if let Some(p) = get_setting(pool, TARGET_KEY_DIR_LEGACY).await? {
        if !p.trim().is_empty() {
            return Ok(Some(BackupTarget::Directory { path: p }));
        }
    }
    Ok(None)
}

/// Persistiert das Ziel als JSON (UPSERT) + Audit-Eintrag (kein Geheimnis).
pub async fn set_target(pool: &SqlitePool, target: &BackupTarget) -> Result<()> {
    let json = serde_json::to_string(target)
        .map_err(|e| Error::Backup(format!("Backup-Ziel nicht serialisierbar: {e}")))?;
    set_setting(pool, TARGET_KEY, &json).await?;
    let detail = json.replace('\\', "\\\\").replace('"', "\\\"");
    crate::db::repo::audit_log::append(
        pool,
        "backup.target.set",
        "backup",
        "target",
        Some(&format!(r#"{{"target":"{detail}"}}"#)),
    )
    .await?;
    Ok(())
}

/// Bequemlichkeit für die Directory-Variante (Settings-UI sendet einen Pfad).
pub async fn set_directory_target(pool: &SqlitePool, path: &str) -> Result<()> {
    set_target(
        pool,
        &BackupTarget::Directory {
            path: path.to_string(),
        },
    )
    .await
}

/// Plattform-abhängiger Default-Vorschlag fürs UI: `~/Documents/Klein.Buch-Backups`.
/// Keine externe `dirs`-Crate — wir lesen die üblichen Env-Variablen.
pub fn default_suggestion() -> PathBuf {
    let home = std::env::var("USERPROFILE") // Windows
        .or_else(|_| std::env::var("HOME")) // Unix/macOS
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents").join(BACKUP_SUBDIR)
}

/// Ermittelt das effektive Ziel: konfiguriert → sonst Directory(Fallback).
/// Ein konfiguriertes, aber leeres Directory zählt als „nicht gesetzt".
pub async fn resolve_target(pool: &SqlitePool, fallback: &Path) -> Result<BackupTarget> {
    let fallback_target = || BackupTarget::Directory {
        path: fallback.to_string_lossy().to_string(),
    };
    match get_target(pool).await? {
        Some(BackupTarget::Directory { path }) if path.trim().is_empty() => Ok(fallback_target()),
        Some(t) => Ok(t),
        None => Ok(fallback_target()),
    }
}

/// Liefert das konfigurierte **Off-Site**-Ziel, sofern es vom lokalen Floor
/// (`floor_dir`) verschieden ist (G1-BKP.4, Tier-Modell).
///
/// Der lokale Floor (`paths.backups_dir`) wird **immer** geschrieben; das hier
/// zurückgegebene Ziel ist die **zusätzliche** Off-Site-Spiegelung. Daher zählen
/// ein leeres Directory **oder** ein Directory, das exakt auf den Floor zeigt, als
/// „kein Off-Site" (`None`) — sonst würde derselbe Ordner doppelt geschrieben.
/// SFTP ist immer Off-Site.
pub async fn offsite_target(pool: &SqlitePool, floor_dir: &Path) -> Result<Option<BackupTarget>> {
    match get_target(pool).await? {
        Some(BackupTarget::Directory { path }) => {
            let is_floor = {
                let t = path.trim();
                t.is_empty() || Path::new(t) == floor_dir
            };
            if is_floor {
                Ok(None)
            } else {
                Ok(Some(BackupTarget::Directory { path }))
            }
        }
        Some(sftp @ BackupTarget::Sftp { .. }) => Ok(Some(sftp)),
        None => Ok(None),
    }
}

/// Schreibt einen Backup-Blob ans Ziel und liefert den vollständigen Pfad/URI
/// zurück (für `backup_history.target_path`, später `backup_log.full_path`).
///
/// `async`, weil der SFTP-Arm netzwerkbasiert ist. Der Directory-Arm ist
/// synchrone Datei-I/O; der Sftp-Arm delegiert an [`super::sftp::upload`], das
/// das Passwort aus dem Keychain liest und den gepinnten Host-Fingerprint prüft.
pub async fn write_backup(target: &BackupTarget, file_name: &str, bytes: &[u8]) -> Result<String> {
    match target {
        BackupTarget::Directory { path } => {
            let dir = PathBuf::from(path);
            std::fs::create_dir_all(&dir)?;
            let full = dir.join(file_name);
            std::fs::write(&full, bytes)?;
            Ok(full.to_string_lossy().to_string())
        }
        BackupTarget::Sftp {
            host,
            port,
            user,
            remote_dir,
            host_fingerprint,
        } => {
            super::sftp::upload(
                host,
                *port,
                user,
                remote_dir,
                host_fingerprint.as_deref(),
                file_name,
                bytes,
            )
            .await
        }
    }
}

// ---------------------------------------------------------------------------
// Protokoll-Metadaten (G1-LOG, ADR 0034)
// ---------------------------------------------------------------------------

/// Liefert `(target_kind, target_label)` für einen `backup_log`-Eintrag.
///
/// - Directory (Off-Site-Ordner: USB/NAS/Cloud-Sync) → `("directory", None)`.
/// - Sftp → `("sftp", Some(host))`.
///
/// Der lokale **Floor** wird vom Aufrufer separat als `("local", None)`
/// protokolliert (er ist kein konfigurierbares Off-Site-Ziel).
pub fn log_meta(target: &BackupTarget) -> (&'static str, Option<String>) {
    match target {
        BackupTarget::Directory { .. } => ("directory", None),
        BackupTarget::Sftp { host, .. } => ("sftp", Some(host.clone())),
    }
}

/// Beste-Wissen-`full_path`-Beschreibung für die **Fehler**-Zeile, wenn
/// [`write_backup`] keinen echten Pfad liefern konnte (Ziel offline o. Ä.):
/// bei Directory der Zielordner + Dateiname, bei Sftp eine
/// `sftp://user@host:port/remote/dateiname`-URI (kein Passwort).
pub fn log_failed_path(target: &BackupTarget, file_name: &str) -> String {
    match target {
        BackupTarget::Directory { path } => PathBuf::from(path)
            .join(file_name)
            .to_string_lossy()
            .to_string(),
        BackupTarget::Sftp {
            host,
            port,
            user,
            remote_dir,
            ..
        } => {
            let rd = remote_dir.trim().trim_matches('/');
            if rd.is_empty() {
                format!("sftp://{user}@{host}:{port}/{file_name}")
            } else {
                format!("sftp://{user}@{host}:{port}/{rd}/{file_name}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-Detect (G1-BKP.2): OS-Cloud-Ordner als 1-Klick-Vorschlag
// ---------------------------------------------------------------------------

/// Eingabe-Umgebung für den (reinen, testbaren) Auto-Detect.
pub struct DetectEnv {
    /// OS-Kennung wie `std::env::consts::OS` (`"windows"`, `"macos"`, …).
    pub os: String,
    /// Home-Verzeichnis (USERPROFILE/HOME), falls bekannt.
    pub home: Option<PathBuf>,
    /// Relevante Umgebungsvariablen (z. B. `OneDrive`).
    pub env: HashMap<String, String>,
}

/// Pure: ermittelt Cloud-Ordner-Vorschläge anhand von OS, Env, Home und einem
/// `exists`-Prädikat (im Test injizierbar). Dedupliziert nach Pfad.
pub fn detect_core(e: &DetectEnv, exists: &dyn Fn(&Path) -> bool) -> Vec<DetectedTarget> {
    let mut out: Vec<DetectedTarget> = Vec::new();
    match e.os.as_str() {
        "windows" => {
            // OneDrive setzt diese Env-Variablen auf den lokalen Sync-Root.
            for (label, var) in [
                ("OneDrive", "OneDrive"),
                ("OneDrive", "OneDriveConsumer"),
                ("OneDrive (Business)", "OneDriveCommercial"),
            ] {
                if let Some(root) = e.env.get(var) {
                    if !root.trim().is_empty() {
                        maybe_add(&mut out, exists, label, PathBuf::from(root));
                    }
                }
            }
        }
        "macos" => {
            if let Some(home) = &e.home {
                maybe_add(
                    &mut out,
                    exists,
                    "iCloud Drive",
                    home.join("Library")
                        .join("Mobile Documents")
                        .join("com~apple~CloudDocs"),
                );
                // OneDrive auf dem Mac: klassisch ~/OneDrive und neue CloudStorage-Lage.
                maybe_add(&mut out, exists, "OneDrive", home.join("OneDrive"));
                maybe_add(
                    &mut out,
                    exists,
                    "OneDrive",
                    home.join("Library")
                        .join("CloudStorage")
                        .join("OneDrive-Personal"),
                );
            }
        }
        // Linux + alles Übrige: kein Auto-Detect → manuelle Ordnerwahl im UI.
        _ => {}
    }
    out
}

/// Fügt einen Vorschlag hinzu, wenn der Root existiert und der Pfad neu ist.
fn maybe_add(
    out: &mut Vec<DetectedTarget>,
    exists: &dyn Fn(&Path) -> bool,
    label: &str,
    root: PathBuf,
) {
    if !exists(&root) {
        return;
    }
    let path = root.join(BACKUP_SUBDIR).to_string_lossy().to_string();
    if out.iter().any(|d| d.path == path) {
        return;
    }
    out.push(DetectedTarget {
        label: label.to_string(),
        path,
    });
}

/// Imperative Shell: reale Env + Home + Dateisystem.
pub fn detect_cloud_targets() -> Vec<DetectedTarget> {
    let env: HashMap<String, String> = std::env::vars().collect();
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from);
    let e = DetectEnv {
        os: std::env::consts::OS.to_string(),
        home,
        env,
    };
    detect_core(&e, &|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn settings_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system', action TEXT NOT NULL,
                entity_type TEXT, entity_id TEXT, details_json TEXT) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    // ---- Persistenz / Resolution -----------------------------------------

    #[tokio::test]
    async fn target_unset_resolves_to_fallback_directory() {
        let pool = settings_pool().await;
        let tmp = tempfile::tempdir().unwrap();
        let fallback = tmp.path().join("backups");
        let t = resolve_target(&pool, &fallback).await.unwrap();
        assert_eq!(
            t,
            BackupTarget::Directory {
                path: fallback.to_string_lossy().to_string()
            }
        );
    }

    #[tokio::test]
    async fn set_then_get_directory_roundtrip() {
        let pool = settings_pool().await;
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("my-onedrive");
        set_directory_target(&pool, custom.to_str().unwrap())
            .await
            .unwrap();
        let got = get_target(&pool).await.unwrap().unwrap();
        assert_eq!(
            got,
            BackupTarget::Directory {
                path: custom.to_string_lossy().to_string()
            }
        );
        // resolve_target liefert das konfigurierte Ziel, nicht den Fallback.
        let resolved = resolve_target(&pool, tmp.path()).await.unwrap();
        assert_eq!(resolved, got);
    }

    #[tokio::test]
    async fn get_target_reads_legacy_plain_string_key() {
        let pool = settings_pool().await;
        set_setting(&pool, TARGET_KEY_DIR_LEGACY, "D:\\old-backups")
            .await
            .unwrap();
        let got = get_target(&pool).await.unwrap().unwrap();
        assert_eq!(
            got,
            BackupTarget::Directory {
                path: "D:\\old-backups".to_string()
            }
        );
    }

    #[tokio::test]
    async fn new_key_takes_precedence_over_legacy() {
        let pool = settings_pool().await;
        set_setting(&pool, TARGET_KEY_DIR_LEGACY, "D:\\old")
            .await
            .unwrap();
        set_directory_target(&pool, "E:\\new").await.unwrap();
        let got = get_target(&pool).await.unwrap().unwrap();
        assert_eq!(
            got,
            BackupTarget::Directory {
                path: "E:\\new".to_string()
            }
        );
    }

    #[tokio::test]
    async fn empty_configured_directory_falls_back() {
        let pool = settings_pool().await;
        set_directory_target(&pool, "   ").await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let fallback = tmp.path().join("backups");
        let t = resolve_target(&pool, &fallback).await.unwrap();
        assert_eq!(
            t,
            BackupTarget::Directory {
                path: fallback.to_string_lossy().to_string()
            }
        );
    }

    #[tokio::test]
    async fn write_backup_directory_writes_file_and_returns_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("nested").join("backups");
        let target = BackupTarget::Directory {
            path: dir.to_string_lossy().to_string(),
        };
        let full = write_backup(&target, "klein-buch-test.kbk", b"hello")
            .await
            .unwrap();
        assert!(Path::new(&full).is_file());
        assert_eq!(std::fs::read(&full).unwrap(), b"hello");
        assert!(full.ends_with("klein-buch-test.kbk"));
    }

    #[test]
    fn backup_target_json_roundtrip_tagged() {
        let t = BackupTarget::Directory {
            path: "C:\\x".to_string(),
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""kind":"directory""#));
        let back: BackupTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn default_suggestion_ends_with_klein_buch_backups() {
        assert!(default_suggestion().ends_with("Klein.Buch-Backups"));
    }

    // ---- SFTP-Variante (G1-BKP.3) ----------------------------------------

    #[test]
    fn backup_target_sftp_json_roundtrip_tagged() {
        let t = BackupTarget::Sftp {
            host: "backup.example.de".to_string(),
            port: 22,
            user: "manuel".to_string(),
            remote_dir: "klein-buch".to_string(),
            host_fingerprint: Some("SHA256:abc123".to_string()),
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""kind":"sftp""#));
        assert!(json.contains(r#""host":"backup.example.de""#));
        assert!(json.contains(r#""port":22"#));
        assert!(json.contains(r#""host_fingerprint":"SHA256:abc123""#));
        // Kein Passwort im persistierten Ziel (lebt im Keychain).
        assert!(!json.contains("password"));
        let back: BackupTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[tokio::test]
    async fn set_then_get_sftp_roundtrip_and_resolve_keeps_sftp() {
        let pool = settings_pool().await;
        let target = BackupTarget::Sftp {
            host: "nas.local".to_string(),
            port: 2222,
            user: "kb".to_string(),
            remote_dir: "/srv/backups/klein-buch".to_string(),
            host_fingerprint: None,
        };
        set_target(&pool, &target).await.unwrap();
        let got = get_target(&pool).await.unwrap().unwrap();
        assert_eq!(got, target);
        // resolve_target liefert das SFTP-Ziel unverändert (kein Directory-Fallback).
        let tmp = tempfile::tempdir().unwrap();
        let resolved = resolve_target(&pool, tmp.path()).await.unwrap();
        assert_eq!(resolved, target);
    }

    // ---- Off-Site-Ziel (G1-BKP.4 Tier-Modell) ----------------------------

    #[tokio::test]
    async fn offsite_none_when_unset() {
        let pool = settings_pool().await;
        let floor = PathBuf::from("/data/backups");
        assert_eq!(offsite_target(&pool, &floor).await.unwrap(), None);
    }

    #[tokio::test]
    async fn offsite_none_when_directory_equals_floor() {
        let pool = settings_pool().await;
        let floor = PathBuf::from("/data/backups");
        set_directory_target(&pool, "/data/backups").await.unwrap();
        // Off-Site == Floor → kein zusätzliches Ziel (sonst doppelt geschrieben).
        assert_eq!(offsite_target(&pool, &floor).await.unwrap(), None);
    }

    #[tokio::test]
    async fn offsite_none_when_directory_blank() {
        let pool = settings_pool().await;
        let floor = PathBuf::from("/data/backups");
        set_directory_target(&pool, "   ").await.unwrap();
        assert_eq!(offsite_target(&pool, &floor).await.unwrap(), None);
    }

    #[tokio::test]
    async fn offsite_some_when_directory_differs_from_floor() {
        let pool = settings_pool().await;
        let floor = PathBuf::from("/data/backups");
        set_directory_target(&pool, "/mnt/onedrive/kb")
            .await
            .unwrap();
        assert_eq!(
            offsite_target(&pool, &floor).await.unwrap(),
            Some(BackupTarget::Directory {
                path: "/mnt/onedrive/kb".to_string()
            })
        );
    }

    #[tokio::test]
    async fn offsite_some_for_sftp() {
        let pool = settings_pool().await;
        let floor = PathBuf::from("/data/backups");
        let sftp = BackupTarget::Sftp {
            host: "nas.local".to_string(),
            port: 22,
            user: "kb".to_string(),
            remote_dir: "backups".to_string(),
            host_fingerprint: Some("SHA256:x".to_string()),
        };
        set_target(&pool, &sftp).await.unwrap();
        assert_eq!(offsite_target(&pool, &floor).await.unwrap(), Some(sftp));
    }

    // ---- Protokoll-Metadaten (G1-LOG) ------------------------------------

    #[test]
    fn log_meta_maps_directory_and_sftp() {
        let dir = BackupTarget::Directory {
            path: "/mnt/usb/kb".into(),
        };
        assert_eq!(log_meta(&dir), ("directory", None));

        let sftp = BackupTarget::Sftp {
            host: "backup.example.de".into(),
            port: 22,
            user: "manuel".into(),
            remote_dir: "klein-buch".into(),
            host_fingerprint: Some("SHA256:x".into()),
        };
        assert_eq!(log_meta(&sftp), ("sftp", Some("backup.example.de".into())));
    }

    #[test]
    fn log_failed_path_builds_directory_path_and_sftp_uri() {
        let dir = BackupTarget::Directory {
            path: "/mnt/usb/kb".into(),
        };
        let p = log_failed_path(&dir, "klein-buch-1.kbk");
        assert!(p.ends_with("klein-buch-1.kbk"));
        assert!(p.contains("kb"));

        let sftp = BackupTarget::Sftp {
            host: "backup.example.de".into(),
            port: 2222,
            user: "manuel".into(),
            remote_dir: "/klein-buch/".into(),
            host_fingerprint: None,
        };
        assert_eq!(
            log_failed_path(&sftp, "klein-buch-1.kbk"),
            "sftp://manuel@backup.example.de:2222/klein-buch/klein-buch-1.kbk"
        );

        // Leerer remote_dir → kein doppelter Slash.
        let sftp_root = BackupTarget::Sftp {
            host: "h".into(),
            port: 22,
            user: "u".into(),
            remote_dir: "".into(),
            host_fingerprint: None,
        };
        assert_eq!(log_failed_path(&sftp_root, "f.kbk"), "sftp://u@h:22/f.kbk");
    }

    // ---- Auto-Detect (pure core) -----------------------------------------

    fn env_with(os: &str, home: Option<&str>, vars: &[(&str, &str)]) -> DetectEnv {
        DetectEnv {
            os: os.to_string(),
            home: home.map(PathBuf::from),
            env: vars
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn detect_windows_onedrive_when_env_and_dir_exist() {
        let e = env_with("windows", None, &[("OneDrive", "C:\\Users\\m\\OneDrive")]);
        let found = detect_core(&e, &|_p| true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "OneDrive");
        assert!(found[0].path.starts_with("C:\\Users\\m\\OneDrive"));
        assert!(found[0].path.ends_with("Klein.Buch-Backups"));
    }

    #[test]
    fn detect_skips_when_root_missing() {
        let e = env_with("windows", None, &[("OneDrive", "C:\\Users\\m\\OneDrive")]);
        let found = detect_core(&e, &|_p| false);
        assert!(found.is_empty());
    }

    #[test]
    fn detect_macos_icloud_only_when_present() {
        let e = env_with("macos", Some("/Users/m"), &[]);
        // exists nur für den iCloud-Pfad.
        let found = detect_core(&e, &|p| p.to_string_lossy().contains("CloudDocs"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "iCloud Drive");
    }

    #[test]
    fn detect_empty_env_var_ignored() {
        let e = env_with("windows", None, &[("OneDrive", "   ")]);
        assert!(detect_core(&e, &|_p| true).is_empty());
    }

    #[test]
    fn detect_other_os_returns_empty() {
        let e = env_with(
            "linux",
            Some("/home/m"),
            &[("OneDrive", "/home/m/OneDrive")],
        );
        assert!(detect_core(&e, &|_p| true).is_empty());
    }
}
