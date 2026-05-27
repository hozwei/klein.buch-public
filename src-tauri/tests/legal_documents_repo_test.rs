//! Integration-Tests für `db::repo::legal_documents` (Block 8, Migration 0006).
//!
//! Läuft gegen eine tempfile-SQLite-DB mit den echten Migrationen — damit
//! greifen die GoBD-Trigger (append-only, immutable) und der partial-unique
//! Index `uq_legal_documents_active`.

use chrono::NaiveDate;
use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::db::repo::invoices::{BuyerSnapshot, SellerSnapshot};
use klein_buch_lib::db::repo::{contacts, legal_documents, quotes};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::quote::{self, QuoteInput, QuoteItemInput};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

async fn setup() -> (SqlitePool, tempfile::TempDir) {
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

/// Archiviert ein Fake-PDF und gibt die archive_entry_id zurück (FK-Ziel für
/// legal_documents.archive_entry_id).
async fn archive_pdf(pool: &SqlitePool, root: &Path, name: &str) -> String {
    let stored = store_bytes(
        pool,
        root,
        2026,
        ArchiveKind::LegalDocument,
        name,
        "application/pdf",
        format!("%PDF-1.7 {name}").as_bytes(),
    )
    .await
    .unwrap();
    stored.archive_id
}

async fn mk_quote(pool: &SqlitePool, root: &Path) -> String {
    let _ = root;
    let contact = contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: "Kunde GmbH".into(),
            legal_form: None,
            vat_id: None,
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
        },
    )
    .await
    .unwrap();
    let input = QuoteInput {
        quote_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
        valid_until: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        currency_code: "EUR".into(),
        items: vec![QuoteItemInput {
            position: 1,
            description: "Beratung".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 10_000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        pdf_template: "default".into(),
    };
    let totals = quote::compute_totals(&input.items);
    let payload = quotes::DraftCreatePayload {
        contact_id: contact.id.clone(),
        fiscal_year: 2026,
        is_kleinunternehmer: true,
        input,
    };
    let seller = SellerSnapshot {
        name: "Wildbach Computerhilfe",
        street: "Beispielweg 1",
        postal_code: "84028",
        city: "Landshut",
        tax_number: Some("123/456/78901"),
        vat_id: None,
    };
    let buyer = BuyerSnapshot {
        name: "Kunde GmbH",
        street: Some("Hauptstr. 7"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: Some("DE123456789"),
        email: Some("info@kunde.de"),
    };
    quotes::create_draft(pool, &payload, "AN-2026-0001", &seller, &buyer, &totals)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn versioning_increments_per_doc_type() {
    let (pool, dir) = setup().await;
    let root = dir.path().join("archive");
    let a1 = archive_pdf(&pool, &root, "agb-1.pdf").await;
    let a2 = archive_pdf(&pool, &root, "agb-2.pdf").await;
    let p1 = archive_pdf(&pool, &root, "privacy-1.pdf").await;

    let v1 = legal_documents::create_version(&pool, "agb", &a1, "AGB v1")
        .await
        .unwrap();
    let v2 = legal_documents::create_version(&pool, "agb", &a2, "AGB v2")
        .await
        .unwrap();
    let pv1 = legal_documents::create_version(&pool, "privacy", &p1, "DS v1")
        .await
        .unwrap();

    assert_eq!(v1.version, 1);
    assert_eq!(v2.version, 2);
    assert_eq!(pv1.version, 1, "Versionszähler ist pro doc_type unabhängig");
    assert_eq!(v1.is_active, 0, "neue Versionen sind zunächst inaktiv");

    let all = legal_documents::list(&pool).await.unwrap();
    assert_eq!(all.len(), 3);
}

#[tokio::test]
async fn activate_enforces_single_active_per_type() {
    let (pool, dir) = setup().await;
    let root = dir.path().join("archive");
    let a1 = archive_pdf(&pool, &root, "agb-1.pdf").await;
    let a2 = archive_pdf(&pool, &root, "agb-2.pdf").await;
    let v1 = legal_documents::create_version(&pool, "agb", &a1, "AGB v1")
        .await
        .unwrap();
    let v2 = legal_documents::create_version(&pool, "agb", &a2, "AGB v2")
        .await
        .unwrap();

    legal_documents::activate(&pool, &v1.id).await.unwrap();
    assert_eq!(
        legal_documents::get_active(&pool, "agb")
            .await
            .unwrap()
            .unwrap()
            .id,
        v1.id
    );

    // Aktivieren von v2 deaktiviert v1 (partial-unique Index nicht verletzt).
    legal_documents::activate(&pool, &v2.id).await.unwrap();
    let active = legal_documents::get_active(&pool, "agb")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.id, v2.id);
    assert_eq!(
        legal_documents::get(&pool, &v1.id)
            .await
            .unwrap()
            .unwrap()
            .is_active,
        0
    );

    // Deaktivieren → kein aktives Dokument mehr.
    legal_documents::deactivate(&pool, &v2.id).await.unwrap();
    assert!(legal_documents::get_active(&pool, "agb")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn core_fields_immutable_and_no_delete() {
    let (pool, dir) = setup().await;
    let root = dir.path().join("archive");
    let a1 = archive_pdf(&pool, &root, "agb-1.pdf").await;
    let v1 = legal_documents::create_version(&pool, "agb", &a1, "AGB v1")
        .await
        .unwrap();

    // Kernfeld-Update → trg_legal_documents_immutable ABORT.
    let res = sqlx::query("UPDATE legal_documents SET title = 'gehackt' WHERE id = ?")
        .bind(&v1.id)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "Kernfeld-Update muss am Trigger scheitern");

    // DELETE → trg_legal_documents_no_delete ABORT.
    let del = sqlx::query("DELETE FROM legal_documents WHERE id = ?")
        .bind(&v1.id)
        .execute(&pool)
        .await;
    assert!(del.is_err(), "Löschen muss am Trigger scheitern");

    // is_active-Änderung (Aktivieren) bleibt erlaubt.
    legal_documents::activate(&pool, &v1.id).await.unwrap();
    assert_eq!(
        legal_documents::get(&pool, &v1.id)
            .await
            .unwrap()
            .unwrap()
            .is_active,
        1
    );
}

#[tokio::test]
async fn bind_active_for_quote_is_idempotent() {
    let (pool, dir) = setup().await;
    let root = dir.path().join("archive");
    let quote_id = mk_quote(&pool, &root).await;

    let a = archive_pdf(&pool, &root, "agb.pdf").await;
    let p = archive_pdf(&pool, &root, "privacy.pdf").await;
    let agb = legal_documents::create_version(&pool, "agb", &a, "AGB v1")
        .await
        .unwrap();
    let priv1 = legal_documents::create_version(&pool, "privacy", &p, "DS v1")
        .await
        .unwrap();
    legal_documents::activate(&pool, &agb.id).await.unwrap();
    legal_documents::activate(&pool, &priv1.id).await.unwrap();

    let bound1 = legal_documents::bind_active_for_quote(&pool, &quote_id)
        .await
        .unwrap();
    assert_eq!(bound1.len(), 2, "AGB + Datenschutz gebunden");

    // Zweiter Aufruf ändert nichts (idempotent), auch wenn inzwischen eine neue
    // Version aktiv wäre — die ursprüngliche Bindung bleibt (Nachweis-Snapshot).
    let a2 = archive_pdf(&pool, &root, "agb-2.pdf").await;
    let agb2 = legal_documents::create_version(&pool, "agb", &a2, "AGB v2")
        .await
        .unwrap();
    legal_documents::activate(&pool, &agb2.id).await.unwrap();

    let bound2 = legal_documents::bind_active_for_quote(&pool, &quote_id)
        .await
        .unwrap();
    assert_eq!(bound2.len(), 2, "weiterhin genau eine Bindung pro doc_type");

    let agb_binding = bound2.iter().find(|b| b.doc_type == "agb").unwrap();
    assert_eq!(
        agb_binding.version, 1,
        "ursprünglich gebundene AGB-Version bleibt v1"
    );
    assert_eq!(agb_binding.legal_document_id, agb.id);
}
