//! Lauf F, Block F5a — Belege, Geld, §19, Pakete, Entwürfe.
//!
//! * Neuer Fund F2a: Rechnungs-Entwürfe bearbeiten (`invoices_update_draft`)
//!   mit denselben Prüfungen wie das Anlegen.
//! * KB-0009/KB-0010/KB-0011: §19-Status beim Festschreiben eingefroren;
//!   Angebots-PDF und Klausel-Check nutzen ihn; Storno spiegelt das Original.
//! * KB-0014: Kategorie E nur mit §19.
//! * LE-CUSTOM-TEMPLATE-STORNO: eigene Vorlage ohne `storno_of` → eingebaute
//!   Vorlage für Stornos + Inbox-Hinweis.
//! * R1-S2-PAYHIST → Zahlungsbuch (0034): JSON-Historie eingefroren, Cache an das Buch gebunden.
//! * R5-S2-QUOTEDATE: Annahmedatum geprüft.
//! * KB-0046/KB-0047: Paket-Hardline (Revisionen, Provenienz, Klartext, §19).
//!
//! Tempfile-DB mit den echten Migrationen, Mock-Sidecar; Testdaten nur mit
//! Dokumentations-Adressen (example.org).

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::NaiveDate;
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::invoices::{self as inv_cmd, CreateDraftArgs};
use klein_buch_lib::commands::packages::{self as pkg_cmd, PackageMaterializeItemArgs};
use klein_buch_lib::commands::quotes::{self as quotes_cmd};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, invoices, packages, quotes, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{self, InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::domain::package::{self as package_domain, PackageRevisionInput};
use klein_buch_lib::domain::quote::{QuoteInput, QuoteItemInput};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Rechnungsvorlage mit §19-Marker, aber ohne Storno-Darstellung.
const TEMPLATE_NO_STORNO: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json(bytes(sys.inputs.at("data-json")))
= Rechnung #data.invoice.number
#data.invoice.gross_amount
#data.kleinunternehmer.hinweis_text
"#;

/// Angebotsvorlage ohne §19-Marker.
const QUOTE_TEMPLATE_NO_MARKER: &str = r#"#set page(paper: "a4")
#let data = json(bytes(sys.inputs.at("data-json")))
= Angebot
"#;

struct Env {
    pool: SqlitePool,
    ctx: Arc<AppCtx>,
    contact: String,
    dir: tempfile::TempDir,
}

fn paths_for(dir: &Path) -> Paths {
    let data = dir.join("data");
    for d in ["archive", "backups", "tmp"] {
        std::fs::create_dir_all(data.join(d)).unwrap();
    }
    std::fs::create_dir_all(dir.join("inputs").join("pdf-templates")).unwrap();
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

async fn setup(is_kleinunternehmer: bool) -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    std::fs::write(
        paths.inputs_dir.join("pdf-templates").join("default.typ"),
        TEMPLATE_NO_STORNO,
    )
    .unwrap();
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
    if !is_kleinunternehmer {
        regel_from_start(&pool).await;
    }
    let contact = mk_contact(&pool, "Kunde GmbH").await;
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    Env {
        pool,
        ctx,
        contact,
        dir,
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

fn item(position: u32, qty: f64, price: i64, code: &str, rate: f64) -> InvoiceItemInput {
    InvoiceItemInput {
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

fn input(items: Vec<InvoiceItemInput>) -> InvoiceInput {
    InvoiceInput {
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        direction: InvoiceDirection::Issued,
        invoice_date: today(),
        delivery_date: Some(today()),
        due_date: Some(today() + chrono::Duration::days(30)),
        currency_code: "EUR".into(),
        items,
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

async fn create_draft(env: &Env, input: InvoiceInput) -> String {
    inv_cmd::invoices_create_draft(
        &env.ctx,
        inv_cmd::InvoicesCreateDraftArgs {
            args: CreateDraftArgs {
                contact_id: env.contact.clone(),
                buyer_reference: None,
                input,
            },
        },
    )
    .await
    .unwrap()
    .invoice
    .id
}

async fn lock(env: &Env, id: &str) -> klein_buch_lib::error::Result<inv_cmd::LockResponse> {
    inv_cmd::run_lock_pipeline(&env.pool, &env.ctx.paths, id, "N/A").await
}

/// Storno einer festgeschriebenen Rechnung wie `invoices_cancel` (Entwurf mit
/// negierten Positionen, Festschreiben im Paar). Liefert die Storno-ID.
async fn storno(env: &Env, original_id: &str) -> klein_buch_lib::error::Result<String> {
    use klein_buch_lib::domain::storno::{
        build_storno_input, OriginalInvoiceView, OriginalItemView,
    };
    let detail = invoices::get_detail(&env.pool, original_id)
        .await?
        .expect("Original");
    let view = OriginalInvoiceView {
        invoice_number: &detail.invoice.invoice_number,
        currency_code: &detail.invoice.currency_code,
        pdf_template: &detail.invoice.pdf_template,
        service_period: None,
        delivery_date: detail
            .invoice
            .delivery_date
            .as_deref()
            .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()),
        invoice_date: chrono::NaiveDate::parse_from_str(&detail.invoice.invoice_date, "%Y-%m-%d")
            .unwrap(),
        items: detail
            .items
            .iter()
            .map(|it| OriginalItemView {
                position: it.position as u32,
                description: &it.description,
                quantity: it.quantity,
                unit_code: &it.unit_code,
                unit_price_cents: it.unit_price_cents,
                tax_rate_percent: it.tax_rate_percent,
                tax_category_code: &it.tax_category_code,
            })
            .collect(),
    };
    let storno_input = build_storno_input(
        &view,
        original_id.to_string(),
        today(),
        Some("Test-Storno".into()),
    );
    let fy = klein_buch_lib::domain::storno::storno_fiscal_year(&today());
    let number = klein_buch_lib::db::numbering::next_number(
        &env.pool,
        klein_buch_lib::domain::numbering::DocType::StornoInvoice,
        fy,
    )
    .await?;
    let seller = seller_profile::get(&env.pool).await?.expect("Profil");
    let snapshot = invoices::SellerSnapshot {
        name: &seller.name,
        street: &seller.street,
        postal_code: &seller.postal_code,
        city: &seller.city,
        tax_number: seller.tax_number.as_deref(),
        vat_id: seller.vat_id.as_deref(),
    };
    let buyer = invoices::BuyerSnapshot {
        name: detail.invoice.buyer_name.as_deref().unwrap_or("Kunde GmbH"),
        street: detail.invoice.buyer_street.as_deref(),
        postal_code: detail.invoice.buyer_postal_code.as_deref(),
        city: detail.invoice.buyer_city.as_deref(),
        country_code: "DE",
        vat_id: None,
        email: detail.invoice.buyer_email.as_deref(),
    };
    let totals = invoice::compute_totals(&storno_input.items);
    let draft = invoices::create_draft(
        &env.pool,
        &invoices::DraftCreatePayload {
            contact_id: detail.invoice.contact_id.clone(),
            fiscal_year: i64::from(fy),
            is_kleinunternehmer: detail.invoice.is_kleinunternehmer == 1,
            input: storno_input,
            derived_from_quote_id: None,
        },
        &number,
        &snapshot,
        &buyer,
        &totals,
    )
    .await?;
    inv_cmd::run_lock_pipeline_with_pair(
        &env.pool,
        &env.ctx.paths,
        &draft.id,
        "N/A",
        inv_cmd::LockCancelPair {
            original_id,
            reason: Some("Test-Storno"),
        },
    )
    .await?;
    Ok(draft.id)
}

/// Wechsel zur Regelbesteuerung ab heute (Wegfall im laufenden Jahr, ADR 0042).
async fn regel_ab_heute(pool: &SqlitePool) {
    use klein_buch_lib::db::repo::tax_status;
    use klein_buch_lib::domain::tax_status::{ChangeReason, TaxStatus};
    tax_status::insert(
        pool,
        &tax_status::NewEntry {
            effective_from: today(),
            status: TaxStatus::Regelbesteuerung,
            reason: ChangeReason::WegfallLaufend,
            binding_until: None,
            trigger_document: None,
            note: None,
        },
        today(),
    )
    .await
    .unwrap();
}

/// Regelbesteuerung seit Beginn (Anfangsstand der Statushistorie, ADR 0042).
async fn regel_from_start(pool: &SqlitePool) {
    use klein_buch_lib::db::repo::tax_status;
    use klein_buch_lib::domain::tax_status::{anfang, ChangeReason, TaxStatus};
    tax_status::insert(
        pool,
        &tax_status::NewEntry {
            effective_from: anfang(),
            status: TaxStatus::Regelbesteuerung,
            reason: ChangeReason::Initial,
            binding_until: None,
            trigger_document: None,
            note: None,
        },
        klein_buch_lib::clock::today_berlin(),
    )
    .await
    .unwrap();
}

async fn flag(pool: &SqlitePool, table: &str, id: &str) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT is_kleinunternehmer FROM {table} WHERE id = ?"
    ))
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn notifications_with_key(pool: &SqlitePool, key: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE dedup_key = ?")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ============================================================================
// KB-0010 / KB-0014: §19-Status beim Festschreiben
// ============================================================================

#[tokio::test]
async fn invoice_lock_freezes_paragraph_19_status() {
    let env = setup(true).await;
    let id = create_draft(&env, input(vec![item(1, 2.0, 10_000, "E", 0.0)])).await;
    // Veralteter Stand am Entwurf (z. B. Profil zwischendurch gewechselt).
    sqlx::query("UPDATE invoices SET is_kleinunternehmer = 0 WHERE id = ?")
        .bind(&id)
        .execute(&env.pool)
        .await
        .unwrap();
    let resp = lock(&env, &id).await.expect("Festschreiben");
    assert!(resp.invoice.locked_at.is_some());
    // Eingefroren ist der Status, unter dem gerendert und geprüft wurde.
    assert_eq!(flag(&env.pool, "invoices", &id).await, 1);
}

#[tokio::test]
async fn exempt_category_without_paragraph_19_is_rejected_at_lock() {
    let env = setup(false).await;
    let id = create_draft(&env, input(vec![item(1, 1.0, 10_000, "E", 0.0)])).await;
    let err = lock(&env, &id).await.unwrap_err().to_string();
    assert!(err.contains("Steuerart E"), "{err}");
    // Regelbesteuerung mit S 19 % geht durch; der Status 0 wird eingefroren.
    let ok = create_draft(&env, input(vec![item(1, 1.0, 10_000, "S", 19.0)])).await;
    let resp = lock(&env, &ok).await.expect("Regelbesteuerung");
    assert_eq!(resp.invoice.tax_amount_cents, 1_900);
    assert_eq!(flag(&env.pool, "invoices", &ok).await, 0);
}

#[tokio::test]
async fn storno_mirrors_status_of_original_after_switch() {
    let env = setup(true).await;
    let id = create_draft(&env, input(vec![item(1, 1.0, 25_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    // Wechsel zur Regelbesteuerung nach der Original-Rechnung.
    regel_ab_heute(&env.pool).await;
    // Der Storno hebt genau die §19-Rechnung auf: E/0 bleibt zulässig, der
    // Storno trägt §19 wie das Original.
    let storno_id = storno(&env, &id).await.expect("Storno nach dem Wechsel");
    assert_eq!(flag(&env.pool, "invoices", &storno_id).await, 1);
    let original = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    assert_eq!(original.status, "canceled");
}

// ============================================================================
// LE-CUSTOM-TEMPLATE-STORNO
// ============================================================================

#[tokio::test]
async fn storno_with_template_without_storno_of_uses_builtin_and_notifies() {
    let env = setup(true).await;
    let key = "storno_template_fallback:default";
    let id = create_draft(&env, input(vec![item(1, 1.0, 25_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    // Eine normale Rechnung rendert mit der eigenen Vorlage, kein Hinweis.
    assert_eq!(notifications_with_key(&env.pool, key).await, 0);
    let storno_id = storno(&env, &id).await.expect("Storno");
    let row = invoices::get(&env.pool, &storno_id).await.unwrap().unwrap();
    assert!(row.locked_at.is_some() && row.pdf_archive_id.is_some());
    assert_eq!(notifications_with_key(&env.pool, key).await, 1);
    // Der Beleg selbst behält den Vorlagennamen (Snapshot ist das PDF).
    assert_eq!(row.pdf_template, "default");
}

// ============================================================================
// R1-S2-PAYHIST → Zahlungsbuch (Migration 0034, ADR 0042 Nr. 7)
// ============================================================================

/// Seit 0034 ist die JSON-Historie Altbestand und unveränderlich; die EÜR liest
/// das Zahlungsbuch.
#[tokio::test]
async fn payment_history_json_is_frozen_ledger_is_the_source() {
    let env = setup(true).await;
    let id = create_draft(&env, input(vec![item(1, 1.0, 50_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    let iso = today().to_string();
    invoices::record_payment(&env.pool, &id, 10_000, &iso, None)
        .await
        .unwrap();
    let err = sqlx::query("UPDATE invoices SET payment_history_json = '{kaputt' WHERE id = ?")
        .bind(&id)
        .execute(&env.pool)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("payment_history_json"), "{err}");
    let row = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    assert_eq!(row.payment_history_json, None, "neue Zahlungen nur im Buch");
    let eur = klein_buch_lib::db::repo::euer::income_payments(&env.pool)
        .await
        .unwrap();
    assert_eq!(eur.iter().map(|p| p.amount_cents).sum::<i64>(), 10_000);
}

/// Der Zahlungsstand der Rechnung ist ein Cache des Buchs: ein Roh-UPDATE, das
/// davon abweicht, scheitert; Buchungen halten Buch und Cache gleich.
#[tokio::test]
async fn payment_cache_is_bound_to_the_ledger() {
    let env = setup(true).await;
    let id = create_draft(&env, input(vec![item(1, 1.0, 50_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    let iso = today().to_string();
    for sql in [
        "UPDATE invoices SET paid_amount_cents = 20000 WHERE id = ?",
        "UPDATE invoices SET status = 'paid' WHERE id = ?",
        "UPDATE invoices SET paid_at = '2026-01-01' WHERE id = ?",
    ] {
        let err = sqlx::query(sql)
            .bind(&id)
            .execute(&env.pool)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Zahlungsbuch"), "{sql}: {err}");
    }
    let row = invoices::record_payment(&env.pool, &id, 5_000, &iso, None)
        .await
        .unwrap();
    assert_eq!(
        (row.paid_amount_cents, row.status.as_str()),
        (5_000, "partially_paid")
    );
    let row = invoices::record_payment(&env.pool, &id, 45_000, &iso, None)
        .await
        .unwrap();
    assert_eq!(
        (row.paid_amount_cents, row.status.as_str()),
        (50_000, "paid")
    );
    assert_eq!(row.paid_at.as_deref(), Some(iso.as_str()));
    let sum: i64 =
        sqlx::query_scalar("SELECT SUM(amount_cents) FROM invoice_payments WHERE invoice_id = ?")
            .bind(&id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(sum, 50_000);
    // Überzahlung lehnt der Server ab, die DB als zweite Linie ebenso.
    let err = invoices::record_payment(&env.pool, &id, 1, &iso, None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Überzahlung"), "{err}");
}

// ============================================================================
// Neuer Fund F2a: Entwürfe bearbeiten
// ============================================================================

#[tokio::test]
async fn update_draft_changes_items_and_keeps_number() {
    let env = setup(true).await;
    let id = create_draft(
        &env,
        input(vec![
            item(1, 2.0, 10_000, "E", 0.0),
            item(2, 1.0, 5_000, "E", 0.0),
        ]),
    )
    .await;
    let before = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    let other = mk_contact(&env.pool, "Andere Kundin").await;
    let mut changed = input(vec![item(1, 0.5, 33_333, "E", 0.0)]);
    changed.notes = Some("geändert".into());
    changed.payment_note = Some("Betrag dankend bar erhalten".into());
    let detail = inv_cmd::update_draft_core(&env.pool, &id, &other, changed, today())
        .await
        .expect("Entwurf bearbeiten");
    assert_eq!(detail.invoice.invoice_number, before.invoice_number);
    assert_eq!(detail.invoice.contact_id, other);
    assert_eq!(detail.invoice.buyer_name.as_deref(), Some("Andere Kundin"));
    assert_eq!(detail.items.len(), 1);
    // 0,5 × 333,33 € = 166,665 € → 166,67 € (kaufmännisch).
    assert_eq!(detail.invoice.gross_amount_cents, 16_667);
    assert_eq!(detail.items[0].net_amount_cents, 16_667);
    assert_eq!(detail.invoice.notes.as_deref(), Some("geändert"));
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.draft_updated' AND entity_id = ?",
    )
    .bind(&id)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
    // Der bearbeitete Entwurf lässt sich festschreiben.
    lock(&env, &id)
        .await
        .expect("Festschreiben nach dem Bearbeiten");
}

#[tokio::test]
async fn update_draft_rejects_closed_states_and_other_year() {
    let env = setup(true).await;
    let good = || input(vec![item(1, 1.0, 10_000, "E", 0.0)]);

    // Anderes Geschäftsjahr als die Nummer.
    let id = create_draft(&env, good()).await;
    let mut other_year = good();
    other_year.invoice_date =
        NaiveDate::from_ymd_opt(chrono::Datelike::year(&today()) - 1, 12, 1).unwrap();
    other_year.delivery_date = Some(other_year.invoice_date);
    let err = inv_cmd::update_draft_core(&env.pool, &id, &env.contact, other_year, today())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Geschäftsjahr"), "{err}");

    // §14c-Schutz: USt-Ausweis bei §19 schon beim Speichern abgelehnt.
    let err = inv_cmd::update_draft_core(
        &env.pool,
        &id,
        &env.contact,
        input(vec![item(1, 1.0, 10_000, "S", 19.0)]),
        today(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("§19"), "{err}");

    // Beträge außerhalb der Grenzen (ASVS V5.4.3).
    let err = inv_cmd::update_draft_core(
        &env.pool,
        &id,
        &env.contact,
        input(vec![item(1, 1e300, 10_000, "E", 0.0)]),
        today(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("außerhalb"), "{err}");

    // Anonymisierter Empfänger.
    let anon = mk_contact(&env.pool, "Wird anonymisiert").await;
    sqlx::query("UPDATE contacts SET anonymized_at = datetime('now','utc') WHERE id = ?")
        .bind(&anon)
        .execute(&env.pool)
        .await
        .unwrap();
    let err = inv_cmd::update_draft_core(&env.pool, &id, &anon, good(), today())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("anonymisiert"), "{err}");

    // Festgeschrieben.
    lock(&env, &id).await.unwrap();
    let err = inv_cmd::update_draft_core(&env.pool, &id, &env.contact, good(), today())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("festgeschrieben"), "{err}");

    // Storno-Entwurf: ein Storno, dessen Festschreiben scheitert, bleibt als
    // Entwurf liegen — bearbeiten lässt er sich nicht.
    let storno_id = storno(&env, &id).await.unwrap();
    let err = inv_cmd::update_draft_core(&env.pool, &storno_id, &env.contact, good(), today())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("festgeschrieben"), "{err}");

    // Verworfen.
    let voided = create_draft(&env, good()).await;
    inv_cmd::void_draft_core(
        &env.pool,
        &voided,
        "Testfall: doppelt angelegt",
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    let err = inv_cmd::update_draft_core(&env.pool, &voided, &env.contact, good(), today())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("verworfen"), "{err}");

    // Storno-Bezug im Input wird abgelehnt.
    let open = create_draft(&env, good()).await;
    let mut as_storno = good();
    as_storno.is_storno_for = Some(id.clone());
    let err = inv_cmd::update_draft_core(&env.pool, &open, &env.contact, as_storno, today())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Stornieren"), "{err}");
}

#[tokio::test]
async fn update_draft_rejects_unlocked_storno_draft() {
    let env = setup(true).await;
    let id = create_draft(&env, input(vec![item(1, 1.0, 10_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    // Storno-Entwurf direkt anlegen (z. B. nach gescheiterter Pipeline).
    let detail = invoices::get_detail(&env.pool, &id).await.unwrap().unwrap();
    let mut storno_input = input(vec![item(1, 1.0, -10_000, "E", 0.0)]);
    storno_input.is_storno_for = Some(id.clone());
    let totals = invoice::compute_totals(&storno_input.items);
    let seller = invoices::SellerSnapshot {
        name: &detail.invoice.seller_name,
        street: &detail.invoice.seller_street,
        postal_code: &detail.invoice.seller_postal_code,
        city: &detail.invoice.seller_city,
        tax_number: detail.invoice.seller_tax_number.as_deref(),
        vat_id: None,
    };
    let buyer = invoices::BuyerSnapshot {
        name: "Kunde GmbH",
        street: None,
        postal_code: None,
        city: None,
        country_code: "DE",
        vat_id: None,
        email: None,
    };
    let fy = i64::from(chrono::Datelike::year(&today()));
    let draft = invoices::create_draft(
        &env.pool,
        &invoices::DraftCreatePayload {
            contact_id: env.contact.clone(),
            fiscal_year: fy,
            is_kleinunternehmer: true,
            input: storno_input,
            derived_from_quote_id: None,
        },
        &format!("ST-{fy}-9999"),
        &seller,
        &buyer,
        &totals,
    )
    .await
    .unwrap();
    let err = inv_cmd::update_draft_core(
        &env.pool,
        &draft.id,
        &env.contact,
        input(vec![item(1, 1.0, 10_000, "E", 0.0)]),
        today(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("Storno-Entwurf"), "{err}");
}

// ============================================================================
// KB-0046 / KB-0047: Paket-Hardline
// ============================================================================

fn revision(title: &str, body: &str, price: i64) -> PackageRevisionInput {
    PackageRevisionInput {
        title: title.into(),
        body_markup: body.into(),
        default_unit_price_cents: price,
        unit_code: "C62".into(),
        tax_category_code: "S".into(),
        note: None,
    }
}

#[tokio::test]
async fn package_revisions_are_append_only_and_rollback_copies() {
    let env = setup(true).await;
    let pkg = packages::create(
        &env.pool,
        None,
        "Hochzeit",
        &revision("Hochzeit Basis", "**Ganztags**\n\n- 8 Stunden", 120_000),
    )
    .await
    .unwrap();
    let rev1 = packages::revision_get(&env.pool, &pkg.id, 1)
        .await
        .unwrap()
        .unwrap();
    let rev1_json = serde_json::to_value(&rev1).unwrap();

    // Direktes UPDATE/DELETE auf eine Revision bricht ab (Trigger).
    assert!(
        sqlx::query("UPDATE package_revisions SET default_unit_price_cents = 1 WHERE id = ?")
            .bind(&rev1.id)
            .execute(&env.pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM package_revisions WHERE id = ?")
        .bind(&rev1.id)
        .execute(&env.pool)
        .await
        .is_err());

    // Bearbeiten = neue Revision; Rollback = neue Revision mit altem Inhalt.
    let p2 = packages::update_as_new_revision(
        &env.pool,
        &pkg.id,
        None,
        "Hochzeit",
        &revision("Hochzeit Premium", "Neu", 150_000),
    )
    .await
    .unwrap();
    assert_eq!(p2.current_revision, Some(2));
    let p3 = packages::rollback(&env.pool, &pkg.id, 1).await.unwrap();
    assert_eq!(p3.current_revision, Some(3));
    let rev3 = packages::revision_get(&env.pool, &pkg.id, 3)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rev3.title, rev1.title);
    assert_eq!(rev3.body_markup, rev1.body_markup);
    assert_eq!(rev3.default_unit_price_cents, rev1.default_unit_price_cents);
    // Der Vorgänger bleibt bitgleich.
    let rev1_after = packages::revision_get(&env.pool, &pkg.id, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::to_value(&rev1_after).unwrap(), rev1_json);
    assert_eq!(
        packages::revisions_list(&env.pool, &pkg.id)
            .await
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn package_item_is_a_snapshot_with_provenance_rules() {
    let env = setup(true).await;
    let body = "**Ganztags**\n\n- 8 Stunden Begleitung";
    let pkg = packages::create(
        &env.pool,
        None,
        "Hochzeit",
        &revision("Hochzeit Basis", body, 120_000),
    )
    .await
    .unwrap();
    let m = pkg_cmd::package_materialize_item(
        &env.ctx,
        PackageMaterializeItemArgs {
            package_id: pkg.id.clone(),
            document_date: None,
        },
    )
    .await
    .unwrap();
    // §19: E und 0 % (auch wenn die Revision S führt).
    assert_eq!(m.tax_category_code, "E");
    assert_eq!(m.tax_rate_percent, 0.0);
    // XRechnung-BT-154 bekommt den geplätteten Klartext, nie das Markup.
    let plain = package_domain::to_plaintext(&package_domain::parse_markup(body));
    assert_eq!(m.description, plain);
    assert!(!m.description.contains("**"));

    let pkg_item = InvoiceItemInput {
        position: 1,
        description: m.description.clone(),
        quantity: m.quantity,
        unit_code: m.unit_code.clone(),
        unit_price_cents: m.unit_price_cents,
        tax_rate_percent: m.tax_rate_percent,
        tax_category_code: m.tax_category_code.clone(),
        description_title: Some(m.description_title.clone()),
        description_markup: Some(m.description_markup.clone()),
        source_package_id: Some(m.source_package_id.clone()),
        source_package_revision: Some(m.source_package_revision),
    };
    let id = create_draft(&env, input(vec![pkg_item.clone()])).await;

    // Neue Paket-Revision ändert die eingefügte Position nicht.
    packages::update_as_new_revision(
        &env.pool,
        &pkg.id,
        None,
        "Hochzeit",
        &revision("Hochzeit Premium", "Neu", 150_000),
    )
    .await
    .unwrap();
    let detail = invoices::get_detail(&env.pool, &id).await.unwrap().unwrap();
    assert_eq!(
        detail.items[0].description_title.as_deref(),
        Some("Hochzeit Basis")
    );
    assert_eq!(detail.items[0].unit_price_cents, 120_000);
    assert_eq!(detail.items[0].source_package_revision, Some(1));

    // Unverändert neu gespeichert: Provenienz bleibt.
    let kept = inv_cmd::update_draft_core(
        &env.pool,
        &id,
        &env.contact,
        input(vec![pkg_item.clone()]),
        today(),
    )
    .await
    .unwrap();
    assert_eq!(
        kept.items[0].source_package_id.as_deref(),
        Some(pkg.id.as_str())
    );
    assert_eq!(kept.items[0].source_package_revision, Some(1));

    // Angepasst (Preis): sauberer Bruch, die Position ist danach Custom.
    let mut edited = pkg_item.clone();
    edited.unit_price_cents = 99_000;
    let broken =
        inv_cmd::update_draft_core(&env.pool, &id, &env.contact, input(vec![edited]), today())
            .await
            .unwrap();
    assert_eq!(broken.items[0].source_package_id, None);
    assert_eq!(broken.items[0].source_package_revision, None);
    assert_eq!(broken.items[0].unit_price_cents, 99_000);
    // Titel und Markup bleiben (frei editierbare Rich-Position).
    assert_eq!(
        broken.items[0].description_title.as_deref(),
        Some("Hochzeit Basis")
    );
}

#[tokio::test]
async fn package_item_under_regelbesteuerung_gets_standard_rate() {
    let env = setup(false).await;
    let pkg = packages::create(
        &env.pool,
        None,
        "Wartung",
        &PackageRevisionInput {
            tax_category_code: "E".into(),
            ..revision("Wartung", "Monatlich", 10_000)
        },
    )
    .await
    .unwrap();
    let m = pkg_cmd::package_materialize_item(
        &env.ctx,
        PackageMaterializeItemArgs {
            package_id: pkg.id,
            document_date: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(m.tax_category_code, "S");
    assert_eq!(m.tax_rate_percent, 19.0);
}

// ============================================================================
// KB-0009 / KB-0010 / KB-0011: Angebot und §19-Klausel
// ============================================================================

fn quote_input() -> QuoteInput {
    QuoteInput {
        quote_date: today(),
        valid_until: today() + chrono::Duration::days(30),
        currency_code: "EUR".into(),
        items: vec![QuoteItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 2.0,
            unit_code: "C62".into(),
            unit_price_cents: 25_000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        pdf_template: "default".into(),
    }
}

async fn issued_quote(env: &Env) -> String {
    let q = quotes_cmd::create_draft_core(
        &env.pool,
        quotes_cmd::CreateDraftArgs {
            contact_id: env.contact.clone(),
            input: quote_input(),
        },
    )
    .await
    .unwrap();
    let id = q.quote.id;
    // Veralteter Stand am Entwurf — das Festschreiben friert den aktuellen ein.
    sqlx::query("UPDATE quotes SET is_kleinunternehmer = 0 WHERE id = ?")
        .bind(&id)
        .execute(&env.pool)
        .await
        .unwrap();
    quotes_cmd::quotes_issue(&env.ctx, quotes_cmd::QuotesIssueArgs { id: id.clone() })
        .await
        .unwrap();
    assert_eq!(flag(&env.pool, "quotes", &id).await, 1);
    id
}

#[tokio::test]
async fn quote_pdf_klausel_check_uses_frozen_status() {
    let env = setup(true).await;
    let id = issued_quote(&env).await;
    // Wechsel zur Regelbesteuerung NACH dem Festschreiben.
    regel_ab_heute(&env.pool).await;
    // Eigene Angebotsvorlage ohne §19-Marker: das Angebot ist ein §19-Beleg,
    // also greift der Klausel-Check trotz des heutigen Profils.
    let quote_tpl = env
        .dir
        .path()
        .join("inputs")
        .join("pdf-templates")
        .join("quote.typ");
    std::fs::write(&quote_tpl, QUOTE_TEMPLATE_NO_MARKER).unwrap();
    let err = quotes_cmd::ensure_quote_pdf(&env.pool, &env.ctx.paths, &id)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("§19-Klausel-Check"), "{err}");
    let row = quotes::get(&env.pool, &id).await.unwrap().unwrap();
    assert!(row.pdf_archive_id.is_none());
    // Mit der eingebauten Vorlage (Marker + Feld) entsteht das PDF.
    std::fs::remove_file(&quote_tpl).unwrap();
    let archive_id = quotes_cmd::ensure_quote_pdf(&env.pool, &env.ctx.paths, &id)
        .await
        .expect("Angebots-PDF mit Default-Vorlage");
    assert!(!archive_id.is_empty());
}

// ============================================================================
// R5-S2-QUOTEDATE
// ============================================================================

#[tokio::test]
async fn quote_accept_validates_accepted_date() {
    let env = setup(true).await;
    let id = issued_quote(&env).await;
    let accept = |date: Option<NaiveDate>| quotes_cmd::QuotesAcceptArgs {
        args: quotes_cmd::AcceptArgs {
            quote_id: id.clone(),
            accepted_date: date,
            signed_contract_upload_id: None,
            attachment_label: None,
        },
    };
    let err =
        quotes_cmd::quotes_accept(&env.ctx, accept(Some(today() - chrono::Duration::days(1))))
            .await
            .unwrap_err()
            .to_string();
    assert!(err.contains("vor dem Angebotsdatum"), "{err}");
    let err =
        quotes_cmd::quotes_accept(&env.ctx, accept(Some(today() + chrono::Duration::days(1))))
            .await
            .unwrap_err()
            .to_string();
    assert!(err.contains("Zukunft"), "{err}");
    assert_eq!(
        quotes::get(&env.pool, &id).await.unwrap().unwrap().status,
        "sent"
    );
    let detail = quotes_cmd::quotes_accept(&env.ctx, accept(Some(today())))
        .await
        .unwrap();
    assert_eq!(detail.quote.status, "accepted");
    assert_eq!(
        detail.quote.accepted_at.as_deref(),
        Some(today().to_string().as_str())
    );
}

// ============================================================================
// F5-fix (Nachbesserung nach der Prüfung)
// ============================================================================

/// F5-fix (S2): Ein Entwurf mit Summen aus der älteren Rundung (USt je
/// Position, hier 0 ct bei 100 × 0,01 € zu 19 %) — etwa aus einem älteren
/// Backup — wird mit den Summen des ausgestellten Belegs festgeschrieben: DB,
/// XML (BT-110/BT-112) und Zahlungsprüfung stimmen überein.
#[tokio::test]
async fn lock_takes_totals_from_items_like_the_issued_document() {
    let env = setup(false).await;
    let items = (1..=100).map(|p| item(p, 1.0, 1, "S", 19.0)).collect();
    let id = create_draft(&env, input(items)).await;
    // Alte Summenlogik direkt am Entwurf (vor F5 bzw. aus einem älteren Backup).
    sqlx::query(
        "UPDATE invoices SET net_amount_cents = 100, tax_amount_cents = 0,
                gross_amount_cents = 100 WHERE id = ?",
    )
    .bind(&id)
    .execute(&env.pool)
    .await
    .unwrap();
    let resp = lock(&env, &id).await.expect("Festschreiben");
    let row = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    assert_eq!(
        (
            row.net_amount_cents,
            row.tax_amount_cents,
            row.gross_amount_cents
        ),
        (100, 19, 119)
    );
    let xml = klein_buch_lib::archive::read_and_verify(&env.pool, &resp.xml_archive_id)
        .await
        .unwrap();
    let xml = String::from_utf8(xml).unwrap();
    assert!(
        xml.contains("<ram:GrandTotalAmount>1.19</ram:GrandTotalAmount>"),
        "BT-112 im XML"
    );
    assert!(xml.contains(">0.19</ram:TaxTotalAmount>"), "BT-110 im XML");
    // Die Zahlung über den ausgestellten Betrag macht die Rechnung „bezahlt“.
    let paid = invoices::record_payment(&env.pool, &id, 119, &today().to_string(), None)
        .await
        .unwrap();
    assert_eq!(paid.status, "paid");
    // Danach sind die Summen gesperrt (Trigger unverändert).
    let err = sqlx::query("UPDATE invoices SET gross_amount_cents = 100 WHERE id = ?")
        .bind(&id)
        .execute(&env.pool)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("immutable"), "{err}");
}

/// F5-fix (S3): Ein Zahlungsbetrag jenseits der Domänengrenze ist ein
/// Fehler, keine Panik durch Überlauf (overflow-checks im Release-Profil).
#[tokio::test]
async fn payment_above_limit_is_a_domain_error() {
    let env = setup(true).await;
    let id = create_draft(&env, input(vec![item(1, 1.0, 10_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    invoices::record_payment(&env.pool, &id, 5_000, &today().to_string(), None)
        .await
        .unwrap();
    let err = invoices::record_payment(&env.pool, &id, i64::MAX, &today().to_string(), None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("zu groß"), "{err}");
    let row = invoices::get(&env.pool, &id).await.unwrap().unwrap();
    assert_eq!(row.paid_amount_cents, 5_000);
}

/// F5-fix (S3): Der Inbox-Hinweis zur eingebauten Storno-Vorlage entsteht
/// erst nach dem erfolgreichen Festschreiben. Scheitert der Storno (hier im
/// letzten Schritt, dem Paar-Update), gibt es keinen Hinweis — und der spätere
/// echte Storno bekommt ihn.
#[tokio::test]
async fn storno_template_notice_only_after_successful_lock() {
    let env = setup(true).await;
    let key = "storno_template_fallback:default";
    let id = create_draft(&env, input(vec![item(1, 1.0, 25_000, "E", 0.0)])).await;
    lock(&env, &id).await.unwrap();
    // Das Original gilt (Testeingriff) schon als storniert → das Paar-Update
    // beim Festschreiben scheitert, die Transaktion rollt zurück.
    sqlx::query("UPDATE invoices SET status = 'canceled' WHERE id = ?")
        .bind(&id)
        .execute(&env.pool)
        .await
        .unwrap();
    let err = storno(&env, &id).await.unwrap_err().to_string();
    assert!(err.contains("bereits storniert"), "{err}");
    assert_eq!(notifications_with_key(&env.pool, key).await, 0);

    let other = create_draft(&env, input(vec![item(1, 1.0, 30_000, "E", 0.0)])).await;
    lock(&env, &other).await.unwrap();
    storno(&env, &other).await.expect("Storno");
    assert_eq!(notifications_with_key(&env.pool, key).await, 1);
}

/// F5-fix (S3, KB-0046): Die Paket-Provenienz gilt auch für Angebote und
/// Abo-Vorlagen — eine bearbeitete Position verliert `source_package_*` schon
/// beim Speichern, eine unveränderte behält sie.
#[tokio::test]
async fn package_provenance_is_enforced_on_quotes_and_templates() {
    use klein_buch_lib::commands::recurring_invoice as ri_cmd;
    use klein_buch_lib::domain::recurring_invoice::RecurringInvoiceInput;
    let env = setup(true).await;
    let pkg = packages::create(
        &env.pool,
        None,
        "Hochzeit",
        &revision("Hochzeit Basis", "**Ganztags**", 120_000),
    )
    .await
    .unwrap();
    let m = pkg_cmd::package_materialize_item(
        &env.ctx,
        PackageMaterializeItemArgs {
            package_id: pkg.id.clone(),
            document_date: None,
        },
    )
    .await
    .unwrap();
    let quote_item = |price: i64, position: u32| QuoteItemInput {
        position,
        description: m.description.clone(),
        quantity: 1.0,
        unit_code: m.unit_code.clone(),
        unit_price_cents: price,
        tax_rate_percent: 0.0,
        tax_category_code: "E".into(),
        description_title: Some(m.description_title.clone()),
        description_markup: Some(m.description_markup.clone()),
        source_package_id: Some(pkg.id.clone()),
        source_package_revision: Some(m.source_package_revision),
    };
    let q = quotes_cmd::create_draft_core(
        &env.pool,
        quotes_cmd::CreateDraftArgs {
            contact_id: env.contact.clone(),
            input: QuoteInput {
                items: vec![quote_item(120_000, 1), quote_item(99_000, 2)],
                ..quote_input()
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q.items[0].source_package_id.as_deref(),
        Some(pkg.id.as_str())
    );
    assert_eq!(q.items[0].source_package_revision, Some(1));
    assert_eq!(q.items[1].source_package_id, None, "bearbeitet: Bruch");
    assert_eq!(q.items[1].source_package_revision, None);
    assert_eq!(q.items[1].unit_price_cents, 99_000);

    let tmpl_item = |price: i64, position: u32| InvoiceItemInput {
        position,
        description: m.description.clone(),
        quantity: 1.0,
        unit_code: m.unit_code.clone(),
        unit_price_cents: price,
        tax_rate_percent: 0.0,
        tax_category_code: "E".into(),
        description_title: Some(m.description_title.clone()),
        description_markup: Some(m.description_markup.clone()),
        source_package_id: Some(pkg.id.clone()),
        source_package_revision: Some(m.source_package_revision),
    };
    let input = RecurringInvoiceInput {
        label: "Wartung".into(),
        contact_id: env.contact.clone(),
        frequency: "monthly".into(),
        day_of_period: 1,
        next_due_date: today() + chrono::Duration::days(1),
        start_date: None,
        end_date: None,
        auto_mode: "draft".into(),
        payment_terms_days: 14,
        pdf_template: "default".into(),
        service_period_note: true,
        buyer_reference: None,
        notes: None,
        items: vec![tmpl_item(120_000, 1), tmpl_item(1_000, 2)],
    };
    let detail = ri_cmd::recurring_invoices_create(
        &env.ctx,
        ri_cmd::RecurringInvoicesCreateArgs {
            input: input.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        detail.items[0].source_package_id.as_deref(),
        Some(pkg.id.as_str())
    );
    assert_eq!(detail.items[1].source_package_id, None);
    assert_eq!(detail.items[1].source_package_revision, None);
    // Bearbeiten: dieselbe Regel.
    let mut edited = input;
    edited.items[0].unit_price_cents = 110_000;
    let detail = ri_cmd::recurring_invoices_update(
        &env.ctx,
        ri_cmd::RecurringInvoicesUpdateArgs {
            id: detail.template.id.clone(),
            input: edited,
        },
    )
    .await
    .unwrap();
    assert!(detail.items.iter().all(|i| i.source_package_id.is_none()));
}
