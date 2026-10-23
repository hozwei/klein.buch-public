//! Rate-Limit für Anmeldung, Einrichtung und Re-Auth ohne Selbstaussperrung
//! (Plan W6).
//!
//! - Je Schlüssel (gültiges Gerätecookie, sonst Client-IP; IPv6 je /64, damit
//!   Adress-Rotation innerhalb eines Anschlusses nichts bringt) ab
//!   [`FREE_ATTEMPTS`] Fehlversuchen Wartezeit ab 1 min, verdoppelnd bis 1 h;
//!   der Zähler verfällt 24 h nach dem letzten Fehlversuch. Keine dauerhafte
//!   Sperre.
//! - Zusätzlich ein globales Budget für Clients **ohne** Gerätecookie (ab
//!   [`FREE_GLOBAL`] Fehlversuchen dieselbe Wartezeit-Staffel, Review W6): wer
//!   die Passphrase kennt, kann den TOTP-Faktor auch mit vielen Adressen nicht
//!   durchprobieren; der eigene Client mit Gerätecookie bleibt frei.
//! - Gerätecookie `__Host-kb_device` = `<id>.<HMAC-SHA256(Serverschlüssel, id)>`
//!   (1 Jahr): ein Client mit gültigem Cookie zählt unter seinem eigenen
//!   Schlüssel und wird von Wartezeiten fremder Clients (z. B. gleiche IP
//!   hinter einem Proxy ohne `TRUSTED_PROXIES`) nicht getroffen.
//! - Der Serverschlüssel liegt in `/data/auth/device.key` (32 Byte, 0600).
//! - Fehlversuche seit der letzten erfolgreichen Anmeldung (für den Hinweis
//!   nach dem Login).

use std::collections::HashMap;
use std::io::Write as _;
use std::net::IpAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use base64::Engine as _;
use hmac::{Hmac, Mac};
use rand::RngCore as _;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::time::Instant;

use crate::error::Result;

pub const FREE_ATTEMPTS: u32 = 5;
/// Freie Fehlversuche aller Clients ohne Gerätecookie zusammen.
pub const FREE_GLOBAL: u32 = 50;
const GLOBAL_KEY: &str = "global";
pub const BASE_WAIT: Duration = Duration::from_secs(60);
pub const MAX_WAIT: Duration = Duration::from_secs(3600);
pub const EXPIRY: Duration = Duration::from_secs(24 * 3600);
pub const DEVICE_COOKIE: &str = "__Host-kb_device";
pub const DEVICE_KEY_FILE: &str = "device.key";
const MAX_ENTRIES: usize = 10_000;

struct Entry {
    failures: u32,
    last_failure: Instant,
    blocked_until: Option<Instant>,
}

pub struct RateLimiter {
    entries: Mutex<HashMap<String, Entry>>,
    failed_since_success: AtomicU64,
    device_key: [u8; 32],
}

/// Schlüssel für das Rate-Limit (IPv6 zusammengefasst je /64).
pub fn key_for(device: Option<&str>, ip: IpAddr) -> String {
    match (device, ip) {
        (Some(id), _) => format!("device:{id}"),
        (None, IpAddr::V6(v6)) => {
            let s = v6.segments();
            format!("ip:{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
        (None, IpAddr::V4(v4)) => format!("ip:{v4}"),
    }
}

impl RateLimiter {
    pub fn new(device_key: [u8; 32]) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            failed_since_success: AtomicU64::new(0),
            device_key,
        }
    }

    /// Lädt bzw. erzeugt den Serverschlüssel unter `<data>/auth/device.key`.
    pub fn load_or_create(data_dir: &Path) -> Result<Self> {
        let dir = data_dir.join(super::totp::AUTH_DIR);
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let path = dir.join(DEVICE_KEY_FILE);
        let mut key = [0u8; 32];
        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() == 32 => key.copy_from_slice(&bytes),
            _ => {
                rand::rngs::OsRng.fill_bytes(&mut key);
                let tmp = dir.join(format!(
                    ".{DEVICE_KEY_FILE}.{}.tmp",
                    uuid::Uuid::new_v4().simple()
                ));
                let mut opts = std::fs::OpenOptions::new();
                opts.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt as _;
                    opts.mode(0o600);
                }
                let mut f = opts.open(&tmp)?;
                f.write_all(&key)?;
                f.sync_all()?;
                std::fs::rename(&tmp, &path)?;
            }
        }
        Ok(Self::new(key))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn wait_of(map: &mut HashMap<String, Entry>, key: &str, now: Instant) -> Option<Duration> {
    let e = map.get(key)?;
    if now.duration_since(e.last_failure) >= EXPIRY {
        map.remove(key);
        return None;
    }
    e.blocked_until
        .filter(|until| now < *until)
        .map(|until| until - now)
}

fn count(map: &mut HashMap<String, Entry>, key: &str, free: u32, now: Instant) {
    let e = map.entry(key.to_string()).or_insert(Entry {
        failures: 0,
        last_failure: now,
        blocked_until: None,
    });
    if now.duration_since(e.last_failure) >= EXPIRY {
        e.failures = 0;
    }
    e.failures += 1;
    e.last_failure = now;
    if e.failures >= free {
        let doublings = (e.failures - free).min(16);
        let wait = BASE_WAIT.saturating_mul(1u32 << doublings).min(MAX_WAIT);
        e.blocked_until = Some(now + wait);
    }
}

impl RateLimiter {
    /// Darf der Schlüssel es versuchen? Ohne Gerätecookie zählt zusätzlich das
    /// globale Budget. Sonst die verbleibende Wartezeit.
    pub fn check(&self, key: &str, has_device: bool) -> std::result::Result<(), Duration> {
        let now = Instant::now();
        let mut map = self.lock();
        let own = wait_of(&mut map, key, now);
        let global = if has_device {
            None
        } else {
            wait_of(&mut map, GLOBAL_KEY, now)
        };
        match own.max(global) {
            Some(wait) => Err(wait),
            None => Ok(()),
        }
    }

    /// Fehlversuch zählen (ohne Gerätecookie auch im globalen Budget).
    pub fn failure(&self, key: &str, has_device: bool) {
        let now = Instant::now();
        self.failed_since_success.fetch_add(1, Ordering::SeqCst);
        let mut map = self.lock();
        if map.len() >= MAX_ENTRIES {
            map.retain(|k, e| k == GLOBAL_KEY || now.duration_since(e.last_failure) < EXPIRY);
        }
        if map.len() >= MAX_ENTRIES {
            // Immer noch voll: die ältesten Zehntel verdrängen (begrenzter Speicher).
            let mut by_age: Vec<(String, Instant)> = map
                .iter()
                .filter(|(k, _)| k.as_str() != GLOBAL_KEY)
                .map(|(k, e)| (k.clone(), e.last_failure))
                .collect();
            by_age.sort_by_key(|(_, t)| *t);
            for (k, _) in by_age.into_iter().take(MAX_ENTRIES / 10) {
                map.remove(&k);
            }
        }
        count(&mut map, key, FREE_ATTEMPTS, now);
        if !has_device {
            count(&mut map, GLOBAL_KEY, FREE_GLOBAL, now);
        }
    }

    /// Erfolgreiche Anmeldung: Zähler des Schlüssels zurücksetzen; liefert die
    /// Fehlversuche seit der letzten erfolgreichen Anmeldung.
    pub fn success_login(&self, key: &str) -> u64 {
        self.lock().remove(key);
        self.failed_since_success.swap(0, Ordering::SeqCst)
    }

    /// Erfolgreiche Einrichtung bzw. Re-Auth: nur den Schlüssel zurücksetzen
    /// (der Hinweis „Fehlversuche seit der letzten Anmeldung“ bleibt).
    pub fn clear(&self, key: &str) {
        self.lock().remove(key);
    }

    fn sign(&self, id: &str) -> String {
        let mut mac = match Hmac::<Sha256>::new_from_slice(&self.device_key) {
            Ok(m) => m,
            Err(_) => return String::new(),
        };
        mac.update(id.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    /// Neuer Gerätecookie-Wert.
    pub fn new_device_cookie(&self) -> String {
        let mut raw = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
        format!("{id}.{}", self.sign(&id))
    }

    /// Geräte-ID aus einem Cookie-Wert, wenn die Signatur stimmt.
    pub fn device_id(&self, value: &str) -> Option<String> {
        let (id, sig) = value.split_once('.')?;
        if id.is_empty() || id.len() > 64 {
            return None;
        }
        let expected = self.sign(id);
        let ok: bool = expected.as_bytes().ct_eq(sig.as_bytes()).into();
        ok.then(|| id.to_string())
    }
}

/// `Set-Cookie`-Wert für das Gerätecookie (1 Jahr).
pub fn device_cookie_header(value: &str) -> String {
    format!("{DEVICE_COOKIE}={value}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=31536000")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn backoff_doubles_and_expires() {
        let rl = RateLimiter::new([7u8; 32]);
        let key = "ip:203.0.113.9";
        for _ in 0..FREE_ATTEMPTS - 1 {
            rl.failure(key, true);
            assert!(rl.check(key, true).is_ok());
        }
        rl.failure(key, true);
        let wait = rl.check(key, true).unwrap_err();
        assert!(wait <= BASE_WAIT && wait > BASE_WAIT - Duration::from_secs(1));
        tokio::time::advance(BASE_WAIT).await;
        assert!(rl.check(key, true).is_ok());
        rl.failure(key, true);
        assert!(rl.check(key, true).unwrap_err() > BASE_WAIT, "verdoppelt");
        for _ in 0..20 {
            rl.failure(key, true);
        }
        assert!(
            rl.check(key, true).unwrap_err() <= MAX_WAIT,
            "höchstens 1 h"
        );
        tokio::time::advance(EXPIRY).await;
        assert!(rl.check(key, true).is_ok(), "verfällt nach 24 h");
        rl.clear(key);
        assert_eq!(
            rl.success_login(key),
            26,
            "Re-Auth/Einrichtung setzen nicht zurück"
        );
        assert_eq!(rl.success_login(key), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn global_budget_spares_device_clients() {
        let rl = RateLimiter::new([7u8; 32]);
        // viele Adressen, je ein Fehlversuch
        for i in 0..FREE_GLOBAL {
            let key = key_for(None, format!("198.51.100.{}", i % 250).parse().unwrap());
            rl.failure(&key, false);
        }
        let fresh = key_for(None, "203.0.113.200".parse().unwrap());
        assert!(
            rl.check(&fresh, false).is_err(),
            "globales Budget erschöpft"
        );
        assert!(
            rl.check("device:abc", true).is_ok(),
            "Gerätecookie nicht betroffen"
        );
    }

    #[test]
    fn ipv6_is_grouped_per_64() {
        let a = key_for(None, "2001:db8:1:2::1".parse().unwrap());
        let b = key_for(None, "2001:db8:1:2:ffff::9".parse().unwrap());
        let c = key_for(None, "2001:db8:1:3::1".parse().unwrap());
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(
            key_for(None, "203.0.113.9".parse().unwrap()),
            "ip:203.0.113.9"
        );
    }

    #[test]
    fn device_cookie_signature() {
        let rl = RateLimiter::new([1u8; 32]);
        let other = RateLimiter::new([2u8; 32]);
        let cookie = rl.new_device_cookie();
        assert!(rl.device_id(&cookie).is_some());
        assert!(other.device_id(&cookie).is_none(), "fremder Schlüssel");
        assert!(rl.device_id("abc.def").is_none());
        assert!(rl.device_id("ohne-punkt").is_none());
        assert!(device_cookie_header(&cookie).contains("Max-Age=31536000"));
    }

    #[test]
    fn key_is_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let a = RateLimiter::load_or_create(dir.path()).unwrap();
        let cookie = a.new_device_cookie();
        let b = RateLimiter::load_or_create(dir.path()).unwrap();
        assert!(
            b.device_id(&cookie).is_some(),
            "Neustart behält den Schlüssel"
        );
    }
}
