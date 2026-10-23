//! Browser-Sitzungen (Plan W5.4/W6).
//!
//! - Cookie `__Host-kb_session`: HttpOnly, Secure, SameSite=Strict, Path=/.
//! - ID: 256 bit aus dem OS-Zufall, serverseitig nur als SHA-256 gespeichert;
//!   bei jedem Login neu.
//! - Idle-Timeout [`IDLE_TIMEOUT`], absolut [`ABSOLUTE_TIMEOUT`] (Plan §7);
//!   abgelaufene Sitzungen werden bei der Prüfung entfernt.
//! - Re-Auth gilt [`REAUTH_WINDOW`] (Plan W6).
//! - Nur im RAM: ein Neustart meldet alle Sitzungen ab (die DB ist danach
//!   ohnehin gesperrt, E3). Die Zeit kommt aus `tokio::time` (in Tests
//!   pausierbar).

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use axum::http::{header, HeaderMap};
use base64::Engine as _;
use rand::RngCore as _;
use sha2::{Digest, Sha256};
use tokio::time::Instant;

pub const COOKIE_NAME: &str = "__Host-kb_session";
/// Höchstzahl gleichzeitiger Sitzungen (ein Nutzer, mehrere Geräte); die
/// älteste fällt heraus.
pub const MAX_SESSIONS: usize = 32;
/// Render-/JVM-Aufrufe je Sitzung und Minute (Plan W5.8).
pub const HEAVY_CALLS_PER_MINUTE: usize = 30;
/// Sitzung endet nach so langer Untätigkeit (Plan §7).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Sitzung endet spätestens nach dieser Zeit ab Anmeldung (Plan §7).
pub const ABSOLUTE_TIMEOUT: Duration = Duration::from_secs(12 * 3600);
/// So lange gilt eine Re-Auth (Plan §7).
pub const REAUTH_WINDOW: Duration = Duration::from_secs(5 * 60);

/// SHA-256 des Cookie-Werts (nie der Wert selbst).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId([u8; 32]);

impl std::fmt::Debug for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionId(…)")
    }
}

struct SessionData {
    /// Anlage-Reihenfolge (für das Verdrängen der ältesten Sitzung).
    seq: u64,
    created: Instant,
    last_seen: Instant,
    reauth_until: Option<Instant>,
    heavy_calls: VecDeque<Instant>,
}

impl SessionData {
    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.last_seen) >= IDLE_TIMEOUT
            || now.duration_since(self.created) >= ABSOLUTE_TIMEOUT
    }
}

#[derive(Default)]
pub struct SessionStore {
    inner: Mutex<HashMap<SessionId, SessionData>>,
    next_seq: std::sync::atomic::AtomicU64,
}

fn hash_token(token: &str) -> SessionId {
    SessionId(Sha256::digest(token.as_bytes()).into())
}

impl SessionStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, SessionData>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Neue Sitzung; liefert den Cookie-Wert.
    pub fn create(&self) -> String {
        let mut raw = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
        let now = Instant::now();
        let seq = self
            .next_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut map = self.lock();
        map.retain(|_, d| !d.expired(now));
        while map.len() >= MAX_SESSIONS {
            let oldest = map.iter().min_by_key(|(_, d)| d.seq).map(|(k, _)| *k);
            match oldest {
                Some(k) => {
                    map.remove(&k);
                }
                None => break,
            }
        }
        map.insert(
            hash_token(&token),
            SessionData {
                seq,
                created: now,
                last_seen: now,
                reauth_until: None,
                heavy_calls: VecDeque::new(),
            },
        );
        token
    }

    /// Gültige Sitzung zum Cookie-Wert (Idle- und Absolut-Timeout geprüft;
    /// abgelaufene werden entfernt). Verlängert die Idle-Frist.
    pub fn validate(&self, token: &str) -> Option<SessionId> {
        let id = hash_token(token);
        let now = Instant::now();
        let mut map = self.lock();
        let expired = map.get(&id)?.expired(now);
        if expired {
            map.remove(&id);
            return None;
        }
        if let Some(data) = map.get_mut(&id) {
            data.last_seen = now;
        }
        Some(id)
    }

    /// Gültige Sitzung aus dem `Cookie`-Header.
    pub fn from_headers(&self, headers: &HeaderMap) -> Option<SessionId> {
        token_from_headers(headers).and_then(|t| self.validate(&t))
    }

    /// Meldet eine Sitzung ab (Logout).
    pub fn remove(&self, id: SessionId) {
        self.lock().remove(&id);
    }

    /// Alle Sitzungen ungültig („Datenbank sperren“, TOTP-Reset).
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Alle außer `keep` ungültig (Passphrase geändert).
    pub fn retain_only(&self, keep: SessionId) {
        self.lock().retain(|id, _| *id == keep);
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Re-Auth bestätigt: gilt [`REAUTH_WINDOW`].
    pub fn mark_reauth(&self, id: SessionId) {
        if let Some(d) = self.lock().get_mut(&id) {
            d.reauth_until = Some(Instant::now() + REAUTH_WINDOW);
        }
    }

    /// Liegt eine frische Re-Auth vor?
    pub fn reauth_fresh(&self, id: SessionId) -> bool {
        let now = Instant::now();
        self.lock()
            .get(&id)
            .and_then(|d| d.reauth_until)
            .is_some_and(|until| now < until)
    }

    /// Zählt einen Render-/JVM-Aufruf; über dem Limit die Wartezeit bis zum
    /// nächsten freien Platz.
    pub fn allow_heavy(&self, id: SessionId) -> Result<(), Duration> {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let mut map = self.lock();
        let Some(data) = map.get_mut(&id) else {
            return Ok(());
        };
        while data
            .heavy_calls
            .front()
            .is_some_and(|t| now.duration_since(*t) >= window)
        {
            data.heavy_calls.pop_front();
        }
        if data.heavy_calls.len() >= HEAVY_CALLS_PER_MINUTE {
            let oldest = data.heavy_calls.front().copied().unwrap_or(now);
            return Err(window.saturating_sub(now.duration_since(oldest)));
        }
        data.heavy_calls.push_back(now);
        Ok(())
    }
}

/// `Set-Cookie`-Wert für eine neue Sitzung.
pub fn set_cookie_value(token: &str) -> String {
    format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; Secure; SameSite=Strict")
}

/// `Set-Cookie`-Wert, der die Sitzung im Browser löscht (Logout).
pub fn clear_cookie_value() -> String {
    format!("{COOKIE_NAME}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}

/// Wert eines Cookies aus allen `Cookie`-Headern.
pub fn cookie_from_headers(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(n, _)| *n == name)
        .map(|(_, value)| value.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Cookie-Wert der Sitzung.
pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    cookie_from_headers(headers, COOKIE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn create_validate_and_cookie_parsing() {
        let store = SessionStore::default();
        let token = store.create();
        assert_eq!(token.len(), 43, "256 bit base64url ohne Padding");
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("andere=1; {COOKIE_NAME}={token}; x=y")).unwrap(),
        );
        assert!(store.from_headers(&headers).is_some());
        assert!(store.validate("falsch").is_none());
        let cookie = set_cookie_value(&token);
        for attr in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/"] {
            assert!(cookie.contains(attr), "{cookie}");
        }
        assert!(cookie.starts_with("__Host-kb_session="));
        store.clear();
        assert!(store.from_headers(&headers).is_none());
    }

    #[test]
    fn session_count_is_bounded() {
        let store = SessionStore::default();
        let first = store.create();
        for _ in 0..MAX_SESSIONS {
            store.create();
        }
        assert_eq!(store.len(), MAX_SESSIONS);
        assert!(store.validate(&first).is_none(), "älteste fällt heraus");
    }

    #[tokio::test(start_paused = true)]
    async fn heavy_calls_are_limited_per_minute() {
        let store = SessionStore::default();
        let id = store.validate(&store.create()).unwrap();
        for _ in 0..HEAVY_CALLS_PER_MINUTE {
            assert!(store.allow_heavy(id).is_ok());
        }
        let wait = store.allow_heavy(id).unwrap_err();
        assert!(wait <= Duration::from_secs(60));
        tokio::time::advance(Duration::from_secs(61)).await;
        assert!(store.allow_heavy(id).is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn idle_and_absolute_timeout() {
        let store = SessionStore::default();
        let token = store.create();
        tokio::time::advance(IDLE_TIMEOUT - Duration::from_secs(1)).await;
        assert!(
            store.validate(&token).is_some(),
            "kurz vor Idle-Ende gültig"
        );
        tokio::time::advance(IDLE_TIMEOUT - Duration::from_secs(1)).await;
        assert!(store.validate(&token).is_some(), "Nutzung verlängert");
        tokio::time::advance(IDLE_TIMEOUT).await;
        assert!(store.validate(&token).is_none(), "Idle abgelaufen");

        // Absolut: trotz ständiger Nutzung nach 12 h vorbei.
        let token = store.create();
        let step = Duration::from_secs(20 * 60);
        let mut elapsed = Duration::ZERO;
        while elapsed + step < ABSOLUTE_TIMEOUT {
            tokio::time::advance(step).await;
            elapsed += step;
            assert!(store.validate(&token).is_some(), "nach {elapsed:?}");
        }
        tokio::time::advance(ABSOLUTE_TIMEOUT - elapsed).await;
        assert!(store.validate(&token).is_none(), "absolut abgelaufen");
    }

    #[tokio::test(start_paused = true)]
    async fn reauth_window_and_logout() {
        let store = SessionStore::default();
        let token = store.create();
        let id = store.validate(&token).unwrap();
        assert!(!store.reauth_fresh(id));
        store.mark_reauth(id);
        assert!(store.reauth_fresh(id));
        tokio::time::advance(REAUTH_WINDOW).await;
        assert!(!store.reauth_fresh(id), "nach 5 min abgelaufen");
        let other = store.create();
        store.retain_only(id);
        assert!(store.validate(&other).is_none());
        store.remove(id);
        assert!(store.validate(&token).is_none());
    }
}
