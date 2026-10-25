//! Lauf F8d (ADR 0039 Nr. 148): Steuerstatus der Kosten nach dem
//! Leistungsbezug, nicht nach dem Zahlungsjahr.
//!
//! Für Vorsteuerabzug und § 9b Abs. 1 EStG ist der Status bei Leistungsbezug
//! maßgeblich (§ 15 Abs. 1 UStG; Abschn. 19.5 UStAE: Vorsteuer aus Leistungen
//! vor dem Übergang zur Regelbesteuerung ist nicht abziehbar, auch bei späterer
//! Zahlung). Kosten haben kein eigenes Leistungsdatum; es gilt das Belegdatum.
//! Die zeitliche Zuordnung der Ausgabe bleibt das Zahlungsjahr (§ 11 EStG).
//!
//! - Leistung im §19-Jahr, Zahlung im Jahr mit Regelbesteuerung: brutto, keine
//!   Vorsteuer (EÜR, Ausfüllhilfe, DATEV gleich).
//! - Leistung unter Regelbesteuerung, Zahlung vorher im §19-Jahr: netto,
//!   Vorsteuer abziehbar im Zahlungsjahr.
//! - Aktivierte Kosten an der Grenze: Anschaffungskosten nach dem Belegdatum;
//!   das Zahldatum lässt sich frei setzen, das Belegdatum gar nicht.
//! - Verzicht auf §19 nach dem Aktivieren: nicht blockiert, Inbox und
//!   Ausfüllhilfe nennen die Anlage mit dem Handlungsweg.
//! - Lauf F8e: Anschaffungsdatum (Lieferung) und Belegdatum in Jahren mit
//!   unterschiedlichem Status: Anlegen/Bearbeiten abgelehnt, nach einem
//!   späteren Verzicht nicht korrigierbar; Hinweis zur 100.000-€-Grenze.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::{
    assets as assets_cmd, euer as euer_cmd, expenses as expenses_cmd, reauth,
    settings as settings_cmd,
};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{euer as euer_repo, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::asset::AssetInput;
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

fn profile(klein: bool) -> seller_profile::SellerProfileInput {
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
        is_kleinunternehmer: klein,
        default_pdf_template: Some("default".into()),
        default_currency: Some("EUR".into()),
        confirm_waive_paragraph_19: (!klein).then_some(true),
    }
}

/// `klein = false`: Verzicht auf §19 ab heute — Regelbesteuerung ab Beginn des
/// laufenden Jahres, das Vorjahr bleibt §19.
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
    seller_profile::upsert(&pool, &profile(true)).await.unwrap();
    if !klein {
        seller_profile::upsert(&pool, &profile(false))
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

fn dec(year: i32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 12, day).unwrap()
}

/// Notebook 1.000 € netto + 190 € USt, Belegdatum `doc`, Zahlung `paid`.
fn laptop(doc: NaiveDate, paid: Option<NaiveDate>) -> ExpenseInput {
    ExpenseInput {
        expense_date: doc,
        paid_date: paid,
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

async fn create_expense(env: &Env, input: ExpenseInput) -> (String, String) {
    let args: expenses_cmd::ExpensesCreateArgs = serde_json::from_value(serde_json::json!({
        "args": { "input": input }
    }))
    .unwrap();
    let e = expenses_cmd::expenses_create(&env.ctx, args)
        .await
        .unwrap()
        .expense;
    (e.id, e.expense_number)
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

async fn create_asset(env: &Env, input: AssetInput) -> Result<(String, String)> {
    let args: assets_cmd::AssetsCreateArgs =
        serde_json::from_value(serde_json::json!({ "input": input })).unwrap();
    let a = assets_cmd::assets_create(&env.ctx, args).await?.asset;
    Ok((a.id, a.asset_number))
}

async fn update_asset(env: &Env, id: &str, input: AssetInput) -> Result<()> {
    let args: assets_cmd::AssetsUpdateArgs =
        serde_json::from_value(serde_json::json!({ "id": id, "input": input })).unwrap();
    assets_cmd::assets_update(&env.ctx, args).await.map(|_| ())
}

async fn set_payment(env: &Env, expense_id: &str, paid: Option<NaiveDate>) -> Result<()> {
    let args: expenses_cmd::ExpensesSetPaymentArgs = serde_json::from_value(serde_json::json!({
        "args": { "expenseId": expense_id, "paidDate": paid, "paidFromAccountId": null }
    }))
    .unwrap();
    expenses_cmd::expenses_set_payment(&env.ctx, args)
        .await
        .map(|_| ())
}

async fn package(env: &Env, year: i32) -> euer_cmd::EuerPackage {
    let args: euer_cmd::EuerPackageArgs =
        serde_json::from_value(serde_json::json!({ "fiscalYear": year })).unwrap();
    euer_cmd::euer_package(&env.ctx, args).await.unwrap()
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

fn kz(pkg: &euer_cmd::EuerPackage, kz: u16) -> Option<i64> {
    pkg.form
        .lines
        .iter()
        .find(|l| l.kennzahl == Some(kz) && l.is_entry)
        .map(|l| l.amount_cents)
}

/// Leistung im §19-Jahr (Belegdatum Dezember des Vorjahres), Zahlung im Jahr
/// mit Regelbesteuerung: Die USt ist keine abziehbare Vorsteuer (Abschn. 19.5
/// UStAE). Ausfüllhilfe brutto in der Kategorie, keine Kz 185, Hinweis mit
/// der Kostennummer; EÜR-Report und Ausfüllhilfe gleich; DATEV brutto ohne
/// BU-Schlüssel. Die Ausgabe zählt im Zahlungsjahr (§ 11 Abs. 2 EStG).
#[tokio::test]
async fn supply_under_par19_paid_under_regel_is_gross_without_input_tax() {
    let env = setup(false).await;
    let now = today();
    let (_, number) = create_expense(&env, laptop(dec(now.year() - 1, 1), Some(now))).await;

    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    let report = aggregate::aggregate(now.year(), &inputs);
    assert_eq!(report.expenses_total_cents, 119_000);
    assert_eq!(
        aggregate::aggregate(now.year() - 1, &inputs).expenses_total_cents,
        0
    );

    let pkg = package(&env, now.year()).await;
    assert!(!pkg.form.is_kleinunternehmer);
    assert_eq!(kz(&pkg, 229), Some(119_000), "brutto in der Kategorie");
    assert_eq!(kz(&pkg, 185), None, "keine Vorsteuer");
    assert_eq!(pkg.form.expense_total_cents, report.total_expenses_cents);
    assert_eq!(pkg.expenses.len(), 1);
    assert!(pkg.expenses[0].is_kleinunternehmer);
    let note = pkg
        .form
        .notes
        .iter()
        .find(|n| n.starts_with("Kosten mit Leistungsbezug"))
        .expect("Hinweis");
    assert!(note.contains(&number), "{note}");

    let b = datev_for(&env, now.year()).await;
    assert_eq!(b.len(), 1);
    assert_eq!(
        (
            b[0].amount_cents,
            b[0].gegenkonto,
            b[0].bu_schluessel.as_str()
        ),
        (119_000, 4930, "")
    );
}

/// Umgekehrt: Leistung unter Regelbesteuerung (Belegdatum heute), Zahlung
/// vorab im §19-Vorjahr. Die Vorsteuer ist abziehbar; sie steht als
/// Betriebsausgabe im Zahlungsjahr (Kz 185), netto in der Kategorie; DATEV mit
/// BU-Schlüssel 9. Das laufende Jahr bleibt ohne Ausgabe.
#[tokio::test]
async fn supply_under_regel_paid_in_par19_year_deducts_input_tax() {
    let env = setup(false).await;
    let now = today();
    let prev = now.year() - 1;
    let (_, number) = create_expense(&env, laptop(now, Some(dec(prev, 15)))).await;

    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    assert_eq!(
        aggregate::aggregate(prev, &inputs).expenses_total_cents,
        119_000
    );
    assert_eq!(
        aggregate::aggregate(now.year(), &inputs).expenses_total_cents,
        0
    );

    let pkg = package(&env, prev).await;
    assert!(pkg.form.is_kleinunternehmer);
    assert_eq!(kz(&pkg, 229), Some(100_000), "netto in der Kategorie");
    assert_eq!(kz(&pkg, 185), Some(19_000), "Vorsteuer abziehbar");
    assert_eq!(pkg.form.expense_total_cents, 119_000);
    assert!(pkg.form.notes.iter().any(|n| n.contains(&number)));
    assert!(pkg
        .form
        .notes
        .iter()
        .any(|n| n.starts_with("Regelbesteuerung")));

    let b = datev_for(&env, prev).await;
    assert_eq!(b.len(), 1);
    assert_eq!(
        (
            b[0].amount_cents,
            b[0].gegenkonto,
            b[0].bu_schluessel.as_str()
        ),
        (119_000, 4930, "9")
    );
}

/// Aktivierte Kosten an der Grenze. (1) Belegdatum im §19-Vorjahr, unbezahlt:
/// brutto ist richtig, netto wird abgelehnt (auch wenn später im Regeljahr
/// gezahlt wird). Das Zahldatum lässt sich jetzt ins Regeljahr setzen (F8c
/// lehnte das ab) — der Status bleibt, EÜR 0, keine Kz 185, kein
/// Abweichungs-Hinweis, DATEV-Zugang brutto ohne BU-Schlüssel. (2) Belegdatum
/// im Regeljahr, Zahlung vorab im §19-Vorjahr: netto; die Vorsteuer wirkt im
/// Zahlungsjahr (Kz 185, DATEV BU 9). (3) Das Belegdatum aktivierter Kosten
/// ist festgeschrieben — ein Statuswechsel darüber ist ausgeschlossen.
#[tokio::test]
async fn capitalized_cost_at_the_boundary_follows_the_document_date() {
    let env = setup(false).await;
    let now = today();
    let prev = now.year() - 1;

    // (1)
    let doc = dec(prev, 1);
    let (expense, number) = create_expense(&env, laptop(doc, None)).await;
    let args: assets_cmd::AssetsSuggestMethodArgs = serde_json::from_value(serde_json::json!({
        "expenseCategory": "office", "acquisitionCostCents": 0, "expenseId": expense
    }))
    .unwrap();
    let source = assets_cmd::assets_suggest_method(&env.ctx, args)
        .await
        .unwrap()
        .source
        .expect("Vorbelegung");
    assert!(source.is_kleinunternehmer);
    assert_eq!(source.acquisition_cost_cents, 119_000);
    let err = create_asset(&env, asset_from(&expense, doc, 100_000))
        .await
        .expect_err("netto bei Leistungsbezug unter §19");
    assert!(err.to_string().contains("Bruttobetrag"), "{err}");
    create_asset(&env, asset_from(&expense, doc, 119_000))
        .await
        .unwrap();

    set_payment(&env, &expense, Some(now))
        .await
        .expect("Zahldatum ändert den Status nicht mehr");
    set_payment(&env, &expense, None).await.unwrap();
    set_payment(&env, &expense, Some(now)).await.unwrap();

    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    assert_eq!(
        aggregate::aggregate(now.year(), &inputs).expenses_total_cents,
        0
    );
    let pkg = package(&env, now.year()).await;
    assert_eq!(kz(&pkg, 185), None);
    assert!(
        !pkg.form
            .notes
            .iter()
            .any(|n| n.contains("abweichenden Anschaffungskosten")),
        "{:?}",
        pkg.form.notes
    );
    assert!(pkg
        .form
        .notes
        .iter()
        .any(|n| n.starts_with("Kosten mit Leistungsbezug") && n.contains(&number)));
    let b = datev_for(&env, now.year()).await;
    let zugang: Vec<_> = b.iter().filter(|x| x.gegenkonto == 420).collect();
    assert_eq!(zugang.len(), 1);
    assert_eq!(
        (zugang[0].amount_cents, zugang[0].bu_schluessel.as_str()),
        (119_000, "")
    );

    // (2)
    let (other, _) = create_expense(&env, laptop(now, Some(dec(prev, 20)))).await;
    let err = create_asset(&env, asset_from(&other, now, 119_000))
        .await
        .expect_err("brutto bei Leistungsbezug unter Regelbesteuerung");
    assert!(err.to_string().contains("Nettobetrag"), "{err}");
    create_asset(&env, asset_from(&other, now, 100_000))
        .await
        .unwrap();
    let inputs = euer_repo::load_inputs(&env.pool).await.unwrap();
    assert_eq!(
        aggregate::aggregate(prev, &inputs).expenses_total_cents,
        19_000,
        "Vorsteuer im Zahlungsjahr"
    );
    let pkg = package(&env, prev).await;
    assert_eq!(kz(&pkg, 185), Some(19_000));
    assert_eq!(pkg.form.expense_total_cents, 19_000);
    let b = datev_for(&env, prev).await;
    let zugang: Vec<_> = b.iter().filter(|x| x.gegenkonto == 420).collect();
    assert_eq!(
        (zugang[0].amount_cents, zugang[0].bu_schluessel.as_str()),
        (119_000, "9")
    );

    // (3) Belegdatum über die Jahresgrenze verschieben: DB-Trigger lehnt ab.
    let err = sqlx::query("UPDATE expenses SET expense_date = ? WHERE id = ?")
        .bind(now.to_string())
        .bind(&expense)
        .execute(&env.pool)
        .await
        .expect_err("Belegdatum ist Kernfeld");
    assert!(err.to_string().contains("expense is locked"), "{err}");
}

/// Verzicht auf §19 nach dem Aktivieren (wirkt ab Jahresbeginn): nicht
/// blockiert. Die Inbox nennt die betroffene Anlage mit Soll-Betrag und
/// Handlungsweg; eine abgegangene Anlage verweist an die Steuerberatung. Die
/// Ausfüllhilfe nennt die Anlage ebenso. Nach der Korrektur der
/// Anschaffungskosten verschwindet der Hinweis.
#[tokio::test]
async fn waiver_after_capitalizing_names_affected_assets() {
    let env = setup(true).await;
    let now = today();
    let (expense, number) = create_expense(&env, laptop(now, Some(now))).await;
    let (asset, asset_number) = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .unwrap();
    let mut second = laptop(now, Some(now));
    second.description = "Monitor".into();
    let (expense2, _) = create_expense(&env, second).await;
    let (asset2, asset_number2) = create_asset(&env, asset_from(&expense2, now, 119_000))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE assets SET disposed = 1, disposal_date = ?, disposal_type = 'scrap',
                disposal_proceeds_cents = 0, disposal_residual_book_value_cents = 119000
          WHERE id = ?",
    )
    .bind(now.to_string())
    .bind(&asset2)
    .execute(&env.pool)
    .await
    .unwrap();

    let args: settings_cmd::SellerProfileUpsertArgs =
        serde_json::from_value(serde_json::json!({ "input": profile(false) })).unwrap();
    let row = reauth::with_reauth(true, settings_cmd::seller_profile_upsert(&env.ctx, args))
        .await
        .expect("Verzicht wird nicht blockiert");
    assert_eq!(row.is_kleinunternehmer, 0);

    let (title, body): (String, String) = sqlx::query_as(
        "SELECT title, body FROM notifications WHERE dedup_key LIKE 'capitalized_status_change:%'",
    )
    .fetch_one(&env.pool)
    .await
    .expect("Inbox-Hinweis");
    assert!(title.contains("§19-Wechsel"), "{title}");
    assert!(
        body.contains(&format!("Anlage {asset_number} (Kosten {number})"))
            && body.contains("1.190,00")
            && body.contains("1.000,00")
            && body.contains("korrigieren")
            && body.contains(&format!("Anlage {asset_number2}"))
            && body.contains("Steuerberatung"),
        "{body}"
    );

    let mismatches = assets_cmd::capitalized_cost_mismatches(&env.pool)
        .await
        .unwrap();
    assert_eq!(mismatches.len(), 2);
    assert!(mismatches[0].correctable && !mismatches[1].correctable);

    let pkg = package(&env, now.year()).await;
    let note = pkg
        .form
        .notes
        .iter()
        .find(|n| n.contains("abweichenden Anschaffungskosten"))
        .expect("Hinweis der Ausfüllhilfe");
    assert!(
        note.contains(&format!("Anlage {asset_number}")) && note.contains("Anlage bearbeiten"),
        "{note}"
    );

    // Korrektur auf den Nettobetrag (keine AfA gebucht): Hinweis weg.
    update_asset(&env, &asset, asset_from(&expense, now, 100_000))
        .await
        .unwrap();
    let left = assets_cmd::capitalized_cost_mismatches(&env.pool)
        .await
        .unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].asset_number, asset_number2);

    // Ein erneutes Speichern ohne Statuswechsel legt keinen Hinweis an.
    let args: settings_cmd::SellerProfileUpsertArgs =
        serde_json::from_value(serde_json::json!({ "input": profile(false) })).unwrap();
    reauth::with_reauth(true, settings_cmd::seller_profile_upsert(&env.ctx, args))
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'capitalized_status_change:%'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(n, 1);
}

/// Lauf F8e (ADR 0039 Nr. 148 (5)): Bei einer Anlage ist der Leistungsbezug die
/// Lieferung (§ 9a EStDV), also das Anschaffungsdatum; Klein.Buch bestimmt den
/// Status der Kosten nach dem Belegdatum. Liegen beide in Jahren mit
/// unterschiedlichem Status, lehnen `assets_create` und `assets_update` ab —
/// mit beiden Daten, beiden Status und dem Verweis an die Steuerberatung, vor
/// der Betragsprüfung (in beide Richtungen).
#[tokio::test]
async fn acquisition_date_in_year_with_other_status_than_document_is_rejected() {
    let env = setup(false).await;
    let now = today();
    let prev = now.year() - 1;
    let delivery = dec(prev, 20);

    // Lieferung im §19-Jahr, Rechnung im Jahr mit Regelbesteuerung.
    let (expense, number) = create_expense(&env, laptop(now, None)).await;
    for cost in [119_000, 100_000] {
        let err = create_asset(&env, asset_from(&expense, delivery, cost))
            .await
            .expect_err("Status von Anschaffungs- und Belegdatum verschieden");
        let msg = err.to_string();
        assert!(
            msg.contains(&format!(
                "Anschaffungsdatum {} (Kleinunternehmer nach §19)",
                delivery.format("%d.%m.%Y")
            )) && msg.contains(&format!(
                "Belegdatum {} der Kosten {number} (Regelbesteuerung)",
                now.format("%d.%m.%Y")
            )) && msg.contains("§ 9a EStDV")
                && msg.contains("Steuerberatung"),
            "{msg}"
        );
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "keine Anlage angelegt");

    // Gleiches Jahr: angelegt; ein Umzug des Anschaffungsdatums ins §19-Jahr
    // wird beim Bearbeiten ebenso abgelehnt.
    let (asset, _) = create_asset(&env, asset_from(&expense, now, 100_000))
        .await
        .unwrap();
    let err = update_asset(&env, &asset, asset_from(&expense, delivery, 119_000))
        .await
        .expect_err("Bearbeiten prüft dasselbe");
    assert!(
        err.to_string().contains("unterschiedlichem Steuerstatus"),
        "{err}"
    );
    update_asset(&env, &asset, asset_from(&expense, now, 100_000))
        .await
        .expect("gleiches Jahr bleibt erlaubt");

    // Umgekehrt: Rechnung im §19-Jahr, Lieferung im Jahr mit Regelbesteuerung.
    let (early, _) = create_expense(&env, laptop(dec(prev, 1), None)).await;
    let err = create_asset(&env, asset_from(&early, now, 100_000))
        .await
        .expect_err("umgekehrte Richtung");
    let msg = err.to_string();
    assert!(
        msg.contains("(Regelbesteuerung) und Belegdatum")
            && msg.contains("(Kleinunternehmer nach §19) liegen"),
        "{msg}"
    );
}

/// Lauf F8e: Ein späterer Verzicht kann Anschaffungs- und Belegdatum einer
/// bestehenden Anlage unter verschiedene Status bringen. Dann ist der
/// Soll-Betrag nach dem Belegdatum kein sicherer Korrekturweg: Die Anlage gilt
/// als nicht korrigierbar (Steuerberatung). Inbox und Ausfüllhilfe nennen
/// außerdem den Wegfall von §19 beim Überschreiten der 100.000-€-Grenze, in
/// dem Anlagen nicht zu korrigieren sind.
#[tokio::test]
async fn waiver_splitting_delivery_and_document_year_is_not_correctable() {
    let env = setup(true).await;
    let now = today();
    let delivery = dec(now.year() - 1, 20);
    let (expense, number) = create_expense(&env, laptop(now, Some(now))).await;
    let (_, asset_number) = create_asset(&env, asset_from(&expense, delivery, 119_000))
        .await
        .expect("beide Jahre §19");

    let args: settings_cmd::SellerProfileUpsertArgs =
        serde_json::from_value(serde_json::json!({ "input": profile(false) })).unwrap();
    reauth::with_reauth(true, settings_cmd::seller_profile_upsert(&env.ctx, args))
        .await
        .expect("Verzicht wird nicht blockiert");

    let mismatches = assets_cmd::capitalized_cost_mismatches(&env.pool)
        .await
        .unwrap();
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].asset_number, asset_number);
    assert!(!mismatches[0].correctable);

    let body: String = sqlx::query_scalar(
        "SELECT body FROM notifications WHERE dedup_key LIKE 'capitalized_status_change:%'",
    )
    .fetch_one(&env.pool)
    .await
    .expect("Inbox-Hinweis");
    assert!(
        body.contains(&format!("Anlage {asset_number} (Kosten {number})"))
            && body.contains("Anschaffungsdatum in einem Jahr mit anderem Steuerstatus")
            && body.contains("100.000-€-Grenze")
            && body.contains("nicht korrigieren"),
        "{body}"
    );

    let pkg = package(&env, now.year()).await;
    let note = pkg
        .form
        .notes
        .iter()
        .find(|n| n.contains("abweichenden Anschaffungskosten"))
        .expect("Hinweis der Ausfüllhilfe");
    assert!(
        note.contains("Nicht korrigieren") && note.contains("100.000-€-Grenze"),
        "{note}"
    );
    assert!(
        pkg.form
            .notes
            .iter()
            .any(|n| n.starts_with("Regelbesteuerung:") && n.contains("100.000-€-Grenze")),
        "{:?}",
        pkg.form.notes
    );
}
