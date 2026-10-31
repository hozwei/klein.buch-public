//! Lauf F, Block F2b — Geschäftsjahr-Abschluss gegen Buchungen.
//!
//! * KB-0032: Nach dem Abschluss eines Geschäftsjahres lehnen alle buchenden
//!   Commands ein Datum in diesem Jahr ab (Aufruf über den `AppCtx` wie im
//!   Server); dieselben Commands im offenen Jahr gehen durch.
//! * R2-S2-GJLOCK / W14-034: Der Abschluss wartet auf laufende Buchungen,
//!   Buchungen warten auf einen laufenden Abschluss und prüfen danach erneut;
//!   der Abschluss läuft in einer Transaktion und schreibt Belegzahlen und
//!   offene Entwürfe ins Audit.
//!
//! Tempfile-DB mit den echten Migrationen; Testdaten nur mit
//! Dokumentations-Adressen (example.org).

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::{assets as assets_cmd, expenses as expenses_cmd};
use klein_buch_lib::commands::{depreciation as dep_cmd, quotes as quotes_cmd};
use klein_buch_lib::commands::{invoices as invoices_cmd, private_movements as pm_cmd};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::asset::AssetInput;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::domain::private_movement::PrivateMovementInput;
use klein_buch_lib::error::Result;
use klein_buch_lib::fiscal_year::{guard, lock};
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
    std::fs::create_dir_all(dir.join("inputs")).unwrap();
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
    // Session gesperrt (keine Auto-Backups je Buchung); Abschluss und AfA
    // entsperren sie bei Bedarf.
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

fn expense_input(date: NaiveDate, paid: Option<NaiveDate>) -> ExpenseInput {
    ExpenseInput {
        expense_date: date,
        paid_date: paid,
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "Bürobedarf Beispiel".into(),
        vendor_invoice_number: None,
        category: "office".into(),
        description: "Papier".into(),
        net_amount_cents: 1_190,
        tax_amount_cents: 0,
        gross_amount_cents: 1_190,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    }
}

fn asset_input(date: NaiveDate) -> AssetInput {
    AssetInput {
        label: "Notebook".into(),
        acquisition_date: date,
        acquisition_cost_cents: 150_000,
        expense_id: None,
        vendor_contact_id: None,
        depreciation_method: "linear".into(),
        useful_life_years: Some(3.0),
        afa_category: None,
        business_share_percent: 100.0,
        notes: None,
    }
}

fn invoice_input(date: NaiveDate) -> InvoiceInput {
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
            unit_price_cents: 10_000,
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

async fn create_expense(env: &Env, date: NaiveDate, paid: Option<NaiveDate>) -> Result<String> {
    let input = expense_input(date, paid);
    let args: expenses_cmd::ExpensesCreateArgs = serde_json::from_value(serde_json::json!({
        "args": { "input": input }
    }))
    .unwrap();
    Ok(expenses_cmd::expenses_create(&env.ctx, args)
        .await?
        .expense
        .id)
}

async fn create_asset(env: &Env, date: NaiveDate) -> Result<String> {
    let args: assets_cmd::AssetsCreateArgs =
        serde_json::from_value(serde_json::json!({ "input": asset_input(date) })).unwrap();
    Ok(assets_cmd::assets_create(&env.ctx, args).await?.asset.id)
}

/// Festgeschriebene Ausgangsrechnung bzw. Angebot direkt in der DB (der
/// Festschreibe-Weg mit PDF/KoSIT ist hier nicht Gegenstand).
async fn insert_locked_invoice(pool: &SqlitePool, id: &str, number: &str, date: NaiveDate) {
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
    .bind(id)
    .bind(number)
    .bind(date.year() as i64)
    .bind(date.to_string())
    .bind(format!("{date} 10:00:00"))
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_quote(pool: &SqlitePool, id: &str, number: &str, date: NaiveDate, sent: bool) {
    sqlx::query(
        "INSERT INTO quotes (
            id, quote_number, fiscal_year, quote_date, valid_until, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents
         ) VALUES (?, ?, ?, ?, ?, (SELECT id FROM contacts LIMIT 1),
            'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt', 10000, 10000)",
    )
    .bind(id)
    .bind(number)
    .bind(date.year() as i64)
    .bind(date.to_string())
    .bind((date + chrono::Days::new(30)).to_string())
    .execute(pool)
    .await
    .unwrap();
    if sent {
        sqlx::query("UPDATE quotes SET status = 'sent', locked_at = ?, sent_at = ? WHERE id = ?")
            .bind(format!("{date} 10:00:00"))
            .bind(format!("{date} 10:00:00"))
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }
}

fn assert_closed<T: std::fmt::Debug>(what: &str, res: Result<T>, year: i32) {
    let err = res.expect_err(what);
    let msg = err.to_string();
    assert!(
        msg.contains(&format!("Geschäftsjahr {year} ist abgeschlossen")),
        "{what}: {msg}"
    );
}

/// KB-0032: alle buchenden Commands nach dem Abschluss.
#[tokio::test]
async fn booking_commands_refuse_dates_in_a_closed_year() {
    let env = setup().await;
    let now = today();
    let cy = now.year();
    let closed = cy - 1;

    // Bestand im (noch offenen) Vorjahr.
    let expense = create_expense(&env, d(closed, 6, 1), None).await.unwrap();
    let asset = create_asset(&env, d(closed, 3, 1)).await.unwrap();
    insert_locked_invoice(
        &env.pool,
        "inv-closed",
        &format!("RE-{closed}-0901"),
        d(closed, 5, 20),
    )
    .await;
    let draft = invoices_cmd::create_invoice_draft_from_input(
        &env.pool,
        &env.contact,
        &invoice_input(d(closed, 11, 3)),
        None,
    )
    .await
    .unwrap()
    .invoice
    .id;
    insert_quote(
        &env.pool,
        "q-draft",
        &format!("AN-{closed}-0901"),
        d(closed, 4, 1),
        false,
    )
    .await;
    insert_quote(
        &env.pool,
        "q-sent",
        &format!("AN-{closed}-0902"),
        d(closed, 4, 2),
        true,
    )
    .await;

    env.ctx.session.set(PASS.into());
    let lock = lock::close_year(
        &env.pool,
        &env.ctx.paths,
        &env.ctx.session,
        closed as i64,
        klein_buch_lib::clock::berlin_to_utc(now, 12, 0),
    )
    .await
    .unwrap();
    env.ctx.session.clear();
    // Offene Entwürfe des Jahres in der Antwort (Rechnungs- und Angebots-Entwurf).
    assert_eq!(
        lock.open_drafts,
        Some(lock::OpenDrafts {
            invoices: 1,
            quotes: 1
        })
    );

    // Kosten: Belegdatum, Zahlungsdatum, Storno und Zahlung im geschlossenen Jahr.
    assert_closed(
        "expenses_create (Belegdatum)",
        create_expense(&env, d(closed, 12, 31), None).await,
        closed,
    );
    assert_closed(
        "expenses_create (Zahlungsdatum)",
        create_expense(&env, now, Some(d(closed, 12, 30))).await,
        closed,
    );
    let set_payment = |paid: NaiveDate| {
        serde_json::from_value::<expenses_cmd::ExpensesSetPaymentArgs>(serde_json::json!({
            "args": { "expenseId": expense, "paidDate": paid, "paidFromAccountId": null }
        }))
        .unwrap()
    };
    assert_closed(
        "expenses_set_payment",
        expenses_cmd::expenses_set_payment(&env.ctx, set_payment(d(closed, 12, 15))).await,
        closed,
    );
    let cancel: expenses_cmd::ExpensesCancelArgs = serde_json::from_value(serde_json::json!({
        "args": { "expenseId": expense, "reason": "Doppelt erfasst, bitte stornieren" }
    }))
    .unwrap();
    assert_closed(
        "expenses_cancel",
        expenses_cmd::expenses_cancel(&env.ctx, cancel).await,
        closed,
    );

    // Anlagen: Anlegen, Ändern, Abgang im geschlossenen Jahr.
    assert_closed(
        "assets_create",
        create_asset(&env, d(closed, 7, 1)).await,
        closed,
    );
    // Eine Anlage des geschlossenen Jahres ohne AfA-Buchung (von außen
    // eingefügt, am Guard vorbei) — Ändern scheitert am Jahr, nicht an der AfA.
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_fiscal_year,
            acquisition_cost_cents, depreciation_method, useful_life_years,
            business_share_percent, book_value_cents)
         VALUES ('a-raw', ?, 'Drucker', ?, ?, 60000, 'linear', 3.0, 100.0, 60000)",
    )
    .bind(format!("AV-{closed}-0901"))
    .bind(d(closed, 9, 1).to_string())
    .bind(closed as i64)
    .execute(&env.pool)
    .await
    .unwrap();
    let update: assets_cmd::AssetsUpdateArgs = serde_json::from_value(serde_json::json!({
        "id": "a-raw", "input": asset_input(d(closed, 9, 2))
    }))
    .unwrap();
    assert_closed(
        "assets_update",
        assets_cmd::assets_update(&env.ctx, update).await,
        closed,
    );
    let dispose = |date: NaiveDate| {
        serde_json::from_value::<assets_cmd::AssetsDisposeArgs>(serde_json::json!({
            "args": { "assetId": asset, "disposalDate": date, "disposalType": "sale",
                      "proceedsCents": 10_000 }
        }))
        .unwrap()
    };
    assert_closed(
        "assets_dispose",
        assets_cmd::assets_dispose(&env.ctx, dispose(d(closed, 12, 31))).await,
        closed,
    );

    // Privatbewegung.
    let pm = |date: NaiveDate| {
        let input = PrivateMovementInput {
            movement_date: date,
            movement_type: "entnahme".into(),
            amount_cents: 5_000,
            account_id: None,
            description: "Privatentnahme".into(),
            notes: None,
        };
        serde_json::from_value::<pm_cmd::PrivateMovementsCreateArgs>(serde_json::json!({
            "args": { "input": input }
        }))
        .unwrap()
    };
    assert_closed(
        "private_movements_create",
        pm_cmd::private_movements_create(&env.ctx, pm(d(closed, 8, 1))).await,
        closed,
    );

    // Rechnungen: Entwurf, Festschreiben, Zahlung, Storno-Datum.
    let create_draft = |date: NaiveDate| {
        serde_json::from_value::<invoices_cmd::InvoicesCreateDraftArgs>(serde_json::json!({
            "args": { "contactId": env.contact, "input": invoice_input(date) }
        }))
        .unwrap()
    };
    assert_closed(
        "invoices_create_draft",
        invoices_cmd::invoices_create_draft(&env.ctx, create_draft(d(closed, 12, 1))).await,
        closed,
    );
    let lock_args: invoices_cmd::InvoicesLockAndIssueArgs =
        serde_json::from_value(serde_json::json!({ "id": draft })).unwrap();
    assert_closed(
        "invoices_lock_and_issue",
        invoices_cmd::invoices_lock_and_issue(&env.ctx, lock_args).await,
        closed,
    );
    let pay = |date: NaiveDate| {
        serde_json::from_value::<invoices_cmd::InvoicesRecordPaymentArgs>(serde_json::json!({
            "args": { "invoiceId": "inv-closed", "amountCents": 5_000, "paidDate": date,
                      "note": null }
        }))
        .unwrap()
    };
    assert_closed(
        "invoices_record_payment",
        invoices_cmd::invoices_record_payment(&env.ctx, pay(d(closed, 12, 15))).await,
        closed,
    );
    let cancel_inv: invoices_cmd::InvoicesCancelArgs = serde_json::from_value(serde_json::json!({
        "args": { "invoiceId": "inv-closed", "reason": "Falscher Empfänger angegeben",
                  "stornoDate": d(closed, 12, 20) }
    }))
    .unwrap();
    assert_closed(
        "invoices_cancel (Storno-Datum)",
        invoices_cmd::invoices_cancel(&env.ctx, cancel_inv).await,
        closed,
    );

    // Angebote: Festschreiben und Annahme mit Vertrag (Archiv des Jahres).
    let issue: quotes_cmd::QuotesIssueArgs =
        serde_json::from_value(serde_json::json!({ "id": "q-draft" })).unwrap();
    assert_closed(
        "quotes_issue",
        quotes_cmd::quotes_issue(&env.ctx, issue).await,
        closed,
    );
    let upload = env
        .ctx
        .uploads
        .put_bytes(
            klein_buch_lib::uploads::UploadKind::Contract,
            "vertrag.pdf",
            b"%PDF-1.7\n%%EOF\n",
        )
        .unwrap();
    let accept: quotes_cmd::QuotesAcceptArgs = serde_json::from_value(serde_json::json!({
        "args": { "quoteId": "q-sent", "acceptedDate": d(closed, 5, 1),
                  "signedContractUploadId": upload.upload_id, "attachmentLabel": null }
    }))
    .unwrap();
    assert_closed(
        "quotes_accept (mit Vertrag)",
        quotes_cmd::quotes_accept(&env.ctx, accept).await,
        closed,
    );

    // AfA: der (von außen eingefügte) Bestand ohne AfA im geschlossenen Jahr
    // bekommt dort keine Buchung — die Prüfung liegt in der Transaktion.
    let accrue: dep_cmd::DepreciationAccrueYearArgs =
        serde_json::from_value(serde_json::json!({ "fiscalYear": closed })).unwrap();
    env.ctx.session.set(PASS.into());
    let report = dep_cmd::depreciation_accrue_year(&env.ctx, accrue)
        .await
        .unwrap();
    env.ctx.session.clear();
    assert!(!report.skipped_locked);
    assert_eq!(report.booked_entries, 0, "keine AfA ins geschlossene Jahr");

    // Gegenprobe im offenen Jahr: dieselben Commands gehen durch.
    create_expense(&env, now, Some(now)).await.unwrap();
    expenses_cmd::expenses_set_payment(&env.ctx, set_payment(now))
        .await
        .unwrap();
    pm_cmd::private_movements_create(&env.ctx, pm(now))
        .await
        .unwrap();
    invoices_cmd::invoices_create_draft(&env.ctx, create_draft(now))
        .await
        .unwrap();
    invoices_cmd::invoices_record_payment(&env.ctx, pay(now))
        .await
        .unwrap();
    assets_cmd::assets_dispose(&env.ctx, dispose(now))
        .await
        .unwrap();
}

/// R2-S2-GJLOCK: der Abschluss beginnt erst, wenn keine Buchung mehr läuft.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn close_waits_for_a_running_booking() {
    let env = setup().await;
    let now = today();
    let closed = now.year() - 1;
    create_expense(&env, d(closed, 2, 1), Some(d(closed, 2, 1)))
        .await
        .unwrap();

    let permit = guard::ensure_year_open(&env.pool, closed as i64)
        .await
        .unwrap();
    let close = {
        let (pool, ctx) = (env.pool.clone(), env.ctx.clone());
        tokio::spawn(async move {
            ctx.session.set(PASS.into());
            lock::close_year(
                &pool,
                &ctx.paths,
                &ctx.session,
                closed as i64,
                klein_buch_lib::clock::berlin_to_utc(now, 12, 0),
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!close.is_finished(), "Abschluss wartet auf die Buchung");
    assert!(!guard::is_closed(&env.pool, closed as i64).await.unwrap());
    // Die laufende Buchung schreibt noch in das (offene) Jahr …
    create_expense(&env, d(closed, 3, 1), Some(d(closed, 3, 1)))
        .await
        .unwrap();
    drop(permit);
    let lock = tokio::time::timeout(Duration::from_secs(60), close)
        .await
        .expect("Abschluss beginnt nach der Buchung")
        .unwrap()
        .unwrap();
    // … und steht im Snapshot.
    assert_eq!(lock.expense_total_cents, 2 * 1_190);
}

/// R2-S2-GJLOCK: eine Buchung, die während des Abschlusses ankommt, wartet und
/// prüft danach erneut — sie landet nie unbemerkt im geschlossenen Jahr.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn booking_during_close_waits_and_rechecks() {
    let env = setup().await;
    let closed = today().year() - 1;
    let closing = guard::closing().await;
    let booking = {
        let pool = env.pool.clone();
        tokio::spawn(async move {
            guard::ensure_year_open(&pool, closed as i64)
                .await
                .map(drop)
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!booking.is_finished(), "Buchung wartet auf den Abschluss");
    // Der Abschluss schreibt sein Protokoll …
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
            afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (?, 0, 0, 0, 0, 'test', 32)",
    )
    .bind(closed as i64)
    .execute(&env.pool)
    .await
    .unwrap();
    drop(closing);
    // … und die wartende Buchung scheitert an der erneuten Prüfung.
    let res = tokio::time::timeout(Duration::from_secs(10), booking)
        .await
        .unwrap()
        .unwrap();
    assert_closed("Buchung nach dem Abschluss", res, closed);
}

/// Audit des Abschlusses: Eckwerte, Belegzahlen, offene Entwürfe; gültiges JSON.
#[tokio::test]
async fn close_audit_carries_document_counts_and_open_drafts() {
    let env = setup().await;
    let now = today();
    let closed = now.year() - 1;
    create_expense(&env, d(closed, 2, 1), Some(d(closed, 2, 1)))
        .await
        .unwrap();
    insert_locked_invoice(
        &env.pool,
        "inv-a",
        &format!("RE-{closed}-0901"),
        d(closed, 5, 20),
    )
    .await;
    invoices_cmd::create_invoice_draft_from_input(
        &env.pool,
        &env.contact,
        &invoice_input(d(closed, 11, 3)),
        None,
    )
    .await
    .unwrap();

    // Übersicht vor dem Abschluss: der offene Entwurf wird gemeldet.
    let overview = klein_buch_lib::commands::fiscal_year::fiscal_year_overview(
        &env.ctx,
        serde_json::from_value(serde_json::json!({})).unwrap(),
    )
    .await
    .unwrap();
    let year = overview
        .years
        .iter()
        .find(|y| y.fiscal_year == closed as i64)
        .unwrap();
    assert_eq!(year.open_drafts.invoices, 1);
    assert_eq!(year.snapshot_matches, None, "offenes Jahr ohne Snapshot");

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
    let details: String =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'fiscal_year.close'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(v["expenses"], 1_190);
    assert_eq!(v["documents"]["invoices"], 1);
    assert_eq!(v["documents"]["expenses"], 1);
    assert_eq!(v["documents"]["stornos"], 0);
    assert_eq!(v["open_drafts"]["invoices"], 1);
    assert_eq!(v["open_drafts"]["quotes"], 0);

    // Übersicht danach: Snapshot stimmt mit der Live-Rechnung überein.
    let overview = klein_buch_lib::commands::fiscal_year::fiscal_year_overview(
        &env.ctx,
        serde_json::from_value(serde_json::json!({})).unwrap(),
    )
    .await
    .unwrap();
    let year = overview
        .years
        .iter()
        .find(|y| y.fiscal_year == closed as i64)
        .unwrap();
    assert!(year.closed);
    assert_eq!(year.snapshot_matches, Some(true));
}
