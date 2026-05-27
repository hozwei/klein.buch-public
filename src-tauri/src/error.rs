//! Zentrale Error-Typen für Klein.Buch.
//!
//! `Error` ist `thiserror`-basiert und wird via `serde` ans Frontend serialisiert.
//! Schema-Mismatch ist eine eigene Variante, damit Frontend und Bootstrap-Logik
//! gleich darauf reagieren können.

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Datenbank: {0}")]
    Db(#[from] sqlx::Error),

    #[error("Datenbank-Migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("Schema-Version-Mismatch: erwartet {expected}, gefunden {found}. {hint}")]
    SchemaMismatch {
        expected: i32,
        found: i32,
        hint: String,
    },

    #[error("IO: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Konfiguration: {0}")]
    Config(String),

    #[error("Domain: {0}")]
    Domain(String),

    #[error("Sidecar: {0}")]
    Sidecar(String),

    #[error("Mail: {0}")]
    Mail(String),

    #[error("Backup: {0}")]
    Backup(String),

    #[error("Verschlüsselung: {0}")]
    Crypto(String),

    #[error("ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

// Tauri-Command-Bridge: serialisiere Error als String fürs Frontend.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
