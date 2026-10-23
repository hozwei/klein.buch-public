//! Server-Einstellungen aus der Umgebung (Plan W5/W6). Die Pfade liest
//! [`crate::config::Paths::from_env`].

use std::ffi::OsString;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::policy::{parse_trusted_proxies, HttpPolicy, PublicUrl};
use crate::error::{Error, Result};

/// Adresse, auf der der Server lauscht (Standard [`DEFAULT_LISTEN`]).
pub const ENV_LISTEN: &str = "KLEIN_BUCH_LISTEN";
/// `1` schaltet die Mountpoint-Prüfung ab — nur für lokale Tests (Plan W5.5).
pub const ENV_ALLOW_UNMOUNTED: &str = "KLEIN_BUCH_ALLOW_UNMOUNTED";
/// Quelle für die AGPL-§13-Angabe (Forks überschreiben sie).
pub const ENV_SOURCE_URL: &str = "KLEIN_BUCH_SOURCE_URL";
/// Öffentliche Adresse der Instanz (Pflicht, Plan W6), z. B. `https://buch.example.org`.
pub const ENV_PUBLIC_URL: &str = "KLEIN_BUCH_PUBLIC_URL";
/// CIDR-Liste vertrauter Reverse-Proxys (Standard leer).
pub const ENV_TRUSTED_PROXIES: &str = "KLEIN_BUCH_TRUSTED_PROXIES";
/// `1` erlaubt Einrichten/Anmelden über unverschlüsseltes HTTP — nur Tests und CI.
pub const ENV_INSECURE_HTTP: &str = "KLEIN_BUCH_INSECURE_HTTP";

pub const DEFAULT_LISTEN: &str = "0.0.0.0:8080";
pub const DEFAULT_SOURCE_URL: &str = "https://github.com/hozwei/klein.buch-public";

/// Schalter für die Prüfungen vor dem Port-Bind.
#[derive(Debug, Clone, Default)]
pub struct StartupOptions {
    /// Mountpoint-Prüfung für `/data` und `/inputs` abschalten (nur lokal).
    pub allow_unmounted: bool,
    /// Inhalt von `/proc/self/mountinfo` (Tests); `None` liest die Datei.
    pub mountinfo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen: SocketAddr,
    pub startup: StartupOptions,
    pub policy: HttpPolicy,
    /// Upload-Grenzen (`KLEIN_BUCH_MIN_FREE_BYTES`, `KLEIN_BUCH_MAX_RESTORE_BYTES`, W7).
    pub uploads: crate::uploads::UploadLimits,
    /// Ports für Verbindungstests (`KLEIN_BUCH_ALLOWED_TEST_PORTS`, W8); `None` = Standard.
    pub test_ports: Option<Vec<u16>>,
}

/// Adresse aus `KLEIN_BUCH_LISTEN` (Standard [`DEFAULT_LISTEN`]).
pub fn listen_from_env_with(get: &impl Fn(&str) -> Option<OsString>) -> Result<SocketAddr> {
    match get(ENV_LISTEN).filter(|v| !v.is_empty()) {
        Some(v) => {
            let s = v.to_string_lossy();
            s.parse::<SocketAddr>().map_err(|_| {
                Error::Config(format!(
                    "{ENV_LISTEN}={s} ist keine Adresse der Form IP:Port — Start abgebrochen."
                ))
            })
        }
        None => DEFAULT_LISTEN
            .parse()
            .map_err(|_| Error::Config("Standardadresse ungültig".into())),
    }
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_env_with(|name| std::env::var_os(name))
    }

    pub fn from_env_with(get: impl Fn(&str) -> Option<OsString>) -> Result<Self> {
        let listen = listen_from_env_with(&get)?;
        let allow_unmounted = get(ENV_ALLOW_UNMOUNTED).is_some_and(|v| v == "1");
        let public_url = get(ENV_PUBLIC_URL)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                Error::Config(format!(
                    "{ENV_PUBLIC_URL} fehlt (öffentliche Adresse, z. B. https://buch.example.org) — \
                     Start abgebrochen."
                ))
            })?;
        let public_url = PublicUrl::parse(&public_url.to_string_lossy())?;
        let trusted_proxies = match get(ENV_TRUSTED_PROXIES) {
            Some(v) => parse_trusted_proxies(&v.to_string_lossy())?,
            None => Vec::new(),
        };
        let insecure_http = get(ENV_INSECURE_HTTP).is_some_and(|v| v == "1");
        let uploads = crate::uploads::UploadLimits::from_env_with(&get)?;
        let test_ports = crate::nettest::ports_from_env_with(&get)?;
        Ok(Self {
            listen,
            startup: StartupOptions {
                allow_unmounted,
                mountinfo: None,
            },
            policy: HttpPolicy {
                public_url,
                trusted_proxies,
                insecure_http,
            },
            uploads,
            test_ports,
        })
    }
}

/// Adresse für den Subcommand `healthcheck`: bei `0.0.0.0`/`::` Loopback,
/// sonst die gebundene Adresse selbst (eine feste IP ist über Loopback nicht
/// erreichbar; den Zustand liefert `/healthz` dann nur ohne Körper).
pub fn healthcheck_addr(listen: SocketAddr) -> SocketAddr {
    let ip = match listen.ip() {
        IpAddr::V4(v4) if v4.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(v6) if v6.is_unspecified() => IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        other => other,
    };
    SocketAddr::new(ip, listen.port())
}

/// Quell-URL für `/api/public/info` (AGPL §13).
pub fn source_url() -> String {
    std::env::var(ENV_SOURCE_URL)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_SOURCE_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    const URL: (&str, &str) = (ENV_PUBLIC_URL, "https://buch.example.org");

    #[test]
    fn defaults_and_overrides() {
        assert!(
            ServerConfig::from_env_with(env(&[])).is_err(),
            "PUBLIC_URL ist Pflicht"
        );
        let cfg = ServerConfig::from_env_with(env(&[URL])).unwrap();
        assert_eq!(cfg.listen.to_string(), DEFAULT_LISTEN);
        assert!(!cfg.startup.allow_unmounted);
        assert!(!cfg.policy.insecure_http);
        assert!(cfg.policy.trusted_proxies.is_empty());

        let cfg = ServerConfig::from_env_with(env(&[
            URL,
            (ENV_LISTEN, "127.0.0.1:18080"),
            (ENV_ALLOW_UNMOUNTED, "1"),
            (ENV_TRUSTED_PROXIES, "192.0.2.0/24 198.51.100.7"),
            (ENV_INSECURE_HTTP, "1"),
        ]))
        .unwrap();
        assert_eq!(cfg.policy.trusted_proxies.len(), 2);
        assert!(cfg.policy.insecure_http);
        assert_eq!(cfg.listen.port(), 18080);
        assert!(cfg.startup.allow_unmounted);
        assert_eq!(healthcheck_addr(cfg.listen).to_string(), "127.0.0.1:18080");
        assert_eq!(
            healthcheck_addr("0.0.0.0:8080".parse().unwrap()).to_string(),
            "127.0.0.1:8080"
        );
        assert_eq!(
            healthcheck_addr("192.0.2.10:8080".parse().unwrap()).to_string(),
            "192.0.2.10:8080"
        );

        // Nur genau „1“ schaltet die Prüfung ab.
        let cfg = ServerConfig::from_env_with(env(&[URL, (ENV_ALLOW_UNMOUNTED, "true")])).unwrap();
        assert!(!cfg.startup.allow_unmounted);

        assert!(ServerConfig::from_env_with(env(&[URL, (ENV_LISTEN, "8080")])).is_err());
    }
}
