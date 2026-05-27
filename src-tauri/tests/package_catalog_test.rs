//! Integrationstest für den Paket-Katalog-Broschüren-Versand (Block P4).
//!
//! Die Broschüre ist KEIN §14-Beleg: kein Nummernkreis, KEIN write-once-Archiv.
//! Der Versand wird ausschließlich im append-only `email_log`
//! (`related_kind = 'package_catalog'`) protokolliert — und zwar je Versuch
//! GENAU EINMAL, auch wenn der eigentliche Versand fehlschlägt.
//!
//! Damit der Test ohne MailHog/SMTP-Server deterministisch läuft, zeigt der
//! Mail-Account auf einen geschlossenen Port (Connection refused). Der Versand
//! schlägt fehl → `send_and_log` schreibt genau einen `failed`-Eintrag. So wird
//! die GoBD-relevante Garantie „ein Protokolleintrag pro Versuch, kein Archiv"
//! geprüft, ohne von einem laufenden SMTP-Server abzuhängen.

use std::path::PathBuf;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use klein_buch_lib::commands::packages::{send_catalog_core, SendPackageCatalogArgs};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, email_log, mail_accounts, packages, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::package::PackageRevisionInput;

struct Env {
    pool: SqlitePool,
    paths: Paths,
    _tmp: tempfile::TempDir,
}

async fn setup_env() -> Env {
    // Plattform-unabhängiger Keychain-Mock (kein Secret-Service-Daemon nötig).
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let archive_dir = data_dir.join("archive");
    let backups_dir = data_dir.join("backups");
    let inputs_dir = tmp.path().join("inputs");
    let db_file = data_dir.join("test.sqlite");
    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::create_dir_all(&backups_dir).unwrap();
    std::fs::create_dir_all(&inputs_dir).unwrap();

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

async fn create_contact(pool: &SqlitePool, email: Option<&str>) -> String {
    contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: None,
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: email.map(str::to_string),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn create_package(pool: &SqlitePool) -> String {
    packages::create(
        pool,
        None,
        "Hochzeit klein",
        &PackageRevisionInput {
            title: "Hochzeit klein".into(),
            body_markup: "Kompaktes Paket:\n\n- Vorbesprechung\n- **Shooting** (2h)\n- Bildauswahl"
                .into(),
            default_unit_price_cents: 90_000,
            unit_code: "C62".into(),
            tax_category_code: "E".into(),
            note: None,
        },
    )
    .await
    .unwrap()
    .id
}

/// Mail-Account, der auf einen geschlossenen Port zeigt → Versand schlägt
/// deterministisch fehl (Connection refused), ohne Auth.
async fn create_unreachable_account(pool: &SqlitePool) -> String {
    mail_accounts::create(
        pool,
        &mail_accounts::MailAccountInput {
            label: "Unerreichbar (Test)".into(),
            auth_type: "smtp_password".into(),
            smtp_host: Some("127.0.0.1".into()),
            smtp_port: Some(59999),
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

/// Kern-Garantie: ein Versandversuch → GENAU EIN append-only `email_log`-Eintrag
/// (`related_kind = 'package_catalog'`), kein write-once-Archiv-Eintrag.
#[tokio::test]
async fn catalog_send_writes_exactly_one_email_log_and_no_archive() {
    let env = setup_env().await;
    let contact_id = create_contact(&env.pool, Some("empfaenger@kunde-test.de")).await;
    let package_id = create_package(&env.pool).await;
    let account_id = create_unreachable_account(&env.pool).await;

    let args = SendPackageCatalogArgs {
        account_id,
        contact_id,
        package_ids: vec![package_id],
        subject: None,
        body: None,
    };

    // Versand schlägt fehl (geschlossener Port) — aber der Versuch MUSS protokolliert sein.
    let res = send_catalog_core(&env.pool, &env.paths, &args).await;
    assert!(
        res.is_err(),
        "Versand an geschlossenen Port muss fehlschlagen"
    );

    let entries = email_log::list(&env.pool, 100).await.unwrap();
    assert_eq!(entries.len(), 1, "genau ein Protokoll-Eintrag pro Versuch");
    let e = &entries[0];
    assert_eq!(e.related_kind, "package_catalog");
    assert_eq!(e.status, "failed");
    assert_eq!(e.to_email, "empfaenger@kunde-test.de");
    assert_eq!(e.attachment_count, 1);
    assert!(e.related_id.is_none(), "Broschüre hat keinen Beleg-Bezug");

    // KEIN write-once-Archiv: die Broschüre ist kein §14-Beleg.
    let archive_count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM archive_entries")
        .fetch_one(&env.pool)
        .await
        .unwrap()
        .get("c");
    assert_eq!(archive_count, 0, "Broschüre darf nichts archivieren");
}

/// Leere Auswahl wird abgelehnt (keine Broschüre ohne Paket) — und es entsteht
/// KEIN Protokoll-Eintrag (Validierung schlägt vor dem Versand zu).
#[tokio::test]
async fn catalog_send_rejects_empty_selection() {
    let env = setup_env().await;
    let contact_id = create_contact(&env.pool, Some("empfaenger@kunde-test.de")).await;
    let account_id = create_unreachable_account(&env.pool).await;

    let args = SendPackageCatalogArgs {
        account_id,
        contact_id,
        package_ids: vec![],
        subject: None,
        body: None,
    };
    let res = send_catalog_core(&env.pool, &env.paths, &args).await;
    assert!(res.is_err(), "leere Auswahl muss abgelehnt werden");

    let entries = email_log::list(&env.pool, 100).await.unwrap();
    assert!(entries.is_empty(), "kein Versand → kein Protokoll-Eintrag");
}

/// Kontakt ohne E-Mail kann nicht Empfänger sein.
#[tokio::test]
async fn catalog_send_requires_contact_email() {
    let env = setup_env().await;
    let contact_id = create_contact(&env.pool, None).await;
    let package_id = create_package(&env.pool).await;
    let account_id = create_unreachable_account(&env.pool).await;

    let args = SendPackageCatalogArgs {
        account_id,
        contact_id,
        package_ids: vec![package_id],
        subject: None,
        body: None,
    };
    let res = send_catalog_core(&env.pool, &env.paths, &args).await;
    assert!(res.is_err(), "ohne Empfänger-E-Mail kein Versand");
}
