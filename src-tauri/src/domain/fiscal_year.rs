//! Geschäftsjahr-Domain (functional core).
//!
//! In v0.1 fix Kalenderjahr. API:
//! - `fiscal_year_for(date) -> i32`
//! - `fiscal_year_bounds(year) -> (NaiveDate, NaiveDate)` — 01.01.–31.12.
//! - `months_remaining_in_year(date) -> u32`
//! - `soft_lock_deadline(fiscal_year) -> NaiveDate` — 31.05. des Folgejahres
//! - `resolve_document_fiscal_year(date, client)` — GJ eines neuen Belegs (Plan W1.2)

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

/// Geschäftsjahr eines neuen Belegs = Jahr des Belegdatums (Plan W1.2). Der
/// Server leitet es selbst ab; ein vom Client mitgeschicktes GJ muss dazu passen,
/// sonst Fehlertext (keine Nummer aus dem falschen Kreis).
pub fn resolve_document_fiscal_year(
    document_date: &NaiveDate,
    client_fiscal_year: Option<i64>,
) -> Result<i64, String> {
    let derived = i64::from(fiscal_year_for(document_date));
    match client_fiscal_year {
        Some(fy) if fy != derived => Err(format!(
            "Geschäftsjahr {fy} passt nicht zum Belegdatum {} (Geschäftsjahr {derived}).",
            document_date.format("%d.%m.%Y")
        )),
        _ => Ok(derived),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_fiscal_year_follows_document_date() {
        let d = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        assert_eq!(resolve_document_fiscal_year(&d, None), Ok(2027));
        assert_eq!(resolve_document_fiscal_year(&d, Some(2027)), Ok(2027));
        let err = resolve_document_fiscal_year(&d, Some(2026)).unwrap_err();
        assert!(err.contains("2026") && err.contains("01.01.2027"), "{err}");
    }

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
