//! Desktop-Nebenwirkungen der Tauri-Hülle (Plan W4): Umsetzung von
//! [`crate::app_ctx::DesktopActions`] über `tauri-plugin-opener` und
//! `AppHandle::restart`.

use crate::app_ctx::DesktopActions;
use crate::error::{Error, Result};
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

pub struct TauriDesktop(pub AppHandle);

impl DesktopActions for TauriDesktop {
    fn open_path(&self, path: &Path) -> Result<()> {
        self.0
            .opener()
            .open_path(path.to_string_lossy().to_string(), None::<&str>)
            .map_err(|e| {
                Error::Other(anyhow::anyhow!(
                    "Öffnen fehlgeschlagen ({}): {e}",
                    path.display()
                ))
            })
    }

    fn reveal(&self, path: &Path) -> Result<()> {
        self.0
            .opener()
            .reveal_item_in_dir(path)
            .map_err(|e| Error::Other(anyhow::anyhow!("Ordner konnte nicht geöffnet werden: {e}")))
    }

    fn open_url(&self, url: &str) -> Result<()> {
        self.0
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| Error::Mail(format!("Browser konnte nicht geöffnet werden: {e}")))
    }

    fn restart(&self) -> Result<()> {
        self.0.restart()
    }
}
