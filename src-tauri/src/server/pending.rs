//! Anmeldung bzw. Einrichtung über die Antwortzeit hinaus (W14-037, Lauf F4a).
//!
//! Das erste Entsperren nach einem Update kann vor der Umstellung der
//! Datenbank ein Pflicht-Backup schreiben und prüfen (ADR 0039 E15); das
//! dauert bei großen Archiven länger als die 30-s-Grenze der API bzw. das
//! Zeitlimit eines Reverse-Proxys. Statt 503 „timeout“ (die Arbeit lief
//! weiter, die Sitzung ging verloren) gilt:
//!
//! - Ist der Ablauf nach [`LOGIN_SYNC_WAIT`] fertig, antwortet die Route wie
//!   bisher (200 mit Sitzung bzw. Fehler).
//! - Sonst `202 {"state":"pending"}` mit dem Cookie [`TICKET_COOKIE`]
//!   (256 bit Zufall, nur als SHA-256 gespeichert, HttpOnly/Secure/
//!   SameSite=Strict, [`TICKET_MAX_AGE`]). Der Ablauf läuft als eigener Task zu
//!   Ende und hinterlegt sein Ergebnis.
//! - `POST /api/auth/login/poll` mit dem Cookie liefert `202` solange er
//!   läuft, danach genau einmal das Ergebnis (200 mit neuer Sitzung bzw. den
//!   Fehler) und löscht das Cookie; unbekannt oder abgelaufen → 410
//!   `login_expired`.
//!
//! Die Sitzung entsteht erst beim Abholen (kein Sitzungs-Token im Speicher
//! dieses Moduls). „Datenbank sperren“ verwirft offene Ergebnisse.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use base64::Engine as _;
use rand::RngCore as _;
use sha2::{Digest, Sha256};
use tokio::time::Instant;

use super::errors::ApiError;

/// So lange wartet die Anmelde-Route auf das Ergebnis, bevor sie mit 202
/// antwortet (5 s unter der 30-s-Grenze der API; gängige Proxys warten
/// 60 s und länger).
pub const LOGIN_SYNC_WAIT: Duration = Duration::from_secs(25);
/// Name des Cookies mit dem Abhol-Ticket.
pub const TICKET_COOKIE: &str = "__Host-kb_login";
/// Laufzeit des Tickets im Browser (Sekunden).
pub const TICKET_MAX_AGE: u64 = 3600;
/// So lange bleibt ein fertiges Ergebnis abholbar.
pub const RESULT_TTL: Duration = Duration::from_secs(15 * 60);
/// Höchstens so viele offene Abläufe (Anmeldung, Einrichtung) gleichzeitig.
pub const MAX_PENDING: usize = 8;

/// Erfolg einer Anmeldung bzw. Einrichtung vor dem Ausstellen der Sitzung:
/// Antwortkörper und vorhandenes Gerätecookie (zum gleitenden Erneuern).
#[derive(Debug)]
pub struct AuthSuccess {
    pub body: serde_json::Value,
    pub device: Option<String>,
}

/// Ergebnis eines Ablaufs.
pub type AuthOutcome = Result<AuthSuccess, ApiError>;

enum Slot {
    Running,
    Done(AuthOutcome, Instant),
}

/// Offene und fertige Abläufe je Ticket (SHA-256).
#[derive(Default)]
pub struct PendingAuths {
    inner: Mutex<HashMap<[u8; 32], Slot>>,
}

/// Was [`PendingAuths::take`] gefunden hat.
pub enum Taken {
    /// Unbekanntes oder abgelaufenes Ticket.
    Unknown,
    /// Läuft noch.
    Running,
    /// Fertig (einmalig abgeholt, danach entfernt).
    Done(AuthOutcome),
}

fn hash(ticket: &str) -> [u8; 32] {
    Sha256::digest(ticket.as_bytes()).into()
}

impl PendingAuths {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<[u8; 32], Slot>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn sweep(map: &mut HashMap<[u8; 32], Slot>, now: Instant) {
        map.retain(|_, s| match s {
            Slot::Running => true,
            Slot::Done(_, at) => now.duration_since(*at) < RESULT_TTL,
        });
    }

    /// Neues Ticket für einen laufenden Ablauf; `None`, wenn schon
    /// [`MAX_PENDING`] offen sind. Liefert (Cookie-Wert, Schlüssel).
    pub fn park(&self) -> Option<(String, [u8; 32])> {
        let mut map = self.lock();
        Self::sweep(&mut map, Instant::now());
        if map.len() >= MAX_PENDING {
            return None;
        }
        let mut raw = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let ticket = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
        let key = hash(&ticket);
        map.insert(key, Slot::Running);
        Some((ticket, key))
    }

    /// Hinterlegt das Ergebnis (verworfen, wenn das Ticket inzwischen
    /// entfernt wurde, z. B. durch „Datenbank sperren“).
    pub fn finish(&self, key: [u8; 32], outcome: AuthOutcome) {
        let mut map = self.lock();
        if let Some(slot) = map.get_mut(&key) {
            *slot = Slot::Done(outcome, Instant::now());
        }
    }

    /// Holt das Ergebnis zum Ticket ab.
    pub fn take(&self, ticket: &str) -> Taken {
        let key = hash(ticket);
        let mut map = self.lock();
        Self::sweep(&mut map, Instant::now());
        match map.get(&key) {
            None => Taken::Unknown,
            Some(Slot::Running) => Taken::Running,
            Some(Slot::Done(..)) => match map.remove(&key) {
                Some(Slot::Done(outcome, _)) => Taken::Done(outcome),
                _ => Taken::Unknown,
            },
        }
    }

    /// Verwirft alle offenen und fertigen Abläufe („Datenbank sperren“).
    pub fn clear(&self) {
        self.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `Set-Cookie` für das Abhol-Ticket.
pub fn ticket_cookie(ticket: &str) -> String {
    format!(
        "{TICKET_COOKIE}={ticket}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={TICKET_MAX_AGE}"
    )
}

/// `Set-Cookie`, das das Ticket im Browser löscht.
pub fn clear_ticket_cookie() -> String {
    format!("{TICKET_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok() -> AuthOutcome {
        Ok(AuthSuccess {
            body: serde_json::json!({"x": 1}),
            device: None,
        })
    }

    #[tokio::test(start_paused = true)]
    async fn park_finish_take_once_and_expire() {
        let p = PendingAuths::default();
        let (ticket, key) = p.park().unwrap();
        assert_eq!(ticket.len(), 43);
        assert!(matches!(p.take(&ticket), Taken::Running));
        assert!(matches!(p.take("unbekannt"), Taken::Unknown));
        p.finish(key, ok());
        assert!(matches!(p.take(&ticket), Taken::Done(Ok(_))));
        assert!(
            matches!(p.take(&ticket), Taken::Unknown),
            "nur einmal abholbar"
        );

        let (late, key) = p.park().unwrap();
        p.finish(key, ok());
        tokio::time::advance(RESULT_TTL).await;
        assert!(matches!(p.take(&late), Taken::Unknown), "abgelaufen");
    }

    #[tokio::test]
    async fn bounded_and_cleared_by_lock() {
        let p = PendingAuths::default();
        let tickets: Vec<_> = (0..MAX_PENDING).map(|_| p.park().unwrap()).collect();
        assert!(p.park().is_none(), "höchstens MAX_PENDING offen");
        p.clear();
        assert!(p.is_empty());
        let (ticket, key) = &tickets[0];
        p.finish(*key, ok());
        assert!(
            matches!(p.take(ticket), Taken::Unknown),
            "nach dem Sperren nicht mehr abholbar"
        );
        assert!(ticket_cookie("t").contains("HttpOnly; Secure; SameSite=Strict"));
        assert!(clear_ticket_cookie().contains("Max-Age=0"));
    }
}
