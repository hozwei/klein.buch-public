//! Lauf F8b (ADR 0039 Nr. 146): Anschaffungskosten aus Kosten genau nach
//! § 9b Abs. 1 EStG, Vorsteuer aktivierter Kosten bei Regelbesteuerung.
//!
//! - Teil-Aktivierung und Mehrbeträge (Nebenkosten aus anderem Beleg) werden
//!   beim Anlegen und Korrigieren abgelehnt — EÜR und DATEV nehmen die ganze
//!   Kostenposition heraus.
//! - §19: Bruttobetrag; Summe AfA + Rest = Anschaffungskosten = Zahlung.
//! - Regelbesteuerung: Nettobetrag; die Vorsteuer bleibt Betriebsausgabe des
//!   Zahlungsjahres (EÜR, Kz 185, DATEV-Vorsteuer), AfA + Rest + Vorsteuer =
//!   Zahlung.
//! - Bestandsanlagen mit abweichendem Betrag: Hinweis in der Ausfüllhilfe,
//!   keine stille Änderung.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::{assets as assets_cmd, euer as euer_cmd, expenses as expenses_cmd};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{euer as euer_repo, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::asset::{
    business_book_value_start_cents, AssetInput, DepreciationMethod,
};
use klein_buch_lib::domain::depreciation::{compute_yearly, DepreciationAsset};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::error::Result;
use klein_buch_lib::euer::{aggregate, datev_csv};
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
        name: "Beispiel IT-Service".into(),
        legal_form: None,
        street: "Beispielweg 1".into(),
        postal_code: "12345".into(),
        city: "Musterstadt".into(),
        country_code: "DE".into(),
        tax_number: Some("123/456/78901".into()),
        vat_id: None,
        email: "kontakt@example.org".into(),
        phone: Some("+49 000 000000".into()),
        iban: None,
        bic: None,
        logo_filename: None,
        is_kleinunternehmer: None,
        default_pdf_template: Some("default".into()),
        default_currency: Some("EUR".into()),
    }
}

/// `klein = false`: Verzicht auf §19 ab Beginn des laufenden Jahres (Eintrag in
/// der Statushistorie, ADR 0042; Regelbesteuerung im laufenden Jahr).
async fn setup(klein: bool) -> Env {
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
    if !klein {
        use klein_buch_lib::db::repo::tax_status;
        use klein_buch_lib::domain::tax_status::{verzicht_binding_until, ChangeReason, TaxStatus};
        let today = klein_buch_lib::clock::today_berlin();
        let jan = NaiveDate::from_ymd_opt(chrono::Datelike::year(&today), 1, 1).unwrap();
        tax_status::insert(
            &pool,
            &tax_status::NewEntry {
                effective_from: jan,
                status: TaxStatus::Regelbesteuerung,
                reason: ChangeReason::Verzicht,
                binding_until: Some(verzicht_binding_until(jan)),
                trigger_document: None,
                note: None,
            },
            today,
        )
        .await
        .unwrap();
    }
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

/// Notebook 1.000 € netto + 190 € USt.
fn laptop(date: NaiveDate) -> ExpenseInput {
    ExpenseInput {
        expense_date: date,
        paid_date: Some(date),
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "Elektronik Beispiel".into(),
        vendor_invoice_number: None,
        category: "office".into(),
        description: "Notebook".into(),
        net_amount_cents: 100_000,
        tax_amount_cents: 19_000,
        gross_amount_cents: 119_000,
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

fn asset_from(expense_id: &str, date: NaiveDate, cost: i64) -> AssetInput {
    AssetInput {
        label: "Notebook".into(),
        acquisition_date: date,
        acquisition_cost_cents: cost,
        expense_id: Some(expense_id.to_string()),
        vendor_contact_id: None,
        depreciation_method: "linear".into(),
        useful_life_years: Some(3.0),
        afa_category: None,
        business_share_percent: 100.0,
        notes: None,
    }
}

async fn create_asset(env: &Env, input: AssetInput) -> Result<String> {
    let args: assets_cmd::AssetsCreateArgs =
        serde_json::from_value(serde_json::json!({ "input": input })).unwrap();
    Ok(assets_cmd::assets_create(&env.ctx, args).await?.asset.id)
}

async fn update_asset(env: &Env, id: &str, input: AssetInput) -> Result<()> {
    let args: assets_cmd::AssetsUpdateArgs =
        serde_json::from_value(serde_json::json!({ "id": id, "input": input })).unwrap();
    assets_cmd::assets_update(&env.ctx, args).await.map(|_| ())
}

async fn count_assets(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Summe der AfA über die ganze Nutzungsdauer und der Rest danach
/// (Erinnerungswert), mit dem Functional Core wie der AfA-Lauf.
fn afa_total_and_rest(cost: i64, date: NaiveDate) -> (i64, i64) {
    let asset = DepreciationAsset {
        depreciation_method: DepreciationMethod::Linear,
        acquisition_date: date,
        acquisition_cost_cents: cost,
        business_share_percent: 100.0,
        useful_life_years: Some(3.0),
    };
    let mut book = business_book_value_start_cents(cost, 100.0);
    let mut total = 0;
    for year in date.year()..date.year() + 6 {
        let calc = compute_yearly(&asset, year, book);
        total += calc.depreciation_amount_cents;
        book = calc.book_value_after_cents;
    }
    (total, book)
}

async fn datev_for(env: &Env, year: i32) -> Vec<datev_csv::DatevBooking> {
    let pool = &env.pool;
    let expenses = euer_repo::expense_detail(pool, year).await.unwrap();
    let capitalized = euer_repo::capitalized_expense_detail(pool, year)
        .await
        .unwrap();
    datev_csv::build_bookings(
        datev_csv::Skr::Skr03,
        year,
        &datev_csv::DatevInputs {
            income: &[],
            storno: &[],
            expenses: &expenses,
            capitalized: &capitalized,
            disposals: &[],
            assets: &[],
            private_movements: &[],
        },
    )
}

async fn package(env: &Env, year: i32) -> euer_cmd::EuerPackage {
    let args: euer_cmd::EuerPackageArgs =
        serde_json::from_value(serde_json::json!({ "fiscalYear": year })).unwrap();
    euer_cmd::euer_package(&env.ctx, args).await.unwrap()
}

/// S1: Teil-Aktivierung (kleiner) und Mehrbetrag (Nebenkosten aus anderem
/// Beleg) werden beim Anlegen abgelehnt, ohne Anlage und ohne Verweis; beim
/// Korrigieren einer korrekt angelegten Anlage ebenso.
#[tokio::test]
async fn partial_activation_and_extra_cost_are_refused() {
    let env = setup(true).await;
    let now = today();
    let expense = create_expense(&env, laptop(now)).await;

    for cost in [50_000, 125_000] {
        let err = create_asset(&env, asset_from(&expense, now, cost))
            .await
            .expect_err("abweichender Betrag");
        let msg = err.to_string();
        assert!(
            msg.contains("1190,00") && msg.contains("getrennte Kostenpositionen"),
            "{msg}"
        );
    }
    assert_eq!(count_assets(&env.pool).await, 0);
    let linked: Option<String> =
        sqlx::query_scalar("SELECT capitalized_as_asset_id FROM expenses WHERE id = ?")
            .bind(&expense)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(linked, None);

    let asset = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .unwrap();
    let err = update_asset(&env, &asset, asset_from(&expense, now, 60_000))
        .await
        .expect_err("Teilbetrag beim Korrigieren");
    assert!(err.to_string().contains("Bruttobetrag"), "{err}");
    let cost: i64 = sqlx::query_scalar("SELECT acquisition_cost_cents FROM assets WHERE id = ?")
        .bind(&asset)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(cost, 119_000);
}

/// §19: Bruttobetrag wird angenommen. Die Kosten zählen nicht selbst in der
/// EÜR, es gibt keine Vorsteuer; Summe AfA + Rest = Anschaffungskosten =
/// gezahlter Betrag. DATEV: Zugang brutto ohne BU-Schlüssel.
#[tokio::test]
async fn par19_gross_activation_sums_to_the_payment() {
    let env = setup(true).await;
    let now = today();
    let expense = create_expense(&env, laptop(now)).await;
    create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .unwrap();

    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    let report = aggregate::aggregate(now.year(), &inputs);
    assert_eq!(report.expenses_total_cents, 0);

    let pkg = package(&env, now.year()).await;
    assert!(pkg.form.lines.iter().all(|l| l.kennzahl != Some(185)));
    assert!(
        !pkg.form
            .notes
            .iter()
            .any(|n| n.contains("abweichenden Anschaffungskosten")),
        "{:?}",
        pkg.form.notes
    );

    let (afa, rest) = afa_total_and_rest(119_000, now);
    assert_eq!(afa + rest, 119_000);

    let b = datev_for(&env, now.year()).await;
    let zugang: Vec<_> = b.iter().filter(|x| x.gegenkonto == 420).collect();
    assert_eq!(zugang.len(), 1);
    assert_eq!(
        (zugang[0].amount_cents, zugang[0].bu_schluessel.as_str()),
        (119_000, "")
    );
}

/// Regelbesteuerung: Nettobetrag wird angenommen (brutto abgelehnt). Die
/// gezahlte Vorsteuer bleibt Betriebsausgabe im Zahlungsjahr — EÜR-Report,
/// Ausfüllhilfe Kz 185 und DATEV (BU-Schlüssel 9 am Anlagekonto). AfA + Rest
/// + Vorsteuer = gezahlter Betrag.
#[tokio::test]
async fn regel_net_activation_keeps_input_tax_as_expense() {
    let env = setup(false).await;
    let now = today();
    let expense = create_expense(&env, laptop(now)).await;

    let err = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .expect_err("Bruttobetrag bei Regelbesteuerung");
    assert!(err.to_string().contains("Nettobetrag"), "{err}");
    create_asset(&env, asset_from(&expense, now, 100_000))
        .await
        .unwrap();

    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    let report = aggregate::aggregate(now.year(), &inputs);
    assert_eq!(report.expenses_total_cents, 19_000, "Vorsteuer als Ausgabe");

    let pkg = package(&env, now.year()).await;
    assert!(!pkg.form.is_kleinunternehmer);
    let vst = pkg
        .form
        .lines
        .iter()
        .find(|l| l.kennzahl == Some(185) && l.is_entry)
        .expect("Kz 185");
    assert_eq!(vst.amount_cents, 19_000);
    assert_eq!(pkg.form.expense_total_cents, report.total_expenses_cents);
    assert!(
        pkg.expenses.is_empty(),
        "keine Betriebsausgabe in der Einzelaufstellung"
    );

    let (afa, rest) = afa_total_and_rest(100_000, now);
    assert_eq!(afa + rest, 100_000);
    assert_eq!(
        afa + rest + vst.amount_cents,
        119_000,
        "ganze Zahlung wirkt"
    );

    let b = datev_for(&env, now.year()).await;
    let zugang: Vec<_> = b.iter().filter(|x| x.gegenkonto == 420).collect();
    assert_eq!(zugang.len(), 1);
    assert_eq!(
        (zugang[0].amount_cents, zugang[0].bu_schluessel.as_str()),
        (119_000, "9"),
        "brutto mit BU 9: netto aufs Anlagekonto, 190 € Vorsteuer"
    );

    // Im Vorjahr (noch §19) entsteht keine Vorsteuer.
    let report_prev = aggregate::aggregate(now.year() - 1, &inputs);
    assert_eq!(report_prev.expenses_total_cents, 0);
}

/// Bestandsanlage mit abweichendem Betrag (Teil-Aktivierung aus einer älteren
/// Version, direkt in der DB): keine stille Änderung — der Hinweis in der
/// Ausfüllhilfe nennt die Kostenposition; Anlage und EÜR bleiben unverändert.
#[tokio::test]
async fn legacy_mismatch_is_flagged_not_changed() {
    let env = setup(true).await;
    let now = today();
    let expense = create_expense(&env, laptop(now)).await;
    let asset = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE assets SET acquisition_cost_cents = 50000, book_value_cents = 50000 WHERE id = ?",
    )
    .bind(&asset)
    .execute(&env.pool)
    .await
    .unwrap();
    let number: String = sqlx::query_scalar("SELECT expense_number FROM expenses WHERE id = ?")
        .bind(&expense)
        .fetch_one(&env.pool)
        .await
        .unwrap();

    let pkg = package(&env, now.year()).await;
    let note = pkg
        .form
        .notes
        .iter()
        .find(|n| n.contains("abweichenden Anschaffungskosten"))
        .expect("Hinweis");
    assert!(note.contains(&number), "{note}");
    assert_eq!(pkg.form.expense_total_cents, 0);
    let cost: i64 = sqlx::query_scalar("SELECT acquisition_cost_cents FROM assets WHERE id = ?")
        .bind(&asset)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(cost, 50_000, "Bestand nicht still geändert");
}

// ---- Lauf F8c (ADR 0039 Nr. 147) ---------------------------------------------
//
// Die Sperre des Zahldatums aktivierter Kosten über einen Wechsel des
// Steuerstatus (F8c, S2) ist mit Lauf F8d gegenstandslos: Der Status folgt dem
// Leistungsbezug (Belegdatum), nicht dem Zahlungsjahr (ADR 0039 Nr. 148,
// `lauf_f8d_test`).

/// F8c (S3, § 13b Abs. 8 UStG, § 9b Abs. 1 EStG): Kleinunternehmer mit
/// Reverse-Charge-Kauf. Die Lieferantenrechnung (netto = brutto) wird zum
/// Rechnungsbetrag aktiviert; der Betrag mit der selbst geschuldeten Steuer wird
/// mit dem Weg über eine eigene Kostenposition abgelehnt. Die Steuerzahlung ans
/// Finanzamt wird als eigene Kostenposition erfasst und als eigene Anlage
/// (gleiches Datum, gleiche Methode) übernommen. Ergebnis: Keine der beiden
/// Kostenpositionen zählt selbst in der EÜR, die AfA beider Anlagen + Rest =
/// Rechnung + Steuer, DATEV-Zugang auf dem Anlagekonto = Rechnung + Steuer.
#[tokio::test]
async fn reverse_charge_tax_under_par19_is_capitalized_once_as_own_asset() {
    let env = setup(true).await;
    let now = today();
    let mut invoice = laptop(now);
    invoice.description = "Server-Lizenz (Reverse-Charge)".into();
    invoice.tax_amount_cents = 0;
    invoice.gross_amount_cents = 100_000;
    invoice.reverse_charge_13b = true;
    let invoice = create_expense(&env, invoice).await;

    let args: assets_cmd::AssetsSuggestMethodArgs = serde_json::from_value(serde_json::json!({
        "expenseCategory": "office", "acquisitionCostCents": 0, "expenseId": invoice
    }))
    .unwrap();
    let source = assets_cmd::assets_suggest_method(&env.ctx, args)
        .await
        .unwrap()
        .source
        .expect("Vorbelegung");
    assert!(source.is_kleinunternehmer && source.reverse_charge_13b);
    assert_eq!(source.acquisition_cost_cents, 100_000);

    let err = create_asset(&env, asset_from(&invoice, now, 119_000))
        .await
        .expect_err("Rechnung + Steuer an der Rechnungsposition");
    let msg = err.to_string();
    assert!(
        msg.contains("§ 13b") && msg.contains("eigene Kostenposition"),
        "{msg}"
    );
    assert_eq!(count_assets(&env.pool).await, 0);
    create_asset(&env, asset_from(&invoice, now, 100_000))
        .await
        .unwrap();

    // Zahlung der selbst geschuldeten Steuer (Anteil dieser Anlage) ans Finanzamt.
    let mut tax = laptop(now);
    tax.vendor_name = "Finanzamt Musterstadt".into();
    tax.category = "fees".into();
    tax.description = "USt § 13b zur Server-Lizenz".into();
    tax.net_amount_cents = 19_000;
    tax.tax_amount_cents = 0;
    tax.gross_amount_cents = 19_000;
    let tax = create_expense(&env, tax).await;
    let mut tax_asset = asset_from(&tax, now, 19_000);
    tax_asset.label = "USt § 13b zur Server-Lizenz".into();
    create_asset(&env, tax_asset).await.unwrap();

    // EÜR: keine der Kostenpositionen als Betriebsausgabe, keine Vorsteuer.
    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    let report = aggregate::aggregate(now.year(), &inputs);
    assert_eq!(report.expenses_total_cents, 0);
    let pkg = package(&env, now.year()).await;
    assert!(pkg.expenses.is_empty());
    assert!(pkg.form.lines.iter().all(|l| l.kennzahl != Some(185)));
    assert!(
        !pkg.form
            .notes
            .iter()
            .any(|n| n.contains("abweichenden Anschaffungskosten")),
        "{:?}",
        pkg.form.notes
    );

    // AfA beider Anlagen + Rest = Rechnung + Steuer (genau einmal).
    let (afa_a, rest_a) = afa_total_and_rest(100_000, now);
    let (afa_b, rest_b) = afa_total_and_rest(19_000, now);
    assert_eq!(afa_a + rest_a + afa_b + rest_b, 119_000);

    // DATEV: Zugang auf dem Anlagekonto = Rechnung + Steuer, ohne BU-Schlüssel.
    let b = datev_for(&env, now.year()).await;
    let zugang: Vec<_> = b.iter().filter(|x| x.gegenkonto == 420).collect();
    assert_eq!(zugang.len(), 2);
    assert_eq!(zugang.iter().map(|x| x.amount_cents).sum::<i64>(), 119_000);
    assert!(zugang.iter().all(|x| x.bu_schluessel.is_empty()));
}
