//! Integration-Tests für `db::repo::assets` + `db::repo::depreciation` +
//! `depreciation::accrue_yearly` (Block 12).
//!
//! Nutzt eine tempfile-SQLite-DB mit den echten Migrationen (inkl. 0010/0011) —
//! damit greifen CHECK-Constraints, FKs und die GoBD-Immutability-Trigger.

use chrono::NaiveDate;
use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::numbering;
use klein_buch_lib::db::repo::{assets, depreciation};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::depreciation::accrue_yearly;
use klein_buch_lib::domain::asset::{business_book_value_start_cents, AssetInput};
use klein_buch_lib::domain::numbering::DocType;
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

fn input(method: &str, acq: NaiveDate, cost: i64, share: f64, life: Option<f64>) -> AssetInput {
    AssetInput {
        label: "Testanlage".into(),
        acquisition_date: acq,
        acquisition_cost_cents: cost,
        expense_id: None,
        vendor_contact_id: None,
        depreciation_method: method.into(),
        useful_life_years: life,
        afa_category: None,
        business_share_percent: share,
        notes: None,
    }
}

/// Legt eine Anlage über die Repo-Schicht an (analog zur Command-Auflösung:
/// effektive Nutzungsdauer + Start-Restbuchwert vorab berechnet).
async fn mk_asset(
    pool: &SqlitePool,
    method: &str,
    acq: NaiveDate,
    cost: i64,
    share: f64,
    life: Option<f64>,
) -> klein_buch_lib::db::models::AssetRow {
    let inp = input(method, acq, cost, share, life);
    let fy = acq.format("%Y").to_string().parse::<i64>().unwrap();
    let number = numbering::next_number(pool, DocType::Asset, fy as i32)
        .await
        .unwrap();
    let effective_life = match method {
        "computer_special_2021" => Some(1.0),
        "gwg_sofort" => None,
        _ => life,
    };
    let book = business_book_value_start_cents(cost, share);
    assets::create(pool, &inp, &number, fy, method, effective_life, book)
        .await
        .unwrap()
}

// ---- CRUD ------------------------------------------------------------------

#[tokio::test]
async fn create_is_unlocked_with_business_book_value() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 15), 250_000, 100.0, Some(3.0)).await;
    assert_eq!(a.asset_number, "AV-2026-0001");
    assert_eq!(a.book_value_cents, 250_000);
    assert!(a.locked_at.is_none(), "Anlage ist bei Anlage unlocked");
    assert_eq!(a.disposed, 0);
    assert_eq!(a.acquisition_fiscal_year, 2026);
}

#[tokio::test]
async fn private_share_reduces_start_book_value() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 15), 100_000, 80.0, Some(4.0)).await;
    assert_eq!(
        a.book_value_cents, 80_000,
        "80 % betrieblich → 800,00 € Basis"
    );
}

#[tokio::test]
async fn numbering_is_gap_free_per_year() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;
    let b = mk_asset(&pool, "linear", d(2026, 2, 1), 100_000, 100.0, Some(5.0)).await;
    assert_eq!(a.asset_number, "AV-2026-0001");
    assert_eq!(b.asset_number, "AV-2026-0002");
}

#[tokio::test]
async fn update_allowed_while_unlocked_blocked_after_lock() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;

    // Pre-lock: Korrektur erlaubt.
    let mut inp = input("linear", d(2026, 1, 1), 120_000, 100.0, Some(5.0));
    inp.label = "Korrigiert".into();
    let updated = assets::update(&pool, &a.id, &inp, "linear", Some(5.0), 120_000)
        .await
        .unwrap();
    assert_eq!(updated.label, "Korrigiert");
    assert_eq!(updated.acquisition_cost_cents, 120_000);

    // Lock → Update verboten.
    assets::lock(&pool, &a.id).await.unwrap();
    let res = assets::update(&pool, &a.id, &inp, "linear", Some(5.0), 120_000).await;
    assert!(
        res.is_err(),
        "festgeschriebene Anlage darf nicht geändert werden"
    );
}

#[tokio::test]
async fn locked_asset_core_fields_immutable_via_trigger() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;
    assets::lock(&pool, &a.id).await.unwrap();

    // Direkter Kernfeld-Update → trg_assets_immutable ABORT.
    let res = sqlx::query("UPDATE assets SET acquisition_cost_cents = 1 WHERE id = ?")
        .bind(&a.id)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "Kernfeld-Update muss am Trigger scheitern");

    // book_value-Fortschreibung bleibt erlaubt (kein Kernfeld).
    assets::set_book_value(&pool, &a.id, 80_000, Some(2026))
        .await
        .unwrap();
    let row = assets::get(&pool, &a.id).await.unwrap().unwrap();
    assert_eq!(row.book_value_cents, 80_000);
}

// ---- Dispose ---------------------------------------------------------------

#[tokio::test]
async fn dispose_sets_fields_and_blocks_double() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;

    let disposed = assets::dispose(&pool, &a.id, "2026-06-30", "sale", 50_000, 100_000)
        .await
        .unwrap();
    assert_eq!(disposed.disposed, 1);
    assert_eq!(disposed.disposal_type.as_deref(), Some("sale"));
    assert_eq!(disposed.disposal_proceeds_cents, Some(50_000));
    assert_eq!(disposed.disposal_residual_book_value_cents, Some(100_000));

    // Doppel-Disposal verhindert.
    assert!(assets::dispose(&pool, &a.id, "2026-07-01", "scrap", 0, 0)
        .await
        .is_err());
}

#[tokio::test]
async fn dispose_allowed_after_lock() {
    let (pool, _d) = setup().await;
    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;
    assets::lock(&pool, &a.id).await.unwrap();
    // Disposal-Felder stehen nicht in der Immutability-Whitelist → erlaubt.
    let disposed = assets::dispose(&pool, &a.id, "2026-06-30", "scrap", 0, 80_000)
        .await
        .unwrap();
    assert_eq!(disposed.disposed, 1);
}

// ---- AfA-Buchungslauf ------------------------------------------------------

#[tokio::test]
async fn accrue_skips_when_session_locked() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default(); // gesperrt
    mk_asset(&pool, "gwg_sofort", d(2026, 3, 1), 60_000, 100.0, None).await;

    let report = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 5, 21))
        .await
        .unwrap();
    assert!(report.skipped_locked);
    assert_eq!(report.booked_entries, 0);
}

#[tokio::test]
async fn accrue_gwg_full_writeoff_does_not_lock() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let a = mk_asset(&pool, "gwg_sofort", d(2026, 3, 1), 60_000, 100.0, None).await;

    let report = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 5, 21))
        .await
        .unwrap();
    assert_eq!(report.booked_entries, 1);
    assert_eq!(report.processed_assets, 1);
    assert_eq!(report.total_depreciation_cents, 60_000);

    let row = assets::get(&pool, &a.id).await.unwrap().unwrap();
    assert_eq!(row.book_value_cents, 0, "GWG voll abgeschrieben");
    assert_eq!(row.last_depreciation_year, Some(2026));
    assert!(
        row.locked_at.is_none(),
        "Festschreibung erst zum GJ-Abschluss (Block 15)"
    );

    let entries = depreciation::list_for_asset(&pool, &a.id).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].depreciation_amount_cents, 60_000);
    assert_eq!(entries[0].is_full_writeoff, 1);
    assert!(
        entries[0].locked_at.is_none(),
        "Buchung ungelockt bis GJ-Abschluss"
    );

    // Idempotent: zweiter Lauf bucht nichts mehr.
    let again = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 5, 21))
        .await
        .unwrap();
    assert_eq!(again.booked_entries, 0);
}

#[tokio::test]
async fn accrue_linear_catches_up_and_runs_to_zero() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    // 3.600,00 € / 3 Jahre, angeschafft Juli 2024.
    let a = mk_asset(&pool, "linear", d(2024, 7, 1), 360_000, 100.0, Some(3.0)).await;

    // Heute 2030 → AfA bis 2029 nachbuchen (alle Perioden auf einmal).
    let report = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2029, d(2030, 1, 1))
        .await
        .unwrap();
    assert_eq!(report.processed_assets, 1);
    assert_eq!(report.booked_entries, 4, "2024(6M)+2025+2026+2027(Rest)");
    // R2-020 / §7 EStG: bei laufender Nutzung bleibt ein Erinnerungswert von
    // 1 Cent stehen → Summe der AfA = AK − 1 Cent.
    assert_eq!(report.total_depreciation_cents, 359_999);

    let row = assets::get(&pool, &a.id).await.unwrap().unwrap();
    assert_eq!(
        row.book_value_cents, 1,
        "Erinnerungswert 1 Cent bleibt im Bestand (R2-020)"
    );
    assert_eq!(row.last_depreciation_year, Some(2027));
    assert!(
        row.locked_at.is_none(),
        "Festschreibung erst zum GJ-Abschluss"
    );

    let entries = depreciation::list_for_asset(&pool, &a.id).await.unwrap();
    let years: Vec<i64> = entries.iter().map(|e| e.fiscal_year).collect();
    assert_eq!(years, vec![2024, 2025, 2026, 2027]);
    assert_eq!(entries[0].depreciation_amount_cents, 60_000); // 6/12
    assert_eq!(entries[1].depreciation_amount_cents, 120_000);
    assert_eq!(entries[2].depreciation_amount_cents, 120_000);
    assert_eq!(
        entries[3].depreciation_amount_cents, 59_999,
        "Letztes Jahr: Rest minus Erinnerungswert (R2-020)"
    );

    // Idempotent.
    let again = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2029, d(2030, 1, 1))
        .await
        .unwrap();
    assert_eq!(again.booked_entries, 0);
}

#[tokio::test]
async fn accrue_skips_disposed_assets() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;
    assets::dispose(&pool, &a.id, "2026-03-01", "sale", 90_000, 100_000)
        .await
        .unwrap();

    let report = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 12, 31))
        .await
        .unwrap();
    assert_eq!(
        report.booked_entries, 0,
        "veräußerte Anlage bekommt keine AfA"
    );
}

#[tokio::test]
async fn accrue_rejects_future_fiscal_year() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());
    mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;

    let res = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2027, d(2026, 5, 21)).await;
    assert!(res.is_err(), "AfA fürs Zukunftsjahr ist nicht buchbar");
}

#[tokio::test]
async fn depreciation_entry_mutable_until_locked_then_immutable() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let a = mk_asset(&pool, "gwg_sofort", d(2026, 3, 1), 60_000, 100.0, None).await;
    accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 5, 21))
        .await
        .unwrap();
    let entry_id = depreciation::list_for_asset(&pool, &a.id).await.unwrap()[0]
        .id
        .clone();

    // Offenes GJ: Buchung ist (noch) ungelockt → Änderung möglich.
    sqlx::query("UPDATE depreciation_entries SET months_in_year = 7 WHERE id = ?")
        .bind(&entry_id)
        .execute(&pool)
        .await
        .expect("ungelockte Buchung ist im offenen GJ änderbar");

    // Festschreibung zum GJ-Abschluss simulieren (Block 15 setzt locked_at).
    sqlx::query("UPDATE depreciation_entries SET locked_at = datetime('now','utc') WHERE id = ?")
        .bind(&entry_id)
        .execute(&pool)
        .await
        .expect("Festschreiben (locked_at von NULL setzen) ist erlaubt");

    // Jetzt unveränderlich → trg_depreciation_immutable ABORT.
    let res =
        sqlx::query("UPDATE depreciation_entries SET depreciation_amount_cents = 1 WHERE id = ?")
            .bind(&entry_id)
            .execute(&pool)
            .await;
    assert!(
        res.is_err(),
        "festgeschriebene AfA-Buchung darf nicht geändert werden"
    );
}

#[tokio::test]
async fn reset_restores_book_value_and_allows_rebook() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;
    accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 12, 31))
        .await
        .unwrap();
    assert_eq!(
        depreciation::count_for_asset(&pool, &a.id).await.unwrap(),
        1
    );
    let after = assets::get(&pool, &a.id).await.unwrap().unwrap();
    assert_eq!(after.book_value_cents, 80_000);
    assert_eq!(after.last_depreciation_year, Some(2026));

    // Zurücksetzen (offenes GJ) → Buchung weg, Restbuchwert + Jahr wiederhergestellt.
    accrue_yearly::reset_asset(&pool, &paths, &session, &a.id)
        .await
        .unwrap();
    assert_eq!(
        depreciation::count_for_asset(&pool, &a.id).await.unwrap(),
        0
    );
    let reset = assets::get(&pool, &a.id).await.unwrap().unwrap();
    assert_eq!(
        reset.book_value_cents, 100_000,
        "Restbuchwert zurück auf Start"
    );
    assert_eq!(reset.last_depreciation_year, None);

    // Neu buchen ist wieder möglich.
    let report = accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 12, 31))
        .await
        .unwrap();
    assert_eq!(report.booked_entries, 1);
    assert_eq!(
        assets::get(&pool, &a.id)
            .await
            .unwrap()
            .unwrap()
            .book_value_cents,
        80_000
    );
}

#[tokio::test]
async fn reset_refuses_when_nothing_unlocked_and_keeps_locked() {
    let (pool, dir) = setup().await;
    let paths = paths_for(dir.path());
    let session = BackupSession::default();
    session.set("passphrase-1234".into());

    let a = mk_asset(&pool, "linear", d(2026, 1, 1), 100_000, 100.0, Some(5.0)).await;
    // Nichts gebucht → nichts zurückzusetzen.
    assert!(accrue_yearly::reset_asset(&pool, &paths, &session, &a.id)
        .await
        .is_err());

    // Buchen + festschreiben (simulierter GJ-Abschluss) → kein offener Eintrag mehr.
    accrue_yearly::accrue_for_year(&pool, &paths, &session, 2026, d(2026, 12, 31))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE depreciation_entries SET locked_at = datetime('now','utc') WHERE asset_id = ?",
    )
    .bind(&a.id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        accrue_yearly::reset_asset(&pool, &paths, &session, &a.id)
            .await
            .is_err(),
        "festgeschriebene AfA ist nicht zurücksetzbar"
    );
    assert_eq!(
        depreciation::count_for_asset(&pool, &a.id).await.unwrap(),
        1,
        "die festgeschriebene Buchung bleibt erhalten"
    );
}
