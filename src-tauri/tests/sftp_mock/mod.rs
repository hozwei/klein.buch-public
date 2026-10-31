//! SFTP-Testserver (Plan W8): russh-Server auf `127.0.0.1` mit einem im Test
//! erzeugten Ed25519-Host-Schlüssel, Passwort-Login für genau einen Benutzer
//! und Dateien unter einem Temp-Verzeichnis. Kann nur, was der Backup-Upload
//! braucht (öffnen/schreiben/schließen, Ordner anlegen, umbenennen, löschen).
//! Auf Wunsch öffnet er nach der Anmeldung einen Agent-Kanal
//! (OpenSSH-Kanaltyp `auth-agent`) zum Client und hält das Ergebnis fest
//! (SEC-2026.11.1-02: der Client lehnt ihn ab).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::PrivateKey;
use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{
    File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};

pub struct SftpMock {
    pub port: u16,
    pub root: PathBuf,
    /// SHA-256-Fingerprint des Host-Schlüssels (Format wie `backup::sftp`).
    pub fingerprint: String,
    /// Anzahl angenommener TCP-Verbindungen (für „ohne Verbindungsaufbau“).
    pub connections: Arc<AtomicUsize>,
    /// Anzahl der Passwort-Anmeldeversuche (ADR 0042 V3c: nach abgelehntem
    /// Host-Key darf keine Anmeldung stattfinden).
    pub auth_attempts: Arc<AtomicUsize>,
    /// Ergebnis des Agent-Kanals, den der Server nach der Anmeldung öffnet
    /// (nur mit [`start_with_agent_probe`]): `accepted`, `rejected:<Grund>`
    /// oder `error:<Fehler>`; `None` = (noch) keins.
    pub agent_open: Arc<Mutex<Option<String>>>,
}

impl SftpMock {
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    pub fn auth_attempt_count(&self) -> usize {
        self.auth_attempts.load(Ordering::SeqCst)
    }

    /// Ergebnis des Agent-Kanals; wartet bis zu fünf Sekunden darauf.
    pub async fn agent_open_result(&self) -> Option<String> {
        for _ in 0..100 {
            if let Some(r) = self.agent_open.lock().unwrap().clone() {
                return Some(r);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        None
    }
}

/// Startet den Server auf einem freien Port von `127.0.0.1` (lebt, bis die
/// Tokio-Runtime endet).
pub async fn start(root: PathBuf, user: &str, password: &str) -> SftpMock {
    start_on("127.0.0.1:0", root, user, password).await
}

/// Wie [`start`]; nach jeder erfolgreichen Anmeldung öffnet der Server einen
/// Agent-Kanal zum Client (SEC-2026.11.1-02).
pub async fn start_with_agent_probe(root: PathBuf, user: &str, password: &str) -> SftpMock {
    start_inner("127.0.0.1:0", root, user, password, true).await
}

pub async fn start_on(addr: &str, root: PathBuf, user: &str, password: &str) -> SftpMock {
    start_inner(addr, root, user, password, false).await
}

async fn start_inner(
    addr: &str,
    root: PathBuf,
    user: &str,
    password: &str,
    open_agent: bool,
) -> SftpMock {
    std::fs::create_dir_all(&root).unwrap();
    let seed: [u8; 32] = rand::random();
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&seed));
    let fingerprint = key.public_key().fingerprint(Default::default()).to_string();
    let config = Arc::new(russh::server::Config {
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::ZERO),
        keys: vec![key],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicUsize::new(0));
    let auth_attempts = Arc::new(AtomicUsize::new(0));
    let agent_open = Arc::new(Mutex::new(None));
    let mut server = MockServer {
        root: Arc::new(root.clone()),
        user: Arc::new(user.to_string()),
        password: Arc::new(password.to_string()),
        connections: connections.clone(),
        auth_attempts: auth_attempts.clone(),
        open_agent,
        agent_open: agent_open.clone(),
    };
    tokio::spawn(async move {
        let _ = server.run_on_socket(config, &listener).await;
    });
    SftpMock {
        port,
        root,
        fingerprint,
        connections,
        auth_attempts,
        agent_open,
    }
}

#[derive(Clone)]
struct MockServer {
    root: Arc<PathBuf>,
    user: Arc<String>,
    password: Arc<String>,
    connections: Arc<AtomicUsize>,
    auth_attempts: Arc<AtomicUsize>,
    open_agent: bool,
    agent_open: Arc<Mutex<Option<String>>>,
}

impl russh::server::Server for MockServer {
    type Handler = MockSession;

    fn new_client(&mut self, _: Option<SocketAddr>) -> MockSession {
        self.connections.fetch_add(1, Ordering::SeqCst);
        MockSession {
            server: self.clone(),
            channels: HashMap::new(),
        }
    }
}

struct MockSession {
    server: MockServer,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

impl russh::server::Handler for MockSession {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.server.auth_attempts.fetch_add(1, Ordering::SeqCst);
        if user == self.server.user.as_str() && password == self.server.password.as_str() {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    /// SEC-2026.11.1-02: Agent-Kanal zum Client öffnen. Im eigenen Task, weil
    /// die Antwort des Clients erst die Sitzungsschleife verarbeitet, die hier
    /// nicht warten darf.
    async fn auth_succeeded(&mut self, session: &mut Session) -> Result<(), Self::Error> {
        if self.server.open_agent {
            let handle = session.handle();
            let result = self.server.agent_open.clone();
            tokio::spawn(async move {
                let r = match handle.channel_open_agent().await {
                    Ok(_) => "accepted".to_string(),
                    Err(russh::Error::ChannelOpenFailure(reason)) => {
                        format!("rejected:{reason:?}")
                    }
                    Err(e) => format!("error:{e:?}"),
                };
                *result.lock().unwrap() = Some(r);
            });
        }
        Ok(())
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.close(channel)?;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        match (name, self.channels.remove(&channel_id)) {
            ("sftp", Some(channel)) => {
                session.channel_success(channel_id)?;
                let fs = MockFs {
                    root: self.server.root.as_ref().clone(),
                    handles: HashMap::new(),
                    next: 0,
                };
                russh_sftp::server::run(channel.into_stream(), fs).await;
            }
            _ => session.channel_failure(channel_id)?,
        }
        Ok(())
    }
}

struct MockFs {
    root: PathBuf,
    handles: HashMap<String, PathBuf>,
    next: u64,
}

impl MockFs {
    /// Pfad unter der Wurzel (absolut wie relativ); `..` wird abgelehnt.
    fn resolve(&self, path: &str) -> Result<PathBuf, StatusCode> {
        let mut out = self.root.clone();
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => return Err(StatusCode::PermissionDenied),
                p => out.push(p),
            }
        }
        Ok(out)
    }
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".into(),
        language_tag: "en-US".into(),
    }
}

impl russh_sftp::server::Handler for MockFs {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = self.resolve(&filename)?;
        std::fs::OpenOptions::new()
            .read(pflags.contains(OpenFlags::READ))
            .write(pflags.contains(OpenFlags::WRITE))
            .create(pflags.contains(OpenFlags::CREATE))
            .truncate(pflags.contains(OpenFlags::TRUNCATE))
            .open(&path)
            .map_err(|_| StatusCode::NoSuchFile)?;
        self.next += 1;
        let handle = format!("h{}", self.next);
        self.handles.insert(handle.clone(), path);
        Ok(Handle { id, handle })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let path = self.handles.get(&handle).ok_or(StatusCode::Failure)?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|_| StatusCode::Failure)?;
        file.write_all_at(&data, offset)
            .map_err(|_| StatusCode::Failure)?;
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        std::fs::create_dir(self.resolve(&path)?).map_err(|_| StatusCode::Failure)?;
        Ok(ok(id))
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        std::fs::rename(self.resolve(&oldpath)?, self.resolve(&newpath)?)
            .map_err(|_| StatusCode::Failure)?;
        Ok(ok(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        std::fs::remove_file(self.resolve(&filename)?).map_err(|_| StatusCode::NoSuchFile)?;
        Ok(ok(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::dummy(format!("/{}", path.trim_start_matches('/')))],
        })
    }
}
