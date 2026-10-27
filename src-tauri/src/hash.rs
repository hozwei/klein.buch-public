//! SHA-256 als Hex-Text (Lauf F6b, LE-HASH-CRATES).
//!
//! Einzige Stelle, die Digest-Bytes in Hex umwandelt: Archiv-Hashes
//! (`archive_entries.file_hash_sha256`), Backup-Manifest und -Historie,
//! Migrations-Export, Steuerberater-Manifest, Startdateien und die Hashes der
//! TOTP-Wiederherstellungscodes. Kleinbuchstaben, 64 Zeichen — das Format ist
//! in Datenbanken und Dateien gespeichert und darf sich nie ändern
//! (Known-Answer-Tests unten und an den Aufrufstellen).

use sha2::{Digest, Sha256};

/// SHA-256 von `bytes` als 64 Hex-Zeichen in Kleinbuchstaben.
pub fn sha256_hex(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(&Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIPS 180-2 Anhang B.
    #[test]
    fn sha256_hex_known_answers() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }
}
