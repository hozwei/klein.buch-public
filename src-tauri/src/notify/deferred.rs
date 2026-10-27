//! Hinweise aus der Zeit, in der die Datenbank gesperrt ist (Lauf F4b, R3-014).
//!
//! Beim Serverstart ist die Instanz gesperrt (E3), die Inbox also nicht
//! beschreibbar. Prüfungen beim Start (z. B. der Sidecar-Health-Check) legen
//! ihre Hinweise deshalb hier im Speicher ab; das nächste Entsperren
//! (`commands::backup::activate_unlocked`) bzw. die Einrichtung übernimmt sie in
//! die Inbox. Jeder Hinweis trägt einen `dedup_key` — ein erneutes Entsperren
//! oder ein abgebrochener Durchlauf legt keinen Hinweis doppelt an. Nach einem
//! Neustart prüft der Server neu.

use std::sync::{Mutex, PoisonError};

use sqlx::SqlitePool;

use crate::error::Result;

/// Ein zurückgestellter Hinweis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredNotice {
    pub title: String,
    pub body: String,
    /// `info` | `warning` | `urgent`
    pub severity: &'static str,
    pub action_url: Option<&'static str>,
    pub dedup_key: String,
}

/// Ablage im `AppCtx`.
#[derive(Debug, Default)]
pub struct DeferredNotices(Mutex<Vec<DeferredNotice>>);

impl DeferredNotices {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<DeferredNotice>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Legt einen Hinweis ab; einer mit demselben `dedup_key` wird ersetzt.
    pub fn push(&self, notice: DeferredNotice) {
        let mut list = self.lock();
        list.retain(|n| n.dedup_key != notice.dedup_key);
        list.push(notice);
    }

    /// Abgelegte Hinweise (Tests, Diagnose).
    pub fn pending(&self) -> Vec<DeferredNotice> {
        self.lock().clone()
    }

    /// Übernimmt alle Hinweise in die Inbox. Was nicht geschrieben werden
    /// konnte, bleibt für das nächste Entsperren liegen.
    pub async fn deliver(&self, pool: &SqlitePool) -> Result<usize> {
        let notices = std::mem::take(&mut *self.lock());
        let mut delivered = 0;
        for (idx, n) in notices.iter().enumerate() {
            let created = crate::notify::store::create(
                pool,
                crate::notify::store::NewNotification {
                    title: &n.title,
                    body: &n.body,
                    severity: n.severity,
                    action_url: n.action_url,
                    dedup_key: Some(&n.dedup_key),
                    ..Default::default()
                },
            )
            .await;
            if let Err(e) = created {
                let mut list = self.lock();
                for rest in notices.into_iter().skip(idx) {
                    if !list.iter().any(|x| x.dedup_key == rest.dedup_key) {
                        list.push(rest);
                    }
                }
                return Err(e);
            }
            delivered += 1;
        }
        Ok(delivered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(key: &str, title: &str) -> DeferredNotice {
        DeferredNotice {
            title: title.into(),
            body: "Text".into(),
            severity: "warning",
            action_url: None,
            dedup_key: key.into(),
        }
    }

    #[test]
    fn push_replaces_same_key() {
        let store = DeferredNotices::default();
        store.push(notice("a", "alt"));
        store.push(notice("b", "b"));
        store.push(notice("a", "neu"));
        let pending = store.pending();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[1].title, "neu");
    }
}
