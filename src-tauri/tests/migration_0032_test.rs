//! Integrationstests für Migration `0032_gobd_hardening.sql` (Lauf F, Block
//! F2a) und die GoBD-Trigger ohne eigene Tests (W13-TRIGGER-TESTS).
//!
//! Gegen eine Tempfile-SQLite mit dem echten `MIGRATOR`:
//!
//!   * `audit_log` und `package_revisions`: UPDATE/DELETE scheitern am Trigger,
//!     INSERT geht (W13-TRIGGER-TESTS).
//!   * Positionen festgeschriebener/verworfener Rechnungen und
//!     festgeschriebener/zurückgezogener Angebote sind unveränderlich (KB-0002).
//!   * `archive_entries` ohne DELETE (KB-0004).
//!   * höchstens ein festgeschriebener Storno je Original; die Migration bricht
//!     bei Bestandsduplikaten mit klarer Meldung ab (LE-STORNO-UNIQUE).
//!   * Storno endgültig, Storno-Paar idempotent (R1-S2-MARKCANCELED).
//!   * höchstens ein Standard-Zahlungskonto, Bestand mit Audit bereinigt
//!     (R1-013).
//!   * Abgangsfelder einer veräußerten Anlage endgültig (R1-S2-ASSET-DISPOSAL).
//!   * Verwerfen nummerierter Entwürfe ohne Löschen (W14-032).
//!   * Integritäts-Historie mit „fehlt“/„nicht lesbar“ (W2-INTEGRITY-UNREADABLE).
//!   * Fremdschlüssel-Prüfung nach den Migrationen (R1-012).
//!   * Seed der Regel `rule_recurring_due` (Lauf F1).
//!
//! Testdaten nur mit Dokumentations-Adressen (example.org).

use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, invoices, payment_accounts};
use klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::error::Error;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

const CONTACT: &str = "01928300-0000-7000-8000-00000000c001";

async fn open(dir: &std::path::Path) -> SqlitePool {
    let db_path = dir.join("klein-buch.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
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

async fn setup() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = open(dir.path()).await;
    MIGRATOR.run(&pool).await.unwrap();
    mk_contact(&pool, CONTACT, "Kunde Beispiel").await;
    (pool, dir)
}

/// Migrator mit allen Migrationen außer 0032 (DB auf Schema 31).
fn migrator_without_last() -> sqlx::migrate::Migrator {
    let all = &MIGRATOR.migrations;
    assert!(all.last().unwrap().description.contains("gobd hardening"));
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(all[..all.len() - 1].to_vec()),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

async fn setup_schema_31() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = open(dir.path()).await;
    migrator_without_last().run(&pool).await.unwrap();
    assert_eq!(schema_version(&pool).await, "31");
    mk_contact(&pool, CONTACT, "Kunde Beispiel").await;
    (pool, dir)
}

async fn schema_version(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn mk_contact(pool: &SqlitePool, id: &str, name: &str) {
    sqlx::query(
        "INSERT INTO contacts (id, contact_type, name, email, country_code)
         VALUES (?, 'customer', ?, 'kunde@example.org', 'DE')",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// Rechnung direkt einfügen (Entwurf oder festgeschrieben), mit einer Position.
async fn mk_invoice(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    locked: bool,
    storno_for: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, locked_at, is_storno_for,
            buyer_name, buyer_country_code
         ) VALUES (?, ?, 2026, 'issued', '2026-05-20', ?,
            'Verkäufer', 'Beispielweg 1', '84028', 'Landshut',
            10000, 10000, ?, ?, ?, 'Kunde Beispiel', 'DE')",
    )
    .bind(id)
    .bind(number)
    .bind(CONTACT)
    .bind(if locked { "issued" } else { "draft" })
    .bind(locked.then_some("2026-05-20 10:00:00"))
    .bind(storno_for)
    .execute(pool)
    .await
    .unwrap();
}

async fn mk_item(pool: &SqlitePool, id: &str, invoice_id: &str, position: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO invoice_items (id, invoice_id, position, description, quantity,
                                    unit_price_cents, net_amount_cents)
         VALUES (?, ?, ?, 'Leistung', 1.0, 10000, 10000)",
    )
    .bind(id)
    .bind(invoice_id)
    .bind(position)
    .execute(pool)
    .await
    .map(|_| ())
}

async fn mk_quote(pool: &SqlitePool, id: &str, number: &str) {
    sqlx::query(
        "INSERT INTO quotes (
            id, quote_number, fiscal_year, quote_date, valid_until, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents
         ) VALUES (?, ?, 2026, '2026-05-20', '2026-06-20', ?,
            'Verkäufer', 'Beispielweg 1', '84028', 'Landshut', 10000, 10000)",
    )
    .bind(id)
    .bind(number)
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO quote_items (id, quote_id, position, description, quantity,
                                  unit_price_cents, net_amount_cents)
         VALUES (?, ?, 1, 'Leistung', 1.0, 10000, 10000)",
    )
    .bind(format!("{id}-1"))
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

/// Erwartet einen Abbruch durch einen Trigger mit `needle` im Text.
fn assert_blocked<T: std::fmt::Debug>(res: sqlx::Result<T>, needle: &str) {
    let err = res.expect_err("Trigger muss abbrechen");
    let msg = err.to_string();
    assert!(msg.contains(needle), "erwartet „{needle}“, war: {msg}");
}

fn ctx_with(pool: &SqlitePool, dir: &std::path::Path) -> Arc<AppCtx> {
    let ctx = AppCtx::for_tests(Paths::under(dir));
    ctx.set_pool(pool.clone());
    ctx
}

const REASON: &str = "Doppelt angelegt, Kunde hat storniert";

// ============================================================================
// Schema, Seed
// ============================================================================

#[tokio::test]
async fn schema_32_and_recurring_due_rule_seeded() {
    let (pool, _dir) = setup().await;
    assert_eq!(EXPECTED_SCHEMA_VERSION, 32);
    assert_eq!(schema_version(&pool).await, "32");
    let (rule_type, enabled, in_app): (String, i64, i64) = sqlx::query_as(
        "SELECT rule_type, enabled, deliver_in_app FROM notification_rules
          WHERE id = 'rule_recurring_due'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (rule_type.as_str(), enabled, in_app),
        ("recurring_due", 1, 1)
    );
    // alle neuen Tabellen bleiben STRICT (keine Tabelle umgebaut)
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
               AND name IN ('invoices', 'archive_integrity_checks', 'payment_accounts')
               AND sql LIKE '%STRICT'"
        )
        .await,
        3
    );
}

// ============================================================================
// W13-TRIGGER-TESTS: audit_log, package_revisions
// ============================================================================

#[tokio::test]
async fn audit_log_is_append_only() {
    let (pool, _dir) = setup().await;
    sqlx::query("INSERT INTO audit_log (action, entity_type, entity_id) VALUES ('t.a', 't', '1')")
        .execute(&pool)
        .await
        .expect("INSERT geht");
    assert_blocked(
        sqlx::query("UPDATE audit_log SET action = 'manipuliert' WHERE action = 't.a'")
            .execute(&pool)
            .await,
        "audit_log is append-only",
    );
    assert_blocked(
        sqlx::query("DELETE FROM audit_log WHERE action = 't.a'")
            .execute(&pool)
            .await,
        "audit_log is append-only",
    );
    let err = sqlx::query("DELETE FROM audit_log")
        .execute(&pool)
        .await
        .unwrap_err();
    // RAISE(ABORT) im Trigger = SQLITE_CONSTRAINT_TRIGGER (1811)
    let code = err
        .as_database_error()
        .and_then(|d| d.code())
        .map(|c| c.into_owned());
    assert_eq!(code.as_deref(), Some("1811"), "{err}");
}

#[tokio::test]
async fn package_revisions_are_immutable_and_append_only() {
    let (pool, _dir) = setup().await;
    sqlx::query("INSERT INTO packages (id, name, current_revision) VALUES ('p1', 'Paket', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO package_revisions (id, package_id, revision, title, body_markup,
                                        default_unit_price_cents)
         VALUES ('r1', 'p1', 1, 'Titel', 'Text', 10000),
                ('r2', 'p1', 2, 'Titel 2', 'Text', 12000)",
    )
    .execute(&pool)
    .await
    .expect("INSERT (neue Revision) geht");
    assert_blocked(
        sqlx::query("UPDATE package_revisions SET default_unit_price_cents = 1 WHERE id = 'r1'")
            .execute(&pool)
            .await,
        "package_revisions are immutable",
    );
    assert_blocked(
        sqlx::query("DELETE FROM package_revisions WHERE id = 'r2'")
            .execute(&pool)
            .await,
        "package_revisions are append-only",
    );
}

// ============================================================================
// KB-0002: Positionen
// ============================================================================

#[tokio::test]
async fn items_of_locked_invoice_are_immutable() {
    let (pool, _dir) = setup().await;
    mk_invoice(&pool, "draft", "RE-2026-0001", false, None).await;
    mk_invoice(&pool, "locked", "RE-2026-0002", true, None).await;
    mk_item(&pool, "d1", "draft", 1).await.unwrap();

    // Entwurf: frei änderbar
    sqlx::query("UPDATE invoice_items SET description = 'neu' WHERE id = 'd1'")
        .execute(&pool)
        .await
        .unwrap();
    mk_item(&pool, "d2", "draft", 2).await.unwrap();
    sqlx::query("DELETE FROM invoice_items WHERE id = 'd2'")
        .execute(&pool)
        .await
        .unwrap();

    // Festgeschrieben: kein INSERT, UPDATE, DELETE, kein Verschieben hinein
    assert_blocked(
        mk_item(&pool, "l1", "locked", 1).await,
        "items are immutable",
    );
    // Position eines festgeschriebenen Belegs (aus dem Entwurf heraus
    // festgeschrieben): Entwurf festschreiben, dann ändern
    sqlx::query("UPDATE invoices SET locked_at = '2026-05-21 10:00:00', status = 'issued' WHERE id = 'draft'")
        .execute(&pool)
        .await
        .unwrap();
    assert_blocked(
        sqlx::query("UPDATE invoice_items SET unit_price_cents = 1 WHERE id = 'd1'")
            .execute(&pool)
            .await,
        "items are immutable",
    );
    assert_blocked(
        sqlx::query("DELETE FROM invoice_items WHERE id = 'd1'")
            .execute(&pool)
            .await,
        "items are immutable",
    );
    mk_invoice(&pool, "draft2", "RE-2026-0003", false, None).await;
    mk_item(&pool, "x1", "draft2", 1).await.unwrap();
    assert_blocked(
        sqlx::query("UPDATE invoice_items SET invoice_id = 'locked', position = 9 WHERE id = 'x1'")
            .execute(&pool)
            .await,
        "items are immutable",
    );
}

#[tokio::test]
async fn items_of_locked_or_canceled_quote_are_immutable() {
    let (pool, _dir) = setup().await;
    mk_quote(&pool, "q-draft", "AN-2026-0001").await;
    mk_quote(&pool, "q-locked", "AN-2026-0002").await;
    mk_quote(&pool, "q-canceled", "AN-2026-0003").await;
    sqlx::query("UPDATE quote_items SET description = 'neu' WHERE id = 'q-draft-1'")
        .execute(&pool)
        .await
        .expect("Entwurf änderbar");
    sqlx::query(
        "UPDATE quotes SET locked_at = '2026-05-21 10:00:00', sent_at = '2026-05-21 10:00:00',
                status = 'sent' WHERE id = 'q-locked'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE quotes SET status = 'canceled', canceled_at = '2026-05-21 10:00:00',
                canceled_reason = 'zurückgezogen' WHERE id = 'q-canceled'",
    )
    .execute(&pool)
    .await
    .unwrap();
    for q in ["q-locked", "q-canceled"] {
        assert_blocked(
            sqlx::query("UPDATE quote_items SET quantity = 2.0 WHERE id = ?")
                .bind(format!("{q}-1"))
                .execute(&pool)
                .await,
            "items are immutable",
        );
        assert_blocked(
            sqlx::query("DELETE FROM quote_items WHERE id = ?")
                .bind(format!("{q}-1"))
                .execute(&pool)
                .await,
            "items are immutable",
        );
        assert_blocked(
            sqlx::query(
                "INSERT INTO quote_items (id, quote_id, position, description, quantity,
                                          unit_price_cents, net_amount_cents)
                 VALUES (?, ?, 2, 'Nachtrag', 1.0, 100, 100)",
            )
            .bind(format!("{q}-2"))
            .bind(q)
            .execute(&pool)
            .await,
            "items are immutable",
        );
    }
}

// ============================================================================
// KB-0004: Archiv ohne DELETE
// ============================================================================

#[tokio::test]
async fn archive_entries_cannot_be_deleted() {
    let (pool, _dir) = setup().await;
    sqlx::query(
        "INSERT INTO archive_entries (id, file_path, file_name, file_hash_sha256,
                                      file_size_bytes, mime_type, source)
         VALUES ('a1', '/data/archive/2026/x.pdf', 'x.pdf', 'abcd', 1, 'application/pdf', 'test')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_blocked(
        sqlx::query("DELETE FROM archive_entries WHERE id = 'a1'")
            .execute(&pool)
            .await,
        "archive_entries are write-once",
    );
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM archive_entries").await,
        1
    );
}

// ============================================================================
// LE-STORNO-UNIQUE + R1-S2-MARKCANCELED
// ============================================================================

#[tokio::test]
async fn at_most_one_locked_storno_per_original() {
    let (pool, _dir) = setup().await;
    mk_invoice(&pool, "orig", "RE-2026-0001", true, None).await;
    // zwei offene Storno-Entwürfe sind (DB-seitig) möglich …
    mk_invoice(&pool, "st1", "ST-2026-0001", false, Some("orig")).await;
    mk_invoice(&pool, "st2", "ST-2026-0002", false, Some("orig")).await;
    sqlx::query(
        "UPDATE invoices SET locked_at = '2026-05-21 10:00:00', status = 'issued' WHERE id = 'st1'",
    )
    .execute(&pool)
    .await
    .unwrap();
    // … festgeschrieben wird höchstens einer
    assert_blocked(
        sqlx::query(
            "UPDATE invoices SET locked_at = '2026-05-21 11:00:00', status = 'issued' WHERE id = 'st2'",
        )
        .execute(&pool)
        .await,
        "UNIQUE constraint failed: invoices.is_storno_for",
    );
    // auch nicht als neu eingefügter festgeschriebener Beleg
    let err = sqlx::query(
        "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city, net_amount_cents,
            gross_amount_cents, status, locked_at, is_storno_for)
         VALUES ('st3', 'ST-2026-0003', 2026, 'issued', '2026-05-20', ?, 'V', 'W', '1', 'O',
            -10000, -10000, 'issued', '2026-05-21 12:00:00', 'orig')",
    )
    .bind(CONTACT)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(err.to_string().contains("UNIQUE"), "{err}");
}

#[tokio::test]
async fn storno_pair_is_final_and_idempotent() {
    let (pool, _dir) = setup().await;
    mk_invoice(&pool, "orig", "RE-2026-0001", true, None).await;
    mk_invoice(&pool, "st1", "ST-2026-0001", false, Some("orig")).await;
    mk_invoice(&pool, "st2", "ST-2026-0002", false, Some("orig")).await;
    let update = invoices::LockUpdate {
        validation_status: "passed",
        validation_report: None,
        pdf_archive_id: "",
        xml_archive_id: "",
        is_kleinunternehmer: None,
        totals: None,
    };
    // Archiv-IDs sind FKs — echte Einträge anlegen
    for (id, p) in [("ap", "/a/p.pdf"), ("ax", "/a/x.xml")] {
        sqlx::query(
            "INSERT INTO archive_entries (id, file_path, file_name, file_hash_sha256,
                                          file_size_bytes, mime_type, source)
             VALUES (?, ?, ?, 'abcd', 1, 'application/pdf', 'test')",
        )
        .bind(id)
        .bind(p)
        .bind(p)
        .execute(&pool)
        .await
        .unwrap();
    }
    let update = invoices::LockUpdate {
        pdf_archive_id: "ap",
        xml_archive_id: "ax",
        ..update
    };
    let pair = invoices::PairCancel {
        original_id: "orig",
        reason: Some("Rückabwicklung"),
    };
    invoices::lock_with_pair_cancel(&pool, "st1", &update, Some(pair))
        .await
        .unwrap();
    let first: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, canceled_at, canceled_by_storno_id FROM invoices WHERE id = 'orig'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first.0, "canceled");
    assert_eq!(first.2.as_deref(), Some("st1"));

    // zweiter Storno-Lauf: abgelehnt (schon der Lock des zweiten Storno-Belegs
    // scheitert am Unique-Index, vor dem Cancel des Originals), nichts
    // geändert (Transaktion zurück)
    let err = invoices::lock_with_pair_cancel(&pool, "st2", &update, Some(pair))
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("UNIQUE constraint failed: invoices.is_storno_for"),
        "{err}"
    );
    let st2_locked: Option<String> =
        sqlx::query_scalar("SELECT locked_at FROM invoices WHERE id = 'st2'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(st2_locked.is_none(), "Storno 2 bleibt Entwurf");
    let again: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, canceled_at, canceled_by_storno_id FROM invoices WHERE id = 'orig'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(again, first);

    // DB-Backstop: Storno-Verweis, Zeitpunkt und Status bleiben
    for sql in [
        "UPDATE invoices SET canceled_by_storno_id = 'st2' WHERE id = 'orig'",
        "UPDATE invoices SET canceled_at = '2027-01-01 00:00:00' WHERE id = 'orig'",
        "UPDATE invoices SET status = 'paid' WHERE id = 'orig'",
        "UPDATE invoices SET cancel_reason = 'anders' WHERE id = 'orig'",
    ] {
        assert_blocked(
            sqlx::query(sql).execute(&pool).await,
            "the storno pair is final",
        );
    }
}

#[tokio::test]
async fn migration_aborts_on_duplicate_locked_stornos_with_clear_message() {
    let (pool, dir) = setup_schema_31().await;
    mk_invoice(&pool, "orig", "RE-2026-0001", true, None).await;
    mk_invoice(&pool, "st1", "ST-2026-0001", true, Some("orig")).await;
    mk_invoice(&pool, "st2", "ST-2026-0002", true, Some("orig")).await;
    // dieselbe Bedingung prüft Phase A eines Restores
    assert_eq!(
        klein_buch_lib::db::duplicate_locked_stornos(&pool)
            .await
            .unwrap(),
        1
    );

    let err = MIGRATOR.run(&pool).await.unwrap_err();
    assert!(
        err.to_string().contains("Migration 0032 abgebrochen"),
        "{err}"
    );
    assert_eq!(schema_version(&pool).await, "31", "zurückgerollt");
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM pragma_table_info('invoices') WHERE name = 'voided_at'"
        )
        .await,
        0,
        "keine Teiländerung"
    );

    // Über das Entsperren: klare Meldung (409 db_integrity_failed im Server)
    let ctx = AppCtx::for_tests(Paths::under(dir.path()));
    let err = klein_buch_lib::db::run_migrations(&ctx, &pool)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(klein_buch_lib::db::INTEGRITY_REFUSED)
            && msg.contains("mehr als einen festgeschriebenen Storno-Beleg"),
        "{msg}"
    );
}

// ============================================================================
// R1-013: höchstens ein Standard-Zahlungskonto
// ============================================================================

#[tokio::test]
async fn migration_cleans_duplicate_default_accounts_with_audit() {
    let (pool, _dir) = setup_schema_31().await;
    for (id, label, default, active) in [
        ("01928300-0000-7000-8000-0000000000a1", "Alt", 1, 1),
        ("01928300-0000-7000-8000-0000000000a2", "Neu", 1, 1),
        ("01928300-0000-7000-8000-0000000000a3", "Inaktiv", 1, 0),
        ("01928300-0000-7000-8000-0000000000a4", "Bar", 0, 1),
    ] {
        sqlx::query(
            "INSERT INTO payment_accounts (id, label, \"type\", is_default, active)
             VALUES (?, ?, 'bank', ?, ?)",
        )
        .bind(id)
        .bind(label)
        .bind(default)
        .bind(active)
        .execute(&pool)
        .await
        .unwrap();
    }
    MIGRATOR.run(&pool).await.unwrap();
    let defaults: Vec<String> =
        sqlx::query_scalar("SELECT label FROM payment_accounts WHERE is_default = 1")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(defaults, vec!["Alt".to_string()], "das älteste bleibt");
    let audits: Vec<(String, String)> = sqlx::query_as(
        "SELECT entity_id, details_json FROM audit_log
          WHERE action = 'payment_account.default_cleared' ORDER BY entity_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audits.len(), 2);
    assert!(audits[0].0.ends_with("a2") && audits[0].1.contains("second_default"));
    assert!(audits[1].0.ends_with("a3") && audits[1].1.contains("inactive"));
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log
              WHERE action = 'payment_account.default_cleared' AND json_valid(details_json)"
        )
        .await,
        2
    );

    // Index: kein zweites Standardkonto
    assert_blocked(
        sqlx::query(
            "UPDATE payment_accounts SET is_default = 1
              WHERE id = '01928300-0000-7000-8000-0000000000a4'",
        )
        .execute(&pool)
        .await,
        "UNIQUE constraint failed: payment_accounts.is_default",
    );
    // App-Pfade bleiben: neues Standardkonto, Deaktivieren des Standards
    let created = payment_accounts::create(
        &pool,
        &payment_accounts::PaymentAccountInput {
            label: "Geschäftskonto".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: true,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.is_default, 1);
    payment_accounts::set_active(&pool, &created.id, false)
        .await
        .unwrap();
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM payment_accounts WHERE is_default = 1 AND active = 1"
        )
        .await,
        1,
        "ein anderes aktives Konto wurde Standard"
    );
}

// ============================================================================
// R1-S2-ASSET-DISPOSAL
// ============================================================================

#[tokio::test]
async fn disposal_fields_are_final() {
    let (pool, _dir) = setup().await;
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
                             acquisition_fiscal_year, depreciation_method, useful_life_years,
                             book_value_cents, locked_at)
         VALUES ('as1', 'AV-2026-0001', 'Laptop', '2026-01-10', 150000, 2026, 'linear', 3.0,
                 150000, '2026-01-10 10:00:00')",
    )
    .execute(&pool)
    .await
    .unwrap();
    klein_buch_lib::db::repo::assets::dispose(&pool, "as1", "2026-06-01", "sale", 50000, 120000)
        .await
        .unwrap();
    let err =
        klein_buch_lib::db::repo::assets::dispose(&pool, "as1", "2026-07-01", "scrap", 0, 100000)
            .await
            .unwrap_err();
    assert!(err.to_string().contains("bereits veräußert"), "{err}");
    for sql in [
        "UPDATE assets SET disposed = 0 WHERE id = 'as1'",
        "UPDATE assets SET disposal_date = '2026-06-02' WHERE id = 'as1'",
        "UPDATE assets SET disposal_type = 'scrap' WHERE id = 'as1'",
        "UPDATE assets SET disposal_proceeds_cents = 1 WHERE id = 'as1'",
        "UPDATE assets SET disposal_residual_book_value_cents = 1 WHERE id = 'as1'",
    ] {
        assert_blocked(
            sqlx::query(sql).execute(&pool).await,
            "disposal fields are immutable",
        );
    }
    // andere Felder (z. B. Notiz) bleiben pflegbar
    sqlx::query("UPDATE assets SET notes = 'verkauft an Privat' WHERE id = 'as1'")
        .execute(&pool)
        .await
        .unwrap();
}

// ============================================================================
// W14-032: Verwerfen nummerierter Entwürfe
// ============================================================================

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 22, 8, 30, 0).unwrap()
}

#[tokio::test]
async fn voiding_a_draft_keeps_the_number_and_is_final() {
    let (pool, dir) = setup().await;
    let number = klein_buch_lib::db::numbering::next_number(
        &pool,
        klein_buch_lib::domain::numbering::DocType::Invoice,
        2026,
    )
    .await
    .unwrap();
    mk_invoice(&pool, "d1", &number, false, None).await;
    mk_item(&pool, "d1-1", "d1", 1).await.unwrap();

    // Begründung Pflicht
    let err = klein_buch_lib::commands::invoices::void_draft_core(&pool, "d1", "kurz", now())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::BadRequest(_)), "{err}");

    let row = klein_buch_lib::commands::invoices::void_draft_core(&pool, "d1", REASON, now())
        .await
        .unwrap();
    assert_eq!(row.voided_at.as_deref(), Some("2026-05-22 08:30:00"));
    assert_eq!(row.void_reason.as_deref(), Some(REASON));
    assert_eq!(row.status, "draft");
    assert_eq!(row.invoice_number, number, "Nummer bleibt am Beleg");
    // Nummer bleibt verbraucht: der Zähler vergibt die nächste
    let next = klein_buch_lib::db::numbering::next_number(
        &pool,
        klein_buch_lib::domain::numbering::DocType::Invoice,
        2026,
    )
    .await
    .unwrap();
    assert_ne!(next, number);
    // genau ein Audit, gültiges JSON mit Nummer und Grund
    let details: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'invoice.draft_voided' AND entity_id = 'd1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(v["number"], number);
    assert_eq!(v["reason"], REASON);

    // kein zweites Verwerfen, kein Festschreiben, keine Änderung
    let err = klein_buch_lib::commands::invoices::void_draft_core(&pool, "d1", REASON, now())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bereits verworfen"), "{err}");
    let paths = Paths::under(dir.path());
    let err = klein_buch_lib::commands::invoices::run_lock_pipeline(&pool, &paths, "d1", "N/A")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("verworfen"), "{err}");
    for sql in [
        "UPDATE invoices SET locked_at = '2026-05-23 10:00:00', status = 'issued' WHERE id = 'd1'",
        "UPDATE invoices SET voided_at = NULL, void_reason = NULL WHERE id = 'd1'",
        "UPDATE invoices SET gross_amount_cents = 1 WHERE id = 'd1'",
    ] {
        assert_blocked(sqlx::query(sql).execute(&pool).await, "voided");
    }
    assert_blocked(
        sqlx::query("UPDATE invoice_items SET quantity = 3.0 WHERE id = 'd1-1'")
            .execute(&pool)
            .await,
        "items are immutable",
    );
    assert_blocked(
        sqlx::query("DELETE FROM invoices WHERE id = 'd1'")
            .execute(&pool)
            .await,
        "use storno",
    );
}

#[tokio::test]
async fn only_open_drafts_can_be_voided() {
    let (pool, _dir) = setup().await;
    mk_invoice(&pool, "l1", "RE-2026-0001", true, None).await;
    let err = klein_buch_lib::commands::invoices::void_draft_core(&pool, "l1", REASON, now())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("festgeschrieben"), "{err}");
    // DB-Backstop ohne App-Prüfung
    assert_blocked(
        sqlx::query(
            "UPDATE invoices SET voided_at = '2026-05-22 00:00:00', void_reason = 'manipuliert'
              WHERE id = 'l1'",
        )
        .execute(&pool)
        .await,
        "only unlocked invoice drafts can be voided",
    );
    mk_invoice(&pool, "d1", "RE-2026-0002", false, None).await;
    assert_blocked(
        sqlx::query("UPDATE invoices SET voided_at = '2026-05-22 00:00:00' WHERE id = 'd1'")
            .execute(&pool)
            .await,
        "only with a reason",
    );
    assert_blocked(
        sqlx::query(
            "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date,
                contact_id, seller_name, seller_street, seller_postal_code, seller_city,
                net_amount_cents, gross_amount_cents, voided_at, void_reason)
             VALUES ('d9', 'RE-2026-0009', 2026, 'issued', '2026-05-20', ?, 'V', 'W', '1', 'O',
                1, 1, '2026-05-22 00:00:00', 'gleich verworfen')",
        )
        .bind(CONTACT)
        .execute(&pool)
        .await,
        "void them afterwards",
    );
}

#[tokio::test]
async fn voided_drafts_no_longer_block_anonymization() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    mk_invoice(&pool, "d1", "RE-2026-0001", false, None).await;
    mk_item(&pool, "d1-1", "d1", 1).await.unwrap();
    mk_quote(&pool, "q1", "AN-2026-0001").await;
    assert_eq!(
        contacts::count_open_drafts(&pool, CONTACT).await.unwrap(),
        (1, 1)
    );
    let err = klein_buch_lib::commands::contacts::anonymize_core(&pool, CONTACT)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("verwerfen"), "{err}");

    klein_buch_lib::commands::invoices::invoices_void_draft(
        &ctx,
        klein_buch_lib::commands::invoices::InvoicesVoidDraftArgs {
            id: "d1".into(),
            reason: REASON.into(),
        },
    )
    .await
    .unwrap();
    // Angebots-Entwurf: „Zurückziehen“ verwirft ihn
    klein_buch_lib::commands::quotes::quotes_cancel(
        &ctx,
        klein_buch_lib::commands::quotes::QuotesCancelArgs {
            args: klein_buch_lib::commands::quotes::CancelArgs {
                quote_id: "q1".into(),
                reason: "nicht mehr benötigt".into(),
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        contacts::count_open_drafts(&pool, CONTACT).await.unwrap(),
        (0, 0)
    );
    // Personendaten im Snapshot der nie ausgestellten Belege (F2c)
    sqlx::query(
        "UPDATE quotes SET buyer_name = 'Kunde Beispiel', buyer_street = 'Hauptstr. 1',
                buyer_email = 'kunde@example.org', notes = 'ruft abends an' WHERE id = 'q1'",
    )
    .execute(&pool)
    .await
    .unwrap();
    klein_buch_lib::commands::contacts::anonymize_core(&pool, CONTACT)
        .await
        .expect("Anonymisierung möglich");
    // F2c (ADR 0039 Nr. 124 (3)): Der verworfene Entwurf und das zurückgezogene,
    // nie ausgestellte Angebot verlieren den Empfänger-Snapshot; Nummer, Grund,
    // Betrag und Positionen bleiben.
    let placeholder = klein_buch_lib::domain::anonymize::anonymized_name(CONTACT);
    for (table, id) in [("invoices", "d1"), ("quotes", "q1")] {
        let row: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = sqlx::query_as(&format!(
            "SELECT buyer_name, buyer_street, buyer_email, notes FROM {table} WHERE id = ?"
        ))
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            row,
            (Some(placeholder.clone()), None, None, None),
            "{table} {id}"
        );
    }
    let kept: (String, Option<String>, i64, Option<String>) = sqlx::query_as(
        "SELECT invoice_number, void_reason, gross_amount_cents, buyer_country_code
           FROM invoices WHERE id = 'd1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        kept,
        (
            "RE-2026-0001".to_string(),
            Some(REASON.to_string()),
            10000,
            Some("DE".to_string())
        )
    );
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM invoice_items WHERE invoice_id = 'd1'"
        )
        .await,
        1,
        "Positionen bleiben"
    );
    let details: String =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'contact.anonymize'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(details["scrubbedVoidedInvoices"][0], "RE-2026-0001");
    assert_eq!(details["scrubbedWithdrawnQuotes"][0], "AN-2026-0001");
    assert!(!details.to_string().contains("Kunde Beispiel"), "{details}");

    // Das zurückgezogene Angebot bleibt zurückgezogen und wird nie festgeschrieben
    let err = klein_buch_lib::commands::quotes::quotes_issue(
        &ctx,
        klein_buch_lib::commands::quotes::QuotesIssueArgs { id: "q1".into() },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("nicht im Entwurf"), "{err}");
    assert_blocked(
        sqlx::query("UPDATE quotes SET status = 'draft' WHERE id = 'q1'")
            .execute(&pool)
            .await,
        "quote is canceled",
    );
}

#[tokio::test]
async fn voiding_clears_recurring_and_restore_markers_and_storno_reuse() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    mk_invoice(&pool, "d1", "RE-2026-0001", false, None).await;
    let pending = "recurring_invoice_pending:tmpl-1";
    let conflict = "restored_number_conflict:invoice:d1";
    for (k, v) in [
        (pending, "d1|2026-05-01|issue"),
        ("recurring_invoice_pending:tmpl-2", "d10|2026-05-01|issue"),
        (conflict, "RE-2026-0001"),
    ] {
        klein_buch_lib::db::repo::app_settings::set(&pool, k, v)
            .await
            .unwrap();
    }
    klein_buch_lib::commands::invoices::invoices_void_draft(
        &ctx,
        klein_buch_lib::commands::invoices::InvoicesVoidDraftArgs {
            id: "d1".into(),
            reason: REASON.into(),
        },
    )
    .await
    .unwrap();
    let get = |k: &'static str| {
        let pool = pool.clone();
        async move {
            klein_buch_lib::db::repo::app_settings::get(&pool, k)
                .await
                .unwrap()
        }
    };
    assert!(get(pending).await.is_none(), "Marker auf den Entwurf weg");
    assert!(get(conflict).await.is_none(), "Restore-Vormerkung erledigt");
    assert!(
        get("recurring_invoice_pending:tmpl-2").await.is_some(),
        "fremder Marker (d10 ≠ d1) bleibt"
    );
    let v: serde_json::Value = serde_json::from_str(
        &sqlx::query_scalar::<_, String>(
            "SELECT details_json FROM audit_log WHERE action = 'invoice.draft_voided'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(v["recurring_markers_cleared"], 1);
    assert_eq!(v["restore_conflict_cleared"], true);

    // Storno-Entwurf: verworfen wird er beim nächsten Stornieren nicht
    // wiederverwendet
    mk_invoice(&pool, "orig", "RE-2026-0002", true, None).await;
    mk_invoice(&pool, "st1", "ST-2026-0001", false, Some("orig")).await;
    assert_eq!(
        invoices::open_storno_draft(&pool, "orig")
            .await
            .unwrap()
            .map(|t| t.0),
        Some("st1".to_string())
    );
    klein_buch_lib::commands::invoices::void_draft_core(&pool, "st1", REASON, now())
        .await
        .unwrap();
    assert!(invoices::open_storno_draft(&pool, "orig")
        .await
        .unwrap()
        .is_none());
    // die Liste zeigt den verworfenen Entwurf mit Zeitpunkt
    let list = invoices::list(&pool, &invoices::ListFilter::default())
        .await
        .unwrap();
    let st1 = list.iter().find(|i| i.id == "st1").unwrap();
    assert!(st1.voided_at.is_some());
}

// ============================================================================
// W2-INTEGRITY-UNREADABLE
// ============================================================================

#[tokio::test]
async fn integrity_history_records_missing_and_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let (pool, dir) = setup().await;
    let archive_dir = dir.path().join("archive");
    std::fs::create_dir_all(&archive_dir).unwrap();
    let mut stored = Vec::new();
    for name in ["RE-2026-0001.pdf", "RE-2026-0002.pdf", "RE-2026-0003.pdf"] {
        stored.push(
            klein_buch_lib::archive::store_bytes(
                &pool,
                &archive_dir,
                2026,
                klein_buch_lib::archive::ArchiveKind::InvoicePdf,
                name,
                "application/pdf",
                name.as_bytes(),
            )
            .await
            .unwrap(),
        );
    }
    std::fs::remove_file(&stored[1].file_path).unwrap();
    std::fs::set_permissions(&stored[2].file_path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let root = std::fs::read(&stored[2].file_path).is_ok();

    let sum = klein_buch_lib::archive::run_full_scan(&pool).await.unwrap();
    std::fs::set_permissions(&stored[2].file_path, std::fs::Permissions::from_mode(0o400)).unwrap();
    let row: (
        i64,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT files_failed, files_missing, files_unreadable, missing_archive_ids_json,
                unreadable_archive_ids_json
           FROM archive_integrity_checks WHERE id = ?",
    )
    .bind(sum.check_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 1, "fehlend zählt als Integritätsverletzung");
    assert_eq!(row.1, Some(1));
    let missing: Vec<String> = serde_json::from_str(row.3.as_deref().unwrap()).unwrap();
    assert_eq!(missing, vec![stored[1].archive_id.clone()]);
    let unreadable: Vec<String> = serde_json::from_str(row.4.as_deref().unwrap()).unwrap();
    if root {
        // als root greifen die Dateirechte nicht
        assert_eq!(row.2, Some(0));
    } else {
        assert_eq!(row.2, Some(1));
        assert_eq!(unreadable, vec![stored[2].archive_id.clone()]);
    }
}

// ============================================================================
// R1-012: Fremdschlüssel nach den Migrationen
// ============================================================================

#[tokio::test]
async fn foreign_key_violation_blocks_unlock() {
    let (pool, dir) = setup().await;
    let ctx = AppCtx::for_tests(Paths::under(dir.path()));
    klein_buch_lib::db::run_migrations(&ctx, &pool)
        .await
        .expect("intakte DB");
    // verwaisten Verweis von außen herstellen (Fremdschlüssel aus)
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (id, rule_id, title, body, severity)
             VALUES ('n1', 'rule_gibt_es_nicht', 'T', 'B', 'info')",
        )
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    let err = klein_buch_lib::db::run_migrations(&ctx, &pool)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(klein_buch_lib::db::INTEGRITY_REFUSED) && msg.contains("notifications"),
        "{msg}"
    );
    assert!(
        !msg.contains("Kunde"),
        "keine Inhalte in der Meldung: {msg}"
    );
    // nichts angefasst
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM notifications WHERE id = 'n1'").await,
        1
    );
}

// ============================================================================
// Lauf F2c: Fremdschlüssel VOR der Umstellung (S1)
// ============================================================================

/// Ein Altfehler (verwaister Verweis) in einer Datenbank mit Schema 31 lehnt
/// das Entsperren ab, **bevor** 0032 etwas speichert: Schema bleibt 31, die
/// Bereinigung der Standardkonten ist nicht gelaufen, das vorige Image öffnet
/// die Datenbank weiter.
#[tokio::test]
async fn foreign_key_violation_before_migration_keeps_old_schema() {
    let (pool, dir) = setup_schema_31().await;
    for (id, label) in [("pa-1", "Bank A"), ("pa-2", "Bank B")] {
        sqlx::query(
            "INSERT INTO payment_accounts (id, label, \"type\", is_default, active)
             VALUES (?, ?, 'bank', 1, 1)",
        )
        .bind(id)
        .bind(label)
        .execute(&pool)
        .await
        .unwrap();
    }
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (id, rule_id, title, body, severity)
             VALUES ('n1', 'rule_gibt_es_nicht', 'T', 'B', 'info')",
        )
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    let ctx = AppCtx::for_tests(Paths::under(dir.path()));
    let err = klein_buch_lib::db::run_migrations(&ctx, &pool)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(klein_buch_lib::db::INTEGRITY_REFUSED)
            && msg.contains("unverändert auf Schema 31")
            && !msg.contains(klein_buch_lib::db::INTEGRITY_AFTER_MIGRATION),
        "{msg}"
    );
    assert_eq!(schema_version(&pool).await, "31", "nichts umgestellt");
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM pragma_table_info('invoices') WHERE name = 'voided_at'"
        )
        .await,
        0,
        "0032 nicht angewendet"
    );
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM payment_accounts WHERE is_default = 1"
        )
        .await,
        2,
        "Bereinigung der Standardkonten nicht gelaufen"
    );
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'payment_account.default_cleared'"
        )
        .await,
        0
    );
    // Nach der Reparatur geht die Umstellung durch.
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query("DELETE FROM notifications WHERE id = 'n1'")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    klein_buch_lib::db::run_migrations(&ctx, &pool)
        .await
        .expect("nach Reparatur");
    assert_eq!(
        schema_version(&pool).await,
        EXPECTED_SCHEMA_VERSION.to_string()
    );
}

/// Die Meldung der Prüfung nach den Migrationen sagt ehrlich, ob die
/// Umstellung schon gespeichert ist (dann kein „nicht angefasst“).
#[test]
fn integrity_message_after_migration_is_honest() {
    let v = vec![klein_buch_lib::db::ForeignKeyViolation {
        table: "notifications".into(),
        rowid: Some(7),
        parent: "notification_rules".into(),
    }];
    let after = klein_buch_lib::db::integrity_message_after(Some(31), &v);
    assert!(
        after.contains(klein_buch_lib::db::INTEGRITY_AFTER_MIGRATION)
            && after.contains("Umstellung ist gespeichert")
            && after.contains("notifications (rowid 7) → notification_rules")
            && !after.contains("nicht angefasst"),
        "{after}"
    );
    let none = klein_buch_lib::db::integrity_message_after(None, &v);
    assert!(
        none.contains("nicht angefasst")
            && !none.contains(klein_buch_lib::db::INTEGRITY_AFTER_MIGRATION),
        "{none}"
    );
}

// ============================================================================
// Lauf F2c: verworfene Entwürfe — nur die Anonymisierung ändert den Snapshot
// ============================================================================

/// Jede Spalte von `invoices` außer den freigegebenen Empfängerfeldern und
/// der Notiz muss in `trg_invoices_voided_final` als „unverändert“ stehen —
/// eine künftige Spalte, die dort fehlt, wäre an verworfenen Entwürfen
/// änderbar (Lockerung durch Vergessen).
#[tokio::test]
async fn voided_final_trigger_lists_every_invoice_column() {
    let (pool, _dir) = setup().await;
    let sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_invoices_voided_final'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('invoices')")
        .fetch_all(&pool)
        .await
        .unwrap();
    let anonymizable = [
        "buyer_name",
        "buyer_street",
        "buyer_postal_code",
        "buyer_city",
        "buyer_vat_id",
        "buyer_email",
        "notes",
    ];
    for col in &columns {
        if anonymizable.contains(&col.as_str()) {
            assert!(
                !sql.contains(&format!("NEW.{col} IS OLD.{col}")),
                "{col} ist anonymisierbar"
            );
        } else {
            assert!(
                sql.contains(&format!("NEW.{col} IS OLD.{col}")),
                "Spalte {col} fehlt in trg_invoices_voided_final"
            );
        }
    }
}

#[tokio::test]
async fn voided_draft_snapshot_changes_only_by_anonymization() {
    let (pool, _dir) = setup().await;
    mk_invoice(&pool, "d1", "RE-2026-0001", false, None).await;
    sqlx::query(
        "UPDATE invoices SET buyer_street = 'Hauptstr. 1', buyer_email = 'kunde@example.org',
                notes = 'ruft abends an' WHERE id = 'd1'",
    )
    .execute(&pool)
    .await
    .unwrap();
    klein_buch_lib::commands::invoices::void_draft_core(&pool, "d1", REASON, now())
        .await
        .unwrap();
    let placeholder = klein_buch_lib::domain::anonymize::anonymized_name(CONTACT);
    let scrub = "UPDATE invoices SET buyer_name = ?, buyer_street = NULL,
                    buyer_postal_code = NULL, buyer_city = NULL, buyer_vat_id = NULL,
                    buyer_email = NULL, notes = NULL WHERE id = 'd1'";
    // Kontakt nicht anonymisiert → gesperrt
    assert_blocked(
        sqlx::query(scrub).bind(&placeholder).execute(&pool).await,
        "invoice draft is voided",
    );
    sqlx::query(
        "UPDATE contacts SET name = ?, anonymized_at = '2026-05-23 10:00:00', archived = 1
          WHERE id = ?",
    )
    .bind(&placeholder)
    .bind(CONTACT)
    .execute(&pool)
    .await
    .unwrap();
    // anderer Name als der Platzhalter → gesperrt
    assert_blocked(
        sqlx::query(scrub)
            .bind("Jemand anderes")
            .execute(&pool)
            .await,
        "invoice draft is voided",
    );
    // zusätzlich eine andere Spalte → gesperrt
    for extra in [
        "gross_amount_cents = 1",
        "void_reason = 'anderer Grund'",
        "updated_at = '2030-01-01 00:00:00'",
        "buyer_country_code = 'AT'",
    ] {
        assert_blocked(
            sqlx::query(&format!(
                "UPDATE invoices SET buyer_name = ?, buyer_street = NULL,
                    buyer_postal_code = NULL, buyer_city = NULL, buyer_vat_id = NULL,
                    buyer_email = NULL, notes = NULL, {extra} WHERE id = 'd1'"
            ))
            .bind(&placeholder)
            .execute(&pool)
            .await,
            "invoice draft is voided",
        );
    }
    // Feld nicht geleert → gesperrt
    assert_blocked(
        sqlx::query("UPDATE invoices SET buyer_name = ? WHERE id = 'd1'")
            .bind(&placeholder)
            .execute(&pool)
            .await,
        "invoice draft is voided",
    );
    // genau die Anonymisierung → erlaubt
    sqlx::query(scrub)
        .bind(&placeholder)
        .execute(&pool)
        .await
        .expect("Anonymisierung des Snapshots");
    // festgeschriebene Rechnung bleibt unberührt
    mk_invoice(&pool, "l1", "RE-2026-0002", true, None).await;
    assert_blocked(
        sqlx::query("UPDATE invoices SET buyer_name = ? WHERE id = 'l1'")
            .bind(&placeholder)
            .execute(&pool)
            .await,
        "invoice is locked",
    );
}

// ============================================================================
// Lauf F2c: verworfener Entwurf aus einem Angebot — Umwandlung zurück
// ============================================================================

#[tokio::test]
async fn voiding_converted_draft_reverts_quote_and_allows_new_conversion() {
    let (pool, _dir) = setup().await;
    mk_quote(&pool, "q1", "AN-2026-0001").await;
    sqlx::query(
        "UPDATE quotes SET locked_at = '2026-05-20 10:00:00', status = 'accepted',
                sent_at = '2026-05-20 10:00:00', accepted_at = '2026-05-21 10:00:00'
          WHERE id = 'q1'",
    )
    .execute(&pool)
    .await
    .unwrap();
    mk_invoice(&pool, "d1", "RE-2026-0001", false, None).await;
    sqlx::query("UPDATE invoices SET derived_from_quote_id = 'q1' WHERE id = 'd1'")
        .execute(&pool)
        .await
        .unwrap();
    klein_buch_lib::db::repo::quotes::mark_converted(&pool, "q1", "d1")
        .await
        .unwrap();

    // umgewandelt ist endgültig — App und DB
    let err =
        klein_buch_lib::db::repo::quotes::cancel(&pool, "q1", "Kunde springt ab, kein Bedarf")
            .await
            .unwrap_err();
    assert!(
        err.to_string()
            .contains("bereits in eine Rechnung umgewandelt"),
        "{err}"
    );
    for sql in [
        "UPDATE quotes SET status = 'canceled' WHERE id = 'q1'",
        "UPDATE quotes SET status = 'accepted', converted_invoice_id = NULL,
                converted_at = NULL WHERE id = 'q1'",
        "UPDATE quotes SET converted_invoice_id = NULL WHERE id = 'q1'",
    ] {
        assert_blocked(sqlx::query(sql).execute(&pool).await, "quote is converted");
    }

    // Entwurf verwerfen → Angebot wieder angenommen, Audit
    klein_buch_lib::commands::invoices::void_draft_core(&pool, "d1", REASON, now())
        .await
        .unwrap();
    let q: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, converted_invoice_id, converted_at FROM quotes WHERE id = 'q1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(q, ("accepted".to_string(), None, None));
    let details: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'quote.conversion_reverted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        details.contains("RE-2026-0001") && details.contains("AN-2026-0001"),
        "{details}"
    );
    let voided: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'invoice.draft_voided'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        voided.contains("\"quote_conversion_reverted\":\"AN-2026-0001\""),
        "{voided}"
    );

    // erneut umwandelbar
    mk_invoice(&pool, "d2", "RE-2026-0002", false, None).await;
    klein_buch_lib::db::repo::quotes::mark_converted(&pool, "q1", "d2")
        .await
        .expect("neue Umwandlung");
}

// ============================================================================
// Lauf F2c: Angebot zurückziehen — Grund, Transaktion, Status-Guard
// ============================================================================

#[tokio::test]
async fn quote_cancel_needs_reason_and_reports_repeat_cleanly() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    mk_quote(&pool, "q1", "AN-2026-0001").await;
    let cancel = |reason: &str| {
        let ctx = ctx.clone();
        let reason = reason.to_string();
        async move {
            klein_buch_lib::commands::quotes::quotes_cancel(
                &ctx,
                klein_buch_lib::commands::quotes::QuotesCancelArgs {
                    args: klein_buch_lib::commands::quotes::CancelArgs {
                        quote_id: "q1".into(),
                        reason,
                    },
                },
            )
            .await
        }
    };
    for short in ["", "   ", "zu kurz"] {
        let err = cancel(short).await.unwrap_err();
        assert!(matches!(err, Error::BadRequest(_)), "{short:?}: {err}");
    }
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM quotes WHERE status = 'canceled'"
        )
        .await,
        0
    );
    let row = cancel("  Kunde hat abgesagt  ").await.unwrap();
    assert_eq!(row.status, "canceled");
    assert_eq!(row.canceled_reason.as_deref(), Some("Kunde hat abgesagt"));
    let err = cancel("Noch einmal zurückziehen").await.unwrap_err();
    assert!(err.to_string().contains("bereits zurückgezogen"), "{err}");
    assert!(
        !err.to_string().contains("RAISE"),
        "kein roher Trigger-Fehler"
    );
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'quote.cancel'"
        )
        .await,
        1,
        "genau ein Audit, in derselben Transaktion"
    );
}

// ============================================================================
// Lauf F2c: halbes Storno-Paar aus dem Altbestand
// ============================================================================

/// Altbestand (vor R1-003): Storno festgeschrieben, Original noch `issued`.
/// Erneutes Stornieren legt keinen zweiten Storno an, sondern vervollständigt
/// das Paar mit Audit.
#[tokio::test]
async fn legacy_half_storno_pair_is_completed_without_second_storno() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    klein_buch_lib::db::repo::seller_profile::upsert(
        &pool,
        &klein_buch_lib::db::repo::seller_profile::SellerProfileInput {
            name: "Verkäufer".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "info@example.org".into(),
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
    mk_invoice(&pool, "orig", "RE-2026-0001", true, None).await;
    mk_invoice(&pool, "st1", "ST-2026-0001", true, Some("orig")).await;
    sqlx::query("UPDATE invoices SET cancel_reason = 'Doppelt berechnet' WHERE id = 'st1'")
        .execute(&pool)
        .await
        .unwrap();
    let before = scalar(&pool, "SELECT COUNT(*) FROM invoices").await;

    let res = klein_buch_lib::commands::invoices::invoices_cancel(
        &ctx,
        klein_buch_lib::commands::invoices::InvoicesCancelArgs {
            args: klein_buch_lib::commands::invoices::CancelArgs {
                invoice_id: "orig".into(),
                reason: "Erneut storniert".into(),
                storno_date: None,
            },
        },
    )
    .await
    .expect("Paar vervollständigt");
    assert_eq!(res.storno.invoice.id, "st1");
    assert_eq!(res.original.status, "canceled");
    assert_eq!(res.original.canceled_by_storno_id.as_deref(), Some("st1"));
    assert_eq!(
        res.original.canceled_at.as_deref(),
        Some("2026-05-20 10:00:00")
    );
    assert_eq!(
        res.original.cancel_reason.as_deref(),
        Some("Doppelt berechnet")
    );
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM invoices").await,
        before,
        "kein neuer Storno-Beleg"
    );
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM archive_entries").await,
        0,
        "nichts archiviert"
    );
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.cancel_pair_completed'
               AND entity_id = 'orig'"
        )
        .await,
        1
    );
    // danach: „bereits storniert“
    let err = klein_buch_lib::commands::invoices::invoices_cancel(
        &ctx,
        klein_buch_lib::commands::invoices::InvoicesCancelArgs {
            args: klein_buch_lib::commands::invoices::CancelArgs {
                invoice_id: "orig".into(),
                reason: "Noch einmal".into(),
                storno_date: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("bereits storniert"), "{err}");
}

// ============================================================================
// Lauf F2c: veräußerte Anlage — AfA und Stammdaten endgültig
// ============================================================================

#[tokio::test]
async fn disposed_asset_keeps_depreciation_and_master_data() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
                             acquisition_fiscal_year, depreciation_method, useful_life_years,
                             book_value_cents)
         VALUES ('as1', 'AV-2026-0001', 'Laptop', '2026-01-10', 120000, 2026, 'linear', 1.0,
                 90000)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO depreciation_entries (id, asset_id, fiscal_year, depreciation_amount_cents,
                months_in_year, book_value_before_cents, book_value_after_cents)
         VALUES ('de1', 'as1', 2026, 30000, 3, 120000, 90000)",
    )
    .execute(&pool)
    .await
    .unwrap();
    klein_buch_lib::db::repo::assets::dispose(&pool, "as1", "2026-03-31", "scrap", 0, 90000)
        .await
        .unwrap();

    let err = klein_buch_lib::depreciation::accrue_yearly::reset_asset(
        &pool,
        &ctx.paths,
        &ctx.session,
        "as1",
        klein_buch_lib::clock::now_utc(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("veräußert"), "{err}");
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM depreciation_entries WHERE asset_id = 'as1'"
        )
        .await,
        1,
        "AfA des Abgangsjahres bleibt"
    );
    assert_blocked(
        sqlx::query("DELETE FROM depreciation_entries WHERE asset_id = 'as1'")
            .execute(&pool)
            .await,
        "depreciation entries are final",
    );
    for sql in [
        "UPDATE assets SET acquisition_cost_cents = 1 WHERE id = 'as1'",
        "UPDATE assets SET depreciation_method = 'gwg_sofort', useful_life_years = NULL
          WHERE id = 'as1'",
        "UPDATE assets SET book_value_cents = 120000, last_depreciation_year = NULL
          WHERE id = 'as1'",
        "UPDATE assets SET label = 'anders' WHERE id = 'as1'",
    ] {
        assert_blocked(
            sqlx::query(sql).execute(&pool).await,
            "disposal fields are immutable",
        );
    }
    let input = klein_buch_lib::domain::asset::AssetInput {
        label: "Laptop neu".into(),
        acquisition_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        acquisition_cost_cents: 100000,
        expense_id: None,
        vendor_contact_id: None,
        depreciation_method: "linear".into(),
        useful_life_years: Some(3.0),
        afa_category: None,
        business_share_percent: 100.0,
        notes: None,
    };
    let err =
        klein_buch_lib::db::repo::assets::update(&pool, "as1", &input, "linear", Some(3.0), 100000)
            .await
            .unwrap_err();
    assert!(err.to_string().contains("veräußert"), "{err}");
    // Jahresabschluss darf die veräußerte Anlage festschreiben
    sqlx::query("UPDATE assets SET locked_at = '2027-01-05 10:00:00' WHERE id = 'as1'")
        .execute(&pool)
        .await
        .expect("locked_at bleibt setzbar");
}

/// Lauf F2c (neuer Fund): Beim Abgang holt `assets_dispose` die AfA der
/// Vorjahre nach, bevor es die anteilige AfA des Abgangsjahres rechnet —
/// danach bucht der AfA-Lauf die veräußerte Anlage nicht mehr.
#[tokio::test]
async fn dispose_catches_up_missing_prior_year_depreciation() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
                             acquisition_fiscal_year, depreciation_method, useful_life_years,
                             book_value_cents)
         VALUES ('as1', 'AV-2025-0001', 'Server', '2025-01-01', 120000, 2025, 'linear', 4.0,
                 120000)",
    )
    .execute(&pool)
    .await
    .unwrap();
    klein_buch_lib::commands::assets::assets_dispose(
        &ctx,
        klein_buch_lib::commands::assets::AssetsDisposeArgs {
            args: klein_buch_lib::commands::assets::AssetDisposeArgs {
                asset_id: "as1".into(),
                disposal_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
                disposal_type: "scrap".into(),
                proceeds_cents: 0,
            },
        },
    )
    .await
    .unwrap();
    let entries: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT fiscal_year, depreciation_amount_cents FROM depreciation_entries
          WHERE asset_id = 'as1' ORDER BY fiscal_year",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        entries.len(),
        2,
        "2025 nachgebucht, 2026 anteilig: {entries:?}"
    );
    assert_eq!(entries[0], (2025, 30000), "volle Jahres-AfA 2025");
    assert_eq!(entries[1], (2026, 15000), "6/12 im Abgangsjahr");
    let residual: i64 = sqlx::query_scalar(
        "SELECT disposal_residual_book_value_cents FROM assets WHERE id = 'as1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(residual, 120000 - 30000 - 15000);
}

// ============================================================================
// Lauf F3b: Nachzügler aus der Nachprüfung von F2
// ============================================================================

/// Anlage mit drei gebuchten, offenen AfA-Jahren (2024–2026, linear 3 Jahre,
/// 1.200 €).
async fn mk_asset_with_three_years(pool: &SqlitePool, id: &str, locked_2026: bool) {
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
                             acquisition_fiscal_year, depreciation_method, useful_life_years,
                             book_value_cents, last_depreciation_year)
         VALUES (?, 'AV-2024-0001', 'Laptop', '2024-01-01', 120000, 2024, 'linear', 3.0, 0, 2026)",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    for (year, before) in [(2024, 120000), (2025, 80000), (2026, 40000)] {
        sqlx::query(
            "INSERT INTO depreciation_entries (id, asset_id, fiscal_year, depreciation_amount_cents,
                    months_in_year, book_value_before_cents, book_value_after_cents, locked_at)
             VALUES (?, ?, ?, 40000, 12, ?, ?, ?)",
        )
        .bind(format!("{id}-{year}"))
        .bind(id)
        .bind(year)
        .bind(before)
        .bind(before - 40000)
        .bind((locked_2026 && year == 2026).then_some("2027-01-05 10:00:00"))
        .execute(pool)
        .await
        .unwrap();
    }
}

fn dispose_args(
    id: &str,
    date: (i32, u32, u32),
) -> klein_buch_lib::commands::assets::AssetsDisposeArgs {
    klein_buch_lib::commands::assets::AssetsDisposeArgs {
        args: klein_buch_lib::commands::assets::AssetDisposeArgs {
            asset_id: id.into(),
            disposal_date: chrono::NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            disposal_type: "scrap".into(),
            proceeds_cents: 0,
        },
    }
}

fn ctx_at(pool: &SqlitePool, dir: &std::path::Path, day: (i32, u32, u32)) -> Arc<AppCtx> {
    let ctx = ctx_with(pool, dir);
    ctx.set_clock(Arc::new(klein_buch_lib::clock::FixedClock::at_berlin(
        chrono::NaiveDate::from_ymd_opt(day.0, day.1, day.2).unwrap(),
        12,
        0,
    )));
    ctx
}

/// F3b (S2): Abgang in ein früheres Jahr, während AfA für spätere Jahre gebucht
/// ist. Die offenen Einträge ab dem Abgangsjahr werden in der Transaktion des
/// Abgangs entfernt (Audit mit den Werten), das Abgangsjahr bekommt die
/// anteilige AfA (R 7.4 Abs. 8 EStR) — AfA plus Restbuchwert ergeben genau die
/// Anschaffungskosten.
#[tokio::test]
async fn dispose_removes_later_depreciation_and_books_pro_rata() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 12, 15));
    mk_asset_with_three_years(&pool, "as1", false).await;

    klein_buch_lib::commands::assets::assets_dispose(&ctx, dispose_args("as1", (2025, 10, 15)))
        .await
        .unwrap();

    let entries: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT fiscal_year, depreciation_amount_cents, months_in_year FROM depreciation_entries
          WHERE asset_id = 'as1' ORDER BY fiscal_year",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        entries,
        vec![(2024, 40000, 12), (2025, 33333, 10)],
        "2026 entfernt, 2025 anteilig (10/12)"
    );
    let (residual, book_value, last_year): (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT disposal_residual_book_value_cents, book_value_cents, last_depreciation_year
           FROM assets WHERE id = 'as1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(residual, 46667);
    assert_eq!((book_value, last_year), (46667, Some(2025)));
    assert_eq!(
        40000 + 33333 + residual,
        120000,
        "AfA + Restbuchwert = Anschaffung"
    );
    let details: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'depreciation.reset_for_disposal'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    let removed: Vec<i64> = details["removed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["fiscal_year"].as_i64().unwrap())
        .collect();
    assert_eq!(removed, vec![2025, 2026], "{details}");
    assert_eq!(details["book_value_restored"], 80000, "{details}");
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'asset.dispose'"
        )
        .await,
        1
    );
}

/// F3b (S2): Festgeschriebene AfA nach dem Abgangsjahr, ein später
/// abgeschlossenes Jahr mit der (voll abgeschriebenen) Anlage im Bestand und ein
/// Abgang vor der Anschaffung lehnen ab, ohne etwas zu ändern.
#[tokio::test]
async fn dispose_refuses_locked_later_depreciation_and_date_before_purchase() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2027, 3, 1));
    mk_asset_with_three_years(&pool, "as1", true).await;
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
                afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (2026, 0, 0, 40000, -40000, 'test', 32)",
    )
    .execute(&pool)
    .await
    .unwrap();
    // GWG, 2024 voll abgeschrieben — ohne AfA-Eintrag 2026, aber im Bestand 2026.
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
                             acquisition_fiscal_year, depreciation_method, book_value_cents,
                             last_depreciation_year)
         VALUES ('as2', 'AV-2024-0002', 'Drucker', '2024-02-01', 50000, 2024, 'gwg_sofort', 0,
                 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let err =
        klein_buch_lib::commands::assets::assets_dispose(&ctx, dispose_args("as2", (2025, 6, 1)))
            .await
            .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("2026 ist abgeschlossen") && msg.contains("2027"),
        "{msg}"
    );

    let err =
        klein_buch_lib::commands::assets::assets_dispose(&ctx, dispose_args("as1", (2025, 10, 15)))
            .await
            .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("2026") && msg.contains("festgeschrieben") && msg.contains("2027"),
        "{msg}"
    );
    let err =
        klein_buch_lib::commands::assets::assets_dispose(&ctx, dispose_args("as1", (2023, 12, 31)))
            .await
            .unwrap_err();
    assert!(
        err.to_string().contains("vor dem Anschaffungsdatum"),
        "{err}"
    );

    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM depreciation_entries WHERE asset_id = 'as1'"
        )
        .await,
        3,
        "nichts entfernt"
    );
    assert_eq!(
        scalar(&pool, "SELECT disposed FROM assets WHERE id = 'as1'").await,
        0
    );
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action LIKE 'depreciation.%' OR action = 'asset.dispose'"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(&pool, "SELECT disposed FROM assets WHERE id = 'as2'").await,
        0
    );
    // Im ersten Jahr nach dem abgeschlossenen geht der Abgang.
    for id in ["as1", "as2"] {
        klein_buch_lib::commands::assets::assets_dispose(&ctx, dispose_args(id, (2027, 2, 1)))
            .await
            .unwrap();
        assert_eq!(
            scalar(
                &pool,
                &format!("SELECT disposed FROM assets WHERE id = '{id}'")
            )
            .await,
            1
        );
    }
}

/// F3b (S3): Scheitert die Prüfung nach einer gespeicherten Umstellung, sagt
/// auch jeder weitere Versuch „nach der Umstellung“ (sonst schickte die
/// Meldung zum vorigen Image, das die neuere DB ablehnt). Erst nach einem
/// gelungenen Entsperren gilt wieder „nicht angefasst“.
#[tokio::test]
async fn integrity_message_after_migration_survives_retries() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (id, rule_id, title, body, severity)
             VALUES ('n1', 'rule_gibt_es_nicht', 'T', 'B', 'info')",
        )
        .execute(&mut *conn)
        .await
        .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .unwrap();
    }
    // Älteres Pflicht-Backup auf ein anderes Schema zählt nicht.
    klein_buch_lib::db::repo::audit_log::append(
        &pool,
        "backup.pre_migration",
        "backup",
        "b0",
        Some(serde_json::json!({ "from": 30, "to": 31, "backup_id": "b0" })),
    )
    .await
    .unwrap();
    assert_eq!(
        klein_buch_lib::db::unfinished_migration_from(&pool)
            .await
            .unwrap(),
        None
    );
    klein_buch_lib::db::repo::audit_log::append(
        &pool,
        "backup.pre_migration",
        "backup",
        "b1",
        Some(serde_json::json!({ "from": 31, "to": EXPECTED_SCHEMA_VERSION, "backup_id": "b1" })),
    )
    .await
    .unwrap();
    assert_eq!(
        klein_buch_lib::db::unfinished_migration_from(&pool)
            .await
            .unwrap(),
        Some(31)
    );
    for _ in 0..2 {
        let msg = klein_buch_lib::db::run_migrations(&ctx, &pool)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            msg.contains(klein_buch_lib::db::INTEGRITY_AFTER_MIGRATION)
                && msg.contains("Schema 31")
                && !msg.contains("nicht angefasst"),
            "{msg}"
        );
    }
    klein_buch_lib::db::repo::audit_log::append(
        &pool,
        "backup.unlock.ok",
        "backup",
        "passphrase",
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        klein_buch_lib::db::unfinished_migration_from(&pool)
            .await
            .unwrap(),
        None
    );
    let msg = klein_buch_lib::db::run_migrations(&ctx, &pool)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("nicht angefasst")
            && !msg.contains(klein_buch_lib::db::INTEGRITY_AFTER_MIGRATION),
        "{msg}"
    );
}

async fn mk_seller(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO seller_profile (id, name, street, postal_code, city, tax_number, email)
         VALUES (1, 'Verkäufer', 'Beispielweg 1', '84028', 'Landshut', '123/456/78901',
                 'verkaeufer@example.org')",
    )
    .execute(pool)
    .await
    .unwrap();
}

fn issue_args(id: &str) -> klein_buch_lib::commands::quotes::QuotesIssueArgs {
    klein_buch_lib::commands::quotes::QuotesIssueArgs { id: id.into() }
}

/// F3b (S3): `quotes_issue` frischt den Empfänger in der Transaktion des
/// Festschreibens auf und wartet auf die Sperre des Angebots. Wird der Entwurf
/// währenddessen zurückgezogen und der Kontakt anonymisiert, bekommt das
/// zurückgezogene Angebot die Personendaten nicht zurück; der Snapshot lässt
/// sich auch direkt nur auf Entwürfe schreiben.
#[tokio::test]
async fn quote_issue_refreshes_buyer_only_on_drafts() {
    let (pool, dir) = setup().await;
    let ctx = ctx_with(&pool, dir.path());
    mk_seller(&pool).await;
    mk_quote(&pool, "q1", "AN-2026-0001").await;
    mk_quote(&pool, "q2", "AN-2026-0002").await;
    sqlx::query("UPDATE contacts SET name = 'Kunde Beispiel Neu', city = 'Landshut' WHERE id = ?")
        .bind(CONTACT)
        .execute(&pool)
        .await
        .unwrap();

    let row = klein_buch_lib::commands::quotes::quotes_issue(&ctx, issue_args("q1"))
        .await
        .unwrap();
    assert_eq!(row.status, "sent");
    assert_eq!(row.buyer_name.as_deref(), Some("Kunde Beispiel Neu"));
    assert_eq!(row.buyer_city.as_deref(), Some("Landshut"));
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'quote.issue'"
        )
        .await,
        1
    );

    // q2: Festschreiben wartet auf die Sperre; derweil zurückgezogen und
    // Kontakt anonymisiert.
    let guard = klein_buch_lib::locks::QUOTES.lock("q2").await;
    let issuing = {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            klein_buch_lib::commands::quotes::quotes_issue(&ctx, issue_args("q2")).await
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    klein_buch_lib::db::repo::quotes::cancel(&pool, "q2", "Kunde hat abgesagt")
        .await
        .unwrap();
    klein_buch_lib::commands::contacts::anonymize_core(&pool, CONTACT)
        .await
        .unwrap();
    drop(guard);
    let err = issuing.await.unwrap().unwrap_err();
    assert!(err.to_string().contains("nicht im Entwurf"), "{err}");
    let placeholder = klein_buch_lib::domain::anonymize::anonymized_name(CONTACT);
    let (name, email, status): (Option<String>, Option<String>, String) =
        sqlx::query_as("SELECT buyer_name, buyer_email, status FROM quotes WHERE id = 'q2'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (name, email, status.as_str()),
        (Some(placeholder), None, "canceled")
    );

    let snapshot = klein_buch_lib::db::repo::invoices::BuyerSnapshot {
        name: "Kunde Beispiel",
        street: Some("Hauptstr. 1"),
        postal_code: None,
        city: None,
        country_code: "DE",
        vat_id: None,
        email: Some("kunde@example.org"),
    };
    let err = klein_buch_lib::db::repo::quotes::set_buyer_snapshot(&pool, "q2", &snapshot)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("nicht (mehr) im Entwurf"), "{err}");
}

/// F3b (S3): Die Anonymisierung eines Lieferanten ersetzt die Bezeichnung
/// seiner Kosten-Abos und die Buchungsvorlage, wenn sie den Namen nennt; andere
/// Abos bleiben unberührt. Das Audit nennt nur Zählwerte.
#[tokio::test]
async fn anonymize_scrubs_subscriptions_of_the_supplier() {
    let (pool, _dir) = setup().await;
    const VENDOR: &str = "01928300-0000-7000-8000-00000000c002";
    mk_contact(&pool, VENDOR, "Hans Müller").await;
    for (id, label, template, vendor) in [
        (
            "s1",
            "Miete Büro – Hans Müller",
            "Miete Büro MÜLLER",
            Some(VENDOR),
        ),
        ("s2", "Heizung Hans M.", "Heizungswartung", Some(VENDOR)),
        ("s3", "Hosting", "Hosting – Hans Müller", None),
    ] {
        sqlx::query(
            "INSERT INTO recurring_subscriptions (id, label, vendor_contact_id, frequency,
                    day_of_period, next_due_date, expected_amount_cents, category,
                    description_template, auto_create_expense, active)
             VALUES (?, ?, ?, 'monthly', 1, '2026-11-01', 50000, 'rent', ?, 1, 1)",
        )
        .bind(id)
        .bind(label)
        .bind(vendor)
        .bind(template)
        .execute(&pool)
        .await
        .unwrap();
    }
    klein_buch_lib::commands::contacts::anonymize_core(&pool, VENDOR)
        .await
        .unwrap();
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT id, label, description_template, active FROM recurring_subscriptions ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let placeholder = contacts::ANONYMIZED_SUBSCRIPTION_LABEL.to_string();
    assert_eq!(
        rows,
        vec![
            ("s1".into(), placeholder.clone(), placeholder.clone(), 0),
            (
                "s2".into(),
                placeholder.clone(),
                "Heizungswartung".into(),
                0
            ),
            (
                "s3".into(),
                "Hosting".into(),
                "Hosting – Hans Müller".into(),
                1
            ),
        ]
    );
    let details: String =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'contact.anonymize'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(details["scrubbedRecurringSubscriptions"], 2, "{details}");
    assert_eq!(details["scrubbedSubscriptionTemplates"], 1, "{details}");
    assert_eq!(details["pausedRecurringSubscriptions"], 2, "{details}");
    assert!(!details.to_string().contains("Müller"), "{details}");
}
