//! End-to-End-Happy-Path für Block 5 (SMTP-Versand).
//!
//! Sequenz: Seller-Profile setzen → Contact anlegen → Rechnung anlegen +
//! locken (Mock-Sidecar) → über einen Mail-Account gegen **Mailpit**
//! versenden. Anschließend wird über die Mailpit-HTTP-API verifiziert, dass
//! die Mail samt ZUGFeRD-PDF-Anhang angekommen ist und die DB den Versand
//! festgehalten hat (`status='sent'`, Audit-Log-Eintrag).
//!
//! ## Lauf-Voraussetzung
//!
//! Ein Mailpit auf `127.0.0.1` (SMTP 1025, HTTP 8025). In CI als Docker-
//! Service (`axllent/mailpit`, per Digest gepinnt, siehe `.github/workflows/ci.yml`;
//! bis Lauf F7a MailHog). **Lokal ohne Mailpit wird der Test übersprungen**
//! (no-op, `cargo test` bleibt grün) — die Mail-Bausteine sind zusätzlich über
//! Unit-Tests abgedeckt.
//!
//! Der Sidecar (KoSIT/Mustang) ist über `KLEIN_BUCH_SIDECAR_MOCK=1` gemockt;
//! der Keychain läuft im plattform-unabhängigen Mock-Store, sodass der Test
//! ohne Java/Secret-Service-Daemon läuft.

mod smtp_support;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chrono::NaiveDate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use klein_buch_lib::commands::invoices as inv_cmd;
use klein_buch_lib::commands::mail as mail_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, invoices, mail_accounts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};

const TEMPLATE_TYP: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json(bytes(sys.inputs.at("data-json")))
= Rechnung #data.invoice.number
Empfänger: #data.buyer.name \
#data.invoice.gross_amount
#data.kleinunternehmer.hinweis_text
"#;

const MAIL_TEMPLATE: &str = "Subject: Rechnung {{ invoice.number }} von {{ seller.name }}\n\
\n\
Sehr geehrte Damen und Herren,\n\
\n\
anbei Rechnung {{ invoice.number }} vom {{ invoice.date }} ueber {{ invoice.gross_amount_formatted }}.\n\
{% if invoice.is_kleinunternehmer -%}\n\
{{ kleinunternehmer.hinweis_text }}\n\
{% endif -%}\n";

struct Env {
    pool: SqlitePool,
    paths: Paths,
    _tmp: tempfile::TempDir,
}

async fn setup_env() -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let archive_dir = data_dir.join("archive");
    let backups_dir = data_dir.join("backups");
    let inputs_dir = tmp.path().join("inputs");
    let templates_dir = inputs_dir.join("pdf-templates");
    let mail_templates_dir = inputs_dir.join("mail-templates");
    let db_file = data_dir.join("test.sqlite");

    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::create_dir_all(&backups_dir).unwrap();
    std::fs::create_dir_all(&templates_dir).unwrap();
    std::fs::create_dir_all(&mail_templates_dir).unwrap();
    std::fs::write(templates_dir.join("default.typ"), TEMPLATE_TYP).unwrap();
    std::fs::write(mail_templates_dir.join("invoice-de.txt"), MAIL_TEMPLATE).unwrap();

    let url = format!("sqlite://{}", db_file.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();

    seller_profile::upsert(
        &pool,
        &seller_profile::SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "rechnung@example.org".into(),
            phone: Some("+49 000 000000".into()),
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: true,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
            confirm_waive_paragraph_19: Some(true),
        },
    )
    .await
    .unwrap();

    let paths = Paths {
        data_dir,
        db_file,
        archive_dir,
        backups_dir,
        inputs_dir,
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
        resource_dir: std::path::PathBuf::from("/non/existent/resources"),
        web_dir: std::path::PathBuf::from("/non/existent/web"),
        backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
    };

    Env {
        pool,
        paths,
        _tmp: tmp,
    }
}

async fn create_locked_invoice(env: &Env) -> (String, String) {
    let contact_id = contacts::create(
        &env.pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: Some("DE111111111".into()),
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: Some("empfaenger@example.org".into()),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        },
    )
    .await
    .unwrap()
    .id;

    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        delivery_date: Some(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()),
        due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 19).unwrap()),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 4.0,
            unit_code: "HUR".into(),
            unit_price_cents: 12_500,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };

    let seller = seller_profile::get(&env.pool).await.unwrap().unwrap();
    let totals = klein_buch_lib::domain::invoice::compute_totals(&input.items);
    let number = klein_buch_lib::db::numbering::next_number(
        &env.pool,
        klein_buch_lib::domain::numbering::DocType::Invoice,
        2026,
    )
    .await
    .unwrap();
    let snapshot = invoices::SellerSnapshot {
        name: &seller.name,
        street: &seller.street,
        postal_code: &seller.postal_code,
        city: &seller.city,
        tax_number: seller.tax_number.as_deref(),
        vat_id: seller.vat_id.as_deref(),
    };
    let buyer = contacts::get(&env.pool, &contact_id)
        .await
        .unwrap()
        .unwrap();
    let buyer_snapshot = invoices::BuyerSnapshot {
        name: &buyer.name,
        street: buyer.street.as_deref(),
        postal_code: buyer.postal_code.as_deref(),
        city: buyer.city.as_deref(),
        country_code: &buyer.country_code,
        vat_id: buyer.vat_id.as_deref(),
        email: buyer.email.as_deref(),
    };
    let payload = invoices::DraftCreatePayload {
        contact_id: contact_id.clone(),
        fiscal_year: 2026,
        is_kleinunternehmer: true,
        input,
        derived_from_quote_id: None,
    };
    let draft = invoices::create_draft(
        &env.pool,
        &payload,
        &number,
        &snapshot,
        &buyer_snapshot,
        &totals,
    )
    .await
    .unwrap();

    inv_cmd::run_lock_pipeline(&env.pool, &env.paths, &draft.id, "N/A")
        .await
        .expect("lock pipeline (mock sidecar) muss grün laufen");

    (draft.id, number)
}

async fn create_mailpit_account(pool: &SqlitePool) -> String {
    // auth_type smtp_password, aber KEINE Passphrase gespeichert → Versand
    // ohne Auth (Mailpit nimmt ohne Anmeldung an). use_tls=false → builder_dangerous.
    mail_accounts::create(
        pool,
        &mail_accounts::MailAccountInput {
            label: "Mailpit (E2E)".into(),
            auth_type: "smtp_password".into(),
            smtp_host: Some("127.0.0.1".into()),
            smtp_port: Some(1025),
            smtp_user: None,
            smtp_use_tls: false,
            from_email: "rechnung@example.org".into(),
            from_name: "Beispiel IT-Service".into(),
            is_default: true,
        },
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn send_invoice_happy_path_against_mailpit() {
    if !port_reachable("127.0.0.1:1025") {
        eprintln!("Mailpit (127.0.0.1:1025) nicht erreichbar — E2E-Test übersprungen.");
        return;
    }

    let env = setup_env().await;
    let (invoice_id, number) = create_locked_invoice(&env).await;
    let account_id = create_mailpit_account(&env.pool).await;

    let result = mail_cmd::send_invoice_core(
        &env.pool,
        &env.paths,
        &account_id,
        &invoice_id,
        None,
        None,
        None,
    )
    .await
    .expect("Versand gegen Mailpit muss gelingen");

    assert_eq!(result.attachment_count, 1);
    assert_eq!(result.to, "empfaenger@example.org");

    // DB: Status auf 'sent', sent_at gesetzt, Audit-Eintrag vorhanden.
    let inv = invoices::get(&env.pool, &invoice_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(inv.status, "sent");
    assert!(inv.sent_at.is_some());
    let sent_audit: i64 =
        sqlx::query("SELECT COUNT(*) AS n FROM audit_log WHERE action = 'invoice.sent'")
            .fetch_one(&env.pool)
            .await
            .unwrap()
            .get("n");
    assert_eq!(sent_audit, 1, "Audit-Log muss invoice.sent enthalten");

    // Mailpit: Mail angekommen, mit ZUGFeRD-PDF-Anhang + gerendertem Betreff.
    let messages = mailpit_latest_message().expect("Mailpit-API muss antworten");
    assert!(
        messages.contains("empfaenger@example.org"),
        "Empfänger fehlt in Mailpit"
    );
    assert!(
        messages.contains(&format!("{number}.pdf")),
        "ZUGFeRD-Attachment {number}.pdf fehlt in Mailpit"
    );
    assert!(
        messages.contains("application/pdf"),
        "PDF-Content-Type fehlt in Mailpit"
    );
    assert!(
        messages.contains(&format!("Rechnung {number}")),
        "Gerenderter Betreff fehlt in Mailpit"
    );
}

// ---- Helpers ---------------------------------------------------------------

fn port_reachable(addr: &str) -> bool {
    let Some(sa) = addr.to_socket_addrs().ok().and_then(|mut it| it.next()) else {
        return false;
    };
    TcpStream::connect_timeout(&sa, Duration::from_secs(1)).is_ok()
}

/// Holt die zuletzt angekommene Mail über die Mailpit-HTTP-API
/// (`/api/v1/message/latest`: Kopf, Text und Anhänge mit Dateiname und
/// Content-Type als JSON). Bewusst minimaler HTTP/1.0-GET ohne extra Crate.
fn mailpit_latest_message() -> Option<String> {
    let sa = "127.0.0.1:8025".to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&sa, Duration::from_secs(2)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream
        .write_all(
            b"GET /api/v1/message/latest HTTP/1.0\r\nHost: 127.0.0.1:8025\r\nConnection: close\r\n\r\n",
        )
        .ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status = text.lines().next().unwrap_or_default();
    assert!(status.contains(" 200 "), "Mailpit-API: {status}");
    Some(text)
}

/// Konto auf der Test-Gegenstelle (`tests/smtp_support`), Klartext ohne Login.
async fn create_fake_smtp_account(pool: &SqlitePool, port: u16) -> String {
    mail_accounts::create(
        pool,
        &mail_accounts::MailAccountInput {
            label: "Test-SMTP".into(),
            auth_type: "smtp_password".into(),
            smtp_host: Some("127.0.0.1".into()),
            smtp_port: Some(i64::from(port)),
            smtp_user: None,
            smtp_use_tls: false,
            from_email: "rechnung@example.org".into(),
            from_name: "Beispiel IT-Service".into(),
            is_default: true,
        },
    )
    .await
    .unwrap()
    .id
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// R4-S2-EMAILLOG (Lauf F5c): Kann der Eintrag im append-only `email_log` nach
/// einem gelungenen Versand nicht geschrieben werden, bleibt der Versand gültig
/// (Rechnung „versendet“), aber Klein.Buch verschweigt es nicht: Audit
/// `mail.log_failed` mit Empfänger und Beleg, Hinweis in der Inbox, Warnung im
/// Ergebnis. Ohne Fehler gibt es keine Warnung.
#[tokio::test]
async fn failed_email_log_entry_is_reported_not_swallowed() {
    let server = smtp_support::start(None).await;
    let env = setup_env().await;
    let (invoice_id, number) = create_locked_invoice(&env).await;
    let account_id = create_fake_smtp_account(&env.pool, server.port).await;

    // Nur im Test: das Protokoll nimmt nichts an.
    sqlx::query(
        "CREATE TRIGGER test_email_log_fail BEFORE INSERT ON email_log
         BEGIN SELECT RAISE(ABORT, 'Testfehler: Protokoll nicht schreibbar'); END",
    )
    .execute(&env.pool)
    .await
    .unwrap();

    let result = mail_cmd::send_invoice_core(
        &env.pool,
        &env.paths,
        &account_id,
        &invoice_id,
        None,
        None,
        None,
    )
    .await
    .expect("der Versand selbst gelingt");
    let warning = result.warning.expect("Warnung an den Aufrufer");
    assert!(warning.contains("E-Mail-Protokoll"), "{warning}");
    assert_eq!(server.messages().len(), 1, "Mail ist hinausgegangen");
    let inv = invoices::get(&env.pool, &invoice_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(inv.status, "sent");
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM email_log").await, 0);

    let details: String =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'mail.log_failed'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(details["related_kind"], "invoice");
    assert_eq!(details["related_number"], number.as_str());
    assert_eq!(details["status"], "success");
    assert_eq!(details["to"], "empfaenger@example.org");
    assert!(details["error"]
        .as_str()
        .unwrap()
        .contains("Protokoll nicht schreibbar"));
    let notice: (String, String) = sqlx::query_as(
        "SELECT severity, body FROM notifications WHERE title = 'E-Mail-Protokoll unvollständig'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(notice.0, "warning");
    assert!(notice.1.contains(&number), "{}", notice.1);
    assert!(notice.1.contains("wurde versendet"), "{}", notice.1);

    // Protokoll wieder schreibbar: kein Hinweis, keine Warnung.
    sqlx::query("DROP TRIGGER test_email_log_fail")
        .execute(&env.pool)
        .await
        .unwrap();
    let result = mail_cmd::send_invoice_core(
        &env.pool,
        &env.paths,
        &account_id,
        &invoice_id,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(result.warning.is_none());
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM email_log").await, 1);
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'mail.log_failed'"
        )
        .await,
        1
    );
    assert_eq!(server.messages().len(), 2);
}

/// W14-009 (Lauf F5c): Im Kern verlangt ein abweichender Empfänger eine frische
/// Bestätigung — vor jedem Versandversuch; ohne Override (Abo-Lauf) nie.
#[tokio::test]
async fn deviating_recipient_requires_fresh_confirmation_in_core() {
    use klein_buch_lib::commands::reauth::with_reauth;
    use klein_buch_lib::error::Error;
    let server = smtp_support::start(None).await;
    let env = setup_env().await;
    let (invoice_id, _) = create_locked_invoice(&env).await;
    let account_id = create_fake_smtp_account(&env.pool, server.port).await;
    let send = |to: Option<&'static str>| {
        mail_cmd::send_invoice_core(
            &env.pool,
            &env.paths,
            &account_id,
            &invoice_id,
            to,
            None,
            Some("Eigener Text"),
        )
    };

    // Ohne Bestätigung (wie ein interner Aufruf): abweichend → abgelehnt.
    let err = send(Some("fremd@example.net")).await.unwrap_err();
    assert!(matches!(err, Error::ReauthRequired), "{err:?}");
    let err = with_reauth(false, send(Some("fremd@example.net")))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::ReauthRequired), "{err:?}");
    assert!(server.messages().is_empty(), "kein Versandversuch");
    // Hinterlegter Empfänger (andere Schreibweise) und kein Override: frei.
    send(None).await.expect("hinterlegter Empfänger");
    send(Some(" Empfaenger@Example.org "))
        .await
        .expect("Schreibweise");
    // Mit frischer Bestätigung: abweichender Empfänger erlaubt.
    with_reauth(true, send(Some("fremd@example.net")))
        .await
        .expect("nach Re-Auth");
    assert_eq!(server.messages().len(), 3);
}
