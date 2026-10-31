//! Integration-Tests für `db::repo::seller_profile`. Den §19-Status führt seit
//! ADR 0042 die Statushistorie; das Profil zeigt ihn nur an.

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::db::repo::audit_log;
use klein_buch_lib::db::repo::seller_profile::{self, SellerProfileInput};
use klein_buch_lib::db::repo::tax_status;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::tax_status::{verzicht_binding_until, ChangeReason, TaxStatus};
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
        name: "Beispiel IT-Service".into(),
        legal_form: Some("e.K.".into()),
        street: "Hauptstr. 1".into(),
        postal_code: "84028".into(),
        city: "Landshut".into(),
        country_code: "DE".into(),
        tax_number: Some("132/456/7890".into()),
        vat_id: None,
        email: "info@example.org".into(),
        phone: Some("+49 000 000000".into()),
        iban: None,
        bic: None,
        logo_filename: None,
        is_kleinunternehmer: None,
        default_pdf_template: Some("default".into()),
        default_currency: Some("EUR".into()),
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
    assert_eq!(row.name, "Beispiel IT-Service");
    // ADR 0042: ohne Eintrag in der Statushistorie gilt §19.
    let view = seller_profile::get_view(&pool).await.unwrap().unwrap();
    assert_eq!(view.is_kleinunternehmer, 1);
    assert!(view.regel_since.is_none());

    // Zweites upsert ersetzt den Row, bleibt id=1.
    let mut second = base_input();
    second.name = "Neuer Name".into();
    let row2 = seller_profile::upsert(&pool, &second).await.unwrap();
    assert_eq!(row2.id, 1);
    assert_eq!(row2.name, "Neuer Name");
}

/// ADR 0042 Nr. 1: Das Profil ändert den Steuerstatus nicht mehr. Ein
/// abweichender Wert wird abgelehnt (nicht still übergangen); weder Historie
/// noch Audit-Log bekommen einen Eintrag.
#[tokio::test]
async fn upsert_rejects_status_change() {
    let (pool, _d) = setup_pool().await;
    seller_profile::upsert(&pool, &base_input()).await.unwrap();

    let mut waive = base_input();
    waive.is_kleinunternehmer = Some(false);
    let err = seller_profile::upsert(&pool, &waive).await.unwrap_err();
    assert!(err.to_string().contains("Statuswechsel eintragen"), "{err}");
    assert!(tax_status::list(&pool).await.unwrap().is_empty());
    let entries = audit_log::recent(&pool, 20).await.unwrap();
    assert!(
        !entries
            .iter()
            .any(|e| e.action.starts_with("tax_status") || e.action.contains("paragraph_19")),
        "{entries:?}"
    );

    // Der passende Wert (oder keiner) geht durch.
    let mut same = base_input();
    same.is_kleinunternehmer = Some(true);
    seller_profile::upsert(&pool, &same).await.unwrap();
}

/// Nach einem Eintrag in der Historie liefert das Profil den Status von heute;
/// ein erneutes Speichern ohne Statusangabe ändert ihn nicht.
#[tokio::test]
async fn profile_view_follows_history_and_upsert_keeps_it() {
    let (pool, _d) = setup_pool().await;
    seller_profile::upsert(&pool, &base_input()).await.unwrap();
    let today = klein_buch_lib::clock::today_berlin();
    let jan = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
    tax_status::insert(
        &pool,
        &tax_status::NewEntry {
            effective_from: jan,
            status: TaxStatus::Regelbesteuerung,
            reason: ChangeReason::Verzicht,
            binding_until: Some(verzicht_binding_until(jan)),
            trigger_document: None,
            note: None,
        },
        today,
    )
    .await
    .unwrap();
    let view = seller_profile::get_view(&pool).await.unwrap().unwrap();
    assert_eq!(view.is_kleinunternehmer, 0);
    assert_eq!(view.regel_since.as_deref(), Some(jan.to_string().as_str()));

    let mut again = base_input();
    again.name = "Umbenannt".into();
    seller_profile::upsert(&pool, &again).await.unwrap();
    let view = seller_profile::get_view(&pool).await.unwrap().unwrap();
    assert_eq!(view.is_kleinunternehmer, 0);
    // Rückkehr über das Profil: abgelehnt.
    let mut back = base_input();
    back.is_kleinunternehmer = Some(true);
    assert!(seller_profile::upsert(&pool, &back).await.is_err());
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
