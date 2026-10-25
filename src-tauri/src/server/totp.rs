//! Zweiter Faktor TOTP (Plan W6, E5): RFC 6238 mit HMAC-SHA1, 6 Ziffern,
//! 30 s. Das Geheimnis (20 Byte aus dem OS-Zufall) liegt außerhalb der DB in
//! `/data/auth/totp.json` (0600, Verzeichnis 0700) und gehört nicht zu
//! Snapshot, Restore oder Reset:
//!
//! `{secret_b32, last_used_step, recovery_sha256[], created_at}`
//!
//! - Ein Code gilt für die Schritte t-1 bis t+1, aber nur für Schritte größer
//!   als `last_used_step` (kein Replay, auch nicht über Neustarts); der Schritt
//!   wird persistiert, bevor die Prüfung „gültig“ meldet — ein Code gilt als
//!   verbraucht, auch wenn die Passphrase danach falsch ist.
//! - Vergleich konstantzeitig (`subtle`).
//! - 10 Einmal-Codes (10 Zeichen Base32), nur als SHA-256 gespeichert,
//!   einmalig; sie ersetzen nur den TOTP-Faktor.
//! - Schreiben atomar (tmp + fsync + rename + Verzeichnis-fsync) unter einer
//!   Prozess-Sperre und `flock` auf `/data/auth/.lock` (gemeinsam mit dem
//!   Subcommand `totp-reset`).

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hmac::{Hmac, Mac};
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};

pub const DIGITS: u32 = 6;
pub const PERIOD_SECS: u64 = 30;
pub const SECRET_LEN: usize = 20;
pub const RECOVERY_CODES: usize = 10;
pub const RECOVERY_LEN: usize = 10;
pub const ISSUER: &str = "Klein.Buch";

/// Inhalt von `totp.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpFile {
    pub secret_b32: String,
    pub last_used_step: u64,
    pub recovery_sha256: Vec<String>,
    pub created_at: String,
}

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Valid,
    Invalid,
    /// Der Code passt zu einem bereits verbrauchten Zeitschritt (Wiederholung,
    /// RFC 6238 §5.2); abgelehnt wie `Invalid`, nur im Log unterschieden
    /// (`reason=totp_replay`, ASVS V2.8.5, Lauf F4a).
    Replay,
    /// `totp.json` fehlt (nach `totp-reset` oder vor der Einrichtung).
    Missing,
}

/// Base32 (RFC 4648, ohne Padding) des Geheimnisses.
pub fn encode_secret(secret: &[u8]) -> String {
    data_encoding::BASE32_NOPAD.encode(secret)
}

pub fn decode_secret(b32: &str) -> Result<Vec<u8>> {
    let normalized: String = b32
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    data_encoding::BASE32_NOPAD
        .decode(normalized.trim_end_matches('=').as_bytes())
        .map_err(|_| Error::Crypto("TOTP-Geheimnis ist kein gültiges Base32".into()))
}

/// Neues Geheimnis aus dem OS-Zufall.
pub fn generate_secret() -> Vec<u8> {
    let mut s = vec![0u8; SECRET_LEN];
    rand::rngs::OsRng.fill_bytes(&mut s);
    s
}

/// Zeitschritt zu einer Unix-Zeit.
pub fn step_at(unix_secs: u64) -> u64 {
    unix_secs / PERIOD_SECS
}

/// Code für einen Zeitschritt (RFC 4226 dynamische Kürzung).
pub fn code_for_step(secret: &[u8], step: u64) -> String {
    let mut mac = match Hmac::<Sha1>::new_from_slice(secret) {
        Ok(m) => m,
        // HMAC akzeptiert jede Schlüssellänge; der Zweig ist unerreichbar.
        Err(_) => return String::new(),
    };
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    format!(
        "{:0width$}",
        bin % 10u32.pow(DIGITS),
        width = DIGITS as usize
    )
}

/// `otpauth://`-URI für Authenticator-Apps.
pub fn otpauth_uri(secret: &[u8], label: &str) -> String {
    let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits={DIGITS}&period={PERIOD_SECS}",
        enc(ISSUER),
        enc(label),
        encode_secret(secret),
        enc(ISSUER)
    )
}

/// QR-Code der URI als SVG (lokal erzeugt).
pub fn qr_svg(uri: &str) -> Result<String> {
    let code = qrcode::QrCode::new(uri.as_bytes())
        .map_err(|e| Error::Other(anyhow::anyhow!("QR-Code: {e}")))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build();
    // Ohne XML-Deklaration, damit das SVG direkt ins HTML eingebettet werden kann.
    Ok(match svg.find("<svg") {
        Some(i) => svg[i..].to_string(),
        None => svg,
    })
}

/// Einmal-Codes: Klartext (einmal anzeigen) und SHA-256 (speichern).
pub fn new_recovery_codes() -> (Vec<String>, Vec<String>) {
    let mut plain = Vec::with_capacity(RECOVERY_CODES);
    let mut hashes = Vec::with_capacity(RECOVERY_CODES);
    for _ in 0..RECOVERY_CODES {
        let mut raw = [0u8; 7];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let code: String = data_encoding::BASE32_NOPAD
            .encode(&raw)
            .chars()
            .take(RECOVERY_LEN)
            .collect();
        hashes.push(recovery_hash(&code));
        plain.push(format!("{}-{}", &code[..5], &code[5..]));
    }
    (plain, hashes)
}

fn normalize_recovery(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn recovery_hash(normalized: &str) -> String {
    let digest = Sha256::digest(normalized.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Ablage `/data/auth/totp.json`.
pub struct TotpStore {
    dir: PathBuf,
    /// Serialisiert Lesen-Prüfen-Schreiben im Prozess.
    guard: Mutex<()>,
}

pub const AUTH_DIR: &str = "auth";
pub const TOTP_FILE: &str = "totp.json";
pub const LOCK_FILE: &str = ".lock";

impl TotpStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join(AUTH_DIR),
            guard: Mutex::new(()),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(TOTP_FILE)
    }

    pub fn exists(&self) -> bool {
        self.path().exists()
    }

    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    /// `flock` auf `/data/auth/.lock` (blockierend, kurz).
    fn flock(&self) -> Result<std::fs::File> {
        self.ensure_dir()?;
        let f = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join(LOCK_FILE))?;
        f.lock()?;
        Ok(f)
    }

    fn read(&self) -> Result<Option<TotpFile>> {
        match std::fs::read(self.path()) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn write(&self, file: &TotpFile) -> Result<()> {
        self.ensure_dir()?;
        let tmp = self.dir.join(format!(
            ".{TOTP_FILE}.{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let written = (|| -> Result<()> {
            let mut opts = OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                opts.mode(0o600);
            }
            let mut f = opts.open(&tmp)?;
            f.write_all(serde_json::to_string_pretty(file)?.as_bytes())?;
            f.sync_all()?;
            std::fs::rename(&tmp, self.path())?;
            Ok(())
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        if let Ok(dir) = std::fs::File::open(&self.dir) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    /// Legt das Geheimnis neu an (Einrichtung, Neueinrichtung) und liefert die
    /// Einmal-Codes im Klartext. `confirmed_step` ist der Zeitschritt des
    /// Bestätigungscodes — er gilt als verbraucht.
    pub fn install(&self, secret: &[u8], confirmed_step: u64) -> Result<Vec<String>> {
        let _g = self.guard.lock().unwrap_or_else(|p| p.into_inner());
        let _f = self.flock()?;
        let (plain, hashes) = new_recovery_codes();
        let file = TotpFile {
            secret_b32: encode_secret(secret),
            last_used_step: confirmed_step,
            recovery_sha256: hashes,
            created_at: crate::clock::now_utc().to_rfc3339(),
        };
        self.write(&file)?;
        Ok(plain)
    }

    /// Prüft einen 6-stelligen Code; ein gültiger Schritt wird vor der Antwort
    /// als verbraucht gespeichert.
    pub fn verify_code(&self, input: &str, now_unix: u64) -> Result<Check> {
        let code: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        let _g = self.guard.lock().unwrap_or_else(|p| p.into_inner());
        let _f = self.flock()?;
        let Some(mut file) = self.read()? else {
            return Ok(Check::Missing);
        };
        let well_formed = code.len() == DIGITS as usize && code.bytes().all(|b| b.is_ascii_digit());
        let secret = decode_secret(&file.secret_b32)?;
        let now = step_at(now_unix);
        let mut matched: Option<u64> = None;
        let mut replayed = false;
        for step in [now.saturating_sub(1), now, now + 1] {
            let expected = code_for_step(&secret, step);
            let eq: bool = expected.as_bytes().ct_eq(code.as_bytes()).into();
            if eq && well_formed && step > file.last_used_step && matched.is_none() {
                matched = Some(step);
            }
            replayed |= eq && well_formed && step <= file.last_used_step;
        }
        match matched {
            Some(step) => {
                file.last_used_step = step;
                self.write(&file)?;
                Ok(Check::Valid)
            }
            None if replayed => Ok(Check::Replay),
            None => Ok(Check::Invalid),
        }
    }

    /// Prüft einen Einmal-Code; ein gültiger wird sofort entfernt.
    pub fn use_recovery(&self, input: &str) -> Result<Check> {
        let normalized = normalize_recovery(input);
        let hash = recovery_hash(&normalized);
        let _g = self.guard.lock().unwrap_or_else(|p| p.into_inner());
        let _f = self.flock()?;
        let Some(mut file) = self.read()? else {
            return Ok(Check::Missing);
        };
        let mut hit: Option<usize> = None;
        for (i, stored) in file.recovery_sha256.iter().enumerate() {
            let eq: bool = stored.as_bytes().ct_eq(hash.as_bytes()).into();
            if eq && hit.is_none() && normalized.len() == RECOVERY_LEN {
                hit = Some(i);
            }
        }
        match hit {
            Some(i) => {
                file.recovery_sha256.remove(i);
                self.write(&file)?;
                Ok(Check::Valid)
            }
            None => Ok(Check::Invalid),
        }
    }

    /// Ersetzt die Einmal-Codes (Re-Auth vorausgesetzt).
    pub fn renew_recovery(&self) -> Result<Vec<String>> {
        let _g = self.guard.lock().unwrap_or_else(|p| p.into_inner());
        let _f = self.flock()?;
        let mut file = self
            .read()?
            .ok_or_else(|| Error::Backup("TOTP ist nicht eingerichtet.".into()))?;
        let (plain, hashes) = new_recovery_codes();
        file.recovery_sha256 = hashes;
        self.write(&file)?;
        Ok(plain)
    }

    /// Verbleibende Einmal-Codes.
    pub fn recovery_left(&self) -> usize {
        self.read()
            .ok()
            .flatten()
            .map(|f| f.recovery_sha256.len())
            .unwrap_or(0)
    }

    /// Subcommand `totp-reset`: löscht `totp.json` unter `flock`.
    pub fn reset(&self) -> Result<bool> {
        let _f = self.flock()?;
        match std::fs::remove_file(self.path()) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

/// Aktuelle Unix-Zeit in Sekunden.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 Anhang B (SHA1, Geheimnis "12345678901234567890"), 6 Stellen.
    #[test]
    fn rfc6238_vectors() {
        let secret = b"12345678901234567890";
        for (t, expected) in [
            (59u64, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
            (20_000_000_000, "353130"),
        ] {
            assert_eq!(code_for_step(secret, step_at(t)), expected, "t={t}");
        }
    }

    #[test]
    fn window_replay_and_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let store = TotpStore::new(dir.path());
        assert_eq!(store.verify_code("123456", 1000).unwrap(), Check::Missing);
        let secret = generate_secret();
        let t0 = 1_800_000_000u64;
        // Bestätigung mit dem Code von Schritt t0+1 (Uhr der App eine Periode vor)
        let codes = store.install(&secret, step_at(t0) + 1).unwrap();
        assert_eq!(codes.len(), RECOVERY_CODES);
        let confirmed = code_for_step(&secret, step_at(t0) + 1);
        assert_eq!(
            store.verify_code(&confirmed, t0).unwrap(),
            Check::Replay,
            "Bestätigungscode verbraucht"
        );
        let t = t0 + 2 * PERIOD_SECS;
        let code = code_for_step(&secret, step_at(t));
        assert_eq!(store.verify_code(&code, t).unwrap(), Check::Valid);
        assert_eq!(
            store.verify_code(&code, t).unwrap(),
            Check::Replay,
            "Replay"
        );
        // Neuer Store (Neustart): Replay bleibt verboten.
        let again = TotpStore::new(dir.path());
        assert_eq!(again.verify_code(&code, t).unwrap(), Check::Replay);
        // t+1 ok, t+2 (zwei Schritte voraus) nicht
        let later = t + PERIOD_SECS;
        let plus1 = code_for_step(&secret, step_at(later) + 1);
        assert_eq!(again.verify_code(&plus1, later).unwrap(), Check::Valid);
        let plus2 = code_for_step(&secret, step_at(later) + 2);
        assert_eq!(again.verify_code(&plus2, later).unwrap(), Check::Invalid);
        // t-1 ok (sofern noch nicht verbraucht), t-2 nicht
        let much_later = later + 10 * PERIOD_SECS;
        let minus2 = code_for_step(&secret, step_at(much_later) - 2);
        assert_eq!(
            again.verify_code(&minus2, much_later).unwrap(),
            Check::Invalid
        );
        let minus1 = code_for_step(&secret, step_at(much_later) - 1);
        assert_eq!(
            again.verify_code(&minus1, much_later).unwrap(),
            Check::Valid
        );
        assert_eq!(again.verify_code("12345", later).unwrap(), Check::Invalid);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(store.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn recovery_codes_are_single_use() {
        let dir = tempfile::tempdir().unwrap();
        let store = TotpStore::new(dir.path());
        let codes = store
            .install(&generate_secret(), step_at(1_800_000_000))
            .unwrap();
        assert_eq!(store.use_recovery(&codes[0]).unwrap(), Check::Valid);
        assert_eq!(store.use_recovery(&codes[0]).unwrap(), Check::Invalid);
        assert_eq!(
            store
                .use_recovery(&codes[1].to_lowercase().replace('-', " "))
                .unwrap(),
            Check::Valid,
            "Schreibweise egal"
        );
        assert_eq!(store.recovery_left(), RECOVERY_CODES - 2);
        let fresh = store.renew_recovery().unwrap();
        assert_eq!(store.recovery_left(), RECOVERY_CODES);
        assert_eq!(
            store.use_recovery(&codes[2]).unwrap(),
            Check::Invalid,
            "alte ungültig"
        );
        assert_eq!(store.use_recovery(&fresh[0]).unwrap(), Check::Valid);
        assert!(store.reset().unwrap());
        assert!(!store.exists());
    }

    #[test]
    fn uri_and_qr() {
        // Testvektor aus RFC 6238 Anhang B (öffentlich, kein Geheimnis).
        let secret = b"12345678901234567890";
        let uri = otpauth_uri(secret, "kontakt@example.org");
        assert!(uri.starts_with("otpauth://totp/Klein.Buch:kontakt%40example.org?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")); // gitleaks:allow
        let svg = qr_svg(&uri).unwrap();
        assert!(svg.starts_with("<svg"), "{}", &svg[..40]);
        assert_eq!(
            decode_secret("gezd gnbv gy3t qojq gezd gnbv gy3t qojq").unwrap(),
            secret
        );
    }
}
