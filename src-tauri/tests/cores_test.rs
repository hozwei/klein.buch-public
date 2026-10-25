//! Plan W4: Tests über die Command-Kerne für Stellen, die bisher nur per
//! Host-Smoke geprüft waren (der Command der Desktop-Fassung brauchte ein
//! `AppHandle`):
//!
//! - Storno-Orchestrierung `invoices_cancel` (vorher nur nachgebaut in
//!   `invoice_lock_pipeline_test.rs`).
//! - R3-004: §19-Klausel-Check in der Vorlagen-Vorschau `pdf_template_preview`.
//! - R3-006: UTF-8-Härtung beim Empfang `expenses_parse_einvoice`.
//!
//! Sidecar gemockt (`KLEIN_BUCH_SIDECAR_MOCK=1`, nur in Debug-/Test-Builds).

use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use klein_buch_lib::app_ctx::{AppCtx, DesktopActions};
use klein_buch_lib::commands::{expenses, invoices as inv_cmd, settings};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::error::{Error, Result};

const TEMPLATE_TEST: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json.decode(sys.inputs.at("data-json"))
= Rechnung #data.invoice.number
#data.kleinunternehmer.hinweis_text
"#;

/// Merkt sich Desktop-Aufrufe statt sie auszuführen.
#[derive(Default)]
struct RecordingDesktop(Mutex<Vec<String>>);

impl DesktopActions for RecordingDesktop {
    fn open_path(&self, path: &Path) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .push(format!("open:{}", path.display()));
        Ok(())
    }
    fn reveal(&self, path: &Path) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .push(format!("reveal:{}", path.display()));
        Ok(())
    }
    fn open_url(&self, url: &str) -> Result<()> {
        self.0.lock().unwrap().push(format!("url:{url}"));
        Ok(())
    }
    fn restart(&self) -> Result<()> {
        Ok(())
    }
}

struct Env {
    ctx: Arc<AppCtx>,
    desktop: Arc<RecordingDesktop>,
    _tmp: tempfile::TempDir,
}

async fn setup(unlocked: bool) -> Env {
    std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
    let tmp = tempfile::tempdir().unwrap();
    let paths = Paths::under(tmp.path());
    for d in [&paths.archive_dir, &paths.backups_dir] {
        std::fs::create_dir_all(d).unwrap();
    }
    let templates = paths.inputs_dir.join("pdf-templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(templates.join("default.typ"), TEMPLATE_TEST).unwrap();
    std::fs::write(
        templates.join("ohne-klausel.typ"),
        "= Rechnung ohne Klausel\n",
    )
    .unwrap();

    let desktop = Arc::new(RecordingDesktop::default());
    let ctx = Arc::new(AppCtx::new(paths, desktop.clone(), "test"));
    if unlocked {
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
                is_kleinunternehmer: true,
                default_pdf_template: Some("default".into()),
                default_currency: Some("EUR".into()),
                confirm_waive_paragraph_19: None,
            },
        )
        .await
        .unwrap();
        ctx.set_pool(pool);
    }
    Env {
        ctx,
        desktop,
        _tmp: tmp,
    }
}

async fn locked_invoice(env: &Env) -> String {
    locked_invoice_on(env, klein_buch_lib::clock::today_berlin()).await
}

async fn locked_invoice_on(env: &Env, today: NaiveDate) -> String {
    let pool = env.ctx.pool().unwrap();
    let contact_id = contacts::create(
        &pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: None,
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "12345".into(),
            city: "Musterstadt".into(),
            country_code: "DE".into(),
            email: Some("kunde@example.org".into()),
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
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: today,
        delivery_date: Some(today),
        due_date: today.checked_add_days(chrono::Days::new(14)),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 2.0,
            unit_code: "HUR".into(),
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
    };
    let draft = inv_cmd::create_invoice_draft_from_input(&pool, &contact_id, &input, None)
        .await
        .unwrap();
    inv_cmd::run_lock_pipeline(&pool, &env.ctx.paths, &draft.invoice.id, "N/A")
        .await
        .unwrap();
    draft.invoice.id
}

#[tokio::test]
async fn storno_core_cancels_original_and_locks_storno() {
    let env = setup(true).await;
    let id = locked_invoice(&env).await;

    let resp = inv_cmd::invoices_cancel(
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
    assert_eq!(resp.original.status, "canceled");
    assert!(resp.storno.invoice.invoice_number.starts_with("ST-"));
    assert!(resp.storno.invoice.locked_at.is_some());
    assert_eq!(
        resp.storno.invoice.is_storno_for.as_deref(),
        Some(id.as_str())
    );

    let pool = env.ctx.pool().unwrap();
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'invoice.cancel' AND entity_id = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);

    // Zweites Storno derselben Rechnung wird abgelehnt.
    let err = inv_cmd::invoices_cancel(
        &env.ctx,
        inv_cmd::InvoicesCancelArgs {
            args: inv_cmd::CancelArgs {
                invoice_id: id,
                reason: "nochmal".into(),
                storno_date: None,
            },
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("bereits storniert"), "{err}");
}

/// W14 (GoBD): Storno-Datum nicht vor der Rechnung, nicht in der Zukunft, nicht
/// in einem abgeschlossenen Geschäftsjahr — die EÜR eines festgeschriebenen
/// Jahres darf sich nicht nachträglich ändern.
#[tokio::test]
async fn storno_date_must_be_in_an_open_year_after_the_invoice() {
    use chrono::Datelike as _;
    let env = setup(true).await;
    let today = klein_buch_lib::clock::today_berlin();
    let last_year = today.year() - 1;
    let invoice_date = NaiveDate::from_ymd_opt(last_year, 12, 10).unwrap();
    let id = locked_invoice_on(&env, invoice_date).await;
    let cancel = |date: Option<NaiveDate>| {
        inv_cmd::invoices_cancel(
            &env.ctx,
            inv_cmd::InvoicesCancelArgs {
                args: inv_cmd::CancelArgs {
                    invoice_id: id.clone(),
                    reason: "Fehler im Betrag".into(),
                    storno_date: date,
                },
            },
        )
    };
    let err = cancel(NaiveDate::from_ymd_opt(last_year, 12, 1))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("Storno-Datum"),
        "vor der Rechnung: {err}"
    );
    let err = cancel(today.checked_add_days(chrono::Days::new(1)))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Storno-Datum"), "Zukunft: {err}");

    let pool = env.ctx.pool().unwrap();
    let session = klein_buch_lib::backup::BackupSession::default();
    session.set("passphrase-1234".into());
    klein_buch_lib::fiscal_year::lock::close_year(
        &pool,
        &env.ctx.paths,
        &session,
        last_year as i64,
        klein_buch_lib::clock::berlin_to_utc(today, 12, 0),
    )
    .await
    .unwrap();
    let err = cancel(NaiveDate::from_ymd_opt(last_year, 12, 20))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("abgeschlossen"),
        "geschlossenes Jahr: {err}"
    );
    // Gegenprüfung Lauf E: Die Ablehnungen hinterlassen weder Beleg noch Zählerstand.
    let stornos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices WHERE is_storno_for = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stornos, 0, "kein Storno-Entwurf nach Ablehnung");
    assert_eq!(
        klein_buch_lib::db::numbering::peek_seq(
            &pool,
            klein_buch_lib::domain::numbering::DocType::StornoInvoice,
            last_year
        )
        .await
        .unwrap(),
        0,
        "kein Zählerstand im abgeschlossenen Jahr"
    );
    let resp = cancel(Some(today)).await.unwrap();
    assert_eq!(
        resp.original.status, "canceled",
        "Storno im offenen Jahr geht"
    );
    // Lauf E: das abgeschlossene Jahr bekommt keinen neuen Beleg.
    assert_eq!(resp.storno.invoice.fiscal_year, i64::from(today.year()));
    assert!(resp
        .storno
        .invoice
        .invoice_number
        .starts_with(&format!("ST-{}-", today.year())));
}

#[tokio::test]
async fn storno_takes_year_and_number_from_the_storno_date() {
    // Lauf E (GoBD Rz. 51): Geschäftsjahr und Nummernkreis des Stornos folgen
    // dem Storno-Datum; der Bezug aufs Original steht als Nummer + Datum im XML.
    use chrono::Datelike as _;
    use klein_buch_lib::db::numbering::peek_seq;
    use klein_buch_lib::domain::numbering::DocType;
    let env = setup(true).await;
    let pool = env.ctx.pool().unwrap();
    let today = klein_buch_lib::clock::today_berlin();
    let this_year = today.year();
    let last_year = this_year - 1;
    let cancel = |id: String| {
        inv_cmd::invoices_cancel(
            &env.ctx,
            inv_cmd::InvoicesCancelArgs {
                args: inv_cmd::CancelArgs {
                    invoice_id: id,
                    reason: "Fehler im Betrag".into(),
                    storno_date: Some(today),
                },
            },
        )
    };

    // Original aus dem (offenen) Vorjahr, Storno heute.
    let old = locked_invoice_on(&env, NaiveDate::from_ymd_opt(last_year, 12, 10).unwrap()).await;
    let old_row = klein_buch_lib::db::repo::invoices::get(&pool, &old)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_row.fiscal_year, i64::from(last_year));
    let resp = cancel(old.clone()).await.unwrap();
    assert_eq!(resp.storno.invoice.fiscal_year, i64::from(this_year));
    assert_eq!(
        resp.storno.invoice.invoice_number,
        format!("ST-{this_year}-0001")
    );
    assert_eq!(
        peek_seq(&pool, DocType::StornoInvoice, last_year)
            .await
            .unwrap(),
        0,
        "Nummernkreis des Vorjahres bleibt unberührt"
    );
    let xml = String::from_utf8(
        klein_buch_lib::archive::store::read_and_verify_silent(&pool, &resp.storno.xml_archive_id)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        xml.contains(&format!(
            "<ram:IssuerAssignedID>{}</ram:IssuerAssignedID>",
            old_row.invoice_number
        )),
        "BT-25 = Nummer des Originals"
    );
    assert!(
        xml.contains(&format!(
            "<qdt:DateTimeString format=\"102\">{last_year}1210</qdt:DateTimeString>"
        )),
        "BT-26 = Rechnungsdatum des Originals"
    );
    assert!(!xml.contains(&old), "keine interne ID im XML");

    // Storno im selben Jahr: fortlaufend im Kreis des Storno-Jahres.
    let fresh = locked_invoice_on(&env, today).await;
    let resp2 = cancel(fresh).await.unwrap();
    assert_eq!(resp2.storno.invoice.fiscal_year, i64::from(this_year));
    assert_eq!(
        resp2.storno.invoice.invoice_number,
        format!("ST-{this_year}-0002")
    );
}

#[tokio::test]
async fn storno_drafts_only_complete_through_cancel_and_are_reused() {
    // Gegenprüfung Lauf E (S1/S2): Ein liegengebliebener Storno-Entwurf lässt
    // sich nicht ohne das Original festschreiben; „Stornieren“ verwendet ihn
    // wieder (keine zweite ST-Nummer); über die API entsteht kein Storno-Entwurf.
    use klein_buch_lib::db::numbering::peek_seq;
    use klein_buch_lib::domain::numbering::DocType;
    let env = setup(true).await;
    let pool = env.ctx.pool().unwrap();
    let today = klein_buch_lib::clock::today_berlin();
    let fy = chrono::Datelike::year(&today);
    let id = locked_invoice_on(&env, today).await;
    let original = klein_buch_lib::db::repo::invoices::get(&pool, &id)
        .await
        .unwrap()
        .unwrap();
    let storno_like = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: today,
        delivery_date: Some(today),
        due_date: None,
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Storno".into(),
            quantity: 2.0,
            unit_code: "HUR".into(),
            unit_price_cents: -30_000,
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
        is_storno_for: Some(id.clone()),
        cancel_reason: Some("liegengeblieben".into()),
    };

    // Über die API: abgelehnt.
    let err = inv_cmd::invoices_create_draft(
        &env.ctx,
        inv_cmd::InvoicesCreateDraftArgs {
            args: inv_cmd::CreateDraftArgs {
                contact_id: original.contact_id.clone(),
                buyer_reference: None,
                input: storno_like.clone(),
            },
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Stornieren"), "{err}");

    // Liegengebliebener Entwurf (wie nach einer gescheiterten Pipeline).
    let stale =
        inv_cmd::create_invoice_draft_from_input(&pool, &original.contact_id, &storno_like, None)
            .await
            .unwrap();
    assert_eq!(
        peek_seq(&pool, DocType::StornoInvoice, fy).await.unwrap(),
        1
    );
    let err = inv_cmd::invoices_lock_and_issue(
        &env.ctx,
        inv_cmd::InvoicesLockAndIssueArgs {
            id: stale.invoice.id.clone(),
            buyer_reference: None,
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Stornieren"), "{err}");

    // „Stornieren“ schließt genau diesen Entwurf ab.
    let cancel = || {
        inv_cmd::invoices_cancel(
            &env.ctx,
            inv_cmd::InvoicesCancelArgs {
                args: inv_cmd::CancelArgs {
                    invoice_id: id.clone(),
                    reason: "Fehler im Betrag".into(),
                    storno_date: Some(today),
                },
            },
        )
    };
    let resp = cancel().await.unwrap();
    assert_eq!(
        resp.storno.invoice.id, stale.invoice.id,
        "Entwurf wiederverwendet"
    );
    assert_eq!(
        resp.storno.invoice.invoice_number,
        stale.invoice.invoice_number
    );
    assert_eq!(
        resp.storno.invoice.cancel_reason.as_deref(),
        Some("Fehler im Betrag")
    );
    assert_eq!(resp.original.status, "canceled");
    assert_eq!(
        peek_seq(&pool, DocType::StornoInvoice, fy).await.unwrap(),
        1,
        "keine zweite ST-Nummer"
    );
    let err = cancel().await.unwrap_err();
    assert!(err.to_string().contains("bereits storniert"), "{err}");
}

#[tokio::test]
async fn no_invoice_draft_or_number_in_a_closed_year() {
    // Gegenprüfung Lauf E (S2): Ein Entwurf mit Datum im abgeschlossenen Jahr
    // verbraucht dort keine Nummer.
    use chrono::Datelike as _;
    use klein_buch_lib::db::numbering::peek_seq;
    use klein_buch_lib::domain::numbering::DocType;
    let env = setup(true).await;
    let pool = env.ctx.pool().unwrap();
    let today = klein_buch_lib::clock::today_berlin();
    let last_year = today.year() - 1;
    let session = klein_buch_lib::backup::BackupSession::default();
    session.set("passphrase-1234".into());
    klein_buch_lib::fiscal_year::lock::close_year(
        &pool,
        &env.ctx.paths,
        &session,
        last_year as i64,
        klein_buch_lib::clock::berlin_to_utc(today, 12, 0),
    )
    .await
    .unwrap();
    let id = locked_invoice_on(&env, today).await;
    let original = klein_buch_lib::db::repo::invoices::get(&pool, &id)
        .await
        .unwrap()
        .unwrap();
    let date = NaiveDate::from_ymd_opt(last_year, 12, 20).unwrap();
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: date,
        delivery_date: Some(date),
        due_date: None,
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Nachtrag".into(),
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
    };
    let err = inv_cmd::create_invoice_draft_from_input(&pool, &original.contact_id, &input, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("abgeschlossen"), "{err}");
    assert_eq!(
        peek_seq(&pool, DocType::Invoice, last_year).await.unwrap(),
        0
    );
}

#[tokio::test]
async fn cores_report_locked_without_pool() {
    let env = setup(false).await;
    let err = inv_cmd::invoices_cancel(
        &env.ctx,
        inv_cmd::InvoicesCancelArgs {
            args: inv_cmd::CancelArgs {
                invoice_id: "x".into(),
                reason: "y".into(),
                storno_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            },
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, Error::Locked), "{err:?}");
}

/// R3-004: Bei §19 lehnt die Vorschau eine Vorlage ohne Klausel-Marker ab und
/// rendert eine korrekte Vorlage (seit W7 als Download-Ticket, kein Öffnen).
#[tokio::test]
async fn template_preview_enforces_paragraph_19_clause() {
    let env = setup(true).await;
    let err = settings::pdf_template_preview(
        &env.ctx,
        settings::PdfTemplatePreviewArgs {
            name: "ohne-klausel".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("nicht §19-kompatibel"), "{err}");
    assert!(env.ctx.downloads.is_empty(), "keine Vorschau");

    let ticket = settings::pdf_template_preview(
        &env.ctx,
        settings::PdfTemplatePreviewArgs {
            name: "default".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        env.desktop.0.lock().unwrap().is_empty(),
        "kein Desktop-Öffner"
    );
    assert!(ticket.file_name.ends_with(".pdf"));
    let entry = env.ctx.downloads.take(&ticket.download_id).unwrap();
    assert!(std::fs::read(&entry.path).unwrap().starts_with(b"%PDF"));
}

/// R3-006: Eine XRechnung in ISO-8859-1 („Müller“ als 0xFC) wird mit klarer
/// Meldung abgelehnt statt still zu „M\u{FFFD}ller“ zu werden.
#[tokio::test]
async fn einvoice_parse_rejects_non_utf8_xml() {
    let env = setup(false).await;
    let mut bytes = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><Invoice><Name>M".to_vec();
    bytes.push(0xFC);
    bytes.extend_from_slice(b"ller</Name></Invoice>");
    // Seit W7 lehnt schon der Upload (Art „erechnung“) die Datei ab.
    let err = env
        .ctx
        .uploads
        .put_bytes(
            klein_buch_lib::uploads::UploadKind::EInvoice,
            "eingang.xml",
            &bytes,
        )
        .unwrap_err();
    assert!(err.to_string().contains("nicht UTF-8-codiert"), "{err}");
    // Der Kern selbst (Ablage-Ordner) prüft weiterhin.
    let err = expenses::parse_einvoice_with_paths(&env.ctx.paths, bytes, "eingang.xml".into())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("nicht UTF-8-codiert"), "{err}");
}

/// Restore-Audit-Kette über den `AppCtx`: `prepare_filesystem` wendet den
/// vorgemerkten Restore an und puffert den Audit in `ctx.pending_restore`;
/// `open_and_init` (Entsperren) schreibt ihn als `backup.restore.applied`.
#[tokio::test]
async fn restore_audit_flows_through_app_ctx() {
    let env = setup(false).await;
    let paths = env.ctx.paths.clone();
    std::fs::create_dir_all(paths.resource_dir.join("inputs")).unwrap();
    // Gestagte DB: frisch migriert, im Staging abgelegt.
    let staging = paths
        .data_dir
        .join(klein_buch_lib::backup::restore::RESTORE_STAGING);
    std::fs::create_dir_all(&staging).unwrap();
    let staged_db = staging.join("klein-buch.sqlite");
    {
        let p = klein_buch_lib::db::open_pool(&staged_db, None)
            .await
            .unwrap();
        MIGRATOR.run(&p).await.unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&p)
            .await
            .unwrap();
        p.close().await;
    }
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", staged_db.display()));
    }
    let info = serde_json::json!({
        "staging": staging.to_string_lossy(),
        "source": "test.kbk",
        "staged_at": "2026-10-23T12:00:00Z",
        "format_version": 2
    });
    std::fs::write(
        paths
            .data_dir
            .join(klein_buch_lib::backup::restore::RESTORE_MARKER),
        info.to_string(),
    )
    .unwrap();

    klein_buch_lib::db::prepare_filesystem(&env.ctx).unwrap();
    assert!(env.ctx.pending_restore.0.lock().unwrap().is_some());
    let pool = klein_buch_lib::db::open_and_init(&env.ctx, None)
        .await
        .unwrap();
    assert!(env.ctx.pending_restore.0.lock().unwrap().is_none());
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'backup.restore.applied'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1);
}
