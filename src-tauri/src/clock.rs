//! Zentrale Uhr in **Europe/Berlin** (Plan W1.8, E12).
//!
//! Belegdatum, Geschäftsjahr, Dateinamen und Monatsgrenzen hängen an der
//! deutschen Ortszeit — unabhängig von der Zeitzone des Prozesses (Container
//! laufen oft in UTC). Deshalb kein `chrono::Local` im Produktivcode, sondern
//! diese Funktionen.
//!
//! **Injizierbare Uhr (Lauf F, ADR 0040):** Zeitabhängige Abläufe (Scheduler-Tick,
//! Abo-Läufe, Auto-AfA) lesen die Zeit über das Trait [`Clock`] aus dem
//! `AppCtx` (`AppCtx::clock`). Im Betrieb ist das [`SystemClock`], in Tests eine
//! [`FixedClock`], die über alle Tokio-Worker und `spawn_blocking` hinweg gilt
//! (geteilt per `Arc`). Die freien Funktionen [`now_utc`] und [`today_berlin`]
//! bleiben die Fassade für Commands ohne zeitkritische Abläufe.
//!
//! Die frühere thread-lokale Fixierung gibt es nur noch in den Unit-Tests dieses
//! Crates (`cfg(test)`); sie erreicht andere Threads nicht.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::{Europe::Berlin, Tz};
use std::sync::{Mutex, PoisonError};

#[cfg(test)]
thread_local! {
    static FIXED_NOW: std::cell::Cell<Option<DateTime<Utc>>> = const { std::cell::Cell::new(None) };
}

/// Uhr der Anwendung (UTC als Quelle, Berliner Ortszeit abgeleitet).
pub trait Clock: Send + Sync {
    /// Aktueller Zeitpunkt in UTC.
    fn now_utc(&self) -> DateTime<Utc>;

    /// Aktueller Zeitpunkt in Europe/Berlin.
    fn now_berlin(&self) -> DateTime<Tz> {
        self.now_utc().with_timezone(&Berlin)
    }

    /// Heutiges Datum in Europe/Berlin.
    fn today_berlin(&self) -> NaiveDate {
        self.now_berlin().date_naive()
    }
}

/// Systemuhr (Betrieb).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        now_utc()
    }
}

/// Feste, verstellbare Uhr für Tests. Gilt für jeden Thread, der dieselbe
/// Instanz hält (anders als die frühere thread-lokale Fixierung).
#[derive(Debug)]
pub struct FixedClock {
    now: Mutex<DateTime<Utc>>,
}

impl FixedClock {
    /// Uhr auf `utc` gestellt.
    pub fn at(utc: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(utc),
        }
    }

    /// Uhr auf `hour:minute` Berliner Ortszeit am Tag `day` gestellt
    /// (siehe [`berlin_to_utc`]).
    pub fn at_berlin(day: NaiveDate, hour: u32, minute: u32) -> Self {
        Self::at(berlin_to_utc(day, hour, minute))
    }

    /// Uhr neu stellen.
    pub fn set(&self, utc: DateTime<Utc>) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = utc;
    }

    /// Uhr auf `hour:minute` Berliner Ortszeit am Tag `day` stellen.
    pub fn set_berlin(&self, day: NaiveDate, hour: u32, minute: u32) {
        self.set(berlin_to_utc(day, hour, minute));
    }

    /// Uhr um `by` vorstellen.
    pub fn advance(&self, by: chrono::Duration) {
        let mut now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *now += by;
    }
}

impl Clock for FixedClock {
    fn now_utc(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Berliner Ortszeit → UTC. Bei doppelter Stunde (Ende der Sommerzeit) gilt der
/// frühere Zeitpunkt, bei übersprungener Stunde (Beginn der Sommerzeit) wird mit
/// Winterzeit gerechnet (02:30 → 01:30 UTC).
pub fn berlin_to_utc(day: NaiveDate, hour: u32, minute: u32) -> DateTime<Utc> {
    use chrono::TimeZone;
    let local = day
        .and_hms_opt(hour.min(23), minute.min(59), 0)
        .unwrap_or_else(|| day.and_time(chrono::NaiveTime::MIN));
    Berlin
        .from_local_datetime(&local)
        .earliest()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|| (local - chrono::Duration::hours(1)).and_utc())
}

/// Aktueller Zeitpunkt in UTC (Systemuhr; in den Unit-Tests dieses Crates ggf.
/// die thread-lokal fixierte Zeit).
pub fn now_utc() -> DateTime<Utc> {
    #[cfg(test)]
    if let Some(fixed) = FIXED_NOW.with(|f| f.get()) {
        return fixed;
    }
    Utc::now()
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
    db_utc(ts).map(|u| u.with_timezone(&Berlin))
}

/// Liest einen DB-Zeitstempel (UTC, `YYYY-MM-DD HH:MM:SS`, optional `T`/`Z`).
pub fn db_utc(ts: &str) -> Option<DateTime<Utc>> {
    let t = ts.trim().trim_end_matches('Z').replace('T', " ");
    let t = t.get(0..19).unwrap_or(&t);
    NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|n| n.and_utc())
}

/// UTC-Zeitpunkt im DB-Format (`YYYY-MM-DD HH:MM:SS`, wie `datetime('now','utc')`).
pub fn to_db_utc(utc: DateTime<Utc>) -> String {
    utc.format("%Y-%m-%d %H:%M:%S").to_string()
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
#[cfg(test)]
#[must_use = "die Fixierung endet mit dem Drop des Guards"]
pub struct FixedClockGuard {
    previous: Option<DateTime<Utc>>,
}

#[cfg(test)]
impl Drop for FixedClockGuard {
    fn drop(&mut self) {
        let prev = self.previous;
        FIXED_NOW.with(|f| f.set(prev));
    }
}

/// Fixiert die Uhr im aktuellen Thread (nur Unit-Tests dieses Crates;
/// Integrationstests nutzen [`FixedClock`]).
#[cfg(test)]
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
        assert_eq!(FixedClock::at(utc).today_berlin(), today);
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
    fn fixed_clock_is_shared_across_threads() {
        let clock = std::sync::Arc::new(FixedClock::at_berlin(
            NaiveDate::from_ymd_opt(2026, 3, 29).unwrap(),
            1,
            30,
        ));
        // 01:30 MEZ am Tag der Umstellung = 00:30 UTC
        assert_eq!(
            clock.now_utc(),
            Utc.with_ymd_and_hms(2026, 3, 29, 0, 30, 0).unwrap()
        );
        let other = clock.clone();
        let seen = std::thread::spawn(move || other.today_berlin())
            .join()
            .unwrap();
        assert_eq!(seen, NaiveDate::from_ymd_opt(2026, 3, 29).unwrap());
        clock.advance(chrono::Duration::hours(2));
        // 02:30 UTC = 04:30 MESZ
        assert_eq!(clock.now_berlin().format("%H:%M").to_string(), "04:30");
    }

    #[test]
    fn berlin_to_utc_handles_dst_edges() {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        // 25.10.2026 02:30 gibt es zweimal → früherer Zeitpunkt (MESZ)
        assert_eq!(
            berlin_to_utc(d(10, 25), 2, 30),
            Utc.with_ymd_and_hms(2026, 10, 25, 0, 30, 0).unwrap()
        );
        // 29.03.2026 02:30 gibt es nicht → mit Winterzeit gerechnet
        assert_eq!(
            berlin_to_utc(d(3, 29), 2, 30),
            Utc.with_ymd_and_hms(2026, 3, 29, 1, 30, 0).unwrap()
        );
        assert_eq!(
            berlin_to_utc(d(1, 1), 0, 0),
            Utc.with_ymd_and_hms(2025, 12, 31, 23, 0, 0).unwrap()
        );
    }

    #[test]
    fn db_timestamps_are_read_as_utc() {
        let b = db_utc_to_berlin("2026-10-31 23:30:00").unwrap();
        assert_eq!(b.format("%Y-%m").to_string(), "2026-11");
        let b = db_utc_to_berlin("2026-10-31T22:59:59Z").unwrap();
        assert_eq!(b.format("%Y-%m-%d %H:%M").to_string(), "2026-10-31 23:59");
        assert!(db_utc_to_berlin("kaputt").is_none());
        let u = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        assert_eq!(db_utc(&to_db_utc(u)), Some(u));
    }
}
