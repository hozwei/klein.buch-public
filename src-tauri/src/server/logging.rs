//! Logging ohne Geheimnisse (Plan W5.9) und mit einer Zeile je Ereignis.
//!
//! sqlx führt `PRAGMA key = '<Passphrase>'` als normales Statement aus und
//! loggt Statements auf DEBUG bzw. langsame auf WARN. `db::open_pool` schaltet
//! das Statement-Logging ab; zusätzlich hängt [`filter`] fest
//! `sqlx::query=off` an, das auch ein gesetztes `RUST_LOG` nicht aufhebt.
//!
//! **Eine Zeile je Ereignis (W14-017, Lauf F4c):** [`OneLine`] ersetzt
//! Zeilenumbrüche und Steuerzeichen innerhalb eines Ereignisses durch
//! Escape-Folgen (`\n`, `\r`, `\u{1b}` …). Ein Wert im Log (Host, Dateiname,
//! Fehlertext einer Gegenstelle) kann so keine zweite, gefälschte Zeile
//! erzeugen — etwa eine `auth.login.failed`-Zeile für fail2ban.
//!
//! **Zugriffsprotokoll (W14-018, Lauf F4c):** Methode, Pfad ohne Query, Status,
//! Dauer, `ip=` (Client-IP nach der Proxy-Regel), `req=` (Kennung der Anfrage)
//! und `sess=` (die ersten 4 Byte des SHA-256 des Sitzungs-Cookies wie in der
//! Sitzungsübersicht, `-` ohne Cookie; kein Rückschluss auf das Cookie). Die
//! Kennung der Anfrage ist zugleich die `ref` interner Fehler
//! ([`super::errors::ApiError::internal`]), auch wenn der Kern als eigener Task
//! läuft ([`with_request_id`]). Rechtsgrundlage für die IP im Log:
//! berechtigtes Interesse an der Sicherheit (Art. 6 Abs. 1 lit. f DSGVO),
//! Aufbewahrung über die Log-Rotation des Containers (security.md §9).

use std::future::Future;
use std::io::Write;
use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

/// Standard-Filter ohne `RUST_LOG`.
pub const DEFAULT_FILTER: &str = "info";
/// Wird immer angehängt (überschreibt eine gleichnamige Direktive aus `RUST_LOG`).
pub const SQLX_QUERY_OFF: &str = "sqlx::query=off";

/// Filter aus `RUST_LOG` (falls gültig) bzw. [`DEFAULT_FILTER`], immer mit
/// [`SQLX_QUERY_OFF`].
pub fn filter(rust_log: Option<&str>) -> EnvFilter {
    let base = rust_log
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_FILTER));
    match SQLX_QUERY_OFF.parse() {
        Ok(directive) => base.add_directive(directive),
        // statisch gültig; ohne Direktive bleibt das Statement-Logging in
        // `db::open_pool` trotzdem aus
        Err(_) => base,
    }
}

/// Initialisiert den globalen Subscriber (einmal pro Prozess).
pub fn init() {
    let rust_log = std::env::var("RUST_LOG").ok();
    // Farbcodes nur im Terminal, nicht im Container-Log.
    let ansi = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter(rust_log.as_deref()))
        .with_ansi(ansi)
        .with_writer(OneLine(std::io::stdout))
        .try_init();
}

/// `MakeWriter`, der jedes Ereignis als genau eine Zeile schreibt (siehe
/// Modul-Doku). `tracing_subscriber::fmt` formatiert ein Ereignis vollständig
/// und schreibt es mit einem `write_all`; ein abschließender Zeilenumbruch
/// bleibt, alle anderen Steuerzeichen außer Tab werden escaped.
#[derive(Clone, Copy, Debug, Default)]
pub struct OneLine<M>(pub M);

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for OneLine<M> {
    type Writer = OneLineWriter<M::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        OneLineWriter(self.0.make_writer())
    }
}

/// Writer zu [`OneLine`].
pub struct OneLineWriter<W>(W);

impl<W: Write> Write for OneLineWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write_all(&escape_event(buf))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Escaped Steuerzeichen eines formatierten Ereignisses; ein Zeilenumbruch am
/// Ende bleibt erhalten. Ohne Steuerzeichen bleibt der Text unverändert.
pub fn escape_event(buf: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    let (body, tail): (&[u8], &[u8]) = match buf.split_last() {
        Some((b'\n', body)) => (body, b"\n"),
        _ => (buf, b""),
    };
    let needs = |b: u8| (b < 0x20 && b != b'\t') || b == 0x7f;
    if !body.iter().copied().any(needs) {
        return std::borrow::Cow::Borrowed(buf);
    }
    let mut out = Vec::with_capacity(buf.len() + 16);
    for &b in body {
        match b {
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b if needs(b) => out.extend_from_slice(format!("\\u{{{b:02x}}}").as_bytes()),
            b => out.push(b),
        }
    }
    out.extend_from_slice(tail);
    std::borrow::Cow::Owned(out)
}

tokio::task_local! {
    static REQUEST_ID: Option<Arc<str>>;
}

/// Kennung der laufenden Anfrage (`req=` im Zugriffsprotokoll), falls der
/// Aufruf aus einer Anfrage stammt.
pub fn current_request_id() -> Option<String> {
    REQUEST_ID
        .try_with(|id| id.as_deref().map(str::to_string))
        .ok()
        .flatten()
}

/// Gibt die Kennung der laufenden Anfrage an einen abgekoppelten Task weiter
/// (Kern eines Commands, Job, Anmeldung): ein interner Fehler dort trägt
/// dieselbe `ref` wie die Zeile im Zugriffsprotokoll.
pub fn with_request_id<F: Future>(fut: F) -> impl Future<Output = F::Output> {
    let id = REQUEST_ID.try_with(Clone::clone).ok().flatten();
    REQUEST_ID.scope(id, fut)
}

/// Zugriffsprotokoll (siehe Modul-Doku).
pub async fn access_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let sess = super::session::short_id_from_headers(req.headers());
    let id: Arc<str> = Arc::from(uuid::Uuid::new_v4().to_string());
    let started = std::time::Instant::now();
    let response = REQUEST_ID.scope(Some(id.clone()), next.run(req)).await;
    let ip = response
        .extensions()
        .get::<super::policy::ClientIp>()
        .map(|c| c.0.to_string());
    tracing::info!(
        target: "klein_buch_lib::http",
        "{} {} {} {} ms ip={} req={} sess={}",
        method,
        path,
        response.status().as_u16(),
        started.elapsed().as_millis(),
        ip.as_deref().unwrap_or("-"),
        id,
        sess.as_deref().unwrap_or("-"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Buffer {
        type Writer = Buffer;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn capture(rust_log: Option<&str>, emit: impl FnOnce()) -> String {
        let buffer = Buffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter(rust_log))
            .with_ansi(false)
            .with_writer(OneLine(buffer.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, emit);
        let text = String::from_utf8_lossy(&buffer.0.lock().unwrap()).into_owned();
        text
    }

    /// W5-LOG-TESTS: Auch ein ausdrückliches `RUST_LOG=sqlx::query=trace`
    /// (oder `trace` für alles) lässt kein Statement-Ereignis von sqlx durch;
    /// andere Ziele bleiben sichtbar.
    #[test]
    fn filter_keeps_sqlx_statements_off_whatever_rust_log_says() {
        // (RUST_LOG, sieht das Ziel klein_buch_lib::auth auf WARN?) — eine reine
        // Ziel-Direktive schaltet alle anderen Ziele ab (EnvFilter-Semantik).
        for (rust_log, visible) in [
            (Some("sqlx::query=trace"), false),
            (Some("sqlx::query=trace,info"), true),
            (Some("trace"), true),
            (Some("sqlx=trace,sqlx::query=trace,warn"), true),
            (Some("ungültig[[["), true),
            (Some(""), true),
            (None, true),
        ] {
            let text = capture(rust_log, || {
                tracing::warn!(target: "sqlx::query", "PRAGMA key = 'geheim-4711'");
                tracing::trace!(target: "sqlx::query", "slow statement geheim-4711");
                tracing::warn!(target: "klein_buch_lib::auth", "sichtbar");
            });
            assert!(!text.contains("geheim-4711"), "{rust_log:?}: {text}");
            assert_eq!(text.contains("sichtbar"), visible, "{rust_log:?}: {text}");
        }
    }

    /// W14-017: Ein Wert mit Zeilenumbruch erzeugt keine zweite Zeile; eine
    /// gefälschte fail2ban-Zeile bleibt Teil der ersten.
    #[test]
    fn one_line_per_event() {
        let host = "smtp.example.org\n2026-10-25T10:00:00.000000Z  WARN klein_buch_lib::auth: \
                    auth.login.failed ip=192.0.2.1 reason=totp\r\u{1b}[2K";
        let text = capture(None, || {
            tracing::warn!("SMTP-Host {host}");
            tracing::info!(host, "Feld");
        });
        assert_eq!(text.lines().count(), 2, "{text}");
        assert!(text.ends_with('\n'));
        for line in text.lines() {
            assert!(
                !line.starts_with("2026-10-25T10:00:00"),
                "gefälschte Zeile: {line}"
            );
        }
        assert!(text.contains("smtp.example.org\\n2026"), "{text}");
        assert!(text.contains("\\r"), "{text}");
        assert!(!text.contains('\u{1b}'), "{text}");
        assert_eq!(
            &*escape_event(b"ohne\tSteuerzeichen\n"),
            b"ohne\tSteuerzeichen\n"
        );
    }

    /// W14-018: Die Kennung der Anfrage reicht in abgekoppelte Tasks.
    #[tokio::test]
    async fn request_id_reaches_detached_tasks() {
        assert_eq!(current_request_id(), None);
        let id: Arc<str> = Arc::from("req-4711");
        let seen = REQUEST_ID
            .scope(Some(id), async {
                tokio::spawn(with_request_id(async { current_request_id() }))
                    .await
                    .unwrap()
            })
            .await;
        assert_eq!(seen.as_deref(), Some("req-4711"));
        let outside = tokio::spawn(with_request_id(async { current_request_id() }))
            .await
            .unwrap();
        assert_eq!(outside, None);
    }
}
