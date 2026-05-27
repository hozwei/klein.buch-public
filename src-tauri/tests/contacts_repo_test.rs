//! Integration-Tests für `db::repo::contacts`.
//!
//! Nutzt eine tempfile-SQLite-DB mit der echten 0001_init.sql-Migration.
//! Damit kommen auch DB-Triggers (GoBD) zum Tragen.

use chrono::Utc;
use klein_buch_lib::db::repo::contacts;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
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

fn good_input(name: &str) -> ContactInput {
    ContactInput {
        contact_type: ContactType::Customer,
        name: name.into(),
        legal_form: Some("GmbH".into()),
        vat_id: Some("DE123456789".into()),
        tax_number: None,
        street: "Musterstr. 1".into(),
        postal_code: "84028".into(),
        city: "Landshut".into(),
        country_code: "DE".into(),
        email: Some("info@beispiel.de".into()),
        phone: None,
        iban: None,
        bic: None,
        accepts_einvoice: true,
        notes: None,
    }
}

#[tokio::test]
async fn create_then_get() {
    let (pool, _d) = setup_pool().await;
    let created = contacts::create(&pool, &good_input("Beispiel GmbH"))
        .await
        .unwrap();
    assert_eq!(created.name, "Beispiel GmbH");
    assert_eq!(created.contact_type, "customer");
    assert_eq!(created.archived, 0);
    assert_eq!(created.accepts_einvoice, 1);

    let fetched = contacts::get(&pool, &created.id).await.unwrap().unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.vat_id.as_deref(), Some("DE123456789"));
}

#[tokio::test]
async fn list_excludes_archived_by_default() {
    let (pool, _d) = setup_pool().await;
    let a = contacts::create(&pool, &good_input("Alpha")).await.unwrap();
    let b = contacts::create(&pool, &good_input("Beta")).await.unwrap();

    contacts::archive(&pool, &b.id).await.unwrap();

    let visible = contacts::list(&pool, false).await.unwrap();
    let with_arch = contacts::list(&pool, true).await.unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, a.id);
    assert_eq!(with_arch.len(), 2);
}

#[tokio::test]
async fn update_changes_fields_and_updated_at() {
    let (pool, _d) = setup_pool().await;
    let c = contacts::create(&pool, &good_input("Original"))
        .await
        .unwrap();
    let before_updated = c.updated_at.clone();
    // SQLite datetime('now','utc') hat Sekunden-Auflösung. Kurz warten.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let mut input = good_input("Geändert");
    input.email = Some("neu@beispiel.de".into());
    let updated = contacts::update(&pool, &c.id, &input).await.unwrap();

    assert_eq!(updated.name, "Geändert");
    assert_eq!(updated.email.as_deref(), Some("neu@beispiel.de"));
    assert_ne!(updated.updated_at, before_updated);
}

#[tokio::test]
async fn search_finds_by_name_email_vatid_city_postal() {
    let (pool, _d) = setup_pool().await;
    let mut input1 = good_input("Wildbach Computerhilfe");
    input1.email = Some("info@wildbach.de".into());
    input1.vat_id = Some("DE111111111".into());
    input1.city = "Landshut".into();
    input1.postal_code = "84028".into();
    contacts::create(&pool, &input1).await.unwrap();

    let mut input2 = good_input("Andere Firma");
    input2.email = Some("kontakt@andere.de".into());
    input2.vat_id = Some("DE222222222".into());
    input2.city = "München".into();
    input2.postal_code = "80331".into();
    contacts::create(&pool, &input2).await.unwrap();

    assert_eq!(
        contacts::search(&pool, "wildbach", false)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        contacts::search(&pool, "ANDERE.DE", false)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        contacts::search(&pool, "111111111", false)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        contacts::search(&pool, "landshut", false)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        contacts::search(&pool, "MÜNCHEN", false)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        contacts::search(&pool, "84028", false).await.unwrap().len(),
        1
    );
    assert_eq!(contacts::search(&pool, "  ", false).await.unwrap().len(), 0);
    assert_eq!(
        contacts::search(&pool, "nichts", false)
            .await
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn archive_and_unarchive_round_trip() {
    let (pool, _d) = setup_pool().await;
    let c = contacts::create(&pool, &good_input("Toggle Me"))
        .await
        .unwrap();

    contacts::archive(&pool, &c.id).await.unwrap();
    assert_eq!(
        contacts::get(&pool, &c.id).await.unwrap().unwrap().archived,
        1
    );

    contacts::unarchive(&pool, &c.id).await.unwrap();
    assert_eq!(
        contacts::get(&pool, &c.id).await.unwrap().unwrap().archived,
        0
    );
}

#[tokio::test]
async fn invalid_input_is_rejected() {
    let (pool, _d) = setup_pool().await;
    let mut bad = good_input("");
    bad.street = "".into();
    let err = contacts::create(&pool, &bad).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("Name"), "msg: {msg}");
    assert!(msg.contains("Straße"), "msg: {msg}");
}

#[tokio::test]
async fn update_unknown_id_errors() {
    let (pool, _d) = setup_pool().await;
    let res = contacts::update(&pool, "no-such-id", &good_input("X")).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn updated_at_is_set_for_today() {
    let (pool, _d) = setup_pool().await;
    let c = contacts::create(&pool, &good_input("Now")).await.unwrap();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    assert!(
        c.created_at.starts_with(&today),
        "created_at = {} ; today = {today}",
        c.created_at
    );
}
