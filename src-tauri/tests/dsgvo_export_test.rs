//! Integrationstest für die DSGVO-Auskunft nach Art. 15 (Block 18).
//!
//! Prüft den read-only Export-Kern [`export_core`]:
//! - sammelt ausschließlich Daten **dieses** Kontakts (Fremd-Kontakt fehlt),
//! - schreibt ein ZIP mit `auskunft.pdf` + `auskunft.json` + `dokumente/`,
//! - legt die archivierte Original-Datei bei (Hash-re-verifiziert),
//! - lässt interne Notizen weg,
//! - protokolliert die Erstellung genau einmal im append-only `audit_log`.

use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::commands::dsgvo::export_core;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};

struct Env {
    pool: SqlitePool,
    paths: Paths,
    _tmp: tempfile::TempDir,
}

async fn setup_env() -> Env {
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

async fn create_contact(
    pool: &SqlitePool,
    name: &str,
    email: Option<&str>,
    notes: Option<&str>,
) -> String {
    contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: name.into(),
            legal_form: None,
            vat_id: Some("DE123456789".into()),
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
            notes: notes.map(str::to_string),
        },
    )
    .await
    .unwrap()
    .id
}

#[allow(clippy::too_many_arguments)]
async fn insert_invoice(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    contact_id: &str,
    pdf_archive_id: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO invoices
            (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
             seller_name, seller_street, seller_postal_code, seller_city,
             net_amount_cents, gross_amount_cents, status, pdf_archive_id)
         VALUES (?, ?, 2026, 'issued', '2026-03-01', ?,
             'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
             10000, 10000, 'paid', ?)",
    )
    .bind(id)
    .bind(number)
    .bind(contact_id)
    .bind(pdf_archive_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO invoice_items
            (id, invoice_id, position, description, quantity, unit_code,
             unit_price_cents, net_amount_cents, tax_rate_percent, tax_category_code)
         VALUES (?, ?, 1, 'Vor-Ort-Service', 2.0, 'HUR', 5000, 10000, 0.0, 'E')",
    )
    .bind(format!("{id}-it1"))
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn export_creates_zip_with_pdf_json_and_bundles_originals_only_for_this_contact() {
    let env = setup_env().await;

    let target = create_contact(
        &env.pool,
        "Ziel Kunde",
        Some("ziel@example.de"),
        Some("INTERN: zahlt spät"),
    )
    .await;
    let other = create_contact(&env.pool, "Anderer Kunde", Some("anderer@example.de"), None).await;

    // Archivierte Rechnung-PDF (write-once) für den Ziel-Kontakt.
    let stored = store_bytes(
        &env.pool,
        &env.paths.archive_dir,
        2026,
        ArchiveKind::InvoicePdf,
        "RE-2026-0001.pdf",
        "application/pdf",
        b"%PDF-1.7 fake invoice",
    )
    .await
    .unwrap();

    insert_invoice(
        &env.pool,
        "i-target",
        "RE-2026-0001",
        &target,
        Some(stored.archive_id.as_str()),
    )
    .await;
    insert_invoice(&env.pool, "i-other", "RE-2026-9999", &other, None).await;

    // Versandprotokoll-Eintrag mit Bezug zur Ziel-Rechnung.
    sqlx::query(
        "INSERT INTO email_log
            (id, channel, related_kind, related_id, related_number, from_email, to_email, subject, status, attachment_count)
         VALUES ('m-1', 'graph', 'invoice', 'i-target', 'RE-2026-0001',
             'info@wildbach.de', 'ziel@example.de', 'Ihre Rechnung RE-2026-0001', 'success', 1)",
    )
    .execute(&env.pool)
    .await
    .unwrap();

    // Audit-Bezug auf die Ziel-Rechnung.
    sqlx::query(
        "INSERT INTO audit_log (action, entity_type, entity_id) VALUES ('invoice.lock', 'invoice', 'i-target')",
    )
    .execute(&env.pool)
    .await
    .unwrap();

    let res = export_core(
        &env.pool,
        &env.paths,
        &target,
        "2026-05-24 12:00",
        "2026-05-24",
    )
    .await
    .unwrap();

    assert_eq!(res.invoice_count, 1, "nur die Ziel-Rechnung");
    assert_eq!(res.document_count, 1);
    assert_eq!(res.bundled_document_count, 1, "Original-PDF muss beiliegen");
    assert_eq!(res.email_count, 1);
    assert_eq!(res.file_name, "auskunft-ziel-kunde-2026-05-24.zip");

    // ZIP öffnen + Inhalte prüfen.
    let file = std::fs::File::open(&res.zip_path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == "auskunft.pdf"),
        "PDF fehlt: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "auskunft.json"),
        "JSON fehlt: {names:?}"
    );
    assert!(names.iter().any(|n| n == "LIESMICH.txt"));
    assert!(
        names.iter().any(|n| n.starts_with("dokumente/")),
        "Original-Datei fehlt im ZIP: {names:?}"
    );

    // PDF nicht leer.
    {
        let pdf = zip.by_name("auskunft.pdf").unwrap();
        assert!(pdf.size() > 0, "PDF ist leer");
    }

    // JSON inhaltlich prüfen.
    let mut json = String::new();
    zip.by_name("auskunft.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    assert!(json.contains("RE-2026-0001"), "Ziel-Rechnung fehlt");
    assert!(json.contains("Ziel Kunde"), "Subjekt-Name fehlt");
    assert!(
        !json.contains("RE-2026-9999"),
        "Fremd-Rechnung darf NICHT in der Auskunft erscheinen"
    );
    assert!(
        !json.contains("Anderer Kunde"),
        "Fremd-Kontakt darf NICHT erscheinen"
    );
    assert!(
        !json.contains("INTERN"),
        "interne Notizen dürfen nicht in der Auskunft stehen"
    );

    // Genau ein 'dsgvo.export'-Audit-Eintrag.
    let cnt: i64 = sqlx::query("SELECT COUNT(*) AS c FROM audit_log WHERE action = 'dsgvo.export'")
        .fetch_one(&env.pool)
        .await
        .unwrap()
        .get("c");
    assert_eq!(cnt, 1, "Erstellung muss genau einmal protokolliert sein");

    // R2-029: `details_json` des Audit-Eintrags darf KEIN PII enthalten —
    // weder den slugifizierten Kontakt-Namen ("ziel-kunde") noch das
    // Dateinamen-Prefix ("auskunft-"). Der Eintrag identifiziert den Export
    // ausschließlich über `entity_id` (Kontakt-UUID) + `timestamp_utc`.
    let details: Option<String> =
        sqlx::query("SELECT details_json FROM audit_log WHERE action = 'dsgvo.export'")
            .fetch_one(&env.pool)
            .await
            .unwrap()
            .get("details_json");
    let details = details.unwrap_or_default();
    assert!(
        !details.contains("auskunft-"),
        "Audit-Detail darf den auskunft-<slug>-Dateinamen nicht enthalten (PII-Leak): {details}"
    );
    assert!(
        !details.to_lowercase().contains("ziel-kunde"),
        "Audit-Detail darf den slugifizierten Kontakt-Namen nicht enthalten: {details}"
    );
}

#[tokio::test]
async fn export_unknown_contact_errors() {
    let env = setup_env().await;
    let res = export_core(
        &env.pool,
        &env.paths,
        "no-such-id",
        "2026-05-24 12:00",
        "2026-05-24",
    )
    .await;
    assert!(res.is_err(), "unbekannter Kontakt muss Fehler liefern");
}
