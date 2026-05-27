//! Notify-Modul (Phase 2D, Block 15).
//! - `store`: persistente In-App-Inbox (Quelle der Wahrheit).
//! - `os_native`: Tauri-Notification-Plugin-Bridge (Windows-Toast, macOS-NC).
//! - `rules`: Reminder-Konfiguration (User-editierbar, per Migration geseedet).
//!
//! [`emit`] ist der zentrale Einstieg: schreibt in die Inbox (idempotent via
//! `dedup_key`) und schickt — falls neu und der Kanal der Regel aktiv ist —
//! zusätzlich eine OS-native Notification.

pub mod os_native;
pub mod rules;
pub mod store;

pub use rules::NotificationRule;
pub use store::{NewNotification, Notification};

use crate::error::Result;
use sqlx::SqlitePool;
use tauri::AppHandle;

/// Zentrale Auslieferung. `app = None` (Tests, reine DB-Pfade) unterdrückt nur
/// den OS-Push; die Inbox wird immer geschrieben.
///
/// Rückgabe: `Some` bei Neuanlage, `None` bei Dedup-Treffer (kein OS-Push).
pub async fn emit(
    pool: &SqlitePool,
    app: Option<&AppHandle>,
    n: NewNotification<'_>,
) -> Result<Option<Notification>> {
    // OS-Kanal der zugehörigen Regel auflösen (kein rule_id ⇒ Default an).
    let os_native = match n.rule_id {
        Some(rid) => rules::get(pool, rid)
            .await?
            .map(|r| r.deliver_os_native != 0)
            .unwrap_or(true),
        None => true,
    };
    let created = store::create(pool, n).await?;
    if let (Some(notif), true, Some(app)) = (created.as_ref(), os_native, app) {
        os_native::push(app, &notif.title, &notif.body);
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
