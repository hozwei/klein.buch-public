//! Integration-Tests für `db::repo::quotes` + `db::repo::attachments` (Block 6).
//!
//! Nutzt eine tempfile-SQLite-DB mit den echten Migrationen (inkl.
//! 0005_quotes.sql) — damit kommen DB-Triggers (GoBD) und der CHECK auf
//! `status`/`tax_category_code` zum Tragen.

use chrono::NaiveDate;
use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::db::numbering;
use klein_buch_lib::db::repo::invoices::{self, BuyerSnapshot, SellerSnapshot};
use klein_buch_lib::db::repo::{attachments, contacts, quotes};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{self, InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::domain::numbering::DocType;
use klein_buch_lib::domain::quote::{self, QuoteInput, QuoteItemInput};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

async fn mk_contact(pool: &SqlitePool, name: &str) -> String {
    let input = ContactInput {
        contact_type: ContactType::Customer,
        name: name.into(),
        legal_form: Some("GmbH".into()),
        vat_id: Some("DE123456789".into()),
        tax_number: None,
        street: "Hauptstr. 7".into(),
        postal_code: "80331".into(),
        city: "München".into(),
        country_code: "DE".into(),
        email: Some("info@kunde.de".into()),
        phone: None,
        iban: None,
        bic: None,
        accepts_einvoice: true,
        notes: None,
    };
    contacts::create(pool, &input).await.unwrap().id
}

fn seller<'a>() -> SellerSnapshot<'a> {
    SellerSnapshot {
        name: "Wildbach Computerhilfe",
        street: "Beispielweg 1",
        postal_code: "84028",
        city: "Landshut",
        tax_number: Some("123/456/78901"),
        vat_id: None,
    }
}

fn quote_input() -> QuoteInput {
    QuoteInput {
        quote_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
        valid_until: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
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
        notes: Some("Bindefrist 30 Tage".into()),
        pdf_template: "default".into(),
    }
}

fn buyer<'a>() -> BuyerSnapshot<'a> {
    BuyerSnapshot {
        name: "Kunde GmbH",
        street: Some("Hauptstr. 7"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: Some("DE123456789"),
        email: Some("info@kunde.de"),
    }
}

async fn mk_draft(pool: &SqlitePool, contact_id: &str, number: &str) -> String {
    let input = quote_input();
    let totals = quote::compute_totals(&input.items);
    let payload = quotes::DraftCreatePayload {
        contact_id: contact_id.to_string(),
        fiscal_year: 2026,
        is_kleinunternehmer: true,
        input,
    };
    quotes::create_draft(pool, &payload, number, &seller(), &buyer(), &totals)
        .await
        .unwrap()
        .id
}

/// Legt eine Rechnungs-Draft an — wie es der Konvertierungs-Command tut
/// (über den gemeinsamen Helper). `derived_from_quote_id` verknüpft sie mit
/// dem Ursprungsangebot.
async fn mk_invoice_draft(
    pool: &SqlitePool,
    contact_id: &str,
    number: &str,
    derived_from_quote_id: Option<&str>,
) -> String {
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        delivery_date: None,
        due_date: None,
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
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
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    let totals = invoice::compute_totals(&input.items);
    let buyer = BuyerSnapshot {
        name: "Kunde GmbH",
        street: Some("Hauptstr. 7"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: Some("DE123456789"),
        email: Some("info@kunde.de"),
    };
    let payload = invoices::DraftCreatePayload {
        contact_id: contact_id.to_string(),
        fiscal_year: 2026,
        is_kleinunternehmer: true,
        input,
        derived_from_quote_id: derived_from_quote_id.map(|s| s.to_string()),
    };
    invoices::create_draft(pool, &payload, number, &seller(), &buyer, &totals)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn create_then_get() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;

    let row = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.quote_number, "AN-2026-0001");
    assert_eq!(row.status, "draft");
    assert!(row.locked_at.is_none());
    assert_eq!(row.net_amount_cents, 50_000);
    assert_eq!(row.tax_amount_cents, 0);
    assert_eq!(row.gross_amount_cents, 50_000);
    assert_eq!(row.is_kleinunternehmer, 1);

    let items = quotes::get_items(&pool, &id).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].net_amount_cents, 50_000);
}

#[tokio::test]
async fn numbering_is_gap_free_per_year() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let n1 = numbering::next_number(&pool, DocType::Quote, 2026)
        .await
        .unwrap();
    let n2 = numbering::next_number(&pool, DocType::Quote, 2026)
        .await
        .unwrap();
    assert_eq!(n1, "AN-2026-0001");
    assert_eq!(n2, "AN-2026-0002");
    let _ = mk_draft(&pool, &cid, &n1).await;
    let _ = mk_draft(&pool, &cid, &n2).await;
    let list = quotes::list(&pool, &quotes::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn list_filters_by_status_and_inactive() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let a = mk_draft(&pool, &cid, "AN-2026-0001").await;
    let b = mk_draft(&pool, &cid, "AN-2026-0002").await;
    quotes::cancel(&pool, &b, "Kunde abgesprungen")
        .await
        .unwrap();

    let all = quotes::list(&pool, &quotes::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    let active = quotes::list(
        &pool,
        &quotes::ListFilter {
            include_inactive: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, a);

    let drafts = quotes::list(
        &pool,
        &quotes::ListFilter {
            status: Some("draft".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].id, a);
}

#[tokio::test]
async fn issue_locks_and_sets_sent() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;

    quotes::issue(&pool, &id).await.unwrap();
    let row = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.status, "sent");
    assert!(row.locked_at.is_some());
    assert!(row.sent_at.is_some());

    // Doppel-Issue ist ein Fehler.
    assert!(quotes::issue(&pool, &id).await.is_err());
}

#[tokio::test]
async fn locked_quote_core_fields_are_immutable() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;
    quotes::issue(&pool, &id).await.unwrap();

    // Direkter Versuch, ein Kernfeld zu ändern → trg_quotes_immutable ABORT.
    let res = sqlx::query("UPDATE quotes SET net_amount_cents = 999 WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "Kernfeld-Update muss am Trigger scheitern");

    // State-Transition-Felder bleiben erlaubt (accept ändert nur status/accepted_at).
    quotes::accept(&pool, &id, "2026-05-20").await.unwrap();
    assert_eq!(
        quotes::get(&pool, &id).await.unwrap().unwrap().status,
        "accepted"
    );
}

#[tokio::test]
async fn accept_only_from_sent() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;

    // Draft → accept ist nicht erlaubt.
    assert!(quotes::accept(&pool, &id, "2026-05-20").await.is_err());

    quotes::issue(&pool, &id).await.unwrap();
    quotes::accept(&pool, &id, "2026-05-20").await.unwrap();
    let row = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.status, "accepted");
    assert_eq!(row.accepted_at.as_deref(), Some("2026-05-20"));
}

#[tokio::test]
async fn reject_only_from_sent() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;
    assert!(quotes::reject(&pool, &id).await.is_err());

    quotes::issue(&pool, &id).await.unwrap();
    quotes::reject(&pool, &id).await.unwrap();
    let row = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.status, "rejected");
    assert!(row.rejected_at.is_some());
}

#[tokio::test]
async fn cancel_sets_canceled_and_is_idempotent_guarded() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;

    quotes::cancel(&pool, &id, "Zurückgezogen").await.unwrap();
    let row = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.status, "canceled");
    assert_eq!(row.canceled_reason.as_deref(), Some("Zurückgezogen"));

    // Doppel-Storno verhindert.
    assert!(quotes::cancel(&pool, &id, "nochmal").await.is_err());
}

#[tokio::test]
async fn accept_with_attachment_shows_in_detail() {
    let (pool, dir) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;
    quotes::issue(&pool, &id).await.unwrap();

    // Vertrag write-once archivieren + verknüpfen (wie quotes_accept im Command).
    let archive_root = dir.path().join("archive");
    let stored = store_bytes(
        &pool,
        &archive_root,
        2026,
        ArchiveKind::Attachment,
        "AN-2026-0001-vertrag.pdf",
        "application/pdf",
        b"%PDF-1.7 signed",
    )
    .await
    .unwrap();
    let sort = attachments::count_for_parent(&pool, "quote", &id)
        .await
        .unwrap();
    assert_eq!(sort, 0);
    attachments::create(
        &pool,
        "quote",
        &id,
        &stored.archive_id,
        Some("Unterschriebener Vertrag"),
        sort,
    )
    .await
    .unwrap();
    quotes::accept(&pool, &id, "2026-05-20").await.unwrap();

    let detail = quotes::get_detail(&pool, &id).await.unwrap().unwrap();
    assert_eq!(detail.quote.status, "accepted");
    assert_eq!(detail.attachments.len(), 1);
    assert_eq!(detail.attachments[0].file_name, "AN-2026-0001-vertrag.pdf");
    assert_eq!(
        detail.attachments[0].label.as_deref(),
        Some("Unterschriebener Vertrag")
    );
    assert_eq!(detail.attachments[0].mime_type, "application/pdf");
}

#[tokio::test]
async fn get_detail_includes_buyer_and_items() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;

    let detail = quotes::get_detail(&pool, &id).await.unwrap().unwrap();
    assert_eq!(detail.items.len(), 1);
    assert!(detail.buyer.is_some());
    assert_eq!(detail.buyer.unwrap().name, "Kunde GmbH");
    assert_eq!(detail.attachments.len(), 0);
}

// ---- Konvertierung Angebot → Rechnung (Block 7) ----

#[tokio::test]
async fn mark_converted_only_from_accepted() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let qid = mk_draft(&pool, &cid, "AN-2026-0001").await;
    let dummy_inv = mk_invoice_draft(&pool, &cid, "RE-2026-0001", None).await;

    // draft → mark_converted nicht erlaubt.
    assert!(quotes::mark_converted(&pool, &qid, &dummy_inv)
        .await
        .is_err());

    quotes::issue(&pool, &qid).await.unwrap();
    // sent → nicht erlaubt.
    assert!(quotes::mark_converted(&pool, &qid, &dummy_inv)
        .await
        .is_err());

    quotes::accept(&pool, &qid, "2026-05-20").await.unwrap();
    // accepted → ok.
    let inv = mk_invoice_draft(&pool, &cid, "RE-2026-0002", Some(&qid)).await;
    quotes::mark_converted(&pool, &qid, &inv).await.unwrap();

    let row = quotes::get(&pool, &qid).await.unwrap().unwrap();
    assert_eq!(row.status, "converted");
    assert_eq!(row.converted_invoice_id.as_deref(), Some(inv.as_str()));
    assert!(row.converted_at.is_some());

    // Zweite Konvertierung blockiert (status converted ≠ accepted).
    assert!(quotes::mark_converted(&pool, &qid, &inv).await.is_err());
}

#[tokio::test]
async fn converted_invoice_links_back_to_quote() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let qid = mk_draft(&pool, &cid, "AN-2026-0001").await;
    quotes::issue(&pool, &qid).await.unwrap();
    quotes::accept(&pool, &qid, "2026-05-20").await.unwrap();

    let inv_id = mk_invoice_draft(&pool, &cid, "RE-2026-0001", Some(&qid)).await;
    quotes::mark_converted(&pool, &qid, &inv_id).await.unwrap();

    // derived_from_quote_id wurde beim Draft-INSERT gesetzt + round-trips.
    let inv = invoices::get(&pool, &inv_id).await.unwrap().unwrap();
    assert_eq!(inv.derived_from_quote_id.as_deref(), Some(qid.as_str()));

    // Angebots-Seite zeigt nach Konvertierung auf die Rechnung.
    let q = quotes::get(&pool, &qid).await.unwrap().unwrap();
    assert_eq!(q.converted_invoice_id.as_deref(), Some(inv_id.as_str()));
}

// ---- Buyer-Snapshot + DSGVO-Anonymisierung (Block 19) ----

#[tokio::test]
async fn buyer_snapshot_persisted_refreshed_and_survives_anonymization() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_contact(&pool, "Kunde GmbH").await;
    let id = mk_draft(&pool, &cid, "AN-2026-0001").await;

    // create_draft hat den Empfänger-Snapshot mitgeschrieben.
    let q = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(q.buyer_name.as_deref(), Some("Kunde GmbH"));
    assert_eq!(q.buyer_city.as_deref(), Some("München"));

    // Refresh (wie quotes_issue im Command) ist auf Drafts erlaubt …
    let refreshed = BuyerSnapshot {
        name: "Kunde GmbH (aktualisiert)",
        street: Some("Neue Str. 1"),
        postal_code: Some("84028"),
        city: Some("Landshut"),
        country_code: "DE",
        vat_id: Some("DE123456789"),
        email: Some("info@kunde.de"),
    };
    quotes::set_buyer_snapshot(&pool, &id, &refreshed)
        .await
        .unwrap();

    quotes::issue(&pool, &id).await.unwrap();

    // … aber auf festgeschriebenem Angebot gesperrt (GoBD).
    assert!(quotes::set_buyer_snapshot(&pool, &id, &refreshed)
        .await
        .is_err());

    // Kontakt anonymisieren (keine offenen Entwürfe mehr) → Snapshot bleibt
    // der eingefrorene Stand, der Live-Kontakt wird zum Platzhalter.
    contacts::anonymize(&pool, &cid).await.unwrap();
    let after = quotes::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(
        after.buyer_name.as_deref(),
        Some("Kunde GmbH (aktualisiert)")
    );
    let contact = contacts::get(&pool, &cid).await.unwrap().unwrap();
    assert!(contact.name.starts_with("Anonymisiert #"));
    assert!(contact.anonymized_at.is_some());
}
