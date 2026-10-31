//! Integrationstests für Migration `0035_service_period_buyer_reference.sql`
//! (ADR 0042 Nr. 10–12, Lauf 2026.11.1, Block V3).
//!
//! Gegen eine Tempfile-SQLite mit dem echten `MIGRATOR`:
//!
//!   * Schema 35; Spalten `service_period_start`/`_end`/`buyer_reference` an
//!     `invoices`, `buyer_reference` an `recurring_invoices`.
//!   * CHECKs: Datum JJJJ-MM-TT, beide Grenzen oder keine, Beginn ≤ Ende, nie
//!     zugleich mit einem Leistungsdatum, Leitweg-ID 1–200 Zeichen.
//!   * Nach dem Festschreiben sind Zeitraum und Leitweg-ID unveränderlich
//!     (`trg_invoices_immutable`), an einem verworfenen Entwurf ebenso
//!     (`trg_invoices_voided_final`); alle bisherigen Bedingungen bleiben.
//!   * Eine Datenbank mit Schema 34 wird vorwärts migriert, der Bestand bleibt.
//!
//! Testdaten nur mit Dokumentations-Adressen (example.org).

use std::borrow::Cow;
use std::str::FromStr;

use klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION;
use klein_buch_lib::db::MIGRATOR;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Ein Testfall: Leistungsdatum, Zeitraum (Beginn, Ende), Leitweg-ID.
type Case<'a> = (Option<&'a str>, Option<(&'a str, &'a str)>, Option<&'a str>);

const CONTACT: &str = "01928300-0000-7000-8000-000000000001";

async fn open(dir: &std::path::Path) -> SqlitePool {
    let url = format!(
        "sqlite://{}",
        dir.join("klein-buch.sqlite").to_string_lossy()
    );
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap()
}

/// Migrator ohne 0035 (DB auf Schema 34).
fn migrator_until_34() -> sqlx::migrate::Migrator {
    let all = &MIGRATOR.migrations;
    let idx = all
        .iter()
        .position(|m| m.description.contains("service period buyer reference"))
        .expect("Migration 0035");
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(all[..idx].to_vec()),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

async fn schema_version(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn mk_contact(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO contacts (id, contact_type, name, email, country_code)
         VALUES (?, 'customer', 'Kunde Beispiel', 'kunde@example.org', 'DE')",
    )
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
}

async fn setup() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    MIGRATOR.run(&pool).await.unwrap();
    mk_contact(&pool).await;
    (pool, dir)
}

/// Rechnung (Entwurf) mit Zeitraum und Leitweg-ID; `locked` schreibt sie fest.
async fn mk_invoice(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    delivery: Option<&str>,
    period: Option<(&str, &str)>,
    buyer_reference: Option<&str>,
    locked: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, delivery_date,
            service_period_start, service_period_end, buyer_reference, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, locked_at,
            buyer_name, buyer_country_code
         ) VALUES (?, ?, 2026, 'issued', '2026-05-20', ?, ?, ?, ?, ?,
            'Beispiel IT', 'Beispielweg 1', '84028', 'Landshut',
            10000, 10000, ?, ?, 'Kunde Beispiel', 'DE')",
    )
    .bind(id)
    .bind(number)
    .bind(delivery)
    .bind(period.map(|p| p.0))
    .bind(period.map(|p| p.1))
    .bind(buyer_reference)
    .bind(CONTACT)
    .bind(if locked { "issued" } else { "draft" })
    .bind(locked.then_some("2026-05-20 10:00:00"))
    .execute(pool)
    .await
    .map(|_| ())
}

fn assert_blocked<T: std::fmt::Debug>(res: sqlx::Result<T>, needle: &str) {
    let err = res.expect_err("muss abbrechen");
    let msg = err.to_string();
    assert!(msg.contains(needle), "erwartet „{needle}“, war: {msg}");
}

#[tokio::test]
async fn schema_35_with_new_columns() {
    let (pool, _dir) = setup().await;
    const { assert!(EXPECTED_SCHEMA_VERSION >= 35) };
    assert_eq!(
        schema_version(&pool).await,
        EXPECTED_SCHEMA_VERSION.to_string()
    );
    let inv: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('invoices')
          WHERE name IN ('service_period_start','service_period_end','buyer_reference')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inv, 3);
    let tmpl: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('recurring_invoices') WHERE name = 'buyer_reference'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tmpl, 1);
}

#[tokio::test]
async fn checks_guard_period_and_buyer_reference() {
    let (pool, _dir) = setup().await;
    // zulässig: Zeitraum ohne Datum, ein Tag, Datum ohne Zeitraum, nichts
    let ok = [
        (
            None,
            Some(("2026-04-01", "2026-06-30")),
            Some("04011000-12345-67"),
        ),
        (None, Some(("2026-05-01", "2026-05-01")), None),
        (Some("2026-09-30"), None, None),
        (None, None, None),
    ];
    for (i, (d, p, r)) in ok.into_iter().enumerate() {
        mk_invoice(
            &pool,
            &format!("ok-{i}"),
            &format!("RE-2026-01{i:02}"),
            d,
            p,
            r,
            false,
        )
        .await
        .unwrap_or_else(|e| panic!("Fall {i}: {e}"));
    }
    // abgelehnt
    let bad: [Case; 5] = [
        (None, Some(("2026-06-30", "2026-04-01")), None), // Ende vor Beginn
        (Some("2026-05-19"), Some(("2026-05-01", "2026-05-31")), None), // beides
        (None, Some(("2026-5-1", "2026-05-31")), None),   // kein JJJJ-MM-TT
        (None, None, Some("")),                           // leere Leitweg-ID
        (None, None, Some(&"x".repeat(201))),             // zu lang
    ];
    for (i, (d, p, r)) in bad.into_iter().enumerate() {
        assert_blocked(
            mk_invoice(
                &pool,
                &format!("bad-{i}"),
                &format!("RE-2026-02{i:02}"),
                d,
                p,
                r,
                false,
            )
            .await,
            "CHECK constraint failed",
        );
    }
    // nur eine Grenze
    for sql in [
        "UPDATE invoices SET service_period_end = NULL WHERE id = 'ok-0'",
        "UPDATE invoices SET service_period_start = '2026-04-01' WHERE id = 'ok-3'",
    ] {
        assert_blocked(
            sqlx::query(sql).execute(&pool).await,
            "CHECK constraint failed",
        );
    }
    // Abo-Vorlage: Leitweg-ID 1–200 Zeichen
    assert_blocked(
        sqlx::query(
            "INSERT INTO recurring_invoices (id, label, contact_id, frequency, day_of_period,
                                             next_due_date, buyer_reference)
             VALUES ('t1', 'Wartung', ?, 'monthly', 1, '2026-06-01', '')",
        )
        .bind(CONTACT)
        .execute(&pool)
        .await,
        "CHECK constraint failed",
    );
}

#[tokio::test]
async fn period_and_buyer_reference_are_immutable_after_lock() {
    let (pool, _dir) = setup().await;
    mk_invoice(
        &pool,
        "inv-l",
        "RE-2026-0300",
        None,
        Some(("2026-04-01", "2026-06-30")),
        Some("04011000-12345-67"),
        true,
    )
    .await
    .unwrap();
    for sql in [
        "UPDATE invoices SET service_period_start = '2026-03-01' WHERE id = 'inv-l'",
        "UPDATE invoices SET service_period_end = '2026-07-31' WHERE id = 'inv-l'",
        "UPDATE invoices SET service_period_start = NULL, service_period_end = NULL,
                             delivery_date = '2026-05-01' WHERE id = 'inv-l'",
        "UPDATE invoices SET buyer_reference = '991-00000-00' WHERE id = 'inv-l'",
        "UPDATE invoices SET buyer_reference = NULL WHERE id = 'inv-l'",
        // bisherige Bedingungen gelten weiter (nur verschärft)
        "UPDATE invoices SET invoice_number = 'RE-2026-0399' WHERE id = 'inv-l'",
        "UPDATE invoices SET seller_name = 'Andere' WHERE id = 'inv-l'",
        "UPDATE invoices SET locked_at = NULL WHERE id = 'inv-l'",
    ] {
        assert_blocked(sqlx::query(sql).execute(&pool).await, "invoice is locked");
    }
    // Ein Entwurf bleibt änderbar; das Festschreiben setzt die Leitweg-ID im
    // selben UPDATE wie locked_at.
    mk_invoice(
        &pool,
        "inv-d",
        "RE-2026-0301",
        Some("2026-05-19"),
        None,
        None,
        false,
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET delivery_date = NULL, service_period_start = '2026-05-01',
                             service_period_end = '2026-05-31', buyer_reference = 'X-1'
          WHERE id = 'inv-d'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET buyer_reference = 'X-2', locked_at = '2026-05-20 11:00:00',
                             status = 'issued' WHERE id = 'inv-d'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_blocked(
        sqlx::query("UPDATE invoices SET buyer_reference = 'X-3' WHERE id = 'inv-d'")
            .execute(&pool)
            .await,
        "invoice is locked",
    );
}

#[tokio::test]
async fn voided_draft_keeps_period_and_buyer_reference() {
    let (pool, _dir) = setup().await;
    mk_invoice(
        &pool,
        "inv-v",
        "RE-2026-0400",
        None,
        Some(("2026-04-01", "2026-04-30")),
        Some("04011000-12345-67"),
        false,
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET voided_at = '2026-05-21 08:00:00', void_reason = 'doppelt'
          WHERE id = 'inv-v'",
    )
    .execute(&pool)
    .await
    .unwrap();
    for sql in [
        "UPDATE invoices SET service_period_end = '2026-05-31' WHERE id = 'inv-v'",
        "UPDATE invoices SET buyer_reference = NULL WHERE id = 'inv-v'",
    ] {
        assert_blocked(sqlx::query(sql).execute(&pool).await, "voided");
    }
}

#[tokio::test]
async fn schema_34_is_migrated_forward_and_keeps_the_stock() {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    migrator_until_34().run(&pool).await.unwrap();
    assert_eq!(schema_version(&pool).await, "34");
    mk_contact(&pool).await;
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, delivery_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, locked_at,
            buyer_name, buyer_country_code
         ) VALUES ('alt', 'RE-2026-0500', 2026, 'issued', '2026-02-20', '2026-02-19', ?,
            'Beispiel IT', 'Beispielweg 1', '84028', 'Landshut',
            10000, 10000, 'issued', '2026-02-20 10:00:00', 'Kunde Beispiel', 'DE')",
    )
    .bind(CONTACT)
    .execute(&pool)
    .await
    .unwrap();

    MIGRATOR.run(&pool).await.unwrap();
    assert_eq!(schema_version(&pool).await, "35");
    let row: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT delivery_date, service_period_start, service_period_end, buyer_reference
           FROM invoices WHERE id = 'alt'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (Some("2026-02-19".into()), None, None, None));
    assert_blocked(
        sqlx::query("UPDATE invoices SET buyer_reference = 'X' WHERE id = 'alt'")
            .execute(&pool)
            .await,
        "invoice is locked",
    );
}
