//! Geschäftsjahr-Domain (functional core).
//!
//! In v0.1 fix Kalenderjahr. API:
//! - `fiscal_year_for(date) -> i32`
//! - `fiscal_year_bounds(year) -> (NaiveDate, NaiveDate)` — 01.01.–31.12.
//! - `months_remaining_in_year(date) -> u32`
//! - `soft_lock_deadline(fiscal_year) -> NaiveDate` — 31.05. des Folgejahres

use chrono::{Datelike, NaiveDate};

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
    fn it_compiles() {}
}
