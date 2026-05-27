//! Integration-Tests für `db::repo::seller_profile` inkl. §19-Toggle-Logik.

use klein_buch_lib::db::repo::audit_log;
use klein_buch_lib::db::repo::seller_profile::{self, SellerProfileInput};
use klein_buch_lib::db::MIGRATOR;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}",
        dir.path().join("test.sqlite").to_string_lossy()
    );
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

fn base_input() -> SellerProfileInput {
    SellerProfileInput {
        name: "Wildbach Computerhilfe".into(),
        legal_form: Some("e.K.".into()),
        street: "Hauptstr. 1".into(),
        postal_code: "84028".into(),
        city: "Landshut".into(),
        country_code: "DE".into(),
        tax_number: Some("132/456/7890".into()),
        vat_id: None,
        email: "info@wildbach-computerhilfe.de".into(),
        phone: None,
        iban: None,
        bic: None,
        logo_filename: None,
        is_kleinunternehmer: true,
        default_pdf_template: Some("default".into()),
        default_currency: Some("EUR".into()),
        confirm_waive_paragraph_19: None,
    }
}

#[tokio::test]
async fn initial_get_returns_none() {
    let (pool, _d) = setup_pool().await;
    assert!(seller_profile::get(&pool).await.unwrap().is_none());
}

#[tokio::test]
async fn upsert_creates_singleton() {
    let (pool, _d) = setup_pool().await;
    let row = seller_profile::upsert(&pool, &base_input()).await.unwrap();
    assert_eq!(row.id, 1);
    assert_eq!(row.name, "Wildbach Computerhilfe");
    assert_eq!(row.is_kleinunternehmer, 1);
    assert!(row.waived_paragraph_19_since.is_none());

    // Zweites upsert ersetzt den Row, bleibt id=1.
    let mut second = base_input();
    second.name = "Neuer Name".into();
    let row2 = seller_profile::upsert(&pool, &second).await.unwrap();
    assert_eq!(row2.id, 1);
    assert_eq!(row2.name, "Neuer Name");
}

#[tokio::test]
async fn waive_requires_explicit_confirmation() {
    let (pool, _d) = setup_pool().await;
    seller_profile::upsert(&pool, &base_input()).await.unwrap();

    let mut waive = base_input();
    waive.is_kleinunternehmer = false;
    waive.confirm_waive_paragraph_19 = None;
    let err = seller_profile::upsert(&pool, &waive).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("5-Jahres-Bindung"), "msg: {msg}");
}

#[tokio::test]
async fn waive_sets_since_and_audit_log() {
    let (pool, _d) = setup_pool().await;
    seller_profile::upsert(&pool, &base_input()).await.unwrap();

    let mut waive = base_input();
    waive.is_kleinunternehmer = false;
    waive.confirm_waive_paragraph_19 = Some(true);
    let row = seller_profile::upsert(&pool, &waive).await.unwrap();
    assert_eq!(row.is_kleinunternehmer, 0);
    assert!(row.waived_paragraph_19_since.is_some());

    let entries = audit_log::recent(&pool, 10).await.unwrap();
    let any_waiver = entries
        .iter()
        .any(|e| e.action == "seller_profile_waive_paragraph_19");
    assert!(
        any_waiver,
        "Audit-Log enthält keinen Verzichts-Eintrag: {entries:?}"
    );
}

#[tokio::test]
async fn return_blocked_within_five_year_binding() {
    let (pool, _d) = setup_pool().await;
    // Profil mit Verzicht "heute" simulieren.
    seller_profile::upsert(&pool, &base_input()).await.unwrap();

    let mut waive = base_input();
    waive.is_kleinunternehmer = false;
    waive.confirm_waive_paragraph_19 = Some(true);
    seller_profile::upsert(&pool, &waive).await.unwrap();

    // Sofortige Rückkehr → muss scheitern.
    let mut back = base_input();
    back.is_kleinunternehmer = true;
    let err = seller_profile::upsert(&pool, &back).await.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("5-Jahres-Bindung") || msg.contains("Rückkehr"),
        "msg: {msg}"
    );
}

#[tokio::test]
async fn no_status_change_keeps_waived_since() {
    let (pool, _d) = setup_pool().await;
    // Erst Status auf "verzichtet" setzen.
    seller_profile::upsert(&pool, &base_input()).await.unwrap();
    let mut waive = base_input();
    waive.is_kleinunternehmer = false;
    waive.confirm_waive_paragraph_19 = Some(true);
    let waived_row = seller_profile::upsert(&pool, &waive).await.unwrap();
    let original_since = waived_row.waived_paragraph_19_since.clone();
    assert!(original_since.is_some());

    // Re-upsert ohne Status-Change → waived_since bleibt erhalten.
    let mut same_again = base_input();
    same_again.is_kleinunternehmer = false; // identischer Status
    let row2 = seller_profile::upsert(&pool, &same_again).await.unwrap();
    assert_eq!(row2.waived_paragraph_19_since, original_since);
}

#[tokio::test]
async fn validation_rejects_missing_pflichtfelder() {
    let (pool, _d) = setup_pool().await;
    let mut bad = base_input();
    bad.name = "  ".into();
    bad.email = "".into();
    let err = seller_profile::upsert(&pool, &bad).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("Pflichtfeld"), "msg: {msg}");
}

#[tokio::test]
async fn tax_number_optional_at_save() {
    // §14-UStG-Check passiert erst in Block 3 beim Issue. Profil-Save geht
    // ohne tax_number durch — typischer Onboarding-Zustand vor FA-Vergabe.
    let (pool, _d) = setup_pool().await;
    let mut input = base_input();
    input.tax_number = None;
    let row = seller_profile::upsert(&pool, &input).await.unwrap();
    assert!(row.tax_number.is_none());
}
