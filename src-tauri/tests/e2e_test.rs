//! End-to-End-Happy-Path für Block 5 (SMTP-Versand).
//!
//! Sequenz: Seller-Profile setzen → Contact anlegen → Rechnung anlegen +
//! locken (Mock-Sidecar) → über einen Mail-Account gegen **MailHog**
//! versenden. Anschließend wird über die MailHog-HTTP-API verifiziert, dass
//! die Mail samt ZUGFeRD-PDF-Anhang angekommen ist und die DB den Versand
//! festgehalten hat (`status='sent'`, Audit-Log-Eintrag).
//!
//! ## Lauf-Voraussetzung
//!
//! Ein MailHog auf `127.0.0.1` (SMTP 1025, HTTP 8025). In CI als Docker-
//! Service (`mailhog/mailhog`, siehe `.github/workflows/ci.yml`). **Lokal
//! ohne MailHog wird der Test übersprungen** (no-op, `cargo test` bleibt
//! grün) — die Mail-Bausteine sind zusätzlich über Unit-Tests abgedeckt.
//!
//! Der Sidecar (KoSIT/Mustang) ist über `KLEIN_BUCH_SIDECAR_MOCK=1` gemockt;
//! der Keychain läuft im plattform-unabhängigen Mock-Store, sodass der Test
//! ohne Java/Secret-Service-Daemon läuft.

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
#let data = json.decode(sys.inputs.at("data-json"))
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
    // Plattform-unabhängiger Keychain-Mock (kein Secret-Service-Daemon nötig).
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

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
            name: "Wildbach Computerhilfe".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "schmidm@wildbach-computerhilfe.de".into(),
            phone: None,
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
            email: Some("empfaenger@kunde-test.de".into()),
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

async fn create_mailhog_account(pool: &SqlitePool) -> String {
    // auth_type smtp_password, aber KEINE Passphrase gespeichert → Versand
    // ohne Auth (MailHog ist ein offener Relay). use_tls=false → builder_dangerous.
    mail_accounts::create(
        pool,
        &mail_accounts::MailAccountInput {
            label: "MailHog (E2E)".into(),
            auth_type: "smtp_password".into(),
            smtp_host: Some("127.0.0.1".into()),
            smtp_port: Some(1025),
            smtp_user: None,
            smtp_use_tls: false,
            from_email: "schmidm@wildbach-computerhilfe.de".into(),
            from_name: "Wildbach Computerhilfe".into(),
            is_default: true,
            oauth_tenant_id: None,
            oauth_client_id: None,
        },
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn send_invoice_happy_path_against_mailhog() {
    if !port_reachable("127.0.0.1:1025") {
        eprintln!("MailHog (127.0.0.1:1025) nicht erreichbar — E2E-Test übersprungen.");
        return;
    }

    let env = setup_env().await;
    let (invoice_id, number) = create_locked_invoice(&env).await;
    let account_id = create_mailhog_account(&env.pool).await;

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
    .expect("Versand gegen MailHog muss gelingen");

    assert_eq!(result.attachment_count, 1);
    assert_eq!(result.to, "empfaenger@kunde-test.de");

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

    // MailHog: Mail angekommen, mit ZUGFeRD-PDF-Anhang + gerendertem Betreff.
    let messages = mailhog_messages().expect("MailHog-API muss antworten");
    assert!(
        messages.contains("empfaenger@kunde-test.de"),
        "Empfänger fehlt in MailHog"
    );
    assert!(
        messages.contains(&format!("{number}.pdf")),
        "ZUGFeRD-Attachment {number}.pdf fehlt in MailHog"
    );
    assert!(
        messages.contains("application/pdf"),
        "PDF-Content-Type fehlt in MailHog"
    );
    assert!(
        messages.contains(&format!("Rechnung {number}")),
        "Gerenderter Betreff fehlt in MailHog"
    );
}

// ---- Helpers ---------------------------------------------------------------

fn port_reachable(addr: &str) -> bool {
    let Some(sa) = addr.to_socket_addrs().ok().and_then(|mut it| it.next()) else {
        return false;
    };
    TcpStream::connect_timeout(&sa, Duration::from_secs(1)).is_ok()
}

/// Holt die MailHog-Message-Liste über die HTTP-API (v2). Bewusst minimaler
/// HTTP/1.0-GET ohne extra Crate-Dependency.
fn mailhog_messages() -> Option<String> {
    let sa = "127.0.0.1:8025".to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&sa, Duration::from_secs(2)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream
        .write_all(
            b"GET /api/v2/messages HTTP/1.0\r\nHost: 127.0.0.1:8025\r\nConnection: close\r\n\r\n",
        )
        .ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}
