//! Geschäftsjahr-Domain (functional core).
//!
//! In v0.1 fix Kalenderjahr. API:
//! - `fiscal_year_for(date) -> i32`
//! - `fiscal_year_bounds(year) -> (NaiveDate, NaiveDate)` — 01.01.–31.12.
//! - `months_remaining_in_year(date) -> u32`
//! - `soft_lock_deadline(fiscal_year) -> NaiveDate` — 31.05. des Folgejahres
//!
//! Das GJ eines neuen Belegs ist immer `fiscal_year_for(Belegdatum)`; der Client
//! schickt keins mit (Plan W1.2/W9).

use chrono::{Datelike, NaiveDate};

/// Kleinstes Geschäftsjahr, das Commands annehmen (Tippfehler-Schutz).
pub const MIN_YEAR: i32 = 1900;
/// Größtes Geschäftsjahr: vierstellig wie die Datums-CHECKs der Datenbank
/// (`date(x) IS x`); darüber liefen Jahresgrenzen aus dem Bereich von
/// `NaiveDate` (SEC-2026.11.1-01).
pub const MAX_YEAR: i32 = 9999;

/// Geschäftsjahr aus einer Anfrage, geprüft gegen [`MIN_YEAR`]..=[`MAX_YEAR`];
/// sonst eine Meldung für den Nutzer (SEC-2026.11.1-01).
pub fn checked_year(year: i64) -> Result<i32, String> {
    i32::try_from(year)
        .ok()
        .filter(|y| (MIN_YEAR..=MAX_YEAR).contains(y))
        .ok_or_else(|| {
            format!("Das Geschäftsjahr {year} liegt außerhalb von {MIN_YEAR} bis {MAX_YEAR}.")
        })
}

pub fn fiscal_year_for(date: &NaiveDate) -> i32 {
    date.year()
}

pub fn fiscal_year_bounds(year: i32) -> (NaiveDate, NaiveDate) {
    (
        NaiveDate::from_ymd_opt(year, 1, 1).expect("valid year"),
        NaiveDate::from_ymd_opt(year, 12, 31).expect("valid year"),
    )
}

pub fn soft_lock_deadline(fiscal_year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(fiscal_year + 1, 5, 31).expect("valid year")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fiscal_year_is_calendar_year() {
        let d = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        assert_eq!(fiscal_year_for(&d), 2026);
    }

    #[test]
    fn bounds_jan1_to_dec31() {
        let (start, end) = fiscal_year_bounds(2026);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn soft_lock_is_may_31_next_year() {
        assert_eq!(
            soft_lock_deadline(2026),
            NaiveDate::from_ymd_opt(2027, 5, 31).unwrap()
        );
    }

    #[test]
    fn checked_year_accepts_four_digit_years_only() {
        assert_eq!(checked_year(2026), Ok(2026));
        assert_eq!(checked_year(1900), Ok(1900));
        assert_eq!(checked_year(9999), Ok(9999));
        for y in [1899, 10_000, 300_000, -1, i64::MAX, i64::MIN] {
            assert!(checked_year(y).unwrap_err().contains("außerhalb"), "{y}");
        }
    }

    #[test]
    fn it_compiles() {}
}
