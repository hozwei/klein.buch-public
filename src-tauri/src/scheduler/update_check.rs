//! Opt-in Versionsprüfung (Plan W12, ADR 0039 E14, Amendment ADR 0013).
//!
//! Nur wenn `app_settings.update_check_enabled` gesetzt ist (Standard aus),
//! fragt der Scheduler-Tick höchstens einmal je 24 h die GitHub-Releases ab
//! (`…/releases/latest` des Repos aus der Quell-URL, sonst
//! [`DEFAULT_RELEASES_URL`]; für Tests und Spiegel per
//! [`ENV_UPDATE_URL`] überschreibbar). Gesendet wird nur eine GET-Anfrage mit
//! festem User-Agent — keine Version, kein Cookie, keine Instanzdaten; keine
//! Weiterleitungen. Eine neuere Version landet als Inbox-Eintrag mit Name und
//! Link **nur als Text** (nie HTML, kein `action_url`), je Version einmal
//! (`dedup_key`). Das Update selbst löst der Betreiber in seiner
//! Container-Verwaltung aus (E14).

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::db::repo::app_settings;
use crate::error::{Error, Result};
use crate::notify::{self, NewNotification};

/// Schalter (Standard aus).
pub const SETTING_ENABLED: &str = "update_check_enabled";
/// Zeitpunkt der letzten Abfrage (RFC 3339, UTC) — auch nach Fehlschlag, damit
/// höchstens einmal täglich eine Verbindung entsteht.
pub const SETTING_LAST_CHECK: &str = "update_check_last_at";
/// Vollständige Abfrage-URL statt der aus der Quell-URL abgeleiteten.
pub const ENV_UPDATE_URL: &str = "KLEIN_BUCH_UPDATE_URL";
pub const DEFAULT_RELEASES_URL: &str =
    "https://api.github.com/repos/hozwei/klein.buch-public/releases/latest";
/// Einziger mitgesendeter Kennwert (GitHub verlangt einen User-Agent).
pub const USER_AGENT: &str = "Klein.Buch-Versionspruefung";
/// Mindestabstand zweier Abfragen (Stunden).
pub const CHECK_INTERVAL_HOURS: i64 = 24;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

/// Ergebnis eines Laufs (für Log und Tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Kein Opt-in — keine Verbindung.
    Disabled,
    /// Letzte Abfrage jünger als [`CHECK_INTERVAL_HOURS`] — keine Verbindung.
    NotDue,
    UpToDate {
        latest: String,
    },
    /// Neuere Version; `created = false`, wenn der Hinweis schon existierte.
    NewVersion {
        tag: String,
        created: bool,
    },
}

/// Ausschnitt der GitHub-Antwort.
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// `https://github.com/<owner>/<repo>` → GitHub-API-URL der neuesten
/// Release; andere Quellen → [`DEFAULT_RELEASES_URL`].
pub fn releases_url_for(source_url: &str) -> String {
    let Ok(url) = url::Url::parse(source_url) else {
        return DEFAULT_RELEASES_URL.to_string();
    };
    let segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    let valid = |s: &str| !s.is_empty() && s.chars().all(allowed);
    match (url.scheme(), url.host_str(), segments.as_slice()) {
        ("https", Some("github.com"), [owner, repo]) if valid(owner) && valid(repo) => {
            let repo = repo.trim_end_matches(".git");
            format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
        }
        _ => DEFAULT_RELEASES_URL.to_string(),
    }
}

/// Abfrage-URL: [`ENV_UPDATE_URL`] (nur http/https), sonst aus der
/// wirksamen Quell-URL.
pub fn endpoint() -> String {
    let override_url = std::env::var(ENV_UPDATE_URL)
        .ok()
        .filter(|v| url::Url::parse(v).is_ok_and(|u| matches!(u.scheme(), "https" | "http")));
    override_url.unwrap_or_else(|| releases_url_for(&crate::build_info::source_url()))
}

/// CalVer `YYYY.M.PATCH` (optional mit `v` davor).
pub fn parse_calver(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim();
    let v = v.strip_prefix(['v', 'V']).unwrap_or(v);
    let mut parts = v.split('.').map(|p| p.parse::<u32>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Ist `candidate` neuer als `current`? Unlesbare Angaben zählen nicht.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_calver(candidate), parse_calver(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}

/// Steuerzeichen und unsichtbare Zeichen, die einen Text anders aussehen
/// lassen können (Zero-Width, Bidi-Steuerung).
fn is_hidden(c: char) -> bool {
    c.is_control()
        || ('\u{200b}'..='\u{200f}').contains(&c)
        || ('\u{202a}'..='\u{202e}').contains(&c)
        || ('\u{2066}'..='\u{2069}').contains(&c)
}

/// Text ohne [`is_hidden`]-Zeichen, gekürzt (landet in der Inbox).
fn plain(text: &str, max_chars: usize) -> String {
    text.chars()
        .filter(|c| !is_hidden(*c))
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Link der Release als Text: `html_url`, wenn es eine http(s)-Adresse ohne
/// Leerzeichen ist, sonst `<Quell-URL>/releases/tag/<tag>`.
fn release_link(html_url: Option<&str>, tag: &str) -> String {
    let ok = |u: &&str| {
        u.len() <= 300
            && !u.chars().any(|c| c.is_whitespace() || c.is_control())
            && url::Url::parse(u).is_ok_and(|p| matches!(p.scheme(), "https" | "http"))
    };
    match html_url.filter(ok) {
        Some(u) => u.to_string(),
        None => format!("{}/releases/tag/{tag}", crate::build_info::source_url()),
    }
}

/// Holt die neueste Release (GET, fester User-Agent, ohne Weiterleitung,
/// Antwort höchstens [`MAX_RESPONSE_BYTES`]).
async fn fetch_latest(url: &str) -> std::result::Result<Release, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT);
    // Ein lokaler Endpunkt (Tests, Spiegel) geht nie über einen Proxy aus der
    // Umgebung; sonst gelten `HTTPS_PROXY`/`NO_PROXY` des Betreibers.
    let loopback = url::Url::parse(url)
        .is_ok_and(|u| matches!(u.host_str(), Some("127.0.0.1" | "[::1]" | "localhost")));
    if loopback {
        builder = builder.no_proxy();
    }
    let client = builder.build().map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err("Antwort zu groß".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|e| format!("Antwort unlesbar: {e}"))
}

/// Ein Lauf: prüft Opt-in und Abstand, fragt ab und legt ggf. den Hinweis an.
/// Ohne Opt-in entsteht keine Verbindung.
pub async fn run_if_due(pool: &SqlitePool, url: &str, now: DateTime<Utc>) -> Result<Outcome> {
    if !app_settings::get_bool(pool, SETTING_ENABLED, false).await? {
        return Ok(Outcome::Disabled);
    }
    let last = app_settings::get(pool, SETTING_LAST_CHECK)
        .await?
        .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
        .map(|t| t.with_timezone(&Utc));
    let interval = chrono::Duration::hours(CHECK_INTERVAL_HOURS);
    // Eine Zeit in der Zukunft (Uhr zurückgestellt) zählt nicht als frisch.
    if last.is_some_and(|t| t <= now && now - t < interval) {
        return Ok(Outcome::NotDue);
    }
    app_settings::set(pool, SETTING_LAST_CHECK, &now.to_rfc3339()).await?;

    let release = fetch_latest(url)
        .await
        .map_err(|e| Error::Other(anyhow::anyhow!("Versionsprüfung: {e}")))?;
    let Some(tag) = crate::build_info::clean_tag(Some(&release.tag_name)) else {
        return Err(Error::Other(anyhow::anyhow!(
            "Versionsprüfung: unbrauchbares Tag"
        )));
    };
    let current = env!("CARGO_PKG_VERSION");
    if release.draft || release.prerelease || !is_newer(&tag, current) {
        return Ok(Outcome::UpToDate { latest: tag });
    }

    let version = tag.trim_start_matches(['v', 'V']).to_string();
    let name = release
        .name
        .as_deref()
        .map(|n| plain(n, 120))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("Klein.Buch {version}"));
    let link = release_link(release.html_url.as_deref(), &tag);
    let title = format!("Neue Version verfügbar: {version}");
    let body = format!(
        "„{name}“ ist erschienen (hier läuft {current}). Versionshinweise: {link} — \
         Das Update spielst du in deiner Container-Verwaltung ein (Image-Tag {version}); \
         vor einer Datenbank-Umstellung legt Klein.Buch beim Entsperren selbst ein Backup an."
    );
    let key = format!("update_available:{tag}");
    let created = notify::emit(
        pool,
        NewNotification {
            title: &title,
            body: &body,
            severity: "info",
            dedup_key: Some(&key),
            ..Default::default()
        },
    )
    .await?
    .is_some();
    Ok(Outcome::NewVersion { tag, created })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calver_comparison() {
        assert_eq!(parse_calver("v2026.10.0"), Some((2026, 10, 0)));
        assert_eq!(parse_calver("2026.5.1"), Some((2026, 5, 1)));
        for bad in ["", "v2026.10", "2026.10.0.1", "2026.x.0", "latest"] {
            assert_eq!(parse_calver(bad), None, "{bad}");
        }
        assert!(is_newer("v2026.10.0", "2026.5.1"));
        assert!(is_newer("v2027.1.0", "2026.12.3"));
        assert!(!is_newer("v2026.5.1", "2026.5.1"));
        assert!(!is_newer("v2026.4.9", "2026.5.0"));
        assert!(!is_newer("nightly", "2026.5.0"));
    }

    #[test]
    fn releases_url_from_source() {
        let api = releases_url_for("https://github.com/example/fork");
        assert_eq!(
            api,
            "https://api.github.com/repos/example/fork/releases/latest"
        );
        let api = releases_url_for("https://github.com/example/fork.git/");
        assert_eq!(
            api,
            "https://api.github.com/repos/example/fork/releases/latest"
        );
        for other in [
            "https://example.org/example/fork",
            "http://github.com/example/fork",
            "https://github.com/example",
            "https://github.com/example/fork/tree/main",
            "kein-url",
        ] {
            assert_eq!(releases_url_for(other), DEFAULT_RELEASES_URL, "{other}");
        }
    }

    #[test]
    fn link_and_name_are_plain_text() {
        let link = release_link(Some("https://example.org/r/v2026.10.0"), "v2026.10.0");
        assert_eq!(link, "https://example.org/r/v2026.10.0");
        let fallback = release_link(Some("javascript:alert(1)"), "v2026.10.0");
        assert!(fallback.ends_with("/releases/tag/v2026.10.0"));
        assert!(fallback.starts_with("http"), "{fallback}");
        assert_eq!(plain(" a\u{7}b\nc\u{202e} ", 120), "abc");
    }
}
