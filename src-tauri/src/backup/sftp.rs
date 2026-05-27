//! SFTP-Backup-Ziel (ADR 0034, Block G1-BKP.3).
//!
//! Schicht: **Imperative Shell** für das Netzwerk-I/O + ein kleiner **pure core**
//! (Pfad-/URI-Helfer) am Anfang. Lädt die verschlüsselte `.kbk`-Datei per
//! SSH/SFTP (`russh` + `russh-sftp`) auf einen eigenen Server.
//!
//! ## Hard-Rules
//!
//! - **Passwort nie in DB/Log/audit.** Es lebt ausschließlich im OS-Keychain
//!   (Windows Credential Manager / macOS Keychain / Linux Secret Service), genau
//!   wie die SMTP-Passphrase (siehe [`crate::mail::keyring`]). Es gibt genau ein
//!   SFTP-Ziel, daher eine feste Service-ID.
//! - **Host-Key-Pinning (MITM-Schutz).** Der Server-Host-Key wird über einen
//!   gepinnten SHA-256-Fingerprint (`SHA256:…`) verifiziert. Ein Upload ohne
//!   passenden Pin wird **abgelehnt**. Den Fingerprint ermittelt der einmalige
//!   Verbindungstest ([`probe`]); der Nutzer bestätigt ihn (TOFU) und er wird im
//!   Ziel ([`super::target::BackupTarget::Sftp`]) gespeichert.
//! - **Nur Passwort-Auth in v1.0.** Public-Key-/Agent-Auth ist bewusst
//!   verschoben (Follow-up) — siehe G1-BKP.3-Report.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::PublicKey;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};

/// Gesamt-Timeout für Handshake + Auth + SFTP-Open.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Keychain-Service-ID des SFTP-Backup-Passworts (es gibt genau EIN Ziel).
const KEYRING_SERVICE: &str = "kleinbuch::backup::sftp";
/// Fixer `user`-Teil des Keychain-Eintrags.
const KEYRING_USER: &str = "sftp-password";

/// Name der temporären Probe-Datei für den Schreibrechte-Test.
const PROBE_FILE: &str = ".klein-buch-write-test";

// ---------------------------------------------------------------------------
// Pure core (testbar, ohne Netzwerk)
// ---------------------------------------------------------------------------

/// Normalisiert den Remote-Ordner: trimmt + entfernt nachstehende `/`.
/// Leer → `"."` (Home-Verzeichnis des SFTP-Users).
pub fn normalize_remote_dir(dir: &str) -> String {
    let d = dir.trim().trim_end_matches('/');
    if d.is_empty() {
        ".".to_string()
    } else {
        d.to_string()
    }
}

/// Fügt Remote-Ordner + Dateiname zu einem Remote-Pfad zusammen (pure).
pub fn remote_join(dir: &str, file_name: &str) -> String {
    let d = normalize_remote_dir(dir);
    if d == "." {
        file_name.to_string()
    } else {
        format!("{d}/{file_name}")
    }
}

/// Baut die `sftp://`-URI für `backup_history.target_path` (rein informativ —
/// der lokale Restore-Wizard liest sie nicht; Off-Site-Restore = Datei erst
/// herunterladen, siehe G1-BKP.3-Report).
pub fn sftp_uri(user: &str, host: &str, port: u16, remote_path: &str) -> String {
    let p = if remote_path.starts_with('/') {
        remote_path.to_string()
    } else {
        format!("/{remote_path}")
    };
    format!("sftp://{user}@{host}:{port}{p}")
}

/// SHA-256-Host-Key-Fingerprint im OpenSSH-Format (`SHA256:…`).
///
/// `Default::default()` liefert [`HashAlg::Sha256`]; wir nennen den ssh-key-Typ
/// bewusst NICHT, damit der Code unabhängig davon kompiliert, ob `russh` die
/// Upstream-`ssh-key`-Crate oder ihren internen Fork re-exportiert.
fn fingerprint_of(key: &PublicKey) -> String {
    key.fingerprint(Default::default()).to_string()
}

// ---------------------------------------------------------------------------
// Keychain (Imperative Shell) — Passwort nie in DB/Log/audit
// ---------------------------------------------------------------------------

/// Speichert das SFTP-Passwort im OS-Credential-Manager.
pub fn set_password(password: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| Error::Backup(format!("Keychain-Eintrag nicht erstellbar: {e}")))?;
    entry
        .set_password(password)
        .map_err(|e| Error::Backup(format!("SFTP-Passwort nicht speicherbar: {e}")))?;
    Ok(())
}

/// Liest das SFTP-Passwort. `Ok(None)`, wenn keines hinterlegt ist; andere
/// Store-Fehler werden propagiert (defekter Keychain ≠ „kein Passwort").
pub fn get_password() -> Result<Option<String>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| Error::Backup(format!("Keychain-Eintrag nicht lesbar: {e}")))?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Backup(format!("SFTP-Passwort nicht lesbar: {e}"))),
    }
}

/// Löscht das SFTP-Passwort. Idempotent (fehlender Eintrag = Ok).
pub fn delete_password() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| Error::Backup(format!("Keychain-Eintrag nicht adressierbar: {e}")))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Backup(format!("SFTP-Passwort nicht löschbar: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// Host-Key-Verifier (russh-Client-Handler)
// ---------------------------------------------------------------------------

/// Prüft den Server-Host-Key. Hält den gesehenen Fingerprint in `seen` fest,
/// damit der Aufrufer ihn nach dem Handshake lesen (Pinning/Anzeige) bzw. eine
/// Abweichung als MITM-Fehler melden kann.
struct HostKeyVerifier {
    /// Gepinnter Fingerprint; `None` heißt „noch keiner hinterlegt".
    pinned: Option<String>,
    /// Im Probe-Modus `true`: unbekannten Key akzeptieren (nur um ihn anzuzeigen,
    /// es verlässt KEIN Backup den Rechner). Im Upload-Modus `false`.
    accept_unknown: bool,
    /// Der vom Server präsentierte Fingerprint (vom Handshake gefüllt).
    seen: Arc<Mutex<Option<String>>>,
}

impl client::Handler for HostKeyVerifier {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        let fp = fingerprint_of(server_public_key);
        *self.seen.lock().expect("seen mutex poisoned") = Some(fp.clone());
        Ok(match &self.pinned {
            Some(p) => *p == fp,
            None => self.accept_unknown,
        })
    }
}

// ---------------------------------------------------------------------------
// Verbindungsaufbau (Shell)
// ---------------------------------------------------------------------------

/// Baut TCP → SSH-Handshake (mit Host-Key-Check) → Passwort-Auth → SFTP-Subsystem.
async fn open(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    handler: HostKeyVerifier,
) -> Result<(Handle<HostKeyVerifier>, SftpSession)> {
    let config = Arc::new(client::Config::default());
    let mut session = client::connect(config, (host, port), handler)
        .await
        .map_err(|e| {
            Error::Backup(format!(
                "SFTP-Verbindung zu {host}:{port} fehlgeschlagen: {e}"
            ))
        })?;
    let auth = session
        .authenticate_password(user, password)
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Authentifizierung fehlgeschlagen: {e}")))?;
    if !auth.success() {
        return Err(Error::Backup(
            "SFTP-Login abgelehnt — Benutzername oder Passwort falsch.".into(),
        ));
    }
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Kanal nicht öffenbar: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Subsystem nicht verfügbar: {e}")))?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Sitzung nicht initialisierbar: {e}")))?;
    Ok((session, sftp))
}

/// Schreibt `bytes` nach `dir/name` und liefert den vollständigen Remote-Pfad.
///
/// Nutzt bewusst **`create()`** (Flags `CREATE|TRUNCATE|WRITE`) statt der
/// `SftpSession::write`-Bequemlichkeit: deren Implementierung öffnet nur mit
/// `OpenFlags::WRITE` (ohne `CREATE`) und scheitert daher beim Anlegen einer
/// neuen Datei mit „No such file". Der Handle wird per `shutdown` sauber
/// geschlossen (laut russh-sftp-Doku Pflicht, sonst bleibt er offen).
///
/// **R4-002:** schlägt der erste Versuch fehl, wird der Zielordner **rekursiv**
/// per `mkdir_p_remote` angelegt (Component-für-Component, „existiert bereits"-
/// Fehler werden idempotent geschluckt) und erneut geschrieben. Vorher war es
/// nur ein einmaliger `mkdir(dir)` — bei nicht-existentem Großeltern-Pfad
/// (z. B. `/srv` fehlt, Nutzer gibt `/srv/backups/klein-buch` an) scheiterte
/// der zweite Versuch mit dunkler Meldung. Jetzt wird der echte Server-Fehler
/// erst durchgereicht, wenn auch das rekursive Anlegen scheitert.
async fn write_remote(sftp: &SftpSession, dir: &str, name: &str, bytes: &[u8]) -> Result<String> {
    let path = remote_join(dir, name);
    if let Err(first) = create_and_write(sftp, &path, bytes).await {
        if dir != "." {
            // Rekursives `mkdir -p` — pro Komponente best-effort, idempotent
            // ggü. „existiert bereits".
            mkdir_p_remote(sftp, dir).await;
        }
        create_and_write(sftp, &path, bytes).await.map_err(|e| {
            Error::Backup(format!(
                "SFTP-Schreiben nach '{path}' fehlgeschlagen: {e} (erster Versuch: {first})"
            ))
        })?;
    }
    Ok(path)
}

/// Legt `dir` und alle Vorgänger-Komponenten rekursiv an (`mkdir -p`-Semantik).
///
/// Jede Komponente wird einzeln per `create_dir` versucht; existiert sie bereits,
/// liefert der Server einen Fehler, der hier bewusst ignoriert wird (kein
/// stat-basierter Pre-Check — der ist server-implementierungs-abhängig).
///
/// Absoluter Pfad (`/srv/backups/x`) und relativer Pfad (`backups/x`) sind beide
/// erlaubt. Best-effort: ein Fehler in einer mittleren Komponente wird nicht
/// propagiert — der nachfolgende `create_and_write`-Versuch liefert dann den
/// realen Fehlertext.
pub(crate) async fn mkdir_p_remote(sftp: &SftpSession, dir: &str) {
    let trimmed = dir.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == "/" {
        return;
    }
    let (start, rest) = if let Some(stripped) = trimmed.strip_prefix('/') {
        (String::from("/"), stripped)
    } else {
        (String::new(), trimmed)
    };
    let mut current = start;
    for part in rest.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if !current.is_empty() && !current.ends_with('/') {
            current.push('/');
        }
        current.push_str(part);
        // `create_dir` ist auf existierenden Ordnern erwartbar fehlerhaft
        // ("File already exists" / EEXIST) — ignorieren.
        let _ = sftp.create_dir(&current).await;
    }
}

/// Legt die Datei an (CREATE|TRUNCATE|WRITE), schreibt alle Bytes und schließt
/// den Handle sauber (`shutdown`).
async fn create_and_write(sftp: &SftpSession, path: &str, bytes: &[u8]) -> Result<()> {
    let mut file = sftp
        .create(path)
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Datei '{path}' nicht anlegbar: {e}")))?;
    file.write_all(bytes)
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Schreiben fehlgeschlagen: {e}")))?;
    file.shutdown()
        .await
        .map_err(|e| Error::Backup(format!("SFTP-Handle nicht schließbar: {e}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Verbindungstest (Probe) — Public
// ---------------------------------------------------------------------------

/// Ergebnis des Verbindungstests fürs UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpProbe {
    /// SHA-256-Host-Key-Fingerprint zum Bestätigen + Pinnen.
    pub fingerprint: String,
    /// Ob im Zielordner geschrieben werden konnte (Ordner + Schreibrechte ok).
    pub write_ok: bool,
    /// Echter Server-Fehlertext, falls der Schreibtest fehlschlug (sonst `None`).
    pub write_error: Option<String>,
}

/// Testet Verbindung + Auth + Schreibrechte und liefert den Host-Key-Fingerprint
/// zurück (zum Bestätigen/Pinnen). Akzeptiert den unbekannten Host-Key bewusst —
/// es verlässt **kein** Backup den Rechner; geschrieben wird nur eine winzige,
/// sofort wieder gelöschte Probe-Datei.
pub async fn probe(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    remote_dir: &str,
) -> Result<SftpProbe> {
    let seen = Arc::new(Mutex::new(None));
    let handler = HostKeyVerifier {
        pinned: None,
        accept_unknown: true,
        seen: seen.clone(),
    };
    let dir = normalize_remote_dir(remote_dir);
    let (_session, sftp) =
        tokio::time::timeout(CONNECT_TIMEOUT, open(host, port, user, password, handler))
            .await
            .map_err(|_| {
                Error::Backup(format!(
                    "SFTP-Verbindung zu {host}:{port} hat das Zeitlimit überschritten."
                ))
            })??;

    let fingerprint = seen
        .lock()
        .expect("seen mutex poisoned")
        .clone()
        .ok_or_else(|| Error::Backup("SFTP-Host-Key konnte nicht gelesen werden.".into()))?;

    let (write_ok, write_error) = match write_remote(&sftp, &dir, PROBE_FILE, b"klein-buch").await {
        Ok(path) => {
            let _ = sftp.remove_file(path.as_str()).await;
            (true, None)
        }
        Err(e) => (false, Some(e.to_string())),
    };
    let _ = sftp.close().await;
    Ok(SftpProbe {
        fingerprint,
        write_ok,
        write_error,
    })
}

// ---------------------------------------------------------------------------
// Upload — Public (vom write_backup-Dispatch aufgerufen)
// ---------------------------------------------------------------------------

/// Lädt einen Backup-Blob per SFTP hoch und liefert die `sftp://`-URI zurück.
///
/// Erzwingt einen gepinnten Host-Fingerprint (sonst Abbruch) und liest das
/// Passwort aus dem Keychain. Bei Host-Key-Abweichung bricht der Upload mit einer
/// expliziten MITM-Warnung ab (kein Blind-Trust).
pub async fn upload(
    host: &str,
    port: u16,
    user: &str,
    remote_dir: &str,
    pinned_fingerprint: Option<&str>,
    file_name: &str,
    bytes: &[u8],
) -> Result<String> {
    let pinned = pinned_fingerprint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Error::Backup(
                "SFTP-Ziel ohne hinterlegten Host-Fingerprint — bitte zuerst die Verbindung \
                 testen und den Fingerprint bestätigen."
                    .into(),
            )
        })?;
    let password = get_password()?.filter(|p| !p.is_empty()).ok_or_else(|| {
        Error::Backup(
            "Kein SFTP-Passwort im Schlüsselbund — bitte das SFTP-Ziel erneut speichern.".into(),
        )
    })?;

    let dir = normalize_remote_dir(remote_dir);
    let seen = Arc::new(Mutex::new(None));
    let handler = HostKeyVerifier {
        pinned: Some(pinned.clone()),
        accept_unknown: false,
        seen: seen.clone(),
    };

    let opened = tokio::time::timeout(CONNECT_TIMEOUT, open(host, port, user, &password, handler))
        .await
        .map_err(|_| {
            Error::Backup(format!(
                "SFTP-Verbindung zu {host}:{port} hat das Zeitlimit überschritten."
            ))
        })?;

    let (_session, sftp) = match opened {
        Ok(v) => v,
        Err(e) => {
            // Host-Key-Abweichung erkennbar machen (MITM-Schutz): wir haben den
            // Key gesehen, aber er passte nicht zum Pin → Verbindung wurde
            // abgelehnt.
            if let Some(seen_fp) = seen.lock().expect("seen mutex poisoned").clone() {
                if seen_fp != pinned {
                    return Err(Error::Backup(format!(
                        "SFTP-Host-Schlüssel weicht vom hinterlegten Fingerprint ab! \
                         Erwartet {pinned}, erhalten {seen_fp}. Upload abgebrochen \
                         (möglicher Man-in-the-Middle)."
                    )));
                }
            }
            return Err(e);
        }
    };

    let remote_path = write_remote(&sftp, &dir, file_name, bytes).await?;
    let _ = sftp.close().await;

    Ok(sftp_uri(user, host, port, &remote_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static MOCK_INIT: Once = Once::new();

    /// Erzwingt den plattform-unabhängigen Keyring-Mock-Store für den Test-Prozess
    /// (CI ohne echten Keychain/Secret-Service).
    fn use_mock_store() {
        MOCK_INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    #[test]
    fn normalize_remote_dir_cases() {
        assert_eq!(normalize_remote_dir(""), ".");
        assert_eq!(normalize_remote_dir("   "), ".");
        assert_eq!(normalize_remote_dir("backups/"), "backups");
        assert_eq!(normalize_remote_dir("/srv/kb//"), "/srv/kb");
        assert_eq!(normalize_remote_dir("klein-buch"), "klein-buch");
    }

    #[test]
    fn remote_join_cases() {
        assert_eq!(remote_join("", "a.kbk"), "a.kbk");
        assert_eq!(remote_join(".", "a.kbk"), "a.kbk");
        assert_eq!(remote_join("backups", "a.kbk"), "backups/a.kbk");
        assert_eq!(remote_join("/srv/kb/", "a.kbk"), "/srv/kb/a.kbk");
    }

    #[test]
    fn sftp_uri_format() {
        assert_eq!(
            sftp_uri("kb", "host.de", 22, "backups/a.kbk"),
            "sftp://kb@host.de:22/backups/a.kbk"
        );
        // Absoluter Remote-Pfad bekommt keinen doppelten Slash.
        assert_eq!(
            sftp_uri("kb", "host.de", 2222, "/srv/a.kbk"),
            "sftp://kb@host.de:2222/srv/a.kbk"
        );
    }

    #[test]
    fn keyring_wrapper_contract_against_mock() {
        use_mock_store();
        // Frischer Eintrag → Ok(None) (NoEntry-Mapping).
        assert_eq!(get_password().unwrap(), None);
        // set + delete dürfen nicht hart fehlschlagen; delete ist idempotent.
        set_password("s3cr3t-sftp").unwrap();
        delete_password().unwrap();
        delete_password().unwrap();
    }
}
