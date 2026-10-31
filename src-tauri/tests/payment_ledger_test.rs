//! Zahlungsbuch und „Zahlung stornieren“ (ADR 0042 Nr. 7–9, Lauf 2026.11.1,
//! Block V2, Migration 0034).
//!
//! * Migration 0034: Übernahme der JSON-Historie und des Altbestands ohne
//!   Historie mit festen IDs; Abbruch mit benannter Meldung bei nicht lesbarer
//!   Historie, Summenabweichung, fehlendem Zahldatum oder unbekanntem Konto —
//!   das Schema bleibt 33 (Vorabprüfung in Rust und CHECK in SQL).
//! * Trigger: Buch append-only, Gegenbuchung passend zur Zahlung, höchstens
//!   eine je Zahlung, keine auf eine Gegenbuchung, keine Buchung im
//!   abgeschlossenen Jahr oder auf Entwurf/Storno/stornierte Rechnung.
//! * Erfassen, Stornieren, neu Erfassen: Status, `paid_at`, Audit in derselben
//!   Transaktion; Ablehnungen (Jahr geschlossen mit Hinweis auf § 153 AO,
//!   Storno-Beleg, stornierte Rechnung, offener Storno-Entwurf, doppelt, ohne
//!   Grund).
//! * Auswertungen: EÜR (Einzelaufstellung mit Gegenbuchung, Jahr des
//!   Originals), ELSTER, DATEV, Gesamtumsatz, Storno mit Erstattung nach einer
//!   Gegenbuchung, Storno-Datum ohne stornierte Paare.
//!
//! Testdaten nur mit Dokumentations-Adressen (example.org).

#![allow(clippy::type_complexity)]

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::NaiveDate;
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::euer as euer_cmd;
use klein_buch_lib::commands::invoices as invoices_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{
    euer as euer_repo, invoice_payments, invoices, seller_profile, tax_status,
};
use klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::euer::aggregate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

const CONTACT: &str = "01928300-0000-7000-8000-00000000c001";
const ACCOUNT: &str = "01928300-0000-7000-8000-00000000a001";
const REASON: &str = "Zahlung irrtümlich doppelt erfasst";

struct Env {
    pool: SqlitePool,
    ctx: Arc<AppCtx>,
    _dir: tempfile::TempDir,
}

fn paths_for(dir: &Path) -> Paths {
    let data = dir.join("data");
    for d in ["archive", "backups", "tmp"] {
        std::fs::create_dir_all(data.join(d)).unwrap();
    }
    Paths {
        data_dir: data.clone(),
        db_file: data.join("klein-buch.sqlite"),
        archive_dir: data.join("archive"),
        backups_dir: data.join("backups"),
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
        resource_dir: std::path::PathBuf::from("/non/existent/resources"),
        web_dir: std::path::PathBuf::from("/non/existent/web"),
        backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
    }
}

async fn open(db: &Path) -> SqlitePool {
    let url = format!("sqlite://{}", db.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap()
}

/// Migrator ohne 0034 (DB auf Schema 33).
fn migrator_until_33() -> sqlx::migrate::Migrator {
    let all = &MIGRATOR.migrations;
    let idx = all
        .iter()
        .position(|m| m.description.contains("invoice payment ledger"))
        .expect("Migration 0034");
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(all[..idx].to_vec()),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

async fn base_data(pool: &SqlitePool) {
    for sql in [
        "INSERT INTO contacts (id, contact_type, name, email, country_code)
         VALUES ('01928300-0000-7000-8000-00000000c001', 'customer', 'Kunde Beispiel',
                 'kunde@example.org', 'DE')",
        "INSERT INTO payment_accounts (id, label, \"type\", is_default, active)
         VALUES ('01928300-0000-7000-8000-00000000a001', 'Kasse', 'cash', 0, 1)",
    ] {
        sqlx::query(sql).execute(pool).await.unwrap();
    }
}

async fn setup() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    let pool = open(&paths.db_file).await;
    MIGRATOR.run(&pool).await.unwrap();
    base_data(&pool).await;
    seller_profile::upsert(
        &pool,
        &seller_profile::SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "kontakt@example.org".into(),
            phone: None,
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: None,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
        },
    )
    .await
    .unwrap();
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    Env {
        pool,
        ctx,
        _dir: dir,
    }
}

/// Festgeschriebene Ausgangsrechnung direkt in der DB (der Festschreibe-Weg
/// mit PDF/KoSIT ist hier nicht Gegenstand). Mit `regel` eine Position zu
/// 19 % (Regelbesteuerung), sonst §19.
async fn insert_locked(pool: &SqlitePool, id: &str, number: &str, gross: i64, regel: bool) {
    insert_locked_on(pool, id, number, gross, regel, "2026-02-01", "2026-02-15").await;
}

/// Wie [`insert_locked`] mit Rechnungs- und Fälligkeitsdatum (Geschäftsjahr
/// aus dem Rechnungsdatum).
async fn insert_locked_on(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    gross: i64,
    regel: bool,
    invoice_date: &str,
    due_date: &str,
) {
    let (net, tax) = if regel {
        let net = gross * 100 / 119;
        (net, gross - net)
    } else {
        (gross, 0)
    };
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, tax_amount_cents, gross_amount_cents, is_kleinunternehmer,
            status, due_date, buyer_name, buyer_country_code
         ) VALUES (?, ?, CAST(substr(?, 1, 4) AS INTEGER), 'issued', ?, ?,
            'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt',
            ?, ?, ?, ?, 'draft', ?, 'Kunde Beispiel', 'DE')",
    )
    .bind(id)
    .bind(number)
    .bind(invoice_date)
    .bind(invoice_date)
    .bind(CONTACT)
    .bind(net)
    .bind(tax)
    .bind(gross)
    .bind(i64::from(!regel))
    .bind(due_date)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invoice_items
            (id, invoice_id, position, description, quantity, unit_code,
             unit_price_cents, net_amount_cents, tax_rate_percent, tax_category_code)
         VALUES (?, ?, 1, 'Beratung', 1.0, 'HUR', ?, ?, ?, ?)",
    )
    .bind(format!("{id}-1"))
    .bind(id)
    .bind(net)
    .bind(net)
    .bind(if regel { 19.0 } else { 0.0 })
    .bind(if regel { "S" } else { "E" })
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET status = 'issued', locked_at = '2026-02-01 10:00:00' WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

async fn pay(pool: &SqlitePool, id: &str, cents: i64, date: &str, account: Option<&str>) {
    invoices::record_payment_to_account(pool, id, cents, date, None, account)
        .await
        .unwrap();
}

async fn payment_ids(pool: &SqlitePool, invoice: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM invoice_payments WHERE invoice_id = ? ORDER BY seq")
        .bind(invoice)
        .fetch_all(pool)
        .await
        .unwrap()
}

fn reverse_args(
    invoice: &str,
    payment: &str,
    reason: &str,
) -> invoices_cmd::InvoicesReversePaymentArgs {
    serde_json::from_value(serde_json::json!({
        "invoiceId": invoice, "paymentId": payment, "reason": reason
    }))
    .unwrap()
}

async fn reverse(
    pool: &SqlitePool,
    invoice: &str,
    payment: &str,
    reason: &str,
) -> klein_buch_lib::error::Result<klein_buch_lib::db::models::InvoiceRow> {
    invoices_cmd::reverse_payment_core(
        pool,
        &reverse_args(invoice, payment, reason),
        chrono::Utc::now(),
    )
    .await
}

fn assert_blocked<T: std::fmt::Debug>(res: sqlx::Result<T>, needle: &str) {
    let err = res.expect_err("muss abbrechen");
    let msg = err.to_string();
    assert!(msg.contains(needle), "erwartet „{needle}“, war: {msg}");
}

async fn close_year(pool: &SqlitePool, year: i64) {
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
                                        afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (?, 0, 0, 0, 0, 'test', 34)",
    )
    .bind(year)
    .execute(pool)
    .await
    .unwrap();
}

// ============================================================================
// Migration 0034
// ============================================================================

async fn old_db() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(&dir.path().join("klein-buch.sqlite")).await;
    migrator_until_33().run(&pool).await.unwrap();
    base_data(&pool).await;
    (pool, dir)
}

/// Rechnung im Stand vor 0034: Zahlungsfelder und JSON-Historie direkt.
async fn insert_old(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    paid: i64,
    paid_at: Option<&str>,
    status: &str,
    history: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, locked_at,
            paid_amount_cents, paid_at, payment_history_json
         ) VALUES (?, ?, 2026, 'issued', '2026-02-01', ?,
            'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt',
            10000, 10000, ?, '2026-02-01 10:00:00', ?, ?, ?)",
    )
    .bind(id)
    .bind(number)
    .bind(CONTACT)
    .bind(status)
    .bind(paid)
    .bind(paid_at)
    .bind(history)
    .execute(pool)
    .await
    .unwrap();
}

async fn schema_version(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn migration_0034_moves_history_and_legacy_into_the_ledger() {
    let (pool, _dir) = old_db().await;
    let history = format!(
        "[{{\"amountCents\":3000,\"paidDate\":\"2026-03-10\",\"note\":\"Anzahlung\",\
          \"recordedAt\":\"2026-03-10T08:00:00.123+00:00\",\"accountId\":\"{ACCOUNT}\"}},\
          {{\"amountCents\":0,\"paidDate\":\"2026-03-11\",\"recordedAt\":\"x\"}},\
          {{\"amountCents\":7000,\"paidDate\":\"2026-03-02\",\"recordedAt\":\"2026-03-12T08:00:00+00:00\"}}]"
    );
    insert_old(
        &pool,
        "b-hist",
        "RE-2026-0001",
        10_000,
        Some("2026-03-02"),
        "paid",
        Some(&history),
    )
    .await;
    insert_old(
        &pool,
        "a-legacy",
        "RE-2026-0002",
        4_000,
        Some("2026-04-01 00:00:00"),
        "partially_paid",
        None,
    )
    .await;
    insert_old(
        &pool,
        "c-empty",
        "RE-2026-0003",
        2_500,
        Some("2026-05-05"),
        "partially_paid",
        Some("[]"),
    )
    .await;
    insert_old(&pool, "d-none", "RE-2026-0004", 0, None, "sent", None).await;
    klein_buch_lib::db::precheck_before_migration(&pool, 33)
        .await
        .expect("übernehmbar");

    MIGRATOR.run(&pool).await.unwrap();
    assert_eq!(EXPECTED_SCHEMA_VERSION, 34);
    assert_eq!(schema_version(&pool).await, "34");

    let rows: Vec<(
        String,
        String,
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, invoice_id, seq, amount_cents, paid_date, account_id, note, created_at
               FROM invoice_payments ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "00000000-0000-7034-8000-000000000001",
            "00000000-0000-7034-8000-000000000002",
            "00000000-0000-7034-8000-000000000003",
            "00000000-0000-7034-8000-000000000004",
        ],
        "feste IDs, laufend nach Rechnungs-ID und Reihenfolge"
    );
    // a-legacy: Altbestand ohne Historie, Datum ohne Uhrzeit.
    assert_eq!(rows[0].1, "a-legacy");
    assert_eq!(
        (rows[0].2, rows[0].3, rows[0].4.as_str()),
        (1, 4_000, "2026-04-01")
    );
    assert!(rows[0].6.as_deref().unwrap().contains("Altbestand"));
    // b-hist: zwei Einträge (der 0-Eintrag entfällt), Reihenfolge der Historie,
    // Konto und Erfassungszeit übernommen.
    assert_eq!(
        (rows[1].1.as_str(), rows[1].2, rows[1].3),
        ("b-hist", 1, 3_000)
    );
    assert_eq!(rows[1].5.as_deref(), Some(ACCOUNT));
    assert_eq!(rows[1].7, "2026-03-10 08:00:00");
    assert_eq!(
        (rows[2].1.as_str(), rows[2].2, rows[2].3),
        ("b-hist", 2, 7_000)
    );
    // c-empty: leere Historie wie Altbestand.
    assert_eq!((rows[3].1.as_str(), rows[3].3), ("c-empty", 2_500));
    // Summe = Cache, je Rechnung ein Audit.
    let mismatch: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices i
          WHERE i.paid_amount_cents IS NOT
                (SELECT COALESCE(SUM(amount_cents), 0) FROM invoice_payments p
                  WHERE p.invoice_id = i.id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mismatch, 0);
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.payments_migrated'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 3);
    // Historie bleibt unverändert stehen.
    let kept: Option<String> =
        sqlx::query_scalar("SELECT payment_history_json FROM invoices WHERE id = 'b-hist'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kept.as_deref(), Some(history.as_str()));
    // paid_at aus dem Buch (ADR 0042 Nr. 7): jüngstes Zahldatum statt Datum
    // der zuletzt erfassten Zahlung, ohne Uhrzeit; ohne Zahlung unverändert.
    let paid_at: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT id, paid_at FROM invoices ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        paid_at,
        vec![
            ("a-legacy".to_string(), Some("2026-04-01".to_string())),
            ("b-hist".to_string(), Some("2026-03-10".to_string())),
            ("c-empty".to_string(), Some("2026-05-05".to_string())),
            ("d-none".to_string(), None),
        ]
    );
    let before: String = sqlx::query_scalar(
        "SELECT json_extract(details_json, '$.paid_at_before') || ' → '
                || json_extract(details_json, '$.paid_at')
           FROM audit_log
          WHERE action = 'invoice.payments_migrated' AND entity_id = 'b-hist'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, "2026-03-02 → 2026-03-10");
    // EÜR wie vorher: 3000 + 7000 + 4000 + 2500.
    let payments = euer_repo::income_payments(&pool).await.unwrap();
    assert_eq!(payments.iter().map(|p| p.amount_cents).sum::<i64>(), 16_500);
}

#[tokio::test]
async fn migration_0034_aborts_on_inconsistent_history_and_keeps_schema_33() {
    let cases: [(&str, i64, Option<&str>, Option<&str>, &str); 5] = [
        (
            "Summe",
            10_000,
            Some("2026-03-01"),
            Some("[{\"amountCents\":7000,\"paidDate\":\"2026-03-01\",\"recordedAt\":\"x\"}]"),
            "ergibt die Zahlungshistorie nicht den gezahlten Betrag",
        ),
        (
            "kaputt",
            10_000,
            Some("2026-03-01"),
            Some("{kaputt"),
            "nicht lesbar",
        ),
        (
            "Datum",
            10_000,
            Some("2026-03-01"),
            Some("[{\"amountCents\":10000,\"paidDate\":\"01.03.2026\",\"recordedAt\":\"x\"}]"),
            "nicht lesbar",
        ),
        (
            "ohne Datum",
            10_000,
            None,
            None,
            "weder Zahlungshistorie noch Zahldatum",
        ),
        (
            "Konto",
            10_000,
            Some("2026-03-01"),
            Some(
                "[{\"amountCents\":10000,\"paidDate\":\"2026-03-01\",\"recordedAt\":\"x\",\
                  \"accountId\":\"gibt-es-nicht\"}]",
            ),
            "unbekanntes Zahlungskonto",
        ),
    ];
    for (label, paid, paid_at, history, needle) in cases {
        let (pool, _dir) = old_db().await;
        insert_old(&pool, "x", "RE-2026-0009", paid, paid_at, "paid", history).await;
        insert_old(&pool, "y", "RE-2026-0010", 0, None, "sent", None).await;
        // Fürs Log: genau die betroffene Rechnung, ohne Beträge.
        assert_eq!(
            klein_buch_lib::db::payment_ledger_invoices(&pool)
                .await
                .unwrap(),
            vec!["RE-2026-0009".to_string()],
            "{label}"
        );
        // Erste Linie: Rust-Vorabprüfung vor dem Pflicht-Backup.
        let err = klein_buch_lib::db::precheck_before_migration(&pool, 33)
            .await
            .expect_err(label)
            .to_string();
        assert!(
            err.contains("Migration 0034 abgebrochen") && err.contains(needle),
            "{label}: {err}"
        );
        assert!(err.contains("Schema 33"), "{label}: {err}");
        // Zweite Linie: benannter CHECK in der Migration, die zurückrollt.
        let err = MIGRATOR.run(&pool).await.expect_err(label).to_string();
        assert!(err.contains("Migration 0034 abgebrochen"), "{label}: {err}");
        assert_eq!(schema_version(&pool).await, "33", "{label}: zurückgerollt");
        let table: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'invoice_payments'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(table, 0, "{label}");
    }
}

// ============================================================================
// Trigger
// ============================================================================

#[allow(clippy::too_many_arguments)]
async fn raw_insert(
    pool: &SqlitePool,
    id: &str,
    invoice: &str,
    seq: i64,
    kind: &str,
    amount: i64,
    date: &str,
    account: Option<&str>,
    reverses: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO invoice_payments
            (id, invoice_id, seq, kind, amount_cents, paid_date, account_id,
             reverses_payment_id, reason, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, '2026-03-01 10:00:00')",
    )
    .bind(id)
    .bind(invoice)
    .bind(seq)
    .bind(kind)
    .bind(amount)
    .bind(date)
    .bind(account)
    .bind(reverses)
    .bind(reverses.map(|_| REASON))
    .execute(pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn ledger_is_append_only_and_reversals_must_match() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 6_000, "2026-03-01", Some(ACCOUNT)).await;
    let p = payment_ids(pool, "inv").await.remove(0);

    assert_blocked(
        sqlx::query("UPDATE invoice_payments SET amount_cents = 1")
            .execute(pool)
            .await,
        "append-only",
    );
    assert_blocked(
        sqlx::query("DELETE FROM invoice_payments")
            .execute(pool)
            .await,
        "append-only",
    );
    // Falscher Betrag, falsches Datum, falsches Konto.
    for (amount, date, account) in [
        (-5_000, "2026-03-01", Some(ACCOUNT)),
        (-6_000, "2026-03-02", Some(ACCOUNT)),
        (-6_000, "2026-03-01", None),
    ] {
        assert_blocked(
            raw_insert(
                pool,
                "r-x",
                "inv",
                2,
                "reversal",
                amount,
                date,
                account,
                Some(&p),
            )
            .await,
            "Gegenbuchung passt nicht",
        );
    }
    // Ohne Grund bzw. ohne Verweis: CHECK.
    assert_blocked(
        sqlx::query(
            "INSERT INTO invoice_payments
                (id, invoice_id, seq, kind, amount_cents, paid_date, account_id,
                 reverses_payment_id, created_at)
             VALUES ('r-y', 'inv', 2, 'reversal', -6000, '2026-03-01', ?, ?, 'x')",
        )
        .bind(ACCOUNT)
        .bind(&p)
        .execute(pool)
        .await,
        "CHECK",
    );
    // Passend: geht; eine zweite Gegenbuchung nicht; keine auf die Gegenbuchung.
    raw_insert(
        pool,
        "r-1",
        "inv",
        2,
        "reversal",
        -6_000,
        "2026-03-01",
        Some(ACCOUNT),
        Some(&p),
    )
    .await
    .unwrap();
    assert_blocked(
        raw_insert(
            pool,
            "r-2",
            "inv",
            3,
            "reversal",
            -6_000,
            "2026-03-01",
            Some(ACCOUNT),
            Some(&p),
        )
        .await,
        "bereits storniert",
    );
    assert_blocked(
        raw_insert(
            pool,
            "r-3",
            "inv",
            3,
            "reversal",
            6_000,
            "2026-03-01",
            Some(ACCOUNT),
            Some("r-1"),
        )
        .await,
        "passt nicht",
    );
    // Lücke in der laufenden Nummer, Überzahlung.
    assert_blocked(
        raw_insert(
            pool,
            "p-9",
            "inv",
            9,
            "payment",
            100,
            "2026-03-01",
            None,
            None,
        )
        .await,
        "laufende Nummer",
    );
    assert_blocked(
        raw_insert(
            pool,
            "p-3",
            "inv",
            3,
            "payment",
            10_001,
            "2026-03-01",
            None,
            None,
        )
        .await,
        "Überzahlung",
    );
    // Cache und eingefrorene Historie.
    assert_blocked(
        sqlx::query(
            "UPDATE invoices SET paid_amount_cents = 5000, status = 'partially_paid' WHERE id = 'inv'",
        )
        .execute(pool)
        .await,
        "Zahlungsbuch",
    );
    assert_blocked(
        sqlx::query("UPDATE invoices SET payment_history_json = '[]' WHERE id = 'inv'")
            .execute(pool)
            .await,
        "Altbestand",
    );
}

/// V2-02 (ADR 0042 Nr. 7): Der Status ist an den Endzustand des Buchs
/// gebunden, auch wenn sich nur die Summe ändert; 'draft' nur ohne
/// Festschreibung und ohne Zahlung.
#[tokio::test]
async fn cache_status_is_bound_to_the_ledger_end_state() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 10_000, "2026-03-01", None).await;
    let p = payment_ids(pool, "inv").await.remove(0);
    // Bezahlt und festgeschrieben: zurück auf 'draft' geht nicht.
    assert_blocked(
        sqlx::query("UPDATE invoices SET status = 'draft' WHERE id = 'inv'")
            .execute(pool)
            .await,
        "Zahlungsbuch",
    );
    // Roh-Gegenbuchung ohne Cache-Pflege (wie eine Manipulation außerhalb der App).
    raw_insert(
        pool,
        "r-1",
        "inv",
        2,
        "reversal",
        -10_000,
        "2026-03-01",
        None,
        Some(&p),
    )
    .await
    .unwrap();
    // Nur die Summe nachziehen, Status 'paid' stehen lassen: abgelehnt.
    assert_blocked(
        sqlx::query("UPDATE invoices SET paid_amount_cents = 0, paid_at = NULL WHERE id = 'inv'")
            .execute(pool)
            .await,
        "Zahlungsbuch",
    );
    // 'draft' auf eine festgeschriebene Rechnung auch ohne Zahlung nicht.
    assert_blocked(
        sqlx::query(
            "UPDATE invoices SET paid_amount_cents = 0, paid_at = NULL, status = 'draft'
              WHERE id = 'inv'",
        )
        .execute(pool)
        .await,
        "Zahlungsbuch",
    );
    // Summe, Datum und Status passend zum Buch: geht.
    sqlx::query(
        "UPDATE invoices SET paid_amount_cents = 0, paid_at = NULL, status = 'issued'
          WHERE id = 'inv'",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// V2-03 (ADR 0042 Nr. 7–8): Die Erstattung im Storno-Jahr kommt aus dem
/// Buch, nicht aus dem Cache — auch wenn eine Roh-Gegenbuchung den Cache
/// stehen ließ, laufen Zahlungs- und Storno-Jahr nicht auseinander.
#[tokio::test]
async fn euer_refund_reads_the_ledger_not_the_cache() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 10_000, "2026-03-01", None).await;
    let p = payment_ids(pool, "inv").await.remove(0);
    raw_insert(
        pool,
        "r-1",
        "inv",
        2,
        "reversal",
        -10_000,
        "2026-03-01",
        None,
        Some(&p),
    )
    .await
    .unwrap();
    // Storno-Beleg festgeschrieben (wie die Pipeline), Cache des Originals alt.
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, is_storno_for
         ) VALUES ('st', 'ST-2026-0001', 2026, 'issued', '2026-03-20', ?,
            'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt',
            -10000, -10000, 'draft', 'inv')",
    )
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET status = 'issued', locked_at = '2026-03-20 10:00:00' WHERE id = 'st'",
    )
    .execute(pool)
    .await
    .unwrap();
    let cache: i64 = sqlx::query_scalar("SELECT paid_amount_cents FROM invoices WHERE id = 'inv'")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(cache, 10_000, "Cache bewusst veraltet");
    let inputs = euer_repo::load_inputs(pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    assert_eq!(
        (report.invoice_income_cents, report.storno_refunds_cents),
        (0, 0)
    );
    assert!(euer_repo::storno_detail(pool, 2026)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn ledger_rejects_closed_year_drafts_stornos_and_canceled() {
    let env = setup().await;
    let pool = &env.pool;
    // Abgeschlossenes Jahr des Zahldatums.
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    close_year(pool, 2025).await;
    assert_blocked(
        raw_insert(
            pool,
            "p-1",
            "inv",
            1,
            "payment",
            100,
            "2025-12-31",
            None,
            None,
        )
        .await,
        "abgeschlossen",
    );
    // Entwurf (nicht festgeschrieben).
    sqlx::query(
        "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents)
         VALUES ('draft', 'RE-2026-0002', 2026, 'issued', '2026-02-01', ?,
            'V', 'W', '12345', 'O', 10000, 10000)",
    )
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
    assert_blocked(
        raw_insert(
            pool,
            "p-2",
            "draft",
            1,
            "payment",
            100,
            "2026-03-01",
            None,
            None,
        )
        .await,
        "festgeschriebene",
    );
    // Storno-Beleg und storniertes Original.
    sqlx::query(
        "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, locked_at, is_storno_for)
         VALUES ('st', 'ST-2026-0001', 2026, 'issued', '2026-03-01', ?,
            'V', 'W', '12345', 'O', -10000, -10000, 'issued', '2026-03-01 10:00:00', 'inv')",
    )
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
    assert_blocked(
        raw_insert(
            pool,
            "p-3",
            "st",
            1,
            "payment",
            100,
            "2026-03-01",
            None,
            None,
        )
        .await,
        "festgeschriebene",
    );
    assert_blocked(
        raw_insert(
            pool,
            "p-4",
            "inv",
            1,
            "payment",
            100,
            "2026-03-01",
            None,
            None,
        )
        .await,
        "festgeschriebene",
    );
    // Neue Rechnung mit Zahlung beim Anlegen.
    assert_blocked(
        sqlx::query(
            "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date,
                contact_id, seller_name, seller_street, seller_postal_code, seller_city,
                net_amount_cents, gross_amount_cents, paid_amount_cents)
             VALUES ('neu', 'RE-2026-0003', 2026, 'issued', '2026-02-01', ?,
                'V', 'W', '12345', 'O', 10000, 10000, 10000)",
        )
        .bind(CONTACT)
        .execute(pool)
        .await,
        "invoice_payments",
    );
}

// ============================================================================
// Erfassen, Stornieren, neu Erfassen
// ============================================================================

#[tokio::test]
async fn record_reverse_and_record_again() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    sqlx::query(
        "UPDATE invoices SET sent_at = '2026-02-02 09:00:00', status = 'sent' WHERE id = 'inv'",
    )
    .execute(pool)
    .await
    .unwrap();
    // Teilzahlung, dann Rest mit falschem (spätem) Datum.
    pay(pool, "inv", 4_000, "2026-03-01", Some(ACCOUNT)).await;
    pay(pool, "inv", 6_000, "2026-06-30", None).await;
    let row = invoices::get(pool, "inv").await.unwrap().unwrap();
    assert_eq!(
        (row.status.as_str(), row.paid_at.as_deref()),
        ("paid", Some("2026-06-30"))
    );
    let ids = payment_ids(pool, "inv").await;
    let audit: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.payment_recorded'
            AND entity_id = 'inv' AND json_extract(details_json, '$.payment_id') IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audit, 2, "Audit je Zahlung in der Transaktion");

    // Ohne ausreichenden Grund.
    let err = reverse(pool, "inv", &ids[1], "zu kurz").await.unwrap_err();
    assert!(err.to_string().contains("mindestens 10 Zeichen"), "{err}");
    // Zahlung einer anderen Rechnung.
    insert_locked(pool, "other", "RE-2026-0002", 10_000, false).await;
    let err = reverse(pool, "other", &ids[1], REASON).await.unwrap_err();
    assert!(
        err.to_string().contains("gibt es zu dieser Rechnung nicht"),
        "{err}"
    );

    // Storno der späten Zahlung: zurück auf teilbezahlt, paid_at = 01.03.
    let row = reverse(pool, "inv", &ids[1], REASON).await.unwrap();
    assert_eq!(row.paid_amount_cents, 4_000);
    assert_eq!(row.status, "partially_paid");
    assert_eq!(row.paid_at.as_deref(), Some("2026-03-01"));
    let (kind, amount, date, reverses, reason): (String, i64, String, String, String) =
        sqlx::query_as(
            "SELECT kind, amount_cents, paid_date, reverses_payment_id, reason
               FROM invoice_payments WHERE invoice_id = 'inv' AND seq = 3",
        )
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        (
            kind.as_str(),
            amount,
            date.as_str(),
            reverses.as_str(),
            reason.as_str()
        ),
        ("reversal", -6_000, "2026-06-30", ids[1].as_str(), REASON)
    );
    let details: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'invoice.payment_reversed'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        details.contains(&ids[1]) && details.contains(REASON),
        "{details}"
    );
    // Zweimal stornieren geht nicht.
    let err = reverse(pool, "inv", &ids[1], REASON).await.unwrap_err();
    assert!(err.to_string().contains("bereits storniert"), "{err}");

    // Lesemodell: Verweise und Stornierbarkeit.
    let detail = invoices::get_detail(pool, "inv").await.unwrap().unwrap();
    assert_eq!(detail.payments.len(), 3);
    assert!(detail.payments[0].reversible);
    assert_eq!(detail.payments[0].row.account_id.as_deref(), Some(ACCOUNT));
    assert_eq!(
        detail.payments[1].reversed_by_id.as_deref(),
        Some(detail.payments[2].row.id.as_str())
    );
    assert!(!detail.payments[1].reversible && !detail.payments[2].reversible);

    // Auch die erste Zahlung storniert: ohne Zahlung zurück auf „versendet“.
    let row = reverse(pool, "inv", &ids[0], REASON).await.unwrap();
    assert_eq!((row.paid_amount_cents, row.status.as_str()), (0, "sent"));
    assert_eq!(row.paid_at, None);
    // Gegenbuchung mit Konto des Originals.
    let acc: Option<String> =
        sqlx::query_scalar("SELECT account_id FROM invoice_payments WHERE reverses_payment_id = ?")
            .bind(&ids[0])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(acc.as_deref(), Some(ACCOUNT));

    // Richtig neu buchen über das Erfassen.
    pay(pool, "inv", 10_000, "2026-03-02", None).await;
    let row = invoices::get(pool, "inv").await.unwrap().unwrap();
    assert_eq!(
        (row.paid_amount_cents, row.status.as_str()),
        (10_000, "paid")
    );
    assert_eq!(row.paid_at.as_deref(), Some("2026-03-02"));

    // Ohne Versand: zurück auf „ausgestellt“.
    insert_locked(pool, "iss", "RE-2026-0003", 5_000, false).await;
    pay(pool, "iss", 5_000, "2026-03-01", None).await;
    let p = payment_ids(pool, "iss").await.remove(0);
    let row = reverse(pool, "iss", &p, REASON).await.unwrap();
    assert_eq!(row.status, "issued");

    // DSGVO-Auskunft (Art. 15): Zahlungen und Gegenbuchungen je Rechnung.
    let gathered = klein_buch_lib::db::repo::dsgvo::gather(pool, CONTACT)
        .await
        .unwrap()
        .unwrap();
    let kinds: Vec<&str> = gathered.invoice_payments["inv"]
        .iter()
        .map(|p| p.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        vec!["payment", "payment", "reversal", "reversal", "payment"]
    );
}

#[tokio::test]
async fn reversal_refused_in_closed_year_with_hint_on_153_ao() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 3_000, "2025-12-30", None).await;
    let p = payment_ids(pool, "inv").await.remove(0);
    close_year(pool, 2025).await;
    let err = reverse(pool, "inv", &p, REASON)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("2025 ist abgeschlossen") && err.contains("§ 153 AO"),
        "{err}"
    );
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM invoice_payments WHERE kind = 'reversal'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(n, 0);
    let detail = invoices::get_detail(pool, "inv").await.unwrap().unwrap();
    assert!(!detail.payments[0].reversible);
    assert!(detail.payments[0]
        .not_reversible_reason
        .as_deref()
        .unwrap()
        .contains("§ 153 AO"));
}

#[tokio::test]
async fn reversal_refused_for_storno_canceled_and_open_storno_draft() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 10_000, "2026-03-01", None).await;
    let p = payment_ids(pool, "inv").await.remove(0);

    // Offener Storno-Entwurf.
    sqlx::query(
        "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, is_storno_for)
         VALUES ('st', 'ST-2026-0001', 2026, 'issued', '2026-03-05', ?,
            'V', 'W', '12345', 'O', -10000, -10000, 'inv')",
    )
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
    let err = reverse(pool, "inv", &p, REASON).await.unwrap_err();
    assert!(err.to_string().contains("offenen Storno-Entwurf"), "{err}");
    // Die DB lehnt es als zweite Linie ebenso ab.
    assert_blocked(
        raw_insert(
            pool,
            "r-1",
            "inv",
            2,
            "reversal",
            -10_000,
            "2026-03-01",
            None,
            Some(&p),
        )
        .await,
        "offenen Storno-Entwurf",
    );

    // Storno festgeschrieben, Original storniert.
    sqlx::query(
        "UPDATE invoices SET locked_at = '2026-03-05 10:00:00', status = 'issued' WHERE id = 'st'",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET status = 'canceled', canceled_at = '2026-03-05 10:00:00',
                canceled_by_storno_id = 'st' WHERE id = 'inv'",
    )
    .execute(pool)
    .await
    .unwrap();
    let err = reverse(pool, "inv", &p, REASON).await.unwrap_err();
    assert!(err.to_string().contains("storniert"), "{err}");
    // Auf einen Storno-Beleg gibt es keine Zahlung; Stornieren einer fremden
    // ID dort scheitert ebenfalls.
    let err = reverse(pool, "st", &p, REASON).await.unwrap_err();
    assert!(
        err.to_string().contains("gibt es zu dieser Rechnung nicht"),
        "{err}"
    );
    let err = invoices::record_payment(pool, "st", 100, "2026-03-06", None)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("storniert") || err.to_string().contains("Storno-Beleg"),
        "{err}"
    );
}

/// Ohne Re-Auth kommt der Command über den Server nicht durch (die Matrix in
/// `server_auth_test` prüft alle Re-Auth-Commands); hier die Einstufung.
#[test]
fn reverse_payment_is_a_reauth_write_command() {
    use klein_buch_lib::commands::registry::{find, Auth, Kind};
    let spec = find("invoices_reverse_payment").expect("registriert");
    assert_eq!(spec.auth, Auth::Reauth);
    assert_eq!(spec.kind, Kind::Write);
}

// ============================================================================
// Auswertungen
// ============================================================================

#[tokio::test]
async fn euer_elster_datev_and_turnover_with_reversal() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "klein", "RE-2026-0001", 10_000, false).await;
    insert_locked(pool, "regel", "RE-2026-0002", 11_900, true).await;
    // §19: Teilzahlung 4000, dann 6000 (falsch, wird storniert).
    pay(pool, "klein", 4_000, "2026-03-01", None).await;
    pay(pool, "klein", 6_000, "2026-03-15", Some(ACCOUNT)).await;
    // Regelbesteuerung: 11.900 voll, storniert, neu 5.950.
    pay(pool, "regel", 11_900, "2026-04-01", None).await;
    let k = payment_ids(pool, "klein").await;
    let r = payment_ids(pool, "regel").await;
    let before = {
        let inputs = euer_repo::load_inputs(pool).await.unwrap();
        aggregate::aggregate(2026, &inputs).invoice_income_cents
    };
    assert_eq!(before, 21_900);
    reverse(pool, "klein", &k[1], REASON).await.unwrap();
    reverse(pool, "regel", &r[0], REASON).await.unwrap();
    pay(pool, "regel", 5_950, "2026-04-03", None).await;

    // EÜR: Paare heben sich auf.
    let inputs = euer_repo::load_inputs(pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    assert_eq!(report.invoice_income_cents, 4_000 + 5_950);

    // Einzelaufstellung: alle Buchungen sichtbar, Gegenbuchung gekennzeichnet.
    let detail = euer_repo::income_detail(pool, 2026).await.unwrap();
    assert_eq!(detail.len(), 6);
    let rev: Vec<_> = detail.iter().filter(|i| i.is_reversal).collect();
    assert_eq!(rev.len(), 2);
    let rk = rev
        .iter()
        .find(|i| i.invoice_number == "RE-2026-0001")
        .unwrap();
    assert_eq!(
        (rk.amount_cents, rk.paid_date.as_str()),
        (-6_000, "2026-03-15")
    );
    assert!(
        rk.description
            .starts_with("Storno der Zahlung vom 15.03.2026: "),
        "{}",
        rk.description
    );
    assert_eq!(
        rk.account_type.as_deref(),
        Some("cash"),
        "Konto des Originals"
    );
    assert_eq!(detail.iter().filter(|i| i.reversed).count(), 2);
    let rr = rev
        .iter()
        .find(|i| i.invoice_number == "RE-2026-0002")
        .unwrap();
    let orig = detail
        .iter()
        .find(|i| i.invoice_number == "RE-2026-0002" && i.reversed)
        .unwrap();
    for (a, b) in rr.shares.iter().zip(&orig.shares) {
        assert_eq!(
            (a.net_cents, a.tax_cents),
            (-b.net_cents, -b.tax_cents),
            "Anteile exakt negiert"
        );
    }
    assert_eq!(detail.iter().map(|i| i.amount_cents).sum::<i64>(), 9_950);

    // ELSTER-Ausfüllhilfe und DATEV über die Commands.
    let package = euer_cmd::euer_package(
        &env.ctx,
        serde_json::from_value(serde_json::json!({ "fiscalYear": 2026 })).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(package.form.income_total_cents, 4_000 + 5_000 + 950);
    let datev = euer_cmd::euer_export_datev(
        &env.ctx,
        serde_json::from_value(serde_json::json!({
            "fiscalYear": 2026, "skr": "SKR03", "consultantNumber": 1001, "clientNumber": 1
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    // §19: 3 Zeilen (Zahlung, Zahlung, Gegenbuchung); Regel: je Zahlung bzw.
    // Gegenbuchung eine Zeile auf das Automatikkonto (19 %) = 3.
    assert_eq!(datev.booking_count, 6);

    // Gesamtumsatz nach § 19 Abs. 2: ohne die stornierten Paare.
    let posten = tax_status::turnover_items(pool, &[2026]).await.unwrap();
    assert_eq!(posten.len(), 2);
    assert!(posten.iter().all(|p| p.net_cents > 0));
}

/// V2-09 (ADR 0042 Nr. 8): Zahlung im Vorjahr (noch offen), storniert im
/// laufenden Jahr — die Gegenbuchung gehört ins Jahr der Zahlung. EÜR, ELSTER
/// und DATEV des Vorjahres sinken, das laufende Jahr bleibt unverändert.
#[tokio::test]
async fn reversal_of_a_prior_year_payment_lands_in_that_year() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked_on(
        pool,
        "old",
        "RE-2025-0001",
        10_000,
        false,
        "2025-12-01",
        "2025-12-15",
    )
    .await;
    pay(pool, "old", 10_000, "2025-12-30", Some(ACCOUNT)).await;
    let income = |year: i32| async move {
        let inputs = euer_repo::load_inputs(pool).await.unwrap();
        aggregate::aggregate(year, &inputs).invoice_income_cents
    };
    assert_eq!((income(2025).await, income(2026).await), (10_000, 0));

    let p = payment_ids(pool, "old").await.remove(0);
    let now = chrono::DateTime::parse_from_rfc3339("2026-01-15T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    invoices_cmd::reverse_payment_core(pool, &reverse_args("old", &p, REASON), now)
        .await
        .unwrap();

    assert_eq!((income(2025).await, income(2026).await), (0, 0));
    let detail_2025 = euer_repo::income_detail(pool, 2025).await.unwrap();
    assert_eq!(detail_2025.len(), 2, "Zahlung und Gegenbuchung im Vorjahr");
    assert!(detail_2025
        .iter()
        .any(|i| i.is_reversal && i.paid_date == "2025-12-30" && i.amount_cents == -10_000));
    assert!(euer_repo::income_detail(pool, 2026)
        .await
        .unwrap()
        .is_empty());

    for (year, income_total, bookings) in [(2025, 0, 2), (2026, 0, 0)] {
        let package = euer_cmd::euer_package(
            &env.ctx,
            serde_json::from_value(serde_json::json!({ "fiscalYear": year })).unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(
            package.form.income_total_cents, income_total,
            "ELSTER {year}"
        );
        let datev = euer_cmd::euer_export_datev(
            &env.ctx,
            serde_json::from_value(serde_json::json!({
                "fiscalYear": year, "skr": "SKR03", "consultantNumber": 1001, "clientNumber": 1
            }))
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(datev.booking_count, bookings, "DATEV-Stapel {year}");
    }
}

/// Atomarität (ADR 0042 Nr. 8): Scheitert der Cache nach dem INSERT der
/// Gegenbuchung, bleibt weder die Buchzeile noch das Audit stehen.
#[tokio::test]
async fn failed_reversal_leaves_neither_ledger_row_nor_audit() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 10_000, "2026-03-01", None).await;
    let p = payment_ids(pool, "inv").await.remove(0);
    // Testsperre: das Nachziehen des Caches scheitert.
    sqlx::query(
        "CREATE TRIGGER test_block_cache BEFORE UPDATE ON invoices
         WHEN NEW.paid_amount_cents IS NOT OLD.paid_amount_cents
         BEGIN SELECT RAISE(ABORT, 'Testsperre'); END",
    )
    .execute(pool)
    .await
    .unwrap();
    let err = reverse(pool, "inv", &p, REASON).await.unwrap_err();
    assert!(err.to_string().contains("Testsperre"), "{err}");
    let (rows, audits): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM invoice_payments WHERE kind = 'reversal'),
                (SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.payment_reversed')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!((rows, audits), (0, 0));
    sqlx::query("DROP TRIGGER test_block_cache")
        .execute(pool)
        .await
        .unwrap();
    reverse(pool, "inv", &p, REASON).await.unwrap();
}

/// Storno mit Erstattung nach einer Gegenbuchung erstattet netto; das
/// Storno-Datum darf vor dem stornierten späten Datum liegen.
#[tokio::test]
async fn storno_after_reversal_refunds_net_and_ignores_reversed_dates() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    pay(pool, "inv", 4_000, "2026-03-01", None).await;
    pay(pool, "inv", 6_000, "2026-09-30", None).await;
    let ids = payment_ids(pool, "inv").await;
    reverse(pool, "inv", &ids[1], REASON).await.unwrap();
    // Storno am 15.03.: früher als das stornierte Datum 30.09., nicht früher als
    // die verbleibende Zahlung (Entwurf entsteht, das Festschreiben scheitert
    // hier mangels Vorlagen/Sidecar).
    let args: invoices_cmd::InvoicesCancelArgs = serde_json::from_value(serde_json::json!({
        "args": { "invoiceId": "inv", "reason": "Leistung nicht erbracht", "stornoDate": "2026-03-15" }
    }))
    .unwrap();
    let res = invoices_cmd::invoices_cancel(&env.ctx, args).await;
    if let Err(e) = &res {
        assert!(
            !e.to_string().contains("vor dem letzten Zahlungseingang"),
            "{e}"
        );
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices WHERE is_storno_for = 'inv'")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(n, 1, "Storno-Entwurf angelegt");
    // Storno-Paar abschließen wie die Pipeline (Festschreiben, Original storniert).
    sqlx::query(
        "UPDATE invoices SET locked_at = '2026-03-15 10:00:00', status = 'issued'
          WHERE is_storno_for = 'inv'",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET status = 'canceled', canceled_at = '2026-03-15 10:00:00',
                canceled_by_storno_id = (SELECT id FROM invoices WHERE is_storno_for = 'inv')
          WHERE id = 'inv'",
    )
    .execute(pool)
    .await
    .unwrap();
    let inputs = euer_repo::load_inputs(pool).await.unwrap();
    let report = aggregate::aggregate(2026, &inputs);
    assert_eq!(report.invoice_income_cents, 4_000);
    assert_eq!(report.storno_refunds_cents, 4_000, "Erstattung netto");
    let storno = euer_repo::storno_detail(pool, 2026).await.unwrap();
    assert_eq!(storno[0].refunded_cents, 4_000);
    // Nach dem Storno keine Gegenbuchung mehr.
    let err = reverse(pool, "inv", &ids[0], REASON).await.unwrap_err();
    assert!(err.to_string().contains("storniert"), "{err}");
    let _ = invoice_payments::KIND_REVERSAL;
}

#[tokio::test]
async fn overdue_hint_returns_after_a_reversal() {
    let env = setup().await;
    let pool = &env.pool;
    insert_locked(pool, "inv", "RE-2026-0001", 10_000, false).await;
    let keys = || async {
        sqlx::query_scalar::<_, String>(
            "SELECT dedup_key FROM notifications WHERE dedup_key LIKE 'invoice_overdue:%'
              ORDER BY dedup_key",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    };
    klein_buch_lib::scheduler::reminders::run(
        pool,
        NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
        Path::new("/backup-targets"),
    )
    .await
    .unwrap();
    assert_eq!(keys().await, vec!["invoice_overdue:inv".to_string()]);
    pay(pool, "inv", 10_000, "2026-03-02", None).await;
    let p = payment_ids(pool, "inv").await.remove(0);
    reverse(pool, "inv", &p, REASON).await.unwrap();
    let run = |d: u32| async move {
        klein_buch_lib::scheduler::reminders::run(
            pool,
            NaiveDate::from_ymd_opt(2026, 3, d).unwrap(),
            Path::new("/backup-targets"),
        )
        .await
        .unwrap();
    };
    // Der erste Hinweis ist noch offen: kein zweiter „überfällig“ zur selben
    // Rechnung (V2-11).
    run(3).await;
    assert_eq!(keys().await, vec!["invoice_overdue:inv".to_string()]);
    // Abgehakt: der Hinweis kehrt zurück.
    sqlx::query("UPDATE notifications SET dismissed_at = '2026-03-03 09:00:00'")
        .execute(pool)
        .await
        .unwrap();
    run(4).await;
    assert_eq!(
        keys().await,
        vec![
            "invoice_overdue:inv".to_string(),
            "invoice_overdue:inv:r1".to_string()
        ]
    );
    // Noch eine Fehlbuchung, solange r1 offen ist: kein r2.
    pay(pool, "inv", 10_000, "2026-03-04", None).await;
    let p2 = payment_ids(pool, "inv").await.remove(2);
    reverse(pool, "inv", &p2, REASON).await.unwrap();
    run(5).await;
    assert_eq!(keys().await.len(), 2, "{:?}", keys().await);
}
