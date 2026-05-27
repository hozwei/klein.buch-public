//! Recurring-Auto-Anlage am Stichtag (Imperative Shell) — Block 10.
//!
//! Erzeugt für fällige Abos (`auto_create_expense=1`) Kosten-Positionen. Die
//! pure Stichtags-Mathematik liegt in [`crate::domain::recurring`]; hier nur
//! die DB-/Archiv-/Backup-Orchestrierung.
//!
//! ## Verhalten (mit Manuel abgestimmt, Block 10)
//!
//! - **Catch-up:** War die App länger zu, werden ALLE verpassten Perioden
//!   nachgeholt — pro Periode eine Kosten-Position mit dem jeweiligen
//!   Stichtag als Beleg-Datum —, bis `next_due_date` wieder in der Zukunft
//!   liegt. Ein [`CATCH_UP_CAP`] verhindert Endlosschleifen.
//! - **Zahlstatus:** Auto-angelegte Kosten entstehen `paid_date = NULL`
//!   („noch nicht bezahlt"). Erst die vom Nutzer bestätigte Zahlung
//!   (`expenses_set_payment`) setzt das Cash-Basis-Datum → die EÜR (Block 13)
//!   zählt nur bestätigte Zahlungen (prüfungssicher).
//! - **Unlock-Gate:** Jede angelegte Position ist ein Lock-Event und braucht
//!   ein Backup (Backup-Hardline). Solange die Backup-Session gesperrt ist
//!   (vor Passphrase-Eingabe), legt der Tick nichts an — der nächste Tick nach
//!   dem Entsperren holt es nach.
//! - **Ein Backup pro Burst:** Statt eines Backups je Position wird nach dem
//!   gesamten Lauf genau ein Backup erstellt (Snapshot = Voll-Stand).
//!
//! ## Beträge
//!
//! Das Abo kennt nur einen erwarteten **Brutto**-Betrag (Eingangsseite; §19
//! betrifft nur Ausgangsbelege). Die erzeugte Position setzt `net = brutto`,
//! `tax = 0`. Weicht der echte Beleg ab, wird per Storno + Neuerfassung
//! korrigiert (Kosten sind sofort gelockt).

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::backup::{self, BackupSession};
use crate::config::Paths;
use crate::db::models::{ExpenseRow, RecurringSubscriptionRow};
use crate::db::numbering;
use crate::db::repo::{audit_log, contacts, expenses, recurring};
use crate::domain::expense::{self, ExpenseInput};
use crate::domain::numbering::DocType;
use crate::domain::recurring::{compute_next_due_date, Frequency};
use crate::error::{Error, Result};
use crate::fiscal_year::guard;

/// Sicherheits-Obergrenze für Catch-up je Abo und Lauf (z. B. 10 Jahre
/// monatlich = 120). Schützt gegen eine Endlosschleife, falls die
/// Stichtags-Rechnung wider Erwarten nicht vorrückt.
pub const CATCH_UP_CAP: usize = 120;

/// Ergebnis eines Due-Laufs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessReport {
    /// Lauf übersprungen, weil die Backup-Session gesperrt war.
    pub skipped_locked: bool,
    /// Anzahl Abos, für die mindestens eine Position erzeugt wurde.
    pub processed_subscriptions: usize,
    /// Gesamtzahl erzeugter Kosten-Positionen.
    pub created_expenses: usize,
}

/// Verarbeitet alle fälligen Auto-Abos zum Stichtag `today` (Europe/Berlin).
pub async fn process_due(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    today: NaiveDate,
) -> Result<ProcessReport> {
    if !session.is_unlocked() {
        tracing::debug!("Recurring-Due übersprungen: Backup-Session gesperrt");
        return Ok(ProcessReport {
            skipped_locked: true,
            ..Default::default()
        });
    }

    let due = recurring::list_due_auto(pool, &today.to_string()).await?;
    let mut report = ProcessReport::default();

    for sub in &due {
        let created = catch_up_subscription(pool, sub, today).await?;
        if created > 0 {
            report.processed_subscriptions += 1;
            report.created_expenses += created;
        }
    }

    // Ein Backup deckt den gesamten Burst ab.
    if report.created_expenses > 0 {
        backup::auto_backup_if_unlocked(pool, paths, session, "recurring.lock")
            .await
            .ok();
    }
    Ok(report)
}

/// Manueller Einzel-Trigger („Jetzt erfassen") für ein fälliges Abo —
/// unabhängig vom `auto_create_expense`-Flag. Legt **genau eine** Position für
/// den aktuellen Stichtag an und rückt das Abo um eine Periode vor.
pub async fn run_now(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    id: &str,
    today: NaiveDate,
) -> Result<ExpenseRow> {
    let sub = recurring::get(pool, id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))?;
    if sub.active == 0 {
        return Err(Error::Domain(
            "Pausiertes Abo: bitte erst fortsetzen, dann erfassen.".into(),
        ));
    }
    let (freq, day) = freq_and_day(&sub)?;
    let occurrence = parse_date(&sub.next_due_date)?;
    if occurrence > today {
        return Err(Error::Domain(format!(
            "Abo ist erst am {occurrence} fällig — eine vorzeitige Erfassung würde ein zukünftiges Beleg-Datum erzeugen."
        )));
    }

    let expense_id =
        create_expense_for_occurrence(pool, &sub, occurrence, today, "recurring.run_now").await?;
    let following = compute_next_due_date(freq, day, occurrence);
    recurring::advance(pool, &sub.id, &expense_id, &following.to_string()).await?;

    backup::auto_backup_if_unlocked(pool, paths, session, "recurring.lock")
        .await
        .ok();

    expenses::get(pool, &expense_id)
        .await?
        .ok_or_else(|| Error::Domain("run_now: post-create SELECT leer".into()))
}

// ---- intern ----------------------------------------------------------------

/// Legt für ein Abo alle bis `today` fälligen Perioden nach. Liefert die Anzahl
/// erzeugter Positionen.
async fn catch_up_subscription(
    pool: &SqlitePool,
    sub: &RecurringSubscriptionRow,
    today: NaiveDate,
) -> Result<usize> {
    let (freq, day) = freq_and_day(sub)?;
    let mut next = parse_date(&sub.next_due_date)?;
    let mut created = 0usize;

    while next <= today && created < CATCH_UP_CAP {
        // R2-005 (GJ-Guard): Im Scheduler-Loop wollen wir bei einem
        // festgeschriebenen GJ nicht den gesamten Tick hart abbrechen —
        // das würde alle 5 Minuten wieder denselben Konflikt werfen. Statt
        // dessen: einmal pro Abo warnen, Abo überspringen, weitere Abos
        // laufen. Defense-in-Depth bleibt: `create_expense_for_occurrence`
        // ruft den Guard zusätzlich.
        if guard::is_closed(pool, next.year() as i64).await? {
            tracing::warn!(
                "Recurring-Catch-up für Abo {} übersprungen: Periode {next} fällt ins festgeschriebene GJ {}",
                sub.id,
                next.year()
            );
            break;
        }

        let expense_id =
            create_expense_for_occurrence(pool, sub, next, today, "recurring.auto_created").await?;
        let following = compute_next_due_date(freq, day, next);
        recurring::advance(pool, &sub.id, &expense_id, &following.to_string()).await?;
        next = following;
        created += 1;
    }

    if created == CATCH_UP_CAP {
        tracing::warn!(
            "Abo {} hat den Catch-up-Cap ({CATCH_UP_CAP}) erreicht — weitere Perioden beim nächsten Lauf",
            sub.id
        );
    }
    Ok(created)
}

/// Baut die [`ExpenseInput`] aus dem Abo-Template, allokiert die Belegnummer,
/// schreibt die Position sofort fest, verknüpft sie mit dem Abo und auditiert
/// den Vorgang. Liefert die Expense-ID.
async fn create_expense_for_occurrence(
    pool: &SqlitePool,
    sub: &RecurringSubscriptionRow,
    occurrence: NaiveDate,
    today: NaiveDate,
    action: &str,
) -> Result<String> {
    let vendor_name = match sub.vendor_contact_id.as_deref() {
        Some(cid) => contacts::get(pool, cid)
            .await?
            .map(|c| c.name)
            .unwrap_or_else(|| sub.label.clone()),
        None => sub.label.clone(),
    };

    let input = ExpenseInput {
        expense_date: occurrence,
        paid_date: None, // prüfungssicher: Zahlung wird vom Nutzer bestätigt
        paid_from_account_id: None,
        vendor_contact_id: sub.vendor_contact_id.clone(),
        vendor_name,
        vendor_invoice_number: None,
        category: sub.category.clone(),
        description: sub.description_template.clone(),
        net_amount_cents: sub.expected_amount_cents,
        tax_amount_cents: 0,
        gross_amount_cents: sub.expected_amount_cents,
        currency_code: "EUR".to_string(),
        reverse_charge_13b: sub.reverse_charge_13b_default == 1,
        notes: Some(format!("Automatisch aus Abo „{}“ erzeugt.", sub.label)),
    };

    // Defense-in-depth: Beträge/Datum müssen valide sein (Aufrufer garantiert
    // occurrence <= today; expected_amount > 0 aus der Abo-Validation).
    if let Err(errs) = expense::validate_expense(&input, today) {
        return Err(Error::Domain(format!(
            "Recurring-Auto-Anlage abgebrochen ({}): {}",
            sub.id,
            errs.iter()
                .map(expense::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    let fiscal_year = occurrence.year() as i64;

    // R2-005 (GJ-Guard): Catch-up darf nicht in ein festgeschriebenes GJ
    // materialisieren. Bei retroaktivem `next_due_date` über eine GJ-Grenze
    // hinweg würde sonst der Lock-Snapshot stillschweigend verbiegen.
    guard::ensure_year_open(pool, fiscal_year).await?;

    let expense_number = numbering::next_number(pool, DocType::Expense, fiscal_year as i32).await?;

    let row = expenses::create(pool, &input, &expense_number, fiscal_year, None).await?;
    expenses::set_recurring_subscription_id(pool, &row.id, &sub.id).await?;

    audit_log::append(
        pool,
        "expense.create",
        "expense",
        &row.id,
        Some(&format!(
            r#"{{"number":"{}","gross":{},"category":"{}","source":"recurring","subscription":"{}"}}"#,
            esc(&expense_number),
            row.gross_amount_cents,
            esc(&row.category),
            esc(&sub.id)
        )),
    )
    .await?;

    audit_log::append(
        pool,
        action,
        "recurring",
        &sub.id,
        Some(&format!(
            r#"{{"label":"{}","occurrence":"{}","expense_id":"{}","expense_number":"{}"}}"#,
            esc(&sub.label),
            occurrence,
            esc(&row.id),
            esc(&expense_number)
        )),
    )
    .await?;

    Ok(row.id)
}

fn freq_and_day(sub: &RecurringSubscriptionRow) -> Result<(Frequency, u32)> {
    let freq = Frequency::from_db(&sub.frequency).ok_or_else(|| {
        Error::Domain(format!(
            "Abo {} hat ungültige Frequenz '{}'",
            sub.id, sub.frequency
        ))
    })?;
    let day = sub.day_of_period.clamp(1, 31) as u32;
    Ok((freq, day))
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("Ungültiges Datum '{s}': {e}")))
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
