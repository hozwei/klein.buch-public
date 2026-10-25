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
//!
//! ## Sperre Jahresabschluss ↔ Buchung (Lauf F, R2-S2-GJLOCK, W14-034)
//!
//! Zwischen der Prüfung „Jahr offen“ und dem Schreiben einer Buchung darf kein
//! Abschluss liegen, sonst stünde eine Buchung im geschlossenen Jahr, die im
//! Festschreibungs-Snapshot fehlt. [`ensure_year_open`] liefert deshalb eine
//! [`BookingPermit`], die der Aufrufer bis nach dem Schreiben hält; der
//! Abschluss ([`crate::fiscal_year::lock::close_year`]) nimmt [`closing`] und
//! wartet, bis keine Buchung mehr läuft — neue Buchungen warten solange der
//! Abschluss läuft. Erlaubnisse lassen sich verschachteln (eine Buchung, die
//! eine weitere auslöst, blockiert sich nie selbst): ein Abschluss beginnt nur,
//! wenn keine Erlaubnis gehalten wird, und solange er läuft, entsteht keine.
//! Der Abschluss selbst darf keine Erlaubnis nehmen.

use crate::error::{Error, Result};
use sqlx::SqlitePool;
use std::sync::{Mutex, PoisonError};
use tokio::sync::Notify;

#[derive(Default)]
struct GateState {
    bookings: usize,
    closing: bool,
}

/// Prozessweite Sperre zwischen Buchungen und Jahresabschluss.
struct YearCloseGate {
    state: Mutex<GateState>,
    changed: Notify,
    closers: tokio::sync::Mutex<()>,
}

static GATE: YearCloseGate = YearCloseGate {
    state: Mutex::new(GateState {
        bookings: 0,
        closing: false,
    }),
    changed: Notify::const_new(),
    closers: tokio::sync::Mutex::const_new(()),
};

impl YearCloseGate {
    fn lock(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Erlaubnis zu buchen: solange sie gehalten wird, beginnt kein
/// Jahresabschluss. Bis nach dem Schreiben der Buchung halten.
#[must_use = "die Erlaubnis bis nach dem Schreiben der Buchung halten (let _fy = …)"]
pub struct BookingPermit {
    _private: (),
}

impl Drop for BookingPermit {
    fn drop(&mut self) {
        let mut st = GATE.lock();
        st.bookings -= 1;
        let idle = st.bookings == 0;
        drop(st);
        if idle {
            GATE.changed.notify_waiters();
        }
    }
}

/// Exklusiver Abschluss: solange gehalten, entsteht keine [`BookingPermit`].
#[must_use = "den Abschluss bis zum Commit halten"]
pub struct ClosingPermit {
    _closer: tokio::sync::MutexGuard<'static, ()>,
}

impl Drop for ClosingPermit {
    fn drop(&mut self) {
        GATE.lock().closing = false;
        GATE.changed.notify_waiters();
    }
}

/// Wartet, bis kein Abschluss läuft, und liefert eine Erlaubnis zu buchen.
pub async fn booking_permit() -> BookingPermit {
    loop {
        let changed = GATE.changed.notified();
        {
            let mut st = GATE.lock();
            if !st.closing {
                st.bookings += 1;
                return BookingPermit { _private: () };
            }
        }
        changed.await;
    }
}

/// Wartet, bis keine Buchung mehr läuft, und sperrt neue Buchungen bis zum
/// Drop. Nie aufrufen, während der eigene Task eine [`BookingPermit`] hält.
pub async fn closing() -> ClosingPermit {
    let closer = GATE.closers.lock().await;
    loop {
        let changed = GATE.changed.notified();
        {
            let mut st = GATE.lock();
            if st.bookings == 0 {
                st.closing = true;
                return ClosingPermit { _closer: closer };
            }
        }
        changed.await;
    }
}

/// `true`, wenn das Geschäftsjahr bereits abgeschlossen (festgeschrieben) ist.
pub async fn is_closed(pool: &SqlitePool, year: i64) -> Result<bool> {
    let row = sqlx::query("SELECT 1 AS x FROM fiscal_year_locks WHERE fiscal_year = ? LIMIT 1")
        .bind(year)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Fehlertext für eine Buchung in einem abgeschlossenen Jahr (Klartext fürs UI).
pub fn closed_error(year: i64) -> Error {
    Error::Domain(format!(
        "Das Geschäftsjahr {year} ist abgeschlossen (festgeschrieben). \
         Eine Buchung mit Datum in diesem Jahr ist nicht mehr möglich. \
         Eine fehlerhafte Rechnung lässt sich weiterhin stornieren."
    ))
}

/// Weitere Prüfung unter einer schon gehaltenen Erlaubnis (z. B. Zahlungs-
/// neben Belegjahr).
pub async fn check_open(pool: &SqlitePool, _permit: &BookingPermit, year: i64) -> Result<()> {
    if is_closed(pool, year).await? {
        return Err(closed_error(year));
    }
    Ok(())
}

/// Nimmt eine [`BookingPermit`] und prüft danach, dass das Geschäftsjahr offen
/// ist. Die Erlaubnis bis nach dem Schreiben der Buchung halten.
pub async fn ensure_year_open(pool: &SqlitePool, year: i64) -> Result<BookingPermit> {
    let permit = booking_permit().await;
    if is_closed(pool, year).await? {
        return Err(closed_error(year));
    }
    Ok(permit)
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
        let _a = ensure_year_open(&pool, 2027).await.unwrap();
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
        let _b = ensure_year_open(&pool, 2027).await.unwrap();
    }

    /// Lauf F (R2-S2-GJLOCK): ein Abschluss wartet auf laufende Buchungen,
    /// neue Buchungen warten auf den Abschluss; verschachtelte Erlaubnisse
    /// blockieren sich nicht.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn closing_waits_for_bookings_and_blocks_new_ones() {
        use std::time::Duration;
        let outer = booking_permit().await;
        // Verschachtelt: sofort.
        let inner = tokio::time::timeout(Duration::from_millis(200), booking_permit())
            .await
            .expect("verschachtelte Erlaubnis sofort");
        let closer = tokio::spawn(closing());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!closer.is_finished(), "Abschluss wartet auf Buchungen");
        drop(inner);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!closer.is_finished(), "noch eine Buchung offen");
        drop(outer);
        let closing = tokio::time::timeout(Duration::from_secs(5), closer)
            .await
            .expect("Abschluss beginnt")
            .unwrap();
        let booking = tokio::spawn(booking_permit());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!booking.is_finished(), "Buchung wartet auf den Abschluss");
        drop(closing);
        let _p = tokio::time::timeout(Duration::from_secs(5), booking)
            .await
            .expect("Buchung nach dem Abschluss")
            .unwrap();
    }
}
