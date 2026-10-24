//! Notify-Modul (Phase 2D, Block 15).
//! - `store`: persistente In-App-Inbox (Quelle der Wahrheit).
//! - [`Notifier`]: Kanal außerhalb der Inbox. Die Desktop-Fassung (Tauri-
//!   Plugin) ist mit W5 entfallen; im Server gibt es keinen ([`InboxOnly`]).
//! - `rules`: Reminder-Konfiguration (User-editierbar, per Migration geseedet).
//!
//! [`emit`] ist der zentrale Einstieg: schreibt in die Inbox (idempotent via
//! `dedup_key`) und reicht den Eintrag — falls neu und der Kanal der Regel
//! aktiv ist — an den [`Notifier`] weiter; im Server ist das [`InboxOnly`],
//! es bleibt also bei der Inbox.

pub mod rules;
pub mod store;

pub use rules::NotificationRule;
pub use store::{NewNotification, Notification};

use crate::error::Result;
use sqlx::SqlitePool;

/// Kanal für OS-native Benachrichtigungen (Desktop-Toast). Best-effort: Fehler
/// werden in der Implementierung geloggt, nie propagiert — die Inbox bleibt die
/// Quelle der Wahrheit.
pub trait Notifier: Send + Sync {
    fn push_os(&self, title: &str, body: &str);
}

/// Kein OS-Kanal (Server-Betrieb, Tests): nur die Inbox.
#[derive(Debug, Default, Clone, Copy)]
pub struct InboxOnly;

impl Notifier for InboxOnly {
    fn push_os(&self, _title: &str, _body: &str) {}
}

/// Zentrale Auslieferung. `notifier = None` (Tests, reine DB-Pfade) unterdrückt
/// nur den OS-Push; die Inbox wird immer geschrieben.
///
/// Rückgabe: `Some` bei Neuanlage, `None` bei Dedup-Treffer (kein OS-Push).
pub async fn emit(
    pool: &SqlitePool,
    notifier: Option<&dyn Notifier>,
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
    if let (Some(notif), true, Some(notifier)) = (created.as_ref(), os_native, notifier) {
        notifier.push_os(&notif.title, &notif.body);
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
