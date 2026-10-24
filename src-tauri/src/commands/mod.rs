//! Command-Kerne (Plan W4) — Brücke zwischen Transport (HTTP-Dispatcher
//! `server::dispatch`, W5) und Domain/DB-Schicht. Jeder Kern ist
//! `async fn name(ctx: &Arc<AppCtx>, args: NameArgs) -> Result<T>`; die
//! Args-Structs übernehmen die camelCase-Schlüssel der bisherigen `invoke`-
//! Aufrufe. [`registry`] listet alle Kerne mit ihren Metadaten.

pub mod attachments;
pub mod backup;
pub mod contacts;
pub mod depreciation;
pub mod dsgvo;
pub mod euer;
pub mod expenses;
pub mod fiscal_year;
pub mod invoices;
pub mod legal_documents;
pub mod mail;
pub mod migration_export;
pub mod notifications;
pub mod packages;
pub mod payment_accounts;
pub mod pdf;
pub mod private_movements;
pub mod quotes;
pub mod recurring;
pub mod recurring_invoice;
pub mod registry;
pub mod settings;
pub mod system;

// Phase 2C
pub mod assets;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
