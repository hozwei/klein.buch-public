//! ADR 0042 Nr. 14 (Lauf 2026.11.1, Block V5): Übersicht der
//! Vorsteuerberichtigung nach § 15a UStG.
//!
//! - Kauf im Vorjahr unter § 19 (Vorsteuer 1.900 €), Verzicht ab 1. Januar des
//!   laufenden Jahres: Übersicht mit Zeitraum, Jahresanteil 1/5 zugunsten und
//!   dem Hinweis „nur Übersicht“; Inbox-Hinweis verweist auf die Übersicht.
//! - Vorsteuer bis 1.000 €: § 44 Abs. 1 UStDV, keine Berichtigung.
//! - Anlage ohne Kostenbeleg: ausgewiesen statt geraten.
//! - Der Statuswechsel ändert keine Anlage und keine Kosten.
//!
//! Tempfile-DB mit den echten Migrationen; nur Dokumentations-Adressen.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::{
    assets as assets_cmd, expenses as expenses_cmd, reauth, tax_status as tax_cmd,
    vat_adjustment as vat_cmd,
};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::seller_profile;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::asset::AssetInput;
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::vat_adjustment::{Direction, Exemption, Filing};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

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
    std::fs::create_dir_all(dir.join("inputs").join("specs")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../inputs/specs/afa-tabellen.json"),
        dir.join("inputs").join("specs").join("afa-tabellen.json"),
    )
    .unwrap();
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

fn profile() -> seller_profile::SellerProfileInput {
    seller_profile::SellerProfileInput {
        name: "Beispiel Studio".into(),
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
    }
}

async fn setup() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    let url = format!("sqlite://{}", paths.db_file.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    seller_profile::upsert(&pool, &profile()).await.unwrap();
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    Env {
        pool,
        ctx,
        _dir: dir,
    }
}

fn today() -> NaiveDate {
    klein_buch_lib::clock::today_berlin()
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn purchase(doc: NaiveDate, net: i64, tax: i64, what: &str) -> ExpenseInput {
    ExpenseInput {
        expense_date: doc,
        paid_date: Some(doc),
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "Technik Beispiel".into(),
        vendor_invoice_number: None,
        category: "hardware".into(),
        description: what.into(),
        net_amount_cents: net,
        tax_amount_cents: tax,
        gross_amount_cents: net + tax,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    }
}

async fn create_expense(env: &Env, input: ExpenseInput) -> String {
    let args: expenses_cmd::ExpensesCreateArgs = serde_json::from_value(serde_json::json!({
        "args": { "input": input }
    }))
    .unwrap();
    expenses_cmd::expenses_create(&env.ctx, args)
        .await
        .unwrap()
        .expense
        .id
}

async fn create_asset(
    env: &Env,
    expense_id: Option<&str>,
    date: NaiveDate,
    cost: i64,
    label: &str,
) -> String {
    let input = AssetInput {
        label: label.into(),
        acquisition_date: date,
        acquisition_cost_cents: cost,
        expense_id: expense_id.map(str::to_string),
        vendor_contact_id: None,
        depreciation_method: "linear".into(),
        useful_life_years: Some(5.0),
        afa_category: None,
        business_share_percent: 100.0,
        notes: None,
    };
    let args: assets_cmd::AssetsCreateArgs =
        serde_json::from_value(serde_json::json!({ "input": input })).unwrap();
    assets_cmd::assets_create(&env.ctx, args)
        .await
        .unwrap()
        .asset
        .asset_number
}

/// Alle Spalten von Anlagen und Kosten, um Unveränderlichkeit zu prüfen.
async fn snapshot(pool: &SqlitePool) -> String {
    let a: Vec<String> = sqlx::query_scalar(
        "SELECT json_object('n', asset_number, 'c', acquisition_cost_cents, 'd', acquisition_date,
                'b', book_value_cents, 'l', locked_at, 'u', updated_at, 'e', expense_id)
           FROM assets ORDER BY asset_number",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let e: Vec<String> = sqlx::query_scalar(
        "SELECT json_object('n', expense_number, 'net', net_amount_cents, 't', tax_amount_cents,
                'g', gross_amount_cents, 'l', locked_at, 's', status, 'cap', capitalized_as_asset_id)
           FROM expenses ORDER BY expense_number",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    format!("{a:?}{e:?}")
}

#[tokio::test]
async fn overview_after_waiver_shows_yearly_share_and_inbox_points_to_it() {
    let env = setup().await;
    let year = today().year();
    let prev = year - 1;

    // Kamera im Vorjahr unter § 19: Vorsteuer 1.900 €, Anschaffungskosten brutto.
    let cam_expense =
        create_expense(&env, purchase(d(prev, 3, 10), 1_000_000, 190_000, "Kamera")).await;
    let cam = create_asset(
        &env,
        Some(&cam_expense),
        d(prev, 3, 10),
        1_190_000,
        "Kamera",
    )
    .await;
    // Objektiv mit Vorsteuer 950 €: § 44 Abs. 1 UStDV.
    let lens_expense =
        create_expense(&env, purchase(d(prev, 4, 2), 500_000, 95_000, "Objektiv")).await;
    let lens = create_asset(
        &env,
        Some(&lens_expense),
        d(prev, 4, 2),
        595_000,
        "Objektiv",
    )
    .await;
    // Anlage ohne Kostenbeleg.
    let desk = create_asset(&env, None, d(prev, 5, 5), 300_000, "Schreibtisch").await;

    let before = snapshot(&env.pool).await;
    let args: tax_cmd::TaxStatusChangeAddArgs = serde_json::from_value(serde_json::json!({
        "input": {
            "effectiveFrom": format!("{year}-01-01"),
            "status": "regelbesteuerung",
            "reason": "verzicht",
        }
    }))
    .unwrap();
    reauth::with_reauth(true, tax_cmd::tax_status_change_add(&env.ctx, args))
        .await
        .expect("Verzicht");
    assert_eq!(
        snapshot(&env.pool).await,
        before,
        "keine Anlage, keine Kosten geändert"
    );

    let o = vat_cmd::vat_adjustment_overview(
        &env.ctx,
        serde_json::from_value(serde_json::json!({ "year": year })).unwrap(),
    )
    .await
    .unwrap();
    assert!(o.notice.contains("Klein.Buch bucht die Berichtigung nicht"));
    assert!(o.notice.contains("Steuerberatung"));

    let a = o
        .assets
        .iter()
        .find(|a| a.asset_number == cam)
        .expect("Kamera");
    assert!(a.has_change);
    assert!(a.adjustment.original_klein);
    assert_eq!(a.adjustment.period.from, d(prev, 3, 1));
    assert_eq!(
        a.adjustment.period.to,
        d(prev + 5, 3, 1).pred_opt().unwrap()
    );
    let y = a.year_share.as_ref().unwrap();
    assert_eq!((y.months, y.months_changed), (12, 12));
    assert_eq!(y.adjustment_cents, 38_000, "1.900 € × 12/60");
    assert_eq!(y.direction, Some(Direction::Zugunsten));
    assert_eq!(y.filing, Some(Filing::AnnualReturn));
    assert_eq!(a.adjustment.year(prev).unwrap().adjustment_cents, 0);

    let l = o
        .assets
        .iter()
        .find(|a| a.asset_number == lens)
        .expect("Objektiv");
    assert!(l.adjustment.input_tax_bagatelle);
    let ly = l.year_share.as_ref().unwrap();
    assert_eq!(ly.exemption, Some(Exemption::InputTaxBagatelle));
    assert_eq!(ly.adjustment_cents, 0);
    assert_eq!(o.total_adjustment_cents, 38_000);

    let u = o
        .unknown
        .iter()
        .find(|u| u.asset_number == desk)
        .expect("ohne Beleg");
    assert_eq!(u.reason, vat_cmd::UnknownReason::NoExpense);

    // Inbox-Hinweis mit Verweis auf die Übersicht.
    let (title, body, url): (String, String, Option<String>) = sqlx::query_as(
        "SELECT title, body, action_url FROM notifications WHERE dedup_key LIKE 'vat_adjustment:add:%'",
    )
    .fetch_one(&env.pool)
    .await
    .expect("Hinweis § 15a");
    assert!(title.contains("§ 15a"), "{title}");
    assert_eq!(url.as_deref(), Some(vat_cmd::OVERVIEW_URL));
    assert!(
        body.contains(&format!("Anlage {cam} (Kamera)"))
            && body.contains(&format!("{year} 380,00 € zugunsten"))
            && body.contains(&format!("Anlage {lens} (Objektiv)"))
            && body.contains("§ 44 Abs. 1 UStDV")
            && !body.contains(&desk),
        "{body}"
    );

    // Jahr vor dem Wechsel: keine Berichtigung; Jahr nach dem Zeitraum: leer.
    let o = vat_cmd::overview_core(&env.pool, prev, today())
        .await
        .unwrap();
    assert_eq!(o.total_adjustment_cents, 0);
    let o = vat_cmd::overview_core(&env.pool, prev + 6, today())
        .await
        .unwrap();
    assert!(o.assets.is_empty() && o.unknown.is_empty());
}

/// Ohne Statuswechsel im Zeitraum: kein Hinweis, nichts zu berichtigen.
#[tokio::test]
async fn no_change_no_notice() {
    let env = setup().await;
    let year = today().year();
    let e = create_expense(&env, purchase(d(year, 2, 3), 1_000_000, 190_000, "Kamera")).await;
    create_asset(&env, Some(&e), d(year, 2, 3), 1_190_000, "Kamera").await;
    let o = vat_cmd::overview_core(&env.pool, year, today())
        .await
        .unwrap();
    assert_eq!(o.assets.len(), 1);
    assert!(!o.assets[0].has_change);
    assert_eq!(o.total_adjustment_cents, 0);
    vat_cmd::notify_after_status_change(&env.pool, "vat_adjustment:test", today()).await;
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE dedup_key = 'vat_adjustment:test'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(n, 0);
}
