//! Integration-Tests für das E-Mail-Versandprotokoll `email_log` (Block 16b).
//!
//! Deckt ab: Schreiben (insert), Lesen (list neueste-zuerst, list_for je Beleg)
//! und die Append-only-Hard-Line (DB-Trigger lehnen UPDATE/DELETE ab).

use klein_buch_lib::db::repo::email_log::{self, EmailLogEntry};
use klein_buch_lib::db::MIGRATOR;
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

fn success_entry(kind: &str, id: Option<&str>, number: Option<&str>, to: &str) -> EmailLogEntry {
    EmailLogEntry {
        account_id: Some("acc-1".into()),
        account_label: Some("Wildbach M365".into()),
        channel: "graph".into(),
        related_kind: kind.into(),
        related_id: id.map(str::to_string),
        related_number: number.map(str::to_string),
        from_email: "rechnung@wildbach-computerhilfe.de".into(),
        to_email: to.into(),
        subject: "Rechnung".into(),
        attachment_count: 1,
        status: "success".into(),
        provider_code: Some("202".into()),
        provider_message: Some("Accepted (Microsoft Graph)".into()),
        request_id: Some("11111111-aaaa-bbbb-cccc-222222222222".into()),
        error: None,
    }
}

fn failed_entry(kind: &str, id: Option<&str>, to: &str) -> EmailLogEntry {
    EmailLogEntry {
        account_id: Some("acc-1".into()),
        account_label: Some("Wildbach M365".into()),
        channel: "smtp".into(),
        related_kind: kind.into(),
        related_id: id.map(str::to_string),
        related_number: None,
        from_email: "rechnung@wildbach-computerhilfe.de".into(),
        to_email: to.into(),
        subject: "Test-Mail".into(),
        attachment_count: 0,
        status: "failed".into(),
        provider_code: None,
        provider_message: None,
        request_id: None,
        error: Some("SMTP-Versand fehlgeschlagen: connection refused".into()),
    }
}

#[tokio::test]
async fn insert_and_list_newest_first() {
    let (pool, _dir) = setup_pool().await;

    email_log::insert(
        &pool,
        &success_entry(
            "invoice",
            Some("inv-1"),
            Some("RE-2026-0001"),
            "a@example.com",
        ),
    )
    .await
    .unwrap();
    email_log::insert(&pool, &failed_entry("test", None, "b@example.com"))
        .await
        .unwrap();

    let all = email_log::list(&pool, 100).await.unwrap();
    assert_eq!(all.len(), 2);
    // created_at-DESC, id-DESC: der zuletzt eingefügte (UUIDv7, größer) zuerst.
    assert_eq!(all[0].related_kind, "test");
    assert_eq!(all[0].status, "failed");
    assert_eq!(
        all[0].error.as_deref(),
        Some("SMTP-Versand fehlgeschlagen: connection refused")
    );
    assert_eq!(all[1].related_kind, "invoice");
    assert_eq!(all[1].provider_code.as_deref(), Some("202"));
    assert_eq!(
        all[1].request_id.as_deref(),
        Some("11111111-aaaa-bbbb-cccc-222222222222")
    );

    // limit greift.
    let one = email_log::list(&pool, 1).await.unwrap();
    assert_eq!(one.len(), 1);
}

#[tokio::test]
async fn list_for_filters_by_document() {
    let (pool, _dir) = setup_pool().await;

    email_log::insert(
        &pool,
        &success_entry(
            "invoice",
            Some("inv-1"),
            Some("RE-2026-0001"),
            "a@example.com",
        ),
    )
    .await
    .unwrap();
    email_log::insert(
        &pool,
        &success_entry(
            "invoice",
            Some("inv-1"),
            Some("RE-2026-0001"),
            "a2@example.com",
        ),
    )
    .await
    .unwrap();
    email_log::insert(
        &pool,
        &success_entry(
            "invoice",
            Some("inv-2"),
            Some("RE-2026-0002"),
            "c@example.com",
        ),
    )
    .await
    .unwrap();
    email_log::insert(
        &pool,
        &success_entry("quote", Some("q-1"), Some("AN-2026-0001"), "d@example.com"),
    )
    .await
    .unwrap();

    let inv1 = email_log::list_for(&pool, "invoice", "inv-1")
        .await
        .unwrap();
    assert_eq!(inv1.len(), 2);
    assert!(inv1
        .iter()
        .all(|e| e.related_id.as_deref() == Some("inv-1")));

    let q1 = email_log::list_for(&pool, "quote", "q-1").await.unwrap();
    assert_eq!(q1.len(), 1);
    assert_eq!(q1[0].related_number.as_deref(), Some("AN-2026-0001"));

    let none = email_log::list_for(&pool, "invoice", "does-not-exist")
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn append_only_rejects_update_and_delete() {
    let (pool, _dir) = setup_pool().await;
    let id = email_log::insert(
        &pool,
        &success_entry(
            "invoice",
            Some("inv-9"),
            Some("RE-2026-0009"),
            "x@example.com",
        ),
    )
    .await
    .unwrap();

    // UPDATE muss vom Trigger abgelehnt werden.
    let upd = sqlx::query("UPDATE email_log SET subject = 'manipuliert' WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(upd.is_err(), "UPDATE auf email_log darf nicht erlaubt sein");

    // DELETE ebenso.
    let del = sqlx::query("DELETE FROM email_log WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(del.is_err(), "DELETE auf email_log darf nicht erlaubt sein");

    // Eintrag ist unverändert vorhanden.
    let all = email_log::list(&pool, 10).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].subject, "Rechnung");
}
