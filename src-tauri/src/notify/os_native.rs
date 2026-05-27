//! OS-native Notification-Bridge (Block 15) über `tauri-plugin-notification`.
//!
//! Windows-Toast / macOS-Notification-Center / Linux-Freedesktop. Best-effort:
//! fehlende Berechtigung oder ein Versandfehler werden nur geloggt — die
//! In-App-Inbox ([`crate::notify::store`]) bleibt die Quelle der Wahrheit.

use tauri::AppHandle;

/// Schickt eine OS-native Notification. Schlägt der Versand fehl (z. B. keine
/// Berechtigung), wird das nur protokolliert und kein Fehler propagiert.
///
/// **Windows-Linker-Hinweis:** Der `show()`-Pfad zieht den WinRT-Toast-Code in
/// jedes Binary, das diese Funktion linkt. Im **Test-Build** (`cfg(test)` der Lib)
/// wird er ausgeklammert — sonst scheitert das Lib-Unit-Test-Binary unter Windows
/// schon beim Laden (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139). In Produktion
/// (`tauri dev`/`build`) ist `cfg(test)` aus → echte Notification.
pub fn push(app: &AppHandle, title: &str, body: &str) {
    #[cfg(not(test))]
    {
        use tauri_plugin_notification::NotificationExt;
        match app.notification().builder().title(title).body(body).show() {
            Ok(()) => tracing::debug!("OS-Notification gesendet: {title}"),
            Err(e) => tracing::warn!("OS-Notification fehlgeschlagen: {e}"),
        }
    }
    #[cfg(test)]
    {
        let _ = (app, title, body);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
