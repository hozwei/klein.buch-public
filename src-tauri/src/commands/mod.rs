//! Tauri-Commands — Brücke zwischen Frontend und Domain/DB-Schicht.
//! Jeder Command ist async, nimmt typsichere Args, ruft Domain + DB,
//! gibt `Result<T, Error>` zurück.

pub mod attachments;
pub mod backup;
pub mod contacts;
pub mod depreciation;
pub mod dsgvo;
pub mod euer;
pub mod expenses;
pub mod factory_reset;
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
pub mod settings;
pub mod system;

// Phase 2C
pub mod assets;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
