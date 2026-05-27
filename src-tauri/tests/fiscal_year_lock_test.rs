//! Integration-Tests für den Geschäftsjahr-Abschluss (Block 15).
//!
//! Nutzt eine tempfile-SQLite-DB mit den echten Migrationen (inkl. 0012/0013) —
//! damit greifen CHECK-Constraints, FKs und die GoBD-Immutability-Trigger
//! (`trg_depreciation_immutable`, `trg_assets_immutable`,
//! `trg_fiscal_year_locks_no_update/_no_delete`).

use chrono::NaiveDate;
use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::numbering;
use klein_buch_lib::db::repo::{assets, depreciation};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::asset::{business_book_value_start_cents, AssetInput};
use klein_buch_lib::domain::numbering::DocType;
use klein_buch_lib::fiscal_year::{self, guard};
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

async fn mk_asset(
    pool: &SqlitePool,
    acq: NaiveDate,
    cost: i64,
) -> klein_buch_lib::db::models::AssetRow {
    let inp = AssetInput {
        label: "Testanlage".into(),
        acquisition_date: acq,
        acquisition_cost_cents: cost,
        expense_id: None,
        vendor_contact_id: None,
        depreciation_method: "linear".into(),
        useful_life_years: Some(5.0),
        afa_category: None,
        business_share_percent: 100.0,
        notes: None,
    };
    let fy = acq.format("%Y").to_string().parse::<i64>().unwrap();
    let number = numbering::next_number(pool, DocType::Asset, fy as i32)
        .await
        .unwrap();
    let book = business_book_value_start_cents(cost, 100.0);
    assets::create(pool, &inp, &number, fy, "linear", Some(5.0), book)
        .await
        .unwrap()
}

#[tokio::test]
async fn close_year_books_locks_and_blocks_reposting() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into()); // entsperrt

    let asset = mk_asset(&pool, d(2026, 1, 15), 250_000).await;

    // GJ 2026 abschließen (today liegt im Folgejahr).
    let lock = fiscal_year::lock::close_year(&pool, &paths, &session, 2026, d(2027, 3, 1))
        .await
        .unwrap();
    assert_eq!(lock.fiscal_year, 2026);
    assert!(lock.depreciation_entries_locked >= 1, "AfA festgeschrieben");
    assert!(lock.assets_locked >= 1, "Anlage festgeschrieben");
    assert!(lock.afa_total_cents > 0, "AfA im Snapshot");

    // Guard: 2026 zu, andere Jahre offen.
    assert!(guard::is_closed(&pool, 2026).await.unwrap());
    assert!(guard::ensure_year_open(&pool, 2026).await.is_err());
    assert!(guard::ensure_year_open(&pool, 2027).await.is_ok());

    // Anlage + AfA-Buchungen sind gelockt.
    let a = assets::get(&pool, &asset.id).await.unwrap().unwrap();
    assert!(a.locked_at.is_some(), "Anlage hat locked_at");
    let entries = depreciation::list_for_year(&pool, 2026).await.unwrap();
    assert!(!entries.is_empty(), "mind. eine 2026-AfA-Buchung");
    assert!(
        entries.iter().all(|e| e.locked_at.is_some()),
        "alle 2026-AfA-Buchungen festgeschrieben"
    );

    // Direkter Update-Versuch auf eine gelockte AfA-Buchung → Trigger ABORT.
    let res = sqlx::query(
        "UPDATE depreciation_entries SET depreciation_amount_cents = 1 WHERE fiscal_year = 2026",
    )
    .execute(&pool)
    .await;
    assert!(res.is_err(), "festgeschriebene AfA ist unveränderlich");

    // Doppel-Abschluss + laufendes Jahr → Fehler.
    assert!(
        fiscal_year::lock::close_year(&pool, &paths, &session, 2026, d(2027, 3, 1))
            .await
            .is_err(),
        "bereits abgeschlossen"
    );
    assert!(
        fiscal_year::lock::close_year(&pool, &paths, &session, 2027, d(2027, 3, 1))
            .await
            .is_err(),
        "laufendes Jahr nicht abschließbar"
    );
}

#[tokio::test]
async fn close_year_requires_unlocked_session() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default(); // gesperrt
    let _a = mk_asset(&pool, d(2026, 1, 15), 120_000).await;
    let res = fiscal_year::lock::close_year(&pool, &paths, &session, 2026, d(2027, 1, 2)).await;
    assert!(res.is_err(), "ohne entsperrtes Backup kein Abschluss");
    assert!(
        !guard::is_closed(&pool, 2026).await.unwrap(),
        "2026 bleibt offen"
    );
}

#[tokio::test]
async fn protocol_is_immutable_and_lock_for_year_idempotent() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());
    let _a = mk_asset(&pool, d(2026, 1, 15), 120_000).await;
    fiscal_year::lock::close_year(&pool, &paths, &session, 2026, d(2027, 1, 2))
        .await
        .unwrap();

    // Erneutes lock_for_year ändert nichts (alle bereits gelockt).
    assert_eq!(depreciation::lock_for_year(&pool, 2026).await.unwrap(), 0);

    // Festschreibungsprotokoll: kein Update, kein Delete.
    assert!(
        sqlx::query("UPDATE fiscal_year_locks SET surplus_cents = 0 WHERE fiscal_year = 2026")
            .execute(&pool)
            .await
            .is_err(),
        "Protokoll ist unveränderlich"
    );
    assert!(
        sqlx::query("DELETE FROM fiscal_year_locks WHERE fiscal_year = 2026")
            .execute(&pool)
            .await
            .is_err(),
        "Protokoll ist nicht löschbar"
    );

    // list_closed liefert das Jahr.
    let closed = fiscal_year::lock::list_closed(&pool).await.unwrap();
    assert!(closed.iter().any(|l| l.fiscal_year == 2026));
}
