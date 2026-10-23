//! Zentrale Error-Typen für Klein.Buch.
//!
//! `Error` ist `thiserror`-basiert. Der HTTP-Server bildet jede Variante vor dem
//! Serialisieren auf Status und Antwort ab (`server::errors`, Plan W5.3); interne
//! Texte gehen nur ins Log. Schema-Mismatch ist eine eigene Variante, damit
//! Frontend und Bootstrap-Logik gleich darauf reagieren können.

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

    /// Die Instanz ist gesperrt (kein offener DB-Pool) — erst entsperren (W4/W5: HTTP 423).
    #[error("Datenbank gesperrt — bitte zuerst entsperren.")]
    Locked,

    /// Argumente eines Commands passen nicht zum Args-Struct (HTTP 400).
    #[error("Ungültige Anfrage: {0}")]
    BadRequest(String),

    #[error("Verschlüsselung: {0}")]
    Crypto(String),

    #[error("ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
