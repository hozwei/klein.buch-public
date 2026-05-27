//! GJ-Festschreibungs-Guard (Block 15).
//!
//! Prüfungssicherer GJ-Lock (Manuel-Entscheidung Block 15, §146 Abs. 4 AO / GoBD):
//! ist ein Geschäftsjahr abgeschlossen (`fiscal_year_locks`-Eintrag vorhanden),
//! darf keine **neue** Buchung mehr ein Datum in diesem Jahr tragen. Die
//! Buchungs-Commands rufen [`ensure_year_open`] mit dem maßgeblichen Jahr auf
//! (Kosten-Datum, Zahlungs-Datum, Rechnungs-/Angebots-Datum).
//!
//! Storno bleibt bewusst möglich: ein Storno-Beleg trägt das **laufende** GJ-Datum
//! und wird daher nicht durch den Lock des Originaljahres blockiert.

use crate::error::{Error, Result};
use sqlx::SqlitePool;

/// `true`, wenn das Geschäftsjahr bereits abgeschlossen (festgeschrieben) ist.
pub async fn is_closed(pool: &SqlitePool, year: i64) -> Result<bool> {
    let row = sqlx::query("SELECT 1 AS x FROM fiscal_year_locks WHERE fiscal_year = ? LIMIT 1")
        .bind(year)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Fehler, wenn das Geschäftsjahr abgeschlossen ist. Klartext-Meldung fürs UI.
pub async fn ensure_year_open(pool: &SqlitePool, year: i64) -> Result<()> {
    if is_closed(pool, year).await? {
        return Err(Error::Domain(format!(
            "Das Geschäftsjahr {year} ist abgeschlossen (festgeschrieben). \
             Eine Buchung mit Datum in diesem Jahr ist nicht mehr möglich. \
             Eine fehlerhafte Rechnung lässt sich weiterhin stornieren."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE fiscal_year_locks (
                fiscal_year INTEGER PRIMARY KEY NOT NULL,
                closed_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
                income_total_cents INTEGER NOT NULL,
                expense_total_cents INTEGER NOT NULL,
                afa_total_cents INTEGER NOT NULL,
                surplus_cents INTEGER NOT NULL,
                assets_locked INTEGER NOT NULL DEFAULT 0,
                depreciation_entries_locked INTEGER NOT NULL DEFAULT 0,
                app_version TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                notes TEXT
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn open_year_passes_closed_year_blocks() {
        let pool = pool().await;
        // 2027 offen → ok
        ensure_year_open(&pool, 2027).await.unwrap();
        // 2026 schließen
        sqlx::query(
            "INSERT INTO fiscal_year_locks
                (fiscal_year, income_total_cents, expense_total_cents, afa_total_cents,
                 surplus_cents, app_version, schema_version)
             VALUES (2026, 0, 0, 0, 0, '0.1.0', 13)",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(is_closed(&pool, 2026).await.unwrap());
        assert!(ensure_year_open(&pool, 2026).await.is_err());
        // andere Jahre weiterhin offen
        ensure_year_open(&pool, 2027).await.unwrap();
    }
}
