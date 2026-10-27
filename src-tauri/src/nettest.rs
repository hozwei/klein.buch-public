//! Verbindungstests (Plan W8): Port-Allowlist, Kontingent und Probe-Speicher.
//!
//! Test-Commands (SMTP-Verbindungstest, Testmail, SFTP-Probe) sind ein Werkzeug,
//! um aus dem Server heraus beliebige Hosts anzusprechen. Daher: nur
//! freigegebene Ports (SMTP 25/465/587/2525, SFTP 22/2222; Override
//! `KLEIN_BUCH_ALLOWED_TEST_PORTS`), höchstens 10 Tests je Minute, Fehler ohne
//! Unterscheidung „Port zu“ / „Host unbekannt“. Der zuletzt bei einer Probe
//! gesehene SFTP-Host-Fingerprint wird je (Host, Port) 10 Minuten gemerkt;
//! nur dieser darf gespeichert werden.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

pub const ENV_ALLOWED_TEST_PORTS: &str = "KLEIN_BUCH_ALLOWED_TEST_PORTS";
/// Tests je Minute (instanzweit; ein Mandant).
pub const TESTS_PER_MINUTE: usize = 10;
/// So lange gilt eine SFTP-Probe für das Speichern des Fingerprints.
pub const PROBE_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    Smtp,
    Sftp,
}

/// Port-Liste aus `KLEIN_BUCH_ALLOWED_TEST_PORTS` (durch Komma getrennt);
/// leer oder ungesetzt → `None` (Standardlisten). Ungültige Einträge brechen
/// den Start ab.
pub fn ports_from_env_with(
    get: &impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<Option<Vec<u16>>> {
    let Some(v) = get(ENV_ALLOWED_TEST_PORTS).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let v = v.to_string_lossy();
    let mut list = Vec::new();
    for p in v.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match p.parse::<u16>() {
            Ok(n) if n > 0 => list.push(n),
            _ => {
                return Err(Error::Config(format!(
                    "{ENV_ALLOWED_TEST_PORTS}={v} enthält keinen gültigen Port ({p}) — Start abgebrochen."
                )))
            }
        }
    }
    Ok((!list.is_empty()).then_some(list))
}

/// Standardliste je Art.
fn default_ports(kind: TestKind) -> Vec<u16> {
    match kind {
        TestKind::Smtp => vec![25, 465, 587, 2525],
        TestKind::Sftp => vec![22, 2222],
    }
}

/// Einheitliche Meldung für gescheiterte Verbindungen (kein Hinweis, ob der
/// Port geschlossen oder der Host unbekannt ist); Einzelheiten nur im Log.
pub fn connect_failed(host: &str, port: u16, detail: &dyn std::fmt::Display) -> Error {
    tracing::debug!(host, port, error = %detail, "Verbindungstest fehlgeschlagen");
    Error::Domain(format!(
        "Verbindung zu {host}:{port} fehlgeschlagen (Host, Port, Zugangsdaten und Firewall prüfen)."
    ))
}

/// Prüft einen SMTP- bzw. SFTP-Host (W14-017, Lauf F4c): ein Hostname
/// (Labels aus Buchstaben, Ziffern, `-` und `_`, je höchstens 63 Zeichen, nicht
/// mit `-` am Rand, optional ein Punkt am Ende) oder eine IPv4-/IPv6-Adresse
/// ohne Klammern. Leerzeichen, Steuerzeichen und alle anderen Zeichen werden
/// abgelehnt — so kann ein Host weder Log-Zeilen fälschen noch eine URL oder
/// einen Port einschleusen. Die Meldung wiederholt den Wert bewusst nicht.
pub fn check_host(host: &str) -> Result<()> {
    if is_valid_host(host) {
        Ok(())
    } else {
        Err(Error::Domain(
            "Ungültiger Server-Name — erlaubt sind ein Hostname (Buchstaben, Ziffern, Punkt, \
             Bindestrich) oder eine IP-Adresse, ohne Leerzeichen, Port oder Protokoll."
                .into(),
        ))
    }
}

fn is_valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    let name = host.strip_suffix('.').unwrap_or(host);
    !name.is_empty()
        && name.split('.').all(|label| {
            let chars = label.chars().count();
            (1..=63).contains(&chars)
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        })
}

/// Kontingent und Probe-Speicher (im `AppCtx`).
#[derive(Default)]
pub struct NetTests {
    /// Ersetzt die Standardlisten beider Arten (`KLEIN_BUCH_ALLOWED_TEST_PORTS`).
    ports: Mutex<Option<Vec<u16>>>,
    calls: Mutex<VecDeque<Instant>>,
    probes: Mutex<HashMap<(String, u16), (String, Instant)>>,
}

impl NetTests {
    /// Port-Liste setzen (Server-Start; `None` = Standardlisten).
    pub fn set_allowed_ports(&self, ports: Option<Vec<u16>>) {
        *self.ports.lock().unwrap_or_else(PoisonError::into_inner) = ports;
    }

    /// Freigegebene Ports je Art.
    pub fn allowed_ports(&self, kind: TestKind) -> Vec<u16> {
        self.ports
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| default_ports(kind))
    }

    /// Port freigegeben? Sonst Fehler ohne Verbindungsaufbau.
    pub fn check_port(&self, kind: TestKind, port: u16) -> Result<()> {
        let allowed = self.allowed_ports(kind);
        if allowed.contains(&port) {
            Ok(())
        } else {
            let list: Vec<String> = allowed.iter().map(u16::to_string).collect();
            Err(Error::Domain(format!(
                "Port {port} ist nicht freigegeben (erlaubt: {}; Betreiber: {ENV_ALLOWED_TEST_PORTS}).",
                list.join(", ")
            )))
        }
    }

    /// Einen Test zulassen (höchstens [`TESTS_PER_MINUTE`] je Minute).
    pub fn admit(&self) -> Result<()> {
        let now = Instant::now();
        let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
        while calls
            .front()
            .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
        {
            calls.pop_front();
        }
        if calls.len() >= TESTS_PER_MINUTE {
            return Err(Error::Domain(
                "Zu viele Verbindungstests — bitte eine Minute warten.".into(),
            ));
        }
        calls.push_back(now);
        Ok(())
    }

    pub fn remember_probe(&self, host: &str, port: u16, fingerprint: &str) {
        self.probes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                (host.to_ascii_lowercase(), port),
                (fingerprint.to_string(), Instant::now()),
            );
    }

    /// Fingerprint der letzten Probe für (Host, Port), höchstens [`PROBE_TTL`] alt.
    pub fn probed_fingerprint(&self, host: &str, port: u16) -> Option<String> {
        let probes = self.probes.lock().unwrap_or_else(PoisonError::into_inner);
        probes
            .get(&(host.to_ascii_lowercase(), port))
            .filter(|(_, at)| at.elapsed() <= PROBE_TTL)
            .map(|(fp, _)| fp.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_and_quota() {
        let t = NetTests::default();
        assert!(t.check_port(TestKind::Smtp, 587).is_ok());
        assert!(t.check_port(TestKind::Smtp, 22).is_err());
        assert!(t.check_port(TestKind::Sftp, 2222).is_ok());
        assert!(t.check_port(TestKind::Sftp, 8080).is_err());
        t.set_allowed_ports(Some(vec![8025]));
        assert!(t.check_port(TestKind::Smtp, 8025).is_ok());
        assert!(t.check_port(TestKind::Smtp, 587).is_err());
        let env = |v: &'static str| move |_: &str| Some(std::ffi::OsString::from(v));
        assert_eq!(
            ports_from_env_with(&env("25, 2525")).unwrap(),
            Some(vec![25, 2525])
        );
        assert_eq!(ports_from_env_with(&env("")).unwrap(), None);
        assert!(ports_from_env_with(&env("25,x")).is_err());
        assert!(ports_from_env_with(&env("0")).is_err());
        for _ in 0..TESTS_PER_MINUTE {
            t.admit().unwrap();
        }
        assert!(t.admit().is_err());
        t.remember_probe("Backup.Example.org", 22, "SHA256:x");
        assert_eq!(
            t.probed_fingerprint("backup.example.org", 22).as_deref(),
            Some("SHA256:x")
        );
        assert_eq!(t.probed_fingerprint("backup.example.org", 2222), None);
    }

    /// W14-017: nur Hostnamen und IP-Adressen; nichts, was eine Log-Zeile
    /// fälschen oder eine URL/einen Port einschleusen könnte.
    #[test]
    fn host_validation() {
        for ok in [
            "smtp.example.org",
            "smtp.example.org.",
            "mail-1.example.org",
            "backup_nas",
            "localhost",
            "mail.bücher.example",
            "192.0.2.10",
            "2001:db8::25",
            "::1",
        ] {
            assert!(check_host(ok).is_ok(), "{ok}");
        }
        let long_label = format!("{}.example.org", "a".repeat(64));
        let too_long = format!("{}.org", "a.".repeat(130));
        for bad in [
            "",
            ".",
            " smtp.example.org",
            "smtp.example.org x",
            "smtp.example.org\nauth.login.failed ip=192.0.2.1 reason=totp",
            "smtp.example.org\r",
            "smtp\u{1b}[31m.example.org",
            "smtp.example.org:587",
            "smtp://smtp.example.org",
            "user@smtp.example.org",
            "smtp..example.org",
            "-smtp.example.org",
            "smtp-.example.org",
            "[2001:db8::25]",
            "smtp.example.org/pfad",
            long_label.as_str(),
            too_long.as_str(),
        ] {
            assert!(check_host(bad).is_err(), "{bad:?}");
        }
    }
}
