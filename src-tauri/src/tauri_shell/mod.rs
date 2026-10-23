//! Tauri-Hülle (Plan W4, entfällt in W5): alles, was `tauri::` braucht.
//!
//! - [`cmd`]: generierte dünne `#[tauri::command]`-Wrapper um die Kerne in
//!   `crate::commands`.
//! - [`special`]: Wrapper mit Desktop-Nebenwirkung (Datei öffnen, im Ordner
//!   zeigen, Neustart), deren Kern nur den Pfad bzw. das Ergebnis liefert.
//! - [`paths_from_handle`]: Desktop-Pfade (Dev: Repo, Release: App-Data).
//! - [`TauriNotifier`]: OS-native Benachrichtigung über das Notification-Plugin.

pub mod cmd;
pub mod special;

use crate::config::Paths;
use crate::notify::Notifier;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Desktop-Pfade aus dem `AppHandle` (vormals `Paths::from_handle`).
///
/// Dev-Build: alles relativ zum Repo (`klein-buch/data`, `klein-buch/inputs`,
/// Sidecar unter `src-tauri/binaries/`). Release: App-Data bzw. Bundle-Ressourcen.
pub fn paths_from_handle(app: &AppHandle) -> crate::error::Result<Paths> {
    let cfg = |e: tauri::Error| crate::error::Error::Config(format!("Tauri-Pfad: {e}"));
    let triple = crate::config::sidecar_target_triple();
    let (data_dir, inputs_dir, resource_dir, sidecar_dir) = if cfg!(debug_assertions) {
        let cwd = std::env::current_dir()?;
        let repo = cwd.join("..");
        (
            repo.join("data"),
            repo.join("inputs"),
            repo.clone(),
            cwd.join("binaries")
                .join(format!("klein-buch-java-{triple}")),
        )
    } else {
        let res = app.path().resource_dir().map_err(cfg)?;
        (
            app.path().app_data_dir().map_err(cfg)?,
            app.path().app_local_data_dir().map_err(cfg)?.join("inputs"),
            // Bundle-Layout: `resources/inputs` (tauri.conf.json bundle.resources).
            res.join("resources"),
            res.join("binaries")
                .join(format!("klein-buch-java-{triple}")),
        )
    };
    let mut paths = Paths::from_dirs(
        data_dir,
        inputs_dir,
        resource_dir.clone(),
        PathBuf::from(&resource_dir),
    );
    paths.sidecar_dir = sidecar_dir;
    Ok(paths)
}

/// OS-native Notification über `tauri-plugin-notification` (Windows-Toast,
/// macOS-Notification-Center, Linux-Freedesktop). Best-effort.
pub struct TauriNotifier(pub AppHandle);

impl Notifier for TauriNotifier {
    fn push_os(&self, title: &str, body: &str) {
        // Windows-Linker-Hinweis: im Test-Build der Lib ausgeklammert, sonst
        // scheitert das Unit-Test-Binary beim Laden (STATUS_ENTRYPOINT_NOT_FOUND).
        #[cfg(not(test))]
        {
            use tauri_plugin_notification::NotificationExt;
            match self
                .0
                .notification()
                .builder()
                .title(title)
                .body(body)
                .show()
            {
                Ok(()) => tracing::debug!("OS-Notification gesendet: {title}"),
                Err(e) => tracing::warn!("OS-Notification fehlgeschlagen: {e}"),
            }
        }
        #[cfg(test)]
        {
            let _ = (title, body);
        }
    }
}
