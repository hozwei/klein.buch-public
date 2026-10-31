//! Lauf F8 (Schlussprüfung, ADR 0039 Nr. 145): Fachlogik-Nachbesserungen.
//!
//! - Aktivieren von Kosten: Beleg- und Zahlungsjahr müssen offen sein, keine
//!   stornierten Kosten, Anschaffungskosten nach § 9b Abs. 1 EStG (bei §19
//!   brutto), Nummer/Anlage/Verweis/Audit in einer Transaktion.
//! - Aktivierte Kosten lassen sich nicht stornieren.
//! - Storno-Datum nicht vor dem letzten Zahlungseingang (§ 11 EStG).
//! - Storno-Entwurf mit Audit `invoice.draft_create` in einer Transaktion.
//! - Angebotsumwandlung: Entwurf und `converted` gemeinsam, keine zweite.
//! - Restore-Konflikt: Ein bestätigter Entwurf wird nach einer Änderung wieder
//!   vorgemerkt.
//! - Privatentnahme einer Anlage mit Entnahmewert.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::backup::after_restore;
use klein_buch_lib::commands::{assets as assets_cmd, expenses as expenses_cmd};
use klein_buch_lib::commands::{invoices as invoices_cmd, quotes as quotes_cmd};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{app_settings, contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::asset::AssetInput;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::error::Result;
use klein_buch_lib::fiscal_year::lock;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

const PASS: &str = "korrekt-pferd-batterie-klammer";

struct Env {
    pool: SqlitePool,
    ctx: Arc<AppCtx>,
    contact: String,
    _dir: tempfile::TempDir,
}

fn paths_for(dir: &Path) -> Paths {
    let data = dir.join("data");
    for d in ["archive", "backups", "tmp"] {
        std::fs::create_dir_all(data.join(d)).unwrap();
    }
    // AfA-Tabelle für den Methodenvorschlag (menschengepflegt, nur gelesen).
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
            phone: Some("+49 000 000000".into()),
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
    let contact = contacts::create(
        &pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            street: "Hauptstr. 7".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            email: Some("kunde@example.org".into()),
            accepts_einvoice: true,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id;
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    Env {
        pool,
        ctx,
        contact,
        _dir: dir,
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn today() -> NaiveDate {
    klein_buch_lib::clock::today_berlin()
}

/// Notebook 1.000 € netto + 190 € USt (Eingangsrechnung mit USt, bei §19
/// zulässig — nur nicht als Vorsteuer abziehbar).
fn laptop(date: NaiveDate, paid: Option<NaiveDate>) -> ExpenseInput {
    ExpenseInput {
        expense_date: date,
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

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn capitalized_of(pool: &SqlitePool, expense_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT capitalized_as_asset_id FROM expenses WHERE id = ?")
        .bind(expense_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// S1: Kosten eines abgeschlossenen Jahres lassen sich nicht (mehr) als Anlage
/// aktivieren — auch nicht mit einem Anschaffungsdatum im offenen Jahr. Die
/// Ablehnung verbraucht keine Anlagennummer.
#[tokio::test]
async fn activating_expense_of_a_closed_year_is_refused() {
    let env = setup().await;
    let now = today();
    let closed = now.year() - 1;
    let expense = create_expense(&env, laptop(d(closed, 12, 1), Some(d(closed, 12, 20)))).await;

    env.ctx.session.set(PASS.into());
    lock::close_year(
        &env.pool,
        &env.ctx.paths,
        &env.ctx.session,
        closed as i64,
        klein_buch_lib::clock::berlin_to_utc(now, 12, 0),
    )
    .await
    .unwrap();
    env.ctx.session.clear();

    let err = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .expect_err("Kosten im abgeschlossenen Jahr");
    let msg = err.to_string();
    assert!(
        msg.contains(&format!("abgeschlossenen Geschäftsjahr {closed}")),
        "{msg}"
    );
    assert_eq!(capitalized_of(&env.pool, &expense).await, None);
    assert_eq!(scalar(&env.pool, "SELECT COUNT(*) FROM assets").await, 0);
    assert_eq!(
        scalar(
            &env.pool,
            "SELECT COUNT(*) FROM doc_number_counters WHERE doc_type = 'asset' AND last_value > 0"
        )
        .await,
        0,
        "keine Anlagennummer verbraucht"
    );
}

/// S1 (§ 9b Abs. 1 EStG): Bei §19 sind die Anschaffungskosten brutto. Der
/// Nettobetrag wird abgelehnt, der Bruttobetrag angenommen; AfA-Grundlage =
/// Bruttobetrag (das DATEV-Anlagekonto geht damit über die AfA auf 0). Ein
/// zweites Aktivieren und das Aktivieren stornierter Kosten scheitern;
/// aktivierte Kosten lassen sich nicht stornieren.
#[tokio::test]
async fn activation_uses_gross_under_par19_once_and_only_from_recorded_expenses() {
    let env = setup().await;
    let now = today();
    let expense = create_expense(&env, laptop(now, Some(now))).await;

    // Vorbelegung über den Vorschlag: brutto bei §19, GWG-Basis netto.
    let args: assets_cmd::AssetsSuggestMethodArgs = serde_json::from_value(serde_json::json!({
        "expenseCategory": "office", "acquisitionCostCents": 0, "expenseId": expense
    }))
    .unwrap();
    let s = assets_cmd::assets_suggest_method(&env.ctx, args)
        .await
        .unwrap();
    let source = s.source.expect("Vorbelegung aus den Kosten");
    assert!(source.is_kleinunternehmer);
    assert_eq!(source.acquisition_cost_cents, 119_000);
    assert_eq!(source.gwg_basis_cents, 100_000);
    assert_eq!(source.acquisition_date, now.to_string());

    let err = create_asset(&env, asset_from(&expense, now, 100_000))
        .await
        .expect_err("Nettobetrag bei §19");
    assert!(err.to_string().contains("Bruttobetrag"), "{err}");
    assert_eq!(capitalized_of(&env.pool, &expense).await, None);

    let asset = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .unwrap();
    assert_eq!(
        capitalized_of(&env.pool, &expense).await.as_deref(),
        Some(asset.as_str())
    );
    let (cost, book): (i64, i64) =
        sqlx::query_as("SELECT acquisition_cost_cents, book_value_cents FROM assets WHERE id = ?")
            .bind(&asset)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!((cost, book), (119_000, 119_000));
    let audit: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'asset.create' AND entity_id = ?",
    )
    .bind(&asset)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(audit.contains(&expense), "{audit}");

    // Zweites Aktivieren derselben Kosten.
    let err = create_asset(&env, asset_from(&expense, now, 119_000))
        .await
        .expect_err("doppelt");
    assert!(err.to_string().contains("bereits"), "{err}");

    // Quell-Kosten lassen sich beim Korrigieren nicht tauschen.
    let other = create_expense(&env, laptop(now, Some(now))).await;
    let update: assets_cmd::AssetsUpdateArgs = serde_json::from_value(serde_json::json!({
        "id": asset, "input": asset_from(&other, now, 119_000)
    }))
    .unwrap();
    let err = assets_cmd::assets_update(&env.ctx, update)
        .await
        .expect_err("Quell-Kosten tauschen");
    assert!(err.to_string().contains("Quell-Kosten"), "{err}");

    // Aktivierte Kosten: kein Storno.
    let cancel: expenses_cmd::ExpensesCancelArgs = serde_json::from_value(serde_json::json!({
        "args": { "expenseId": expense, "reason": "Kauf rückabgewickelt" }
    }))
    .unwrap();
    let err = expenses_cmd::expenses_cancel(&env.ctx, cancel)
        .await
        .expect_err("aktivierte Kosten stornieren");
    assert!(err.to_string().contains("als Anlage"), "{err}");
    let status: String = sqlx::query_scalar("SELECT status FROM expenses WHERE id = ?")
        .bind(&expense)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(status, "recorded");

    // Stornierte Kosten lassen sich nicht aktivieren.
    let cancel: expenses_cmd::ExpensesCancelArgs = serde_json::from_value(serde_json::json!({
        "args": { "expenseId": other, "reason": "Doppelt erfasst, storniert" }
    }))
    .unwrap();
    expenses_cmd::expenses_cancel(&env.ctx, cancel)
        .await
        .unwrap();
    let err = create_asset(&env, asset_from(&other, now, 119_000))
        .await
        .expect_err("stornierte Kosten");
    assert!(err.to_string().contains("Stornierte Kosten"), "{err}");
}

/// Festgeschriebene, bezahlte Ausgangsrechnung direkt in der DB (der
/// Festschreibe-Weg mit PDF/KoSIT ist hier nicht Gegenstand).
async fn insert_paid_invoice(pool: &SqlitePool, date: NaiveDate, paid: NaiveDate) -> String {
    let id = "inv-paid".to_string();
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status, locked_at,
            buyer_name, buyer_country_code
         ) VALUES (?, ?, ?, 'issued', ?, (SELECT id FROM contacts LIMIT 1),
            'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt',
            10000, 10000, 'issued', ?, 'Kunde GmbH', 'DE')",
    )
    .bind(&id)
    .bind(format!("RE-{}-0901", date.year()))
    .bind(date.year() as i64)
    .bind(date.to_string())
    .bind(format!("{date} 10:00:00"))
    .execute(pool)
    .await
    .unwrap();
    // Seit 0034 über das Zahlungsbuch (ADR 0042 Nr. 7).
    klein_buch_lib::db::repo::invoices::record_payment(pool, &id, 10_000, &paid.to_string(), None)
        .await
        .unwrap();
    id
}

fn cancel_args(id: &str, date: NaiveDate) -> invoices_cmd::InvoicesCancelArgs {
    serde_json::from_value(serde_json::json!({
        "args": { "invoiceId": id, "reason": "Falscher Empfänger", "stornoDate": date }
    }))
    .unwrap()
}

/// S2 (§ 11 EStG): Die Erstattung steht am Storno-Datum in der EÜR; vor dem
/// letzten Zahlungseingang lehnt der Storno ab. S3: Ab dem Zahlungstag
/// entsteht der Storno-Entwurf samt Audit `invoice.draft_create` (mit
/// `storno_for`) in einer Transaktion — auch wenn das Festschreiben danach
/// scheitert (hier ohne Sidecar/Vorlagen).
#[tokio::test]
async fn storno_date_not_before_payment_and_storno_draft_is_audited() {
    let env = setup().await;
    let now = today();
    let invoice_date = now - chrono::Days::new(20);
    let paid = now - chrono::Days::new(5);
    let id = insert_paid_invoice(&env.pool, invoice_date, paid).await;

    let err =
        invoices_cmd::invoices_cancel(&env.ctx, cancel_args(&id, now - chrono::Days::new(10)))
            .await
            .expect_err("Storno vor dem Zahlungseingang");
    let msg = err.to_string();
    assert!(
        msg.contains("vor dem letzten Zahlungseingang")
            && msg.contains(&paid.format("%d.%m.%Y").to_string()),
        "{msg}"
    );
    assert_eq!(
        scalar(
            &env.pool,
            "SELECT COUNT(*) FROM invoices WHERE is_storno_for = 'inv-paid'"
        )
        .await,
        0,
        "kein Storno-Entwurf, keine Nummer"
    );

    // Am Zahlungstag: Entwurf + Audit entstehen, das Festschreiben scheitert
    // hier mangels Vorlagen/Sidecar.
    let _ = invoices_cmd::invoices_cancel(&env.ctx, cancel_args(&id, paid)).await;
    let storno: String =
        sqlx::query_scalar("SELECT id FROM invoices WHERE is_storno_for = 'inv-paid'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let audit: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'invoice.draft_create' AND entity_id = ?",
    )
    .bind(&storno)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(audit.contains("\"storno_for\":\"inv-paid\""), "{audit}");
}

fn invoice_input(date: NaiveDate, price: i64) -> InvoiceInput {
    InvoiceInput {
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        direction: InvoiceDirection::Issued,
        invoice_date: date,
        delivery_date: Some(date),
        due_date: date.checked_add_days(chrono::Days::new(14)),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 1.0,
            unit_code: "HUR".into(),
            unit_price_cents: price,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

/// S3: Angebotsumwandlung — Entwurf, `converted` und Audit gemeinsam; eine
/// zweite Umwandlung scheitert ohne zweiten Entwurf.
#[tokio::test]
async fn quote_conversion_commits_draft_and_status_together() {
    let env = setup().await;
    let now = today();
    sqlx::query(
        "INSERT INTO quotes (
            id, quote_number, fiscal_year, quote_date, valid_until, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents
         ) VALUES ('q1', ?, ?, ?, ?, ?,
            'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt', 10000, 10000)",
    )
    .bind(format!("AN-{}-0901", now.year()))
    .bind(now.year() as i64)
    .bind(now.to_string())
    .bind((now + chrono::Days::new(30)).to_string())
    .bind(&env.contact)
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE quotes SET status = 'sent', locked_at = ?, sent_at = ? WHERE id = 'q1'")
        .bind(format!("{now} 10:00:00"))
        .bind(format!("{now} 10:00:00"))
        .execute(&env.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE quotes SET status = 'accepted', accepted_at = ? WHERE id = 'q1'")
        .bind(format!("{now} 11:00:00"))
        .execute(&env.pool)
        .await
        .unwrap();

    let args = || -> quotes_cmd::QuotesConvertToInvoiceArgs {
        serde_json::from_value(serde_json::json!({ "args": {
            "quoteId": "q1", "invoiceDate": now, "deliveryDate": now, "dueDate": null,
            "pdfTemplate": null, "notes": null, "paymentNote": null,
            "items": [{
                "position": 1, "description": "Beratung", "quantity": 1.0,
                "unitCode": "HUR", "unitPriceCents": 10000, "taxRatePercent": 0.0,
                "taxCategoryCode": "E"
            }]
        }}))
        .unwrap()
    };
    let detail = quotes_cmd::quotes_convert_to_invoice(&env.ctx, args())
        .await
        .unwrap();
    let (status, converted): (String, Option<String>) =
        sqlx::query_as("SELECT status, converted_invoice_id FROM quotes WHERE id = 'q1'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(status, "converted");
    assert_eq!(converted.as_deref(), Some(detail.invoice.id.as_str()));
    assert_eq!(
        detail.invoice.derived_from_quote_id.as_deref(),
        Some("q1"),
        "Entwurf mit Bezug"
    );
    assert_eq!(
        scalar(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action IN ('quote.converted', 'invoice.draft_create')"
        )
        .await,
        2
    );

    assert!(quotes_cmd::quotes_convert_to_invoice(&env.ctx, args())
        .await
        .is_err());
    assert_eq!(
        scalar(
            &env.pool,
            "SELECT COUNT(*) FROM invoices WHERE derived_from_quote_id = 'q1'"
        )
        .await,
        1,
        "kein zweiter Entwurf"
    );
}

/// S3 (§ 14 Abs. 4 Nr. 4 UStG): Ein nach dem Restore bestätigter Entwurf wird
/// nach jeder Änderung wieder vorgemerkt und lässt sich erst nach erneuter
/// Bestätigung festschreiben.
#[tokio::test]
async fn confirmed_restore_conflict_is_flagged_again_after_edit() {
    let env = setup().await;
    let now = today();
    let draft = invoices_cmd::create_invoice_draft_from_input(
        &env.pool,
        &env.contact,
        &invoice_input(now, 10_000),
        None,
    )
    .await
    .unwrap()
    .invoice;
    let key = format!("{}invoice:{}", after_restore::CONFLICT_PREFIX, draft.id);
    app_settings::set(&env.pool, &key, &draft.invoice_number)
        .await
        .unwrap();
    assert!(
        after_restore::ensure_no_conflict(&env.pool, "invoice", &draft.id)
            .await
            .is_err()
    );
    after_restore::confirm_conflict(
        &env.pool,
        "invoice",
        &draft.id,
        "Entspricht dem versandten Original",
    )
    .await
    .unwrap();
    after_restore::ensure_no_conflict(&env.pool, "invoice", &draft.id)
        .await
        .unwrap();

    // Preis ändern → wieder vorgemerkt, Audit.
    invoices_cmd::update_draft_core(
        &env.pool,
        &draft.id,
        &env.contact,
        invoice_input(now, 25_000),
        now,
    )
    .await
    .unwrap();
    assert!(
        after_restore::ensure_no_conflict(&env.pool, "invoice", &draft.id)
            .await
            .is_err()
    );
    assert_eq!(
        scalar(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'numbering.restored_draft_reflagged'"
        )
        .await,
        1
    );

    // Ein Entwurf ohne Restore-Konflikt bleibt nach dem Bearbeiten frei.
    let plain = invoices_cmd::create_invoice_draft_from_input(
        &env.pool,
        &env.contact,
        &invoice_input(now, 10_000),
        None,
    )
    .await
    .unwrap()
    .invoice;
    invoices_cmd::update_draft_core(
        &env.pool,
        &plain.id,
        &env.contact,
        invoice_input(now, 12_000),
        now,
    )
    .await
    .unwrap();
    after_restore::ensure_no_conflict(&env.pool, "invoice", &plain.id)
        .await
        .unwrap();
}

/// S2 (§ 6 Abs. 1 Nr. 4 EStG): Privatentnahme einer Anlage mit Entnahmewert —
/// er steht als Erlös im Abgang (Betriebseinnahme Kz 102), negativ wird
/// abgelehnt.
#[tokio::test]
async fn private_withdrawal_of_asset_records_withdrawal_value() {
    let env = setup().await;
    let now = today();
    let acquired = d(now.year(), 1, 2);
    let args: assets_cmd::AssetsCreateArgs = serde_json::from_value(serde_json::json!({
        "input": {
            "label": "Kamera", "acquisitionDate": acquired, "acquisitionCostCents": 120_000,
            "expenseId": null, "vendorContactId": null, "depreciationMethod": "linear",
            "usefulLifeYears": 3.0, "afaCategory": null, "businessSharePercent": 100.0,
            "notes": null
        }
    }))
    .unwrap();
    let asset = assets_cmd::assets_create(&env.ctx, args)
        .await
        .unwrap()
        .asset
        .id;
    let dispose = |proceeds: i64| -> assets_cmd::AssetsDisposeArgs {
        serde_json::from_value(serde_json::json!({ "args": {
            "assetId": asset, "disposalDate": now, "disposalType": "given_away",
            "proceedsCents": proceeds
        }}))
        .unwrap()
    };
    assert!(assets_cmd::assets_dispose(&env.ctx, dispose(-1))
        .await
        .is_err());
    let detail = assets_cmd::assets_dispose(&env.ctx, dispose(30_000))
        .await
        .unwrap();
    assert_eq!(detail.asset.disposal_type.as_deref(), Some("given_away"));
    assert_eq!(detail.asset.disposal_proceeds_cents, Some(30_000));
    let items = klein_buch_lib::db::repo::euer::disposal_detail(&env.pool, now.year())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].disposal_type, "given_away");
    assert_eq!(items[0].proceeds_cents, 30_000);
}
