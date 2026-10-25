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
//! - Obergrenze in Folge (W14-004, NIST SP 800-63B §5.2.2, Lauf F4a): Nach
//!   [`TOTP_LOCK_AFTER`] Fehlversuchen ohne Gerätecookie (Anmeldung und Re-Auth
//!   mit TOTP) ohne zwischenzeitliche erfolgreiche Anmeldung ist die Anmeldung
//!   mit TOTP ohne Gerätecookie gesperrt — unabhängig von der Zeit, bis eine
//!   Anmeldung von einem bekannten Gerät (Gerätecookie) oder mit Einmal-Code
//!   gelingt bzw. die Einrichtung (`totp-reset`) abgeschlossen ist. Der Zähler
//!   liegt in `/data/auth/totp-lock.json` (übersteht Neustarts).

use std::collections::HashMap;
use std::io::Write as _;
use std::net::IpAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use rand::RngCore as _;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::time::Instant;

use super::errors::ApiError;
use super::policy::ClientInfo;
use super::state::ServerState;
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
/// Fehlversuche in Folge ohne Gerätecookie, nach denen die Anmeldung mit TOTP
/// ohne Gerätecookie gesperrt ist (W14-004, NIST SP 800-63B §5.2.2: höchstens
/// 100 aufeinanderfolgende Fehlversuche je Konto).
pub const TOTP_LOCK_AFTER: u32 = 100;
/// Zähler der Fehlversuche in Folge (`/data/auth/totp-lock.json`, 0600).
pub const TOTP_LOCK_FILE: &str = "totp-lock.json";
/// Falsche Passphrase nach **gültigem** zweitem Faktor in Folge, ab der ein
/// Hinweis „App-Code womöglich bekannt“ entsteht (Lauf F4d). Anders als die
/// Obergrenze [`TOTP_LOCK_AFTER`] (auch ohne jede Kenntnis auslösbar) setzt
/// dieser Fall einen gültigen App-Code bzw. Einmal-Code voraus. Drei, damit
/// ein einzelner Tippfehler des Inhabers keinen Alarm auslöst.
pub const SUSPECT_FACTOR_AFTER: u32 = 3;
const MAX_ENTRIES: usize = 10_000;

struct Entry {
    failures: u32,
    last_failure: Instant,
    blocked_until: Option<Instant>,
}

/// Ergebnis von [`RateLimiter::streak_failure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Streak {
    /// Weiter unter der Grenze.
    Counted(u32),
    /// Dieser Fehlversuch hat die Grenze erreicht: ab jetzt gesperrt.
    Tripped,
    /// Schon gesperrt (ein Versuch, der vor dem Erreichen der Grenze
    /// angenommen wurde).
    AlreadyLocked,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct StreakFile {
    failures: u32,
}

pub struct RateLimiter {
    entries: Mutex<HashMap<String, Entry>>,
    failed_since_success: AtomicU64,
    device_key: [u8; 32],
    /// Fehlversuche in Folge ohne Gerätecookie (W14-004).
    streak: Mutex<u32>,
    /// Ablage des Zählers; `None` in Unit-Tests ohne Datenverzeichnis.
    streak_file: Option<std::path::PathBuf>,
    /// Falsche Passphrase nach gültigem zweitem Faktor in Folge (Lauf F4d;
    /// nur im Speicher, eine erfolgreiche Anmeldung bzw. Re-Auth setzt ihn
    /// zurück): (Anzahl, Kennung der Folge). Die Kennung ist eine UUIDv7 je
    /// Folge (F5-fix): ein Zeitstempel in Sekunden kollidierte, wenn zwei
    /// Folgen — etwa vor und nach einem Neustart — in derselben Sekunde
    /// begannen, und der zweite Hinweis fiel dem `dedup_key` zum Opfer.
    factor_suspect: Mutex<(u32, String)>,
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
            streak: Mutex::new(0),
            streak_file: None,
            factor_suspect: Mutex::new((0, String::new())),
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
        let streak_file = dir.join(TOTP_LOCK_FILE);
        // Unlesbar oder kaputt: wie gesperrt behandeln wäre eine Aussperrung
        // ohne Anlass; der Zähler beginnt neu (Log-WARN).
        let failures = match std::fs::read(&streak_file) {
            Ok(bytes) => match serde_json::from_slice::<StreakFile>(&bytes) {
                Ok(f) => f.failures,
                Err(e) => {
                    tracing::warn!("{TOTP_LOCK_FILE} unlesbar ({e}) — Zähler beginnt neu");
                    0
                }
            },
            Err(_) => 0,
        };
        let mut limiter = Self::new(key);
        limiter.streak = Mutex::new(failures);
        limiter.streak_file = Some(streak_file);
        Ok(limiter)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn streak_guard(&self) -> std::sync::MutexGuard<'_, u32> {
        self.streak.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Schreibt den Zähler (Temp-Datei + rename, 0600). Ein Fehler wird
    /// geloggt; der Zähler im RAM gilt weiter.
    fn persist_streak(&self, failures: u32) {
        let Some(path) = self.streak_file.as_deref() else {
            return;
        };
        let write = || -> std::io::Result<()> {
            let dir = path.parent().unwrap_or(Path::new("."));
            let tmp = dir.join(format!(
                ".{TOTP_LOCK_FILE}.{}.tmp",
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
            f.write_all(&serde_json::to_vec(&StreakFile { failures }).unwrap_or_default())?;
            f.sync_all()?;
            std::fs::rename(&tmp, path)
        };
        if let Err(e) = write() {
            tracing::error!("{TOTP_LOCK_FILE} nicht geschrieben: {e}");
        }
    }

    /// Ist die Anmeldung mit TOTP ohne Gerätecookie gesperrt (W14-004)?
    pub fn totp_locked(&self) -> bool {
        *self.streak_guard() >= TOTP_LOCK_AFTER
    }

    /// Fehlversuche in Folge ohne Gerätecookie.
    pub fn streak(&self) -> u32 {
        *self.streak_guard()
    }

    /// Zählt einen Fehlversuch ohne Gerätecookie (Anmeldung bzw. Re-Auth mit
    /// TOTP; nicht Einmal-Codes, nicht die Einrichtung).
    pub fn streak_failure(&self) -> Streak {
        let mut g = self.streak_guard();
        if *g >= TOTP_LOCK_AFTER {
            return Streak::AlreadyLocked;
        }
        *g += 1;
        let now = *g;
        drop(g);
        self.persist_streak(now);
        if now >= TOTP_LOCK_AFTER {
            Streak::Tripped
        } else {
            Streak::Counted(now)
        }
    }

    /// Erfolgreiche Anmeldung bzw. Einrichtung: Zähler auf 0; liefert den
    /// vorigen Stand (≥ [`TOTP_LOCK_AFTER`] = war gesperrt).
    pub fn streak_success(&self) -> u32 {
        let mut g = self.streak_guard();
        let before = *g;
        *g = 0;
        drop(g);
        if before > 0 {
            self.persist_streak(0);
        }
        before
    }

    /// Zählt eine falsche Passphrase nach gültigem zweitem Faktor (Lauf F4d);
    /// liefert den neuen Stand und die Kennung der Folge (eindeutig je Folge,
    /// für einen Hinweis genau einmal je Folge).
    pub fn factor_suspect_failure(&self) -> (u32, String) {
        let mut g = self
            .factor_suspect
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if g.0 == 0 {
            g.1 = uuid::Uuid::now_v7().to_string();
        }
        g.0 = g.0.saturating_add(1);
        g.clone()
    }

    /// Erfolgreiche Anmeldung bzw. Re-Auth: beendet die Folge der falschen
    /// Passphrasen nach gültigem zweitem Faktor; liefert den vorigen Stand.
    pub fn factor_suspect_clear(&self) -> (u32, String) {
        std::mem::take(
            &mut *self
                .factor_suspect
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Setzt den Zähler (nur Tests: Sperre ohne 100 echte Fehlversuche).
    #[doc(hidden)]
    pub fn set_streak_for_tests(&self, failures: u32) {
        *self.streak_guard() = failures;
        self.persist_streak(failures);
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

// ---------------------------------------------------------------------------
// Öffentliche Lese-Routen (Plan W12)
// ---------------------------------------------------------------------------

/// Anfragen je [`PUBLIC_WINDOW`] und Schlüssel auf `GET /api/public/info` und
/// `/legal/` (ohne Sitzung; ADR 0039 „mit Rate-Limit“).
pub const PUBLIC_PER_WINDOW: u32 = 120;
pub const PUBLIC_WINDOW: Duration = Duration::from_secs(60);
/// Sammelschlüssel, wenn die Tabelle voll ist (begrenzter Speicher).
const OVERFLOW_KEY: &str = "overflow";

/// Festes Zeitfenster je Client-IP (IPv6 je /64, wie [`key_for`]); zählt jede
/// Anfrage, nicht nur Fehlversuche.
#[derive(Default)]
pub struct PublicRateLimiter {
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl PublicRateLimiter {
    /// Zählt eine Anfrage; über dem Limit die Wartezeit bis zum neuen Fenster.
    pub fn hit(&self, key: &str) -> std::result::Result<(), Duration> {
        let now = Instant::now();
        let mut map = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        let mut key = key;
        if map.len() >= MAX_ENTRIES && !map.contains_key(key) {
            map.retain(|_, (start, _)| now.duration_since(*start) < PUBLIC_WINDOW);
            if map.len() >= MAX_ENTRIES {
                key = OVERFLOW_KEY;
            }
        }
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= PUBLIC_WINDOW {
            *entry = (now, 0);
        }
        if entry.1 >= PUBLIC_PER_WINDOW {
            return Err(PUBLIC_WINDOW.saturating_sub(now.duration_since(entry.0)));
        }
        entry.1 += 1;
        Ok(())
    }
}

/// Middleware der öffentlichen Lese-Routen: 429 + `Retry-After` über dem
/// Limit. Die Client-IP kommt aus der Policy-Middleware (vertraute Proxys).
pub async fn public_limit(
    State(st): State<Arc<ServerState>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = req
        .extensions()
        .get::<ClientInfo>()
        .map(|c| c.ip)
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    match st.public_rate.hit(&key_for(None, ip)) {
        Ok(()) => next.run(req).await,
        Err(wait) => {
            let secs = wait.as_secs().max(1);
            ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                format!("Zu viele Anfragen — bitte {secs} s warten."),
            )
            .with_retry_after(secs)
            .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn public_window_per_key() {
        let rl = PublicRateLimiter::default();
        let key = "ip:203.0.113.9";
        for _ in 0..PUBLIC_PER_WINDOW {
            assert!(rl.hit(key).is_ok());
        }
        let wait = rl.hit(key).unwrap_err();
        assert!(wait <= PUBLIC_WINDOW && wait > Duration::ZERO);
        assert!(rl.hit("ip:198.51.100.7").is_ok(), "andere IP frei");
        tokio::time::advance(PUBLIC_WINDOW).await;
        assert!(rl.hit(key).is_ok(), "neues Fenster");
    }

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

    /// W14-004 (Lauf F4a): Sperre nach 100 Fehlversuchen in Folge,
    /// Erfolg setzt zurück und meldet den vorigen Stand.
    #[test]
    fn streak_locks_after_limit_and_success_resets() {
        let rl = RateLimiter::new([3u8; 32]);
        for i in 1..TOTP_LOCK_AFTER {
            assert_eq!(rl.streak_failure(), Streak::Counted(i));
        }
        assert!(!rl.totp_locked(), "99 Fehlversuche: noch frei");
        assert_eq!(rl.streak_failure(), Streak::Tripped);
        assert!(rl.totp_locked());
        assert_eq!(rl.streak_failure(), Streak::AlreadyLocked);
        assert_eq!(rl.streak(), TOTP_LOCK_AFTER, "zählt gesperrt nicht weiter");
        assert_eq!(rl.streak_success(), TOTP_LOCK_AFTER);
        assert!(!rl.totp_locked());
        assert_eq!(rl.streak_success(), 0);
        // Erfolg zwischendurch: wieder von vorn.
        for _ in 0..50 {
            rl.streak_failure();
        }
        rl.streak_success();
        assert_eq!(rl.streak_failure(), Streak::Counted(1));
    }

    /// Der Zähler übersteht einen Neustart (Datei 0600); eine kaputte Datei
    /// sperrt nicht aus.
    #[test]
    fn streak_is_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let a = RateLimiter::load_or_create(dir.path()).unwrap();
        a.set_streak_for_tests(TOTP_LOCK_AFTER - 1);
        assert_eq!(a.streak_failure(), Streak::Tripped);
        let b = RateLimiter::load_or_create(dir.path()).unwrap();
        assert!(b.totp_locked(), "Neustart hebt die Sperre nicht auf");
        let path = dir
            .path()
            .join(super::super::totp::AUTH_DIR)
            .join(TOTP_LOCK_FILE);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        b.streak_success();
        assert!(!RateLimiter::load_or_create(dir.path())
            .unwrap()
            .totp_locked());
        std::fs::write(&path, b"kaputt").unwrap();
        assert_eq!(RateLimiter::load_or_create(dir.path()).unwrap().streak(), 0);
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

    /// F5-fix (CI 40184691919): Jede Folge falscher Passphrasen nach gültigem
    /// zweitem Faktor hat eine eigene Kennung — auch zwei Folgen im selben
    /// Augenblick und über einen Neustart (neuer Limiter) hinweg. Innerhalb
    /// einer Folge bleibt sie gleich (ein Hinweis je Folge).
    #[test]
    fn factor_suspect_streak_ids_are_unique_per_streak() {
        let rl = RateLimiter::new([7u8; 32]);
        let (n1, first) = rl.factor_suspect_failure();
        let (n2, same) = rl.factor_suspect_failure();
        assert_eq!((n1, n2), (1, 2));
        assert_eq!(first, same);
        assert_eq!(rl.factor_suspect_clear(), (2, first.clone()));
        let (_, second) = rl.factor_suspect_failure();
        assert_ne!(second, first);
        let restarted = RateLimiter::new([7u8; 32]);
        let (_, third) = restarted.factor_suspect_failure();
        assert_ne!(third, first);
        assert_ne!(third, second);
    }
}
