//! Browser-Sitzungen (Plan W5.4/W6).
//!
//! - Cookie `__Host-kb_session`: HttpOnly, Secure, SameSite=Strict, Path=/.
//! - ID: 256 bit aus dem OS-Zufall, serverseitig nur als SHA-256 gespeichert;
//!   bei jedem Login neu.
//! - Idle-Timeout [`IDLE_TIMEOUT`], absolut [`ABSOLUTE_TIMEOUT`] (Plan §7);
//!   abgelaufene Sitzungen werden bei der Prüfung entfernt.
//! - Re-Auth gilt [`REAUTH_WINDOW`] (Plan W6).
//! - Eine Anmeldung (Einrichtung, Login) gilt [`REAUTH_WINDOW`] lang als
//!   frisch für Re-Auth-Prüfungen, die das ausdrücklich zulassen
//!   ([`SessionStore::login_fresh`], W14-038, Lauf F4a).
//! - Jede Anmeldung trägt eine Folgenummer, die eine Rotation durch Re-Auth
//!   übersteht ([`SessionStore::login_seq`]); an sie sind Idempotenz-Schlüssel
//!   gebunden (W14-033).
//! - Übersicht und „andere abmelden“ ([`SessionStore::overview`],
//!   [`SessionStore::retain_only`], W14-005).
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

impl SessionId {
    /// Gekürzte Kennung für die Anzeige (die ersten 4 Byte des SHA-256 des
    /// Cookie-Werts als Hex; erlaubt keinen Rückschluss auf das Cookie).
    pub fn short(&self) -> String {
        self.0[..4].iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Eintrag der Sitzungsübersicht (W14-005). Zeitangaben relativ zu jetzt
/// (die Antwort rechnet sie mit der Uhr des Servers in Zeitpunkte um).
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub short_id: String,
    pub since_login: Duration,
    pub since_seen: Duration,
    pub current: bool,
    pub reauth_active: bool,
}

struct SessionData {
    /// Anlage-Reihenfolge (für das Verdrängen der ältesten Sitzung); bleibt
    /// bei einer Rotation durch Re-Auth gleich (Folgenummer der Anmeldung).
    seq: u64,
    created: Instant,
    last_seen: Instant,
    reauth_until: Option<Instant>,
    heavy_calls: VecDeque<Instant>,
    /// Nach einer Rotation (Re-Auth) gilt die alte ID nur noch kurz.
    retire_at: Option<Instant>,
}

impl SessionData {
    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.last_seen) >= IDLE_TIMEOUT
            || now.duration_since(self.created) >= ABSOLUTE_TIMEOUT
            || self.retire_at.is_some_and(|at| now >= at)
    }
}

/// Wie lange eine durch Re-Auth ersetzte Sitzungs-ID noch gilt (laufende
/// Anfragen des eigenen Browsers), ohne Re-Auth-Recht.
pub const ROTATION_GRACE: Duration = Duration::from_secs(10);

fn new_token() -> String {
    let mut raw = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
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
        let token = new_token();
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
                retire_at: None,
            },
        );
        token
    }

    /// Re-Auth bestätigt (W14): neue Sitzungs-ID mit frischer Re-Auth
    /// ([`REAUTH_WINDOW`]); die alte verliert das Re-Auth-Recht und gilt nur
    /// noch [`ROTATION_GRACE`]. Ein gestohlenes Cookie endet so mit der
    /// Bestätigung des Inhabers. Liefert den neuen Cookie-Wert.
    pub fn rotate_with_reauth(&self, old: SessionId) -> Option<String> {
        let now = Instant::now();
        let token = new_token();
        let mut map = self.lock();
        let data = map.get_mut(&old)?;
        if data.expired(now) || data.retire_at.is_some() {
            return None;
        }
        let rotated = SessionData {
            seq: data.seq,
            created: data.created,
            last_seen: now,
            reauth_until: Some(now + REAUTH_WINDOW),
            heavy_calls: data.heavy_calls.clone(),
            retire_at: None,
        };
        data.reauth_until = None;
        data.retire_at = Some(now + ROTATION_GRACE);
        map.insert(hash_token(&token), rotated);
        Some(token)
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

    /// Alle außer `keep` ungültig (Passphrase geändert, „andere abmelden“);
    /// liefert die Zahl der beendeten fremden Anmeldungen (ohne abgelaufene
    /// und ohne die durch Re-Auth ersetzten IDs der eigenen Anmeldung).
    pub fn retain_only(&self, keep: SessionId) -> usize {
        let now = Instant::now();
        let mut map = self.lock();
        let own_seq = map.get(&keep).map(|d| d.seq);
        let ended = map
            .iter()
            .filter(|(id, d)| {
                **id != keep && !d.expired(now) && d.retire_at.is_none() && Some(d.seq) != own_seq
            })
            .count();
        map.retain(|id, _| *id == keep);
        ended
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

    /// Folgenummer der Anmeldung (übersteht die Rotation durch Re-Auth);
    /// Idempotenz-Schlüssel gelten nur innerhalb einer Anmeldung (W14-033).
    pub fn login_seq(&self, id: SessionId) -> Option<u64> {
        self.lock().get(&id).map(|d| d.seq)
    }

    /// Liegt die Anmeldung (Einrichtung bzw. Login mit Passphrase und
    /// zweitem Faktor) dieser Sitzung weniger als [`REAUTH_WINDOW`] zurück?
    /// Gilt nur für eine noch nicht durch Re-Auth ersetzte ID (W14-038:
    /// bedingte Re-Auth direkt nach der Anmeldung).
    pub fn login_fresh(&self, id: SessionId) -> bool {
        let now = Instant::now();
        self.lock().get(&id).is_some_and(|d| {
            d.retire_at.is_none()
                && !d.expired(now)
                && now.duration_since(d.created) < REAUTH_WINDOW
        })
    }

    /// Übersicht der gültigen Sitzungen (W14-005), älteste Anmeldung zuerst.
    /// Durch Re-Auth ersetzte IDs (laufen in [`ROTATION_GRACE`] ab) fehlen.
    pub fn overview(&self, current: SessionId) -> Vec<SessionSummary> {
        let now = Instant::now();
        let map = self.lock();
        let mut out: Vec<(u64, SessionSummary)> = map
            .iter()
            .filter(|(_, d)| !d.expired(now) && d.retire_at.is_none())
            .map(|(id, d)| {
                (
                    d.seq,
                    SessionSummary {
                        short_id: id.short(),
                        since_login: now.duration_since(d.created),
                        since_seen: now.duration_since(d.last_seen),
                        current: *id == current,
                        reauth_active: d.reauth_until.is_some_and(|u| now < u),
                    },
                )
            })
            .collect();
        out.sort_by_key(|(seq, _)| *seq);
        out.into_iter().map(|(_, s)| s).collect()
    }

    /// Verschiebt alle Zeitpunkte der Sitzungen um `by` in die Vergangenheit
    /// (nur Tests: Frist nach Anmeldung bzw. Re-Auth ablaufen lassen, ohne die
    /// Tokio-Uhr anzuhalten).
    #[doc(hidden)]
    pub fn age_for_tests(&self, by: Duration) {
        let mut map = self.lock();
        for d in map.values_mut() {
            d.created = d.created.checked_sub(by).unwrap_or(d.created);
            d.last_seen = d.last_seen.checked_sub(by).unwrap_or(d.last_seen);
            d.reauth_until = d.reauth_until.and_then(|u| u.checked_sub(by));
        }
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

/// Gekürzte Kennung (wie [`SessionId::short`]) des Sitzungs-Cookies einer
/// Anfrage — ohne Prüfung, ob die Sitzung gilt, und ohne die Idle-Frist zu
/// verlängern. Für das Zugriffsprotokoll (W14-018).
pub fn short_id_from_headers(headers: &HeaderMap) -> Option<String> {
    token_from_headers(headers).map(|t| hash_token(&t).short())
}

/// Cookie-Wert der Sitzung.
pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    cookie_from_headers(headers, COOKIE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lauf F6b (LE-HASH-CRATES): Sitzungs-IDs sind der SHA-256 des Tokens.
    #[test]
    fn hash_token_known_answer() {
        let SessionId(h) = hash_token("beispiel-token");
        let hex: String = h.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "42a2c3f9b5df85389230def351f7c7ace46a0439d2641bdc462748d0bbeddb40"
        );
    }
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

    #[tokio::test(start_paused = true)]
    async fn reauth_rotates_the_session_id() {
        let store = SessionStore::default();
        let old = store.create();
        let old_id = store.validate(&old).unwrap();
        let new = store.rotate_with_reauth(old_id).unwrap();
        let new_id = store.validate(&new).unwrap();
        assert_ne!(old, new);
        assert!(store.reauth_fresh(new_id), "neue ID trägt die Re-Auth");
        assert!(!store.reauth_fresh(old_id), "alte ID ohne Re-Auth");
        assert!(store.validate(&old).is_some(), "alte ID gilt noch kurz");
        assert!(
            store.rotate_with_reauth(old_id).is_none(),
            "eine ersetzte ID lässt sich nicht erneut rotieren"
        );
        tokio::time::advance(ROTATION_GRACE).await;
        assert!(
            store.validate(&old).is_none(),
            "alte ID nach der Frist ungültig"
        );
        assert!(store.validate(&new).is_some());
    }

    /// W14-033/W14-038 (Lauf F4a): Folgenummer übersteht die Rotation, die
    /// Frische der Anmeldung endet nach dem Re-Auth-Fenster und gilt nicht für
    /// eine ersetzte ID.
    #[tokio::test(start_paused = true)]
    async fn login_seq_survives_rotation_and_login_freshness_expires() {
        let store = SessionStore::default();
        let a = store.create();
        let b = store.create();
        let a_id = store.validate(&a).unwrap();
        let b_id = store.validate(&b).unwrap();
        assert_ne!(store.login_seq(a_id), store.login_seq(b_id));
        assert!(store.login_fresh(a_id), "direkt nach der Anmeldung frisch");
        let rotated = store.rotate_with_reauth(a_id).unwrap();
        let rotated_id = store.validate(&rotated).unwrap();
        assert_eq!(store.login_seq(rotated_id), store.login_seq(a_id));
        assert!(
            !store.login_fresh(a_id),
            "eine ersetzte ID gilt nicht als frisch angemeldet"
        );
        tokio::time::advance(REAUTH_WINDOW).await;
        assert!(store.validate(&b).is_some());
        assert!(!store.login_fresh(b_id), "nach 5 min nicht mehr frisch");
        assert!(store.login_seq(b_id).is_some());
    }

    /// W14-005 (Lauf F4a): Übersicht ohne ersetzte IDs, „andere abmelden“
    /// zählt nur fremde Anmeldungen.
    #[tokio::test(start_paused = true)]
    async fn overview_lists_valid_sessions_and_retain_only_counts() {
        let store = SessionStore::default();
        let a = store.create();
        tokio::time::advance(Duration::from_secs(60)).await;
        let b = store.create();
        let c = store.create();
        let a_id = store.validate(&a).unwrap();
        let b_id = store.validate(&b).unwrap();
        store.validate(&c).unwrap();
        let b2 = store.rotate_with_reauth(b_id).unwrap();
        let b2_id = store.validate(&b2).unwrap();
        let list = store.overview(b2_id);
        assert_eq!(list.len(), 3, "a, b (rotiert), c");
        assert!(
            list[0].since_login >= Duration::from_secs(60),
            "älteste zuerst"
        );
        assert_eq!(list[0].short_id, a_id.short());
        assert_eq!(a_id.short().len(), 8);
        assert_eq!(list.iter().filter(|s| s.current).count(), 1);
        assert!(list.iter().any(|s| s.current && s.reauth_active));
        assert_eq!(store.retain_only(b2_id), 2, "a und c beendet");
        assert!(store.validate(&a).is_none());
        assert!(store.validate(&c).is_none());
        assert!(
            store.validate(&b).is_none(),
            "die ersetzte ID der eigenen Anmeldung endet mit"
        );
        assert!(store.validate(&b2).is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn age_for_tests_moves_login_and_reauth_into_the_past() {
        let store = SessionStore::default();
        let id = store.validate(&store.create()).unwrap();
        store.mark_reauth(id);
        assert!(store.login_fresh(id) && store.reauth_fresh(id));
        tokio::time::advance(Duration::from_secs(1)).await;
        store.age_for_tests(REAUTH_WINDOW);
        assert!(!store.login_fresh(id));
        assert!(!store.reauth_fresh(id));
    }
}
