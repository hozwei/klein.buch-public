//! Prozessweite Sperren je Schlüssel (Archivpfad, Beleg, Abo-Vorlage).
//!
//! Klein.Buch läuft als **ein** Prozess (Desktop bzw. ein Server-Container,
//! ADR 0015 Single-Tenant); diese Sperren serialisieren zusammengehörige
//! Abläufe innerhalb des Prozesses. Die Datenbank sichert zusätzlich über
//! Constraints, Trigger und Compare-and-Swap ab (zweite Linie).
//!
//! Die Sperren sind `tokio`-Mutexe und dürfen über `.await` gehalten werden.
//! Einträge bleiben in der Tabelle (eine Handvoll je Beleg/Vorlage/Pfad).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use tokio::sync::OwnedMutexGuard;

/// Tabelle benannter Sperren.
pub struct KeyedLocks {
    map: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl KeyedLocks {
    pub const fn new() -> Self {
        Self {
            map: LazyLock::new(Default::default),
        }
    }

    /// Wartet auf die Sperre für `key` und hält sie bis zum Drop des Guards.
    pub async fn lock(&self, key: &str) -> OwnedMutexGuard<()> {
        let entry = {
            let mut map = self.map.lock().unwrap_or_else(PoisonError::into_inner);
            map.entry(key.to_string()).or_default().clone()
        };
        entry.lock_owned().await
    }
}

impl Default for KeyedLocks {
    fn default() -> Self {
        Self::new()
    }
}

/// Archivpfade: `exists` → Hash-Vergleich → Schreiben → `archive_entries`-INSERT
/// laufen je Pfad am Stück (Plan W2.1, Review W2 S0-1).
pub static ARCHIVE_PATHS: KeyedLocks = KeyedLocks::new();
/// Belege: Festschreiben/Ausstellen einer Rechnung läuft je Rechnung am Stück.
pub static INVOICES: KeyedLocks = KeyedLocks::new();
/// Abo-Vorlagen: Tick und „Jetzt erstellen“ nie gleichzeitig (Plan W1.4).
pub static RECURRING_TEMPLATES: KeyedLocks = KeyedLocks::new();

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn same_key_is_serialized_other_keys_are_not() {
        static LOCKS: KeyedLocks = KeyedLocks::new();
        static INSIDE: AtomicUsize = AtomicUsize::new(0);
        static MAX_INSIDE: AtomicUsize = AtomicUsize::new(0);
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..8 {
            set.spawn(async {
                let _g = LOCKS.lock("a").await;
                let now = INSIDE.fetch_add(1, Ordering::SeqCst) + 1;
                MAX_INSIDE.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                INSIDE.fetch_sub(1, Ordering::SeqCst);
            });
        }
        while set.join_next().await.is_some() {}
        assert_eq!(MAX_INSIDE.load(Ordering::SeqCst), 1);

        let _a = LOCKS.lock("a").await;
        // Anderer Schlüssel ist sofort frei.
        let b = tokio::time::timeout(std::time::Duration::from_millis(200), LOCKS.lock("b")).await;
        assert!(b.is_ok());
    }
}
