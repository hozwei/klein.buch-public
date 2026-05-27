//! Integration-Tests für `db::repo::expenses` + `db::repo::payment_accounts`
//! + Attachment-Verknüpfung (Block 9).
//!
//! Nutzt eine tempfile-SQLite-DB mit den echten Migrationen (inkl. 0007/0008) —
//! damit greifen die DB-Triggers (GoBD) und CHECK-Constraints.

use chrono::NaiveDate;
use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::db::numbering;
use klein_buch_lib::db::repo::payment_accounts::PaymentAccountInput;
use klein_buch_lib::db::repo::{attachments, contacts, expenses, payment_accounts};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::numbering::DocType;
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

async fn mk_vendor(pool: &SqlitePool, name: &str) -> String {
    let input = ContactInput {
        contact_type: ContactType::Vendor,
        name: name.into(),
        legal_form: None,
        vat_id: None,
        tax_number: None,
        street: "Lieferweg 3".into(),
        postal_code: "10115".into(),
        city: "Berlin".into(),
        country_code: "DE".into(),
        email: None,
        phone: None,
        iban: None,
        bic: None,
        accepts_einvoice: false,
        notes: None,
    };
    contacts::create(pool, &input).await.unwrap().id
}

fn expense_input(vendor_contact_id: Option<&str>) -> ExpenseInput {
    ExpenseInput {
        expense_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        paid_date: Some(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()),
        paid_from_account_id: None,
        vendor_contact_id: vendor_contact_id.map(|s| s.to_string()),
        vendor_name: "Microsoft Ireland".into(),
        vendor_invoice_number: Some("MS-42".into()),
        category: "software".into(),
        description: "Microsoft 365 Business".into(),
        net_amount_cents: 10_000,
        tax_amount_cents: 1_900,
        gross_amount_cents: 11_900,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    }
}

async fn mk_expense(pool: &SqlitePool, number: &str, vendor: Option<&str>) -> String {
    expenses::create(pool, &expense_input(vendor), number, 2026, None)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn create_then_get_locks_immediately() {
    let (pool, _d) = setup_pool().await;
    let id = mk_expense(&pool, "KO-2026-0001", None).await;

    let row = expenses::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.expense_number, "KO-2026-0001");
    assert_eq!(row.status, "recorded");
    assert!(
        row.locked_at.is_some(),
        "Kosten werden sofort festgeschrieben"
    );
    assert_eq!(row.net_amount_cents, 10_000);
    assert_eq!(row.tax_amount_cents, 1_900);
    assert_eq!(row.gross_amount_cents, 11_900);
    assert_eq!(row.reverse_charge_13b, 0);
}

#[tokio::test]
async fn numbering_is_gap_free_per_year() {
    let (pool, _d) = setup_pool().await;
    let n1 = numbering::next_number(&pool, DocType::Expense, 2026)
        .await
        .unwrap();
    let n2 = numbering::next_number(&pool, DocType::Expense, 2026)
        .await
        .unwrap();
    assert_eq!(n1, "KO-2026-0001");
    assert_eq!(n2, "KO-2026-0002");
    let _ = mk_expense(&pool, &n1, None).await;
    let _ = mk_expense(&pool, &n2, None).await;
    let list = expenses::list(&pool, &expenses::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn locked_expense_core_fields_are_immutable() {
    let (pool, _d) = setup_pool().await;
    let id = mk_expense(&pool, "KO-2026-0001", None).await;

    // Direkter Versuch, ein Kernfeld zu ändern → trg_expenses_immutable ABORT.
    let res = sqlx::query("UPDATE expenses SET net_amount_cents = 999 WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "Kernfeld-Update muss am Trigger scheitern");

    // Status-Wechsel (Cancel) bleibt erlaubt.
    expenses::cancel(&pool, &id, "Doppelt erfasst")
        .await
        .unwrap();
    let row = expenses::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.status, "canceled");
}

#[tokio::test]
async fn cancel_sets_canceled_and_is_guarded() {
    let (pool, _d) = setup_pool().await;
    let id = mk_expense(&pool, "KO-2026-0001", None).await;

    expenses::cancel(&pool, &id, "Storniert").await.unwrap();
    let row = expenses::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(row.status, "canceled");
    assert_eq!(row.canceled_reason.as_deref(), Some("Storniert"));
    assert!(row.canceled_at.is_some());

    // Doppel-Storno verhindert.
    assert!(expenses::cancel(&pool, &id, "nochmal").await.is_err());
}

#[tokio::test]
async fn list_filters_by_status_and_canceled() {
    let (pool, _d) = setup_pool().await;
    let a = mk_expense(&pool, "KO-2026-0001", None).await;
    let b = mk_expense(&pool, "KO-2026-0002", None).await;
    expenses::cancel(&pool, &b, "weg").await.unwrap();

    let all = expenses::list(&pool, &expenses::ListFilter::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    let active = expenses::list(
        &pool,
        &expenses::ListFilter {
            include_canceled: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, a);

    let by_cat = expenses::list(
        &pool,
        &expenses::ListFilter {
            category: Some("software".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(by_cat.len(), 2);
}

#[tokio::test]
async fn get_detail_includes_vendor_contact() {
    let (pool, _d) = setup_pool().await;
    let cid = mk_vendor(&pool, "Lieferant GmbH").await;
    let id = mk_expense(&pool, "KO-2026-0001", Some(&cid)).await;

    let detail = expenses::get_detail(&pool, &id).await.unwrap().unwrap();
    assert!(detail.vendor.is_some());
    assert_eq!(detail.vendor.unwrap().name, "Lieferant GmbH");
    assert_eq!(detail.attachments.len(), 0);
}

#[tokio::test]
async fn receipt_and_attachment_show_in_detail() {
    let (pool, dir) = setup_pool().await;
    let archive_root = dir.path().join("archive");

    // Primärer Beleg (ExpenseOriginal) → receipt_archive_id beim Anlegen.
    let receipt = store_bytes(
        &pool,
        &archive_root,
        2026,
        ArchiveKind::ExpenseOriginal,
        "KO-2026-0001-beleg.pdf",
        "application/pdf",
        b"%PDF-1.7 receipt",
    )
    .await
    .unwrap();
    let id = expenses::create(
        &pool,
        &expense_input(None),
        "KO-2026-0001",
        2026,
        Some(&receipt.archive_id),
    )
    .await
    .unwrap()
    .id;

    let row = expenses::get(&pool, &id).await.unwrap().unwrap();
    assert_eq!(
        row.receipt_archive_id.as_deref(),
        Some(receipt.archive_id.as_str())
    );

    // Zusätzlicher Anhang über die generische attachments-Tabelle.
    let extra = store_bytes(
        &pool,
        &archive_root,
        2026,
        ArchiveKind::Attachment,
        "KO-2026-0001-lieferschein.pdf",
        "application/pdf",
        b"%PDF-1.7 delivery note",
    )
    .await
    .unwrap();
    let sort = attachments::count_for_parent(&pool, "expense", &id)
        .await
        .unwrap();
    attachments::create(
        &pool,
        "expense",
        &id,
        &extra.archive_id,
        Some("Lieferschein"),
        sort,
    )
    .await
    .unwrap();

    let detail = expenses::get_detail(&pool, &id).await.unwrap().unwrap();
    assert_eq!(detail.attachments.len(), 1);
    assert_eq!(
        detail.attachments[0].file_name,
        "KO-2026-0001-lieferschein.pdf"
    );
    assert_eq!(detail.attachments[0].label.as_deref(), Some("Lieferschein"));
}

#[tokio::test]
async fn set_payment_marks_and_unmarks_and_guards_canceled() {
    let (pool, _d) = setup_pool().await;
    // Kosten ohne Zahldatum erfassen ("noch nicht bezahlt").
    let mut inp = expense_input(None);
    inp.paid_date = None;
    let id = expenses::create(&pool, &inp, "KO-2026-0001", 2026, None)
        .await
        .unwrap()
        .id;
    assert!(expenses::get(&pool, &id)
        .await
        .unwrap()
        .unwrap()
        .paid_date
        .is_none());

    // Als bezahlt markieren.
    let row = expenses::set_payment(&pool, &id, Some("2026-06-01"), None)
        .await
        .unwrap();
    assert_eq!(row.paid_date.as_deref(), Some("2026-06-01"));

    // Wieder auf „offen" setzen (Fehl-Markierung korrigieren).
    let row = expenses::set_payment(&pool, &id, None, None).await.unwrap();
    assert!(row.paid_date.is_none());

    // Nach Storno keine Zahlungs-Markierung mehr möglich.
    expenses::cancel(&pool, &id, "weg").await.unwrap();
    assert!(expenses::set_payment(&pool, &id, Some("2026-06-02"), None)
        .await
        .is_err());
}

// ---- payment_accounts ------------------------------------------------------

#[tokio::test]
async fn ensure_defaults_seeds_hauptkonto_and_bargeld_once() {
    let (pool, _d) = setup_pool().await;
    assert!(payment_accounts::ensure_defaults(&pool).await.unwrap());
    // Zweiter Aufruf seedet NICHT erneut.
    assert!(!payment_accounts::ensure_defaults(&pool).await.unwrap());

    let list = payment_accounts::list(&pool, false).await.unwrap();
    assert_eq!(list.len(), 2);
    // Default kommt zuerst (ORDER BY is_default DESC) und ist das Hauptkonto.
    assert_eq!(list[0].label, "Hauptkonto");
    assert_eq!(list[0].is_default, 1);
    assert_eq!(list[0].account_type, "bank");
    assert!(list
        .iter()
        .any(|a| a.label == "Bargeld" && a.account_type == "cash"));
}

#[tokio::test]
async fn only_one_default_account() {
    let (pool, _d) = setup_pool().await;
    let a = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Konto A".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: true,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();
    let b = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Konto B".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: true,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();

    let list = payment_accounts::list(&pool, false).await.unwrap();
    let defaults: Vec<_> = list.iter().filter(|x| x.is_default == 1).collect();
    assert_eq!(defaults.len(), 1, "höchstens ein Standard-Konto");
    assert_eq!(defaults[0].id, b.id);
    assert_ne!(defaults[0].id, a.id);
}

#[tokio::test]
async fn set_active_hides_from_default_list() {
    let (pool, _d) = setup_pool().await;
    let a = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Altkonto".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: false,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();
    payment_accounts::set_active(&pool, &a.id, false)
        .await
        .unwrap();

    let active = payment_accounts::list(&pool, false).await.unwrap();
    assert!(active.iter().all(|x| x.id != a.id));
    let all = payment_accounts::list(&pool, true).await.unwrap();
    assert!(all.iter().any(|x| x.id == a.id && x.active == 0));
}

#[tokio::test]
async fn deactivating_default_promotes_another_active_account() {
    let (pool, _d) = setup_pool().await;
    let a = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Hauptkonto".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: true,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();
    let b = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Bargeld".into(),
            account_type: "cash".into(),
            iban: None,
            bic: None,
            is_default: false,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();

    // Standard-Konto deaktivieren → es muss weiterhin genau ein (anderes,
    // aktives) Standard-Konto geben.
    payment_accounts::set_active(&pool, &a.id, false)
        .await
        .unwrap();

    let active = payment_accounts::list(&pool, false).await.unwrap();
    let defaults: Vec<_> = active.iter().filter(|x| x.is_default == 1).collect();
    assert_eq!(
        defaults.len(),
        1,
        "es muss immer genau ein Standard-Konto geben"
    );
    assert_eq!(defaults[0].id, b.id, "Bargeld wird zum Standard befördert");
    // Das deaktivierte (ehemalige) Standard-Konto ist nicht mehr Standard.
    let all = payment_accounts::list(&pool, true).await.unwrap();
    let old = all.iter().find(|x| x.id == a.id).unwrap();
    assert_eq!(old.active, 0);
    assert_eq!(old.is_default, 0);
}

#[tokio::test]
async fn unsetting_default_via_update_keeps_one_default() {
    let (pool, _d) = setup_pool().await;
    let a = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Hauptkonto".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: true,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();

    // Default-Haken im Edit entfernen darf nicht „null Standard" hinterlassen.
    payment_accounts::update(
        &pool,
        &a.id,
        &PaymentAccountInput {
            label: "Hauptkonto".into(),
            account_type: "bank".into(),
            iban: None,
            bic: None,
            is_default: false,
            show_on_invoice: false,
            details: None,
        },
    )
    .await
    .unwrap();

    let active = payment_accounts::list(&pool, false).await.unwrap();
    let defaults = active.iter().filter(|x| x.is_default == 1).count();
    assert_eq!(
        defaults, 1,
        "Invariante: solange aktive Konten existieren, genau ein Standard"
    );
}

#[tokio::test]
async fn invalid_account_type_rejected() {
    let (pool, _d) = setup_pool().await;
    let res = payment_accounts::create(
        &pool,
        &PaymentAccountInput {
            label: "Krypto".into(),
            account_type: "bitcoin".into(),
            iban: None,
            bic: None,
            is_default: false,
            show_on_invoice: false,
            details: None,
        },
    )
    .await;
    assert!(res.is_err());
}
