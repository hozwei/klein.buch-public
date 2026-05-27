//! Integration-Tests für `db::repo::recurring` + `scheduler::recurring`
//! (Block 10).
//!
//! Nutzt eine tempfile-SQLite-DB mit den echten Migrationen (inkl. 0009) —
//! damit greifen CHECK-Constraints, FKs und (für die erzeugten Kosten) der
//! GoBD-Immutability-Trigger.

use chrono::NaiveDate;
use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{expenses, recurring};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::recurring::RecurringInput;
use klein_buch_lib::scheduler;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

async fn setup() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("klein-buch.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
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
    (pool, dir)
}

/// Manuell gebaute `Paths` in den Tempdir — damit `process_due`/`run_now` ihre
/// Backup-Hooks (best-effort) gegen ein echtes Ziel fahren können.
fn paths_for(dir: &Path) -> Paths {
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    std::fs::create_dir_all(dir.join("archive")).unwrap();
    Paths {
        data_dir: dir.to_path_buf(),
        db_file: dir.join("klein-buch.sqlite"),
        archive_dir: dir.join("archive"),
        backups_dir: backups,
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

/// Legt eine echte (sofort gelockte) Kosten-Position an und liefert ihre ID —
/// für FK-konforme `advance`-Tests (`last_expense_id REFERENCES expenses(id)`).
async fn mk_expense(pool: &SqlitePool, number: &str) -> String {
    let input = ExpenseInput {
        expense_date: d(2026, 5, 1),
        paid_date: None,
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "MS365".into(),
        vendor_invoice_number: None,
        category: "software".into(),
        description: "MS365 — Abo".into(),
        net_amount_cents: 1_190,
        tax_amount_cents: 0,
        gross_amount_cents: 1_190,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    };
    expenses::create(pool, &input, number, 2026, None)
        .await
        .unwrap()
        .id
}

fn sub_input(
    label: &str,
    frequency: &str,
    day_of_period: i64,
    next_due: NaiveDate,
    auto: bool,
) -> RecurringInput {
    RecurringInput {
        label: label.into(),
        vendor_contact_id: None,
        frequency: frequency.into(),
        day_of_period,
        next_due_date: next_due,
        expected_amount_cents: 1_190,
        category: "software".into(),
        description_template: format!("{label} — Abo"),
        auto_create_expense: auto,
        reverse_charge_13b_default: false,
    }
}

// ---- CRUD ------------------------------------------------------------------

#[tokio::test]
async fn create_then_get_roundtrip() {
    let (pool, _d) = setup().await;
    let row = recurring::create(
        &pool,
        &sub_input("MS365", "monthly", 1, d(2026, 6, 1), true),
    )
    .await
    .unwrap();
    assert_eq!(row.label, "MS365");
    assert_eq!(row.frequency, "monthly");
    assert_eq!(row.day_of_period, 1);
    assert_eq!(row.next_due_date, "2026-06-01");
    assert_eq!(row.auto_create_expense, 1);
    assert_eq!(row.active, 1);
    assert!(row.last_expense_id.is_none());

    let got = recurring::get(&pool, &row.id).await.unwrap().unwrap();
    assert_eq!(got.id, row.id);
}

#[tokio::test]
async fn update_changes_config() {
    let (pool, _d) = setup().await;
    let row = recurring::create(
        &pool,
        &sub_input("Adobe", "monthly", 5, d(2026, 6, 5), false),
    )
    .await
    .unwrap();

    let mut upd = sub_input("Adobe CC", "annually", 15, d(2027, 1, 15), true);
    upd.expected_amount_cents = 23_988;
    let after = recurring::update(&pool, &row.id, &upd).await.unwrap();
    assert_eq!(after.label, "Adobe CC");
    assert_eq!(after.frequency, "annually");
    assert_eq!(after.day_of_period, 15);
    assert_eq!(after.next_due_date, "2027-01-15");
    assert_eq!(after.expected_amount_cents, 23_988);
    assert_eq!(after.auto_create_expense, 1);
}

#[tokio::test]
async fn set_active_pauses_and_list_excludes() {
    let (pool, _d) = setup().await;
    let row = recurring::create(
        &pool,
        &sub_input("Backblaze", "monthly", 1, d(2026, 6, 1), true),
    )
    .await
    .unwrap();

    recurring::set_active(&pool, &row.id, false).await.unwrap();
    let active = recurring::list(&pool, false).await.unwrap();
    assert!(
        active.iter().all(|s| s.id != row.id),
        "pausiert → nicht in aktiver Liste"
    );

    let all = recurring::list(&pool, true).await.unwrap();
    let found = all.iter().find(|s| s.id == row.id).unwrap();
    assert_eq!(found.active, 0);

    recurring::set_active(&pool, &row.id, true).await.unwrap();
    assert!(recurring::list(&pool, false)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == row.id));
}

// ---- list_due_auto ---------------------------------------------------------

#[tokio::test]
async fn list_due_auto_filters_active_auto_and_due() {
    let (pool, _d) = setup().await;
    let today = "2026-05-20";

    let a = recurring::create(
        &pool,
        &sub_input("A-due-auto", "monthly", 1, d(2026, 5, 1), true),
    )
    .await
    .unwrap();
    // B: auto, aber Stichtag in der Zukunft.
    recurring::create(
        &pool,
        &sub_input("B-future-auto", "monthly", 1, d(2026, 6, 1), true),
    )
    .await
    .unwrap();
    // C: fällig, aber manuell (auto=0).
    recurring::create(
        &pool,
        &sub_input("C-due-manual", "monthly", 1, d(2026, 5, 1), false),
    )
    .await
    .unwrap();
    // D: auto + fällig, aber pausiert.
    let dd = recurring::create(
        &pool,
        &sub_input("D-due-paused", "monthly", 1, d(2026, 5, 1), true),
    )
    .await
    .unwrap();
    recurring::set_active(&pool, &dd.id, false).await.unwrap();

    let due = recurring::list_due_auto(&pool, today).await.unwrap();
    assert_eq!(due.len(), 1, "nur A ist aktiv+auto+fällig");
    assert_eq!(due[0].id, a.id);
}

// ---- advance ---------------------------------------------------------------

#[tokio::test]
async fn advance_writes_next_and_last() {
    let (pool, _d) = setup().await;
    let row = recurring::create(
        &pool,
        &sub_input("MS365", "monthly", 1, d(2026, 5, 1), true),
    )
    .await
    .unwrap();
    let expense_id = mk_expense(&pool, "KO-2026-9999").await;

    recurring::advance(&pool, &row.id, &expense_id, "2026-06-01")
        .await
        .unwrap();
    let after = recurring::get(&pool, &row.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-06-01");
    assert_eq!(after.last_expense_id.as_deref(), Some(expense_id.as_str()));
    assert!(after.last_executed_at.is_some());
}

// ---- scheduler::process_due ------------------------------------------------

#[tokio::test]
async fn process_due_skips_when_locked() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default(); // gesperrt

    recurring::create(
        &pool,
        &sub_input("MS365", "monthly", 1, d(2026, 5, 1), true),
    )
    .await
    .unwrap();

    let report = scheduler::recurring::process_due(&pool, &paths, &session, d(2026, 5, 20))
        .await
        .unwrap();
    assert!(
        report.skipped_locked,
        "gesperrte Session → Lauf übersprungen"
    );
    assert_eq!(report.created_expenses, 0);

    let list = expenses::list(&pool, &expenses::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 0, "ohne Unlock keine Kosten angelegt");
}

#[tokio::test]
async fn process_due_catches_up_all_missed_periods() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into()); // entsperrt → Lauf läuft

    let sub = recurring::create(
        &pool,
        &sub_input("MS365", "monthly", 1, d(2026, 3, 1), true),
    )
    .await
    .unwrap();

    // Heute 2026-05-20 → fällige Stichtage 03-01, 04-01, 05-01 (3 Stück);
    // nächster 06-01 liegt in der Zukunft.
    let report = scheduler::recurring::process_due(&pool, &paths, &session, d(2026, 5, 20))
        .await
        .unwrap();
    assert!(!report.skipped_locked);
    assert_eq!(report.processed_subscriptions, 1);
    assert_eq!(
        report.created_expenses, 3,
        "drei verpasste Perioden nachgeholt"
    );

    // next_due_date ist auf die nächste Zukunfts-Periode fortgeschrieben.
    let after = recurring::get(&pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-06-01");
    assert!(after.last_expense_id.is_some());

    // Drei Kosten-Positionen, alle: nicht bezahlt, mit Abo verknüpft, KO-Nummern lückenlos.
    let list = expenses::list(&pool, &expenses::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 3);
    for item in &list {
        let row = expenses::get(&pool, &item.id).await.unwrap().unwrap();
        assert_eq!(
            row.recurring_subscription_id.as_deref(),
            Some(sub.id.as_str())
        );
        assert!(
            row.paid_date.is_none(),
            "auto-angelegt = noch nicht bezahlt"
        );
        assert!(row.locked_at.is_some(), "Kosten sind sofort gelockt");
        assert_eq!(row.gross_amount_cents, 1_190);
        assert_eq!(row.tax_amount_cents, 0, "Template ohne USt-Split");
    }
    let mut numbers: Vec<String> = list.iter().map(|i| i.expense_number.clone()).collect();
    numbers.sort();
    assert_eq!(
        numbers,
        vec![
            "KO-2026-0001".to_string(),
            "KO-2026-0002".to_string(),
            "KO-2026-0003".to_string()
        ]
    );

    // Zweiter Lauf am selben Tag legt nichts Neues an (idempotent über Stichtag).
    let again = scheduler::recurring::process_due(&pool, &paths, &session, d(2026, 5, 20))
        .await
        .unwrap();
    assert_eq!(again.created_expenses, 0);
}

// ---- scheduler::run_now ----------------------------------------------------

#[tokio::test]
async fn run_now_creates_one_and_advances() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let sub = recurring::create(
        &pool,
        &sub_input("Hosting", "monthly", 1, d(2026, 5, 1), false),
    )
    .await
    .unwrap();

    let expense = scheduler::recurring::run_now(&pool, &paths, &session, &sub.id, d(2026, 5, 20))
        .await
        .unwrap();
    assert_eq!(expense.expense_date, "2026-05-01");
    assert_eq!(
        expense.recurring_subscription_id.as_deref(),
        Some(sub.id.as_str())
    );
    assert!(expense.paid_date.is_none());

    let after = recurring::get(&pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(
        after.next_due_date, "2026-06-01",
        "um eine Periode vorgerückt"
    );

    // Nur EINE Position (kein Catch-up bei manuell).
    let list = expenses::list(&pool, &expenses::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn run_now_rejects_future_due() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let sub = recurring::create(
        &pool,
        &sub_input("Hosting", "monthly", 1, d(2026, 6, 1), false),
    )
    .await
    .unwrap();

    // next_due 2026-06-01 liegt nach today 2026-05-20 → vorzeitige Erfassung
    // würde ein zukünftiges Beleg-Datum erzeugen → Fehler.
    let res = scheduler::recurring::run_now(&pool, &paths, &session, &sub.id, d(2026, 5, 20)).await;
    assert!(res.is_err());
    let list = expenses::list(&pool, &expenses::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 0);
}

#[tokio::test]
async fn run_now_rejects_paused_subscription() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let sub = recurring::create(
        &pool,
        &sub_input("Hosting", "monthly", 1, d(2026, 5, 1), false),
    )
    .await
    .unwrap();
    recurring::set_active(&pool, &sub.id, false).await.unwrap();

    let res = scheduler::recurring::run_now(&pool, &paths, &session, &sub.id, d(2026, 5, 20)).await;
    assert!(res.is_err(), "pausiertes Abo darf nicht erfasst werden");
}
