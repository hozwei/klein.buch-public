//! Atomare Belegnummer-Allokation pro (`doc_type`, `fiscal_year`).
//!
//! Schicht: **Imperative Shell**. Format-Layer (Prefix-Schema, Zero-Padding)
//! lebt in [`crate::domain::numbering`]; hier nur die Counter-Persistenz.
//!
//! ## Pattern
//!
//! Pro Aufruf wird in einer Transaktion ausgeführt:
//!
//! 1. `INSERT OR IGNORE INTO doc_number_counters (doc_type, fiscal_year, last_value) VALUES (?, ?, 0)`
//!    legt eine Zeile für ein neues Geschäftsjahr an, wenn noch keine existiert.
//! 2. `UPDATE doc_number_counters SET last_value = last_value + 1
//!    WHERE doc_type = ? AND fiscal_year = ? RETURNING last_value`
//!    inkrementiert atomar und liefert den frischen Counter.
//!
//! SQLite garantiert über die Transaktion + den PRIMARY KEY (doc_type,
//! fiscal_year) Lückenlosigkeit und Eindeutigkeit ohne Race-Condition. WAL
//! reicht; kein expliziter `BEGIN IMMEDIATE` nötig, weil sqlx-Pool die
//! Connection für die Dauer der Transaktion exklusiv hält.
//!
//! ## GoBD
//!
//! Lückenlose Nummerierung pro Geschäftsjahr ist GoBD-Pflicht. Storno
//! erhält einen **eigenen** Counter (`DocType::StornoInvoice`), damit
//! die Rechnungs-Sequenz weiterhin lückenlos bleibt.
//!
//! ## Forward-only
//!
//! Counter werden niemals dekrementiert. Wenn eine Allokation passiert ist
//! aber die nachfolgende Operation scheitert (z. B. KoSIT-Validierung
//! schlägt fehl), bleibt die Nummer "verbraucht" und produziert eine Lücke.
//! Das ist GoBD-konform dokumentierbar (Storno-Vorgang); siehe
//! `memory/klein-buch/gobd-archivierung.md`.

use crate::{
    domain::numbering::{format, DocType},
    error::Result,
};
use sqlx::{Row, SqlitePool};

/// Allokiert die nächste Belegnummer und liefert die formatierte Version.
///
/// Wirft `Error::Db`, wenn die Transaktion scheitert (z. B. Disk-Voll,
/// Lock-Konflikt). Wirft niemals `last_value < 1` — wenn das passiert,
/// ist die DB korrupt.
pub async fn next_number(pool: &SqlitePool, doc_type: DocType, fiscal_year: i32) -> Result<String> {
    let seq = next_seq(pool, doc_type, fiscal_year).await?;
    Ok(format(doc_type, fiscal_year, seq))
}

/// Allokiert nur den Counter (ohne Formatierung). Hauptsächlich für Tests
/// und sehr seltene Sonderfälle, die den Slug direkt brauchen.
pub async fn next_seq(pool: &SqlitePool, doc_type: DocType, fiscal_year: i32) -> Result<u32> {
    let slug = doc_type.db_slug();
    let mut tx = pool.begin().await?;

    // Zeile für (slug, year) sicherstellen — kollidiert sie schon, ignorieren.
    sqlx::query(
        "INSERT OR IGNORE INTO doc_number_counters (doc_type, fiscal_year, last_value)
         VALUES (?, ?, 0)",
    )
    .bind(slug)
    .bind(fiscal_year)
    .execute(&mut *tx)
    .await?;

    // Atomar inkrementieren + neuen Wert zurückgeben.
    let row = sqlx::query(
        "UPDATE doc_number_counters
            SET last_value = last_value + 1
          WHERE doc_type = ? AND fiscal_year = ?
        RETURNING last_value",
    )
    .bind(slug)
    .bind(fiscal_year)
    .fetch_one(&mut *tx)
    .await?;

    let last_value: i64 = row.try_get("last_value")?;
    tx.commit().await?;

    if last_value < 1 {
        return Err(crate::error::Error::Domain(format!(
            "doc_number_counters.last_value < 1 für ({}, {}): {}",
            slug, fiscal_year, last_value
        )));
    }
    Ok(last_value as u32)
}

/// Liefert den aktuellen Counter-Stand, ohne ihn zu verändern. Für UI und
/// Tests; nicht für die Allokation benutzen.
pub async fn peek_seq(pool: &SqlitePool, doc_type: DocType, fiscal_year: i32) -> Result<u32> {
    let slug = doc_type.db_slug();
    let row = sqlx::query(
        "SELECT last_value FROM doc_number_counters
         WHERE doc_type = ? AND fiscal_year = ?",
    )
    .bind(slug)
    .bind(fiscal_year)
    .fetch_optional(pool)
    .await?;

    Ok(row
        .map(|r| r.get::<i64, _>("last_value") as u32)
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// In-Memory-DB mit dem Numbering-Counter-Schema. Genügt für diese
    /// Modul-Tests; die produktive Migration läuft separat via
    /// [`crate::db::MIGRATOR`].
    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE doc_number_counters (
                doc_type    TEXT NOT NULL,
                fiscal_year INTEGER NOT NULL,
                last_value  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (doc_type, fiscal_year)
            ) STRICT",
        )
        .execute(&pool)
        .await
        .expect("create table");

        pool
    }

    #[tokio::test]
    async fn first_invoice_in_fiscal_year_is_one() {
        let pool = fresh_pool().await;
        let n = next_number(&pool, DocType::Invoice, 2026).await.unwrap();
        assert_eq!(n, "RE-2026-0001");
    }

    #[tokio::test]
    async fn sequential_invoices_increment_per_year() {
        let pool = fresh_pool().await;
        for expected in 1..=5u32 {
            let n = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
            assert_eq!(n, expected);
        }
        // Anderer GJ → eigener Counter, startet bei 1.
        let n = next_seq(&pool, DocType::Invoice, 2027).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn storno_and_invoice_have_independent_counters() {
        let pool = fresh_pool().await;
        let _ = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        let _ = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        let s1 = next_number(&pool, DocType::StornoInvoice, 2026)
            .await
            .unwrap();
        assert_eq!(s1, "ST-2026-0001");
        // Rechnungs-Sequenz bleibt unberührt
        let i3 = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        assert_eq!(i3, 3);
    }

    #[tokio::test]
    async fn peek_does_not_advance_counter() {
        let pool = fresh_pool().await;
        assert_eq!(peek_seq(&pool, DocType::Invoice, 2026).await.unwrap(), 0);
        let _ = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        assert_eq!(peek_seq(&pool, DocType::Invoice, 2026).await.unwrap(), 1);
        assert_eq!(peek_seq(&pool, DocType::Invoice, 2026).await.unwrap(), 1);
    }

    /// Pessimistischer Concurrency-Test: 50 parallele Allokationen müssen
    /// eine kontinuierliche, kollisionsfreie Sequenz 1..=50 ergeben.
    #[tokio::test]
    async fn parallel_allocations_have_no_gaps_or_duplicates() {
        let pool = fresh_pool().await;
        let mut handles = Vec::new();
        for _ in 0..50 {
            let p = pool.clone();
            handles.push(tokio::spawn(async move {
                next_seq(&p, DocType::Invoice, 2026).await.unwrap()
            }));
        }
        let mut got: Vec<u32> = Vec::new();
        for h in handles {
            got.push(h.await.unwrap());
        }
        got.sort_unstable();
        let expected: Vec<u32> = (1..=50).collect();
        assert_eq!(got, expected, "parallele Allokation hat Lücken/Duplikate");
    }

    #[tokio::test]
    async fn all_doc_types_have_independent_counters() {
        let pool = fresh_pool().await;
        for dt in [
            DocType::Quote,
            DocType::Invoice,
            DocType::StornoInvoice,
            DocType::Expense,
            DocType::PrivateMovement,
            DocType::Asset,
        ] {
            assert_eq!(next_seq(&pool, dt, 2026).await.unwrap(), 1);
        }
    }
}
