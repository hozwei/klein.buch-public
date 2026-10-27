//! W8-SMTP-CA (Lauf F5c): `KLEIN_BUCH_SMTP_CA_FILE` bis zum echten Handshake.
//!
//! Eine selbst erzeugte Test-CA signiert das Zertifikat einer SMTP-Gegenstelle
//! auf `127.0.0.1` (STARTTLS). Geprüft wird der vollständige Versand über
//! [`klein_buch_lib::mail::smtp::send`] samt rustls-Handshake:
//! - ohne CA-Datei: Handshake scheitert (unbekannter Aussteller), nichts zugestellt;
//! - mit der passenden CA: Handshake und Zustellung gelingen, auch der
//!   Verbindungstest;
//! - mit einer fremden CA bzw. einer Datei ohne PEM-Zertifikat: abgelehnt.
//!
//! Eigene Test-Datei (eigener Prozess), weil die Umgebungsvariable prozessweit
//! gilt; alle Fälle laufen nacheinander in einem Test.

mod smtp_support;

use klein_buch_lib::mail::smtp::{self, OutgoingMail, SmtpConfig, ENV_CA_FILE};

fn config(port: u16) -> SmtpConfig {
    SmtpConfig {
        host: "127.0.0.1".into(),
        port,
        use_tls: true,
        username: None,
        password: None,
    }
}

fn mail() -> OutgoingMail {
    OutgoingMail {
        from_name: "Beispiel IT-Service".into(),
        from_email: "rechnung@example.org".into(),
        to: "kunde@example.org".into(),
        subject: "TLS-Probe mit eigener CA".into(),
        body_text: "Zustellung über STARTTLS.".into(),
        attachments: Vec::new(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn smtp_starttls_handshake_with_own_ca() {
    let pki = smtp_support::test_pki("Klein.Buch Test-CA");
    let server = smtp_support::start(Some(pki.server.clone())).await;
    let dir = tempfile::tempdir().unwrap();

    // 1. Ohne eigene CA: nur die eingebauten Wurzeln — Handshake scheitert.
    std::env::remove_var(ENV_CA_FILE);
    let err = smtp::send(&config(server.port), &mail())
        .await
        .expect_err("ohne CA-Datei darf der Handshake nicht gelingen");
    assert!(
        err.to_string().contains("SMTP-Versand fehlgeschlagen"),
        "{err}"
    );
    assert_eq!(server.tls_sessions(), 0);
    assert!(server.messages().is_empty());

    // 2. Mit der passenden CA: echter Handshake, Mail kommt an.
    let ca_file = dir.path().join("eigene-ca.pem");
    std::fs::write(&ca_file, &pki.ca_pem).unwrap();
    std::env::set_var(ENV_CA_FILE, &ca_file);
    let info = smtp::send(&config(server.port), &mail())
        .await
        .expect("Versand mit eigener CA");
    assert_eq!(info.code.as_deref(), Some("250"));
    assert_eq!(server.tls_sessions(), 1, "ein TLS-Handshake");
    let messages = server.messages();
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0].contains("TLS-Probe mit eigener CA"),
        "{}",
        messages[0]
    );
    smtp::test_connection(&config(server.port))
        .await
        .expect("Verbindungstest mit eigener CA");
    assert_eq!(server.tls_sessions(), 2);

    // 3. Fremde CA: Handshake scheitert, nichts zugestellt.
    let other = smtp_support::test_pki("Fremde Test-CA");
    let other_file = dir.path().join("fremde-ca.pem");
    std::fs::write(&other_file, &other.ca_pem).unwrap();
    std::env::set_var(ENV_CA_FILE, &other_file);
    assert!(smtp::send(&config(server.port), &mail()).await.is_err());
    assert_eq!(server.tls_sessions(), 2);
    assert_eq!(server.messages().len(), 1);

    // 4. Datei ohne PEM-Zertifikat: Fehler vor jedem Verbindungsaufbau.
    let junk = dir.path().join("kein-zertifikat.pem");
    std::fs::write(&junk, "kein Zertifikat").unwrap();
    std::env::set_var(ENV_CA_FILE, &junk);
    let err = smtp::send(&config(server.port), &mail())
        .await
        .expect_err("ungültige CA-Datei");
    assert!(err.to_string().contains(ENV_CA_FILE), "{err}");

    std::env::remove_var(ENV_CA_FILE);
}
