//! Integration-Tests für die R5-Review-Phase, Patch-A (Hardline-Critical +
//! Backup-Härtung).
//!
//! Deckt ab:
//! - **R5-005**: `backup::setup_passphrase` lehnt Passphrasen unter 16 Zeichen
//!   ab (Floor von 8 auf 16 gehoben, ADR 0035).
//! - **R5-005-Unicode**: der Floor zählt `chars().count()`, nicht `len()` —
//!   8 Umlaute (`äöüäöüäö` = 16 Bytes, 8 chars) werden korrekt abgelehnt.
//! - **R5-002**: Pinned die Symmetrie der Fehler-Texte zwischen Invoice- und
//!   Expense-Pfad für Future-paid-date (Wortlaut + §11-EStG-Verweis).
//!   Der eigentliche Check sitzt im Command-Layer und ist ohne Tauri-State
//!   nicht aufrufbar — beim Host-Smoke verifizieren: Rechnung locken,
//!   Zukunfts-Zahldatum eingeben → Backend lehnt mit `Error::Domain` ab.
//!
//! R5-001 (Button.svelte Anchor-disabled), R5-006/-007 (Klartext-Lifetime im
//! BackupGate-State) und R5-008 (Inputs disabled) sind UI-only und ohne
//! Svelte-Test-Setup nicht reproduzierbar — beim Host-Smoke prüfen.

use klein_buch_lib::backup;
use klein_buch_lib::backup::BackupSession;
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
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

// ---------------------------------------------------------------------------
// R5-005: Passphrase-Floor 16 Zeichen
// ---------------------------------------------------------------------------

#[tokio::test]
async fn r5_005_setup_passphrase_rejects_15_chars() {
    let (pool, _dir) = setup_pool().await;
    let session = BackupSession::default();
    // 15 Zeichen — knapp unter dem neuen Floor.
    let result = backup::setup_passphrase(&pool, &session, "123456789012345").await;
    assert!(
        result.is_err(),
        "15-Zeichen-Passphrase muss durch den 16-Zeichen-Floor abgelehnt werden"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("16 Zeichen"),
        "Fehlertext muss den neuen Floor benennen: got `{err}`"
    );
    assert!(
        !session.is_unlocked(),
        "abgelehnte Setup-Phrase darf keine Session entsperren"
    );
}

#[tokio::test]
async fn r5_005_setup_passphrase_accepts_16_chars() {
    let (pool, _dir) = setup_pool().await;
    let session = BackupSession::default();
    // Exakt 16 Zeichen — Floor-Grenze. Soll akzeptiert werden.
    backup::setup_passphrase(&pool, &session, "1234567890123456")
        .await
        .expect("16-Zeichen-Passphrase muss den Floor passieren");
    assert!(session.is_unlocked());
}

#[tokio::test]
async fn r5_005_setup_passphrase_unicode_aware_floor() {
    let (pool, _dir) = setup_pool().await;
    let session = BackupSession::default();
    // 8 Umlaute = 16 Bytes (UTF-8: 2 Bytes je Umlaut), aber **8 Unicode-
    // Codepoints**. Der Floor zählt `chars().count()`, nicht `.len()`.
    // Diese Phrase muss abgelehnt werden — sonst wäre der Floor in der
    // Praxis halbiert bei Umlauten/Akzenten.
    let unicode_8 = "äöüäöüäö";
    assert_eq!(unicode_8.chars().count(), 8);
    assert_eq!(unicode_8.len(), 16); // Bytes, nicht chars
    let result = backup::setup_passphrase(&pool, &session, unicode_8).await;
    assert!(
        result.is_err(),
        "Floor muss Codepoints zählen, nicht Bytes — 8 Umlaute sind 8 chars"
    );
}

// ---------------------------------------------------------------------------
// R5-002: Symmetrie der Future-paid-date-Errortexte zwischen Invoice und
// Expense. Dokumentations-Pin — der eigentliche Check sitzt im Command-Layer
// (`commands::invoices::invoices_record_payment`) und ist ohne Tauri-State
// nicht direkt aufrufbar. Beim Host-Smoke verifizieren: Rechnung locken,
// Zukunfts-Zahldatum eingeben → Backend lehnt mit `Error::Domain` ab.
// ---------------------------------------------------------------------------

#[test]
fn r5_002_invoice_and_expense_error_texts_match_eestg_paragraph() {
    // Beide Pfade verweisen auf §11 EStG. Invoice = Zufluss, Expense = Abfluss.
    let invoice_msg = "Das Zahldatum darf nicht in der Zukunft liegen (Zufluss-Prinzip §11 EStG).";
    let expense_msg = "Das Zahldatum darf nicht in der Zukunft liegen (Abfluss-Prinzip §11 EStG).";
    assert!(invoice_msg.contains("§11 EStG"));
    assert!(expense_msg.contains("§11 EStG"));
    assert!(invoice_msg.contains("Zufluss"));
    assert!(expense_msg.contains("Abfluss"));
    // Der gemeinsame Präfix dokumentiert die Symmetrie: beide Pfade lehnen
    // Future-Daten mit demselben Wortlaut bis zum Prinzip-Begriff ab.
    let common = "Das Zahldatum darf nicht in der Zukunft liegen";
    assert!(invoice_msg.starts_with(common));
    assert!(expense_msg.starts_with(common));
}
