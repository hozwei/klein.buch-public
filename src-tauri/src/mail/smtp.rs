//! SMTP-Versand via `lettre` (Block 5).
//!
//! Schicht: **Imperative Shell** (Netzwerk-I/O). Das Message-Building
//! ([`build_message`]) ist jedoch pure und damit ohne Server testbar.
//!
//! ## Multi-Attachment
//!
//! [`OutgoingMail::attachments`] ist von Anfang an eine Liste — eine Mail kann
//! mehrere PDFs tragen. Das ist die Voraussetzung für das Angebots-Dokument-
//! Bundle (Angebot + Datenschutz + AGB) in Block 8; in Block 5 hängt der
//! Rechnungsversand genau ein ZUGFeRD-PDF an.
//!
//! ## TLS-Strategie
//!
//! - `use_tls = false` → `builder_dangerous` (Klartext, z. B. lokaler MailHog
//!   im E2E-Test oder ein interner Relay).
//! - `use_tls = true` + Port 465 → implizites TLS (`relay`).
//! - `use_tls = true` + sonst (typ. 587) → STARTTLS (`starttls_relay`).
//!
//! TLS basiert auf rustls (`tokio1-rustls-tls`-Feature) — kein native-tls,
//! konsistent mit dem restlichen Stack.

use lettre::message::{header::ContentType, Attachment, Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

use crate::error::{Error, Result};

/// Verbindungsdaten für einen SMTP-Versand. `password` wird **niemals**
/// geloggt oder persistiert — es kommt aus dem OS-Keychain
/// ([`crate::mail::keyring`]) und lebt nur für die Dauer des Sendevorgangs.
///
/// `Debug` ist bewusst manuell implementiert und **redigiert** das Passwort
/// (Credentials nie in Logs, ADR 0011).
#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl std::fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("use_tls", &self.use_tls)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Ein einzelner Anhang.
#[derive(Debug, Clone)]
pub struct MailAttachment {
    pub filename: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

/// Provider-Antwort des SMTP-Servers nach erfolgreichem Versand (Block 16b) —
/// Nachweis + Troubleshooting. `code` ist der SMTP-Statuscode (z. B. "250"),
/// `message` die erste Reply-Zeile (enthält bei vielen Servern die Queue-ID).
#[derive(Debug, Clone)]
pub struct SmtpResponseInfo {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Eine ausgehende Mail. `attachments` ist eine Liste (Multi-Attachment).
#[derive(Debug, Clone)]
pub struct OutgoingMail {
    pub from_name: String,
    pub from_email: String,
    pub to: String,
    pub subject: String,
    pub body_text: String,
    pub attachments: Vec<MailAttachment>,
}

/// Baut die `lettre::Message` (pure). Validiert Adressen + MIME-Types.
pub fn build_message(mail: &OutgoingMail) -> Result<Message> {
    let from_addr: Address = mail.from_email.parse().map_err(|e| {
        Error::Mail(format!(
            "Absender-Adresse ungültig '{}': {e}",
            mail.from_email
        ))
    })?;
    let from = Mailbox::new(Some(mail.from_name.clone()), from_addr);

    let to: Mailbox = mail
        .to
        .parse()
        .map_err(|e| Error::Mail(format!("Empfänger-Adresse ungültig '{}': {e}", mail.to)))?;

    let builder = Message::builder()
        .from(from)
        .to(to)
        .subject(mail.subject.clone());

    // Ohne Anhang: saubere reine text/plain-Mail (kein Multipart-Wrapper um nur
    // einen Teil — das ist unüblich und ein leichtes Spam-Signal). Mit Anhang:
    // multipart/mixed (Body als erster Part, dann jeder Anhang).
    if mail.attachments.is_empty() {
        builder
            .singlepart(SinglePart::plain(mail.body_text.clone()))
            .map_err(|e| Error::Mail(format!("Mail-Aufbau fehlgeschlagen: {e}")))
    } else {
        let mut multipart =
            MultiPart::mixed().singlepart(SinglePart::plain(mail.body_text.clone()));
        for att in &mail.attachments {
            let content_type = ContentType::parse(&att.mime_type).map_err(|e| {
                Error::Mail(format!("Ungültiger MIME-Type '{}': {e}", att.mime_type))
            })?;
            let part = Attachment::new(att.filename.clone()).body(att.bytes.clone(), content_type);
            multipart = multipart.singlepart(part);
        }
        builder
            .multipart(multipart)
            .map_err(|e| Error::Mail(format!("Mail-Aufbau fehlgeschlagen: {e}")))
    }
}

/// Baut den async SMTP-Transport nach der TLS-Strategie aus dem Modul-Header.
fn build_transport(config: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let builder = if !config.use_tls {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
    } else if config.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|e| Error::Mail(format!("SMTP-Relay-Setup ({}): {e}", config.host)))?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|e| Error::Mail(format!("SMTP-STARTTLS-Setup ({}): {e}", config.host)))?
    };

    let mut builder = builder.port(config.port);
    if let (Some(user), Some(pass)) = (config.username.as_ref(), config.password.as_ref()) {
        builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }
    Ok(builder.build())
}

/// Versendet die Mail. Netzwerk-I/O.
///
/// Loggt Verbindungs- und Versand-Eckdaten via `tracing` (Host/Port/TLS/User,
/// Empfänger, Anhang-Anzahl, Ergebnis) — **niemals** das Passwort. lettres
/// eigenes `tracing`-Feature ist bewusst NICHT aktiviert, da es den rohen
/// AUTH-Befehl (base64-Credentials) mitloggen würde.
pub async fn send(config: &SmtpConfig, mail: &OutgoingMail) -> Result<SmtpResponseInfo> {
    let message = build_message(mail)?;
    let transport = build_transport(config)?;
    tracing::info!(
        host = %config.host,
        port = config.port,
        use_tls = config.use_tls,
        user = config.username.as_deref().unwrap_or("-"),
        to = %mail.to,
        attachments = mail.attachments.len(),
        "SMTP: sende Mail (Passwort wird nie geloggt)"
    );
    let response = transport.send(message).await.map_err(|e| {
        tracing::warn!(host = %config.host, to = %mail.to, error = %e, "SMTP: Versand fehlgeschlagen");
        Error::Mail(format!("SMTP-Versand fehlgeschlagen: {e}"))
    })?;
    let info = SmtpResponseInfo {
        code: Some(response.code().to_string()),
        message: response.first_line().map(|s| s.to_string()),
    };
    tracing::info!(
        to = %mail.to,
        code = info.code.as_deref().unwrap_or("-"),
        "SMTP: Mail erfolgreich an den Server übergeben"
    );
    Ok(info)
}

/// Prüft, ob der SMTP-Server erreichbar ist (Connect + Hello). Nutzt KEINE
/// Credentials zum Authentifizieren — nur die Erreichbarkeit.
pub async fn test_connection(config: &SmtpConfig) -> Result<()> {
    let transport = build_transport(config)?;
    tracing::info!(
        host = %config.host,
        port = config.port,
        use_tls = config.use_tls,
        "SMTP: teste Verbindung"
    );
    let reachable = transport.test_connection().await.map_err(|e| {
        tracing::warn!(host = %config.host, error = %e, "SMTP: Verbindungstest fehlgeschlagen");
        Error::Mail(format!("Verbindungstest fehlgeschlagen: {e}"))
    })?;
    if !reachable {
        return Err(Error::Mail(
            "SMTP-Server antwortet nicht auf Hello (nicht erreichbar).".into(),
        ));
    }
    tracing::info!(host = %config.host, "SMTP: Verbindung ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mail() -> OutgoingMail {
        OutgoingMail {
            from_name: "Wildbach Computerhilfe".into(),
            from_email: "schmidm@wildbach-computerhilfe.de".into(),
            to: "kunde@example.com".into(),
            subject: "Rechnung RE-2026-0001".into(),
            body_text: "Anbei die Rechnung. Mit freundlichen Gruessen".into(),
            attachments: vec![
                MailAttachment {
                    filename: "RE-2026-0001.pdf".into(),
                    mime_type: "application/pdf".into(),
                    bytes: b"%PDF-1.7 fake".to_vec(),
                },
                MailAttachment {
                    filename: "AGB.pdf".into(),
                    mime_type: "application/pdf".into(),
                    bytes: b"%PDF-1.7 agb".to_vec(),
                },
            ],
        }
    }

    #[test]
    fn build_message_includes_subject_body_and_all_attachments() {
        let msg = build_message(&sample_mail()).unwrap();
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(raw.contains("Rechnung RE-2026-0001"), "Subject fehlt");
        assert!(raw.contains("Anbei die Rechnung"), "Body fehlt");
        // Multi-Attachment: beide Dateinamen müssen auftauchen.
        assert!(raw.contains("RE-2026-0001.pdf"), "Attachment 1 fehlt");
        assert!(raw.contains("AGB.pdf"), "Attachment 2 fehlt");
        assert!(raw.contains("multipart/mixed"), "kein mixed-multipart");
    }

    #[test]
    fn build_message_rejects_bad_recipient() {
        let mut mail = sample_mail();
        mail.to = "not-an-email".into();
        assert!(build_message(&mail).is_err());
    }

    #[test]
    fn build_message_without_attachments_is_plain_text_not_multipart() {
        let mut mail = sample_mail();
        mail.attachments.clear();
        let msg = build_message(&mail).unwrap();
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(raw.contains("text/plain"), "kein text/plain-Part");
        assert!(
            !raw.contains("multipart/"),
            "anhanglose Mail darf kein Multipart sein"
        );
        assert!(raw.contains("Anbei die Rechnung"), "Body fehlt");
    }
}
