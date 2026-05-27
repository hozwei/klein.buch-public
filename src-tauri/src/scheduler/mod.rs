//! Scheduler-Modul (Phase 2B+).
//! Periodischer Tick (täglich beim App-Start + Hourly), der Recurring-Expenses,
//! Reminders, Integrity-Checks und AfA-Year-Close anstößt.

pub mod depreciation_year_close;
pub mod drop_folder;
pub mod integrity_check_cron;
pub mod recurring;
pub mod recurring_invoice;
pub mod reminders;
pub mod tick;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
