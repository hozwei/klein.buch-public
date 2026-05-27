//! Repository-Schicht (Imperative Shell).
//!
//! Typsichere SQL-Funktionen pro Entity. Block 2 baut `contacts`,
//! `seller_profile`, `audit_log`. Weitere folgen in Block 3+.

pub mod app_settings;
pub mod assets;
pub mod attachments;
pub mod audit_log;
pub mod backup_log;
pub mod contacts;
pub mod depreciation;
pub mod dsgvo;
pub mod email_log;
pub mod euer;
pub mod expenses;
pub mod invoices;
pub mod legal_documents;
pub mod mail_accounts;
pub mod packages;
pub mod payment_accounts;
pub mod private_movements;
pub mod quotes;
pub mod recurring;
pub mod recurring_invoice;
pub mod seller_profile;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
