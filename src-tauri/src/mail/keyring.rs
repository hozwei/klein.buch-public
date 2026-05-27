//! Bridge zur `keyring`-Crate für SMTP-Passwörter (Block 5) und später
//! OAuth-Refresh-Tokens (Block 16).
//!
//! Schicht: **Imperative Shell** — alle Credential-I/O lebt hier.
//!
//! ## Hard-Rules (Backup-Hardline)
//!
//! - **Passwort niemals** in der DB, in Logs oder im `audit_log`. Sie lebt
//!   ausschließlich im OS-Credential-Manager (Windows Credential Manager,
//!   macOS Keychain, Linux Secret Service).
//! - Service-ID-Schema: `kleinbuch::mail::{account_id}` (siehe
//!   [`service_id`]). Pro Mail-Account ein eigener Keychain-Eintrag, damit
//!   das Löschen eines Accounts seine Credentials gezielt mit-entfernt.
//! - keyring 3 hat **keine** Default-Backends — die nötigen Feature-Flags
//!   (`windows-native`/`apple-native`/`sync-secret-service`) sind in
//!   `Cargo.toml` gesetzt. Ohne sie liefe alles in einen flüchtigen
//!   Mock-Store.

use crate::error::{Error, Result};

/// Der `user`-Teil des Keychain-Eintrags für die SMTP-Passphrase. Konstant — wir
/// identifizieren den Eintrag über die Service-ID (die den Account enthält); der
/// User ist nur ein zweiter, fixer Schlüsselbestandteil.
const KEYCHAIN_USER: &str = "smtp";

/// Der `user`-Teil für das OAuth-Token-Bundle (Block 16). Eigener Slot neben der
/// SMTP-Passphrase unter derselben Service-ID, damit ein Account beide Auth-Arten
/// sauber getrennt halten und beim Löschen gezielt entfernen kann.
const KEYCHAIN_OAUTH_USER: &str = "oauth";

/// Baut die Service-ID für einen Mail-Account: `kleinbuch::mail::{account_id}`.
///
/// Pure Funktion — wird auch beim Anlegen eines Accounts genutzt, um den Wert
/// für `mail_accounts.keychain_service_id` zu erzeugen.
pub fn service_id(account_id: &str) -> String {
    format!("kleinbuch::mail::{account_id}")
}

/// Speichert die SMTP-Passwort im OS-Credential-Manager.
///
/// `service` ist die [`service_id`] des Accounts. Wirft [`Error::Mail`] bei
/// Store-Fehlern. Die Passwort wird **nirgends sonst** persistiert.
pub fn set_password(service: &str, password: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, KEYCHAIN_USER)
        .map_err(|e| Error::Mail(format!("Keychain-Eintrag nicht erstellbar: {e}")))?;
    entry
        .set_password(password)
        .map_err(|e| Error::Mail(format!("Passwort nicht speicherbar: {e}")))?;
    Ok(())
}

/// Liest die SMTP-Passwort. `Ok(None)`, wenn kein Eintrag existiert
/// (z. B. ein Open-Relay ohne Auth wie MailHog) — alle anderen Store-Fehler
/// werden propagiert, damit ein defekter Keychain nicht als „kein Passwort"
/// fehlinterpretiert wird.
pub fn get_password(service: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(service, KEYCHAIN_USER)
        .map_err(|e| Error::Mail(format!("Keychain-Eintrag nicht lesbar: {e}")))?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Mail(format!("Passwort nicht lesbar: {e}"))),
    }
}

/// Löscht die Passwort aus dem Credential-Manager. Idempotent: ein bereits
/// fehlender Eintrag ist kein Fehler (für Account-Löschung relevant).
pub fn delete_password(service: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, KEYCHAIN_USER)
        .map_err(|e| Error::Mail(format!("Keychain-Eintrag nicht adressierbar: {e}")))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Mail(format!("Passwort nicht löschbar: {e}"))),
    }
}

// =============================================================================
// OAuth-Refresh-Token (Block 16)
// =============================================================================
//
// Hier liegt ausschließlich der (langlebige) Refresh-Token — niemals in
// DB/Log/audit. Der Access-Token (großes JWT) wird bei jedem Versand frisch
// geholt und NICHT persistiert.
//
// Der Windows Credential Manager begrenzt EINEN Eintrag auf 2560 UTF-16-Zeichen;
// MS-Refresh-Token (2–4 KB) sprengen das schon allein. Daher wird der Token in
// Chunks ≤ 1024 Zeichen zerlegt und auf mehrere Einträge verteilt (eigene
// Service-ID je Chunk) + ein Header-Eintrag unter `(service, KEYCHAIN_OAUTH_USER)`
// mit der Chunk-Anzahl. Beim Lesen wird wieder zusammengesetzt.

/// Max. Zeichen pro Schlüsselbund-Eintrag. Bewusst weit unter dem Windows-
/// Credential-Manager-Limit (2560 UTF-16-Zeichen) — 1024 Zeichen ≈ 2048 Byte.
const OAUTH_CHUNK_CHARS: usize = 1024;
/// Obergrenze für das Aufräumen alter Chunks (32 × 1024 = 32k Zeichen Token).
const OAUTH_MAX_CHUNKS: usize = 32;

/// Service-ID eines Token-Chunks. Eigene Service-ID je Chunk ⇒ garantiert
/// eigener Eintrag im OS-Store (unabhängig davon, wie das Backend (service,user)
/// auf einen Ziel-Schlüssel abbildet).
fn chunk_service(service: &str, i: usize) -> String {
    format!("{service}::oauth.{i}")
}

fn put_secret(service: &str, user: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| Error::Mail(format!("Keychain-Eintrag (OAuth) nicht erstellbar: {e}")))?;
    entry
        .set_password(value)
        .map_err(|e| Error::Mail(format!("OAuth-Token nicht speicherbar: {e}")))?;
    Ok(())
}

fn fetch_secret(service: &str, user: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| Error::Mail(format!("Keychain-Eintrag (OAuth) nicht lesbar: {e}")))?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Mail(format!("OAuth-Token nicht lesbar: {e}"))),
    }
}

fn drop_secret(service: &str, user: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| Error::Mail(format!("Keychain-Eintrag (OAuth) nicht adressierbar: {e}")))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Mail(format!("OAuth-Token nicht löschbar: {e}"))),
    }
}

/// Zerlegt einen String in Chunks von höchstens `max` Zeichen (pure).
fn split_into_chunks(s: &str, max: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(max).map(|c| c.iter().collect()).collect()
}

/// Speichert den OAuth-Refresh-Token im OS-Credential-Manager. Da MS-Refresh-Token
/// das Windows-Eintrags-Limit überschreiten, wird der Token in Chunks zerlegt:
/// pro Chunk ein eigener Eintrag, plus ein Header-Eintrag mit der Chunk-Anzahl.
pub fn set_oauth_tokens(service: &str, refresh_token: &str) -> Result<()> {
    // Alte Chunks/Header zuerst entfernen (best-effort).
    delete_oauth_tokens(service)?;
    let chunks = split_into_chunks(refresh_token, OAUTH_CHUNK_CHARS);
    for (i, chunk) in chunks.iter().enumerate() {
        put_secret(&chunk_service(service, i), KEYCHAIN_OAUTH_USER, chunk)?;
    }
    // Header (Chunk-Anzahl) ZULETZT — markiert die Token-Ablage als vollständig.
    put_secret(service, KEYCHAIN_OAUTH_USER, &chunks.len().to_string())?;
    Ok(())
}

/// Liest den OAuth-Refresh-Token (aus Header + Chunks zusammengesetzt).
/// `Ok(None)`, wenn der Account noch nicht verbunden ist.
pub fn get_oauth_tokens(service: &str) -> Result<Option<String>> {
    let header = match fetch_secret(service, KEYCHAIN_OAUTH_USER)? {
        Some(h) => h,
        None => return Ok(None),
    };
    let n: usize = header
        .trim()
        .parse()
        .map_err(|_| Error::Mail("OAuth-Eintrag beschädigt — bitte neu verbinden.".into()))?;
    let mut out = String::new();
    for i in 0..n {
        match fetch_secret(&chunk_service(service, i), KEYCHAIN_OAUTH_USER)? {
            Some(c) => out.push_str(&c),
            None => {
                return Err(Error::Mail(
                    "OAuth-Token im Schlüsselbund unvollständig — bitte neu verbinden.".into(),
                ))
            }
        }
    }
    Ok(Some(out))
}

/// Löscht Header + alle Token-Chunks. Idempotent (fehlende Einträge = Ok).
pub fn delete_oauth_tokens(service: &str) -> Result<()> {
    drop_secret(service, KEYCHAIN_OAUTH_USER)?;
    for i in 0..OAUTH_MAX_CHUNKS {
        drop_secret(&chunk_service(service, i), KEYCHAIN_OAUTH_USER)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static MOCK_INIT: Once = Once::new();

    /// Erzwingt den plattform-unabhängigen Mock-Store für den Test-Prozess,
    /// damit `cargo test` ohne echten Keychain/Secret-Service-Daemon (CI!)
    /// deterministisch läuft.
    fn use_mock_store() {
        MOCK_INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    #[test]
    fn service_id_follows_schema() {
        assert_eq!(service_id("abc-123"), "kleinbuch::mail::abc-123");
    }

    #[test]
    fn chunk_round_trip_and_size_bound() {
        // Großer Token (4 KB) wie ein echter MS-Refresh-Token.
        let token: String = "A".repeat(4096);
        let chunks = split_into_chunks(&token, OAUTH_CHUNK_CHARS);
        assert_eq!(chunks.len(), 4);
        assert!(chunks
            .iter()
            .all(|c| c.chars().count() <= OAUTH_CHUNK_CHARS));
        assert_eq!(chunks.concat(), token);

        // Kurzer Token → genau ein Chunk.
        let short = "refresh.token.value";
        let one = split_into_chunks(short, OAUTH_CHUNK_CHARS);
        assert_eq!(one.len(), 1);
        assert_eq!(one.concat(), short);

        // Leerer Token → ein leerer Chunk (kein Panic).
        assert_eq!(
            split_into_chunks("", OAUTH_CHUNK_CHARS),
            vec![String::new()]
        );

        // Chunk-Service-IDs sind eindeutig.
        assert_ne!(chunk_service("svc", 0), chunk_service("svc", 1));
        assert_eq!(chunk_service("svc", 2), "svc::oauth.2");
    }

    // Hinweis: Der keyring-Mock-Store teilt seinen Zustand NICHT über separate
    // `Entry::new`-Aufrufe hinweg — jede Entry ist eine eigene In-Memory-
    // Credential. Die echte Persistenz (set hier, get dort) leistet der OS-Store
    // (Windows Credential Manager / macOS Keychain / Linux Secret Service) und
    // ist hier bewusst NICHT abgedeckt (würde den realen Schlüsselbund
    // verschmutzen und auf headless-CI scheitern). Getestet wird der Vertrag des
    // Wrappers: Fehler-Mapping + Idempotenz.
    #[test]
    fn wrapper_contract_against_mock() {
        use_mock_store();
        let svc = service_id("11111111-1111-7111-8111-111111111111");

        // Unbekannter Eintrag → Ok(None) (NoEntry-Mapping).
        assert_eq!(get_password(&svc).unwrap(), None);

        // set + delete dürfen nicht hart fehlschlagen; delete ist idempotent.
        set_password(&svc, "s3cr3t-passphrase").unwrap();
        delete_password(&svc).unwrap();
        delete_password(&svc).unwrap();
    }
}
