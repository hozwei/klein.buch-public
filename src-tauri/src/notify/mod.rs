//! Notify-Modul (Phase 2D, Block 15).
//! - `store`: persistente In-App-Inbox (Quelle der Wahrheit und einziger Kanal).
//! - `rules`: Reminder-Konfiguration (User-editierbar, per Migration geseedet).
//! - `deferred`: Hinweise aus dem gesperrten Zustand, beim Entsperren in die Inbox (Lauf F4b).
//!
//! [`emit`] ist der zentrale Einstieg: schreibt in die Inbox, idempotent über
//! `dedup_key`. Einen Kanal außerhalb der Inbox gibt es im Server-Betrieb nicht;
//! die OS-Benachrichtigungen der Desktop-Fassung sind mit W5 entfallen, der
//! zuletzt tote Weiterleitungs-Trait mit Lauf F (D-16, ADR 0040). Die Spalte
//! `notification_rules.deliver_os_native` bleibt (Migrationen sind forward-only),
//! wird aber nicht mehr gelesen.

pub mod deferred;
pub mod rules;
pub mod store;

pub use rules::NotificationRule;
pub use store::{NewNotification, Notification};

use crate::error::Result;
use sqlx::SqlitePool;

/// Zentrale Auslieferung in die Inbox.
///
/// Rückgabe: `Some` bei Neuanlage, `None` bei Dedup-Treffer.
pub async fn emit(pool: &SqlitePool, n: NewNotification<'_>) -> Result<Option<Notification>> {
    store::create(pool, n).await
}
