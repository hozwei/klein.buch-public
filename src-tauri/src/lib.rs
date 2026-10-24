//! Klein.Buch — Lokale EÜR-Buchhaltung für deutsche §19-Kleinunternehmer.
//!
//! Architektur: Functional Core (domain, einvoice::{parser,generator},
//! pdf::klausel_check, depreciation::compute, euer::aggregate) +
//! Imperative Shell (commands, db, archive, mail, scheduler, sidecar bridges,
//! backup, migration_export). Einstieg ist der HTTP-Server [`server`]
//! (Binary `klein-buch-server`, Plan W5).

pub mod app_ctx;
pub mod archive;
pub mod assets;
pub mod backup;
pub mod branding;
pub mod clock;
pub mod commands;
pub mod config;
pub mod db;
pub mod depreciation;
pub mod domain;
pub mod downloads;
pub mod einvoice;
pub mod error;
pub mod euer;
pub mod fiscal_year;
pub mod limits;
pub mod locks;
pub mod mail;
pub mod migration_export;
pub mod nettest;
pub mod notify;
pub mod pdf;
pub mod scheduler;
pub mod secrets;
pub mod seed;
pub mod server;
pub mod uploads;
