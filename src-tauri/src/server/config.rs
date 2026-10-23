//! Server-Einstellungen aus der Umgebung (Plan W5). Die Pfade liest
//! [`crate::config::Paths::from_env`].

use std::ffi::OsString;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::error::{Error, Result};

/// Adresse, auf der der Server lauscht (Standard [`DEFAULT_LISTEN`]).
pub const ENV_LISTEN: &str = "KLEIN_BUCH_LISTEN";
/// `1` schaltet die Mountpoint-Prüfung ab — nur für lokale Tests (Plan W5.5).
pub const ENV_ALLOW_UNMOUNTED: &str = "KLEIN_BUCH_ALLOW_UNMOUNTED";
/// Quelle für die AGPL-§13-Angabe (Forks überschreiben sie).
pub const ENV_SOURCE_URL: &str = "KLEIN_BUCH_SOURCE_URL";

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
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_env_with(|name| std::env::var_os(name))
    }

    pub fn from_env_with(get: impl Fn(&str) -> Option<OsString>) -> Result<Self> {
        let listen = match get(ENV_LISTEN).filter(|v| !v.is_empty()) {
            Some(v) => {
                let s = v.to_string_lossy();
                s.parse::<SocketAddr>().map_err(|_| {
                    Error::Config(format!(
                        "{ENV_LISTEN}={s} ist keine Adresse der Form IP:Port — Start abgebrochen."
                    ))
                })?
            }
            None => DEFAULT_LISTEN
                .parse()
                .map_err(|_| Error::Config("Standardadresse ungültig".into()))?,
        };
        let allow_unmounted = get(ENV_ALLOW_UNMOUNTED).is_some_and(|v| v == "1");
        Ok(Self {
            listen,
            startup: StartupOptions {
                allow_unmounted,
                mountinfo: None,
            },
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

    #[test]
    fn defaults_and_overrides() {
        let cfg = ServerConfig::from_env_with(env(&[])).unwrap();
        assert_eq!(cfg.listen.to_string(), DEFAULT_LISTEN);
        assert!(!cfg.startup.allow_unmounted);

        let cfg = ServerConfig::from_env_with(env(&[
            (ENV_LISTEN, "127.0.0.1:18080"),
            (ENV_ALLOW_UNMOUNTED, "1"),
        ]))
        .unwrap();
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
        let cfg = ServerConfig::from_env_with(env(&[(ENV_ALLOW_UNMOUNTED, "true")])).unwrap();
        assert!(!cfg.startup.allow_unmounted);

        assert!(ServerConfig::from_env_with(env(&[(ENV_LISTEN, "8080")])).is_err());
    }
}
