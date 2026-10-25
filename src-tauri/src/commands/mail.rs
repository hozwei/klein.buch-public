//! Commands für den Mail-Versand (Block 5, SMTP).
//!
//! Orchestriert:
//! - Mail-Account-Verwaltung ([`crate::db::repo::mail_accounts`])
//! - Passwort-Handling über die Datenbank ([`crate::secrets`], W8)
//! - Body-Rendering via Tera ([`crate::mail::templates`])
//! - Versand via lettre ([`crate::mail::smtp`]), Multi-Attachment-fähig
//! - GoBD: nach Versand `status = 'sent'` + Audit-Log-Eintrag
//!
//! ## Hard-Rules
//!
//! - **Passwort niemals** im Konto-Datensatz, in Logs oder `audit_log` — nur in
//!   der Tabelle `secrets` der verschlüsselten DB (Plan W8).
//! - Versand nur für **gelockte** Rechnungen mit archiviertem ZUGFeRD-PDF.
//! - Das ZUGFeRD-PDF wird vor dem Anhängen per SHA-256 gegen das Archiv
//!   verifiziert ([`crate::archive::read_and_verify`]) — Tamper-Schutz.

use crate::app_ctx::AppCtx;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::archive;
use crate::config::Paths;
use crate::db::models::MailAccountRow;
use crate::db::repo::{
    audit_log, contacts, email_log, invoices, mail_accounts, quotes, seller_profile,
};
use crate::domain::kleinunternehmer;
use crate::error::{Error, Result};
use crate::mail::templates::RenderedMail;
use crate::mail::{smtp, templates};
use crate::nettest::{self, TestKind};
use crate::redact::redacted_debug;
use crate::secrets;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionArgs {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_use_tls: bool,
    pub smtp_user: Option<String>,
    /// Klartext-Passwort NUR für den Verbindungstest — wird nicht
    /// gespeichert. Bei `None` wird ohne Auth getestet.
    pub password: Option<String>,
}
redacted_debug!(TestConnectionArgs { smtp_host, smtp_port, smtp_use_tls, smtp_user } secret { password });

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendInvoiceArgs {
    pub account_id: String,
    pub invoice_id: String,
    /// Optionaler Empfänger-Override. Default ist der Buyer-Snapshot der
    /// Rechnung bzw. die E-Mail des Kontakts.
    pub to: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub invoice_id: String,
    pub to: String,
    pub subject: String,
    pub attachment_count: u32,
    /// R4-S2-EMAILLOG (Lauf F5c): gesetzt, wenn die Mail hinausging, der Eintrag im
    /// Versandprotokoll aber nicht geschrieben werden konnte (Hinweis in der Inbox,
    /// Audit `mail.log_failed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendQuoteArgs {
    pub account_id: String,
    pub quote_id: String,
    /// Optionaler Empfänger-Override. Default: E-Mail des verknüpften Kontakts.
    pub to: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendQuoteResult {
    pub quote_id: String,
    pub to: String,
    pub subject: String,
    /// Angebot-PDF + AGB + Datenschutz = i. d. R. 3.
    pub attachment_count: u32,
    /// R4-S2-EMAILLOG (Lauf F5c): gesetzt, wenn die Mail hinausging, der Eintrag im
    /// Versandprotokoll aber nicht geschrieben werden konnte (Hinweis in der Inbox,
    /// Audit `mail.log_failed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

// =============================================================================
// Commands — Accounts
// =============================================================================

/// Argumente von [`mail_accounts_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountsListArgs {}

pub async fn mail_accounts_list(
    ctx: &Arc<AppCtx>,
    _args: MailAccountsListArgs,
) -> Result<Vec<MailAccountRow>> {
    let pool = ctx.pool()?;

    mail_accounts::list(&pool).await
}

/// Argumente von [`mail_account_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountCreateArgs {
    pub input: mail_accounts::MailAccountInput,
    #[serde(default)]
    pub password: Option<String>,
}
redacted_debug!(MailAccountCreateArgs { input } secret { password });

/// Legt einen Mail-Account an und speichert (falls vorhanden) das Passwort in
/// `secrets` (verschlüsselte DB), nie im Konto-Datensatz.
pub async fn mail_account_create(
    ctx: &Arc<AppCtx>,
    args: MailAccountCreateArgs,
) -> Result<MailAccountRow> {
    let MailAccountCreateArgs {
        mut input,
        password,
    } = args;
    normalize_smtp_host(&mut input)?;
    let pool = ctx.pool()?;

    let pool = &pool;
    check_smtp_only(&input)?;
    check_saved_port(ctx, &input)?;
    check_plaintext(&input, password.as_deref())?;
    let row = mail_accounts::create(pool, &input).await?;

    if let (Some(_), Some(pass)) = (row.keychain_service_id.as_deref(), password.as_deref()) {
        if !pass.is_empty() {
            secrets::set(pool, &secrets::smtp_key(&row.id), pass).await?;
        }
    }

    audit_log::append(
        pool,
        "mail_account.create",
        "mail_account",
        &row.id,
        Some(serde_json::json!({ "label": &row.label, "auth_type": &row.auth_type, "from_email": &row.from_email, "has_password": password.as_deref().map(|p| !p.is_empty()).unwrap_or(false) })),
    )
    .await?;

    Ok(row)
}

/// Argumente von [`mail_account_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountUpdateArgs {
    pub id: String,
    pub input: mail_accounts::MailAccountInput,
    #[serde(default)]
    pub password: Option<String>,
}
redacted_debug!(MailAccountUpdateArgs { id, input } secret { password });

/// Aktualisiert einen Mail-Account. Ein nicht-leeres `password` überschreibt
/// das gespeicherte Passwort; bleibt das Feld leer, bleibt das bestehende
/// Passwort unverändert, solange Server, Port und Benutzer gleich bleiben.
pub async fn mail_account_update(
    ctx: &Arc<AppCtx>,
    args: MailAccountUpdateArgs,
) -> Result<MailAccountRow> {
    let MailAccountUpdateArgs {
        id,
        mut input,
        password,
    } = args;
    let pool = ctx.pool()?;

    normalize_smtp_host(&mut input)?;
    let pool = &pool;
    let before = mail_accounts::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {id}")))?;
    check_smtp_only(&input)?;
    // Neuer Server oder Port: Port-Allowlist wie beim Test (ASVS V12.6.1); ein
    // unveränderter Bestandswert bleibt nutzbar.
    if before.smtp_host.as_deref().map(str::trim) != input.smtp_host.as_deref()
        || before.smtp_port != input.smtp_port
    {
        check_saved_port(ctx, &input)?;
    }
    let new_password = password.as_deref().filter(|p| !p.is_empty());
    let stored = secrets::get(pool, &secrets::smtp_key(&id)).await?;
    check_plaintext(
        &input,
        new_password.or(stored.as_deref().map(String::as_str)),
    )?;
    // Plan W8: Zugangsdaten gelten nur für (Host, Port, Benutzer). Ändert sich
    // eins davon, wird das gespeicherte Passwort gelöscht; mit Benutzer ist
    // dann ein neues Pflicht (sonst ginge es an den neuen Host).
    let rebound = before.smtp_host.as_deref().map(str::trim)
        != input.smtp_host.as_deref().map(str::trim)
        || before.smtp_port != input.smtp_port
        || before
            .smtp_user
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            != input
                .smtp_user
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty());
    let has_user = input
        .smtp_user
        .as_deref()
        .map(str::trim)
        .is_some_and(|u| !u.is_empty());
    if rebound && stored.is_some() && has_user && new_password.is_none() {
        return Err(Error::Mail(
            "Server, Port oder Benutzer geändert — bitte das Passwort neu eingeben.".into(),
        ));
    }
    let row = mail_accounts::update(pool, &id, &input).await?;
    if rebound {
        secrets::delete(pool, &secrets::smtp_key(&id)).await?;
    }
    // Umstellung eines (Desktop-)Microsoft-Kontos auf SMTP: Token verwerfen.
    if before.auth_type != row.auth_type {
        secrets::delete(pool, &secrets::oauth_key(&id)).await?;
    }
    if let (Some(_), Some(pass)) = (row.keychain_service_id.as_deref(), new_password) {
        secrets::set(pool, &secrets::smtp_key(&row.id), pass).await?;
    }

    audit_log::append(
        pool,
        "mail_account.update",
        "mail_account",
        &row.id,
        Some(serde_json::json!({ "label": &row.label, "from_email": &row.from_email, "password_changed": password.as_deref().map(|p| !p.is_empty()).unwrap_or(false) })),
    )
    .await?;

    Ok(row)
}

/// Argumente von [`mail_account_delete`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountDeleteArgs {
    pub id: String,
}

/// Löscht einen Mail-Account samt gespeichertem Passwort bzw. Token.
pub async fn mail_account_delete(ctx: &Arc<AppCtx>, args: MailAccountDeleteArgs) -> Result<()> {
    let MailAccountDeleteArgs { id } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let account = mail_accounts::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {id}")))?;

    // Gespeicherte Zugangsdaten entfernen (SMTP-Passwort + OAuth-Token).
    secrets::delete(pool, &secrets::smtp_key(&account.id)).await?;
    secrets::delete(pool, &secrets::oauth_key(&account.id)).await?;

    mail_accounts::delete(pool, &id).await?;

    audit_log::append(
        pool,
        "mail_account.delete",
        "mail_account",
        &id,
        Some(serde_json::json!({ "label": &account.label })),
    )
    .await?;

    Ok(())
}

/// Argumente von [`mail_account_test_connection`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountTestConnectionArgs {
    pub args: TestConnectionArgs,
}

/// Testet eine SMTP-Verbindung mit Ad-hoc-Daten (vor dem Speichern eines
/// Accounts). Gibt bei Erfolg `()` zurück, sonst einen sprechenden Fehler.
pub async fn mail_account_test_connection(
    ctx: &Arc<AppCtx>,
    args: MailAccountTestConnectionArgs,
) -> Result<()> {
    let MailAccountTestConnectionArgs { args } = args;
    // Plan W8: nie ein gespeichertes Passwort an einen frei gewählten Host —
    // gespeicherte Konten testet `mail_account_test_saved`.
    ctx.net_tests.check_port(TestKind::Smtp, args.smtp_port)?;
    crate::nettest::check_host(args.smtp_host.trim())?;
    let has_user = args
        .smtp_user
        .as_deref()
        .is_some_and(|u| !u.trim().is_empty());
    let has_password = args.password.as_deref().is_some_and(|p| !p.is_empty());
    if has_user && !has_password {
        return Err(Error::Domain(
            "Bitte das Passwort eingeben (gespeicherte Postfächer über „Testen“ in der Liste)."
                .into(),
        ));
    }
    if !args.smtp_use_tls && (has_user || has_password) {
        return Err(plaintext_refused());
    }
    ctx.net_tests.admit()?;
    let config = smtp::SmtpConfig {
        host: args.smtp_host.trim().to_string(),
        port: args.smtp_port,
        use_tls: args.smtp_use_tls,
        username: args.smtp_user,
        password: args.password.map(zeroize::Zeroizing::new),
    };
    smtp::test_connection(&config)
        .await
        .map_err(|e| nettest::connect_failed(&config.host, config.port, &e))
}

/// Argumente von [`mail_account_test_saved`].
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountTestSavedArgs {
    pub id: String,
}

/// Testet ein gespeichertes Konto mit seinem gespeicherten Passwort (Plan W8,
/// nur per ID; Port-Allowlist und Kontingent wie beim Ad-hoc-Test).
pub async fn mail_account_test_saved(
    ctx: &Arc<AppCtx>,
    args: MailAccountTestSavedArgs,
) -> Result<()> {
    let MailAccountTestSavedArgs { id } = args;
    let pool = ctx.pool()?;
    let account = mail_accounts::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {id}")))?;
    let config = smtp_config_for(&pool, &account).await?;
    ctx.net_tests.check_port(TestKind::Smtp, config.port)?;
    ctx.net_tests.admit()?;
    smtp::test_connection(&config)
        .await
        .map_err(|e| nettest::connect_failed(&config.host, config.port, &e))
}

/// Im Server-Betrieb nur SMTP-Postfächer (E9: MS-Graph ausgeblendet).
fn check_smtp_only(input: &mail_accounts::MailAccountInput) -> Result<()> {
    ensure_smtp_auth(&input.auth_type)
}

/// Nur Konten mit SMTP-Anmeldung versenden (E9). Ein aus der Desktop-Fassung
/// übernommenes Microsoft-Konto (`oauth_microsoft`, Graph) lehnt der Versand mit
/// derselben Meldung ab, bis es in den Einstellungen auf SMTP umgestellt ist
/// (ADR 0039 Nr. 138).
fn ensure_smtp_auth(auth_type: &str) -> Result<()> {
    if auth_type != "smtp_password" {
        return Err(Error::Domain(
            "Die Microsoft-Anmeldung (Graph) ist im Server-Betrieb nicht verfügbar — bitte das \
             Postfach per SMTP einrichten."
                .into(),
        ));
    }
    Ok(())
}

/// Gespeicherte Ports unterliegen derselben Allowlist wie die Verbindungstests
/// (ASVS V5.2.6/V12.6.1, Lauf F5c; Standard 25/465/587/2525, Betreiber:
/// `KLEIN_BUCH_ALLOWED_TEST_PORTS`): was nicht getestet werden darf, wird auch nicht
/// für den Versand gespeichert. Private und Loopback-Adressen bleiben erlaubt
/// (Relay im LAN oder auf dem Docker-Host, ADR 0039 Nr. 136).
fn check_saved_port(ctx: &AppCtx, input: &mail_accounts::MailAccountInput) -> Result<()> {
    match input.smtp_port {
        None => Ok(()),
        Some(p) => ctx
            .net_tests
            .check_port(TestKind::Smtp, u16::try_from(p).unwrap_or(0)),
    }
}

/// SMTP-Host getrimmt speichern und prüfen (W14-017, Lauf F4c): nur Hostname
/// oder IP-Adresse; ein leerer Host wird zu `None`.
fn normalize_smtp_host(input: &mut mail_accounts::MailAccountInput) -> Result<()> {
    input.smtp_host = input
        .smtp_host
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map(str::to_string);
    if let Some(host) = input.smtp_host.as_deref() {
        crate::nettest::check_host(host)?;
    }
    Ok(())
}

/// Klartext-SMTP nur ohne Zugangsdaten (Plan W8).
fn plaintext_refused() -> Error {
    Error::Mail(
        "Unverschlüsseltes SMTP nur ohne Zugangsdaten — bitte TLS einschalten oder Benutzer \
         und Passwort entfernen."
            .into(),
    )
}

fn check_plaintext(input: &mail_accounts::MailAccountInput, password: Option<&str>) -> Result<()> {
    let has_user = input
        .smtp_user
        .as_deref()
        .is_some_and(|u| !u.trim().is_empty());
    if input.auth_type == "smtp_password"
        && !input.smtp_use_tls
        && (has_user || password.is_some_and(|p| !p.is_empty()))
    {
        return Err(plaintext_refused());
    }
    Ok(())
}

/// SMTP-Verbindungsdaten eines gespeicherten Kontos (Passwort aus der DB).
async fn smtp_config_for(pool: &SqlitePool, account: &MailAccountRow) -> Result<smtp::SmtpConfig> {
    let host = account
        .smtp_host
        .clone()
        .ok_or_else(|| Error::Mail("Mail-Account ohne SMTP-Host".into()))?;
    let port: u16 = account
        .smtp_port
        .and_then(|p| u16::try_from(p).ok())
        .ok_or_else(|| Error::Mail("Mail-Account ohne gültigen SMTP-Port".into()))?;
    let password = match account.keychain_service_id.as_deref() {
        Some(_) => secrets::get(pool, &secrets::smtp_key(&account.id)).await?,
        None => None,
    };
    Ok(smtp::SmtpConfig {
        host,
        port,
        use_tls: account.smtp_use_tls == 1,
        username: account.smtp_user.clone(),
        password,
    })
}

// =============================================================================
// Commands — E-Mail-Protokoll (Block 16b)
// =============================================================================

/// Argumente von [`email_log_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailLogListArgs {
    #[serde(default)]
    pub limit: Option<i64>,
}

/// Die jüngsten Protokoll-Einträge, neueste zuerst. `limit` Default 200 (1–1000).
pub async fn email_log_list(
    ctx: &Arc<AppCtx>,
    args: EmailLogListArgs,
) -> Result<Vec<crate::db::models::EmailLogRow>> {
    let EmailLogListArgs { limit } = args;
    let pool = ctx.pool()?;

    let limit = limit.unwrap_or(200).clamp(1, 1000);
    email_log::list(&pool, limit).await
}

/// Argumente von [`email_log_for`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailLogForArgs {
    pub kind: String,
    pub id: String,
}

/// Versand-Historie eines Belegs (`kind` = "invoice" | "quote", `id` = dessen ID).
pub async fn email_log_for(
    ctx: &Arc<AppCtx>,
    args: EmailLogForArgs,
) -> Result<Vec<crate::db::models::EmailLogRow>> {
    let EmailLogForArgs { kind, id } = args;
    let pool = ctx.pool()?;

    email_log::list_for(&pool, &kind, &id).await
}

/// Argumente von [`email_log_search`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailLogSearchArgs {
    pub filter: email_log::EmailLogFilter,
}

/// Serverseitige Suche/Filterung über das Protokoll (Volltext + Zeitfenster +
/// Status/Art/Kanal). Für die Protokoll-Seite.
pub async fn email_log_search(
    ctx: &Arc<AppCtx>,
    args: EmailLogSearchArgs,
) -> Result<Vec<crate::db::models::EmailLogRow>> {
    let EmailLogSearchArgs { filter } = args;
    let pool = ctx.pool()?;

    email_log::search(&pool, &filter).await
}

// =============================================================================
// Commands — Versand
// =============================================================================

/// Argumente von [`mail_invoice_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailInvoicePreviewArgs {
    pub invoice_id: String,
}

/// Erzeugt die Body-Vorschau (Betreff + Text) für eine Rechnung aus dem
/// `invoice-de`-Template — für die Send-UI.
pub async fn mail_invoice_preview(
    ctx: &Arc<AppCtx>,
    args: MailInvoicePreviewArgs,
) -> Result<RenderedMail> {
    let MailInvoicePreviewArgs { invoice_id } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    render_invoice_mail(&pool, &paths, &invoice_id).await
}

/// Argumente von [`mail_send_invoice`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSendInvoiceArgs {
    pub args: SendInvoiceArgs,
}

pub async fn mail_send_invoice(ctx: &Arc<AppCtx>, args: MailSendInvoiceArgs) -> Result<SendResult> {
    let MailSendInvoiceArgs { args } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    send_invoice_core(
        &pool,
        &paths,
        &args.account_id,
        &args.invoice_id,
        args.to.as_deref(),
        args.subject.as_deref(),
        args.body.as_deref(),
    )
    .await
}

/// Argumente von [`mail_send_test`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSendTestArgs {
    pub account_id: String,
    pub to: String,
}

/// Sendet eine kurze Test-Mail über ein **gespeichertes** Konto an `to`.
/// Im Gegensatz zu `mail_account_test_connection` (nur Connect/Login) prüft das
/// den vollständigen echten Pfad: Passwort aus `secrets` → SMTP → Zustellung.
pub async fn mail_send_test(ctx: &Arc<AppCtx>, args: MailSendTestArgs) -> Result<()> {
    let MailSendTestArgs { account_id, to } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let recipient = to.trim();
    if recipient.is_empty() {
        return Err(Error::Mail("Bitte eine Empfänger-Adresse angeben.".into()));
    }

    let account = mail_accounts::get(pool, &account_id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {account_id}")))?;
    ensure_smtp_auth(&account.auth_type)?;
    // Testversand wie ein Verbindungstest begrenzt (Plan W8).
    let port = account
        .smtp_port
        .and_then(|p| u16::try_from(p).ok())
        .unwrap_or(0);
    ctx.net_tests.check_port(TestKind::Smtp, port)?;
    ctx.net_tests.admit()?;
    let now = crate::clock::now_berlin()
        .format("%d.%m.%Y %H:%M")
        .to_string();
    let mail = smtp::OutgoingMail {
        from_name: account.from_name.clone(),
        from_email: account.from_email.clone(),
        to: recipient.to_string(),
        subject: "Klein.Buch — Test-Mail".into(),
        body_text: format!(
            "Dies ist eine Test-Mail von Klein.Buch.\n\n\
             Konto: {} <{}>\n\
             Gesendet am: {now}\n\n\
             Wenn diese Nachricht ankommt, ist der Versand korrekt konfiguriert.",
            account.from_name, account.from_email
        ),
        attachments: Vec::new(),
    };
    send_and_log(
        pool,
        &account,
        &mail,
        SendContext {
            related_kind: "test",
            related_id: None,
            related_number: None,
        },
    )
    .await?;

    audit_log::append(
        pool,
        "mail_account.test_send",
        "mail_account",
        &account_id,
        Some(serde_json::json!({ "to": recipient })),
    )
    .await?;

    Ok(())
}

/// Argumente von [`mail_quote_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailQuotePreviewArgs {
    pub quote_id: String,
}

/// Erzeugt die Body-Vorschau (Betreff + Text) für eine Angebots-Mail aus dem
/// `quote-de`-Template — für die Send-UI.
pub async fn mail_quote_preview(
    ctx: &Arc<AppCtx>,
    args: MailQuotePreviewArgs,
) -> Result<RenderedMail> {
    let MailQuotePreviewArgs { quote_id } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    render_quote_mail(&pool, &paths, &quote_id).await
}

/// Argumente von [`mail_send_quote`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSendQuoteArgs {
    pub args: SendQuoteArgs,
}

/// Angebotsversand (Block 8). Hängt das **Bundle** als Multi-Attachment an:
/// Angebots-PDF + aktive AGB + aktive Datenschutz-Version. Bindet die
/// ausgegebenen Legal-Versionen append-only ans Angebot (rechtlicher Nachweis)
/// und verlangt aktive AGB + Datenschutz (Pflicht für Versand).
pub async fn mail_send_quote(
    ctx: &Arc<AppCtx>,
    args: MailSendQuoteArgs,
) -> Result<SendQuoteResult> {
    let MailSendQuoteArgs { args } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    send_quote_core(
        &pool,
        &paths,
        &args.account_id,
        &args.quote_id,
        args.to.as_deref(),
        args.subject.as_deref(),
        args.body.as_deref(),
    )
    .await
}

// =============================================================================
// Core (ohne `AppCtx` — für Integration-/E2E-Tests aufrufbar)
// =============================================================================

/// Rendert Betreff + Body einer Rechnungs-Mail aus dem `invoice-de`-Template.
pub async fn render_invoice_mail(
    pool: &SqlitePool,
    paths: &Paths,
    invoice_id: &str,
) -> Result<RenderedMail> {
    let invoice = invoices::get(pool, invoice_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {invoice_id}")))?;
    render_invoice_mail_from(pool, paths, &invoice).await
}

/// Kern-Pipeline für den Rechnungsversand. Public, damit E2E-Tests sie ohne
/// Server treiben können (Paths kommen aus tempdir).
#[allow(clippy::too_many_arguments)]
pub async fn send_invoice_core(
    pool: &SqlitePool,
    paths: &Paths,
    account_id: &str,
    invoice_id: &str,
    to_override: Option<&str>,
    subject_override: Option<&str>,
    body_override: Option<&str>,
) -> Result<SendResult> {
    // 1. Account laden (Versand nur per SMTP, dispatch_send).
    let account = mail_accounts::get(pool, account_id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {account_id}")))?;

    // 2. Rechnung laden + Versand-Vorbedingungen prüfen.
    let invoice = invoices::get(pool, invoice_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Rechnung nicht gefunden: {invoice_id}")))?;
    if invoice.locked_at.is_none() {
        return Err(Error::Domain(
            "Rechnung ist noch nicht festgeschrieben — erst 'Lock & Issue' ausführen.".into(),
        ));
    }
    if invoice.status == "canceled" {
        return Err(Error::Domain(
            "Stornierte Rechnungen können nicht versendet werden.".into(),
        ));
    }
    let pdf_archive_id = invoice
        .pdf_archive_id
        .clone()
        .ok_or_else(|| Error::Domain("Rechnung hat kein archiviertes ZUGFeRD-PDF.".into()))?;

    // 3. Empfänger bestimmen: Override → Buyer-Snapshot → Live-Kontakt. Ein
    //    abweichender Empfänger verlangt eine frische Bestätigung (W14-009).
    let stored = resolve_recipient(pool, &invoice).await;
    let recipient = match to_override.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            require_step_up_for_recipient(t, stored.as_deref().ok())?;
            t.to_string()
        }
        None => stored?,
    };

    // 4. Betreff + Body: Overrides gewinnen, sonst Template.
    let rendered = render_invoice_mail_from(pool, paths, &invoice).await?;
    let subject = subject_override
        .map(|s| s.to_string())
        .unwrap_or(rendered.subject);
    let body = body_override
        .map(|b| b.to_string())
        .unwrap_or(rendered.body);

    // 5. ZUGFeRD-PDF aus dem Archiv lesen + Hash verifizieren.
    let pdf_bytes = archive::read_and_verify(pool, &pdf_archive_id).await?;
    let attachment = smtp::MailAttachment {
        filename: format!("{}.pdf", invoice.invoice_number),
        mime_type: "application/pdf".into(),
        bytes: pdf_bytes,
    };

    // 6. Mail bauen + per SMTP versenden.
    let mail = smtp::OutgoingMail {
        from_name: account.from_name.clone(),
        from_email: account.from_email.clone(),
        to: recipient.clone(),
        subject: subject.clone(),
        body_text: body,
        attachments: vec![attachment],
    };
    let receipt = send_and_log(
        pool,
        &account,
        &mail,
        SendContext {
            related_kind: "invoice",
            related_id: Some(invoice_id),
            related_number: Some(&invoice.invoice_number),
        },
    )
    .await?;

    // 8. GoBD: Status → 'sent', Audit-Log. Account-Nutzung tracken.
    let now = crate::clock::to_db_utc(crate::clock::now_utc());
    invoices::mark_sent(pool, invoice_id, &now).await?;
    mail_accounts::touch_last_used(pool, account_id).await.ok();

    audit_log::append(
        pool,
        "invoice.sent",
        "invoice",
        invoice_id,
        Some(serde_json::json!({ "account": account_id, "to": &recipient, "subject": &subject, "attachments": 1 })),
    )
    .await?;

    Ok(SendResult {
        invoice_id: invoice_id.to_string(),
        to: recipient,
        subject,
        attachment_count: 1,
        warning: receipt.warning(),
    })
}

/// Rendert Betreff + Body einer Angebots-Mail aus dem `quote-de`-Template.
pub async fn render_quote_mail(
    pool: &SqlitePool,
    paths: &Paths,
    quote_id: &str,
) -> Result<RenderedMail> {
    let quote = quotes::get(pool, quote_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {quote_id}")))?;
    let seller = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten (seller_profile) fehlen".into()))?;
    let ctx = templates::build_quote_context(&quote, &seller, kleinunternehmer::hinweis_text());
    let source = templates::load_quote_template(&paths.inputs_dir);
    templates::render_quote_mail(&source, &ctx)
}

/// Kern-Pipeline für den Angebotsversand (Bundle als Multi-Attachment). Public,
/// damit E2E-Tests sie ohne Server treiben können.
#[allow(clippy::too_many_arguments)]
pub async fn send_quote_core(
    pool: &SqlitePool,
    paths: &Paths,
    account_id: &str,
    quote_id: &str,
    to_override: Option<&str>,
    subject_override: Option<&str>,
    body_override: Option<&str>,
) -> Result<SendQuoteResult> {
    // 1. Account laden (Versand nur per SMTP, dispatch_send).
    let account = mail_accounts::get(pool, account_id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {account_id}")))?;

    // 2. Empfänger: Override → E-Mail des verknüpften Kontakts. Ein abweichender
    //    Empfänger verlangt eine frische Bestätigung (W14-009) — vor dem
    //    Binden der Rechtsdokumente, damit eine Ablehnung nichts schreibt.
    let quote_row = quotes::get(pool, quote_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Angebot nicht gefunden: {quote_id}")))?;
    let stored = resolve_quote_recipient(pool, &quote_row).await;
    let recipient = match to_override.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            require_step_up_for_recipient(t, stored.as_deref().ok())?;
            t.to_string()
        }
        None => stored?,
    };

    // 3. Bundle vorbereiten: Angebots-PDF sicherstellen + aktive Legal-Versionen
    //    append-only ans Angebot binden (Pflicht für Versand). Liefert die
    //    Archiv-IDs für die Anhänge.
    let dispatch = crate::commands::quotes::prepare_quote_dispatch(pool, paths, quote_id).await?;

    // 4. Betreff + Body: Overrides gewinnen, sonst Template.
    let rendered = render_quote_mail(pool, paths, quote_id).await?;
    let subject = subject_override
        .map(|s| s.to_string())
        .unwrap_or(rendered.subject);
    let body = body_override
        .map(|b| b.to_string())
        .unwrap_or(rendered.body);

    // 5. Anhänge: Angebots-PDF + jede gebundene Legal-Version (Hash-verifiziert).
    let mut attachments = Vec::new();
    let quote_pdf = archive::read_and_verify(pool, &dispatch.quote_pdf_archive_id).await?;
    attachments.push(smtp::MailAttachment {
        filename: format!("{}.pdf", dispatch.quote.quote_number),
        mime_type: "application/pdf".into(),
        bytes: quote_pdf,
    });
    for ld in &dispatch.legal {
        let bytes = archive::read_and_verify(pool, &ld.archive_entry_id).await?;
        attachments.push(smtp::MailAttachment {
            filename: legal_attachment_name(&ld.doc_type),
            mime_type: "application/pdf".into(),
            bytes,
        });
    }
    let attachment_count = attachments.len() as u32;

    // 6. Mail bauen + per SMTP versenden.
    let mail = smtp::OutgoingMail {
        from_name: account.from_name.clone(),
        from_email: account.from_email.clone(),
        to: recipient.clone(),
        subject: subject.clone(),
        body_text: body,
        attachments,
    };
    let receipt = send_and_log(
        pool,
        &account,
        &mail,
        SendContext {
            related_kind: "quote",
            related_id: Some(quote_id),
            related_number: Some(&dispatch.quote.quote_number),
        },
    )
    .await?;

    // 8. GoBD: KEIN Status-Wechsel (Angebot ist mit dem Festschreiben bereits
    //    'sent'). Audit-Log mit den gebundenen Legal-Versionen (Nachweis,
    //    welche Fassung an wen ging). Account-Nutzung tracken.
    mail_accounts::touch_last_used(pool, account_id).await.ok();
    let legal_audit: serde_json::Map<String, serde_json::Value> = dispatch
        .legal
        .iter()
        .map(|l| (l.doc_type.clone(), serde_json::Value::from(l.version)))
        .collect();
    audit_log::append(
        pool,
        "quote.sent",
        "quote",
        quote_id,
        Some(serde_json::json!({
            "account": account_id,
            "to": &recipient,
            "subject": &subject,
            "attachments": attachment_count,
            "legal": legal_audit,
        })),
    )
    .await?;

    Ok(SendQuoteResult {
        quote_id: quote_id.to_string(),
        to: recipient,
        subject,
        attachment_count,
        warning: receipt.warning(),
    })
}

// =============================================================================
// Versand-Dispatch (SMTP)
// =============================================================================

/// Vereinheitlichte Provider-Antwort eines Versands — Grundlage für das
/// E-Mail-Protokoll (Block 16b).
pub(crate) struct SendReceipt {
    channel: &'static str,
    provider_code: Option<String>,
    provider_message: Option<String>,
    request_id: Option<String>,
    /// Der Eintrag im `email_log` konnte nicht geschrieben werden (R4-S2-EMAILLOG).
    log_failed: bool,
}

impl SendReceipt {
    /// Warnung für den Aufrufer, wenn das Versandprotokoll unvollständig ist.
    pub(crate) fn warning(&self) -> Option<String> {
        self.log_failed.then(|| LOG_FAILED_WARNING.to_string())
    }
}

/// Warnung an den Aufrufer bei unvollständigem Versandprotokoll.
pub(crate) const LOG_FAILED_WARNING: &str =
    "Die E-Mail wurde versendet, der Eintrag im Versandprotokoll fehlt aber (Hinweis in der \
     Inbox; der Versand steht im Audit-Log).";

/// Versendet eine fertig zusammengebaute Mail per SMTP (Passwort aus `secrets`).
/// Liefert die Provider-Antwort zurück. Microsoft Graph gibt es im Server nicht
/// (E9); ältere Konten dieser Art lehnt [`ensure_smtp_auth`] ab.
async fn dispatch_send(
    pool: &SqlitePool,
    account: &MailAccountRow,
    mail: &smtp::OutgoingMail,
) -> Result<SendReceipt> {
    ensure_smtp_auth(&account.auth_type)?;
    // smtp_password (Default). Ohne gespeichertes Passwort wird ohne Auth
    // gesendet (interner Relay, Mailpit im E2E-Test).
    let config = smtp_config_for(pool, account).await?;
    let info = smtp::send(&config, mail).await?;
    Ok(SendReceipt {
        channel: "smtp",
        provider_code: info.code,
        provider_message: info.message,
        request_id: None,
        log_failed: false,
    })
}

/// Beleg-Bezug eines Versands für das Protokoll.
pub(crate) struct SendContext<'a> {
    pub(crate) related_kind: &'a str,
    pub(crate) related_id: Option<&'a str>,
    pub(crate) related_number: Option<&'a str>,
}

/// Versendet eine Mail UND protokolliert den Versuch (Block 16b): schreibt JEDEN
/// Versand — Erfolg wie Fehlschlag — in das append-only `email_log` und reicht
/// das Versand-Ergebnis durch. Ein Fehler beim Protokollieren überschreibt nie das
/// Versand-Ergebnis; er landet im Audit-Log und in der Inbox und setzt
/// [`SendReceipt::warning`] ([`report_log_failure`]).
pub(crate) async fn send_and_log(
    pool: &SqlitePool,
    account: &MailAccountRow,
    mail: &smtp::OutgoingMail,
    ctx: SendContext<'_>,
) -> Result<SendReceipt> {
    let result = dispatch_send(pool, account, mail).await;
    let entry = match &result {
        Ok(rc) => email_log::EmailLogEntry {
            account_id: Some(account.id.clone()),
            account_label: Some(account.label.clone()),
            channel: rc.channel.to_string(),
            related_kind: ctx.related_kind.to_string(),
            related_id: ctx.related_id.map(str::to_string),
            related_number: ctx.related_number.map(str::to_string),
            from_email: account.from_email.clone(),
            to_email: mail.to.clone(),
            subject: mail.subject.clone(),
            attachment_count: mail.attachments.len() as i64,
            status: "success".to_string(),
            provider_code: rc.provider_code.clone(),
            provider_message: rc.provider_message.clone(),
            request_id: rc.request_id.clone(),
            error: None,
        },
        Err(e) => email_log::EmailLogEntry {
            account_id: Some(account.id.clone()),
            account_label: Some(account.label.clone()),
            channel: "smtp".to_string(),
            related_kind: ctx.related_kind.to_string(),
            related_id: ctx.related_id.map(str::to_string),
            related_number: ctx.related_number.map(str::to_string),
            from_email: account.from_email.clone(),
            to_email: mail.to.clone(),
            subject: mail.subject.clone(),
            attachment_count: mail.attachments.len() as i64,
            status: "failed".to_string(),
            provider_code: None,
            provider_message: None,
            request_id: None,
            error: Some(e.to_string()),
        },
    };
    match email_log::insert(pool, &entry).await {
        Ok(_) => result,
        Err(log_err) => {
            report_log_failure(pool, &entry, &log_err).await;
            result.map(|mut rc| {
                rc.log_failed = true;
                rc
            })
        }
    }
}

/// R4-S2-EMAILLOG (Lauf F5c): Der Versand ist nicht rückholbar; fehlt sein Eintrag im
/// append-only `email_log`, hält Klein.Buch ihn im Audit-Log fest (`mail.log_failed`,
/// mit Empfänger, Betreff und Beleg — wie `invoice.sent`) und legt einen Hinweis in
/// die Inbox. Beides nach bestem Bemühen: ein weiterer Fehler wird nur geloggt.
async fn report_log_failure(pool: &SqlitePool, entry: &email_log::EmailLogEntry, err: &Error) {
    tracing::error!(
        error = %err,
        related_kind = %entry.related_kind,
        "E-Mail-Protokoll konnte nicht geschrieben werden"
    );
    let sent = entry.status == "success";
    if let Err(e) = audit_log::append(
        pool,
        "mail.log_failed",
        "email_log",
        entry.related_id.as_deref().unwrap_or("-"),
        Some(serde_json::json!({
            "related_kind": &entry.related_kind,
            "related_number": &entry.related_number,
            "status": &entry.status,
            "account": &entry.account_id,
            "to": &entry.to_email,
            "subject": &entry.subject,
            "error": err.to_string(),
        })),
    )
    .await
    {
        tracing::error!(error = %e, "Audit mail.log_failed konnte nicht geschrieben werden");
    }
    let what = match entry.related_number.as_deref() {
        Some(n) => format!("Die E-Mail zu {n}"),
        None => "Eine E-Mail".to_string(),
    };
    let outcome = if sent {
        "wurde versendet"
    } else {
        "konnte nicht versendet werden"
    };
    let body = format!(
        "{what} {outcome}, der Eintrag im Versandprotokoll fehlt aber ({err}). Den Versuch \
         belegt das Audit-Log (mail.log_failed). Bitte die Ursache prüfen (z. B. freier \
         Speicherplatz)."
    );
    if let Err(e) = crate::notify::store::create(
        pool,
        crate::notify::store::NewNotification {
            title: "Versandprotokoll unvollständig",
            body: &body,
            severity: "warning",
            related_entity_type: Some(entry.related_kind.as_str()),
            related_entity_id: entry.related_id.as_deref(),
            action_url: Some("/settings/mail-log"),
            ..Default::default()
        },
    )
    .await
    {
        tracing::error!(error = %e, "Hinweis „Versandprotokoll unvollständig“ fehlgeschlagen");
    }
}

// =============================================================================
// Step-up beim Versand an einen abweichenden Empfänger (W14-009, Lauf F5c)
// =============================================================================

/// Verlangt eine frische Bestätigung (Passphrase + zweiter Faktor, bzw. eine
/// Anmeldung der letzten 5 min), wenn `recipient` nicht die hinterlegte Adresse
/// des Belegs ist (Rechnung: Käufer-Snapshot, sonst Kontakt; Angebot: Kontakt).
/// Groß-/Kleinschreibung zählt nicht. Ohne hinterlegte Adresse ist jeder
/// eingegebene Empfänger „abweichend“. Betreff und Text bleiben frei (ADR 0039
/// Nr. 136): Abwägung BEC-Risiko gegen Bedienbarkeit, Begründung in security.md.
pub(crate) fn require_step_up_for_recipient(recipient: &str, stored: Option<&str>) -> Result<()> {
    if stored.is_some_and(|s| s.trim().eq_ignore_ascii_case(recipient.trim())) {
        Ok(())
    } else {
        crate::commands::reauth::require_fresh()
    }
}

/// Vorprüfung von [`mail_send_invoice`] vor der Annahme des Jobs: ein
/// abweichender Empfänger ergibt sofort 403 `reauth_required` (Dialog im
/// Frontend, danach dieselbe Anfrage). Fehlt die Rechnung, meldet das der Kern.
pub async fn mail_send_invoice_precheck(
    ctx: &Arc<AppCtx>,
    args: &MailSendInvoiceArgs,
) -> Result<()> {
    let Some(to) = args
        .args
        .to
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return Ok(());
    };
    let pool = ctx.pool()?;
    let Some(invoice) = invoices::get(&pool, &args.args.invoice_id).await? else {
        return Ok(());
    };
    let stored = resolve_recipient(&pool, &invoice).await.ok();
    require_step_up_for_recipient(to, stored.as_deref())
}

/// Vorprüfung von [`mail_send_quote`] (wie [`mail_send_invoice_precheck`]).
pub async fn mail_send_quote_precheck(ctx: &Arc<AppCtx>, args: &MailSendQuoteArgs) -> Result<()> {
    let Some(to) = args
        .args
        .to
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return Ok(());
    };
    let pool = ctx.pool()?;
    let Some(quote) = quotes::get(&pool, &args.args.quote_id).await? else {
        return Ok(());
    };
    let stored = resolve_quote_recipient(&pool, &quote).await.ok();
    require_step_up_for_recipient(to, stored.as_deref())
}

// =============================================================================
// Helpers
// =============================================================================

async fn resolve_quote_recipient(
    pool: &SqlitePool,
    quote: &crate::db::models::QuoteRow,
) -> Result<String> {
    let contact = contacts::get(pool, &quote.contact_id).await?;
    contact
        .and_then(|c| c.email)
        .filter(|e| !e.trim().is_empty())
        .map(|e| e.trim().to_string())
        .ok_or_else(|| {
            Error::Domain(
                "Kein Empfänger: beim verknüpften Kontakt ist keine E-Mail hinterlegt.".into(),
            )
        })
}

/// Sprechender Anhang-Dateiname je Dokumenttyp fürs Bundle.
fn legal_attachment_name(doc_type: &str) -> String {
    match doc_type {
        "agb" => "AGB.pdf".to_string(),
        "privacy" => "Datenschutz.pdf".to_string(),
        other => format!("{other}.pdf"),
    }
}

async fn resolve_recipient(
    pool: &SqlitePool,
    invoice: &crate::db::models::InvoiceRow,
) -> Result<String> {
    if let Some(email) = invoice.buyer_email.as_deref() {
        if !email.trim().is_empty() {
            return Ok(email.trim().to_string());
        }
    }
    let contact = contacts::get(pool, &invoice.contact_id).await?;
    contact
        .and_then(|c| c.email)
        .filter(|e| !e.trim().is_empty())
        .map(|e| e.trim().to_string())
        .ok_or_else(|| {
            Error::Domain(
                "Kein Empfänger: weder auf der Rechnung noch beim Kontakt ist eine E-Mail hinterlegt.".into(),
            )
        })
}

/// Wie [`render_invoice_mail`], aber mit bereits geladener Rechnung.
/// Storno-Belege bekommen den Storno-Text (`storno-de` bzw. eingebettet) mit
/// Nummer und Datum der stornierten Rechnung (Lauf E).
async fn render_invoice_mail_from(
    pool: &SqlitePool,
    paths: &Paths,
    invoice: &crate::db::models::InvoiceRow,
) -> Result<RenderedMail> {
    let seller = seller_profile::get(pool)
        .await?
        .ok_or_else(|| Error::Domain("Stammdaten (seller_profile) fehlen".into()))?;
    let mut ctx =
        templates::build_invoice_context(invoice, &seller, kleinunternehmer::hinweis_text());
    let source = match invoice.is_storno_for.as_deref() {
        Some(original_id) => {
            let original = invoices::get(pool, original_id)
                .await?
                .ok_or_else(|| Error::Domain("Storno: Original-Rechnung fehlt".into()))?;
            ctx.invoice.storno_of = Some(templates::StornoRefCtx {
                number: original.invoice_number,
                date: templates::format_german_date(&original.invoice_date),
            });
            templates::load_storno_template(&paths.inputs_dir)
        }
        None => templates::load_template(&paths.inputs_dir, "invoice-de")?,
    };
    templates::render(&source, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lauf F4d (S3): SMTP-Passwörter erscheinen nicht in der Debug-Ausgabe.
    #[test]
    fn debug_output_of_password_args_is_redacted() {
        let args = TestConnectionArgs {
            smtp_host: "smtp.example.org".into(),
            smtp_port: 587,
            smtp_use_tls: true,
            smtp_user: Some("kb@example.org".into()),
            password: Some("smtp-geheim-4711".into()),
        };
        let out = format!("{:?}", MailAccountTestConnectionArgs { args });
        assert!(!out.contains("smtp-geheim-4711"), "{out}");
        assert!(out.contains("password: Some(***)"), "{out}");
        assert!(out.contains("smtp.example.org"), "{out}");
    }
}
