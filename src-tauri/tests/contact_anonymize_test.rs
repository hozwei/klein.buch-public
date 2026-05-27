//! Integrationstest für die DSGVO-Anonymisierung nach Art. 17 (Block 19).
//!
//! Prüft den testbaren Kern [`anonymize_core`]:
//! - blockiert, solange offene Entwürfe (unlocked Rechnungen) existieren,
//! - überschreibt die personenbezogenen Stammdaten (name → Platzhalter, Rest NULL),
//!   setzt `anonymized_at` + `archived = 1`,
//! - lässt den eingefrorenen Buyer-Snapshot festgeschriebener Rechnungen UNberührt
//!   (§147 AO / GoBD),
//! - protokolliert genau einmal im append-only `audit_log`,
//! - lehnt eine erneute Anonymisierung ab.

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use klein_buch_lib::commands::contacts::anonymize_core;
use klein_buch_lib::db::repo::contacts;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_file = tmp.path().join("test.sqlite");
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
    (pool, tmp)
}

async fn create_contact(pool: &SqlitePool, name: &str) -> String {
    contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: name.into(),
            legal_form: Some("Einzelunternehmen".into()),
            vat_id: Some("DE123456789".into()),
            tax_number: Some("123/456/78901".into()),
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: Some("erika@example.de".into()),
            phone: Some("0123456".into()),
            iban: Some("DE89370400440532013000".into()),
            bic: Some("COBADEFFXXX".into()),
            accepts_einvoice: true,
            notes: Some("INTERN: Stammkunde".into()),
        },
    )
    .await
    .unwrap()
    .id
}

/// Fügt eine Rechnung direkt ein (ohne die schwere Lock-Pipeline/Sidecar).
/// `locked` steuert `locked_at`; `with_snapshot` setzt den Buyer-Snapshot.
async fn insert_invoice(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    contact_id: &str,
    buyer_name: &str,
    locked: bool,
) {
    let locked_at = if locked {
        Some("2026-03-01 10:00:00")
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO invoices
            (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
             seller_name, seller_street, seller_postal_code, seller_city,
             net_amount_cents, gross_amount_cents, status, locked_at,
             buyer_name, buyer_street, buyer_postal_code, buyer_city,
             buyer_country_code, buyer_vat_id, buyer_email)
         VALUES (?, ?, 2026, 'issued', '2026-03-01', ?,
             'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
             10000, 10000, 'paid', ?,
             ?, 'Hauptstr. 7', '80331', 'München', 'DE', 'DE123456789', 'erika@example.de')",
    )
    .bind(id)
    .bind(number)
    .bind(contact_id)
    .bind(locked_at)
    .bind(buyer_name)
    .execute(pool)
    .await
    .unwrap();
}

async fn audit_count(pool: &SqlitePool) -> i64 {
    sqlx::query("SELECT COUNT(*) AS c FROM audit_log WHERE action = 'contact.anonymize'")
        .fetch_one(pool)
        .await
        .unwrap()
        .get("c")
}

#[tokio::test]
async fn anonymize_blocked_while_open_draft_exists() {
    let (pool, _tmp) = setup_pool().await;
    let cid = create_contact(&pool, "Erika Musterfrau").await;
    // Unlocked = offener Entwurf → Sperre.
    insert_invoice(
        &pool,
        "i-draft",
        "RE-2026-0001",
        &cid,
        "Erika Musterfrau",
        false,
    )
    .await;

    let res = anonymize_core(&pool, &cid).await;
    assert!(
        res.is_err(),
        "offener Entwurf muss die Anonymisierung blocken"
    );
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("nicht möglich"), "Meldung: {msg}");

    // Kontakt unverändert, kein Audit.
    let c = contacts::get(&pool, &cid).await.unwrap().unwrap();
    assert_eq!(c.name, "Erika Musterfrau");
    assert!(c.anonymized_at.is_none());
    assert_eq!(audit_count(&pool).await, 0);
}

#[tokio::test]
async fn anonymize_overwrites_fields_keeps_snapshot_and_audits_once() {
    let (pool, _tmp) = setup_pool().await;
    let cid = create_contact(&pool, "Erika Musterfrau").await;
    // Festgeschriebene Rechnung mit eingefrorenem Empfänger-Snapshot.
    insert_invoice(
        &pool,
        "i-locked",
        "RE-2026-0001",
        &cid,
        "Erika Musterfrau",
        true,
    )
    .await;

    let row = anonymize_core(&pool, &cid)
        .await
        .expect("Anonymisierung sollte gelingen");

    // Stammdaten überschrieben.
    assert!(row.name.starts_with("Anonymisiert #"), "Name: {}", row.name);
    assert!(row.street.is_none());
    assert!(row.email.is_none());
    assert!(row.vat_id.is_none());
    assert!(row.tax_number.is_none());
    assert!(row.phone.is_none());
    assert!(row.iban.is_none());
    assert!(row.bic.is_none());
    assert!(row.notes.is_none());
    assert!(row.anonymized_at.is_some());
    assert_eq!(row.archived, 1);
    // country_code bleibt (NOT NULL).
    assert_eq!(row.country_code, "DE");

    // Buyer-Snapshot der festgeschriebenen Rechnung bleibt der ORIGINAL-Stand.
    let snap_name: String =
        sqlx::query("SELECT buyer_name AS n FROM invoices WHERE id = 'i-locked'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("n");
    assert_eq!(
        snap_name, "Erika Musterfrau",
        "Snapshot darf nicht anonymisiert werden"
    );

    // Genau ein Audit-Eintrag.
    assert_eq!(audit_count(&pool).await, 1);

    // Zweite Anonymisierung wird abgelehnt — kein weiterer Audit-Eintrag.
    let again = anonymize_core(&pool, &cid).await;
    assert!(again.is_err(), "doppelte Anonymisierung muss scheitern");
    assert_eq!(audit_count(&pool).await, 1, "kein zweiter Audit-Eintrag");
}
