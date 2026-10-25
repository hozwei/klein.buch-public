//! HTTP-Policy (Plan W6, ADR 0039 Standardentscheidungen): kein TLS im
//! Prozess, TLS terminiert der Reverse-Proxy des Betreibers.
//!
//! - `KLEIN_BUCH_PUBLIC_URL` (Pflicht): Host der Anfrage muss passen, sonst
//!   421 (Ausnahme `/healthz` über Loopback); ein vorhandener `Origin` muss bei
//!   ändernden Methoden der öffentliche Origin sein, sonst 403.
//! - `KLEIN_BUCH_TRUSTED_PROXIES` (CIDR-Liste, Standard leer): nur von dort
//!   werden `X-Forwarded-For/-Proto/-Host` ausgewertet; Client-IP = von rechts
//!   erster Eintrag außerhalb der Liste. Kein Extractor, der Headern blind
//!   vertraut.
//! - Einrichten und Anmelden nur über Loopback, über einen vertrauten Proxy
//!   mit `X-Forwarded-Proto: https` oder mit `KLEIN_BUCH_INSECURE_HTTP=1` (nur
//!   Tests und CI).

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;

use super::errors::ApiError;
use super::state::ServerState;
use crate::error::{Error, Result};

/// Öffentliche Adresse der Instanz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    pub scheme: String,
    pub host: String,
    /// Nur gesetzt, wenn vom Standard-Port des Schemas abweichend.
    pub port: Option<u16>,
    /// `scheme://host[:port]` (für den `Origin`-Vergleich).
    pub origin: String,
}

impl PublicUrl {
    pub fn parse(raw: &str) -> Result<Self> {
        let bad = |why: &str| {
            Error::Config(format!(
                "KLEIN_BUCH_PUBLIC_URL={raw} ungültig ({why}) — erwartet z. B. https://buch.example.org"
            ))
        };
        let url = url::Url::parse(raw.trim()).map_err(|_| bad("keine URL"))?;
        let scheme = url.scheme().to_string();
        if scheme != "https" && scheme != "http" {
            return Err(bad("Schema muss https oder http sein"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(bad("ohne Benutzerangaben"));
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(bad("ohne Pfad, Query und Fragment"));
        }
        let host = url
            .host_str()
            .ok_or_else(|| bad("Host fehlt"))?
            .to_ascii_lowercase();
        let port = url.port();
        let origin = match port {
            Some(p) => format!("{scheme}://{host}:{p}"),
            None => format!("{scheme}://{host}"),
        };
        Ok(Self {
            scheme,
            host,
            port,
            origin,
        })
    }

    fn default_port(&self) -> u16 {
        if self.scheme == "https" {
            443
        } else {
            80
        }
    }

    /// Passt ein `Host`-Wert (ggf. mit Port) zur öffentlichen Adresse?
    pub fn host_matches(&self, value: &str) -> bool {
        let value = value.trim().to_ascii_lowercase();
        let (host, port) = split_host_port(&value);
        if host != self.host {
            return false;
        }
        let expected = self.port.unwrap_or_else(|| self.default_port());
        match port {
            Some(p) => p == expected,
            None => self.port.is_none(),
        }
    }
}

/// `host[:port]` bzw. `[v6]:port` zerlegen.
fn split_host_port(value: &str) -> (&str, Option<u16>) {
    if let Some(rest) = value.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let host = &value[..end + 2];
            let port = rest[end + 1..]
                .strip_prefix(':')
                .and_then(|p| p.parse().ok());
            return (host, port);
        }
    }
    match value.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => match p.parse() {
            Ok(port) => (h, Some(port)),
            Err(_) => (value, None),
        },
        _ => (value, None),
    }
}

/// Einstellungen der HTTP-Policy.
#[derive(Debug, Clone)]
pub struct HttpPolicy {
    pub public_url: PublicUrl,
    pub trusted_proxies: Vec<IpNet>,
    pub insecure_http: bool,
}

/// CIDR-Liste (Komma oder Leerzeichen getrennt); einzelne Adressen gelten als
/// /32 bzw. /128.
pub fn parse_trusted_proxies(raw: &str) -> Result<Vec<IpNet>> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<IpNet>()
                .or_else(|_| s.parse::<IpAddr>().map(IpNet::from))
                .map_err(|_| {
                    Error::Config(format!(
                        "KLEIN_BUCH_TRUSTED_PROXIES: „{s}“ ist kein CIDR-Bereich"
                    ))
                })
        })
        .collect()
}

/// Ausgewertete Gegenstelle einer Anfrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientInfo {
    /// Client-IP (über vertraute Proxys aufgelöst).
    pub ip: IpAddr,
    /// TCP-Gegenstelle ist Loopback.
    pub loopback: bool,
    /// TCP-Gegenstelle ist ein vertrauter Proxy.
    pub via_trusted_proxy: bool,
    /// Anfrage kam laut vertrautem Proxy über HTTPS.
    pub https: bool,
}

/// Client-IP als Antwort-Extension für das Zugriffsprotokoll (W14-018), auch
/// bei abgewiesenen Anfragen (421/403) — anders als [`ClientInfo`], das nur
/// angenommene Anfragen tragen (HSTS).
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub IpAddr);

fn header_values<'a>(headers: &'a HeaderMap, name: &str) -> Vec<&'a str> {
    headers
        .get_all(name)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect()
}

impl HttpPolicy {
    fn trusted(&self, ip: IpAddr) -> bool {
        self.trusted_proxies.iter().any(|net| net.contains(&ip))
    }

    /// Wertet Gegenstelle und (nur von vertrauten Proxys) Weiterleitungs-
    /// Header aus. Liefert außerdem den maßgeblichen Host.
    pub fn client_info(
        &self,
        peer: Option<IpAddr>,
        headers: &HeaderMap,
    ) -> (ClientInfo, Option<String>) {
        let peer = peer
            .map(|ip| ip.to_canonical())
            .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        // Loopback zählt nur ohne Weiterleitungs-Header: ein Reverse-Proxy auf
        // demselben Host (Loopback-Gegenstelle) leitet sonst Klartext-HTTP aus
        // dem Netz als „lokal“ durch (Review W6).
        let forwarded = [
            "x-forwarded-for",
            "x-forwarded-proto",
            "x-forwarded-host",
            "forwarded",
        ]
        .iter()
        .any(|h| headers.contains_key(*h));
        let loopback = peer.is_loopback() && !forwarded;
        let host_header = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        if !self.trusted(peer) {
            return (
                ClientInfo {
                    ip: peer,
                    loopback,
                    via_trusted_proxy: false,
                    https: false,
                },
                host_header,
            );
        }
        // Ein unlesbarer Eintrag bricht die Kette: dann zählt der Proxy selbst
        // (sonst würde ein Eintrag links davon zur Client-IP, Review W6).
        let raw = header_values(headers, "x-forwarded-for");
        let parsed: Option<Vec<IpAddr>> = raw
            .iter()
            .map(|v| v.parse::<IpAddr>().ok().map(|ip| ip.to_canonical()))
            .collect();
        let chain = parsed.unwrap_or_default();
        let ip = chain
            .iter()
            .rev()
            .find(|ip| !self.trusted(**ip))
            .or_else(|| chain.first())
            .copied()
            .unwrap_or(peer);
        let https = header_values(headers, "x-forwarded-proto")
            .last()
            .is_some_and(|p| p.eq_ignore_ascii_case("https"));
        let host = header_values(headers, "x-forwarded-host")
            .last()
            .map(|s| s.to_string())
            .or(host_header);
        (
            ClientInfo {
                ip,
                loopback,
                via_trusted_proxy: true,
                https,
            },
            host,
        )
    }

    /// Einrichten/Anmelden erlaubt? (Loopback, vertrauter Proxy + HTTPS,
    /// oder ausdrücklich unsicher für Tests.)
    pub fn auth_transport_ok(&self, info: &ClientInfo) -> bool {
        info.loopback || (info.via_trusted_proxy && info.https) || self.insecure_http
    }
}

fn is_unsafe(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Middleware: Host (421), Origin (403), `ClientInfo` als Extension an
/// Anfrage und Antwort (für HSTS).
pub async fn http_policy(State(st): State<Arc<ServerState>>, req: Request, next: Next) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let (info, host) = st.policy.client_info(peer, req.headers());
    let mut response = admit(&st.policy, info, host, req, next).await;
    response.extensions_mut().insert(ClientIp(info.ip));
    response
}

/// Host- und Origin-Prüfung, dann der eigentliche Aufruf.
async fn admit(
    policy: &HttpPolicy,
    info: ClientInfo,
    host: Option<String>,
    mut req: Request,
    next: Next,
) -> Response {
    let healthz_local = req.uri().path() == "/healthz" && info.loopback;
    if !healthz_local {
        let ok = host
            .as_deref()
            .is_some_and(|h| policy.public_url.host_matches(h));
        if !ok {
            return ApiError::new(
                StatusCode::MISDIRECTED_REQUEST,
                "misdirected",
                "Unbekannter Host.",
            )
            .into_response();
        }
        if is_unsafe(req.method()) {
            if let Some(origin) = req.headers().get(header::ORIGIN) {
                let matches = origin
                    .to_str()
                    .is_ok_and(|o| o.eq_ignore_ascii_case(&policy.public_url.origin));
                if !matches {
                    return ApiError::new(
                        StatusCode::FORBIDDEN,
                        "origin_mismatch",
                        "Anfrage von fremder Herkunft abgelehnt.",
                    )
                    .into_response();
                }
            }
        }
    }
    req.extensions_mut().insert(info);
    let mut response = next.run(req).await;
    response.extensions_mut().insert(info);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn policy(trusted: &str) -> HttpPolicy {
        HttpPolicy {
            public_url: PublicUrl::parse("https://buch.example.org").unwrap(),
            trusted_proxies: parse_trusted_proxies(trusted).unwrap(),
            insecure_http: false,
        }
    }

    #[test]
    fn public_url_and_host_matching() {
        let p = PublicUrl::parse("https://Buch.Example.org/").unwrap();
        assert_eq!(p.origin, "https://buch.example.org");
        assert!(p.host_matches("buch.example.org"));
        assert!(p.host_matches("BUCH.example.org:443"));
        assert!(!p.host_matches("buch.example.org:8443"));
        assert!(!p.host_matches("evil.example.org"));
        let p = PublicUrl::parse("http://127.0.0.1:18085").unwrap();
        assert!(p.host_matches("127.0.0.1:18085"));
        assert!(!p.host_matches("127.0.0.1"));
        let p = PublicUrl::parse("http://[::1]:8080").unwrap();
        assert!(p.host_matches("[::1]:8080"));
        for bad in [
            "ftp://x.example.org",
            "https://x.example.org/pfad",
            "buch",
            "https://u:p@x.example.org",
        ] {
            assert!(PublicUrl::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn forwarded_headers_only_from_trusted_proxies() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_static("buch.example.org"));
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.9, 198.51.100.7"),
        );
        h.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        // nicht vertraut: XFF ignoriert, kein https
        let (info, _) = policy("").client_info(Some("192.0.2.5".parse().unwrap()), &h);
        assert_eq!(info.ip, "192.0.2.5".parse::<IpAddr>().unwrap());
        assert!(!info.https && !info.via_trusted_proxy);
        // vertrauter Proxy + vertrauter Zwischen-Proxy: Client = erster fremder von rechts
        let (info, host) = policy("192.0.2.0/24, 198.51.100.7")
            .client_info(Some("192.0.2.5".parse().unwrap()), &h);
        assert_eq!(info.ip, "203.0.113.9".parse::<IpAddr>().unwrap());
        assert!(info.https && info.via_trusted_proxy);
        assert_eq!(host.as_deref(), Some("buch.example.org"));
        // gefälschter Eintrag links wird nicht genommen
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.200, 203.0.113.9"),
        );
        let (info, _) = policy("192.0.2.0/24").client_info(Some("192.0.2.5".parse().unwrap()), &h);
        assert_eq!(info.ip, "203.0.113.9".parse::<IpAddr>().unwrap());
        assert!(parse_trusted_proxies("kein-netz").is_err());
        // unlesbarer Eintrag → Proxy selbst gilt als Client
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.66, kaputt"),
        );
        let (info, _) = policy("192.0.2.0/24").client_info(Some("192.0.2.5".parse().unwrap()), &h);
        assert_eq!(info.ip, "192.0.2.5".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn loopback_with_forwarding_headers_is_not_local() {
        let mut h = HeaderMap::new();
        let (info, _) = policy("").client_info(Some("127.0.0.1".parse().unwrap()), &h);
        assert!(info.loopback);
        h.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.9"));
        let (info, _) = policy("").client_info(Some("127.0.0.1".parse().unwrap()), &h);
        assert!(!info.loopback, "lokaler Proxy ist nicht „lokal“");
        assert!(!policy("").auth_transport_ok(&info));
    }

    #[test]
    fn auth_transport_rule() {
        let p = policy("192.0.2.0/24");
        let mut info = ClientInfo {
            ip: "203.0.113.9".parse().unwrap(),
            loopback: false,
            via_trusted_proxy: false,
            https: false,
        };
        assert!(!p.auth_transport_ok(&info), "HTTP ohne Proxy");
        info.via_trusted_proxy = true;
        assert!(!p.auth_transport_ok(&info), "Proxy ohne HTTPS");
        info.https = true;
        assert!(p.auth_transport_ok(&info));
        let local = ClientInfo {
            loopback: true,
            ..info
        };
        assert!(p.auth_transport_ok(&local));
    }
}
