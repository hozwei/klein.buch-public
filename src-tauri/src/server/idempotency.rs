//! Idempotenz-Schlüssel für ändernde Commands und Jobs (Plan W5.6).
//!
//! Das Frontend schickt je Aktion einen Schlüssel im Header
//! [`HEADER`]. Eine Wiederholung mit demselben Schlüssel (z. B. nach einem
//! abgebrochenen Request) führt den Kern nicht noch einmal aus, sondern liefert
//! das gespeicherte Ergebnis bzw. wartet auf den laufenden Aufruf. Derselbe
//! Schlüssel mit anderem Inhalt wird abgelehnt. 5xx-Ergebnisse werden nicht
//! gespeichert, damit eine Wiederholung nach einem vorübergehenden Fehler
//! erneut ausgeführt wird.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use tokio::time::Instant;

use super::errors::ApiError;

pub const HEADER: &str = "idempotency-key";
pub const TTL: Duration = Duration::from_secs(3600);
pub const MAX_ENTRIES: usize = 2048;
const MAX_KEY_LEN: usize = 128;

/// Gespeicherte Antwort (Status + JSON-Körper).
#[derive(Debug, Clone)]
pub struct StoredResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

impl StoredResponse {
    pub fn json(status: StatusCode, value: &serde_json::Value) -> Self {
        Self {
            status,
            body: Bytes::from(value.to_string()),
        }
    }
}

impl IntoResponse for StoredResponse {
    fn into_response(self) -> Response {
        let mut response = (self.status, self.body).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    }
}

enum Slot {
    InFlight(watch::Receiver<Option<StoredResponse>>),
    Done(StoredResponse),
}

struct Entry {
    hash: [u8; 32],
    slot: Slot,
    at: Instant,
}

type Map = HashMap<(String, String), Entry>;

#[derive(Default, Clone)]
pub struct IdempotencyStore {
    inner: Arc<Mutex<Map>>,
}

/// Ergebnis von [`IdempotencyStore::begin`].
pub enum Begin {
    /// Erster Aufruf: ausführen und über den [`Completer`] abschließen.
    New(Completer),
    /// Schon fertig: gespeicherte Antwort.
    Replay(StoredResponse),
    /// Läuft noch: auf das Ergebnis warten.
    Wait(watch::Receiver<Option<StoredResponse>>),
    /// Gleicher Schlüssel, anderer Inhalt.
    Mismatch,
}

/// Schließt einen Eintrag ab. Fällt er ohne [`Completer::complete`] weg
/// (Panik, Ablehnung vor dem Start), wird der Eintrag entfernt — eine
/// Wiederholung führt dann neu aus.
pub struct Completer {
    store: IdempotencyStore,
    key: (String, String),
    tx: Option<watch::Sender<Option<StoredResponse>>>,
}

impl Completer {
    pub fn complete(mut self, response: StoredResponse) {
        let mut map = self.store.lock();
        if response.status.is_server_error() {
            map.remove(&self.key);
        } else if let Some(entry) = map.get_mut(&self.key) {
            entry.slot = Slot::Done(response.clone());
            entry.at = Instant::now();
        }
        drop(map);
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Some(response));
        }
    }
}

impl Drop for Completer {
    fn drop(&mut self) {
        if self.tx.is_some() {
            self.store.lock().remove(&self.key);
        }
    }
}

/// Hash über Command-Name und Request-Körper.
pub fn body_hash(command: &str, body: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(command.as_bytes());
    h.update([0u8]);
    h.update(body);
    h.finalize().into()
}

/// Schlüssel aus dem Header; fehlt er, `None`. Erlaubt: 1–128 Zeichen aus
/// `A–Z a–z 0–9 - _`.
pub fn key_from_headers(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    let Some(value) = headers.get(HEADER) else {
        return Ok(None);
    };
    let key = value
        .to_str()
        .map_err(|_| ApiError::bad_request("Ungültiger Idempotenz-Schlüssel."))?;
    let valid = !key.is_empty()
        && key.len() <= MAX_KEY_LEN
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
        return Err(ApiError::bad_request("Ungültiger Idempotenz-Schlüssel."));
    }
    Ok(Some(key.to_string()))
}

impl IdempotencyStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, Map> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn begin(&self, command: &str, key: &str, hash: [u8; 32]) -> Begin {
        let now = Instant::now();
        let mut map = self.lock();
        map.retain(|_, e| matches!(e.slot, Slot::InFlight(_)) || now.duration_since(e.at) < TTL);
        let k = (command.to_string(), key.to_string());
        if let Some(entry) = map.get(&k) {
            if entry.hash != hash {
                return Begin::Mismatch;
            }
            return match &entry.slot {
                Slot::Done(resp) => Begin::Replay(resp.clone()),
                Slot::InFlight(rx) => Begin::Wait(rx.clone()),
            };
        }
        if map.len() >= MAX_ENTRIES {
            // älteste abgeschlossene Einträge verdrängen
            let mut done: Vec<_> = map
                .iter()
                .filter(|(_, e)| matches!(e.slot, Slot::Done(_)))
                .map(|(k, e)| (k.clone(), e.at))
                .collect();
            done.sort_by_key(|(_, at)| *at);
            for (old, _) in done.into_iter().take(map.len() + 1 - MAX_ENTRIES) {
                map.remove(&old);
            }
        }
        let (tx, rx) = watch::channel(None);
        map.insert(
            k.clone(),
            Entry {
                hash,
                slot: Slot::InFlight(rx),
                at: now,
            },
        );
        Begin::New(Completer {
            store: self.clone(),
            key: k,
            tx: Some(tx),
        })
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Wartet auf das Ergebnis eines laufenden Aufrufs mit demselben Schlüssel.
pub async fn wait_for(
    mut rx: watch::Receiver<Option<StoredResponse>>,
) -> Result<StoredResponse, ApiError> {
    loop {
        if let Some(resp) = rx.borrow().clone() {
            return Ok(resp);
        }
        if rx.changed().await.is_err() {
            // erster Aufruf ohne Ergebnis abgebrochen → erneut versuchen lassen
            return Err(ApiError::busy(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replay_wait_mismatch_and_abort() {
        let store = IdempotencyStore::default();
        let h1 = body_hash("x", b"{}");
        let h2 = body_hash("x", b"{\"a\":1}");

        let Begin::New(c) = store.begin("x", "k1", h1) else {
            panic!("erster Aufruf")
        };
        let Begin::Wait(rx) = store.begin("x", "k1", h1) else {
            panic!("läuft noch")
        };
        assert!(matches!(store.begin("x", "k1", h2), Begin::Mismatch));
        c.complete(StoredResponse::json(
            StatusCode::OK,
            &serde_json::json!({"n": 1}),
        ));
        assert_eq!(wait_for(rx).await.unwrap().status, StatusCode::OK);
        let Begin::Replay(r) = store.begin("x", "k1", h1) else {
            panic!("fertig")
        };
        assert_eq!(&r.body[..], b"{\"n\":1}");

        // Abbruch ohne Ergebnis → Eintrag weg, Wartende bekommen 503.
        let Begin::New(c) = store.begin("x", "k2", h1) else {
            panic!()
        };
        let Begin::Wait(rx) = store.begin("x", "k2", h1) else {
            panic!()
        };
        drop(c);
        assert_eq!(
            wait_for(rx).await.unwrap_err().status,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(matches!(store.begin("x", "k2", h1), Begin::New(_)));

        // 5xx wird nicht gespeichert.
        let Begin::New(c) = store.begin("x", "k3", h1) else {
            panic!()
        };
        c.complete(StoredResponse::json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &serde_json::json!({}),
        ));
        assert!(matches!(store.begin("x", "k3", h1), Begin::New(_)));
    }

    #[test]
    fn key_validation() {
        let mut h = HeaderMap::new();
        assert_eq!(key_from_headers(&h).unwrap(), None);
        h.insert(HEADER, HeaderValue::from_static("abc-DEF_123"));
        assert_eq!(
            key_from_headers(&h).unwrap().as_deref(),
            Some("abc-DEF_123")
        );
        h.insert(HEADER, HeaderValue::from_static("a b"));
        assert!(key_from_headers(&h).is_err());
        h.insert(HEADER, HeaderValue::from_str(&"a".repeat(129)).unwrap());
        assert!(key_from_headers(&h).is_err());
    }
}
