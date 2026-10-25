//! Auto-AfA zur Geschäftsjahr-Wende (Block 15).
//!
//! Ab dem 01.01. bucht der Cron die Jahres-AfA des **Vorjahres** automatisch
//! (Manuel-Entscheidung Block 15: Auto-Buchung als Default, in den Einstellungen
//! abschaltbar via `depreciation_auto_year_close`). Die Buchung ist idempotent
//! (UNIQUE asset_id/fiscal_year) und bleibt bis zum GJ-Abschluss korrigierbar.
//!
//! Einmal pro Jahr: `depreciation_auto_close_last_year` merkt sich das zuletzt
//! automatisch gebuchte Jahr, damit der 5-Minuten-Tick nicht erneut bucht.
//!
//! Backup-Gate: ohne entsperrte Session bucht [`crate::depreciation::accrue_yearly`]
//! nichts (`skipped_locked`) — dann wird das Jahr NICHT als erledigt markiert,
//! der nächste Tick nach dem Entsperren holt es nach.
//!
//! Lauf F (D-03, ADR 0040): Pause ([`crate::scheduler::hold`]) stoppt auch die
//! Auto-AfA (ein Uhrsprung nach vorn buchte sonst Jahre im Voraus). Scheitert
//! der Lauf, entsteht ein Hinweis (einmal je Jahr) und ein persistierter
//! Backoff von [`FAILURE_BACKOFF_HOURS`] Stunden statt einer Warnung bei jedem
//! Tick. Jede Buchung (Eintrag, Restbuchwert, Audit) ist eine Transaktion
//! ([`crate::depreciation::accrue_yearly`]).

use crate::db::repo::app_settings;
use crate::depreciation::accrue_yearly;
use crate::error::Result;
use crate::notify::{self, NewNotification};
use crate::scheduler::{backoff, hold, JobCtx};
use chrono::Datelike;

/// Schlüssel des Backoffs nach einem gescheiterten Auto-AfA-Lauf.
pub const BACKOFF_KEY: &str = "depreciation_auto_last_failure_at";
/// Wartezeit nach einem Fehlschlag.
pub const FAILURE_BACKOFF_HOURS: i64 = 6;

pub async fn run(job: &JobCtx<'_>) -> Result<()> {
    let (pool, paths, session) = (job.pool, job.paths, job.session);
    let today = job.today();
    // Setting-Gate (Default an).
    if !app_settings::get_bool(pool, "depreciation_auto_year_close", true).await? {
        return Ok(());
    }

    let target_year = (today.year() - 1) as i64;

    // Einmal pro Jahr.
    let last = app_settings::get(pool, "depreciation_auto_close_last_year")
        .await?
        .and_then(|v| v.parse::<i64>().ok());
    if last == Some(target_year) {
        return Ok(());
    }

    // Ohne entsperrtes Backup nichts tun — nicht als erledigt markieren.
    if !session.is_unlocked() {
        return Ok(());
    }
    if hold::gate(pool, job.now()).await?.is_some() {
        return Ok(());
    }
    if backoff::blocked(
        pool,
        BACKOFF_KEY,
        job.now(),
        chrono::Duration::hours(FAILURE_BACKOFF_HOURS),
    )
    .await?
    {
        return Ok(());
    }

    // Lauf F (R2-S2-GJLOCK): kein Jahresabschluss während der Buchung.
    let fy = crate::fiscal_year::guard::booking_permit().await;
    let accrued = accrue_yearly::accrue_for_year(pool, paths, session, target_year, today).await;
    drop(fy);
    let report = match accrued {
        Ok(r) => r,
        Err(e) => {
            backoff::record_failure(pool, BACKOFF_KEY, job.now()).await?;
            let body = failure_text(target_year, &e);
            notify::emit(
                pool,
                NewNotification {
                    title: "Abschreibung fehlgeschlagen",
                    body: &body,
                    severity: "warning",
                    action_url: Some("/assets"),
                    dedup_key: Some(&format!("afa_auto_failed:{target_year}")),
                    ..Default::default()
                },
            )
            .await?;
            return Err(e);
        }
    };
    backoff::clear(pool, BACKOFF_KEY).await?;
    if report.skipped_locked {
        return Ok(());
    }

    // Jahr als automatisch verarbeitet markieren (auch bei 0 Anlagen — einmal/Jahr reicht).
    app_settings::set(
        pool,
        "depreciation_auto_close_last_year",
        &target_year.to_string(),
    )
    .await?;

    if report.booked_entries > 0 {
        let body = format!(
            "Für das Geschäftsjahr {target_year} wurden {} Abschreibungs-Buchungen automatisch erfasst \
             (Summe {}). Bitte prüfen und anschließend das Geschäftsjahr abschließen.",
            report.booked_entries,
            euro(report.total_depreciation_cents)
        );
        notify::emit(
            pool,
            NewNotification {
                rule_id: None,
                title: "Abschreibung gebucht",
                body: &body,
                severity: "info",
                action_url: Some("/assets"),
                dedup_key: Some(&format!("afa_auto:{target_year}")),
                ..Default::default()
            },
        )
        .await?;
    }
    Ok(())
}

/// Text des Hinweises „Abschreibung fehlgeschlagen“ (pure; F1d: eine Zeile
/// ohne eingebettete Leerzeichen-Blöcke).
fn failure_text(target_year: i64, e: &dyn std::fmt::Display) -> String {
    format!(
        "Die automatische Abschreibung für {target_year} ist fehlgeschlagen ({e}). \
         Bereits gebuchte Anlagen bleiben gebucht; Klein.Buch versucht es in \
         {FAILURE_BACKOFF_HOURS} Stunden erneut. Bitte die Anlage prüfen oder die \
         AfA unter Anschaffungen manuell buchen."
    )
}

/// Cent → "1.234,56 €" (deutsche Schreibweise, ohne externe Abhängigkeit).
fn euro(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.abs();
    let euros = abs / 100;
    let rest = abs % 100;
    // Tausenderpunkte.
    let mut s = euros.to_string();
    let bytes: Vec<char> = s.chars().rev().collect();
    let mut grouped = String::new();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(*c);
    }
    s = grouped.chars().rev().collect();
    format!("{}{},{:02} €", if neg { "-" } else { "" }, s, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euro_formats_german() {
        assert_eq!(euro(0), "0,00 €");
        assert_eq!(euro(5), "0,05 €");
        assert_eq!(euro(123_456), "1.234,56 €");
        assert_eq!(euro(-9_900), "-99,00 €");
    }

    /// F1d: Der Hinweistext hat keine Blöcke aus Leerzeichen (verlorene
    /// Zeilenfortsetzungen).
    #[test]
    fn failure_text_has_single_spaces() {
        let t = failure_text(2026, &"Datenbank gesperrt");
        assert!(!t.contains("  "), "{t}");
        assert!(t.contains("(Datenbank gesperrt). Bereits gebuchte"), "{t}");
        assert!(t.contains("in 6 Stunden erneut"), "{t}");
    }
}
