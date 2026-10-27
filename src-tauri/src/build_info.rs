//! Build- und Quellangaben (AGPL §13, Plan W12, ADR 0014/0039).
//!
//! Drei Werte kommen zur Übersetzungszeit aus der Umgebung des Builds
//! (`option_env!`, im Image als späte `ARG`/`ENV` vor `cargo build`):
//!
//! - `KLEINBUCH_SOURCE_URL` — Repository des Builds, z. B.
//!   `https://github.com/hozwei/klein.buch-public`
//! - `KLEINBUCH_BUILD_COMMIT` — voller Commit-SHA
//! - `KLEINBUCH_BUILD_TAG` — Release-Tag (`v<CalVer>`), nur bei Release-Builds
//!
//! Betreiber mit eigenen Änderungen setzen zur Laufzeit
//! `KLEIN_BUCH_SOURCE_URL` auf ihren Quellstand. Der Link zeigt immer auf einen
//! festen Stand (`/tree/<commit>` bzw. `/releases/tag/<tag>`), nie auf einen
//! Zweig; ohne bekannten Commit (Entwicklungs-Build) gibt es keinen Link.
//! `GET /api/public/info` und `app_info` lesen beide [`current`].

use serde::Serialize;

/// SPDX-Kennung der App-Lizenz.
pub const LICENSE_SPDX: &str = "AGPL-3.0-or-later";
/// Laufzeit-Override der Quell-URL (Forks mit eigenen Änderungen).
pub const ENV_SOURCE_URL: &str = "KLEIN_BUCH_SOURCE_URL";
/// Quell-URL, wenn weder Build noch Laufzeit eine liefern.
pub const DEFAULT_SOURCE_URL: &str = "https://github.com/hozwei/klein.buch-public";
/// Anzeige ohne bekannten Commit (dieselbe Formulierung wie in der Oberfläche).
pub const DEV_SOURCE_TEXT: &str = "Entwicklungs-Build, Quellstand unbekannt";

/// Quellangaben der laufenden Version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub version: String,
    /// Voller Commit-SHA des Builds; `None` im Entwicklungs-Build.
    pub commit: Option<String>,
    /// Release-Tag; `None` außerhalb von Release-Builds.
    pub tag: Option<String>,
    pub source_url: String,
    /// `<sourceUrl>/releases/tag/<tag>` bzw. `<sourceUrl>/tree/<commit>`;
    /// `None` ohne bekannten Commit.
    pub source_link: Option<String>,
    pub license_spdx: String,
}

/// Commit-SHA: 7–64 Hex-Zeichen (SHA-1 bzw. SHA-256), sonst `None`
/// (`dev`, `unbekannt`, leer).
pub fn clean_commit(raw: Option<&str>) -> Option<String> {
    let v = raw?.trim();
    let ok = (7..=64).contains(&v.len()) && v.chars().all(|c| c.is_ascii_hexdigit());
    ok.then(|| v.to_ascii_lowercase())
}

/// Tag: beginnt mit Buchstabe oder Ziffer, danach nur `A-Za-z0-9._+-`,
/// höchstens 100 Zeichen (er landet im Link-Pfad).
pub fn clean_tag(raw: Option<&str>) -> Option<String> {
    let v = raw?.trim();
    let mut chars = v.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-');
    let rest_ok = chars.all(allowed);
    (first_ok && rest_ok && v.len() <= 100).then(|| v.to_string())
}

/// Quell-URL: `http://` bzw. `https://` mit Host, ohne Benutzerangaben, Query
/// und Fragment; zurück kommt die normalisierte Form ohne abschließenden
/// Schrägstrich. Sonst `None` (Tippfehler wie `https:/…` warnen beim Serverstart).
pub fn clean_source_url(raw: Option<&str>) -> Option<String> {
    let v = raw?.trim();
    if v.is_empty() || v.len() > 512 || v.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = v.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return None;
    }
    let url = url::Url::parse(v).ok()?;
    let ok = matches!(url.scheme(), "https" | "http")
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    ok.then(|| url.as_str().trim_end_matches('/').to_string())
}

/// Commit des Builds (`KLEINBUCH_BUILD_COMMIT`).
pub fn build_commit() -> Option<String> {
    clean_commit(option_env!("KLEINBUCH_BUILD_COMMIT"))
}

/// Tag des Builds (`KLEINBUCH_BUILD_TAG`).
pub fn build_tag() -> Option<String> {
    clean_tag(option_env!("KLEINBUCH_BUILD_TAG"))
}

/// Quell-URL des Builds (`KLEINBUCH_SOURCE_URL`).
pub fn build_source_url() -> Option<String> {
    clean_source_url(option_env!("KLEINBUCH_SOURCE_URL"))
}

/// Wirksame Quell-URL: Laufzeit-Override, sonst Build-Wert, sonst
/// [`DEFAULT_SOURCE_URL`]. Ein ungültiger Override zählt nicht (Warnung beim
/// Start, [`source_url_warning`]).
pub fn source_url() -> String {
    clean_source_url(std::env::var(ENV_SOURCE_URL).ok().as_deref())
        .or_else(build_source_url)
        .unwrap_or_else(|| DEFAULT_SOURCE_URL.to_string())
}

/// Warnung, wenn `KLEIN_BUCH_SOURCE_URL` gesetzt, aber unbrauchbar ist.
pub fn source_url_warning() -> Option<String> {
    let raw = std::env::var(ENV_SOURCE_URL).ok()?;
    if raw.trim().is_empty() || clean_source_url(Some(&raw)).is_some() {
        return None;
    }
    Some(format!(
        "{ENV_SOURCE_URL} ist keine http(s)-Adresse ohne Query/Fragment — es gilt {}",
        build_source_url().unwrap_or_else(|| DEFAULT_SOURCE_URL.to_string())
    ))
}

/// Link auf den exakten Quellstand: Release-Tag, sonst Commit; ohne Commit
/// keiner (nie ein Zweig wie `main`).
pub fn source_link(source_url: &str, commit: Option<&str>, tag: Option<&str>) -> Option<String> {
    let commit = commit?;
    Some(match tag {
        Some(tag) => format!("{source_url}/releases/tag/{tag}"),
        None => format!("{source_url}/tree/{commit}"),
    })
}

/// Setzt die Angaben aus Einzelwerten zusammen (testbar ohne Build-Umgebung).
pub fn source_info(
    version: &str,
    commit: Option<String>,
    tag: Option<String>,
    source_url: String,
) -> SourceInfo {
    // Ein Tag ohne Commit ist kein nachprüfbarer Stand.
    let tag = commit.as_ref().and(tag);
    let source_link = source_link(&source_url, commit.as_deref(), tag.as_deref());
    SourceInfo {
        version: version.to_string(),
        commit,
        tag,
        source_url,
        source_link,
        license_spdx: LICENSE_SPDX.to_string(),
    }
}

/// Angaben der laufenden Version.
pub fn current() -> SourceInfo {
    source_info(
        env!("CARGO_PKG_VERSION"),
        build_commit(),
        build_tag(),
        source_url(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn commit_and_tag_validation() {
        assert_eq!(clean_commit(Some(SHA)).as_deref(), Some(SHA));
        let upper = clean_commit(Some(&SHA.to_ascii_uppercase()));
        assert_eq!(upper.as_deref(), Some(SHA));
        for bad in ["", "dev", "unbekannt", "abc", "0123456789abcdeg", "main"] {
            assert_eq!(clean_commit(Some(bad)), None, "{bad}");
        }
        assert_eq!(clean_commit(None), None);
        let tag = clean_tag(Some("v2026.10.0"));
        assert_eq!(tag.as_deref(), Some("v2026.10.0"));
        for bad in ["", "-v1", "v1/../x", "v1 2", "v1?x", "<b>"] {
            assert_eq!(clean_tag(Some(bad)), None, "{bad}");
        }
    }

    #[test]
    fn source_url_validation() {
        assert_eq!(
            clean_source_url(Some("https://example.org/fork/klein-buch/")).as_deref(),
            Some("https://example.org/fork/klein-buch")
        );
        for bad in [
            "",
            "javascript:alert(1)",
            "ftp://example.org/x",
            "https://user:pw@example.org/x",
            "https://example.org/x?y=1",
            "https://example.org/x#y",
            "https://example.org/a b",
            "https:/example.org/fork",
            "https:example.org/fork",
        ] {
            assert_eq!(clean_source_url(Some(bad)), None, "{bad}");
        }
        // normalisiert (Host klein) — der Link beginnt immer mit http(s)://
        assert_eq!(
            clean_source_url(Some("HTTPS://Example.ORG/fork")).as_deref(),
            Some("https://example.org/fork")
        );
    }

    #[test]
    fn link_points_to_exact_state_never_to_a_branch() {
        let url = "https://example.org/kb".to_string();
        let dev = source_info("2026.10.0", None, Some("v2026.10.0".into()), url.clone());
        assert_eq!(dev.source_link, None, "ohne Commit kein Link");
        assert_eq!(dev.tag, None, "Tag ohne Commit zählt nicht");
        let commit = source_info("2026.10.0", Some(SHA.into()), None, url.clone());
        assert_eq!(
            commit.source_link.as_deref(),
            Some(format!("https://example.org/kb/tree/{SHA}").as_str())
        );
        let tagged = source_info(
            "2026.10.0",
            Some(SHA.into()),
            Some("v2026.10.0".into()),
            url,
        );
        assert_eq!(
            tagged.source_link.as_deref(),
            Some("https://example.org/kb/releases/tag/v2026.10.0")
        );
        assert_eq!(tagged.license_spdx, LICENSE_SPDX);
        for info in [&dev, &commit, &tagged] {
            let link = info.source_link.clone().unwrap_or_default();
            assert!(!link.contains("/main"), "{link}");
        }
    }

    #[test]
    fn current_uses_the_build_values() {
        let info = current();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.commit, build_commit());
        let json = serde_json::to_value(&info).unwrap();
        for key in [
            "version",
            "commit",
            "tag",
            "sourceUrl",
            "sourceLink",
            "licenseSpdx",
        ] {
            assert!(json.get(key).is_some(), "{key} fehlt: {json}");
        }
    }
}
