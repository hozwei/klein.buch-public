//! Minimale SMTP-Gegenstelle für Tests (Lauf F5c: W8-SMTP-CA, R4-S2-EMAILLOG).
//!
//! Lauscht auf `127.0.0.1` (freier Port), spricht so viel ESMTP, wie lettre für
//! einen Versand braucht (EHLO, optional STARTTLS, MAIL, RCPT, DATA, RSET, NOOP,
//! QUIT), und sammelt die empfangenen Nachrichten. Mit TLS-Konfiguration bietet
//! sie STARTTLS an; Zertifikate und Test-CA erzeugt [`test_pki`] bei jedem Aufruf
//! neu (keine Schlüssel im Repo, nur Dokumentations-Namen).
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio_rustls::rustls;
use tokio_rustls::TlsAcceptor;

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// Laufende Gegenstelle.
pub struct FakeSmtp {
    pub port: u16,
    messages: Arc<Mutex<Vec<String>>>,
    tls_sessions: Arc<AtomicUsize>,
}

impl FakeSmtp {
    /// Empfangene Nachrichten (Rohtext nach `DATA`).
    pub fn messages(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }

    /// Anzahl erfolgreich abgeschlossener TLS-Handshakes.
    pub fn tls_sessions(&self) -> usize {
        self.tls_sessions.load(Ordering::SeqCst)
    }
}

/// Startet die Gegenstelle; mit `tls` wird STARTTLS angeboten.
pub async fn start(tls: Option<Arc<rustls::ServerConfig>>) -> FakeSmtp {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let messages = Arc::new(Mutex::new(Vec::new()));
    let tls_sessions = Arc::new(AtomicUsize::new(0));
    let acceptor = tls.map(TlsAcceptor::from);
    let (m, t) = (messages.clone(), tls_sessions.clone());
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let (m, t, a) = (m.clone(), t.clone(), acceptor.clone());
            tokio::spawn(async move {
                let _ = session(Box::new(sock), a, m, t).await;
            });
        }
    });
    FakeSmtp {
        port,
        messages,
        tls_sessions,
    }
}

enum Next {
    Quit,
    StartTls,
}

async fn session(
    mut io: Box<dyn Io>,
    acceptor: Option<TlsAcceptor>,
    messages: Arc<Mutex<Vec<String>>>,
    tls_sessions: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    io.write_all(b"220 smtp.example.org ESMTP Test\r\n").await?;
    io.flush().await?;
    let mut secured = false;
    loop {
        let offer = acceptor.is_some() && !secured;
        let mut reader = BufReader::new(io);
        let next = commands(&mut reader, offer, &messages).await?;
        io = reader.into_inner();
        match next {
            Next::Quit => return Ok(()),
            Next::StartTls => {
                let Some(acceptor) = acceptor.clone() else {
                    return Ok(());
                };
                let tls = acceptor.accept(io).await?;
                tls_sessions.fetch_add(1, Ordering::SeqCst);
                io = Box::new(tls);
                secured = true;
            }
        }
    }
}

async fn reply(reader: &mut BufReader<Box<dyn Io>>, line: &[u8]) -> std::io::Result<()> {
    let w = reader.get_mut();
    w.write_all(line).await?;
    w.flush().await
}

async fn commands(
    reader: &mut BufReader<Box<dyn Io>>,
    offer_starttls: bool,
    messages: &Mutex<Vec<String>>,
) -> std::io::Result<Next> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(Next::Quit);
        }
        let cmd = line.trim_end().to_ascii_uppercase();
        if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
            let caps: &[u8] = if offer_starttls {
                b"250-smtp.example.org\r\n250-8BITMIME\r\n250 STARTTLS\r\n"
            } else {
                b"250-smtp.example.org\r\n250 8BITMIME\r\n"
            };
            reply(reader, caps).await?;
        } else if cmd == "STARTTLS" && offer_starttls {
            reply(reader, b"220 2.0.0 Ready to start TLS\r\n").await?;
            return Ok(Next::StartTls);
        } else if cmd.starts_with("MAIL FROM")
            || cmd.starts_with("RCPT TO")
            || cmd == "RSET"
            || cmd == "NOOP"
        {
            reply(reader, b"250 2.0.0 Ok\r\n").await?;
        } else if cmd == "DATA" {
            reply(reader, b"354 End data with <CR><LF>.<CR><LF>\r\n").await?;
            let mut data = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).await? == 0 {
                    return Ok(Next::Quit);
                }
                if line == ".\r\n" || line == ".\n" {
                    break;
                }
                data.push_str(&line);
            }
            messages.lock().unwrap().push(data);
            reply(reader, b"250 2.0.0 Ok: queued as TEST1\r\n").await?;
        } else if cmd == "QUIT" {
            reply(reader, b"221 2.0.0 Bye\r\n").await?;
            return Ok(Next::Quit);
        } else {
            reply(reader, b"502 5.5.2 Error: command not recognized\r\n").await?;
        }
    }
}

/// Selbst erzeugte Test-PKI: eine CA und ein von ihr signiertes
/// Server-Zertifikat für `127.0.0.1`/`localhost`.
pub struct TestPki {
    /// CA-Zertifikat als PEM (für `KLEIN_BUCH_SMTP_CA_FILE`).
    pub ca_pem: String,
    /// TLS-Konfiguration der Gegenstelle mit dem Server-Zertifikat.
    pub server: Arc<rustls::ServerConfig>,
}

/// Erzeugt CA und Server-Zertifikat (ECDSA P-256, ring wie im Produktivcode).
pub fn test_pki(ca_name: &str) -> TestPki {
    use rcgen::{
        BasicConstraints, CertificateParams, CertifiedIssuer, DnType, ExtendedKeyUsagePurpose,
        IsCa, KeyPair, KeyUsagePurpose,
    };
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, ca_name);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let ca = CertifiedIssuer::self_signed(ca_params, KeyPair::generate().unwrap()).unwrap();

    let key = KeyPair::generate().unwrap();
    let mut params =
        CertificateParams::new(vec!["127.0.0.1".to_string(), "localhost".to_string()]).unwrap();
    params
        .distinguished_name
        .push(DnType::CommonName, "smtp.example.org");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let cert = params.signed_by(&key, &*ca).unwrap();

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into()),
        )
        .unwrap();
    TestPki {
        ca_pem: ca.pem(),
        server: Arc::new(server),
    }
}
