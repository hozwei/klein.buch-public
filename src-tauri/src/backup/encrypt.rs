//! Krypto-Primitive für Backups (Block 4).
//!
//! - **KDF:** Argon2id, `m=64 MiB (65536 KiB), t=3, p=4`, 32-Byte-Output
//!   (OWASP-Empfehlung 2024). Salt zufällig pro Backup.
//! - **AEAD:** AES-256-GCM, 96-Bit-Nonce zufällig pro Backup. Das GCM-Tag
//!   (16 Byte, an den Ciphertext angehängt) liefert Integritätsschutz —
//!   eine Byte-Manipulation am Cipher schlägt beim Decrypt als Auth-Fehler an.
//!
//! Die **Passphrase wird nie persistiert** (nicht in DB, Logs, audit_log).
//! Sie lebt nur im Prozess-Memory der laufenden Session (siehe
//! `backup::BackupSession`) und wird zum Restore neu abgefragt.
//!
//! Verifiziert gegen argon2 0.5.3 (`Params::new(m_kib, t, p, Some(len))`,
//! `hash_password_into`) und aes-gcm 0.10.3 (`Aes256Gcm::new` + `encrypt`/
//! `decrypt`, Tag wird angehängt).

use crate::error::{Error, Result};
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

pub const KDF_M_COST_KIB: u32 = 65536; // 64 MiB
pub const KDF_T_COST: u32 = 3;
pub const KDF_P_COST: u32 = 4;
pub const KEY_LEN: usize = 32; // AES-256
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12; // 96-bit GCM nonce

/// Magic-Klartext für den Passphrase-Verifier. Wird beim Setup mit dem
/// abgeleiteten Schlüssel verschlüsselt und in `app_settings` abgelegt.
/// Beim Unlock wird er wieder entschlüsselt — schlägt das GCM-Tag fehl,
/// war die Passphrase falsch. So muss die Passphrase nirgends gespeichert
/// werden (auch kein Hash nötig).
pub const VERIFIER_PLAINTEXT: &[u8] = b"KLEIN-BUCH-PASSPHRASE-OK";

/// Erzeugt `N` kryptographisch zufällige Bytes (OS-RNG).
pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf
}

/// Leitet aus Passphrase + Salt den 32-Byte-AES-Schlüssel ab (Argon2id).
pub fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    if salt.len() < 8 {
        return Err(Error::Crypto("Salt zu kurz (min. 8 Byte)".into()));
    }
    let params = Params::new(KDF_M_COST_KIB, KDF_T_COST, KDF_P_COST, Some(KEY_LEN))
        .map_err(|e| Error::Crypto(format!("Argon2-Params: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(passphrase, salt, &mut key)
        .map_err(|e| Error::Crypto(format!("Argon2-KDF: {e}")))?;
    Ok(key)
}

/// AES-256-GCM-Verschlüsselung. Rückgabe: Ciphertext **inkl. angehängtem
/// 16-Byte-GCM-Tag**.
pub fn encrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|e| Error::Crypto(format!("AES-GCM-Encrypt: {e}")))
}

/// AES-256-GCM-Entschlüsselung. Erwartet Ciphertext inkl. GCM-Tag.
/// Ein manipuliertes Byte (oder falscher Schlüssel) führt zu `Err(Crypto)`.
pub fn decrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| {
            Error::Crypto(
                "Authentifizierung fehlgeschlagen (falsche Passphrase oder beschädigtes Backup)"
                    .into(),
            )
        })
}

/// Hilfsfunktion: Salt-Slice → fixes Array.
pub fn salt_array(salt: &[u8]) -> Result<[u8; SALT_LEN]> {
    salt.try_into()
        .map_err(|_| Error::Crypto(format!("Salt muss {SALT_LEN} Byte sein")))
}

/// Hilfsfunktion: Nonce-Slice → fixes Array.
pub fn nonce_array(nonce: &[u8]) -> Result<[u8; NONCE_LEN]> {
    nonce
        .try_into()
        .map_err(|_| Error::Crypto(format!("Nonce muss {NONCE_LEN} Byte sein")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let salt = random_bytes(SALT_LEN);
        let key = derive_key(b"correct horse battery staple", &salt).unwrap();
        let nonce: [u8; NONCE_LEN] = random_bytes(NONCE_LEN).try_into().unwrap();
        let pt = b"Klein.Buch Backup Content";
        let ct = encrypt(&key, &nonce, pt).unwrap();
        assert_ne!(&ct[..], &pt[..]); // wirklich verschlüsselt
        assert_eq!(ct.len(), pt.len() + 16); // + GCM-Tag
        let back = decrypt(&key, &nonce, &ct).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let salt = random_bytes(SALT_LEN);
        let key = derive_key(b"passphrase A", &salt).unwrap();
        let nonce: [u8; NONCE_LEN] = random_bytes(NONCE_LEN).try_into().unwrap();
        let ct = encrypt(&key, &nonce, b"secret").unwrap();

        let wrong = derive_key(b"passphrase B", &salt).unwrap();
        assert!(decrypt(&wrong, &nonce, &ct).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_auth() {
        let salt = random_bytes(SALT_LEN);
        let key = derive_key(b"pw", &salt).unwrap();
        let nonce: [u8; NONCE_LEN] = random_bytes(NONCE_LEN).try_into().unwrap();
        let mut ct = encrypt(&key, &nonce, b"important data").unwrap();
        ct[0] ^= 0x01; // ein Byte kippen
        assert!(decrypt(&key, &nonce, &ct).is_err());
    }

    #[test]
    fn same_passphrase_different_salt_different_key() {
        let k1 = derive_key(b"pw", &random_bytes(SALT_LEN)).unwrap();
        let k2 = derive_key(b"pw", &random_bytes(SALT_LEN)).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn verifier_roundtrip() {
        // Simuliert Setup→Unlock: Verifier verschlüsseln, dann mit gleicher
        // Passphrase entschlüsseln; mit falscher Passphrase muss es scheitern.
        let salt = random_bytes(SALT_LEN);
        let nonce: [u8; NONCE_LEN] = random_bytes(NONCE_LEN).try_into().unwrap();
        let key = derive_key(b"meine-passphrase", &salt).unwrap();
        let verifier = encrypt(&key, &nonce, VERIFIER_PLAINTEXT).unwrap();

        let key_ok = derive_key(b"meine-passphrase", &salt).unwrap();
        assert_eq!(
            decrypt(&key_ok, &nonce, &verifier).unwrap(),
            VERIFIER_PLAINTEXT
        );

        let key_bad = derive_key(b"falsch", &salt).unwrap();
        assert!(decrypt(&key_bad, &nonce, &verifier).is_err());
    }
}
