//! Backup-Ziele (ADR 0034 + Amendment 2026-10-23, Plan W7).
//!
//! Neben dem lokalen Floor (`<data>/backups`, immer geschrieben) gibt es
//! beliebig viele **Off-Site-Ziele**, gespeichert als JSON-Array in
//! `app_settings.backup_targets` (`[{id, label, kind, …}]`):
//!
//! - [`BackupTarget::Directory`]: ein Ordner unter der Mount-Wurzel
//!   `/backup-targets` (`KLEIN_BUCH_BACKUP_TARGETS_DIR`), gespeichert **relativ**
//!   zur Wurzel; beim Schreiben per `canonicalize` gegen die Wurzel geprüft (keine
//!   Symlinks nach draußen). Was dahinter liegt (NAS, Cloud-Sync, USB), ist
//!   Sache des Betreibers — keine Cloud-API im Code.
//! - [`BackupTarget::Sftp`]: eigener SSH/SFTP-Server mit gepinntem
//!   SHA-256-Host-Key-Fingerprint; das Passwort liegt je Ziel-ID im
//!   Schlüsselspeicher (nie in DB/Log/Audit, siehe [`super::sftp`]).
//!
//! Klein.Buch schreibt überall nur die **verschlüsselte** `.kbk`. Lesefallback:
//! die alten Einzel-Keys `backup_target` (JSON) und `backup_target_path`
//! (String) — ein Verzeichnis außerhalb der Wurzel wird dabei verworfen.

use crate::backup::{get_setting, set_setting};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Component, Path, PathBuf};

/// Liste der Off-Site-Ziele (JSON-Array von [`TargetEntry`]).
pub const TARGETS_KEY: &str = "backup_targets";
/// Alter Einzel-Ziel-Key (JSON-serialisiertes [`BackupTarget`]) — nur Lesefallback.
pub const TARGET_KEY: &str = "backup_target";
/// Ältester Key (reiner Verzeichnis-Pfad) — nur Lesefallback.
pub const TARGET_KEY_DIR_LEGACY: &str = "backup_target_path";
/// Ergebnis des letzten Laufs je Ziel (für den Status `offsite`).
pub const RESULTS_KEY: &str = "backup_offsite_results";
/// Höchstzahl der Off-Site-Ziele.
pub const MAX_TARGETS: usize = 10;

/// Art eines Backup-Ziels. Persistiert getaggt (`kind`).
///
/// - Directory: `{"kind":"directory","path":"nas/klein-buch"}` (relativ zur Wurzel;
///   der lokale Floor wird intern mit absolutem Pfad beschrieben)
/// - Sftp: `{"kind":"sftp","host":…,"port":22,"user":…,"remote_dir":…,"host_fingerprint":…}`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackupTarget {
    /// Ordner (relativ zur Ziel-Wurzel bzw. absolut für den Floor).
    Directory { path: String },
    /// Eigener SSH/SFTP-Server. Ohne `host_fingerprint` lehnt der Upload ab.
    Sftp {
        host: String,
        port: u16,
        user: String,
        remote_dir: String,
        host_fingerprint: Option<String>,
    },
}

/// Ein konfiguriertes Off-Site-Ziel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetEntry {
    pub id: String,
    pub label: String,
    #[serde(flatten)]
    pub target: BackupTarget,
}

/// Ergebnis des letzten Laufs eines Ziels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetResult {
    pub ok: bool,
    pub at: String,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Persistenz
// ---------------------------------------------------------------------------

/// Konfigurierte Off-Site-Ziele. Liest `backup_targets`, sonst die alten
/// Einzel-Keys (Directory nur, wenn es unter `root` liegt).
pub async fn list_targets(pool: &SqlitePool, root: &Path) -> Result<Vec<TargetEntry>> {
    if let Some(json) = get_setting(pool, TARGETS_KEY).await? {
        if !json.trim().is_empty() {
            return serde_json::from_str::<Vec<TargetEntry>>(&json)
                .map_err(|e| Error::Backup(format!("Backup-Ziele ungültig: {e}")));
        }
    }
    let legacy = match get_setting(pool, TARGET_KEY).await? {
        Some(json) if !json.trim().is_empty() => Some(
            serde_json::from_str::<BackupTarget>(&json)
                .map_err(|e| Error::Backup(format!("Backup-Ziel ungültig: {e}")))?,
        ),
        _ => get_setting(pool, TARGET_KEY_DIR_LEGACY)
            .await?
            .filter(|p| !p.trim().is_empty())
            .map(|path| BackupTarget::Directory { path }),
    };
    Ok(legacy
        .and_then(|t| legacy_entry(t, root))
        .into_iter()
        .collect())
}

/// Altes Einzel-Ziel als Eintrag; absolute Verzeichnisse nur unter `root`.
fn legacy_entry(target: BackupTarget, root: &Path) -> Option<TargetEntry> {
    match target {
        BackupTarget::Directory { path } => {
            let p = Path::new(path.trim());
            let rel = if p.is_absolute() {
                match p.strip_prefix(root) {
                    Ok(rel) => rel.to_string_lossy().to_string(),
                    Err(_) => {
                        tracing::warn!(
                            "Altes Backup-Ziel {} liegt nicht unter {} — wird ignoriert",
                            p.display(),
                            root.display()
                        );
                        return None;
                    }
                }
            } else {
                path.trim().to_string()
            };
            check_relative(&rel).ok()?;
            Some(TargetEntry {
                id: "verzeichnis".into(),
                label: rel.clone(),
                target: BackupTarget::Directory { path: rel },
            })
        }
        sftp @ BackupTarget::Sftp { .. } => {
            let label = match &sftp {
                BackupTarget::Sftp { host, .. } => host.clone(),
                BackupTarget::Directory { .. } => String::new(),
            };
            Some(TargetEntry {
                id: "sftp".into(),
                label,
                target: sftp,
            })
        }
    }
}

/// Speichert die Ziel-Liste (UPSERT) + Audit (ohne Geheimnisse).
pub async fn save_targets(pool: &SqlitePool, targets: &[TargetEntry]) -> Result<()> {
    if targets.len() > MAX_TARGETS {
        return Err(Error::Domain(format!(
            "Höchstens {MAX_TARGETS} Backup-Ziele."
        )));
    }
    let json = serde_json::to_string(targets)?;
    set_setting(pool, TARGETS_KEY, &json).await?;
    let summary: Vec<String> = targets
        .iter()
        .map(|t| format!("{}:{}", kind_of(&t.target), t.id))
        .collect();
    crate::db::repo::audit_log::append(
        pool,
        "backup.targets.set",
        "backup",
        "targets",
        Some(serde_json::json!({ "targets": summary })),
    )
    .await?;
    Ok(())
}

/// Letzte Ergebnisse je Ziel-ID.
pub async fn load_results(
    pool: &SqlitePool,
) -> Result<std::collections::BTreeMap<String, TargetResult>> {
    Ok(get_setting(pool, RESULTS_KEY)
        .await?
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

/// Ergebnisse eines Laufs übernehmen (entfernte Ziele fallen heraus).
pub async fn store_results(
    pool: &SqlitePool,
    targets: &[TargetEntry],
    run: &[(String, TargetResult)],
) -> Result<()> {
    let mut all = load_results(pool).await?;
    for (id, r) in run {
        all.insert(id.clone(), r.clone());
    }
    all.retain(|id, _| targets.iter().any(|t| &t.id == id));
    set_setting(pool, RESULTS_KEY, &serde_json::to_string(&all)?).await
}

/// Höchstalter einer Off-Site-Kopie in Tagen, bevor der Status `stale` lautet
/// (Standard; die Regel `backup_overdue` kann es über `max_age_days` ändern).
pub const OFFSITE_MAX_AGE_DAYS: i64 = 7;

/// Status der Off-Site-Sicherung (Plan W7, Lauf F3a W7-OFFSITE-STALE):
///
/// - `missing`: kein Ziel eingerichtet;
/// - `failing`: ein Ziel scheiterte beim letzten Lauf;
/// - `stale`: kein Ziel scheiterte, aber mindestens eines hat noch keine
///   erfolgreiche Kopie oder seine letzte ist älter als `max_age_days` der
///   Regel `backup_overdue` (Standard [`OFFSITE_MAX_AGE_DAYS`], Berliner
///   Kalendertage wie die Regel) — etwa während einer Pause nach einem
///   Restore (Spiegelung ausgesetzt) oder wenn keine Backups mehr laufen;
/// - `ok`.
pub async fn offsite_status(
    pool: &SqlitePool,
    root: &Path,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<&'static str> {
    let targets = list_targets(pool, root).await?;
    if targets.is_empty() {
        return Ok("missing");
    }
    let results = load_results(pool).await?;
    let failing = targets
        .iter()
        .any(|t| results.get(&t.id).is_some_and(|r| !r.ok));
    if failing {
        return Ok("failing");
    }
    let max_age = crate::notify::rules::get(pool, "rule_backup_overdue")
        .await
        .ok()
        .flatten()
        .map(|r| r.config_i64("max_age_days", OFFSITE_MAX_AGE_DAYS))
        .unwrap_or(OFFSITE_MAX_AGE_DAYS);
    let today = crate::clock::berlin_date(now);
    let stale = targets.iter().any(|t| match results.get(&t.id) {
        None => true,
        Some(r) => match chrono::DateTime::parse_from_rfc3339(&r.at) {
            Ok(at) => {
                let day = at.with_timezone(&chrono_tz::Europe::Berlin).date_naive();
                (today - day).num_days() > max_age
            }
            // Unlesbarer Zeitpunkt: keine Aussage über das Alter.
            Err(_) => false,
        },
    });
    Ok(if stale { "stale" } else { "ok" })
}

/// Zeitpunkt der jüngsten erfolgreichen Kopie auf einem Ziel (RFC 3339,
/// Europe/Berlin; Lauf 2026.11 U4): das letzte Ergebnis, wenn es erfolgreich
/// war, sonst der jüngste erfolgreiche Eintrag im Backup-Protokoll mit Art,
/// Bezeichnung **und Ort** des Ziels. `None` = noch keine Kopie bekannt.
///
/// U7 (R2-03): Bezeichnungen sind frei wählbar und nicht eindeutig (Standard:
/// Ordner bzw. Host). Ohne den Ort übernähme ein Ziel die Kopien eines entfernten
/// oder eines zweiten Ziels mit derselben Bezeichnung (etwa SFTP auf demselben
/// Host mit anderem Remote-Ordner).
pub async fn last_copy_at(
    pool: &SqlitePool,
    entry: &TargetEntry,
    last: Option<&TargetResult>,
) -> Result<Option<String>> {
    if let Some(r) = last.filter(|r| r.ok) {
        return Ok(Some(r.at.clone()));
    }
    let (kind, label) = log_meta(entry);
    let location = log_location(&entry.target);
    let newest: Option<String> = sqlx::query_scalar(
        "SELECT MAX(created_at) FROM backup_log
          WHERE status = 'ok' AND target_kind = ?1 AND target_label = ?2
            AND substr(full_path, -length(?3 || file_name)) = ?3 || file_name",
    )
    .bind(kind)
    .bind(label)
    .bind(location)
    .fetch_one(pool)
    .await?;
    Ok(newest
        .as_deref()
        .and_then(crate::clock::db_utc_to_berlin)
        .map(|t| t.to_rfc3339()))
}

/// Stand der Off-Site-Sicherung (Lauf 2026.11 U4): der älteste der je Ziel
/// jüngsten Kopie-Zeitpunkte ([`last_copy_at`]) und sein Alter in Berliner
/// Kalendertagen (0 = heute; gerechnet wie der Status `stale`). `None`, wenn
/// kein Ziel eingerichtet ist oder ein Ziel noch keine (lesbare) Kopie hat.
pub fn oldest_copy(
    copies: &[Option<String>],
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(String, i64)> {
    let mut oldest: Option<(chrono::DateTime<chrono::FixedOffset>, &String)> = None;
    for copy in copies {
        let text = copy.as_ref()?;
        let at = chrono::DateTime::parse_from_rfc3339(text).ok()?;
        if oldest.is_none_or(|(o, _)| at < o) {
            oldest = Some((at, text));
        }
    }
    let (at, text) = oldest?;
    let day = at.with_timezone(&chrono_tz::Europe::Berlin).date_naive();
    let age = (crate::clock::berlin_date(now) - day).num_days().max(0);
    Some((text.clone(), age))
}

pub fn kind_of(target: &BackupTarget) -> &'static str {
    match target {
        BackupTarget::Directory { .. } => "directory",
        BackupTarget::Sftp { .. } => "sftp",
    }
}

// ---------------------------------------------------------------------------
// Verzeichnis-Ziele unter der Mount-Wurzel
// ---------------------------------------------------------------------------

/// Relativer Pfad aus normalen Bestandteilen (kein `..`, kein `/`, kein `C:`).
pub fn check_relative(rel: &str) -> Result<()> {
    let p = Path::new(rel);
    let ok = !rel.trim().is_empty()
        && !rel.contains(['\\', ':', '\0'])
        && p.components().all(|c| matches!(c, Component::Normal(_)));
    if ok {
        Ok(())
    } else {
        Err(Error::Domain(format!(
            "Ungültiger Zielordner „{rel}“ — erwartet z. B. „nas/klein-buch“ unterhalb der \
             Backup-Ziel-Wurzel."
        )))
    }
}

/// Löst ein Verzeichnis-Ziel unter `root` auf (Plan W7): Wurzel und jeder
/// vorhandene Vorfahr müssen nach `canonicalize` unter der Wurzel liegen;
/// fehlende Unterordner werden angelegt. Symlinks nach draußen → Fehler.
pub fn resolve_directory(root: &Path, rel: &str) -> Result<PathBuf> {
    check_relative(rel)?;
    let root_real = root.canonicalize().map_err(|e| {
        Error::Backup(format!(
            "Backup-Ziel-Wurzel {} fehlt ({e}) — Volume einbinden.",
            root.display()
        ))
    })?;
    let outside = || {
        Error::Domain(format!(
            "Backup-Ziel „{rel}“ zeigt aus {} hinaus — abgelehnt.",
            root.display()
        ))
    };
    // Die erste Ebene ist das eingebundene Volume — nie selbst anlegen, sonst
    // landen Backups im Container (Gegenprüfung W7).
    let volume = rel.split('/').next().unwrap_or_default();
    if !root_real.join(volume).is_dir() {
        return Err(Error::Domain(format!(
            "Volume {}/{volume} ist nicht eingebunden.",
            root.display()
        )));
    }
    let full = root_real.join(rel);
    let mut existing = full.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or_else(outside)?;
    }
    if !existing.canonicalize()?.starts_with(&root_real) {
        return Err(outside());
    }
    std::fs::create_dir_all(&full)?;
    let real = full.canonicalize()?;
    if !real.starts_with(&root_real) || real == root_real || !real.is_dir() {
        return Err(outside());
    }
    Ok(real)
}

/// Unterordner der Wurzel (eine Ebene, ohne versteckte) zur Auswahl.
pub fn available_dirs(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort();
    out
}

/// Ziel zum Schreiben: Verzeichnisse absolut und geprüft, SFTP unverändert.
pub fn resolve(entry: &TargetEntry, root: &Path) -> Result<BackupTarget> {
    match &entry.target {
        BackupTarget::Directory { path } => Ok(BackupTarget::Directory {
            path: resolve_directory(root, path)?.to_string_lossy().to_string(),
        }),
        sftp @ BackupTarget::Sftp { .. } => Ok(sftp.clone()),
    }
}

/// Gruppe eines `backup_history.target_path` für die Rotation je Ziel: der
/// enthaltende Ordner bzw. die SFTP-URI ohne Dateinamen.
pub fn group_key(target_path: &str) -> String {
    match target_path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// Schreibt `bytes` dauerhaft nach `full` (Plan W2.3): Temp-Datei im selben
/// Verzeichnis, `fsync`, `rename`, Verzeichnis-`fsync`. Ein Abbruch hinterlässt
/// nie eine halbe `.kbk` unter dem endgültigen Namen.
fn write_file_durable(full: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let name = full
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "backup".into());
    let tmp = full.with_file_name(format!(".{name}.{}.part", uuid::Uuid::now_v7().simple()));
    let result = (|| -> Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, full)?;
        if let Some(parent) = full.parent() {
            if let Ok(d) = std::fs::File::open(parent) {
                let _ = d.sync_all();
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Schreibt einen Backup-Blob ans Ziel und liefert den vollständigen Pfad/URI
/// (für `backup_history.target_path` und `backup_log.full_path`). Verzeichnisse
/// müssen absolut sein (vorher [`resolve`]); der SFTP-Arm bekommt das Passwort
/// des Ziels (aus `crate::secrets`) und prüft den gepinnten Host-Fingerprint.
pub async fn write_backup(
    target: &BackupTarget,
    password: Option<&str>,
    file_name: &str,
    bytes: &[u8],
) -> Result<String> {
    write_backup_shared(
        target,
        password,
        file_name,
        std::sync::Arc::new(bytes.to_vec()),
    )
    .await
}

/// Wie [`write_backup`], mit geteiltem Puffer (Lauf F3a): Floor und jedes
/// Ziel schreiben dieselbe Datei, ohne sie je Ziel zu kopieren.
pub async fn write_backup_shared(
    target: &BackupTarget,
    password: Option<&str>,
    file_name: &str,
    bytes: std::sync::Arc<Vec<u8>>,
) -> Result<String> {
    match target {
        BackupTarget::Directory { path } => {
            // blockierende Datei-I/O außerhalb der Runtime (hängende Mounts)
            let dir = PathBuf::from(path);
            let full = dir.join(file_name);
            let data = bytes;
            let target = full.clone();
            tokio::task::spawn_blocking(move || -> Result<()> {
                std::fs::create_dir_all(&dir)?;
                write_file_durable(&target, &data)
            })
            .await
            .map_err(|e| Error::Backup(format!("Schreiben abgebrochen: {e}")))??;
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
                password,
                host,
                *port,
                user,
                remote_dir,
                host_fingerprint.as_deref(),
                file_name,
                &bytes,
            )
            .await
        }
    }
}

// ---------------------------------------------------------------------------
// Protokoll-Metadaten (G1-LOG, ADR 0034)
// ---------------------------------------------------------------------------

/// `(target_kind, target_label)` für einen `backup_log`-Eintrag eines
/// Off-Site-Ziels (der Floor wird als `("local", None)` protokolliert).
pub fn log_meta(entry: &TargetEntry) -> (&'static str, Option<String>) {
    (kind_of(&entry.target), Some(entry.label.clone()))
}

/// Ort eines Ziels, wie ihn `full_path` einer erfolgreichen Kopie im
/// Backup-Protokoll unmittelbar vor dem Dateinamen trägt (U7, R2-03): bei
/// Verzeichnissen das Ende `/<Ordner>/` (davor steht die aufgelöste Wurzel), bei
/// SFTP die URI bis zum Dateinamen wie in [`super::sftp::upload`].
fn log_location(target: &BackupTarget) -> String {
    match target {
        BackupTarget::Directory { path } => format!("/{}/", path.trim().trim_matches('/')),
        BackupTarget::Sftp {
            host,
            port,
            user,
            remote_dir,
            ..
        } => super::sftp::sftp_uri(user, host, *port, &super::sftp::remote_join(remote_dir, "")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn settings_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            // ADR 0042 Nr. 16 (V6-R01): Test-Pool mit demselben Zeitlimit.
            .acquire_timeout(crate::db::ACQUIRE_TIMEOUT)
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

    fn sftp() -> BackupTarget {
        BackupTarget::Sftp {
            host: "backup.example.org".into(),
            port: 22,
            user: "kb".into(),
            remote_dir: "klein-buch".into(),
            host_fingerprint: Some("SHA256:abc123".into()),
        }
    }

    #[tokio::test]
    async fn targets_roundtrip_and_status() {
        let pool = settings_pool().await;
        let root = Path::new("/backup-targets");
        let now = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 10, 20, 10, 0, 0).unwrap();
        let status = |pool: SqlitePool| async move {
            offsite_status(&pool, Path::new("/backup-targets"), now)
                .await
                .unwrap()
        };
        assert!(list_targets(&pool, root).await.unwrap().is_empty());
        assert_eq!(status(pool.clone()).await, "missing");
        let targets = vec![
            TargetEntry {
                id: "a".into(),
                label: "NAS".into(),
                target: BackupTarget::Directory {
                    path: "nas/kb".into(),
                },
            },
            TargetEntry {
                id: "b".into(),
                label: "Server".into(),
                target: sftp(),
            },
        ];
        save_targets(&pool, &targets).await.unwrap();
        assert_eq!(list_targets(&pool, root).await.unwrap(), targets);
        let json = get_setting(&pool, TARGETS_KEY).await.unwrap().unwrap();
        assert!(json.contains(r#""kind":"directory""#) && json.contains(r#""id":"a""#));
        assert!(!json.contains("password"));
        // W7-OFFSITE-STALE: Ziele ohne erfolgreiche Kopie sind „veraltet“.
        assert_eq!(status(pool.clone()).await, "stale");
        let r = |ok, at: &str| TargetResult {
            ok,
            at: at.into(),
            error: (!ok).then(|| "weg".into()),
        };
        let recent = "2026-10-19T02:30:00+02:00";
        store_results(
            &pool,
            &targets,
            &[
                ("a".into(), r(true, recent)),
                ("b".into(), r(false, recent)),
            ],
        )
        .await
        .unwrap();
        assert_eq!(status(pool.clone()).await, "failing");
        // Ziel b entfernt → sein Ergebnis fällt heraus
        save_targets(&pool, &targets[..1]).await.unwrap();
        store_results(&pool, &targets[..1], &[]).await.unwrap();
        assert_eq!(status(pool.clone()).await, "ok");
        // Letzte Kopie vor 7 Kalendertagen: noch ok; vor 8: veraltet.
        let at_days = |s: &str| [("a".to_string(), r(true, s))];
        store_results(&pool, &targets[..1], &at_days("2026-10-13T23:59:00+02:00"))
            .await
            .unwrap();
        assert_eq!(status(pool.clone()).await, "ok");
        store_results(&pool, &targets[..1], &at_days("2026-10-12T23:59:00+02:00"))
            .await
            .unwrap();
        assert_eq!(status(pool.clone()).await, "stale");
        // Unlesbarer Zeitpunkt: keine Aussage über das Alter.
        store_results(&pool, &targets[..1], &at_days("t"))
            .await
            .unwrap();
        assert_eq!(status(pool.clone()).await, "ok");
        assert!(
            save_targets(&pool, &vec![targets[0].clone(); MAX_TARGETS + 1])
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn legacy_keys_are_read() {
        let pool = settings_pool().await;
        let root = Path::new("/backup-targets");
        set_setting(&pool, TARGET_KEY_DIR_LEGACY, "/backup-targets/usb/kb")
            .await
            .unwrap();
        let got = list_targets(&pool, root).await.unwrap();
        assert_eq!(
            got[0].target,
            BackupTarget::Directory {
                path: "usb/kb".into()
            }
        );
        // außerhalb der Wurzel → verworfen
        set_setting(&pool, TARGET_KEY_DIR_LEGACY, "/home/x/OneDrive")
            .await
            .unwrap();
        assert!(list_targets(&pool, root).await.unwrap().is_empty());
        // JSON-Key hat Vorrang, SFTP bleibt erhalten
        set_setting(&pool, TARGET_KEY, &serde_json::to_string(&sftp()).unwrap())
            .await
            .unwrap();
        let got = list_targets(&pool, root).await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].target, sftp());
        assert_eq!(got[0].id, "sftp");
    }

    #[test]
    fn resolve_directory_stays_under_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("backup-targets");
        std::fs::create_dir_all(root.join("nas")).unwrap();
        let ok = resolve_directory(&root, "nas/klein-buch").unwrap();
        assert!(ok.is_dir() && ok.ends_with("nas/klein-buch"));
        for bad in [
            "../draussen",
            "/etc",
            "nas/../../x",
            "",
            ".",
            "C:x",
            "nas\\x",
        ] {
            assert!(resolve_directory(&root, bad).is_err(), "{bad}");
        }
        // Symlink nach draußen
        let outside = tmp.path().join("draussen");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        assert!(resolve_directory(&root, "link").is_err());
        assert!(resolve_directory(&root, "link/sub").is_err());
        assert!(!outside.join("sub").exists(), "nichts draußen angelegt");
        // fehlende Wurzel
        assert!(resolve_directory(&tmp.path().join("fehlt"), "x").is_err());
        assert_eq!(
            available_dirs(&root),
            vec!["link".to_string(), "nas".to_string()]
        );
    }

    #[tokio::test]
    async fn write_backup_directory_writes_file_and_returns_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("nested").join("backups");
        let target = BackupTarget::Directory {
            path: dir.to_string_lossy().to_string(),
        };
        let full = write_backup(&target, None, "klein-buch-test.kbk", b"hello")
            .await
            .unwrap();
        assert!(Path::new(&full).is_file());
        assert_eq!(std::fs::read(&full).unwrap(), b"hello");
        assert!(full.ends_with("klein-buch-test.kbk"));
    }

    #[test]
    fn group_key_strips_file_name() {
        assert_eq!(
            group_key("/backup-targets/nas/kb/x.kbk"),
            "/backup-targets/nas/kb"
        );
        assert_eq!(
            group_key("sftp://kb@backup.example.org:22/klein-buch/x.kbk"),
            "sftp://kb@backup.example.org:22/klein-buch"
        );
    }

    #[test]
    fn log_meta_uses_label() {
        let e = TargetEntry {
            id: "b".into(),
            label: "Server".into(),
            target: sftp(),
        };
        assert_eq!(log_meta(&e), ("sftp", Some("Server".into())));
    }

    /// U7 (R2-03): Der Ort passt genau auf den `full_path` erfolgreicher Kopien.
    #[test]
    fn log_location_is_the_path_before_the_file_name() {
        let dir = BackupTarget::Directory {
            path: "nas/kb/".into(),
        };
        assert_eq!(log_location(&dir), "/nas/kb/");
        for remote_dir in ["klein-buch", "/srv/kb/", "", "."] {
            let target = BackupTarget::Sftp {
                host: "backup.example.org".into(),
                port: 2222,
                user: "kb".into(),
                remote_dir: remote_dir.into(),
                host_fingerprint: None,
            };
            let uploaded = crate::backup::sftp::sftp_uri(
                "kb",
                "backup.example.org",
                2222,
                &crate::backup::sftp::remote_join(remote_dir, "a.kbk"),
            );
            assert_eq!(format!("{}a.kbk", log_location(&target)), uploaded);
        }
    }

    #[test]
    fn log_failed_path_builds_directory_path_and_sftp_uri() {
        let dir = BackupTarget::Directory {
            path: "/backup-targets/usb/kb".into(),
        };
        let p = log_failed_path(&dir, "klein-buch-1.kbk");
        assert!(p.ends_with("klein-buch-1.kbk"));
        let sftp = BackupTarget::Sftp {
            host: "backup.example.org".into(),
            port: 2222,
            user: "kb".into(),
            remote_dir: "/klein-buch/".into(),
            host_fingerprint: None,
        };
        assert_eq!(
            log_failed_path(&sftp, "klein-buch-1.kbk"),
            "sftp://kb@backup.example.org:2222/klein-buch/klein-buch-1.kbk"
        );
        let sftp_root = BackupTarget::Sftp {
            host: "h".into(),
            port: 22,
            user: "u".into(),
            remote_dir: "".into(),
            host_fingerprint: None,
        };
        assert_eq!(log_failed_path(&sftp_root, "f.kbk"), "sftp://u@h:22/f.kbk");
    }

    // Lauf 2026.11 U4 (Warteliste 13): Stand der Off-Site-Sicherung = älteste der
    // je Ziel jüngsten Kopien, Alter in Berliner Kalendertagen.
    #[test]
    fn oldest_copy_takes_weakest_target_in_berlin_days() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let now = crate::clock::berlin_to_utc(day, 0, 30);
        let fresh = Some("2026-06-10T00:10:00+02:00".to_string());
        // Am Vortag kurz nach Mitternacht (Berliner Zeit) → 1 Tag.
        let older = Some("2026-06-09T00:30:00+02:00".to_string());
        assert_eq!(
            oldest_copy(&[fresh.clone(), older], now),
            Some(("2026-06-09T00:30:00+02:00".to_string(), 1))
        );
        assert_eq!(
            oldest_copy(std::slice::from_ref(&fresh), now),
            Some(("2026-06-10T00:10:00+02:00".to_string(), 0))
        );
        // Ein Ziel ohne Kopie, kein Ziel oder unlesbare Zeit → keine Aussage.
        assert_eq!(oldest_copy(&[fresh.clone(), None], now), None);
        assert_eq!(oldest_copy(&[], now), None);
        assert_eq!(oldest_copy(&[Some("gestern".into())], now), None);
    }
}
