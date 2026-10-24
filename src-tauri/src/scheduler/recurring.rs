//! Kosten-Abos am Stichtag buchen (Imperative Shell) — Block 10, Dauerbetrieb
//! seit Lauf F (ADR 0040).
//!
//! Erzeugt für fällige Abos (`auto_create_expense=1`) Kosten-Positionen. Die
//! pure Stichtags-Mathematik liegt in [`crate::domain::recurring`]; hier nur
//! die DB-/Backup-Orchestrierung.
//!
//! ## Verhalten
//!
//! - **Sperre, Compare-and-Swap, Transaktion (W14-022):** Tick, „Alle fälligen
//!   buchen“, „Buchen“, Bearbeiten und Pausieren laufen je Abo unter
//!   [`crate::locks::RECURRING_SUBSCRIPTIONS`]. Nummer, Kosten-Position (mit
//!   Abo-Bezug), beide Audit-Einträge und das Vorrücken des Stichtags (nur wenn
//!   er noch auf dem gebuchten Termin steht) committen in **einer**
//!   Transaktion — ein Abbruch dazwischen hinterlässt weder Beleg noch
//!   verbrauchte Nummer, und kein Termin wird doppelt gebucht (W14-025).
//! - **Fehler je Abo:** Ein fehlerhaftes Abo bricht den Lauf nicht ab; es wird
//!   protokolliert, bekommt einen Hinweis, die übrigen Abos laufen weiter. Das
//!   Backup für bereits gebuchte Belege entsteht trotzdem.
//! - **Nachholen (W14-026):** bis [`catch_up::AUTO_LIMIT`] fällige Termine je Abo
//!   bucht der Lauf selbst (je Termin eine Position mit dem Stichtag als
//!   Belegdatum, Sammelhinweis), darüber nur nach Bestätigung
//!   (siehe [`catch_up`]); die Bestätigung deckt nur Termine bis zu ihrem Tag,
//!   und nach einer Wiederherstellung ohne bekannten Stand wartet das Abo auf
//!   die Entscheidung (F1d).
//! - **Abgeschlossenes Geschäftsjahr (W14-034):** liegt der nächste Termin in
//!   einem abgeschlossenen Jahr, entsteht ein Hinweis statt eines Fehlers bei
//!   jedem Tick; nach Bestätigung („nachholen“) werden solche Termine ohne Beleg
//!   übersprungen (Audit), offene Jahre gebucht.
//! - **Pause:** Bei einer Pause ([`hold`], z. B. Uhrsprung) bucht weder der Tick
//!   noch ein manueller Auslöser.
//! - **Zahlstatus:** Auto-angelegte Kosten entstehen `paid_date = NULL`
//!   („noch nicht bezahlt“); erst die bestätigte Zahlung setzt das
//!   Cash-Basis-Datum (EÜR, ADR 0023).
//! - **Unlock-Gate + ein Backup pro Lauf** (Backup-Hardline).
//!
//! ## Beträge
//!
//! Das Abo kennt nur einen erwarteten **Brutto**-Betrag (Eingangsseite; §19
//! betrifft nur Ausgangsbelege). Die erzeugte Position setzt `net = brutto`,
//! `tax = 0`. Weicht der echte Beleg ab, wird per Storno + Neuerfassung
//! korrigiert (Kosten sind sofort gelockt).

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

use crate::db::models::{ExpenseRow, RecurringSubscriptionRow};
use crate::db::numbering;
use crate::db::repo::{audit_log, contacts, expenses, recurring};
use crate::domain::expense::{self, ExpenseInput};
use crate::domain::numbering::DocType;
use crate::domain::recurring::{
    compute_next_due_date, count_due_occurrences, first_occurrence_on_or_after, Frequency,
};
use crate::error::{Error, Result};
use crate::fiscal_year::guard;
use crate::notify::{self, NewNotification};
use crate::scheduler::catch_up::{self, Plan, ResumeMode, Series};
use crate::scheduler::{hold, JobCtx};

/// Ergebnis eines Due-Laufs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessReport {
    /// Lauf übersprungen, weil die Backup-Session gesperrt war.
    pub skipped_locked: bool,
    /// Lauf übersprungen wegen einer Pause (Grund), siehe [`hold`].
    pub skipped_hold: Option<String>,
    /// Anzahl Abos, für die mindestens eine Position erzeugt wurde.
    pub processed_subscriptions: usize,
    /// Gesamtzahl erzeugter Kosten-Positionen.
    pub created_expenses: usize,
    /// Abos, deren Lauf mit Fehler endete (Hinweis angelegt).
    pub failed_subscriptions: usize,
    /// Abos mit Rückstand über der Grenze — warten auf die Entscheidung.
    pub awaiting_decision: usize,
}

/// Verarbeitet alle fälligen Auto-Abos zum heutigen Tag der injizierten Uhr.
pub async fn process_due(job: &JobCtx<'_>) -> Result<ProcessReport> {
    if !job.session.is_unlocked() {
        tracing::debug!("Abo-Kosten übersprungen: Backup-Session gesperrt");
        return Ok(ProcessReport {
            skipped_locked: true,
            ..Default::default()
        });
    }
    if let Some(h) = hold::gate(job.pool, job.now()).await? {
        return Ok(ProcessReport {
            skipped_hold: Some(h.reason),
            ..Default::default()
        });
    }

    let today = job.today();
    let due = recurring::list_due_auto(job.pool, &today.to_string()).await?;
    let mut report = ProcessReport::default();

    for listed in &due {
        if job.stop_requested() {
            tracing::info!("Abo-Kosten: Lauf auf Anforderung beendet");
            break;
        }
        match process_subscription(job, &listed.id, today).await {
            Ok(SubOutcome::Booked(0)) => {}
            Ok(SubOutcome::Booked(n)) => {
                report.processed_subscriptions += 1;
                report.created_expenses += n;
            }
            Ok(SubOutcome::AwaitingDecision) => report.awaiting_decision += 1,
            // F1d: Hinweis und Dedup nennen den Termin, der scheiterte — nicht
            // den ersten des Laufs.
            Ok(SubOutcome::Failed { booked, at, error }) => {
                if booked > 0 {
                    report.processed_subscriptions += 1;
                    report.created_expenses += booked;
                }
                report.failed_subscriptions += 1;
                failure_notice(job, listed, &at, &error).await;
            }
            Err(e) => {
                report.failed_subscriptions += 1;
                failure_notice(job, listed, &listed.next_due_date, &e).await;
            }
        }
    }

    // Ein Backup deckt alle in diesem Lauf gebuchten Belege ab — auch wenn
    // ein anderes Abo scheiterte (Backup-Hardline); im Tick legt es `run_jobs`
    // am Ende an (D-19).
    if report.created_expenses > 0 {
        job.lock_backup("recurring.lock").await;
    }
    Ok(report)
}

/// Manueller Einzel-Trigger („Buchen“ in der Abo-Liste) für ein fälliges Abo —
/// unabhängig vom `auto_create_expense`-Flag. Bucht **genau einen** Termin
/// (den ältesten fälligen) und rückt das Abo um eine Periode vor.
pub async fn run_now(job: &JobCtx<'_>, id: &str) -> Result<ExpenseRow> {
    if let Some(h) = hold::gate(job.pool, job.now()).await? {
        return Err(hold::blocked_error(&h));
    }
    let today = job.today();
    let expense_id = {
        let _guard = crate::locks::RECURRING_SUBSCRIPTIONS.lock(id).await;
        let sub = recurring::get(job.pool, id)
            .await?
            .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))?;
        if sub.active == 0 {
            return Err(Error::Domain(
                "Pausiertes Abo: bitte erst fortsetzen, dann erfassen.".into(),
            ));
        }
        let occurrence = parse_date(&sub.next_due_date)?;
        if occurrence > today {
            return Err(Error::Domain(format!(
                "Abo ist erst am {occurrence} fällig — eine vorzeitige Erfassung würde ein zukünftiges Beleg-Datum erzeugen."
            )));
        }
        let expense_id = book_occurrence(job, &sub, occurrence, today, "recurring.run_now").await?;
        // Eine Bestätigung bzw. Prüfung nach einem Restore ist erledigt, sobald
        // der nächste Termin danach liegt (F1d).
        if let Some(fresh) = recurring::get(job.pool, id).await? {
            catch_up::clear_if_done(job.pool, Series::Subscription, id, &fresh.next_due_date)
                .await?;
        }
        expense_id
    };

    job.lock_backup("recurring.lock").await;

    expenses::get(job.pool, &expense_id)
        .await?
        .ok_or_else(|| Error::Domain("run_now: post-create SELECT leer".into()))
}

/// Wendet die Wahl des Inhabers auf ein Abo mit Rückstand an (Fortsetzen nach
/// Pause oder Entscheidung aus dem Hinweis). Unter der Abo-Sperre aufrufen.
///
/// - [`ResumeMode::Skip`]: Stichtag auf den ersten Termin ab heute; die
///   ausgelassenen Termine stehen im Audit (`recurring.backlog_skipped`). Ist
///   nach einer Wiederherstellung die Prüfung offen, wird auch der heutige
///   Termin ausgelassen (er kann vor dem Ausfall schon gebucht worden sein,
///   F1d).
/// - [`ResumeMode::CatchUp`]: Bestätigung für den nächsten Lauf (höchstens
///   [`catch_up::CONFIRMED_CAP`] Termine), Audit `recurring.backlog_confirmed`.
pub async fn apply_resume_mode(
    pool: &sqlx::SqlitePool,
    sub: &RecurringSubscriptionRow,
    mode: ResumeMode,
    today: NaiveDate,
) -> Result<()> {
    let (freq, day) = freq_and_day(sub)?;
    let next = parse_date(&sub.next_due_date)?;
    let review =
        catch_up::restore_review_through(pool, Series::Subscription, &sub.id, &sub.next_due_date)
            .await?
            .is_some();
    match mode {
        ResumeMode::Skip => {
            let from = if review {
                today.succ_opt().unwrap_or(today)
            } else {
                today
            };
            let (target, skipped) = first_occurrence_on_or_after(freq, day, next, from);
            catch_up::clear(pool, Series::Subscription, &sub.id).await?;
            catch_up::clear_restore_review(pool, Series::Subscription, &sub.id).await?;
            if skipped == 0 {
                return Ok(());
            }
            let mut tx = crate::db::begin_immediate(pool).await?;
            recurring::advance_in(
                &mut tx,
                &sub.id,
                &sub.next_due_date,
                None,
                &target.to_string(),
            )
            .await?;
            audit_log::append_in(
                &mut tx,
                "recurring.backlog_skipped",
                "recurring",
                &sub.id,
                Some(
                    &serde_json::json!({
                        "from": sub.next_due_date,
                        "to": target.to_string(),
                        "skipped": skipped,
                        "after_restore": review,
                    })
                    .to_string(),
                ),
            )
            .await?;
            tx.commit().await?;
        }
        ResumeMode::CatchUp => {
            let due = count_due_occurrences(freq, day, next, today, None, catch_up::COUNT_CAP);
            if due > catch_up::CONFIRMED_CAP {
                return Err(Error::Domain(format!(
                    "{due} offene Termine sind zu viele zum Nachholen (höchstens {}). Bitte „ab heute \
                     fortsetzen“ wählen oder den nächsten Termin im Abo anpassen.",
                    catch_up::CONFIRMED_CAP
                )));
            }
            if due == 0 {
                return Ok(());
            }
            catch_up::confirm(pool, Series::Subscription, &sub.id, today).await?;
            audit_log::append(
                pool,
                "recurring.backlog_confirmed",
                "recurring",
                &sub.id,
                Some(&serde_json::json!({ "from": sub.next_due_date, "due": due }).to_string()),
            )
            .await?;
        }
    }
    Ok(())
}

// ---- intern ----------------------------------------------------------------

enum SubOutcome {
    Booked(usize),
    AwaitingDecision,
    /// Fehler beim Termin `at`, nachdem `booked` Termine gebucht waren.
    Failed {
        booked: usize,
        at: String,
        error: Error,
    },
}

/// Bucht die fälligen Termine eines Abos unter seiner Sperre. Liest die Zeile
/// darunter frisch (ein paralleles „Buchen“, Bearbeiten oder Pausieren
/// kann sie verändert haben).
async fn process_subscription(job: &JobCtx<'_>, id: &str, today: NaiveDate) -> Result<SubOutcome> {
    let _guard = crate::locks::RECURRING_SUBSCRIPTIONS.lock(id).await;
    let Some(sub) = recurring::get(job.pool, id).await? else {
        return Ok(SubOutcome::Booked(0));
    };
    if sub.active == 0 || sub.auto_create_expense == 0 {
        return Ok(SubOutcome::Booked(0));
    }
    let (freq, day) = freq_and_day(&sub)?;
    let first = parse_date(&sub.next_due_date)?;
    let due = count_due_occurrences(freq, day, first, today, None, catch_up::COUNT_CAP);
    // F1d: Die Bestätigung deckt nur Termine bis zu ihrem Tag; die Prüfung
    // nach einer Wiederherstellung verlangt sonst immer die Entscheidung.
    let confirmed_through =
        catch_up::confirmed_through(job.pool, Series::Subscription, id, &sub.next_due_date).await?;
    let within = confirmed_through.map(|through| {
        count_due_occurrences(
            freq,
            day,
            first,
            through.min(today),
            None,
            catch_up::COUNT_CAP,
        )
    });
    let review =
        catch_up::restore_review_through(job.pool, Series::Subscription, id, &sub.next_due_date)
            .await?
            .is_some();
    let limit = match catch_up::plan_for(due, within, review) {
        Plan::Nothing => return Ok(SubOutcome::Booked(0)),
        Plan::AwaitDecision { due } => {
            decision_notice(job, &sub, due, review).await;
            return Ok(SubOutcome::AwaitingDecision);
        }
        Plan::Run { n } => n,
    };

    let mut booked = 0usize;
    let mut first_booked: Option<NaiveDate> = None;
    let mut last_booked: Option<NaiveDate> = None;
    let mut sub = sub;
    let mut skipped = 0usize;
    while booked < limit && skipped <= catch_up::CONFIRMED_CAP {
        if booked > 0 {
            if job.stop_requested() {
                break;
            }
            match recurring::get(job.pool, id).await? {
                Some(fresh) if fresh.active != 0 => sub = fresh,
                _ => break,
            }
        }
        let occurrence = parse_date(&sub.next_due_date)?;
        if occurrence > today {
            break;
        }
        if guard::is_closed(job.pool, occurrence.year() as i64).await? {
            if confirmed_through.is_some_and(|through| occurrence <= through) {
                skip_closed_year_period(job, &sub, occurrence).await?;
                skipped += 1;
                // Zeile neu lesen (Stichtag vorgerückt), Termin zählt nicht.
                match recurring::get(job.pool, id).await? {
                    Some(fresh) => {
                        sub = fresh;
                        continue;
                    }
                    None => break,
                }
            }
            closed_year_notice(job, &sub, occurrence).await;
            break;
        }
        match book_occurrence(job, &sub, occurrence, today, "recurring.auto_created").await {
            Ok(_) => {
                booked += 1;
                first_booked.get_or_insert(occurrence);
                last_booked = Some(occurrence);
            }
            Err(e) => {
                if let (true, Some(from), Some(to)) = (booked > 1, first_booked, last_booked) {
                    catch_up_notice(job, &sub, booked, from, to).await;
                }
                return Ok(SubOutcome::Failed {
                    booked,
                    at: sub.next_due_date.clone(),
                    error: e,
                });
            }
        }
    }
    if let Some(fresh) = recurring::get(job.pool, id).await? {
        catch_up::clear_if_done(job.pool, Series::Subscription, id, &fresh.next_due_date).await?;
    }
    if booked > 1 {
        if let (Some(from), Some(to)) = (first_booked, last_booked) {
            catch_up_notice(job, &sub, booked, from, to).await;
        }
    }
    Ok(SubOutcome::Booked(booked))
}

/// Bucht einen Termin: Nummer, Kosten-Position (sofort festgeschrieben, mit
/// Abo-Bezug), Audits und Vorrücken des Stichtags (Compare-and-Swap) in einer
/// Transaktion. Liefert die Expense-ID. Unter der Abo-Sperre aufrufen.
async fn book_occurrence(
    job: &JobCtx<'_>,
    sub: &RecurringSubscriptionRow,
    occurrence: NaiveDate,
    today: NaiveDate,
    action: &str,
) -> Result<String> {
    let pool = job.pool;
    let (freq, day) = freq_and_day(sub)?;
    let following = compute_next_due_date(freq, day, occurrence);
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

    // Defense-in-depth: Beträge/Datum müssen valide sein (occurrence <= today;
    // expected_amount > 0 aus der Abo-Validation).
    if let Err(errs) = expense::validate_expense(&input, today) {
        return Err(Error::Domain(format!(
            "Abo-Kosten nicht gebucht ({}): {}",
            sub.label,
            errs.iter()
                .map(expense::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    let fiscal_year = occurrence.year() as i64;
    // R2-005 (GJ-Guard): nie in ein festgeschriebenes GJ buchen.
    guard::ensure_year_open(pool, fiscal_year).await?;

    let mut tx = crate::db::begin_immediate(pool).await?;
    let expense_number =
        numbering::next_number_in(&mut tx, DocType::Expense, fiscal_year as i32).await?;
    let expense_id = expenses::insert_in(
        &mut tx,
        &input,
        &expense_number,
        fiscal_year,
        None,
        Some(&sub.id),
    )
    .await?;
    audit_log::append_in(
        &mut tx,
        "expense.create",
        "expense",
        &expense_id,
        Some(
            &serde_json::json!({
                "number": expense_number,
                "gross": input.gross_amount_cents,
                "category": input.category,
                "source": "recurring",
                "subscription": sub.id,
            })
            .to_string(),
        ),
    )
    .await?;
    audit_log::append_in(
        &mut tx,
        action,
        "recurring",
        &sub.id,
        Some(
            &serde_json::json!({
                "label": sub.label,
                "occurrence": occurrence.to_string(),
                "expense_id": expense_id,
                "expense_number": expense_number,
            })
            .to_string(),
        ),
    )
    .await?;
    recurring::advance_in(
        &mut tx,
        &sub.id,
        &occurrence.to_string(),
        Some(&expense_id),
        &following.to_string(),
    )
    .await?;
    tx.commit().await?;
    Ok(expense_id)
}

/// Termin im abgeschlossenen Geschäftsjahr nach Bestätigung ohne Beleg
/// überspringen (Stichtag vorrücken, Audit).
async fn skip_closed_year_period(
    job: &JobCtx<'_>,
    sub: &RecurringSubscriptionRow,
    occurrence: NaiveDate,
) -> Result<()> {
    let (freq, day) = freq_and_day(sub)?;
    let following = compute_next_due_date(freq, day, occurrence);
    let mut tx = crate::db::begin_immediate(job.pool).await?;
    recurring::advance_in(
        &mut tx,
        &sub.id,
        &occurrence.to_string(),
        None,
        &following.to_string(),
    )
    .await?;
    audit_log::append_in(
        &mut tx,
        "recurring.period_skipped",
        "recurring",
        &sub.id,
        Some(
            &serde_json::json!({
                "occurrence": occurrence.to_string(),
                "reason": "fiscal_year_closed",
                "fiscal_year": occurrence.year(),
            })
            .to_string(),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn notice(
    job: &JobCtx<'_>,
    title: &str,
    body: &str,
    severity: &str,
    sub_id: &str,
    dedup: &str,
) {
    let n = NewNotification {
        title,
        body,
        severity,
        related_entity_type: Some("recurring"),
        related_entity_id: Some(sub_id),
        action_url: Some("/expenses/recurring"),
        dedup_key: Some(dedup),
        ..Default::default()
    };
    if let Err(e) = notify::emit(job.pool, n).await {
        tracing::warn!("Abo-Hinweis nicht angelegt: {e}");
    }
}

/// Hinweis zu einem gescheiterten Termin `at` (F1d: der gescheiterte, nicht
/// der erste des Laufs).
async fn failure_notice(job: &JobCtx<'_>, sub: &RecurringSubscriptionRow, at: &str, e: &Error) {
    tracing::warn!(
        "Abo-Kosten für „{}“ ({}, Termin {at}) fehlgeschlagen: {e}",
        sub.label,
        sub.id
    );
    let body = format!(
        "„{}“: Der fällige Termin {} konnte nicht gebucht werden ({e}). Die übrigen Abos \
         liefen weiter. Bitte das Abo prüfen; der nächste Lauf versucht es erneut.",
        sub.label,
        de_date(at)
    );
    let dedup = format!("recurring_failed:{}:{at}", sub.id);
    notice(
        job,
        "Abo-Kosten: Buchung fehlgeschlagen",
        &body,
        "warning",
        &sub.id,
        &dedup,
    )
    .await;
}

async fn decision_notice(
    job: &JobCtx<'_>,
    sub: &RecurringSubscriptionRow,
    due: usize,
    after_restore: bool,
) {
    let body = if after_restore {
        format!(
            "„{}“: Nach der Wiederherstellung {} seit {} offen. Ob vor der Wiederherstellung \
             schon gebucht wurde, weiß Klein.Buch nicht (kein Stand der ersetzten Datenbank). \
             Bitte unter Kosten → Wiederkehrende Abos beim Abo „Rückstand …“ wählen: nachholen \
             oder ab heute fortsetzen (lässt auch einen heute fälligen Termin aus).",
            sub.label,
            if due == 1 {
                "ist ein Termin".to_string()
            } else {
                format!("sind {due} Termine")
            },
            de_date(&sub.next_due_date),
        )
    } else {
        format!(
            "„{}“: {due} Termine seit {} sind offen — mehr als Klein.Buch ohne Rückfrage \
             nachbucht ({}). Bitte unter Kosten → Wiederkehrende Abos beim Abo „Rückstand …“ \
             wählen: nachholen (höchstens {}) oder ab heute fortsetzen.",
            sub.label,
            de_date(&sub.next_due_date),
            catch_up::AUTO_LIMIT,
            catch_up::CONFIRMED_CAP
        )
    };
    let dedup = format!("recurring_backlog:{}:{}", sub.id, sub.next_due_date);
    notice(
        job,
        "Abo-Kosten: Entscheidung nötig",
        &body,
        "warning",
        &sub.id,
        &dedup,
    )
    .await;
}

async fn closed_year_notice(
    job: &JobCtx<'_>,
    sub: &RecurringSubscriptionRow,
    occurrence: NaiveDate,
) {
    let body = format!(
        "„{}“: Der Termin {} liegt im bereits abgeschlossenen Geschäftsjahr {}; dort kann \
         nichts mehr gebucht werden. Bitte beim Abo „Rückstand …“ wählen: „nachholen“ \
         überspringt Termine in abgeschlossenen Jahren und bucht die offenen, „ab heute \
         fortsetzen“ setzt beim nächsten Termin ab heute fort.",
        sub.label,
        occurrence.format("%d.%m.%Y"),
        occurrence.year()
    );
    let dedup = format!("recurring_closed_year:{}:{}", sub.id, occurrence);
    notice(
        job,
        "Abo-Kosten: Termin im abgeschlossenen Jahr",
        &body,
        "warning",
        &sub.id,
        &dedup,
    )
    .await;
}

async fn catch_up_notice(
    job: &JobCtx<'_>,
    sub: &RecurringSubscriptionRow,
    booked: usize,
    from: NaiveDate,
    to: NaiveDate,
) {
    let body = format!(
        "„{}“: {booked} Termine vom {} bis {} wurden nachgebucht (je Termin eine Kosten-Position \
         mit dem Stichtag als Belegdatum, noch nicht bezahlt). Bitte prüfen.",
        sub.label,
        from.format("%d.%m.%Y"),
        to.format("%d.%m.%Y")
    );
    let dedup = format!("recurring_catch_up:{}:{}", sub.id, from);
    notice(
        job,
        "Abo-Kosten nachgebucht",
        &body,
        "info",
        &sub.id,
        &dedup,
    )
    .await;
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

fn de_date(iso: &str) -> String {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(|d| d.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| iso.to_string())
}
