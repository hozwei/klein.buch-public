//! Lauf 2026.11.1 V4 — Angebots-Entwürfe bearbeiten, Restore-Hinweis vor dem
//! Speichern, Rückstand-Vorschau (ADR 0042 Nr. 13 und 16), gegen eine
//! Tempfile-SQLite mit dem echten `MIGRATOR`:
//!
//!   * `quotes_update_draft`: Erfolg (Nummer bleibt, Positionen ersetzt, Audit),
//!     Ablehnung nach Fertigstellen/Verwerfen, in einem anderen oder
//!     abgeschlossenen Geschäftsjahr, bei anonymisiertem Empfänger und bei
//!     USt-Ausweis unter §19; Trigger unverändert (Raw-SQL bleibt gesperrt);
//!   * nach einem Restore vorgemerkte Entwürfe bleiben gesperrt, ein bestätigter
//!     wird mit dem Speichern wieder vorgemerkt;
//!   * Lesefeld `restoreConflict` in `invoices_get` und `quotes_get`;
//!   * `recurring_backlog_preview` rechnet wie `apply_resume_mode` und
//!     `has_backlog` (Vorschau = Lauf).
//!
//! Testdaten nur mit Dokumentations-Adressen (example.org).

use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::backup::after_restore;
use klein_buch_lib::commands::invoices as inv_cmd;
use klein_buch_lib::commands::quotes::{self as quotes_cmd, CreateDraftArgs};
use klein_buch_lib::commands::recurring::{self as rec_cmd, BacklogSeries};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::models::RestoreConflictState;
use klein_buch_lib::db::repo::{app_settings, contacts, quotes, recurring, recurring_invoice};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::domain::quote::{QuoteInput, QuoteItemInput};
use klein_buch_lib::scheduler::catch_up::{self, ResumeMode, Series};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

struct Env {
    pool: SqlitePool,
    ctx: Arc<AppCtx>,
    contact: String,
    _dir: tempfile::TempDir,
}

async fn setup() -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::under(dir.path());
    for d in [&paths.archive_dir, &paths.backups_dir] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::create_dir_all(paths.data_dir.join("tmp")).unwrap();
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
    klein_buch_lib::db::repo::seller_profile::upsert(
        &pool,
        &klein_buch_lib::db::repo::seller_profile::SellerProfileInput {
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
    let contact = mk_contact(&pool, "Kunde Beispiel").await;
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    Env {
        pool,
        ctx,
        contact,
        _dir: dir,
    }
}

async fn mk_contact(pool: &SqlitePool, name: &str) -> String {
    contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: name.into(),
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
    .id
}

fn today() -> NaiveDate {
    klein_buch_lib::clock::today_berlin()
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn qitem(position: u32, qty: f64, price: i64, code: &str, rate: f64) -> QuoteItemInput {
    QuoteItemInput {
        position,
        description: format!("Leistung {position}"),
        quantity: qty,
        unit_code: "C62".into(),
        unit_price_cents: price,
        tax_rate_percent: rate,
        tax_category_code: code.into(),
        description_title: None,
        description_markup: None,
        source_package_id: None,
        source_package_revision: None,
    }
}

fn qinput(items: Vec<QuoteItemInput>) -> QuoteInput {
    QuoteInput {
        quote_date: today(),
        valid_until: today() + chrono::Duration::days(30),
        currency_code: "EUR".into(),
        items,
        notes: None,
        pdf_template: "default".into(),
    }
}

async fn draft(env: &Env) -> String {
    quotes_cmd::create_draft_core(
        &env.pool,
        CreateDraftArgs {
            contact_id: env.contact.clone(),
            input: qinput(vec![qitem(1, 2.0, 25_000, "E", 0.0)]),
        },
    )
    .await
    .unwrap()
    .quote
    .id
}

async fn update(
    env: &Env,
    id: &str,
    contact: &str,
    input: QuoteInput,
) -> klein_buch_lib::error::Result<klein_buch_lib::db::models::QuoteDetail> {
    quotes_cmd::update_draft_core(
        &env.pool,
        id,
        CreateDraftArgs {
            contact_id: contact.to_string(),
            input,
        },
        today(),
    )
    .await
}

async fn issue(env: &Env, id: &str) -> klein_buch_lib::error::Result<()> {
    quotes_cmd::quotes_issue(&env.ctx, quotes_cmd::QuotesIssueArgs { id: id.into() })
        .await
        .map(|_| ())
}

async fn quote_state(env: &Env, id: &str) -> Option<RestoreConflictState> {
    quotes_cmd::quotes_get(&env.ctx, quotes_cmd::QuotesGetArgs { id: id.into() })
        .await
        .unwrap()
        .unwrap()
        .restore_conflict
}

async fn count(pool: &SqlitePool, action: &str, id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = ? AND entity_id = ?")
        .bind(action)
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ============================================================================
// (1) Angebots-Entwürfe bearbeiten (ADR 0042 Nr. 13)
// ============================================================================

#[tokio::test]
async fn update_draft_replaces_items_and_keeps_number() {
    let env = setup().await;
    let id = draft(&env).await;
    let before = quotes::get(&env.pool, &id).await.unwrap().unwrap();
    let other = mk_contact(&env.pool, "Andere Kundin").await;
    let mut changed = qinput(vec![
        qitem(1, 0.5, 33_333, "E", 0.0),
        qitem(2, 1.0, 5_000, "E", 0.0),
    ]);
    changed.notes = Some("geändert".into());
    changed.valid_until = today() + chrono::Duration::days(14);
    let detail = update(&env, &id, &other, changed)
        .await
        .expect("bearbeiten");
    assert_eq!(detail.quote.quote_number, before.quote_number);
    assert_eq!(detail.quote.fiscal_year, before.fiscal_year);
    assert_eq!(detail.quote.contact_id, other);
    assert_eq!(detail.quote.buyer_name.as_deref(), Some("Andere Kundin"));
    assert_eq!(detail.quote.notes.as_deref(), Some("geändert"));
    assert_eq!(detail.quote.status, "draft");
    assert!(detail.quote.locked_at.is_none());
    assert_eq!(detail.items.len(), 2);
    // 0,5 × 333,33 € = 166,665 € → 166,67 € (kaufmännisch) + 50,00 €.
    assert_eq!(detail.quote.gross_amount_cents, 21_667);
    assert_eq!(detail.quote.tax_amount_cents, 0);
    assert_eq!(detail.quote.is_kleinunternehmer, 1);
    assert_eq!(count(&env.pool, "quote.draft_updated", &id).await, 1);
    // Der bearbeitete Entwurf lässt sich fertigstellen.
    issue(&env, &id)
        .await
        .expect("Fertigstellen nach dem Bearbeiten");
}

#[tokio::test]
async fn update_draft_rejects_closed_states_other_year_and_blockers() {
    let env = setup().await;
    let good = || qinput(vec![qitem(1, 1.0, 10_000, "E", 0.0)]);
    let id = draft(&env).await;

    // Anderes Geschäftsjahr als die Nummer: Fehler am Feld Angebotsdatum.
    let mut other_year = good();
    other_year.quote_date = d(today().year() - 1, 12, 1);
    other_year.valid_until = other_year.quote_date + chrono::Duration::days(30);
    let err = update(&env, &id, &env.contact, other_year)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Das Angebotsdatum muss im Geschäftsjahr"),
        "{err}"
    );

    // §14c-Schutz: USt-Ausweis unter §19 schon beim Speichern abgelehnt.
    let err = update(
        &env,
        &id,
        &env.contact,
        qinput(vec![qitem(1, 1.0, 10_000, "S", 19.0)]),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("§19"), "{err}");

    // Keine Positionen.
    let err = update(&env, &id, &env.contact, qinput(vec![]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("nicht gespeichert"), "{err}");

    // Anonymisierter Empfänger.
    let anon = mk_contact(&env.pool, "Wird anonymisiert").await;
    sqlx::query("UPDATE contacts SET anonymized_at = datetime('now','utc') WHERE id = ?")
        .bind(&anon)
        .execute(&env.pool)
        .await
        .unwrap();
    let err = update(&env, &id, &anon, good())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("anonymisiert"), "{err}");

    // Nichts davon hat den Entwurf geändert.
    assert_eq!(count(&env.pool, "quote.draft_updated", &id).await, 0);
    let unchanged = quotes::get_items(&env.pool, &id).await.unwrap();
    assert_eq!(unchanged.len(), 1);
    assert_eq!(unchanged[0].unit_price_cents, 25_000);

    // Fertiggestellt.
    issue(&env, &id).await.unwrap();
    let err = update(&env, &id, &env.contact, good())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("fertiggestellt"), "{err}");

    // Verworfen (zurückgezogener Entwurf).
    let voided = draft(&env).await;
    quotes_cmd::quotes_cancel(
        &env.ctx,
        quotes_cmd::QuotesCancelArgs {
            args: quotes_cmd::CancelArgs {
                quote_id: voided.clone(),
                reason: "Testfall: doppelt angelegt".into(),
            },
        },
    )
    .await
    .unwrap();
    let err = update(&env, &voided, &env.contact, good())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("verworfen"), "{err}");

    // Geschlossenes Geschäftsjahr.
    let open = draft(&env).await;
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
                afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (?, 0, 0, 0, 0, 'test', 35)",
    )
    .bind(i64::from(today().year()))
    .execute(&env.pool)
    .await
    .unwrap();
    let err = update(&env, &open, &env.contact, good())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("abgeschlossen"), "{err}");
    // Anlegen in einem abgeschlossenen Jahr ebenso.
    let err = quotes_cmd::create_draft_core(
        &env.pool,
        CreateDraftArgs {
            contact_id: env.contact.clone(),
            input: good(),
        },
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("abgeschlossen"), "{err}");
}

/// GoBD: Die Trigger aus 0028/0032 bleiben unverändert — ein fertiggestelltes
/// und ein verworfenes Angebot lassen sich auch per Raw-SQL nicht ändern, das
/// Repo lehnt beide über das `WHERE` ab.
#[tokio::test]
async fn triggers_still_freeze_issued_and_canceled_quotes() {
    let env = setup().await;
    let issued = draft(&env).await;
    issue(&env, &issued).await.unwrap();
    assert!(
        sqlx::query("UPDATE quotes SET gross_amount_cents = 1 WHERE id = ?")
            .bind(&issued)
            .execute(&env.pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM quote_items WHERE quote_id = ?")
        .bind(&issued)
        .execute(&env.pool)
        .await
        .is_err());

    let canceled = draft(&env).await;
    quotes_cmd::quotes_cancel(
        &env.ctx,
        quotes_cmd::QuotesCancelArgs {
            args: quotes_cmd::CancelArgs {
                quote_id: canceled.clone(),
                reason: "Testfall: nicht mehr benötigt".into(),
            },
        },
    )
    .await
    .unwrap();
    assert!(
        sqlx::query("UPDATE quotes SET status = 'draft' WHERE id = ?")
            .bind(&canceled)
            .execute(&env.pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM quote_items WHERE quote_id = ?")
        .bind(&canceled)
        .execute(&env.pool)
        .await
        .is_err());

    // Das Repo selbst: kein Treffer im WHERE → Fehler, nichts geändert.
    for id in [&issued, &canceled] {
        let err = update(
            &env,
            id,
            &env.contact,
            qinput(vec![qitem(1, 1.0, 1, "E", 0.0)]),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, klein_buch_lib::error::Error::Domain(_)));
        let items = quotes::get_items(&env.pool, id).await.unwrap();
        assert_eq!(items[0].unit_price_cents, 25_000);
    }
}

/// ADR 0040 Nr. 13 / ADR 0042 Nr. 13: Ein vorgemerkter Entwurf lässt sich an
/// das versandte Original anpassen, aber erst nach der Bestätigung
/// fertigstellen; ein bestätigter wird mit jeder Änderung wieder vorgemerkt.
#[tokio::test]
async fn restored_quote_draft_stays_flagged_and_confirmed_one_is_reflagged() {
    let env = setup().await;
    let id = draft(&env).await;
    let number = quotes::get(&env.pool, &id)
        .await
        .unwrap()
        .unwrap()
        .quote_number;
    assert_eq!(quote_state(&env, &id).await, None);
    let key = format!("{}quote:{id}", after_restore::CONFLICT_PREFIX);
    app_settings::set(&env.pool, &key, &number).await.unwrap();
    assert_eq!(
        quote_state(&env, &id).await,
        Some(RestoreConflictState::Flagged)
    );

    // Vorgemerkt: Bearbeiten geht, die Sperre bleibt.
    update(
        &env,
        &id,
        &env.contact,
        qinput(vec![qitem(1, 1.0, 30_000, "E", 0.0)]),
    )
    .await
    .expect("Anpassen an das Original");
    assert_eq!(
        quote_state(&env, &id).await,
        Some(RestoreConflictState::Flagged)
    );
    let err = issue(&env, &id).await.unwrap_err().to_string();
    assert!(err.contains("bereits festgeschrieben"), "{err}");

    // Bestätigt: „confirmed“ im Lesefeld, Fertigstellen wäre frei.
    after_restore::confirm_conflict(&env.pool, "quote", &id, "Entspricht dem Original")
        .await
        .unwrap();
    assert_eq!(
        quote_state(&env, &id).await,
        Some(RestoreConflictState::Confirmed)
    );
    let json = serde_json::to_value(
        quotes_cmd::quotes_get(&env.ctx, quotes_cmd::QuotesGetArgs { id: id.clone() })
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(json["restoreConflict"], "confirmed");

    // Speichern nach der Bestätigung merkt wieder vor (Audit), Fertigstellen
    // scheitert erneut.
    update(
        &env,
        &id,
        &env.contact,
        qinput(vec![qitem(1, 1.0, 31_000, "E", 0.0)]),
    )
    .await
    .unwrap();
    assert_eq!(
        quote_state(&env, &id).await,
        Some(RestoreConflictState::Flagged)
    );
    assert_eq!(
        count(&env.pool, "numbering.restored_draft_reflagged", &id).await,
        1
    );
    assert!(issue(&env, &id).await.is_err());

    // Nach erneuter Bestätigung fertiggestellt: kein Lesefeld mehr.
    after_restore::confirm_conflict(&env.pool, "quote", &id, "Erneut abgeglichen")
        .await
        .unwrap();
    issue(&env, &id).await.unwrap();
    assert_eq!(quote_state(&env, &id).await, None);
}

// ============================================================================
// (2) Lesefeld restoreConflict an Rechnungs-Entwürfen (Warteliste 8)
// ============================================================================

fn iinput(price: i64) -> InvoiceInput {
    InvoiceInput {
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        direction: InvoiceDirection::Issued,
        invoice_date: today(),
        delivery_date: Some(today()),
        due_date: Some(today() + chrono::Duration::days(30)),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
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

#[tokio::test]
async fn invoice_restore_conflict_read_field() {
    let env = setup().await;
    let inv =
        inv_cmd::create_invoice_draft_from_input(&env.pool, &env.contact, &iinput(10_000), None)
            .await
            .unwrap()
            .invoice;
    let state = |env: &Env, id: String| {
        let ctx = env.ctx.clone();
        async move {
            inv_cmd::invoices_get(&ctx, inv_cmd::InvoicesGetArgs { id })
                .await
                .unwrap()
                .unwrap()
                .restore_conflict
        }
    };
    assert_eq!(state(&env, inv.id.clone()).await, None);
    let json = serde_json::to_value(
        inv_cmd::invoices_get(&env.ctx, inv_cmd::InvoicesGetArgs { id: inv.id.clone() })
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(json["restoreConflict"].is_null());

    let key = format!("{}invoice:{}", after_restore::CONFLICT_PREFIX, inv.id);
    app_settings::set(&env.pool, &key, &inv.invoice_number)
        .await
        .unwrap();
    assert_eq!(
        state(&env, inv.id.clone()).await,
        Some(RestoreConflictState::Flagged)
    );
    after_restore::confirm_conflict(&env.pool, "invoice", &inv.id, "Entspricht dem Original")
        .await
        .unwrap();
    assert_eq!(
        state(&env, inv.id.clone()).await,
        Some(RestoreConflictState::Confirmed)
    );
    // confirmed → Speichern → flagged (gleiche Quelle wie reflag_confirmed_in).
    inv_cmd::update_draft_core(&env.pool, &inv.id, &env.contact, iinput(12_000), today())
        .await
        .unwrap();
    assert_eq!(
        state(&env, inv.id.clone()).await,
        Some(RestoreConflictState::Flagged)
    );
    // Die Liste der vorgemerkten Entwürfe nennt denselben Beleg.
    let listed = after_restore::list_conflicts(&env.pool).await.unwrap();
    assert!(listed.iter().any(|c| c.kind == "invoice" && c.id == inv.id));
}

// ============================================================================
// (3) Rückstand-Vorschau (V1-11): Vorschau = Lauf
// ============================================================================

async fn template(env: &Env, freq: &str, day: i64, next: &str, end: Option<&str>) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO recurring_invoices
            (id, label, contact_id, frequency, day_of_period, next_due_date, end_date)
         VALUES (?, 'Wartung', ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&env.contact)
    .bind(freq)
    .bind(day)
    .bind(next)
    .bind(end)
    .execute(&env.pool)
    .await
    .unwrap();
    id
}

async fn subscription(env: &Env, day: i64, next: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO recurring_subscriptions
            (id, label, vendor_contact_id, frequency, day_of_period, next_due_date,
             expected_amount_cents, category, description_template)
         VALUES (?, 'Hosting', ?, 'monthly', ?, ?, 1500, 'software', 'Hosting')",
    )
    .bind(&id)
    .bind(&env.contact)
    .bind(day)
    .bind(next)
    .execute(&env.pool)
    .await
    .unwrap();
    id
}

async fn confirmed_due(pool: &SqlitePool, action: &str, id: &str) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT json_extract(details_json, '$.due') FROM audit_log
          WHERE action = ? AND entity_id = ?",
    )
    .bind(action)
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn backlog_preview_matches_resume_mode_for_templates() {
    let env = setup().await;
    let now = d(2026, 10, 31);

    // 22 Termine (01.01.2025 … 01.10.2026): nachholen möglich.
    let id = template(&env, "monthly", 1, "2025-01-01", None).await;
    let p = rec_cmd::backlog_preview_core(&env.pool, BacklogSeries::Template, &id, now)
        .await
        .unwrap();
    assert!(p.backlog);
    assert_eq!(p.due, 22);
    assert_eq!(p.dates.len(), 22);
    assert_eq!(p.dates[0], d(2025, 1, 1));
    assert_eq!(p.dates[21], d(2026, 10, 1));
    assert_eq!(p.cap, catch_up::CONFIRMED_CAP);
    assert!(!p.exceeds_cap && !p.after_restore);
    assert_eq!((p.skip_to, p.skipped), (d(2026, 11, 1), 22));
    let tmpl = recurring_invoice::get_row(&env.pool, &id)
        .await
        .unwrap()
        .unwrap();
    klein_buch_lib::scheduler::recurring_invoice::apply_resume_mode(
        &env.pool,
        &tmpl,
        ResumeMode::CatchUp,
        now,
    )
    .await
    .unwrap();
    assert_eq!(
        confirmed_due(&env.pool, "recurring_invoice.backlog_confirmed", &id).await,
        Some(p.due as i64)
    );

    // 34 Termine: über der Obergrenze — die Vorschau sagt es, der Lauf lehnt ab.
    let many = template(&env, "monthly", 1, "2024-01-01", None).await;
    let p = rec_cmd::backlog_preview_core(&env.pool, BacklogSeries::Template, &many, now)
        .await
        .unwrap();
    assert_eq!(p.due, 34);
    assert!(p.exceeds_cap);
    assert_eq!(p.dates.len(), catch_up::CONFIRMED_CAP);
    let tmpl = recurring_invoice::get_row(&env.pool, &many)
        .await
        .unwrap()
        .unwrap();
    let err = klein_buch_lib::scheduler::recurring_invoice::apply_resume_mode(
        &env.pool,
        &tmpl,
        ResumeMode::CatchUp,
        now,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("34 offene Termine"), "{err}");
    // „Ab heute fortsetzen“ landet auf dem Termin der Vorschau.
    klein_buch_lib::scheduler::recurring_invoice::apply_resume_mode(
        &env.pool,
        &tmpl,
        ResumeMode::Skip,
        now,
    )
    .await
    .unwrap();
    let after = recurring_invoice::get_row(&env.pool, &many)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.next_due_date, p.skip_to.to_string());

    // Laufzeit-Ende: nur die Termine bis zum Ende zählen.
    let ending = template(&env, "quarterly", 15, "2025-01-15", Some("2025-12-31")).await;
    let p = rec_cmd::backlog_preview_core(&env.pool, BacklogSeries::Template, &ending, now)
        .await
        .unwrap();
    assert_eq!(
        p.dates,
        vec![
            d(2025, 1, 15),
            d(2025, 4, 15),
            d(2025, 7, 15),
            d(2025, 10, 15)
        ]
    );
    assert!(p.backlog && p.skip_ends_series);

    // Prüfung nach einer Wiederherstellung: auch der heutige Termin verlangt
    // die Wahl, „ab heute fortsetzen“ lässt ihn aus.
    let restored = template(&env, "monthly", 31, "2026-10-31", None).await;
    let p = rec_cmd::backlog_preview_core(&env.pool, BacklogSeries::Template, &restored, now)
        .await
        .unwrap();
    assert!(!p.backlog && !p.after_restore);
    app_settings::set(
        &env.pool,
        &catch_up::review_key(Series::Template, &restored),
        "2026-10-31",
    )
    .await
    .unwrap();
    let p = rec_cmd::backlog_preview_core(&env.pool, BacklogSeries::Template, &restored, now)
        .await
        .unwrap();
    assert!(p.backlog && p.after_restore);
    assert_eq!(p.due, 1);
    assert_eq!((p.skip_to, p.skipped), (d(2026, 11, 30), 1));
    let tmpl = recurring_invoice::get_row(&env.pool, &restored)
        .await
        .unwrap()
        .unwrap();
    klein_buch_lib::scheduler::recurring_invoice::apply_resume_mode(
        &env.pool,
        &tmpl,
        ResumeMode::Skip,
        now,
    )
    .await
    .unwrap();
    let after = recurring_invoice::get_row(&env.pool, &restored)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.next_due_date, "2026-11-30");
}

#[tokio::test]
async fn backlog_preview_matches_resume_mode_for_subscriptions() {
    let env = setup().await;
    let now = d(2026, 10, 31);
    let id = subscription(&env, 5, "2026-03-05").await;
    let p = rec_cmd::backlog_preview_core(&env.pool, BacklogSeries::Subscription, &id, now)
        .await
        .unwrap();
    assert!(p.backlog && !p.exceeds_cap);
    assert_eq!(p.due, 8);
    assert_eq!(p.dates.last(), Some(&d(2026, 10, 5)));
    let sub = recurring::get(&env.pool, &id).await.unwrap().unwrap();
    klein_buch_lib::scheduler::recurring::apply_resume_mode(
        &env.pool,
        &sub,
        ResumeMode::CatchUp,
        now,
    )
    .await
    .unwrap();
    assert_eq!(
        confirmed_due(&env.pool, "recurring.backlog_confirmed", &id).await,
        Some(p.due as i64)
    );

    // Unbekannte Reihe: Fehler statt leerer Vorschau.
    assert!(rec_cmd::backlog_preview_core(
        &env.pool,
        BacklogSeries::Subscription,
        "01928300-0000-7000-8000-000000000000",
        now
    )
    .await
    .is_err());

    // Der Command liest die Uhr des Kontexts und nimmt `kind` in camelCase.
    let args: rec_cmd::RecurringBacklogPreviewArgs =
        serde_json::from_value(serde_json::json!({ "kind": "subscription", "id": id })).unwrap();
    let via_cmd = rec_cmd::recurring_backlog_preview(&env.ctx, args)
        .await
        .unwrap();
    let json = serde_json::to_value(&via_cmd).unwrap();
    for key in [
        "due",
        "dates",
        "cap",
        "exceedsCap",
        "afterRestore",
        "skipTo",
        "backlog",
    ] {
        assert!(json.get(key).is_some(), "{key}: {json}");
    }
}
