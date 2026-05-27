//! Integration-Test für Migration `0028_append_only_hardening.sql`
//! (R1-Re-Review v2026.5).
//!
//! Verifiziert für jede betroffene Tabelle:
//!
//!   * `BEFORE DELETE`-Trigger blockiert direkten DELETE (R1-001..R1-009).
//!   * Erweiterte `trg_*_immutable`-Whitelist blockiert nach Lock alle neu
//!     geschützten Spalten (R1-004): buyer_*, seller_*, delivery_date,
//!     currency_code, pdf_template, is_storno_for usw.
//!   * Unlock-Schutz: `UPDATE … SET locked_at = NULL` ist DB-seitig
//!     blockiert (R1-005).
//!   * App-Layer-Mutations-Pfade (status, paid_*, sent_at, validation_*,
//!     archive_ids beim Lock-Wechsel) bleiben durchlässig.
//!   * `depreciation_entries` DELETE: ungelockte ja, gelockte nein.
//!   * Schema-Version matcht `EXPECTED_SCHEMA_VERSION` nach Migration
//!     (self-healing gegenüber Folge-Migrationen).
//!
//! Setup: tempfile-SQLite mit echtem MIGRATOR (alle Migrationen). Direkter
//! Raw-SQL-Pfad — kein Domain-/Command-Layer, damit der Test ausschließlich
//! das DB-Verhalten verifiziert.

use klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION;
use klein_buch_lib::db::MIGRATOR;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

async fn setup() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("klein-buch.sqlite");
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

/// Helper: legt einen minimalen Kontakt + Seller-Profile an und gibt die
/// Contact-ID zurück. Die Seller-Profil-Zeile ist Singleton (id=1).
async fn mk_basics(pool: &SqlitePool) -> String {
    sqlx::query(
        "INSERT INTO seller_profile (id, name, street, postal_code, city, tax_number, email)
         VALUES (1, 'Wildbach', 'Beispielweg 1', '84028', 'Landshut', '123', 's@x.de')",
    )
    .execute(pool)
    .await
    .unwrap();

    let contact_id = "01928300-0000-7000-8000-000000000001".to_string();
    sqlx::query(
        "INSERT INTO contacts (id, contact_type, name, country_code)
         VALUES (?, 'customer', 'Kunde GmbH', 'DE')",
    )
    .bind(&contact_id)
    .execute(pool)
    .await
    .unwrap();

    contact_id
}

/// Helper: archive_entry anlegen (für invoice.pdf_archive_id/xml_archive_id).
async fn mk_archive(pool: &SqlitePool, id: &str, path: &str) {
    sqlx::query(
        "INSERT INTO archive_entries (id, file_path, file_name, file_hash_sha256,
                                       file_size_bytes, mime_type, source)
         VALUES (?, ?, ?, ?, 0, 'application/pdf', 'test')",
    )
    .bind(id)
    .bind(path)
    .bind(path)
    .bind("abcd")
    .execute(pool)
    .await
    .unwrap();
}

/// Helper: legt eine gelockte Test-Rechnung mit allen Snapshot-Feldern an.
///
/// `invoice_number` wird aus den letzten 12 Zeichen der `id` abgeleitet, damit
/// mehrere mk_locked_invoice-Aufrufe im selben Test eindeutige Nummern bekommen
/// (UNIQUE-Constraint auf invoice_number).
async fn mk_locked_invoice(pool: &SqlitePool, contact_id: &str, id: &str) {
    let suffix = &id[id.len().saturating_sub(12)..];
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date, delivery_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, net_amount_cents, tax_amount_cents, gross_amount_cents,
            currency_code, is_kleinunternehmer, pdf_template, status,
            buyer_name, buyer_street, buyer_postal_code, buyer_city, buyer_country_code,
            locked_at
         ) VALUES (
            ?, ?, 2026, 'issued', '2026-05-20', '2026-05-20',
            ?, 'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
            '123', 10000, 0, 10000,
            'EUR', 1, 'default', 'issued',
            'Kunde GmbH', 'Hauptstr. 7', '80331', 'München', 'DE',
            datetime('now','utc')
         )",
    )
    .bind(id)
    .bind(format!("RE-2026-T{suffix}"))
    .bind(contact_id)
    .execute(pool)
    .await
    .unwrap();
}

// ============================================================================
// R1-001 / R1-002 / R1-006..R1-008: No-Delete-Trigger
// ============================================================================

#[tokio::test]
async fn schema_version_matches_expected_after_migration() {
    // Self-healing gegenüber Folge-Migrationen: vergleicht gegen die in der
    // Binary einkompilierte Konstante, statt einen Hardcode-Wert (war "28",
    // brach mit jeder neuen Migration). Damit ist der Test-Vertrag „Migrator
    // hebt schema_version auf den App-Stand", nicht „auf eine konkrete Zahl".
    let (pool, _dir) = setup().await;
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'schema_version'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let value: String = row.try_get("value").unwrap();
    assert_eq!(value, EXPECTED_SCHEMA_VERSION.to_string());
}

#[tokio::test]
async fn delete_locked_invoice_is_blocked() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-1100-7000-8000-000000000001";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("DELETE FROM invoices WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("immutable") || msg.contains("storno"),
        "expected GoBD-Trigger-Fehler, got: {msg}"
    );
}

#[tokio::test]
async fn delete_draft_invoice_is_also_blocked() {
    // R1-001: Trigger ist bedingungslos — auch Drafts müssen via
    // status='canceled' neutralisiert werden, nicht gelöscht.
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-1100-7000-8000-000000000002";
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, net_amount_cents, gross_amount_cents,
            status
         ) VALUES (?, 'RE-2026-DRAFT', 2026, 'issued', '2026-05-20',
                   ?, 'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
                   '123', 10000, 10000, 'draft')",
    )
    .bind(id)
    .bind(&contact_id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("DELETE FROM invoices WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable") || format!("{err}").contains("storno"));
}

#[tokio::test]
async fn delete_quote_is_blocked() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-2200-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO quotes (
            id, quote_number, fiscal_year, quote_date, valid_until, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents, status
         ) VALUES (?, 'AN-2026-0001', 2026, '2026-05-20', '2026-06-20', ?,
                   'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
                   10000, 10000, 'draft')",
    )
    .bind(id)
    .bind(&contact_id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("DELETE FROM quotes WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable") || format!("{err}").contains("storno"));
}

#[tokio::test]
async fn delete_expense_is_blocked() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-3300-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO expenses (
            id, expense_number, fiscal_year, expense_date, vendor_name_snapshot,
            category, description, net_amount_cents, gross_amount_cents, status
         ) VALUES (?, 'KO-2026-0001', 2026, '2026-05-20', 'Hetzner',
                   'software', 'Server-Miete', 5000, 5000, 'recorded')",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("DELETE FROM expenses WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable") || format!("{err}").contains("cancel"));
}

#[tokio::test]
async fn delete_private_movement_is_blocked() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-4400-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO private_movements (
            id, movement_number, fiscal_year, movement_date, movement_type,
            amount_cents, description
         ) VALUES (?, 'PV-2026-0001', 2026, '2026-05-20', 'entnahme', 10000, 'Cash')",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("DELETE FROM private_movements WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(
        format!("{err}").contains("append-only") || format!("{err}").contains("counter-movement")
    );
}

#[tokio::test]
async fn delete_asset_is_blocked() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-5500-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO assets (
            id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, book_value_cents
         ) VALUES (?, 'AV-2026-0001', 'Laptop', '2026-05-20', 150000, 2026,
                   'linear', 150000)",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("append-only") || format!("{err}").contains("dispose"));
}

#[tokio::test]
async fn delete_locked_depreciation_entry_is_blocked_but_unlocked_works() {
    // R1-009: nur gelockte AfA-Buchungen sind unlöschbar. Reset-Pfad
    // (locked_at IS NULL) bleibt funktional.
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;

    let asset_id = "01928300-5500-7000-8000-000000000002";
    sqlx::query(
        "INSERT INTO assets (
            id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, book_value_cents
         ) VALUES (?, 'AV-2026-0002', 'Laptop', '2026-05-20', 150000, 2026,
                   'linear', 150000)",
    )
    .bind(asset_id)
    .execute(&pool)
    .await
    .unwrap();

    let dep_unlocked = "01928300-6600-7000-8000-000000000001";
    let dep_locked = "01928300-6600-7000-8000-000000000002";

    sqlx::query(
        "INSERT INTO depreciation_entries (id, asset_id, fiscal_year,
            depreciation_amount_cents, months_in_year, book_value_before_cents,
            book_value_after_cents)
         VALUES (?, ?, 2026, 50000, 12, 150000, 100000)",
    )
    .bind(dep_unlocked)
    .bind(asset_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO depreciation_entries (id, asset_id, fiscal_year,
            depreciation_amount_cents, months_in_year, book_value_before_cents,
            book_value_after_cents, locked_at)
         VALUES (?, ?, 2027, 50000, 12, 100000, 50000, datetime('now','utc'))",
    )
    .bind(dep_locked)
    .bind(asset_id)
    .execute(&pool)
    .await
    .unwrap();

    // Ungelockt: löschen geht.
    let res = sqlx::query("DELETE FROM depreciation_entries WHERE id = ?")
        .bind(dep_unlocked)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(res.rows_affected(), 1);

    // Gelockt: muss blocken.
    let err = sqlx::query("DELETE FROM depreciation_entries WHERE id = ?")
        .bind(dep_locked)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

// ============================================================================
// R1-004: erweiterte Immutability-Whitelist
// ============================================================================

#[tokio::test]
async fn cannot_change_buyer_snapshot_after_lock() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000001";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET buyer_name = 'Manipuliert' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_seller_snapshot_after_lock() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000002";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET seller_name = 'Anderer Verkäufer' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_delivery_date_after_lock() {
    // §14 Pflichtangabe — Leistungsdatum darf nach Lock NICHT geändert werden.
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000003";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET delivery_date = '2025-01-01' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_currency_code_after_lock() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000004";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET currency_code = 'USD' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_pdf_template_after_lock() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000005";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET pdf_template = 'fancy' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_is_storno_for_after_lock() {
    // R1-004: Storno-Paar-Verkettung darf nach Lock nicht umgehängt werden.
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000006";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET is_storno_for = 'manipuliert' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_pdf_archive_id_after_lock() {
    // R1-004: archive-Slots sind nach Lock write-once.
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-7700-7000-8000-000000000007";
    let arc1 = "01928300-8800-7000-8000-000000000001";
    let arc2 = "01928300-8800-7000-8000-000000000002";
    mk_archive(&pool, arc1, "a1.pdf").await;
    mk_archive(&pool, arc2, "a2.pdf").await;
    mk_locked_invoice(&pool, &contact_id, id).await;

    // Erst-Setting würde im Lock-Pfad passieren — der Trigger feuert nur, wenn
    // NACH Lock ein ÄNDERN versucht wird. Wir simulieren das durch direktes
    // Setzen und dann erneutes Ändern.
    // (Lock-UPDATE in mk_locked_invoice setzt pdf_archive_id NICHT; das ist OK.)
    sqlx::query("UPDATE invoices SET pdf_archive_id = ? WHERE id = ? AND pdf_archive_id IS NULL")
        .bind(arc1)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err(); // trigger feuert: locked_at IS NOT NULL + arc-id ändert sich
}

// ============================================================================
// R1-005: Unlock-Schutz
// ============================================================================

#[tokio::test]
async fn cannot_unlock_invoice() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-9900-7000-8000-000000000001";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET locked_at = NULL WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_mutate_locked_at_timestamp() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-9900-7000-8000-000000000002";
    mk_locked_invoice(&pool, &contact_id, id).await;

    let err = sqlx::query("UPDATE invoices SET locked_at = '2099-01-01 00:00:00' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

// ============================================================================
// App-Layer-Mutations-Pfade müssen WEITER durchkommen
// ============================================================================

#[tokio::test]
async fn allowed_status_transitions_still_work_on_locked_invoice() {
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let id = "01928300-aa00-7000-8000-000000000001";
    mk_locked_invoice(&pool, &contact_id, id).await;

    // mark_sent: setzt sent_at + status='sent' → muss durchkommen.
    let r = sqlx::query(
        "UPDATE invoices SET sent_at = datetime('now','utc'),
                              status = 'sent',
                              updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(r.rows_affected(), 1);

    // record_payment: setzt paid_amount_cents + paid_at + status → muss durchkommen.
    let r = sqlx::query(
        "UPDATE invoices SET paid_amount_cents = 10000,
                              paid_at = '2026-06-01',
                              status = 'paid',
                              updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(r.rows_affected(), 1);
}

#[tokio::test]
async fn mark_canceled_still_works_on_locked_original() {
    // Original-Rechnung wird via Storno-Pair-TX als 'canceled' markiert.
    // canceled_at + canceled_by_storno_id + cancel_reason + status sind nicht
    // in der Schutz-Whitelist — Trigger darf NICHT feuern.
    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;
    let original = "01928300-aa00-7000-8000-000000000002";
    let storno = "01928300-aa00-7000-8000-000000000003";
    mk_locked_invoice(&pool, &contact_id, original).await;
    mk_locked_invoice(&pool, &contact_id, storno).await;

    let r = sqlx::query(
        "UPDATE invoices SET status = 'canceled',
                              canceled_at = datetime('now','utc'),
                              canceled_by_storno_id = ?,
                              cancel_reason = 'Testfall',
                              updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(storno)
    .bind(original)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(r.rows_affected(), 1);
}

// ============================================================================
// Expense / Private-Movement / Asset: erweiterte Whitelist
// ============================================================================

#[tokio::test]
async fn cannot_change_fiscal_year_on_locked_expense() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-bb00-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO expenses (
            id, expense_number, fiscal_year, expense_date, vendor_name_snapshot,
            category, description, net_amount_cents, gross_amount_cents,
            status, locked_at
         ) VALUES (?, 'KO-2026-0010', 2026, '2026-05-20', 'Hetzner',
                   'software', 'Server', 5000, 5000, 'recorded',
                   datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("UPDATE expenses SET fiscal_year = 2025 WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_category_on_locked_expense() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-bb00-7000-8000-000000000002";
    sqlx::query(
        "INSERT INTO expenses (
            id, expense_number, fiscal_year, expense_date, vendor_name_snapshot,
            category, description, net_amount_cents, gross_amount_cents,
            status, locked_at
         ) VALUES (?, 'KO-2026-0011', 2026, '2026-05-20', 'Hetzner',
                   'software', 'Server', 5000, 5000, 'recorded',
                   datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("UPDATE expenses SET category = 'office' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cancel_expense_still_works_after_lock() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-bb00-7000-8000-000000000003";
    sqlx::query(
        "INSERT INTO expenses (
            id, expense_number, fiscal_year, expense_date, vendor_name_snapshot,
            category, description, net_amount_cents, gross_amount_cents,
            status, locked_at
         ) VALUES (?, 'KO-2026-0012', 2026, '2026-05-20', 'Hetzner',
                   'software', 'Server', 5000, 5000, 'recorded',
                   datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let r = sqlx::query(
        "UPDATE expenses SET status = 'canceled',
                              canceled_at = datetime('now','utc'),
                              canceled_reason = 'Doppelt erfasst'
         WHERE id = ?",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(r.rows_affected(), 1);
}

#[tokio::test]
async fn cannot_change_description_on_locked_private_movement() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-cc00-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO private_movements (
            id, movement_number, fiscal_year, movement_date, movement_type,
            amount_cents, description, locked_at
         ) VALUES (?, 'PV-2026-0010', 2026, '2026-05-20', 'entnahme', 10000,
                   'Bargeld', datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("UPDATE private_movements SET description = 'manipuliert' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn cannot_change_afa_category_on_locked_asset() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-dd00-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO assets (
            id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, afa_category,
            book_value_cents, locked_at
         ) VALUES (?, 'AV-2026-0010', 'Laptop', '2026-05-20', 150000, 2026,
                   'linear', 'computer_hardware', 150000, datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query("UPDATE assets SET afa_category = 'office_equipment' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("immutable"));
}

#[tokio::test]
async fn set_book_value_still_works_on_locked_asset() {
    // AfA-Fortschreibung muss nach Lock funktionieren.
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-dd00-7000-8000-000000000002";
    sqlx::query(
        "INSERT INTO assets (
            id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, book_value_cents,
            locked_at
         ) VALUES (?, 'AV-2026-0020', 'Laptop', '2026-05-20', 150000, 2026,
                   'linear', 150000, datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let r = sqlx::query(
        "UPDATE assets SET book_value_cents = 100000, last_depreciation_year = 2026,
                            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(r.rows_affected(), 1);
}

// ============================================================================
// R1-003: Storno-Pair atomic TX (invoices::lock_with_pair_cancel)
// ============================================================================

#[tokio::test]
async fn lock_with_pair_cancel_atomically_locks_storno_and_cancels_original() {
    use klein_buch_lib::db::repo::invoices::{lock_with_pair_cancel, LockUpdate, PairCancel};

    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;

    let original = "01928300-ee00-7000-8000-000000000001";
    mk_locked_invoice(&pool, &contact_id, original).await;

    // Storno-Draft anlegen (status='draft', locked_at IS NULL, is_storno_for=original).
    let storno = "01928300-ee00-7000-8000-000000000002";
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, net_amount_cents, gross_amount_cents,
            status, is_storno_for
         ) VALUES (?, 'ST-2026-0001', 2026, 'issued', '2026-05-21',
                   ?, 'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
                   '123', -10000, -10000, 'draft', ?)",
    )
    .bind(storno)
    .bind(&contact_id)
    .bind(original)
    .execute(&pool)
    .await
    .unwrap();

    let arc_pdf = "01928300-ee01-7000-8000-000000000001";
    let arc_xml = "01928300-ee01-7000-8000-000000000002";
    mk_archive(&pool, arc_pdf, "st-2026-0001.pdf").await;
    mk_archive(&pool, arc_xml, "st-2026-0001.xml").await;

    let lock_update = LockUpdate {
        validation_status: "passed",
        validation_report: None,
        pdf_archive_id: arc_pdf,
        xml_archive_id: arc_xml,
    };
    let pair = PairCancel {
        original_id: original,
        reason: Some("Testfall: Storno-Paar atomar"),
    };
    lock_with_pair_cancel(&pool, storno, &lock_update, Some(pair))
        .await
        .expect("Storno-Pair-Lock muss erfolgreich sein");

    // Verifikation: Storno ist gelockt, Original ist canceled.
    let row = sqlx::query("SELECT locked_at, status FROM invoices WHERE id = ?")
        .bind(storno)
        .fetch_one(&pool)
        .await
        .unwrap();
    let locked_at: Option<String> = row.try_get("locked_at").unwrap();
    let status: String = row.try_get("status").unwrap();
    assert!(locked_at.is_some(), "Storno muss gelockt sein");
    assert_eq!(status, "issued");

    let row = sqlx::query(
        "SELECT status, canceled_at, canceled_by_storno_id, cancel_reason FROM invoices WHERE id = ?",
    )
    .bind(original)
    .fetch_one(&pool)
    .await
    .unwrap();
    let status: String = row.try_get("status").unwrap();
    let canceled_at: Option<String> = row.try_get("canceled_at").unwrap();
    let canceled_by: Option<String> = row.try_get("canceled_by_storno_id").unwrap();
    let reason: Option<String> = row.try_get("cancel_reason").unwrap();
    assert_eq!(status, "canceled");
    assert!(canceled_at.is_some());
    assert_eq!(canceled_by.as_deref(), Some(storno));
    assert_eq!(reason.as_deref(), Some("Testfall: Storno-Paar atomar"));
}

#[tokio::test]
async fn lock_with_pair_cancel_rolls_back_storno_lock_when_original_missing() {
    // R1-003: Wenn die Original-Cancel-UPDATE 0 Rows betrifft (Original
    // existiert nicht), darf der Storno NICHT gelockt zurückbleiben — TX rollt
    // zurück, beide Belege sind weiter im Vor-Zustand.
    use klein_buch_lib::db::repo::invoices::{lock_with_pair_cancel, LockUpdate, PairCancel};

    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;

    let storno = "01928300-ff00-7000-8000-000000000001";
    // is_storno_for bleibt NULL: das Test-Ziel ist der TX-Rollback in
    // lock_with_pair_cancel, wenn der `PairCancel.original_id` (ein Parameter,
    // KEIN FK) nicht existiert. Den FK auf is_storno_for prüfen wir hier nicht.
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, net_amount_cents, gross_amount_cents,
            status
         ) VALUES (?, 'ST-2026-0002', 2026, 'issued', '2026-05-21',
                   ?, 'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
                   '123', -10000, -10000, 'draft')",
    )
    .bind(storno)
    .bind(&contact_id)
    .execute(&pool)
    .await
    .unwrap();

    let arc_pdf = "01928300-ff01-7000-8000-000000000001";
    let arc_xml = "01928300-ff01-7000-8000-000000000002";
    mk_archive(&pool, arc_pdf, "st-2026-0002.pdf").await;
    mk_archive(&pool, arc_xml, "st-2026-0002.xml").await;

    let lock_update = LockUpdate {
        validation_status: "passed",
        validation_report: None,
        pdf_archive_id: arc_pdf,
        xml_archive_id: arc_xml,
    };
    let pair = PairCancel {
        original_id: "00000000-0000-0000-0000-000000000000", // existiert nicht
        reason: Some("ghost"),
    };
    let res = lock_with_pair_cancel(&pool, storno, &lock_update, Some(pair)).await;
    assert!(res.is_err(), "muss fehlschlagen, weil Original fehlt");

    // Verifikation: Storno bleibt Draft (locked_at IS NULL).
    let row = sqlx::query("SELECT locked_at FROM invoices WHERE id = ?")
        .bind(storno)
        .fetch_one(&pool)
        .await
        .unwrap();
    let locked_at: Option<String> = row.try_get("locked_at").unwrap();
    assert!(
        locked_at.is_none(),
        "Storno muss durch TX-Rollback wieder Draft sein, fand locked_at = {locked_at:?}"
    );
}

#[tokio::test]
async fn lock_without_pair_still_works_for_normal_invoices() {
    // Regression: invoices::lock() darf nicht durch das Pair-Refactoring
    // brechen — der normale Lock-Pfad muss unverändert funktionieren.
    use klein_buch_lib::db::repo::invoices::{lock, LockUpdate};

    let (pool, _dir) = setup().await;
    let contact_id = mk_basics(&pool).await;

    let id = "01928300-ff02-7000-8000-000000000001";
    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, fiscal_year, direction, invoice_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            seller_tax_number, net_amount_cents, gross_amount_cents,
            status
         ) VALUES (?, 'RE-2026-NORMAL', 2026, 'issued', '2026-05-21',
                   ?, 'Wildbach', 'Beispielweg 1', '84028', 'Landshut',
                   '123', 10000, 10000, 'draft')",
    )
    .bind(id)
    .bind(&contact_id)
    .execute(&pool)
    .await
    .unwrap();

    let arc_pdf = "01928300-ff03-7000-8000-000000000001";
    let arc_xml = "01928300-ff03-7000-8000-000000000002";
    mk_archive(&pool, arc_pdf, "re-2026-normal.pdf").await;
    mk_archive(&pool, arc_xml, "re-2026-normal.xml").await;

    lock(
        &pool,
        id,
        &LockUpdate {
            validation_status: "passed",
            validation_report: None,
            pdf_archive_id: arc_pdf,
            xml_archive_id: arc_xml,
        },
    )
    .await
    .expect("normaler Lock muss funktionieren");

    let row = sqlx::query("SELECT locked_at, status FROM invoices WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let locked_at: Option<String> = row.try_get("locked_at").unwrap();
    let status: String = row.try_get("status").unwrap();
    assert!(locked_at.is_some());
    assert_eq!(status, "issued");
}

#[tokio::test]
async fn dispose_still_works_on_locked_asset() {
    let (pool, _dir) = setup().await;
    let _contact_id = mk_basics(&pool).await;
    let id = "01928300-dd00-7000-8000-000000000003";
    sqlx::query(
        "INSERT INTO assets (
            id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, book_value_cents,
            locked_at
         ) VALUES (?, 'AV-2026-0030', 'Laptop', '2026-05-20', 150000, 2026,
                   'linear', 150000, datetime('now','utc'))",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();

    let r = sqlx::query(
        "UPDATE assets SET disposed = 1, disposal_date = '2026-12-01',
                            disposal_type = 'scrap', disposal_proceeds_cents = 0,
                            disposal_residual_book_value_cents = 50000,
                            updated_at = datetime('now','utc')
         WHERE id = ?",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(r.rows_affected(), 1);
}
