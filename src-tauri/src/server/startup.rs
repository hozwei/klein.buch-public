//! Prüfungen vor dem Port-Bind (fail-closed, Plan W5.5).
//!
//! Reihenfolge:
//! 0. exklusive Sperre `/data/.klein-buch.lock` (ein Prozess je Instanz;
//!    die folgenden Schritte verändern das Datenverzeichnis),
//! 1. `/data` und `/inputs` vorhanden und beschreibbar,
//! 2. laut `/proc/self/mountinfo` Mountpoints (abschaltbar nur per
//!    `KLEIN_BUCH_ALLOW_UNMOUNTED=1` für lokale Tests),
//! 3. `db::prepare_filesystem` synchron (vorgemerkter Restore, Seeding von
//!    `/inputs`, Verzeichnisse),
//! 4. DB und Instanz-Marker passen zusammen; ohne beide muss `archive/` leer
//!    sein,
//! 5. keine unverschlüsselte DB,
//! 6. Selbsttest in `archive/`: anlegen, `chmod 0400`, prüfen, löschen
//!    (scheitert z. B. bei NFSv4-ACLs oder als root),
//! 7. `<data>/tmp` leeren (Reste abgebrochener Läufe).
//!
//! Die Stichprobe `archive_entries.file_path` unter dem aktuellen
//! `archive_dir` braucht die entschlüsselte DB und läuft deshalb beim
//! Entsperren ([`check_archive_paths`]).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use super::config::StartupOptions;
use super::instance;
use crate::app_ctx::AppCtx;
use crate::config::Paths;
use crate::error::{Error, Result};

fn fail(msg: impl Into<String>) -> Error {
    Error::Config(msg.into())
}

/// Name der Sperrdatei im Datenverzeichnis.
pub const LOCK_FILE: &str = ".klein-buch.lock";

/// Alle Prüfungen vor dem Port-Bind; ein Fehler bricht den Start ab. Liefert
/// die gehaltene Instanz-Sperre (für die Laufzeit des Prozesses behalten).
pub fn run_checks(ctx: &AppCtx, opts: &StartupOptions) -> Result<std::fs::File> {
    let paths = &ctx.paths;
    for (label, dir) in [("Daten", &paths.data_dir), ("Inputs", &paths.inputs_dir)] {
        if !dir.is_dir() {
            if opts.allow_unmounted {
                std::fs::create_dir_all(dir)?;
            } else {
                return Err(fail(format!(
                    "{label}-Verzeichnis {} fehlt — Volume nicht eingebunden?",
                    dir.display()
                )));
            }
        }
        check_writable(label, dir)?;
    }
    let lock = acquire_lock(&paths.data_dir)?;
    if !opts.allow_unmounted {
        let mountinfo = match &opts.mountinfo {
            Some(text) => text.clone(),
            None => std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| {
                fail(format!(
                    "/proc/self/mountinfo nicht lesbar ({e}) — Start abgebrochen."
                ))
            })?,
        };
        let mounts = mount_points(&mountinfo);
        for (label, dir) in [("Daten", &paths.data_dir), ("Inputs", &paths.inputs_dir)] {
            if !is_mount_point(dir, &mounts) {
                return Err(fail(format!(
                    "{label}-Verzeichnis {} ist kein eingebundenes Volume — ohne Volume gingen \
                     die Daten mit dem Container verloren. Start abgebrochen.",
                    dir.display()
                )));
            }
        }
    }

    crate::db::prepare_filesystem(ctx)?;

    check_instance(paths)?;

    if paths.db_file.exists() && crate::db::db_file_is_plaintext(&paths.db_file) {
        return Err(fail(format!(
            "{} ist unverschlüsselt — der Server öffnet nur SQLCipher-Datenbanken (ADR 0035/0039). \
             Start abgebrochen.",
            paths.db_file.display()
        )));
    }

    archive_selftest(&paths.archive_dir)?;
    clean_tmp(&paths.data_dir)?;
    Ok(lock)
}

/// Exklusive Sperre auf `<data>/.klein-buch.lock`. Läuft schon ein Prozess auf
/// diesem Datenverzeichnis, bricht der Start ab — sonst würde er unter der
/// laufenden Instanz einen Restore tauschen oder `tmp/` leeren.
fn acquire_lock(data_dir: &Path) -> Result<std::fs::File> {
    let path = data_dir.join(LOCK_FILE);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(fail(format!(
            "Auf {} läuft bereits ein Klein.Buch-Prozess ({} gesperrt). Start abgebrochen.",
            data_dir.display(),
            LOCK_FILE
        ))),
        Err(std::fs::TryLockError::Error(e)) => Err(fail(format!(
            "Sperre {} nicht möglich ({e}). Start abgebrochen.",
            path.display()
        ))),
    }
}

/// DB und Marker passen zusammen.
fn check_instance(paths: &Paths) -> Result<()> {
    let db = paths.db_file.exists();
    let marker = instance::marker_path(&paths.data_dir).exists();
    match (db, marker) {
        (true, true) => Ok(()),
        (true, false) => Err(fail(format!(
            "Datenbank {} ohne Instanz-Marker {} — fremdes Datenverzeichnis oder abgebrochene \
             Einrichtung. Start abgebrochen; Datenverzeichnis prüfen.",
            paths.db_file.display(),
            instance::MARKER_FILE
        ))),
        (false, true) => Err(fail(format!(
            "Instanz-Marker {} ohne Datenbank — Datenverzeichnis unvollständig. Start \
             abgebrochen; Backup einspielen oder Datenverzeichnis prüfen.",
            instance::MARKER_FILE
        ))),
        (false, false) => {
            let archive_used = match std::fs::read_dir(&paths.archive_dir) {
                Ok(mut it) => it.next().is_some(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => {
                    return Err(fail(format!(
                        "Archiv {} nicht lesbar ({e}). Start abgebrochen.",
                        paths.archive_dir.display()
                    )))
                }
            };
            if archive_used {
                return Err(fail(format!(
                    "Archiv {} enthält Dateien, aber es gibt weder Datenbank noch Instanz-Marker. \
                     Start abgebrochen (keine Neueinrichtung über einem Archiv).",
                    paths.archive_dir.display()
                )));
            }
            Ok(())
        }
    }
}

fn check_writable(label: &str, dir: &Path) -> Result<()> {
    let probe = dir.join(format!(
        ".klein-buch-probe-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let result = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|mut f| f.write_all(b"probe"));
    let _ = std::fs::remove_file(&probe);
    result.map_err(|e| {
        fail(format!(
            "{label}-Verzeichnis {} ist nicht beschreibbar ({e}). Start abgebrochen.",
            dir.display()
        ))
    })
}

/// Selbsttest des Write-once-Schutzes im Archiv (R6).
fn archive_selftest(archive_dir: &Path) -> Result<()> {
    archive_selftest_with(archive_dir, set_readonly)
}

/// Wie [`archive_selftest`] mit austauschbarem `chmod` (Tests simulieren damit
/// Dateisysteme, auf denen der Schreibschutz nicht greift).
pub fn archive_selftest_with(
    archive_dir: &Path,
    set_readonly: fn(&Path) -> std::io::Result<()>,
) -> Result<()> {
    let probe = archive_dir.join(format!(".selftest-{}", uuid::Uuid::new_v4().simple()));
    let outcome = selftest_inner(&probe, set_readonly);
    let _ = std::fs::remove_file(&probe);
    outcome.map_err(|reason| {
        fail(format!(
            "Selbsttest im Archiv {} gescheitert: {reason}. Festgeschriebene Belege wären nicht \
             schreibgeschützt (GoBD). Start abgebrochen.",
            archive_dir.display()
        ))
    })
}

fn selftest_inner(
    probe: &Path,
    set_readonly: fn(&Path) -> std::io::Result<()>,
) -> std::result::Result<(), String> {
    use std::fs::OpenOptions;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(probe)
        .map_err(|e| format!("Anlegen nicht möglich ({e})"))?;
    f.write_all(b"klein-buch write-once selftest")
        .map_err(|e| format!("Schreiben nicht möglich ({e})"))?;
    drop(f);
    set_readonly(probe).map_err(|e| format!("Schreibschutz nicht setzbar ({e})"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(probe)
            .map_err(|e| format!("Rechte nicht lesbar ({e})"))?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o400 {
            return Err(format!("Rechte sind {mode:o} statt 400 (ACL?)"));
        }
    }
    if OpenOptions::new().write(true).open(probe).is_ok() {
        return Err(
            "schreibgeschützte Datei bleibt beschreibbar (läuft der Server als root?)".into(),
        );
    }
    Ok(())
}

fn set_readonly(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o400))
    }
    #[cfg(not(unix))]
    {
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(path, perms)
    }
}

/// Leert `<data>/tmp` (Snapshots, Exporte, Uploads abgebrochener Läufe).
fn clean_tmp(data_dir: &Path) -> Result<()> {
    let tmp = data_dir.join("tmp");
    let Ok(entries) = std::fs::read_dir(&tmp) else {
        return Ok(());
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let res = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match res {
            Ok(()) => removed += 1,
            Err(e) => tracing::warn!("tmp-Rest {} nicht löschbar: {e}", path.display()),
        }
    }
    if removed > 0 {
        tracing::info!("{removed} Rest(e) in {} entfernt", tmp.display());
    }
    Ok(())
}

/// Mountpoints aus `/proc/self/mountinfo` (5. Feld, Oktal-Escapes wie `\040`).
pub fn mount_points(mountinfo: &str) -> Vec<PathBuf> {
    mountinfo
        .lines()
        .filter_map(|line| line.split(' ').nth(4))
        .map(|field| PathBuf::from(unescape_octal(field)))
        .collect()
}

fn unescape_octal(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            let oct = &bytes[i + 1..i + 4];
            if oct.iter().all(|b| (b'0'..=b'7').contains(b)) {
                let v = oct
                    .iter()
                    .fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
                if let Ok(v) = u8::try_from(v) {
                    out.push(v);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Ist `dir` (kanonisch) genau ein Mountpoint?
pub fn is_mount_point(dir: &Path, mounts: &[PathBuf]) -> bool {
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    mounts.iter().any(|m| *m == canonical || m == dir)
}

/// Stichprobe beim Entsperren (Plan W5.5): die ersten und letzten Einträge von
/// `archive_entries.file_path` müssen unter dem aktuellen `archive_dir` liegen.
/// Sonst passt das Volume nicht zur DB (z. B. anderer Mount-Pfad).
pub async fn check_archive_paths(pool: &SqlitePool, paths: &Paths) -> Result<()> {
    let sample: Vec<String> = sqlx::query_scalar(
        "SELECT file_path FROM (SELECT file_path FROM archive_entries ORDER BY rowid ASC LIMIT 3)
         UNION ALL
         SELECT file_path FROM (SELECT file_path FROM archive_entries ORDER BY rowid DESC LIMIT 3)",
    )
    .fetch_all(pool)
    .await?;
    let root = &paths.archive_dir;
    let canonical = root.canonicalize().ok();
    for fp in sample {
        let p = Path::new(&fp);
        let inside = p.starts_with(root) || canonical.as_ref().is_some_and(|c| p.starts_with(c));
        if !inside {
            return Err(fail(format!(
                "Archiv-Eintrag {fp} liegt nicht unter {} — Datenverzeichnis passt nicht zur \
                 Datenbank. Entsperren verweigert.",
                root.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mountinfo_with_escapes() {
        let text = "36 35 98:0 /mnt1 /data rw,noatime master:1 - ext3 /dev/root rw\n\
                    37 35 98:0 / /mit\\040leer rw - ext4 /dev/sda1 rw\n";
        let mounts = mount_points(text);
        assert_eq!(
            mounts,
            vec![PathBuf::from("/data"), PathBuf::from("/mit leer")]
        );
    }
}
