//! Mail-Schicht.
//!
//! - `smtp` (Block 5): lettre + STARTTLS, Test-Connection.
//! - `oauth_ms` (Block 16): Microsoft Graph OAuth-PKCE-Flow für Exchange Online.
//! - Zugangsdaten liegen seit W8 in der Datenbank ([`crate::secrets`]).
//! - `templates`: Tera-Template-Render für Subject + Body.

pub mod oauth_ms;
pub mod smtp;
pub mod templates;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
