//! Leistungszeitraum, Vorausrechnung und Leitweg-ID am Entwurf (ADR 0042
//! Nr. 10–12, Lauf 2026.11.1, Block V3) über die Commands mit Mock-Sidecar:
//!
//!   * Die Leitweg-ID bleibt vom Entwurf bis zum Ausstellen erhalten (BT-10 im
//!     XML), ein übergebener Wert überschreibt sie und wird gespeichert; nach
//!     dem Festschreiben ist sie unveränderlich.
//!   * Eine Vorausrechnung (Leistungsdatum nach dem Rechnungsdatum) lässt sich
//!     ausstellen.
//!   * Eine Rechnung mit Leistungszeitraum trägt BG-14 statt BT-72; eine
//!     eigene Vorlage ohne Zeitraum fällt auf die eingebaute zurück (Hinweis).
//!   * Der Storno übernimmt den Zeitraum des Originals, die Leitweg-ID
//!     unverändert aus der E-Rechnung des Originals, auch eine überlange von
//!     vor V3 (V3-R02).
//!   * Ein unvollständiger oder verkehrter Zeitraum blockiert schon das
//!     Speichern des Entwurfs.
//!
//! Die echte KoSIT-Prüfung von BG-14 steht in `sidecar_real_test.rs`.
//! Testdaten nur mit Dokumentations-Adressen (example.org).

use std::sync::Arc;

use chrono::{Datelike, Days, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::invoices as inv_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};

/// Eigene Vorlage mit §19-Klausel, aber ohne Leistungszeitraum.
const TEMPLATE_TEST: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json(bytes(sys.inputs.at("data-json")))
= Rechnung #data.invoice.number
#data.kleinunternehmer.hinweis_text
"#;

struct Env {
    ctx: Arc<AppCtx>,
    contact_id: String,
    _tmp: tempfile::TempDir,
}

async fn setup() -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::under(tmp.path());
    for d in [&paths.archive_dir, &paths.backups_dir] {
        std::fs::create_dir_all(d).unwrap();
    }
    let templates = paths.inputs_dir.join("pdf-templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(templates.join("default.typ"), TEMPLATE_TEST).unwrap();
    let ctx = Arc::new(AppCtx::new(paths, "test"));
    let pool = klein_buch_lib::db::open_pool(&ctx.paths.db_file, None)
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
    let contact_id = contacts::create(
        &pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Stadtverwaltung Beispiel".into(),
            legal_form: None,
            vat_id: None,
            tax_number: None,
            street: "Rathausplatz 1".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            email: Some("rechnung@example.org".into()),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        },
    )
    .await
    .unwrap()
    .id;
    ctx.set_pool(pool);
    Env {
        ctx,
        contact_id,
        _tmp: tmp,
    }
}

fn today() -> NaiveDate {
    klein_buch_lib::clock::today_berlin()
}

/// Rechnungsdatum: heute, aber im laufenden Jahr (Nummernkreis).
fn input() -> InvoiceInput {
    let t = today();
    InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: t,
        delivery_date: Some(t),
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        due_date: t.checked_add_days(Days::new(14)),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Wartung".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 30_000,
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

/// Zeitraum um heute: Beginn 1. des Monats, Ende 30 Tage nach heute (auch in
/// der Zukunft erlaubt, Vorausrechnung).
fn period() -> (NaiveDate, NaiveDate) {
    let t = today();
    (
        t.with_day0(0).unwrap(),
        t.checked_add_days(Days::new(30)).unwrap(),
    )
}

async fn create(env: &Env, input: InvoiceInput, legacy_ref: Option<&str>) -> String {
    inv_cmd::invoices_create_draft(
        &env.ctx,
        inv_cmd::InvoicesCreateDraftArgs {
            args: inv_cmd::CreateDraftArgs {
                contact_id: env.contact_id.clone(),
                buyer_reference: legacy_ref.map(str::to_string),
                input,
            },
        },
    )
    .await
    .unwrap()
    .invoice
    .id
}

async fn issue(env: &Env, id: &str, buyer_reference: Option<&str>) -> inv_cmd::LockResponse {
    inv_cmd::invoices_lock_and_issue(
        &env.ctx,
        inv_cmd::InvoicesLockAndIssueArgs {
            id: id.to_string(),
            buyer_reference: buyer_reference.map(str::to_string),
        },
    )
    .await
    .unwrap()
}

async fn archived(env: &Env, archive_id: &str) -> Vec<u8> {
    let pool = env.ctx.pool().unwrap();
    klein_buch_lib::archive::store::read_and_verify_silent(&pool, archive_id)
        .await
        .unwrap()
}

async fn row(env: &Env, id: &str) -> klein_buch_lib::db::models::InvoiceRow {
    let pool = env.ctx.pool().unwrap();
    klein_buch_lib::db::repo::invoices::get(&pool, id)
        .await
        .unwrap()
        .unwrap()
}

fn fmt102(d: NaiveDate) -> String {
    d.format("%Y%m%d").to_string()
}

#[tokio::test]
async fn buyer_reference_survives_from_draft_to_issue() {
    let env = setup().await;
    let leitweg = "04011000-12345-67";
    let mut inp = input();
    inp.buyer_reference = Some(format!("  {leitweg} "));
    let id = create(&env, inp.clone(), None).await;
    assert_eq!(
        row(&env, &id).await.buyer_reference.as_deref(),
        Some(leitweg)
    );

    // Bearbeiten ohne Feld in `input`, aber mit der älteren Form `args`.
    inp.buyer_reference = None;
    inv_cmd::invoices_update_draft(
        &env.ctx,
        inv_cmd::InvoicesUpdateDraftArgs {
            id: id.clone(),
            args: inv_cmd::CreateDraftArgs {
                contact_id: env.contact_id.clone(),
                buyer_reference: Some(leitweg.into()),
                input: inp,
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(
        row(&env, &id).await.buyer_reference.as_deref(),
        Some(leitweg)
    );

    // Ausstellen ohne Argument (bzw. mit „N/A“ der älteren Oberfläche): der
    // gespeicherte Wert gilt.
    let resp = issue(&env, &id, Some("N/A")).await;
    let xml = String::from_utf8(archived(&env, &resp.xml_archive_id).await).unwrap();
    assert!(
        xml.contains(&format!(
            "<ram:BuyerReference>{leitweg}</ram:BuyerReference>"
        )),
        "{xml}"
    );
    assert_eq!(resp.invoice.buyer_reference.as_deref(), Some(leitweg));
    let pool = env.ctx.pool().unwrap();
    let err = sqlx::query("UPDATE invoices SET buyer_reference = 'X' WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invoice is locked"), "{err}");

    // Ein übergebener Wert überschreibt den des Entwurfs und wird gespeichert.
    let mut other = input();
    other.buyer_reference = Some("991-00000-00".into());
    let id2 = create(&env, other, None).await;
    let resp = issue(&env, &id2, Some("991-11111-11")).await;
    let xml = String::from_utf8(archived(&env, &resp.xml_archive_id).await).unwrap();
    assert!(xml.contains("<ram:BuyerReference>991-11111-11</ram:BuyerReference>"));
    assert_eq!(
        resp.invoice.buyer_reference.as_deref(),
        Some("991-11111-11")
    );

    // Ohne Leitweg-ID: „N/A“ im XML, Spalte leer.
    let id3 = create(&env, input(), None).await;
    let resp = issue(&env, &id3, None).await;
    let xml = String::from_utf8(archived(&env, &resp.xml_archive_id).await).unwrap();
    assert!(xml.contains("<ram:BuyerReference>N/A</ram:BuyerReference>"));
    assert_eq!(resp.invoice.buyer_reference, None);
}

#[tokio::test]
async fn advance_invoice_with_later_delivery_date_is_issued() {
    let env = setup().await;
    let later = today().checked_add_days(Days::new(45)).unwrap();
    let mut inp = input();
    inp.delivery_date = Some(later);
    let id = create(&env, inp, None).await;
    let resp = issue(&env, &id, None).await;
    let xml = String::from_utf8(archived(&env, &resp.xml_archive_id).await).unwrap();
    assert!(xml.contains("<ram:ActualDeliverySupplyChainEvent>"));
    assert!(xml.contains(&format!(
        "<udt:DateTimeString format=\"102\">{}</udt:DateTimeString>",
        fmt102(later)
    )));
    assert_eq!(resp.invoice.delivery_date, Some(later.to_string()));
}

#[tokio::test]
async fn service_period_is_issued_with_bg14_and_taken_over_by_the_storno() {
    let env = setup().await;
    let (start, end) = period();
    let mut inp = input();
    inp.delivery_date = None;
    inp.service_period_start = Some(start);
    inp.service_period_end = Some(end);
    let id = create(&env, inp, None).await;
    let draft = row(&env, &id).await;
    assert_eq!(draft.service_period_start, Some(start.to_string()));
    assert_eq!(draft.service_period_end, Some(end.to_string()));

    let resp = issue(&env, &id, None).await;
    let xml = String::from_utf8(archived(&env, &resp.xml_archive_id).await).unwrap();
    assert!(xml.contains("<ram:BillingSpecifiedPeriod>"), "{xml}");
    assert!(xml.contains(&fmt102(start)) && xml.contains(&fmt102(end)));
    assert!(
        !xml.contains("<ram:ActualDeliverySupplyChainEvent>"),
        "kein BT-72-Rückfall neben dem Zeitraum"
    );
    let pdf = archived(&env, &resp.pdf_archive_id).await;
    assert!(pdf.starts_with(b"%PDF-"));
    // Die eigene Vorlage kennt den Zeitraum nicht → eingebaute Vorlage + Hinweis.
    let pool = env.ctx.pool().unwrap();
    let notes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE dedup_key = 'period_template_fallback:default'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(notes, 1);
    for sql in [
        "UPDATE invoices SET service_period_end = service_period_start WHERE id = ?",
        "UPDATE invoices SET service_period_start = NULL, service_period_end = NULL WHERE id = ?",
    ] {
        let err = sqlx::query(sql).bind(&id).execute(&pool).await.unwrap_err();
        assert!(
            err.to_string().contains("invoice is locked"),
            "{sql}: {err}"
        );
    }

    // Storno übernimmt den Zeitraum, BG-14 auch im Storno-XML.
    let cancel = inv_cmd::invoices_cancel(
        &env.ctx,
        inv_cmd::InvoicesCancelArgs {
            args: inv_cmd::CancelArgs {
                invoice_id: id.clone(),
                reason: "Doppelt gestellt".into(),
                storno_date: None,
            },
        },
    )
    .await
    .unwrap();
    let st = &cancel.storno.invoice;
    assert_eq!(st.service_period_start, Some(start.to_string()));
    assert_eq!(st.service_period_end, Some(end.to_string()));
    assert_eq!(st.delivery_date, None);
    let st_xml = String::from_utf8(archived(&env, &cancel.storno.xml_archive_id).await).unwrap();
    assert!(st_xml.contains("<ram:BillingSpecifiedPeriod>"));
    assert!(!st_xml.contains("<ram:ActualDeliverySupplyChainEvent>"));
}

#[tokio::test]
async fn incomplete_or_reversed_period_blocks_saving_the_draft() {
    let env = setup().await;
    let (start, end) = period();
    let cases = [
        (Some(start), None, None, "fehlt der Beginn oder das Ende"),
        (Some(end), Some(start), None, "liegt vor dessen Beginn"),
        (Some(start), Some(end), Some(today()), "nicht beides"),
    ];
    for (s, e, d, needle) in cases {
        let mut inp = input();
        inp.delivery_date = d;
        inp.service_period_start = s;
        inp.service_period_end = e;
        let err = inv_cmd::invoices_create_draft(
            &env.ctx,
            inv_cmd::InvoicesCreateDraftArgs {
                args: inv_cmd::CreateDraftArgs {
                    contact_id: env.contact_id.clone(),
                    buyer_reference: None,
                    input: inp,
                },
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains(needle), "{needle}: {err}");
    }
    let mut inp = input();
    inp.buyer_reference = Some("a\u{7}b".into());
    let err = inv_cmd::invoices_create_draft(
        &env.ctx,
        inv_cmd::InvoicesCreateDraftArgs {
            args: inv_cmd::CreateDraftArgs {
                contact_id: env.contact_id.clone(),
                buyer_reference: None,
                input: inp,
            },
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Leitweg-ID"), "{err}");
}

/// V3-R02 (ADR 0042 Nr. 12, ADR 0041 Nr. 250): Ein Original von vor V3 trägt
/// in seiner E-Rechnung eine Käuferreferenz über 200 Zeichen (damals ohne
/// Grenze). Der Storno übernimmt sie unverändert ins XML und lässt sich
/// ausstellen; die Spalte bleibt leer (Quelle ist das Archiv).
#[tokio::test]
async fn storno_keeps_an_overlong_legacy_buyer_reference() {
    let env = setup().await;
    let pool = env.ctx.pool().unwrap();
    let id = create(&env, input(), None).await;
    let resp = issue(&env, &id, Some("991-12345-67")).await;

    // Altbestand nachbilden: XML mit langer BT-10 archivieren und am
    // festgeschriebenen Original hinterlegen (nur im Test am Trigger vorbei).
    let long_ref = format!(
        "Abteilung Beschaffung {}",
        "Referenz-0123456789 ".repeat(11)
    );
    let long_ref = long_ref.trim().to_string();
    assert!(long_ref.chars().count() > 200);
    let xml = String::from_utf8(archived(&env, &resp.xml_archive_id).await).unwrap();
    let legacy_xml = xml.replace(
        "<ram:BuyerReference>991-12345-67</ram:BuyerReference>",
        &format!("<ram:BuyerReference>{long_ref}</ram:BuyerReference>"),
    );
    assert_ne!(xml, legacy_xml);
    let stored = klein_buch_lib::archive::store::store_bytes(
        &pool,
        &env.ctx.paths.archive_dir,
        today().year(),
        klein_buch_lib::archive::store::ArchiveKind::InvoiceXml,
        "ALT-BT10.xml",
        "application/xml",
        legacy_xml.as_bytes(),
    )
    .await
    .unwrap();
    let trigger_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_invoices_immutable'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // Eine Verbindung: eine zweite sähe das Schema beim Vorbereiten veraltet.
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("DROP TRIGGER trg_invoices_immutable")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("UPDATE invoices SET xml_archive_id = ?, buyer_reference = NULL WHERE id = ?")
        .bind(&stored.archive_id)
        .bind(&id)
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(&trigger_sql).execute(&mut *conn).await.unwrap();
    drop(conn);

    let cancel = inv_cmd::invoices_cancel(
        &env.ctx,
        inv_cmd::InvoicesCancelArgs {
            args: inv_cmd::CancelArgs {
                invoice_id: id.clone(),
                reason: "Falscher Empfänger".into(),
                storno_date: None,
            },
        },
    )
    .await
    .expect("Storno trotz langer Alt-Referenz");
    let st_xml = String::from_utf8(archived(&env, &cancel.storno.xml_archive_id).await).unwrap();
    assert!(
        st_xml.contains(&format!(
            "<ram:BuyerReference>{long_ref}</ram:BuyerReference>"
        )),
        "{st_xml}"
    );
    assert_eq!(cancel.storno.invoice.buyer_reference, None);
    assert_eq!(cancel.original.status, "canceled");
}
