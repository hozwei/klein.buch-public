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
//! **Format 3 (Lauf F3a, W14-020):** Der Klartext-Kopf (beide Zeilen samt
//! Zeilenumbrüchen, also alle Bytes vor dem Ciphertext) ist als *Associated
//! Data* an AES-GCM gebunden. Ein veränderter Kopf (Schema-Version, Zeitpunkt,
//! KDF-Angaben, auch das Herabsetzen der Format-Version auf 2) lässt die
//! Entschlüsselung scheitern. Formate 1 und 2 (ohne AAD) bleiben lesbar; vor
//! dem Entschlüsseln bleibt der Kopf wie bisher eine ungeprüfte Anzeige.
//!
//! Es werden ausschließlich Hex-Strings verwendet (kein base64-Crate in den
//! Dependencies) — selbst-enthalten und reversibel.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Magic-Marker in Zeile 1. Versionsnummer am Ende erlaubt späteres Format-Bump.
pub const MAGIC: &str = "KLEIN-BUCH-BACKUP-1";

/// Aktuelle Backup-Format-Version (nicht zu verwechseln mit der DB-Schema-Version).
///
/// - 1: DB + `archive/` + `inputs/branding/`
/// - 2 (Plan W1.5): DB + `archive/` + `branding/` aus `data_dir` (Logo, Unterschrift)
/// - 3 (Lauf F3a, W14-020): Inhalt wie 2; der Klartext-Kopf ist als AAD an
///   AES-GCM gebunden ([`AAD_MIN_FORMAT_VERSION`])
pub const FORMAT_VERSION: u32 = 3;

/// Ab dieser Format-Version ist der Klartext-Kopf als AAD gebunden.
pub const AAD_MIN_FORMAT_VERSION: u32 = 3;

/// Bindet diese Format-Version den Kopf als AAD?
pub fn binds_header(format_version: u32) -> bool {
    format_version >= AAD_MIN_FORMAT_VERSION
}

/// Älteste Format-Version, die der Restore noch einspielt.
pub const MIN_SUPPORTED_FORMAT_VERSION: u32 = 1;

/// Älteste Format-Version, die noch geschrieben wird. Format 1 sicherte
/// `inputs/branding` statt `data/branding` und entsteht nicht mehr.
pub const MIN_WRITE_FORMAT_VERSION: u32 = 2;

/// Höchstes Backup-Format, das die Klein.Buch-Version lesen kann, deren
/// Datenbank-Schema höchstens den angegebenen Stand hat (Lauf F3c, ADR 0039
/// Nr. 129 (1)). Je Eintrag: (Schema bis einschließlich, höchstes lesbares
/// Format), aufsteigend; über dem letzten Eintrag gilt [`FORMAT_VERSION`].
///
/// - Schema ≤ 31: v2026.10.0 (Schema 31, liest Format 1–2). Auch ein Schema-30-
///   Stand (eingespieltes Desktop-Backup) fällt darunter: Rückfall-Ziel ist
///   v2026.10.0, die Schema 30 einspielt.
///
/// **Bei jeder Erhöhung von [`FORMAT_VERSION`]** einen Eintrag
/// (`EXPECTED_SCHEMA_VERSION` der Vorversion, deren `FORMAT_VERSION`) ergänzen,
/// sonst schreibt das Pflicht-Backup vor der nächsten Migration ein Format, das
/// die Vorversion nicht liest (Test `rollback_formats_cover_current_format`).
pub const READABLE_FORMAT_BY_SCHEMA: &[(i32, u32)] = &[(31, 2)];

/// Format für ein Backup, das die Version mit dem Schema-Stand `schema` noch
/// einspielen können muss — das Pflicht-Backup vor einer Migration von
/// `schema` aus, damit der dokumentierte Rückfall auf die Vorversion
/// funktioniert (Handbuch „Updates und Rückfall“).
pub fn readable_format_for_schema(schema: i32) -> u32 {
    READABLE_FORMAT_BY_SCHEMA
        .iter()
        .find(|(max_schema, _)| schema <= *max_schema)
        .map_or(FORMAT_VERSION, |(_, format)| *format)
}

/// Prüft, ob ein Backup in diesem Format geschrieben werden darf
/// ([`MIN_WRITE_FORMAT_VERSION`] bis [`FORMAT_VERSION`]).
pub fn ensure_writable_format(format_version: u32) -> Result<()> {
    if (MIN_WRITE_FORMAT_VERSION..=FORMAT_VERSION).contains(&format_version) {
        Ok(())
    } else {
        Err(Error::Backup(format!(
            "Backup-Format {format_version} kann nicht geschrieben werden (möglich \
             {MIN_WRITE_FORMAT_VERSION}–{FORMAT_VERSION})."
        )))
    }
}

/// Prüft, ob der Restore ein Backup dieses Formats einspielen kann.
pub fn ensure_supported_format(format_version: u32) -> Result<()> {
    if (MIN_SUPPORTED_FORMAT_VERSION..=FORMAT_VERSION).contains(&format_version) {
        Ok(())
    } else {
        Err(Error::Backup(format!(
            "Backup-Format {format_version} wird nicht unterstützt (erwartet \
             {MIN_SUPPORTED_FORMAT_VERSION}–{FORMAT_VERSION}) — bitte eine passende Klein.Buch-Version (Image) verwenden."
        )))
    }
}

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

/// Klartext-Kopf eines Backup-Files: `MAGIC\n + manifest-json\n`. Ab Format 3
/// sind genau diese Bytes die AAD der Verschlüsselung.
pub fn header_bytes(manifest: &Manifest) -> Result<Vec<u8>> {
    let json = manifest.to_json_line()?;
    let mut out = Vec::with_capacity(MAGIC.len() + json.len() + 2);
    out.extend_from_slice(MAGIC.as_bytes());
    out.push(b'\n');
    out.extend_from_slice(json.as_bytes());
    out.push(b'\n');
    Ok(out)
}

/// Serialisiert ein vollständiges Backup-File: `MAGIC\n + manifest-json\n + ciphertext`.
pub fn frame(manifest: &Manifest, ciphertext: &[u8]) -> Result<Vec<u8>> {
    let mut out = header_bytes(manifest)?;
    out.reserve_exact(ciphertext.len());
    out.extend_from_slice(ciphertext);
    Ok(out)
}

/// Zerlegt ein Backup-File in (Manifest, Ciphertext). Liest nur den Plain-Header,
/// ohne Passphrase. Kopiert den Ciphertext — für große Dateien
/// [`parse_header`] mit dem Offset nutzen.
pub fn unframe(bytes: &[u8]) -> Result<(Manifest, Vec<u8>)> {
    let (manifest, offset) = parse_header(bytes)?;
    Ok((manifest, bytes[offset..].to_vec()))
}

/// Liest den Klartext-Kopf: (Manifest, Offset des Ciphertexts). `bytes[..offset]`
/// ist der Kopf (ab Format 3 die AAD), ohne Passphrase lesbar.
pub fn parse_header(bytes: &[u8]) -> Result<(Manifest, usize)> {
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
    Ok((manifest, nl1 + 1 + nl2 + 1))
}

/// Liest nur den Manifest-Header eines Backup-Files (für die Restore-Anzeige),
/// ohne den (potentiell großen) Ciphertext zu kopieren. Ungeprüft — erst die
/// Entschlüsselung authentifiziert ihn (ab Format 3).
pub fn read_manifest_only(bytes: &[u8]) -> Result<Manifest> {
    parse_header(bytes).map(|(m, _)| m)
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
    fn parse_header_offset_points_to_ciphertext() {
        let m = sample_manifest();
        let ct = [7u8, 8, 9];
        let framed = frame(&m, &ct).unwrap();
        let (m2, offset) = parse_header(&framed).unwrap();
        assert_eq!(&framed[offset..], &ct);
        assert_eq!(&framed[..offset], header_bytes(&m).unwrap().as_slice());
        assert_eq!(m2.nonce_hex, m.nonce_hex);
        assert!(binds_header(FORMAT_VERSION));
        assert!(!binds_header(2) && !binds_header(1));
    }

    /// Lauf F3c (S1): Das Pflicht-Backup vor einer Migration von Schema 31
    /// (v2026.10.0) entsteht im Format 2, das v2026.10.0 liest; ab dem
    /// aktuellen Schema gilt das aktuelle Format.
    #[test]
    fn rollback_formats_cover_current_format() {
        use crate::db::schema_version::{EXPECTED_SCHEMA_VERSION, MIN_RESTORE_SCHEMA_VERSION};
        assert_eq!(readable_format_for_schema(31), 2, "v2026.10.0 liest 1–2");
        assert_eq!(readable_format_for_schema(MIN_RESTORE_SCHEMA_VERSION), 2);
        assert_eq!(
            readable_format_for_schema(EXPECTED_SCHEMA_VERSION),
            FORMAT_VERSION
        );
        // Wer FORMAT_VERSION erhöht, ergänzt READABLE_FORMAT_BY_SCHEMA um
        // (EXPECTED_SCHEMA_VERSION der Vorversion, deren Format) und passt
        // diese Erwartung an.
        assert_eq!(
            FORMAT_VERSION, 3,
            "Format erhöht: READABLE_FORMAT_BY_SCHEMA ergänzen"
        );
        let mut last = (i32::MIN, 0);
        for &(schema, format) in READABLE_FORMAT_BY_SCHEMA {
            assert!(schema > last.0 && format >= last.1, "aufsteigend");
            assert!(schema < EXPECTED_SCHEMA_VERSION && format < FORMAT_VERSION);
            assert!(ensure_writable_format(format).is_ok());
            last = (schema, format);
        }
        assert!(
            ensure_writable_format(1).is_err(),
            "Format 1 entsteht nicht mehr"
        );
        assert!(ensure_writable_format(FORMAT_VERSION + 1).is_err());
    }

    #[test]
    fn unframe_rejects_wrong_magic() {
        let mut framed = frame(&sample_manifest(), &[1, 2]).unwrap();
        framed[0] = b'X';
        assert!(unframe(&framed).is_err());
    }
}
