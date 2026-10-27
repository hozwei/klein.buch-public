//! ADR 0042 Nr. 1–6 (Lauf 2026.11.1, Block V1): Steuerstatus nach § 19 UStG
//! als Historie — Commands, Lesestellen und Auswertungen.
//!
//! * Wegfall im laufenden Jahr am 15.08.: Rechnung mit Leistung am 14.08. bleibt
//!   §19 (Klausel im XML, keine USt), ab 15.08. Regelbesteuerung (USt, keine
//!   Klausel); §14c-Schutz bleibt; eine schon festgeschriebene Rechnung nach dem
//!   Stichtag wird gemeldet, nicht geändert.
//! * Verzicht 2026 bindet bis einschließlich 2030: Rückkehr 2031 ja, 2030 nein.
//! * Wegfall wegen des Vorjahres: Rückkehr im Folgejahr ohne Sperre.
//! * Rückwirkung in ein abgeschlossenes Jahr: abgelehnt, Snapshot bleibt.
//! * Widerruf nur des jüngsten Eintrags, mit Grund und Audit.
//! * Mischjahr in EÜR, ELSTER-Ausfüllhilfe und DATEV: Kosten vor dem Stichtag
//!   brutto, danach netto mit Vorsteuer.
//! * Profil-Upsert ändert den Status nicht; Status an einem Datum für Editoren;
//!   Paket-Position nach dem Datum des Zielbelegs; Angebot → Rechnung über die
//!   Grenze.
//!
//! Tempfile-DB mit den echten Migrationen, Mock-Sidecar; Testdaten nur mit
//! Dokumentations-Adressen (example.org). Feste Daten im Jahr 2026 (offenes
//! Geschäftsjahr in der frischen Datenbank).

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::NaiveDate;
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::invoices::{self as inv_cmd, CreateDraftArgs};
use klein_buch_lib::commands::{
    assets as assets_cmd, euer as euer_cmd, expenses as expenses_cmd, packages as pkg_cmd, reauth,
    tax_status as tax_cmd,
};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, euer as euer_repo, packages, seller_profile, tax_status};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::domain::kleinunternehmer::HINWEIS_TEXT;
use klein_buch_lib::euer::datev_csv;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Rechnungsvorlage mit §19-Marker.
const TEMPLATE: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json(bytes(sys.inputs.at("data-json")))
= Rechnung #data.invoice.number
#data.invoice.gross_amount
#data.kleinunternehmer.hinweis_text
"#;

struct Env {
    pool: SqlitePool,
    ctx: Arc<AppCtx>,
    contact: String,
    _dir: tempfile::TempDir,
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn paths_for(dir: &Path) -> Paths {
    let data = dir.join("data");
    for sub in ["archive", "backups", "tmp"] {
        std::fs::create_dir_all(data.join(sub)).unwrap();
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
        vat_id: Some("DE123456788".into()),
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

async fn setup() -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    std::fs::write(
        paths.inputs_dir.join("pdf-templates").join("default.typ"),
        TEMPLATE,
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
    let contact = contacts::create(
        &pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
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

fn change_json(from: Option<&str>, status: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "input": {
            "effectiveFrom": from,
            "status": status,
            "reason": reason,
            "triggerDocument": if reason == "wegfall_laufend" { Some("RE-2026-0099") } else { None },
        }
    })
}

async fn add(
    env: &Env,
    from: Option<&str>,
    status: &str,
    reason: &str,
) -> klein_buch_lib::error::Result<tax_cmd::ChangeResult> {
    let args: tax_cmd::TaxStatusChangeAddArgs =
        serde_json::from_value(change_json(from, status, reason)).unwrap();
    reauth::with_reauth(true, tax_cmd::tax_status_change_add(&env.ctx, args)).await
}

async fn preview(
    env: &Env,
    from: Option<&str>,
    status: &str,
    reason: &str,
) -> tax_cmd::ChangePreview {
    let args: tax_cmd::TaxStatusChangePreviewArgs =
        serde_json::from_value(change_json(from, status, reason)).unwrap();
    tax_cmd::tax_status_change_preview(&env.ctx, args)
        .await
        .unwrap()
}

async fn revoke(
    env: &Env,
    id: &str,
    reason: &str,
) -> klein_buch_lib::error::Result<tax_cmd::ChangeResult> {
    tax_cmd::tax_status_change_revoke(
        &env.ctx,
        tax_cmd::TaxStatusChangeRevokeArgs {
            id: id.into(),
            reason: reason.into(),
        },
    )
    .await
}

fn item(position: u32, price: i64, code: &str, rate: f64) -> InvoiceItemInput {
    InvoiceItemInput {
        position,
        description: format!("Leistung {position}"),
        quantity: 1.0,
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

fn input(invoice_date: NaiveDate, delivery: NaiveDate, it: InvoiceItemInput) -> InvoiceInput {
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date,
        delivery_date: Some(delivery),
        due_date: Some(invoice_date + chrono::Duration::days(14)),
        currency_code: "EUR".into(),
        items: vec![it],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    }
}

async fn create_draft(env: &Env, input: InvoiceInput) -> klein_buch_lib::error::Result<String> {
    Ok(inv_cmd::invoices_create_draft(
        &env.ctx,
        inv_cmd::InvoicesCreateDraftArgs {
            args: CreateDraftArgs {
                contact_id: env.contact.clone(),
                buyer_reference: None,
                input,
            },
        },
    )
    .await?
    .invoice
    .id)
}

async fn lock(env: &Env, id: &str) -> klein_buch_lib::error::Result<inv_cmd::LockResponse> {
    inv_cmd::run_lock_pipeline(&env.pool, &env.ctx.paths, id, "N/A").await
}

async fn xml_of(env: &Env, resp: &inv_cmd::LockResponse) -> String {
    let bytes = klein_buch_lib::archive::read_and_verify(&env.pool, &resp.xml_archive_id)
        .await
        .unwrap();
    String::from_utf8(bytes).unwrap()
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

// ============================================================================
// Wegfall im laufenden Jahr (Wechsel mitten im Jahr)
// ============================================================================

#[tokio::test]
async fn wegfall_laufend_splits_invoices_at_the_effective_date() {
    let env = setup().await;
    // Vor dem Eintrag als §19-Rechnung festgeschrieben: Leistung 20.08.
    let early = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 20), item(1, 50_000, "E", 0.0)),
    )
    .await
    .unwrap();
    let early = lock(&env, &early).await.unwrap();
    assert_eq!(early.invoice.is_kleinunternehmer, 1);

    let res = add(
        &env,
        Some("2026-08-15"),
        "regelbesteuerung",
        "wegfall_laufend",
    )
    .await
    .expect("Wegfall laufend");
    assert_eq!(res.entry.reason, "wegfall_laufend");
    assert_eq!(res.entry.binding_until, None, "Wegfall bindet nicht");
    assert_eq!(res.entry.trigger_document.as_deref(), Some("RE-2026-0099"));
    // Rückwirkung: die festgeschriebene Rechnung wird gemeldet, nicht geändert.
    assert_eq!(
        res.consequences.first_difference.as_deref(),
        Some("2026-08-15")
    );
    assert_eq!(res.consequences.affected_invoices.len(), 1);
    assert_eq!(
        res.consequences.affected_invoices[0].number,
        early.invoice.invoice_number
    );
    assert!(res.consequences.reexport_years.contains(&2026));
    let body: String = sqlx::query_scalar(
        "SELECT body FROM notifications WHERE dedup_key LIKE 'tax_status_affected:%'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(
        body.contains(&early.invoice.invoice_number) && body.contains("§ 31 Abs. 5 UStDV"),
        "{body}"
    );
    let frozen: i64 = sqlx::query_scalar("SELECT is_kleinunternehmer FROM invoices WHERE id = ?")
        .bind(&early.invoice.id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(frozen, 1, "festgeschriebener Beleg unverändert");

    // Leistung 14.08.: §19 — Klausel im XML, keine USt.
    let a = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 14), item(1, 10_000, "E", 0.0)),
    )
    .await
    .unwrap();
    let a = lock(&env, &a).await.expect("§19 am 14.08.");
    assert_eq!(a.invoice.is_kleinunternehmer, 1);
    assert_eq!(a.invoice.tax_amount_cents, 0);
    assert!(xml_of(&env, &a).await.contains(HINWEIS_TEXT));

    // Leistung 15.08.: Regelbesteuerung — USt, keine Klausel.
    let b = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 15), item(1, 10_000, "S", 19.0)),
    )
    .await
    .unwrap();
    let b = lock(&env, &b).await.expect("Regel am 15.08.");
    assert_eq!(b.invoice.is_kleinunternehmer, 0);
    assert_eq!(b.invoice.tax_amount_cents, 1_900);
    assert!(!xml_of(&env, &b).await.contains(HINWEIS_TEXT));

    // Steuerart E ab dem Stichtag: Festschreiben abgelehnt (KB-0014).
    let c = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 15), item(1, 10_000, "E", 0.0)),
    )
    .await
    .unwrap();
    let err = lock(&env, &c).await.unwrap_err().to_string();
    assert!(err.contains("Steuerart E"), "{err}");

    // §14c-Schutz: vor dem Stichtag kein USt-Ausweis, schon der Entwurf scheitert.
    let err = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 14), item(1, 10_000, "S", 19.0)),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("Entwurf kann nicht gespeichert werden"),
        "{err}"
    );

    // Status an einem Datum (Editoren).
    for (day, klein) in [(d(2026, 8, 14), true), (d(2026, 8, 15), false)] {
        let at = tax_cmd::tax_status_at(&env.ctx, tax_cmd::TaxStatusAtArgs { date: day })
            .await
            .unwrap();
        assert_eq!(at.is_kleinunternehmer, klein, "{day}");
    }
    // Profil zeigt den Status von heute; ändern lässt er sich dort nicht.
    let mut back = profile();
    back.is_kleinunternehmer = Some(true);
    let err = seller_profile::upsert(&env.pool, &back).await.unwrap_err();
    assert!(err.to_string().contains("Statuswechsel eintragen"), "{err}");
}

/// Paket-Position nach dem Datum des Zielbelegs; Angebot → Rechnung über die
/// Grenze (Regel → §19: steuerfrei, Nettopreis bleibt).
#[tokio::test]
async fn package_and_quote_conversion_follow_the_document_date() {
    let env = setup().await;
    // Regelbesteuerung seit Beginn, Rückkehr zu §19 zum 01.01.2027 (keine Bindung).
    tax_status::insert(
        &env.pool,
        &tax_status::NewEntry {
            effective_from: klein_buch_lib::domain::tax_status::anfang(),
            status: klein_buch_lib::domain::tax_status::TaxStatus::Regelbesteuerung,
            reason: klein_buch_lib::domain::tax_status::ChangeReason::Initial,
            binding_until: None,
            trigger_document: None,
            note: None,
        },
        d(2026, 6, 1),
    )
    .await
    .unwrap();
    add(&env, Some("2027-01-01"), "kleinunternehmer", "rueckkehr")
        .await
        .expect("Rückkehr ohne Bindung");

    let pkg = packages::create(
        &env.pool,
        None,
        "Wartungspaket",
        &klein_buch_lib::domain::package::PackageRevisionInput {
            title: "Wartung".into(),
            body_markup: "Wartung der Anlage".into(),
            default_unit_price_cents: 10_000,
            unit_code: "C62".into(),
            tax_category_code: "S".into(),
            note: None,
        },
    )
    .await
    .unwrap();
    for (day, code) in [("2026-12-31", "S"), ("2027-01-01", "E")] {
        let m = pkg_cmd::package_materialize_item(
            &env.ctx,
            serde_json::from_value(serde_json::json!({
                "packageId": pkg.id, "documentDate": day
            }))
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(m.tax_category_code, code, "{day}");
    }

    // Angebot unter Regelbesteuerung (S 19 %), Rechnung mit Leistung 2027
    // (§19): Positionen steuerfrei, Nettopreis bleibt (§14c-Schutz).
    use klein_buch_lib::commands::quotes as quotes_cmd;
    use klein_buch_lib::domain::quote::{QuoteInput, QuoteItemInput};
    let q = quotes_cmd::create_draft_core(
        &env.pool,
        quotes_cmd::CreateDraftArgs {
            contact_id: env.contact.clone(),
            input: QuoteInput {
                quote_date: d(2026, 6, 1),
                valid_until: d(2026, 7, 1),
                currency_code: "EUR".into(),
                items: vec![QuoteItemInput {
                    position: 1,
                    description: "Wartung".into(),
                    quantity: 1.0,
                    unit_code: "C62".into(),
                    unit_price_cents: 10_000,
                    tax_rate_percent: 19.0,
                    tax_category_code: "S".into(),
                    description_title: None,
                    description_markup: None,
                    source_package_id: None,
                    source_package_revision: None,
                }],
                notes: None,
                pdf_template: "default".into(),
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        q.quote.is_kleinunternehmer, 0,
        "Angebot am 01.06.2026: Regel"
    );
    let quote_id = q.quote.id.clone();
    quotes_cmd::quotes_issue(
        &env.ctx,
        quotes_cmd::QuotesIssueArgs {
            id: quote_id.clone(),
        },
    )
    .await
    .unwrap();
    let accept: quotes_cmd::QuotesAcceptArgs = serde_json::from_value(serde_json::json!({
        "args": { "quoteId": quote_id, "acceptedDate": "2026-06-10", "attachmentLabel": null }
    }))
    .unwrap();
    quotes_cmd::quotes_accept(&env.ctx, accept).await.unwrap();
    let args: quotes_cmd::QuotesConvertToInvoiceArgs = serde_json::from_value(serde_json::json!({
        "args": {
            "quoteId": quote_id,
            "invoiceDate": "2027-01-05",
            "deliveryDate": "2027-01-04",
        }
    }))
    .unwrap();
    // Der Entwurf darf ein künftiges Rechnungsdatum tragen (geprüft wird beim
    // Festschreiben).
    let inv = quotes_cmd::quotes_convert_to_invoice(&env.ctx, args)
        .await
        .unwrap();
    assert_eq!(inv.invoice.is_kleinunternehmer, 1);
    assert!(inv
        .items
        .iter()
        .all(|it| it.tax_category_code == "E" && it.tax_rate_percent == 0.0));
    assert_eq!(inv.invoice.net_amount_cents, 10_000);
}

// ============================================================================
// Verzicht, Wegfall, Rückkehr, Widerruf
// ============================================================================

#[tokio::test]
async fn verzicht_2026_allows_return_2031_not_2030_and_revoke_latest_only() {
    let env = setup().await;
    let v = add(&env, Some("2026-01-01"), "regelbesteuerung", "verzicht")
        .await
        .expect("Verzicht");
    assert_eq!(v.entry.binding_until.as_deref(), Some("2031-01-01"));

    let p = preview(&env, Some("2030-01-01"), "kleinunternehmer", "rueckkehr").await;
    assert!(!p.ok);
    assert!(
        p.errors.iter().any(|e| e.contains("01.01.2031")),
        "{:?}",
        p.errors
    );
    let err = add(&env, Some("2030-01-01"), "kleinunternehmer", "rueckkehr")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("01.01.2031"), "{err}");
    let p = preview(&env, Some("2031-01-01"), "kleinunternehmer", "rueckkehr").await;
    assert!(p.ok, "{:?}", p.errors);
    let r = add(&env, Some("2031-01-01"), "kleinunternehmer", "rueckkehr")
        .await
        .expect("Rückkehr nach Ablauf der Bindung");

    // Widerruf nur des jüngsten Eintrags, mit Grund.
    let err = revoke(&env, &v.entry.id, "Irrtum")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("jüngste"), "{err}");
    assert!(revoke(&env, &r.entry.id, "  ").await.is_err());
    let rv = revoke(&env, &r.entry.id, "Doppelt erfasst").await.unwrap();
    assert!(rv.entry.revoked_at.is_some());
    assert_eq!(rv.entry.revoke_reason.as_deref(), Some("Doppelt erfasst"));
    let err = revoke(&env, &r.entry.id, "nochmal")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("bereits widerrufen"), "{err}");
    // Historie behält den widerrufenen Eintrag; die Übersicht nennt ihn.
    let o = tax_cmd::tax_status_overview(&env.ctx, tax_cmd::TaxStatusOverviewArgs {})
        .await
        .unwrap();
    assert_eq!(o.entries.len(), 2);
    assert_eq!(o.revocable_id.as_deref(), Some(v.entry.id.as_str()));
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'tax_status.changed'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'tax_status.revoked'"
        )
        .await,
        1
    );
    // Ein abgelehnter Eintrag hinterlässt weder Eintrag noch Audit.
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM tax_status_changes").await,
        2
    );
}

#[tokio::test]
async fn wegfall_vorjahr_allows_return_next_year_without_lock() {
    let env = setup().await;
    let w = add(
        &env,
        Some("2026-01-01"),
        "regelbesteuerung",
        "wegfall_vorjahr",
    )
    .await
    .expect("Wegfall Vorjahr");
    assert_eq!(w.entry.binding_until, None);
    add(&env, Some("2027-01-01"), "kleinunternehmer", "rueckkehr")
        .await
        .expect("Rückkehr im Folgejahr");
    // Grund und Datum passen nicht: Wegfall wegen des Vorjahres nur zum 01.01.
    let p = preview(
        &env,
        Some("2027-03-01"),
        "regelbesteuerung",
        "wegfall_vorjahr",
    )
    .await;
    assert!(!p.ok);
    assert!(
        p.errors.iter().any(|e| e.contains("1. Januar")),
        "{:?}",
        p.errors
    );
}

#[tokio::test]
async fn retroactive_entry_in_closed_year_is_rejected_and_snapshot_stays() {
    let env = setup().await;
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
                                        afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (2025, 0, 0, 0, 0, 'test', 33)",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let p = preview(
        &env,
        Some("2025-11-03"),
        "regelbesteuerung",
        "wegfall_laufend",
    )
    .await;
    assert!(!p.ok);
    let err = add(
        &env,
        Some("2025-11-03"),
        "regelbesteuerung",
        "wegfall_laufend",
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("abgeschlossenen Geschäftsjahr 2025"), "{err}");
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM tax_status_changes").await,
        0
    );
    // Ins offene 2026 geht es; das abgeschlossene Jahr bleibt unverändert.
    add(
        &env,
        Some("2026-01-01"),
        "regelbesteuerung",
        "wegfall_vorjahr",
    )
    .await
    .expect("offenes Jahr");
    let check = klein_buch_lib::fiscal_year::snapshot::check(&env.pool, 2025)
        .await
        .unwrap()
        .unwrap();
    assert!(check.matches, "{check:?}");
    // Widerruf, der 2025 änderte, gibt es nicht (Eintrag wirkt erst 2026).
    let o = tax_cmd::tax_status_overview(&env.ctx, tax_cmd::TaxStatusOverviewArgs {})
        .await
        .unwrap();
    revoke(&env, o.revocable_id.as_deref().unwrap(), "Irrtum")
        .await
        .expect("Widerruf im offenen Jahr");
}

// ============================================================================
// Mischjahr in EÜR, ELSTER-Ausfüllhilfe, DATEV
// ============================================================================

fn laptop(doc: NaiveDate, paid: NaiveDate) -> ExpenseInput {
    ExpenseInput {
        expense_date: doc,
        paid_date: Some(paid),
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

async fn create_expense(env: &Env, input: ExpenseInput) {
    let args: expenses_cmd::ExpensesCreateArgs = serde_json::from_value(serde_json::json!({
        "args": { "input": input }
    }))
    .unwrap();
    expenses_cmd::expenses_create(&env.ctx, args).await.unwrap();
}

#[tokio::test]
async fn mixed_year_in_euer_elster_and_datev() {
    let env = setup().await;
    add(
        &env,
        Some("2026-08-15"),
        "regelbesteuerung",
        "wegfall_laufend",
    )
    .await
    .unwrap();
    create_expense(&env, laptop(d(2026, 8, 14), d(2026, 8, 20))).await;
    create_expense(&env, laptop(d(2026, 8, 16), d(2026, 8, 20))).await;

    let report = euer_cmd::euer_compute_report(
        &env.ctx,
        euer_cmd::EuerComputeReportArgs { fiscal_year: 2026 },
    )
    .await
    .unwrap();
    assert!(!report.is_kleinunternehmer);
    assert_eq!(
        report.year_status,
        klein_buch_lib::domain::tax_status::YearStatus::Wechsel
    );
    assert!(report
        .status_change
        .as_deref()
        .unwrap()
        .contains("15.08.2026"));

    let args: euer_cmd::EuerPackageArgs =
        serde_json::from_value(serde_json::json!({ "fiscalYear": 2026 })).unwrap();
    let pkg = euer_cmd::euer_package(&env.ctx, args).await.unwrap();
    let kz = |k: u16| {
        pkg.form
            .lines
            .iter()
            .find(|l| l.kennzahl == Some(k) && l.is_entry)
            .map(|l| l.amount_cents)
    };
    assert_eq!(kz(185), Some(19_000), "Vorsteuer nur nach dem Stichtag");
    assert_eq!(kz(229), Some(219_000), "brutto + netto in der Kategorie");
    assert!(pkg.form.status_change.is_some());
    assert!(
        pkg.form
            .notes
            .iter()
            .any(|n| n.starts_with("Wechsel des Steuerstatus in 2026")),
        "{:?}",
        pkg.form.notes
    );
    let klein: Vec<bool> = pkg.expenses.iter().map(|e| e.is_kleinunternehmer).collect();
    assert_eq!(klein, vec![true, false]);

    let expenses = euer_repo::expense_detail(&env.pool, 2026).await.unwrap();
    let bookings = datev_csv::build_bookings(
        datev_csv::Skr::Skr03,
        2026,
        &datev_csv::DatevInputs {
            income: &[],
            storno: &[],
            expenses: &expenses,
            capitalized: &[],
            disposals: &[],
            assets: &[],
            private_movements: &[],
        },
    );
    let mut amounts: Vec<(i64, String)> = bookings
        .iter()
        .map(|b| (b.amount_cents, b.bu_schluessel.clone()))
        .collect();
    amounts.sort();
    assert_eq!(
        amounts,
        vec![(119_000, String::new()), (119_000, "9".into())],
        "vor dem Stichtag brutto ohne, danach mit Vorsteuer-Schlüssel"
    );
}

/// Grenzhinweis (ADR 0042 Nr. 5): Gesamtumsatz über 100.000 € im laufenden Jahr
/// mit dem auslösenden Beleg; Klein.Buch wechselt den Status nicht selbst.
#[tokio::test]
async fn threshold_hint_names_trigger_document_and_does_not_switch() {
    let env = setup().await;
    let today = klein_buch_lib::clock::today_berlin();
    let year = chrono::Datelike::year(&today);
    // Umsatz im Vorjahr (keine Gründungsjahr-Vermutung) und zwei Zahlungen heute.
    let prev = d(year - 1, 6, 1);
    let first = create_draft(&env, input(prev, prev, item(1, 1_000, "E", 0.0)))
        .await
        .unwrap();
    lock(&env, &first).await.unwrap();
    klein_buch_lib::db::repo::invoices::record_payment(
        &env.pool,
        &first,
        1_000,
        &prev.to_string(),
        None,
    )
    .await
    .unwrap();
    let mut numbers = Vec::new();
    for price in [6_000_000, 4_000_001] {
        let id = create_draft(&env, input(today, today, item(1, price, "E", 0.0)))
            .await
            .unwrap();
        let resp = lock(&env, &id).await.unwrap();
        klein_buch_lib::db::repo::invoices::record_payment(
            &env.pool,
            &id,
            price,
            &today.to_string(),
            None,
        )
        .await
        .unwrap();
        numbers.push(resp.invoice.invoice_number);
    }
    let o = tax_cmd::tax_status_overview(&env.ctx, tax_cmd::TaxStatusOverviewArgs {})
        .await
        .unwrap();
    assert_eq!(o.turnover.current_year_cents, 10_000_001);
    assert_eq!(o.turnover.previous_year_cents, 1_000);
    let hint = o
        .threshold_hints
        .iter()
        .find(|h| h.kind == klein_buch_lib::domain::tax_status::Grenzart::Laufend)
        .expect("Grenzhinweis");
    assert!(hint.exceeded);
    assert_eq!(hint.trigger_document.as_deref(), Some(numbers[1].as_str()));
    assert!(!o
        .threshold_hints
        .iter()
        .any(|h| h.kind == klein_buch_lib::domain::tax_status::Grenzart::Gruendungsjahr));
    let n = tax_cmd::notify_thresholds(&env.pool, today).await.unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        tax_cmd::notify_thresholds(&env.pool, today).await.unwrap(),
        0,
        "einmal"
    );
    // Kein automatischer Wechsel.
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM tax_status_changes").await,
        0
    );
    assert_eq!(
        o.current_status,
        klein_buch_lib::domain::tax_status::TaxStatus::Kleinunternehmer
    );
}

// ============================================================================
// Befunde der Prüfung V1 (V1-R01, V1-R03, V1-R04, V1-R07)
// ============================================================================

/// V1-R01: Eintrag und Widerruf desselben Eintrags legen je einen eigenen
/// Hinweis an; der Widerruf meldet die unter dem widerrufenen Status
/// festgeschriebenen Rechnungen.
#[tokio::test]
async fn revoke_reports_invoices_locked_under_the_revoked_status() {
    let env = setup().await;
    let a = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 20), item(1, 50_000, "E", 0.0)),
    )
    .await
    .unwrap();
    let a = lock(&env, &a).await.unwrap();
    let w = add(
        &env,
        Some("2026-08-15"),
        "regelbesteuerung",
        "wegfall_laufend",
    )
    .await
    .unwrap();
    assert_eq!(w.consequences.affected_invoices.len(), 1);

    // Unter dem neuen Status mit USt festgeschrieben.
    let b = create_draft(
        &env,
        input(d(2026, 8, 21), d(2026, 8, 21), item(1, 10_000, "S", 19.0)),
    )
    .await
    .unwrap();
    let b = lock(&env, &b).await.unwrap();
    assert_eq!(b.invoice.is_kleinunternehmer, 0);

    let r = revoke(&env, &w.entry.id, "Grenze doch nicht überschritten")
        .await
        .unwrap();
    let numbers: Vec<&str> = r
        .consequences
        .affected_invoices
        .iter()
        .map(|x| x.number.as_str())
        .collect();
    assert_eq!(numbers, vec![b.invoice.invoice_number.as_str()]);

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT dedup_key, body FROM notifications
          WHERE dedup_key LIKE 'tax_status_affected:%' ORDER BY dedup_key",
    )
    .fetch_all(&env.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].0, format!("tax_status_affected:add:{}", w.entry.id));
    assert!(
        rows[0].1.contains(&a.invoice.invoice_number),
        "{}",
        rows[0].1
    );
    assert_eq!(
        rows[1].0,
        format!("tax_status_affected:revoke:{}", w.entry.id)
    );
    assert!(
        rows[1].1.contains(&b.invoice.invoice_number),
        "{}",
        rows[1].1
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'tax_status_followups:%'"
        )
        .await,
        2,
        "Folgen-Hinweis je Eintrag und Widerruf"
    );
}

/// V1-R03: Kosten mit Belegdatum im offenen Jahr, gezahlt und als Anlage
/// übernommen im abgeschlossenen Vorjahr: Ein Verzicht ab dem offenen Jahr
/// verschöbe Vorsteuer ins abgeschlossene Jahr und wird abgelehnt.
#[tokio::test]
async fn change_that_moves_input_tax_into_a_closed_year_is_rejected() {
    let env = setup().await;
    let args: expenses_cmd::ExpensesCreateArgs = serde_json::from_value(serde_json::json!({
        "args": { "input": laptop(d(2026, 1, 5), d(2025, 12, 28)) }
    }))
    .unwrap();
    let expense = expenses_cmd::expenses_create(&env.ctx, args)
        .await
        .unwrap()
        .expense
        .id;
    let asset: assets_cmd::AssetsCreateArgs = serde_json::from_value(serde_json::json!({
        "input": {
            "label": "Notebook",
            "acquisitionDate": "2026-01-05",
            "acquisitionCostCents": 119_000,
            "expenseId": expense,
            "vendorContactId": null,
            "depreciationMethod": "linear",
            "usefulLifeYears": 3.0,
            "afaCategory": null,
            "businessSharePercent": 100.0,
            "notes": null
        }
    }))
    .unwrap();
    assets_cmd::assets_create(&env.ctx, asset).await.unwrap();
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
                                        afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (2025, 0, 0, 0, 0, 'test', 33)",
    )
    .execute(&env.pool)
    .await
    .unwrap();

    let p = preview(&env, Some("2026-01-01"), "regelbesteuerung", "verzicht").await;
    assert!(!p.ok);
    assert!(
        p.errors
            .iter()
            .any(|e| e.contains("Vorsteuer") && e.contains("2025")),
        "{:?}",
        p.errors
    );
    let err = add(&env, Some("2026-01-01"), "regelbesteuerung", "verzicht")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("abgeschlossenen Geschäftsjahr 2025"), "{err}");
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM tax_status_changes").await,
        0
    );
    // Ab 2027 bleibt die Kostenposition §19: erlaubt.
    let p = preview(&env, Some("2027-01-01"), "regelbesteuerung", "verzicht").await;
    assert!(p.ok, "{:?}", p.errors);
}

/// V1-R04: Ändert sich der Status zwischen Statusbestimmung und Commit des
/// Festschreibens, bricht der Lock in seiner Schreibtransaktion ab.
#[tokio::test]
async fn lock_rechecks_the_tax_status_in_its_write_transaction() {
    let env = setup().await;
    let id = create_draft(
        &env,
        input(d(2026, 8, 20), d(2026, 8, 20), item(1, 10_000, "E", 0.0)),
    )
    .await
    .unwrap();
    for (aid, p) in [("ap", "/a/p.pdf"), ("ax", "/a/x.xml")] {
        sqlx::query(
            "INSERT INTO archive_entries (id, file_path, file_name, file_hash_sha256,
                                          file_size_bytes, mime_type, source)
             VALUES (?, ?, ?, 'abcd', 1, 'application/pdf', 'test')",
        )
        .bind(aid)
        .bind(p)
        .bind(p)
        .execute(&env.pool)
        .await
        .unwrap();
    }
    // Pipeline hat §19 bestimmt; danach trägt ein zweiter Tab den Wegfall ein.
    add(
        &env,
        Some("2026-08-15"),
        "regelbesteuerung",
        "wegfall_laufend",
    )
    .await
    .unwrap();
    let update = klein_buch_lib::db::repo::invoices::LockUpdate {
        validation_status: "passed",
        validation_report: None,
        pdf_archive_id: "ap",
        xml_archive_id: "ax",
        is_kleinunternehmer: Some(true),
        tax_status_date: Some(d(2026, 8, 20)),
        totals: None,
    };
    let err = klein_buch_lib::db::repo::invoices::lock(&env.pool, &id, &update)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("während des Festschreibens"), "{err}");
    let locked: Option<String> = sqlx::query_scalar("SELECT locked_at FROM invoices WHERE id = ?")
        .bind(&id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(locked.is_none(), "Entwurf bleibt Entwurf");
}

/// V1-R07: Warnungen sperren nicht — Verzicht nach Ablauf der
/// Erklärungsfrist, Rückkehr trotz Vorjahresumsatz über 25.000 €.
#[tokio::test]
async fn preview_warns_on_late_verzicht_and_return_above_threshold() {
    let env = setup().await;
    let p = preview(&env, Some("2024-01-01"), "regelbesteuerung", "verzicht").await;
    assert!(p.ok, "{:?}", p.errors);
    assert!(
        p.warnings
            .iter()
            .any(|w| w.contains("29.02.2026") || w.contains("28.02.2026")),
        "{:?}",
        p.warnings
    );
    let p = preview(&env, Some("2026-01-01"), "regelbesteuerung", "verzicht").await;
    assert!(p.ok && p.warnings.is_empty(), "{:?}", p.warnings);

    // Regelbesteuerung seit Beginn, 30.000 € netto vereinnahmt 2026.
    tax_status::insert(
        &env.pool,
        &tax_status::NewEntry {
            effective_from: klein_buch_lib::domain::tax_status::anfang(),
            status: klein_buch_lib::domain::tax_status::TaxStatus::Regelbesteuerung,
            reason: klein_buch_lib::domain::tax_status::ChangeReason::Initial,
            binding_until: None,
            trigger_document: None,
            note: None,
        },
        d(2026, 6, 1),
    )
    .await
    .unwrap();
    let id = create_draft(
        &env,
        input(d(2026, 9, 1), d(2026, 9, 1), item(1, 3_000_000, "S", 19.0)),
    )
    .await
    .unwrap();
    lock(&env, &id).await.unwrap();
    klein_buch_lib::db::repo::invoices::record_payment(
        &env.pool,
        &id,
        3_570_000,
        "2026-09-10",
        None,
    )
    .await
    .unwrap();
    let p = preview(&env, Some("2027-01-01"), "kleinunternehmer", "rueckkehr").await;
    assert!(p.ok, "{:?}", p.errors);
    assert!(
        p.warnings
            .iter()
            .any(|w| w.contains("30.000,00 €") && w.contains("01.01.2027")),
        "{:?}",
        p.warnings
    );
    add(&env, Some("2027-01-01"), "kleinunternehmer", "rueckkehr")
        .await
        .expect("Warnung sperrt nicht");
}
