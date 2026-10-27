//! App-Konfiguration: Pfade zu DB, Archive, Backups, Inputs, Sidecar, Web.
//!
//! Plan W4: Die Kerne kennen nur [`Paths`]. Der Server liest sie aus der Umgebung
//! ([`Paths::from_env`], ohne Debug-Sonderwege).
//!
//! ## `inputs/` — Zwei-Welten-Modell (R7-INPUTS)
//!
//! - **Seed-Quelle (read-only):** `resource_dir/inputs/` — im Image
//!   `KLEIN_BUCH_RESOURCE_DIR/inputs`, in der Entwicklung das Repo-`inputs/`.
//! - **User-editable:** `inputs_dir` — im Container das Volume `/inputs`. Beim
//!   Serverstart gleicht [`ensure_inputs_seeded`] mit der Quelle ab (Seed-Manifest, W11);
//!   vom Nutzer geänderte Dateien bleiben unangetastet (`inputs/`-Hardline).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Umgebungsvariablen des Servers (Plan W4, alle Pflicht).
pub const ENV_DATA_DIR: &str = "KLEIN_BUCH_DATA_DIR";
pub const ENV_INPUTS_DIR: &str = "KLEIN_BUCH_INPUTS_DIR";
pub const ENV_RESOURCE_DIR: &str = "KLEIN_BUCH_RESOURCE_DIR";
pub const ENV_WEB_DIR: &str = "KLEIN_BUCH_WEB_DIR";
/// Wurzel der Verzeichnis-Backup-Ziele (optional, Standard [`DEFAULT_BACKUP_TARGETS_DIR`]).
pub const ENV_BACKUP_TARGETS_DIR: &str = "KLEIN_BUCH_BACKUP_TARGETS_DIR";
pub const DEFAULT_BACKUP_TARGETS_DIR: &str = "/backup-targets";

#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub archive_dir: PathBuf,
    pub backups_dir: PathBuf,
    /// User-editierbares inputs-Verzeichnis (Container: Volume `/inputs`).
    pub inputs_dir: PathBuf,
    pub sidecar_dir: PathBuf,
    /// Mitgelieferte Ressourcen: `inputs/` (Seed-Quelle), `sidecar/`, später `legal/`.
    pub resource_dir: PathBuf,
    /// Gebautes Frontend (SPA), das der Server ausliefert (ab W5).
    pub web_dir: PathBuf,
    /// Wurzel der Verzeichnis-Backup-Ziele (Mounts unter `/backup-targets`,
    /// `KLEIN_BUCH_BACKUP_TARGETS_DIR`, Plan W7).
    pub backup_targets_dir: PathBuf,
}

impl Paths {
    /// Leitet alle Pfade aus den vier Grundverzeichnissen ab.
    pub fn from_dirs(
        data_dir: PathBuf,
        inputs_dir: PathBuf,
        resource_dir: PathBuf,
        web_dir: PathBuf,
    ) -> Self {
        Self {
            db_file: data_dir.join("klein-buch.sqlite"),
            archive_dir: data_dir.join("archive"),
            backups_dir: data_dir.join("backups"),
            sidecar_dir: resource_dir.join("sidecar"),
            data_dir,
            inputs_dir,
            resource_dir,
            web_dir,
            backup_targets_dir: PathBuf::from(DEFAULT_BACKUP_TARGETS_DIR),
        }
    }

    /// Server: Pfade aus `KLEIN_BUCH_DATA_DIR`, `KLEIN_BUCH_INPUTS_DIR`,
    /// `KLEIN_BUCH_RESOURCE_DIR`, `KLEIN_BUCH_WEB_DIR` (alle Pflicht, keine
    /// Debug-Sonderwege). Fehlt eine oder liegt `INPUTS_DIR` unter `DATA_DIR`,
    /// bricht der Start mit klarer Meldung ab.
    pub fn from_env() -> crate::error::Result<Self> {
        Self::from_env_with(|name| std::env::var_os(name))
    }

    /// Wie [`Paths::from_env`], mit austauschbarer Quelle (Tests).
    pub fn from_env_with(get: impl Fn(&str) -> Option<OsString>) -> crate::error::Result<Self> {
        let req = |name: &str| -> crate::error::Result<PathBuf> {
            get(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| {
                    crate::error::Error::Config(format!(
                        "Umgebungsvariable {name} fehlt — Start abgebrochen."
                    ))
                })
        };
        let data_dir = req(ENV_DATA_DIR)?;
        let inputs_dir = req(ENV_INPUTS_DIR)?;
        let resource_dir = req(ENV_RESOURCE_DIR)?;
        let web_dir = req(ENV_WEB_DIR)?;
        if inputs_dir.starts_with(&data_dir) || data_dir.starts_with(&inputs_dir) {
            return Err(crate::error::Error::Config(format!(
                "{ENV_INPUTS_DIR} ({}) und {ENV_DATA_DIR} ({}) dürfen nicht ineinander liegen — \
                 /inputs ist ein eigenes Volume.",
                inputs_dir.display(),
                data_dir.display()
            )));
        }
        let mut paths = Self::from_dirs(data_dir, inputs_dir, resource_dir, web_dir);
        if let Some(dir) = get(ENV_BACKUP_TARGETS_DIR).filter(|v| !v.is_empty()) {
            paths.backup_targets_dir = PathBuf::from(dir);
        }
        // Gegenprüfung W7: die Ziel-Wurzel darf weder Daten noch Vorlagen
        // enthalten oder in ihnen liegen (Ziel „archive“ o. Ä.).
        let t = &paths.backup_targets_dir;
        if t.parent().is_none()
            || t.starts_with(&paths.data_dir)
            || paths.data_dir.starts_with(t)
            || t.starts_with(&paths.inputs_dir)
            || paths.inputs_dir.starts_with(t)
        {
            return Err(crate::error::Error::Config(format!(
                "{ENV_BACKUP_TARGETS_DIR} ({}) darf nicht / sein und nicht mit \
                 {ENV_DATA_DIR}/{ENV_INPUTS_DIR} ineinander liegen — Start abgebrochen.",
                t.display()
            )));
        }
        Ok(paths)
    }

    /// Alles unter einem Wurzelverzeichnis (`data/`, `inputs/`, `resources/`,
    /// `web/`) — für Tests und lokale Probeläufe.
    pub fn under(root: &Path) -> Self {
        let mut paths = Self::from_dirs(
            root.join("data"),
            root.join("inputs"),
            root.join("resources"),
            root.join("web"),
        );
        paths.backup_targets_dir = root.join("backup-targets");
        paths
    }
}

/// Seed von `resource_dir/inputs` nach `inputs_dir` mit Manifest (Plan W11,
/// [`crate::seed`]): fehlende Dateien kopieren, unveränderte durch neue
/// Fassungen ersetzen, vom Nutzer geänderte nie anfassen (neue Fassung als
/// `<name>.neu`). Sind Quelle und Ziel derselbe Ordner, ist das ein No-op.
///
/// Wird in `db::prepare_filesystem` aufgerufen, vor jedem Pool-Open.
pub fn ensure_inputs_seeded(paths: &Paths) -> crate::error::Result<()> {
    let bundled = paths.resource_dir.join("inputs");
    if !bundled.exists() {
        // Seed-Quelle fehlt — beschädigte Installation bzw. falsches Image.
        // Hart als Fehler, damit nicht später ein Feature mit „Datei nicht
        // gefunden“ abstürzt.
        return Err(crate::error::Error::Config(format!(
            "Seed-Quelle für inputs/ fehlt unter {}.",
            bundled.display()
        )));
    }
    let same = match (bundled.canonicalize(), paths.inputs_dir.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same {
        return Ok(());
    }

    let report = crate::seed::seed(&bundled, &paths.inputs_dir, env!("CARGO_PKG_VERSION"))?;
    tracing::info!(
        target: "klein_buch_lib::config",
        "inputs/ aus {}: {} kopiert, {} aktualisiert, {} geändert (neue Fassung als .neu)",
        bundled.display(),
        report.copied.len(),
        report.updated.len(),
        report.kept_with_new.len()
    );
    Ok(())
}

/// Test-only: Seed ohne `Paths` (Integrations-Tests).
#[doc(hidden)]
pub fn seed_inputs_for_test(bundled: &Path, target: &Path) -> crate::error::Result<()> {
    crate::seed::seed(bundled, target, env!("CARGO_PKG_VERSION")).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: std::collections::HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).map(OsString::from)
    }

    #[test]
    fn from_env_requires_all_four_variables() {
        let ok = env(&[
            (ENV_DATA_DIR, "/data"),
            (ENV_INPUTS_DIR, "/inputs"),
            (ENV_RESOURCE_DIR, "/opt/klein-buch"),
            (ENV_WEB_DIR, "/opt/klein-buch/web"),
        ]);
        let p = Paths::from_env_with(ok).unwrap();
        assert_eq!(p.db_file, PathBuf::from("/data/klein-buch.sqlite"));
        assert_eq!(p.sidecar_dir, PathBuf::from("/opt/klein-buch/sidecar"));
        for missing in [ENV_DATA_DIR, ENV_INPUTS_DIR, ENV_RESOURCE_DIR, ENV_WEB_DIR] {
            let all = [
                (ENV_DATA_DIR, "/data"),
                (ENV_INPUTS_DIR, "/inputs"),
                (ENV_RESOURCE_DIR, "/opt/klein-buch"),
                (ENV_WEB_DIR, "/opt/klein-buch/web"),
            ];
            let some: Vec<_> = all.iter().copied().filter(|(k, _)| *k != missing).collect();
            let err = Paths::from_env_with(env(&some)).unwrap_err();
            assert!(err.to_string().contains(missing), "{err}");
        }
    }

    #[test]
    fn inputs_under_data_is_refused() {
        let get = env(&[
            (ENV_DATA_DIR, "/data"),
            (ENV_INPUTS_DIR, "/data/inputs"),
            (ENV_RESOURCE_DIR, "/opt/klein-buch"),
            (ENV_WEB_DIR, "/opt/klein-buch/web"),
        ]);
        let err = Paths::from_env_with(get).unwrap_err();
        assert!(err.to_string().contains("eigenes Volume"), "{err}");
        let reverse = env(&[
            (ENV_DATA_DIR, "/srv/kb/data"),
            (ENV_INPUTS_DIR, "/srv/kb"),
            (ENV_RESOURCE_DIR, "/opt/klein-buch"),
            (ENV_WEB_DIR, "/opt/klein-buch/web"),
        ]);
        assert!(Paths::from_env_with(reverse).is_err());
    }

    #[test]
    fn seeding_copies_missing_and_keeps_user_edits() {
        let dir = tempfile::TempDir::new().unwrap();
        let paths = Paths::under(dir.path());
        let src = paths.resource_dir.join("inputs/specs");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.json"), "neu").unwrap();
        std::fs::write(src.join("b.json"), "neu").unwrap();
        std::fs::create_dir_all(paths.inputs_dir.join("specs")).unwrap();
        std::fs::write(paths.inputs_dir.join("specs/a.json"), "vom Nutzer").unwrap();
        ensure_inputs_seeded(&paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(paths.inputs_dir.join("specs/a.json")).unwrap(),
            "vom Nutzer"
        );
        assert_eq!(
            std::fs::read_to_string(paths.inputs_dir.join("specs/b.json")).unwrap(),
            "neu"
        );
    }
}
