//! Fehlerabbildung auf HTTP (Plan W5.3).
//!
//! Die Variante wird **vor** dem Serialisieren ausgewertet (früher wurde jeder
//! Fehler zum String). Fachliche Fehler kommen mit Meldung zurück, interne nur
//! mit `ref`; der volle Text steht im Log unter derselben `ref`.
//!
//! | Variante | Status | `code` |
//! |---|---|---|
//! | `Domain` | 422 | `domain` |
//! | `Mail` | 422 | `mail` |
//! | `Backup` | 409 | `backup` |
//! | `Crypto` | 400 | `crypto` |
//! | `BadRequest` | 400 | `bad_request` |
//! | `Locked` | 423 | `locked` |
//! | `SchemaMismatch` | 409 | `schema_mismatch` |
//! | `Db` mit `SQLITE_CONSTRAINT_TRIGGER` (RAISE in den Migrationen) | 409 | `constraint` |
//! | `Sidecar` | 503 | `sidecar` (fester Text + `ref`) |
//! | `Db` (sonst), `Migrate`, `Io`, `Json`, `Zip`, `Config`, `Other` | 500 | `internal` (+ `ref`) |

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::error::Error;

/// Erweiterter SQLite-Code für `RAISE(ABORT, …)` in Triggern.
pub const SQLITE_CONSTRAINT_TRIGGER: &str = "1811";

/// Fester Text für Sidecar-Ausfälle (Details nur im Log).
pub const SIDECAR_MESSAGE: &str =
    "Der Prüf- und Konvertierungsdienst (Java) ist nicht verfügbar. Bitte später erneut versuchen.";

/// Fehlerantwort der API.
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    #[serde(skip)]
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip)]
    pub retry_after: Option<u64>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            reference: None,
            retry_after: None,
        }
    }

    pub fn with_retry_after(mut self, secs: u64) -> Self {
        self.retry_after = Some(secs);
        self
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", "Nicht gefunden.")
    }

    pub fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Nicht angemeldet — bitte anmelden.",
        )
    }

    pub fn reauth_required() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "reauth_required",
            "Bitte zur Bestätigung erneut anmelden (Passphrase und TOTP).",
        )
    }

    pub fn missing_request_header() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "request_header_missing",
            "Anfrage ohne Pflicht-Header abgelehnt.",
        )
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "Anfrage zu groß.",
        )
    }

    pub fn busy(retry_after_secs: u64) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "Der Server ist ausgelastet — bitte gleich erneut versuchen.",
        )
        .with_retry_after(retry_after_secs)
    }

    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Zu viele aufwendige Aufrufe in kurzer Zeit — bitte kurz warten.",
        )
        .with_retry_after(retry_after_secs)
    }

    pub fn timeout() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "timeout",
            "Zeitüberschreitung. Eine begonnene Aktion läuft im Hintergrund zu Ende; \
             mit demselben Idempotenz-Schlüssel liefert eine Wiederholung ihr Ergebnis.",
        )
        .with_retry_after(5)
    }

    pub fn restarting() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "restarting",
            "Der Server startet neu — bitte gleich erneut versuchen.",
        )
        .with_retry_after(10)
    }

    /// Interner Fehler: Text nur ins Log, nach außen `ref`.
    pub fn internal(detail: &dyn std::fmt::Display) -> Self {
        let reference = uuid::Uuid::new_v4().to_string();
        tracing::error!(target: "klein_buch_lib::http", reference = %reference, "interner Fehler: {detail}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: format!("Interner Fehler (Ref {reference})."),
            reference: Some(reference),
            retry_after: None,
        }
    }

    /// Abbildung einer Kern-Variante (Tabelle oben).
    pub fn from_error(err: &Error) -> Self {
        match err {
            Error::Domain(m) => Self::new(StatusCode::UNPROCESSABLE_ENTITY, "domain", m.clone()),
            Error::Mail(m) => Self::new(StatusCode::UNPROCESSABLE_ENTITY, "mail", m.clone()),
            Error::Backup(m) => Self::new(StatusCode::CONFLICT, "backup", m.clone()),
            Error::Crypto(m) => Self::new(StatusCode::BAD_REQUEST, "crypto", m.clone()),
            Error::BadRequest(_) => Self::bad_request(err.to_string()),
            Error::Locked => Self::new(StatusCode::LOCKED, "locked", err.to_string()),
            Error::SchemaMismatch { .. } => {
                Self::new(StatusCode::CONFLICT, "schema_mismatch", err.to_string())
            }
            Error::Db(sqlx::Error::Database(db))
                if db.code().as_deref() == Some(SQLITE_CONSTRAINT_TRIGGER) =>
            {
                Self::new(StatusCode::CONFLICT, "constraint", db.message().to_string())
            }
            Error::Sidecar(_) => {
                let reference = uuid::Uuid::new_v4().to_string();
                tracing::error!(target: "klein_buch_lib::http", reference = %reference, "Sidecar: {err}");
                Self {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    code: "sidecar",
                    message: format!("{SIDECAR_MESSAGE} (Ref {reference})"),
                    reference: Some(reference),
                    retry_after: Some(30),
                }
            }
            Error::Db(_)
            | Error::Migrate(_)
            | Error::Io(_)
            | Error::Json(_)
            | Error::Zip(_)
            | Error::Config(_)
            | Error::Other(_) => Self::internal(err),
        }
    }

    /// JSON-Körper der Antwort.
    pub fn body(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({"code": "internal"}))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, axum::Json(self.body())).into_response();
        if let Some(secs) = self.retry_after {
            if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        response
    }
}

impl From<Error> for ApiError {
    fn from(err: Error) -> Self {
        Self::from_error(&err)
    }
}

/// 404 für alles außerhalb der Routen bzw. der Allowlist.
pub async fn not_found() -> ApiError {
    ApiError::not_found()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Statustabelle für jede Variante aus `error.rs` (Plan W5 Done-Check).
    #[test]
    fn status_table_covers_every_variant() {
        let io = || std::io::Error::other("geheim: /data/archive/x.pdf");
        let cases: Vec<(Error, StatusCode, &str)> = vec![
            (
                Error::Db(sqlx::Error::RowNotFound),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
            (
                Error::Migrate(sqlx::migrate::MigrateError::VersionMissing(1)),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
            (
                Error::SchemaMismatch {
                    expected: 30,
                    found: 31,
                    hint: "neuer".into(),
                },
                StatusCode::CONFLICT,
                "schema_mismatch",
            ),
            (
                Error::Io(io()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
            (
                Error::Json(serde_json::from_str::<i32>("x").unwrap_err()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
            (
                Error::Config("c".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
            (
                Error::Domain("d".into()),
                StatusCode::UNPROCESSABLE_ENTITY,
                "domain",
            ),
            (
                Error::Sidecar("java weg".into()),
                StatusCode::SERVICE_UNAVAILABLE,
                "sidecar",
            ),
            (
                Error::Mail("m".into()),
                StatusCode::UNPROCESSABLE_ENTITY,
                "mail",
            ),
            (Error::Backup("b".into()), StatusCode::CONFLICT, "backup"),
            (Error::Locked, StatusCode::LOCKED, "locked"),
            (
                Error::BadRequest("x".into()),
                StatusCode::BAD_REQUEST,
                "bad_request",
            ),
            (Error::Crypto("k".into()), StatusCode::BAD_REQUEST, "crypto"),
            (
                Error::Zip(zip::result::ZipError::FileNotFound),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
            (
                Error::Other(anyhow::anyhow!("o")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
            ),
        ];
        for (err, status, code) in cases {
            let api = ApiError::from_error(&err);
            assert_eq!(api.status, status, "{err:?}");
            assert_eq!(api.code, code, "{err:?}");
            if status.is_server_error() {
                let body = api.body().to_string();
                assert!(api.reference.is_some(), "{err:?} braucht ref");
                assert!(!body.contains("geheim"), "kein interner Text: {body}");
                assert!(!body.contains("/data"), "kein Pfad: {body}");
            }
        }
    }

    #[test]
    fn retry_after_header_is_set() {
        let resp = ApiError::busy(7).into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers().get(header::RETRY_AFTER).unwrap(), "7");
    }
}
