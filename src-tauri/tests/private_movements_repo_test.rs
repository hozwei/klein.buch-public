//! Integration-Tests für `db::repo::private_movements` (Block 9).
//!
//! Privatbewegungen sind EÜR-neutral, werden sofort festgeschrieben (locked_at)
//! und sind danach unveränderlich (trg_private_movements_immutable). Kein Storno.

use chrono::NaiveDate;
use klein_buch_lib::db::numbering;
use klein_buch_lib::db::repo::payment_accounts::PaymentAccountInput;
use klein_buch_lib::db::repo::{payment_accounts, private_movements};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::numbering::DocType;
use klein_buch_lib::domain::private_movement::PrivateMovementInput;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite");
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

fn input(movement_type: &str, account_id: Option<&str>) -> PrivateMovementInput {
    PrivateMovementInput {
        movement_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        movement_type: movement_type.into(),
        amount_cents: 50_000,
        account_id: account_id.map(|s| s.to_string()),
        description: "Privatentnahme Mai".into(),
        notes: None,
    }
}

async fn mk(pool: &SqlitePool, number: &str, mtype: &str, account: Option<&str>) -> String {
    private_movements::create(pool, &input(mtype, account), number, 2026, None)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn create_then_get_locks_immediately() {
    let (pool, _d) = setup_pool().await;
    let id = mk(&pool, "PV-2026-0001", "entnahme", None).await;

    let row = private_movements::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.movement_number, "PV-2026-0001");
    assert_eq!(row.movement_type, "entnahme");
    assert_eq!(row.amount_cents, 50_000);
    assert!(row.locked_at.is_some());
}

#[tokio::test]
async fn numbering_is_gap_free_per_year() {
    let (pool, _d) = setup_pool().await;
    let n1 = numbering::next_number(&pool, DocType::PrivateMovement, 2026)
        .await
        .unwrap();
    let n2 = numbering::next_number(&pool, DocType::PrivateMovement, 2026)
        .await
        .unwrap();
    assert_eq!(n1, "PV-2026-0001");
    assert_eq!(n2, "PV-2026-0002");
    let _ = mk(&pool, &n1, "entnahme", None).await;
    let _ = mk(&pool, &n2, "einlage", None).await;
    let list = private_movements::list(&pool, &private_movements::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn locked_movement_core_fields_are_immutable() {
    let (pool, _d) = setup_pool().await;
    let id = mk(&pool, "PV-2026-0001", "entnahme", None).await;

    let res = sqlx::query("UPDATE private_movements SET amount_cents = 1 WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "Kernfeld-Update muss am Trigger scheitern");
}

#[tokio::test]
async fn list_filters_by_type_and_joins_account_label() {
    let (pool, _d) = setup_pool().await;
    let acc = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Hauptkonto".into(),
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

    let _ = mk(&pool, "PV-2026-0001", "entnahme", Some(&acc.id)).await;
    let _ = mk(&pool, "PV-2026-0002", "einlage", None).await;

    let all = private_movements::list(&pool, &private_movements::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    // Konto-Label wird per LEFT JOIN aufgelöst.
    let with_acc = all
        .iter()
        .find(|m| m.movement_number == "PV-2026-0001")
        .unwrap();
    assert_eq!(with_acc.account_label.as_deref(), Some("Hauptkonto"));
    let without = all
        .iter()
        .find(|m| m.movement_number == "PV-2026-0002")
        .unwrap();
    assert!(without.account_label.is_none());

    let entnahmen = private_movements::list(
        &pool,
        &private_movements::ListFilter {
            movement_type: Some("entnahme".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(entnahmen.len(), 1);
    assert_eq!(entnahmen[0].movement_number, "PV-2026-0001");
}
