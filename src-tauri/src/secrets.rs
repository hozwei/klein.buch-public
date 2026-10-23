//! Ablage für Zugangsdaten (SMTP-/SFTP-Passwörter), Plan W4.
//!
//! Bis W8 liegen Passwörter im OS-Keychain (`mail::keyring`, `backup::sftp`);
//! W8 ersetzt das durch eine STRICT-Tabelle in der SQLCipher-DB (Amendment
//! ADR 0011/0028). Der Trait trennt die Kerne von der Ablage.

use crate::error::Result;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// OS-Keychain über das bestehende `mail::keyring`-Modul (Desktop bis W8).
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        crate::mail::keyring::get_password(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        crate::mail::keyring::set_password(key, value)
    }
    fn delete(&self, key: &str) -> Result<()> {
        crate::mail::keyring::delete_password(key)
    }
}

/// Nur im Arbeitsspeicher (Tests).
#[derive(Debug, Default)]
pub struct MemorySecretStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip() {
        let s = MemorySecretStore::default();
        assert_eq!(s.get("a").unwrap(), None);
        s.set("a", "geheim").unwrap();
        assert_eq!(s.get("a").unwrap().as_deref(), Some("geheim"));
        s.delete("a").unwrap();
        assert_eq!(s.get("a").unwrap(), None);
    }
}
