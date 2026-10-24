//! Zentrale Uhr in **Europe/Berlin** (Plan W1.8, E12).
//!
//! Belegdatum, Geschäftsjahr, Dateinamen und Monatsgrenzen hängen an der
//! deutschen Ortszeit — unabhängig von der Zeitzone des Prozesses (Container
//! laufen oft in UTC). Deshalb kein `chrono::Local` im Produktivcode, sondern
//! diese Funktionen.
//!
//! Für Tests ist die Uhr pro Thread fixierbar ([`fix_now_for_tests`]); die
//! Fixierung gilt nur im aufrufenden Thread (z. B. `#[tokio::test]` mit
//! `current_thread`-Runtime) und endet mit dem Drop des Guards.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::{Europe::Berlin, Tz};
use std::cell::Cell;

thread_local! {
    static FIXED_NOW: Cell<Option<DateTime<Utc>>> = const { Cell::new(None) };
}

/// Aktueller Zeitpunkt in UTC (bzw. die für Tests fixierte Zeit).
pub fn now_utc() -> DateTime<Utc> {
    FIXED_NOW.with(|f| f.get()).unwrap_or_else(Utc::now)
}

/// Aktueller Zeitpunkt in Europe/Berlin.
pub fn now_berlin() -> DateTime<Tz> {
    now_utc().with_timezone(&Berlin)
}

/// Heutiges Datum in Europe/Berlin.
pub fn today_berlin() -> NaiveDate {
    now_berlin().date_naive()
}

/// Kalenderdatum eines UTC-Zeitpunkts in Europe/Berlin (pure).
pub fn berlin_date(utc: DateTime<Utc>) -> NaiveDate {
    utc.with_timezone(&Berlin).date_naive()
}

/// Wandelt einen in der DB gespeicherten UTC-Zeitstempel (`datetime('now','utc')`,
/// Format `YYYY-MM-DD HH:MM:SS`, optional mit `T`/`Z`) in Berliner Ortszeit.
/// `None`, wenn das Format nicht passt.
pub fn db_utc_to_berlin(ts: &str) -> Option<DateTime<Tz>> {
    let t = ts.trim().trim_end_matches('Z').replace('T', " ");
    let t = t.get(0..19).unwrap_or(&t);
    NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|n| n.and_utc().with_timezone(&Berlin))
}

/// Beginn eines Berliner Kalendertags als DB-Zeitstempel in UTC
/// (`YYYY-MM-DD HH:MM:SS`) — für Tagesfilter auf `created_at`, ohne
/// `date(…, 'localtime')` (das hing an der Prozess-Zeitzone, W9).
pub fn berlin_day_start_db_utc(day: NaiveDate) -> String {
    use chrono::TimeZone;
    let midnight = day.and_time(chrono::NaiveTime::MIN);
    // Mitternacht gibt es in Berlin immer (Umstellung um 2 bzw. 3 Uhr).
    let start = Berlin
        .from_local_datetime(&midnight)
        .earliest()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|| midnight.and_utc());
    start.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Hält die Fixierung aus [`fix_now_for_tests`]; beim Drop gilt wieder die echte Uhr.
#[must_use = "die Fixierung endet mit dem Drop des Guards"]
pub struct FixedClockGuard {
    previous: Option<DateTime<Utc>>,
}

impl Drop for FixedClockGuard {
    fn drop(&mut self) {
        let prev = self.previous;
        FIXED_NOW.with(|f| f.set(prev));
    }
}

/// Fixiert die Uhr im aktuellen Thread (nur für Tests gedacht).
#[doc(hidden)]
pub fn fix_now_for_tests(utc: DateTime<Utc>) -> FixedClockGuard {
    let previous = FIXED_NOW.with(|f| f.replace(Some(utc)));
    FixedClockGuard { previous }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    #[test]
    fn berlin_day_bounds_in_utc() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(
            berlin_day_start_db_utc(d(2026, 7, 15)),
            "2026-07-14 22:00:00"
        );
        assert_eq!(
            berlin_day_start_db_utc(d(2026, 1, 1)),
            "2025-12-31 23:00:00"
        );
        // Tag der Zeitumstellung (29.03./25.10.2026): Mitternacht noch in der alten Zone
        assert_eq!(
            berlin_day_start_db_utc(d(2026, 3, 29)),
            "2026-03-28 23:00:00"
        );
        assert_eq!(
            berlin_day_start_db_utc(d(2026, 10, 25)),
            "2026-10-24 22:00:00"
        );
    }

    #[test]
    fn new_years_eve_late_evening_utc_is_next_year_in_berlin() {
        let utc = Utc.with_ymd_and_hms(2026, 12, 31, 23, 30, 0).unwrap();
        let _g = fix_now_for_tests(utc);
        let today = today_berlin();
        assert_eq!(today, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
        assert_eq!(today.year(), 2027);
    }

    #[test]
    fn summer_time_offset_is_two_hours() {
        let utc = Utc.with_ymd_and_hms(2026, 6, 30, 22, 30, 0).unwrap();
        assert_eq!(
            berlin_date(utc),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
        let utc = Utc.with_ymd_and_hms(2026, 6, 30, 21, 30, 0).unwrap();
        assert_eq!(
            berlin_date(utc),
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap()
        );
    }

    #[test]
    fn guard_restores_real_clock() {
        let utc = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        {
            let _g = fix_now_for_tests(utc);
            assert_eq!(now_utc(), utc);
        }
        assert!(now_utc().year() >= 2026);
    }

    #[test]
    fn db_timestamps_are_read_as_utc() {
        let b = db_utc_to_berlin("2026-10-31 23:30:00").unwrap();
        assert_eq!(b.format("%Y-%m").to_string(), "2026-11");
        let b = db_utc_to_berlin("2026-10-31T22:59:59Z").unwrap();
        assert_eq!(b.format("%Y-%m-%d %H:%M").to_string(), "2026-10-31 23:59");
        assert!(db_utc_to_berlin("kaputt").is_none());
    }
}
