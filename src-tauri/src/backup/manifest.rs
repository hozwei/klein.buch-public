//! Backup-File-Manifest (Block 4).
//!
//! Datei-Layout (plain Header, verschlüsselter Body):
//! ```text
//! Zeile 1: MAGIC ("KLEIN-BUCH-BACKUP-1")        \n
//! Zeile 2: Manifest-JSON (eine Zeile, kein Pretty) \n
//! ab Byte danach: AES-256-GCM-Ciphertext (Content-ZIP + 16-Byte-Tag)
//! ```
//!
//! Der Header ist absichtlich **plain JSON**, damit der Restore-Wizard
//! Versions-/Datums-Metadaten anzeigen kann, **ohne** die Passphrase zu kennen.
//! Der Body (DB-Snapshot + Archive + Branding) ist verschlüsselt und über das
//! GCM-Tag integritätsgeschützt.
//!
//! Es werden ausschließlich Hex-Strings verwendet (kein base64-Crate in den
//! Dependencies) — selbst-enthalten und reversibel.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Magic-Marker in Zeile 1. Versionsnummer am Ende erlaubt späteres Format-Bump.
pub const MAGIC: &str = "KLEIN-BUCH-BACKUP-1";

/// Aktuelle Backup-Format-Version (nicht zu verwechseln mit der DB-Schema-Version).
pub const FORMAT_VERSION: u32 = 1;

/// KDF-Parameter, die zur Wiederherstellung des Schlüssels nötig sind.
/// Werden mitgeschrieben, damit ein künftiger Parameter-Wechsel alte Backups
/// nicht unlesbar macht.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    /// Immer "argon2id".
    pub algo: String,
    /// Argon2 memory cost in 1-KiB-Blöcken (64 MiB = 65536).
    pub m_cost_kib: u32,
    /// Argon2 iterations (t).
    pub t_cost: u32,
    /// Argon2 parallelism (p).
    pub p_cost: u32,
}

/// Plain-Header eines Backup-Files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub magic: String,
    pub format_version: u32,
    /// DB-Schema-Version zum Backup-Zeitpunkt (für Restore-Kompatibilitätscheck).
    pub schema_version: i32,
    /// Klein.Buch-App-Version (CARGO_PKG_VERSION).
    pub app_version: String,
    /// ISO-8601-UTC-Zeitstempel der Erstellung.
    pub created_at: String,
    pub kdf: KdfParams,
    /// Hex-codiertes Argon2-Salt (16 Byte).
    pub salt_hex: String,
    /// Hex-codierter AES-GCM-Nonce (12 Byte).
    pub nonce_hex: String,
    /// SHA-256 (hex) des **entschlüsselten** Content-ZIP — Doppel-Check nach Decrypt.
    pub content_sha256: String,
    /// Größe des entschlüsselten Content-ZIP in Byte.
    pub content_size_bytes: u64,
}

impl Manifest {
    /// Serialisiert als einzeilige JSON-Repräsentation (kein Pretty → keine `\n`).
    pub fn to_json_line(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Error::from)
    }

    pub fn from_json_line(line: &str) -> Result<Self> {
        let m: Manifest = serde_json::from_str(line)?;
        if m.magic != MAGIC {
            return Err(Error::Backup(format!(
                "ungültiges Backup: Magic '{}' erwartet, '{}' gefunden",
                MAGIC, m.magic
            )));
        }
        Ok(m)
    }
}

/// Serialisiert ein vollständiges Backup-File: `MAGIC\n + manifest-json\n + ciphertext`.
pub fn frame(manifest: &Manifest, ciphertext: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(ciphertext.len() + 512);
    out.extend_from_slice(MAGIC.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(manifest.to_json_line()?.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(ciphertext);
    Ok(out)
}

/// Zerlegt ein Backup-File in (Manifest, Ciphertext). Liest nur den Plain-Header,
/// ohne Passphrase.
pub fn unframe(bytes: &[u8]) -> Result<(Manifest, Vec<u8>)> {
    // Erste Newline = Ende MAGIC.
    let nl1 = bytes
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| Error::Backup("Backup-Datei: kein MAGIC-Header".into()))?;
    let magic = std::str::from_utf8(&bytes[..nl1])
        .map_err(|_| Error::Backup("Backup-Datei: MAGIC nicht UTF-8".into()))?;
    if magic != MAGIC {
        return Err(Error::Backup(format!(
            "ungültiges Backup: Magic '{MAGIC}' erwartet, '{magic}' gefunden"
        )));
    }
    // Zweite Newline = Ende Manifest-JSON.
    let rest = &bytes[nl1 + 1..];
    let nl2 = rest
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| Error::Backup("Backup-Datei: kein Manifest-Header".into()))?;
    let manifest_line = std::str::from_utf8(&rest[..nl2])
        .map_err(|_| Error::Backup("Backup-Datei: Manifest nicht UTF-8".into()))?;
    let manifest = Manifest::from_json_line(manifest_line)?;
    let ciphertext = rest[nl2 + 1..].to_vec();
    Ok((manifest, ciphertext))
}

/// Liest nur den Manifest-Header eines Backup-Files (für die Restore-Anzeige),
/// ohne den (potentiell großen) Ciphertext zu kopieren.
pub fn read_manifest_only(bytes: &[u8]) -> Result<Manifest> {
    let nl1 = bytes
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| Error::Backup("Backup-Datei: kein MAGIC-Header".into()))?;
    let rest = &bytes[nl1 + 1..];
    let nl2 = rest
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| Error::Backup("Backup-Datei: kein Manifest-Header".into()))?;
    let manifest_line = std::str::from_utf8(&rest[..nl2])
        .map_err(|_| Error::Backup("Backup-Datei: Manifest nicht UTF-8".into()))?;
    Manifest::from_json_line(manifest_line)
}

// ---------------------------------------------------------------------------
// Hex-Helfer (kein base64-Crate in den Deps).
// ---------------------------------------------------------------------------

pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
    }
    s
}

pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err(Error::Backup("Hex-String mit ungerader Länge".into()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or_else(|| Error::Backup("ungültiges Hex-Zeichen".into()))?;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| Error::Backup("ungültiges Hex-Zeichen".into()))?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            magic: MAGIC.into(),
            format_version: FORMAT_VERSION,
            schema_version: 4,
            app_version: "0.1.0".into(),
            created_at: "2026-05-20T10:00:00Z".into(),
            kdf: KdfParams {
                algo: "argon2id".into(),
                m_cost_kib: 65536,
                t_cost: 3,
                p_cost: 4,
            },
            salt_hex: to_hex(&[1u8; 16]),
            nonce_hex: to_hex(&[2u8; 12]),
            content_sha256: to_hex(&[3u8; 32]),
            content_size_bytes: 12345,
        }
    }

    #[test]
    fn hex_roundtrip() {
        let data = [0x00u8, 0x0f, 0xa5, 0xff, 0x10, 0x7c];
        let hex = to_hex(&data);
        assert_eq!(hex, "000fa5ff107c");
        assert_eq!(from_hex(&hex).unwrap(), data);
    }

    #[test]
    fn from_hex_rejects_bad_input() {
        assert!(from_hex("abc").is_err()); // ungerade Länge
        assert!(from_hex("zz").is_err()); // ungültiges Zeichen
    }

    #[test]
    fn frame_unframe_roundtrip() {
        let m = sample_manifest();
        let ct = vec![9u8, 8, 7, 6, 5, b'\n', 0, 255]; // enthält absichtlich ein \n
        let framed = frame(&m, &ct).unwrap();
        let (m2, ct2) = unframe(&framed).unwrap();
        assert_eq!(ct2, ct);
        assert_eq!(m2.schema_version, m.schema_version);
        assert_eq!(m2.salt_hex, m.salt_hex);
        assert_eq!(m2.kdf, m.kdf);
    }

    #[test]
    fn read_manifest_only_matches() {
        let m = sample_manifest();
        let framed = frame(&m, &[1, 2, 3]).unwrap();
        let m2 = read_manifest_only(&framed).unwrap();
        assert_eq!(m2.created_at, m.created_at);
        assert_eq!(m2.app_version, m.app_version);
    }

    #[test]
    fn unframe_rejects_wrong_magic() {
        let mut framed = frame(&sample_manifest(), &[1, 2]).unwrap();
        framed[0] = b'X';
        assert!(unframe(&framed).is_err());
    }
}
