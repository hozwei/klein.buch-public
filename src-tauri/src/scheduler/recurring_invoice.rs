//! Abo-Rechnungen am Stichtag materialisieren (Imperative Shell) — Block RI-2,
//! Dauerbetrieb seit Lauf F (ADR 0040).
//!
//! Erzeugt für fällige Abo-Vorlagen ([`crate::db::repo::recurring_invoice`])
//! echte Ausgangsrechnungen über die **bestehende** Pipeline
//! ([`prepare_invoice_draft`]/[`insert_prepared_draft`] + Lock-Pipeline).
//! §19-Klausel, Pflichtangaben-Check, XRechnung, PDF, KoSIT, Archiv und Lock
//! kommen dadurch 1:1 aus dem normalen Rechnungsweg — hier nur die
//! Orchestrierung.
//!
//! ## Verhalten
//!
//! - **Belegdatum = Erstellungstag (heute), nie rückdatiert** (§14 Abs. 4 Nr. 3
//!   UStG + GoBD). Der Leistungszeitraum der Periode geht ins `delivery_date`
//!   und — bei `service_period_note` — als Klartext in die erste Position.
//! - **Absturzfest (W14-025):** Nummer, Entwurf, Audit `recurring_invoice.run`
//!   und das Vorrücken der Vorlage (Compare-and-Swap) committen in **einer**
//!   Transaktion. Festschreiben und Versand folgen danach; bis sie erledigt
//!   sind, steht ein Fortsetzungs-Marker (`recurring_invoice_pending:<id>`) in
//!   derselben Transaktion. Bricht der Prozess dazwischen ab, setzt der nächste
//!   Lauf genau dort fort (festschreiben, versenden, falls noch nicht
//!   versendet) — es entsteht nie eine zweite Rechnung für denselben Termin.
//! - **Nachholen (W14-026, [`catch_up`]):** bis [`catch_up::AUTO_LIMIT`]
//!   fällige Termine holt der Lauf selbst nach; nur der jüngste folgt der
//!   Automatik-Stufe, ältere entstehen als Entwurf (nie automatischer Versand
//!   nachgeholter Termine), dazu ein Sammelhinweis. Mehr Termine nur nach
//!   Bestätigung des Inhabers; sie deckt nur Termine bis zu ihrem Tag (F1d).
//!   Nach einer Wiederherstellung ohne bekannten Stand der Vorlage entstehen
//!   Termine bis zum Restore-Tag nur nach Entscheidung und nur als Entwurf.
//! - **Sperre je Vorlage (W1.4, W14-031):** Tick, „Jetzt erstellen“,
//!   Bearbeiten und Pausieren laufen unter [`crate::locks::RECURRING_TEMPLATES`];
//!   die Vorlage wird je Termin frisch gelesen (Pausieren wirkt ab dem nächsten
//!   Termin).
//! - **Überspringen mit Hinweis:** Vorlagen mit archiviertem oder
//!   anonymisiertem Kunden, ohne Positionen oder mit USt-Ausweis trotz §19
//!   (R1-017) erzeugen keinen (nummerierten) Entwurf, sondern einen Hinweis.
//! - **Automatik-Stufe je Vorlage** ([`AutoMode`]): `draft` → Entwurf + Hinweis;
//!   `issue` → festschreiben; `issue_send` → festschreiben + Versand über das
//!   Standard-Mail-Konto (best-effort; Fehler → Hinweis, Beleg bleibt
//!   festgeschrieben).
//! - **Pause ([`hold`]), Unlock-Gate, ein Backup pro Lauf.**

use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::commands::invoices::{
    insert_prepared_draft, prepare_invoice_draft, run_lock_pipeline_on,
};
use crate::commands::mail::send_invoice_core;
use crate::db::models::{RecurringInvoiceItemRow, RecurringInvoiceRow};
use crate::db::repo::{
    app_settings, audit_log, contacts, invoices, mail_accounts, recurring_invoice,
};
use crate::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use crate::domain::recurring::{
    compute_next_due_date, count_due_occurrences, first_occurrence_on_or_after, Frequency,
};
use crate::domain::recurring_invoice::AutoMode;
use crate::error::{Error, Result};
use crate::fiscal_year::guard;
use crate::notify::{self, NewNotification};
use crate::scheduler::catch_up::{self, Plan, ResumeMode, Series};
use crate::scheduler::{hold, JobCtx};

/// Präfix der Fortsetzungs-Marker (`<präfix><vorlage>` = `rechnung|termin|stufe`).
pub const PENDING_PREFIX: &str = "recurring_invoice_pending:";

/// Deutsche Monatsnamen (Index 1..=12) für die Leistungszeitraum-Beschriftung.
const MONTHS_DE: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

/// Ergebnis eines Due-Laufs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessReport {
    /// Lauf übersprungen, weil die Backup-Session gesperrt war.
    pub skipped_locked: bool,
    /// Lauf übersprungen wegen einer Pause (Grund), siehe [`hold`].
    pub skipped_hold: Option<String>,
    /// Anzahl Vorlagen, für die mindestens eine Rechnung erzeugt wurde.
    pub processed_templates: usize,
    /// Gesamtzahl erzeugter Rechnungen (Entwurf oder festgeschrieben).
    pub created_invoices: usize,
    /// Vorlagen, deren Lauf mit Fehler endete oder die übersprungen wurden
    /// (Hinweis angelegt).
    pub failed_templates: usize,
    /// Vorlagen mit Rückstand über der Grenze — warten auf die Entscheidung.
    pub awaiting_decision: usize,
    /// Nach einem Abbruch fortgesetzte Termine (festschreiben/versenden).
    pub resumed: usize,
}

/// Verarbeitet alle fälligen Abo-Vorlagen zum heutigen Tag der injizierten Uhr.
pub async fn process_due(job: &JobCtx<'_>) -> Result<ProcessReport> {
    if !job.session.is_unlocked() {
        tracing::debug!("Abo-Rechnungen übersprungen: Backup-Session gesperrt");
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

    let mut report = ProcessReport {
        resumed: resume_pending(job).await?,
        ..Default::default()
    };

    let today = job.today();
    let due = recurring_invoice::list_due(job.pool, &today.to_string()).await?;
    for listed in &due {
        if job.stop_requested() {
            tracing::info!("Abo-Rechnungen: Lauf auf Anforderung beendet");
            break;
        }
        match process_template(job, &listed.id, today).await {
            Ok(TmplOutcome::Created(0)) => {}
            Ok(TmplOutcome::Created(n)) => {
                report.processed_templates += 1;
                report.created_invoices += n;
            }
            Ok(TmplOutcome::AwaitingDecision) => report.awaiting_decision += 1,
            Ok(TmplOutcome::Blocked) => report.failed_templates += 1,
            // F1d: Hinweis und Dedup nennen den Termin, der scheiterte.
            Ok(TmplOutcome::Failed { created, at, error }) => {
                if created > 0 {
                    report.processed_templates += 1;
                    report.created_invoices += created;
                }
                report.failed_templates += 1;
                failure_notice(job, listed, &at, &error).await;
            }
            // Ein Fehler bei einer Vorlage darf die anderen nicht abbrechen.
            Err(e) => {
                report.failed_templates += 1;
                failure_notice(job, listed, &listed.next_due_date, &e).await;
            }
        }
    }

    // Ein Backup deckt den gesamten Lauf ab (Lock-Events); im Tick legt es
    // `run_jobs` am Ende an (D-19).
    if report.created_invoices > 0 || report.resumed > 0 {
        job.lock_backup("recurring_invoice.lock").await;
    }
    Ok(report)
}

/// Manueller Einzel-Trigger („Jetzt erstellen“) für eine fällige Vorlage —
/// legt **genau eine** Rechnung für den ältesten fälligen Termin an (je
/// Automatik-Stufe Entwurf, festgeschrieben bzw. versendet) und rückt die
/// Vorlage um eine Periode vor.
///
/// F1d: Ein nach einem Abbruch begonnener Termin derselben Vorlage
/// (Fortsetzungs-Marker) wird zuerst abgeschlossen — sonst überschriebe der
/// neue Marker ihn und die frühere Rechnung bliebe still als Entwurf liegen.
/// Ist nach einer Wiederherstellung die Prüfung der Vorlage offen, entsteht nur
/// ein Entwurf (kein automatisches Festschreiben oder Versenden).
pub async fn run_now(job: &JobCtx<'_>, id: &str) -> Result<String> {
    if let Some(h) = hold::gate(job.pool, job.now()).await? {
        return Err(hold::blocked_error(&h));
    }
    let today = job.today();
    let invoice_id = {
        // Plan W1.4: nie gleichzeitig mit dem Tick auf derselben Vorlage.
        let _guard = crate::locks::RECURRING_TEMPLATES.lock(id).await;
        let pending_key = format!("{PENDING_PREFIX}{id}");
        if let Some(value) = app_settings::get(job.pool, &pending_key).await? {
            resume_one_locked(job, id, &pending_key, &value).await?;
        }
        let tmpl = recurring_invoice::get_row(job.pool, id)
            .await?
            .ok_or_else(|| Error::Domain(format!("Abo-Vorlage nicht gefunden: {id}")))?;
        if tmpl.active == 0 {
            return Err(Error::Domain(
                "Pausierte Vorlage: bitte erst fortsetzen, dann erstellen.".into(),
            ));
        }
        let items = recurring_invoice::items_for(job.pool, id).await?;
        let occurrence = parse_date(&tmpl.next_due_date)?;
        if let Some(reason) = blocker(job.pool, &tmpl, &items, occurrence).await? {
            return Err(Error::Domain(reason));
        }
        if occurrence > today {
            return Err(Error::Domain(format!(
                "Vorlage ist erst am {occurrence} fällig — eine vorzeitige Erstellung würde ein zukünftiges Periodendatum erzeugen."
            )));
        }
        if end_date(&tmpl).is_some_and(|end| occurrence > end) {
            return Err(Error::Domain(
                "Die Laufzeit der Vorlage ist beendet — es entstehen keine weiteren Rechnungen."
                    .into(),
            ));
        }
        let review =
            catch_up::restore_review_through(job.pool, Series::Template, id, &tmpl.next_due_date)
                .await?
                .is_some();
        let mode = if review {
            AutoMode::Draft
        } else {
            AutoMode::from_db(&tmpl.auto_mode).unwrap_or(AutoMode::Draft)
        };
        let Some((invoice_id, number)) =
            create_occurrence(job, &tmpl, &items, occurrence, today, mode).await?
        else {
            return Err(Error::Domain(
                "Die Vorlage wurde inzwischen pausiert oder ihr Kunde archiviert bzw. \
                 anonymisiert — es wurde keine Rechnung erstellt."
                    .into(),
            ));
        };
        post_process(
            job,
            &tmpl.label,
            &tmpl.id,
            &invoice_id,
            &number,
            occurrence,
            mode,
            true,
        )
        .await;
        if let Some(fresh) = recurring_invoice::get_row(job.pool, id).await? {
            catch_up::clear_if_done(job.pool, Series::Template, id, &fresh.next_due_date).await?;
        }
        invoice_id
    };

    job.lock_backup("recurring_invoice.lock").await;
    Ok(invoice_id)
}

/// Wendet die Wahl des Inhabers auf eine Vorlage mit Rückstand an (Fortsetzen
/// nach Pause oder Entscheidung aus dem Hinweis). Unter der Vorlagen-Sperre
/// aufrufen.
///
/// - [`ResumeMode::Skip`]: nächster Termin = erster Termin ab heute; die
///   ausgelassenen Termine stehen im Audit (`recurring_invoice.backlog_skipped`).
///   Ist nach einer Wiederherstellung die Prüfung offen, wird auch der heutige
///   Termin ausgelassen (er kann vor dem Ausfall schon abgerechnet worden
///   sein, F1d).
/// - [`ResumeMode::CatchUp`]: Bestätigung für den nächsten Lauf (höchstens
///   [`catch_up::CONFIRMED_CAP`] Termine), Audit
///   `recurring_invoice.backlog_confirmed`. Mit offener Restore-Prüfung
///   entstehen die Termine bis zum Tag der Wiederherstellung nur als Entwurf.
pub async fn apply_resume_mode(
    pool: &SqlitePool,
    tmpl: &RecurringInvoiceRow,
    mode: ResumeMode,
    today: NaiveDate,
) -> Result<()> {
    let (freq, day) = freq_and_day(tmpl)?;
    let next = parse_date(&tmpl.next_due_date)?;
    let end = end_date(tmpl);
    let review =
        catch_up::restore_review_through(pool, Series::Template, &tmpl.id, &tmpl.next_due_date)
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
            catch_up::clear(pool, Series::Template, &tmpl.id).await?;
            catch_up::clear_restore_review(pool, Series::Template, &tmpl.id).await?;
            if skipped == 0 {
                return Ok(());
            }
            let mut tx = crate::db::begin_immediate(pool).await?;
            recurring_invoice::advance_in(
                &mut tx,
                &tmpl.id,
                &tmpl.next_due_date,
                None,
                &target.to_string(),
            )
            .await?;
            audit_log::append_in(
                &mut tx,
                "recurring_invoice.backlog_skipped",
                "recurring_invoice",
                &tmpl.id,
                Some(serde_json::json!({
                    "from": tmpl.next_due_date,
                    "to": target.to_string(),
                    "skipped": skipped,
                    "ends": end.is_some_and(|e| target > e),
                    "after_restore": review,
                })),
            )
            .await?;
            tx.commit().await?;
        }
        ResumeMode::CatchUp => {
            let due = count_due_occurrences(freq, day, next, today, end, catch_up::COUNT_CAP);
            if due > catch_up::CONFIRMED_CAP {
                return Err(Error::Domain(format!(
                    "{due} offene Termine sind zu viele zum Nachholen (höchstens {}). Bitte „ab heute \
                     fortsetzen“ wählen und vergangene Zeiträume bei Bedarf einzeln abrechnen.",
                    catch_up::CONFIRMED_CAP
                )));
            }
            if due == 0 {
                return Ok(());
            }
            catch_up::confirm(pool, Series::Template, &tmpl.id, today).await?;
            audit_log::append(
                pool,
                "recurring_invoice.backlog_confirmed",
                "recurring_invoice",
                &tmpl.id,
                Some(serde_json::json!({ "from": tmpl.next_due_date, "due": due })),
            )
            .await?;
        }
    }
    Ok(())
}

// ---- intern ----------------------------------------------------------------

enum TmplOutcome {
    Created(usize),
    AwaitingDecision,
    /// Übersprungen (Kunde archiviert/anonymisiert, keine Positionen, §14c).
    Blocked,
    /// Fehler beim Termin `at`, nachdem `created` Termine erzeugt waren.
    Failed {
        created: usize,
        at: String,
        error: Error,
    },
}

/// Verarbeitet die fälligen Termine einer Vorlage. Die Entscheidung (wie viele
/// Termine) fällt am Anfang; danach wird je Termin die Sperre genommen und die
/// Vorlage frisch gelesen (Pausieren/Bearbeiten wirkt ab dem nächsten Termin).
async fn process_template(job: &JobCtx<'_>, id: &str, today: NaiveDate) -> Result<TmplOutcome> {
    let (limit, review_through) = {
        let _guard = crate::locks::RECURRING_TEMPLATES.lock(id).await;
        let Some(tmpl) = recurring_invoice::get_row(job.pool, id).await? else {
            return Ok(TmplOutcome::Created(0));
        };
        if tmpl.active == 0 {
            return Ok(TmplOutcome::Created(0));
        }
        let (freq, day) = freq_and_day(&tmpl)?;
        let first = parse_date(&tmpl.next_due_date)?;
        let due = count_due_occurrences(
            freq,
            day,
            first,
            today,
            end_date(&tmpl),
            catch_up::COUNT_CAP,
        );
        if due == 0 {
            return Ok(TmplOutcome::Created(0));
        }
        let items = recurring_invoice::items_for(job.pool, id).await?;
        if let Some(reason) = blocker(job.pool, &tmpl, &items, first).await? {
            blocked_notice(job, &tmpl, &reason).await;
            return Ok(TmplOutcome::Blocked);
        }
        // F1d: Die Bestätigung deckt nur Termine bis zu ihrem Tag; die Prüfung
        // nach einer Wiederherstellung verlangt sonst immer die Entscheidung.
        let confirmed_through =
            catch_up::confirmed_through(job.pool, Series::Template, id, &tmpl.next_due_date)
                .await?;
        let within = confirmed_through.map(|through| {
            count_due_occurrences(
                freq,
                day,
                first,
                through.min(today),
                end_date(&tmpl),
                catch_up::COUNT_CAP,
            )
        });
        let review_through =
            catch_up::restore_review_through(job.pool, Series::Template, id, &tmpl.next_due_date)
                .await?;
        match catch_up::plan_for(due, within, review_through.is_some()) {
            Plan::Nothing => return Ok(TmplOutcome::Created(0)),
            Plan::AwaitDecision { due } => {
                decision_notice(job, &tmpl, due, review_through.is_some()).await;
                return Ok(TmplOutcome::AwaitingDecision);
            }
            Plan::Run { n } => (n, review_through),
        }
    };

    let mut created = 0usize;
    let mut older_drafts: Vec<String> = Vec::new();
    let mut label = String::new();
    while created < limit {
        if job.stop_requested() {
            break;
        }
        let _guard = crate::locks::RECURRING_TEMPLATES.lock(id).await;
        let Some(tmpl) = recurring_invoice::get_row(job.pool, id).await? else {
            break;
        };
        if tmpl.active == 0 {
            break;
        }
        label.clone_from(&tmpl.label);
        let items = recurring_invoice::items_for(job.pool, id).await?;
        let (freq, day) = freq_and_day(&tmpl)?;
        let end = end_date(&tmpl);
        let next = parse_date(&tmpl.next_due_date)?;
        if next > today || end.is_some_and(|e| next > e) {
            break;
        }
        // ADR 0042 Nr. 3: §19-Status je Termin — ein nachgeholter Termin nach einem
        // Wechsel erzeugt keine Rechnung mit unpassender Steuerart (§14c-Schutz).
        if let Some(reason) = blocker(job.pool, &tmpl, &items, next).await? {
            blocked_notice(job, &tmpl, &reason).await;
            break;
        }
        let remaining = count_due_occurrences(freq, day, next, today, end, catch_up::COUNT_CAP);
        // Nur der jüngste fällige Termin folgt der Automatik-Stufe; ältere
        // (nachgeholte) Termine entstehen als Entwurf (W14-026). Termine bis
        // zum Tag einer Wiederherstellung ohne bekannten Stand ebenfalls (F1d).
        let latest = remaining <= 1;
        let reviewed = review_through.is_some_and(|day| next <= day);
        let mode = if latest && !reviewed {
            AutoMode::from_db(&tmpl.auto_mode).unwrap_or(AutoMode::Draft)
        } else {
            AutoMode::Draft
        };
        match create_occurrence(job, &tmpl, &items, next, today, mode).await {
            Ok(Some((invoice_id, number))) => {
                created += 1;
                post_process(
                    job,
                    &tmpl.label,
                    &tmpl.id,
                    &invoice_id,
                    &number,
                    next,
                    mode,
                    latest && !reviewed,
                )
                .await;
                if !latest || reviewed {
                    older_drafts.push(number);
                }
            }
            // Vorlage inzwischen pausiert oder Kunde anonymisiert/archiviert:
            // kein Beleg, keine Nummer (F1d).
            Ok(None) => break,
            Err(e) => {
                if !older_drafts.is_empty() {
                    catch_up_notice(job, id, &label, &older_drafts).await;
                }
                return Ok(TmplOutcome::Failed {
                    created,
                    at: tmpl.next_due_date.clone(),
                    error: e,
                });
            }
        }
    }
    if let Some(fresh) = recurring_invoice::get_row(job.pool, id).await? {
        catch_up::clear_if_done(job.pool, Series::Template, id, &fresh.next_due_date).await?;
    }
    if !older_drafts.is_empty() {
        catch_up_notice(job, id, &label, &older_drafts).await;
    }
    Ok(TmplOutcome::Created(created))
}

/// Gründe, aus denen eine Vorlage keine Rechnung erzeugen darf (dann kein
/// nummerierter Entwurf, sondern ein Hinweis). `None` = alles in Ordnung.
async fn blocker(
    pool: &SqlitePool,
    tmpl: &RecurringInvoiceRow,
    items: &[RecurringInvoiceItemRow],
    occurrence: NaiveDate,
) -> Result<Option<String>> {
    match contacts::get(pool, &tmpl.contact_id).await? {
        None => return Ok(Some("Der Kunde der Vorlage existiert nicht mehr.".into())),
        Some(c) if c.anonymized_at.is_some() => {
            return Ok(Some(
                "Der Kunde der Vorlage wurde anonymisiert (DSGVO) — an ihn werden keine \
                 Rechnungen mehr erstellt. Bitte die Vorlage pausieren oder einen anderen Kunden \
                 wählen."
                    .into(),
            ))
        }
        Some(c) if c.archived != 0 => {
            return Ok(Some(
                "Der Kunde der Vorlage ist archiviert. Bitte den Kunden reaktivieren oder die \
                 Vorlage pausieren."
                    .into(),
            ))
        }
        Some(_) => {}
    }
    if items.is_empty() {
        return Ok(Some("Die Vorlage hat keine Positionen.".into()));
    }
    // R1-017: §14c-Schutz auch im Scheduler — keine Entwürfe mit USt-Ausweis
    // bei aktivem §19 (sonst nummerierte Entwürfe, die erst beim Festschreiben
    // scheitern). Status am Termin (Leistungsdatum, ADR 0042 Nr. 3).
    let inputs: Vec<InvoiceItemInput> = items.iter().map(row_to_item_input).collect();
    if let Err(e) =
        crate::commands::recurring_invoice::check_items_no_vat(pool, &inputs, occurrence).await
    {
        return Ok(Some(e.to_string()));
    }
    Ok(None)
}

/// Legt für einen Termin die Rechnung als Entwurf an und rückt die Vorlage
/// vor — Nummer, Entwurf, Audits, Vorrücken (Compare-and-Swap) und ggf. der
/// Fortsetzungs-Marker in **einer** Transaktion. Liefert `Some((id, nummer))`.
///
/// F1d: Innerhalb der Transaktion wird erneut geprüft, dass die Vorlage aktiv
/// und ihr Kunde weder archiviert noch anonymisiert ist (die Käuferdaten liest
/// [`prepare_invoice_draft`] vorher; eine dazwischen abgeschlossene
/// Anonymisierung pausiert die Vorlage). Sonst `None` — ohne Beleg und ohne
/// verbrauchte Nummer. Ein noch offener Fortsetzungs-Marker der Vorlage wird
/// nie überschrieben (Fehler; der nächste Lauf setzt ihn zuerst fort).
async fn create_occurrence(
    job: &JobCtx<'_>,
    tmpl: &RecurringInvoiceRow,
    items: &[RecurringInvoiceItemRow],
    occurrence: NaiveDate,
    today: NaiveDate,
    mode: AutoMode,
) -> Result<Option<(String, String)>> {
    let (freq, day) = freq_and_day(tmpl)?;
    let following = compute_next_due_date(freq, day, occurrence);
    // GJ leitet der Draft-Weg aus dem Rechnungsdatum ab (Plan W1.2/W9).
    let (input, _fiscal_year) = build_invoice_input(tmpl, items, occurrence, today);
    let prepared = prepare_invoice_draft(job.pool, &tmpl.contact_id, &input, None, today).await?;

    let mut tx = crate::db::begin_immediate(job.pool).await?;
    let still_valid: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM recurring_invoices r JOIN contacts c ON c.id = r.contact_id
          WHERE r.id = ? AND r.active = 1 AND c.archived = 0 AND c.anonymized_at IS NULL",
    )
    .bind(&tmpl.id)
    .fetch_optional(&mut *tx)
    .await?;
    if still_valid.is_none() {
        tx.rollback().await?;
        tracing::info!(
            "Abo-Rechnung „{}“: Vorlage pausiert oder Kunde nicht mehr verfügbar — kein Entwurf",
            tmpl.label
        );
        return Ok(None);
    }
    let pending_key = format!("{PENDING_PREFIX}{}", tmpl.id);
    if mode != AutoMode::Draft {
        let open: Option<String> =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?")
                .bind(&pending_key)
                .fetch_optional(&mut *tx)
                .await?;
        if open.is_some() {
            tx.rollback().await?;
            return Err(Error::Domain(
                "Ein begonnener Termin dieser Vorlage ist noch nicht abgeschlossen; der nächste \
                 Lauf setzt ihn zuerst fort."
                    .into(),
            ));
        }
    }
    let (invoice_id, number) = insert_prepared_draft(&mut tx, &prepared).await?;
    audit_log::append_in(
        &mut tx,
        "recurring_invoice.run",
        "recurring_invoice",
        &tmpl.id,
        Some(serde_json::json!({
            "template": tmpl.id,
            "invoice_id": invoice_id,
            "number": number,
            "occurrence": occurrence.to_string(),
            "mode": mode.as_db(),
            "template_mode": tmpl.auto_mode,
        })),
    )
    .await?;
    recurring_invoice::advance_in(
        &mut tx,
        &tmpl.id,
        &occurrence.to_string(),
        Some(&invoice_id),
        &following.to_string(),
    )
    .await?;
    if mode != AutoMode::Draft {
        app_settings::set_in(
            &mut tx,
            &pending_key,
            &format!("{invoice_id}|{occurrence}|{}", mode.as_db()),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Some((invoice_id, number)))
}

/// Festschreiben bzw. Versand nach dem Anlegen (oder beim Fortsetzen nach
/// einem Abbruch) und Hinweis. Fehler werden als Hinweis gemeldet; der Entwurf
/// bzw. die festgeschriebene Rechnung bleibt. Entfernt am Ende den
/// Fortsetzungs-Marker.
#[allow(clippy::too_many_arguments)]
async fn post_process(
    job: &JobCtx<'_>,
    label: &str,
    template_id: &str,
    invoice_id: &str,
    number: &str,
    occurrence: NaiveDate,
    mode: AutoMode,
    notify_draft: bool,
) {
    let pool = job.pool;
    let dedup = format!("recurring_invoice:{template_id}:{occurrence}");
    match mode {
        AutoMode::Draft => {
            if notify_draft {
                emit_notice(
                    job,
                    "Abo-Rechnung als Entwurf bereit",
                    &format!(
                        "„{label}“: Rechnungs-Entwurf {number} angelegt — bitte prüfen und festschreiben."
                    ),
                    "info",
                    invoice_id,
                    &dedup,
                )
                .await;
            }
        }
        AutoMode::Issue | AutoMode::IssueSend => {
            let invoice = match invoices::get(pool, invoice_id).await {
                Ok(Some(inv)) => inv,
                Ok(None) => {
                    tracing::warn!("Abo-Rechnung {number}: Rechnung {invoice_id} fehlt");
                    clear_pending(pool, template_id).await;
                    return;
                }
                Err(e) => {
                    // Marker bleibt: der nächste Lauf versucht es erneut.
                    tracing::warn!("Abo-Rechnung {number}: nicht lesbar: {e}");
                    return;
                }
            };
            // W14-032: Ein vom Inhaber verworfener Entwurf wird nicht mehr
            // festgeschrieben (das Verwerfen entfernt den Marker selbst; hier
            // nur der Fall, dass es zwischen Marker-Lesen und Festschreiben lag).
            if invoice.voided_at.is_some() {
                tracing::info!("Abo-Rechnung {number}: Entwurf verworfen — nicht fortgesetzt");
                clear_pending(pool, template_id).await;
                return;
            }
            if invoice.locked_at.is_none() {
                // R2-006 (GJ-Guard): nie automatisch in ein abgeschlossenes GJ.
                if guard::is_closed(pool, invoice.fiscal_year)
                    .await
                    .unwrap_or(true)
                {
                    emit_notice(
                        job,
                        "Abo-Rechnung: Auto-Festschreiben übersprungen",
                        &format!(
                            "„{label}“: Entwurf {number} liegt vor, kann aber nicht automatisch festgeschrieben werden, weil das Geschäftsjahr {} bereits abgeschlossen ist. Bitte den Beleg manuell prüfen.",
                            invoice.fiscal_year
                        ),
                        "warning",
                        invoice_id,
                        &dedup,
                    )
                    .await;
                    clear_pending(pool, template_id).await;
                    return;
                }
                if let Err(e) =
                    run_lock_pipeline_on(pool, job.paths, invoice_id, "N/A", job.today()).await
                {
                    // Entwurf bleibt bestehen (nummeriert, erneut festschreibbar).
                    tracing::warn!(
                        "Abo-Rechnung {number}: automatisches Festschreiben fehlgeschlagen: {e}"
                    );
                    emit_notice(
                        job,
                        "Abo-Rechnung: Festschreiben fehlgeschlagen",
                        &format!(
                            "„{label}“: Entwurf {number} liegt vor, konnte aber nicht automatisch festgeschrieben werden ({e}). Bitte manuell festschreiben."
                        ),
                        "warning",
                        invoice_id,
                        &dedup,
                    )
                    .await;
                    clear_pending(pool, template_id).await;
                    return;
                }
            }
            if mode == AutoMode::IssueSend {
                let already_sent = invoices::get(pool, invoice_id)
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|i| i.sent_at.is_some());
                if !already_sent {
                    try_auto_send(job, label, invoice_id, number, &dedup).await;
                }
            } else {
                emit_notice(
                    job,
                    "Abo-Rechnung erstellt",
                    &format!("„{label}“: Rechnung {number} automatisch festgeschrieben."),
                    "info",
                    invoice_id,
                    &dedup,
                )
                .await;
            }
        }
    }
    clear_pending(pool, template_id).await;
}

async fn clear_pending(pool: &SqlitePool, template_id: &str) {
    if let Err(e) = app_settings::delete(pool, &format!("{PENDING_PREFIX}{template_id}")).await {
        tracing::warn!("Fortsetzungs-Marker der Vorlage {template_id} nicht entfernt: {e}");
    }
}

/// Setzt nach einem Abbruch begonnene Termine fort (Marker aus
/// [`create_occurrence`]): festschreiben, falls noch Entwurf; versenden, falls
/// `issue_send` und noch nicht versendet. Liefert die Zahl fortgesetzter Termine.
pub async fn resume_pending(job: &JobCtx<'_>) -> Result<usize> {
    let markers = app_settings::list_prefix(job.pool, PENDING_PREFIX).await?;
    let mut resumed = 0usize;
    for (key, value) in markers {
        if job.stop_requested() {
            break;
        }
        let template_id = key.trim_start_matches(PENDING_PREFIX).to_string();
        let _guard = crate::locks::RECURRING_TEMPLATES.lock(&template_id).await;
        if resume_one_locked(job, &template_id, &key, &value).await? {
            resumed += 1;
        }
    }
    Ok(resumed)
}

/// Setzt den Fortsetzungs-Marker `key` = `value` einer Vorlage fort. Der
/// Aufrufer hält die Vorlagen-Sperre (Tick: [`resume_pending`]; „Jetzt
/// erstellen“: [`run_now`], F1d). `true` = ein Termin wurde fortgesetzt.
async fn resume_one_locked(
    job: &JobCtx<'_>,
    template_id: &str,
    key: &str,
    value: &str,
) -> Result<bool> {
    let mut parts = value.splitn(3, '|');
    let (Some(invoice_id), Some(occurrence), Some(mode)) = (
        parts.next(),
        parts
            .next()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
        parts.next().and_then(AutoMode::from_db),
    ) else {
        tracing::warn!("Fortsetzungs-Marker {key} unlesbar ({value}) — entfernt");
        clear_pending(job.pool, template_id).await;
        return Ok(false);
    };
    // Unter der Sperre erneut prüfen (ein paralleler Lauf kann ihn erledigt haben).
    if app_settings::get(job.pool, key).await?.as_deref() != Some(value) {
        return Ok(false);
    }
    let Some(invoice) = invoices::get(job.pool, invoice_id).await? else {
        clear_pending(job.pool, template_id).await;
        return Ok(false);
    };
    let label = recurring_invoice::get_row(job.pool, template_id)
        .await?
        .map(|t| t.label)
        .unwrap_or_else(|| "Abo-Rechnung".into());
    tracing::info!(
        "Abo-Rechnung {}: Fortsetzung nach Abbruch (Termin {occurrence})",
        invoice.invoice_number
    );
    post_process(
        job,
        &label,
        template_id,
        invoice_id,
        &invoice.invoice_number,
        occurrence,
        mode,
        true,
    )
    .await;
    Ok(true)
}

/// Auto-Versand für `issue_send` nach erfolgreichem Festschreiben (best-effort).
/// Nutzt das Standard-Mail-Konto; der Empfänger kommt aus dem Buyer-Snapshot bzw.
/// Kontakt. Fehler rollen die festgeschriebene Rechnung NICHT zurück — sie werden
/// nur als Hinweis gemeldet. Genau EIN Hinweis je Periode (gleicher `dedup_key`).
async fn try_auto_send(job: &JobCtx<'_>, label: &str, invoice_id: &str, number: &str, dedup: &str) {
    let pool = job.pool;
    match mail_accounts::get_default(pool).await {
        Ok(Some(account)) => {
            match send_invoice_core(pool, job.paths, &account.id, invoice_id, None, None, None)
                .await
            {
                Ok(_) => {
                    emit_notice(
                        job,
                        "Abo-Rechnung erstellt & versendet",
                        &format!(
                            "„{label}“: Rechnung {number} automatisch festgeschrieben und per E-Mail versendet."
                        ),
                        "info",
                        invoice_id,
                        dedup,
                    )
                    .await;
                }
                Err(e) => {
                    // W14-016: Der Fehlertext nennt oft die Empfängeradresse (Antwort des
                    // Mailservers) — WARN nur mit Fehlerart, Volltext auf DEBUG, im
                    // Hinweis und im E-Mail-Protokoll.
                    tracing::warn!(
                        "Abo-Rechnung {number}: Auto-Versand fehlgeschlagen (Fehlerart {})",
                        e.kind()
                    );
                    tracing::debug!("Abo-Rechnung {number}: Auto-Versand: {e}");
                    emit_notice(
                        job,
                        "Abo-Rechnung: Versand fehlgeschlagen",
                        &format!(
                            "„{label}“: Rechnung {number} ist festgeschrieben, der automatische Versand schlug fehl ({e}). Bitte manuell versenden."
                        ),
                        "warning",
                        invoice_id,
                        dedup,
                    )
                    .await;
                }
            }
        }
        Ok(None) => {
            emit_notice(
                job,
                "Abo-Rechnung erstellt — Versand offen",
                &format!(
                    "„{label}“: Rechnung {number} festgeschrieben. Kein Standard-Mail-Konto gesetzt — bitte manuell versenden (Einstellungen → E-Mail)."
                ),
                "warning",
                invoice_id,
                dedup,
            )
            .await;
        }
        Err(e) => {
            tracing::warn!(
                "Abo-Rechnung {number}: Mail-Konto-Abruf fehlgeschlagen (Fehlerart {})",
                e.kind()
            );
            tracing::debug!("Abo-Rechnung {number}: Mail-Konto-Abruf: {e}");
            emit_notice(
                job,
                "Abo-Rechnung erstellt — Versand offen",
                &format!(
                    "„{label}“: Rechnung {number} festgeschrieben, Versand nicht möglich ({e})."
                ),
                "warning",
                invoice_id,
                dedup,
            )
            .await;
        }
    }
}

/// Hinweis zu einem gescheiterten Termin `at` (F1d: der gescheiterte, nicht
/// der erste des Laufs).
async fn failure_notice(job: &JobCtx<'_>, tmpl: &RecurringInvoiceRow, at: &str, e: &Error) {
    tracing::warn!(
        "Abo-Rechnung „{}“ ({}, Termin {at}) fehlgeschlagen: {e}",
        tmpl.label,
        tmpl.id
    );
    let body = format!(
        "„{}“: Der fällige Termin {} konnte nicht erstellt werden ({e}). Die übrigen Vorlagen \
         liefen weiter; der nächste Lauf versucht es erneut.",
        tmpl.label,
        de_date(at)
    );
    template_notice(
        job,
        "Abo-Rechnung: Erstellen fehlgeschlagen",
        &body,
        "warning",
        &tmpl.id,
        "/recurring-invoices",
        &format!("recurring_invoice_failed:{}:{at}", tmpl.id),
    )
    .await;
}

async fn blocked_notice(job: &JobCtx<'_>, tmpl: &RecurringInvoiceRow, reason: &str) {
    tracing::warn!("Abo-Rechnung „{}“ übersprungen: {reason}", tmpl.label);
    let body = format!(
        "„{}“: Der fällige Termin {} wurde nicht erstellt. {reason}",
        tmpl.label,
        de_date(&tmpl.next_due_date)
    );
    template_notice(
        job,
        "Abo-Rechnung übersprungen",
        &body,
        "warning",
        &tmpl.id,
        &format!("/recurring-invoices/new?id={}", tmpl.id),
        &format!(
            "recurring_invoice_blocked:{}:{}",
            tmpl.id, tmpl.next_due_date
        ),
    )
    .await;
}

async fn decision_notice(
    job: &JobCtx<'_>,
    tmpl: &RecurringInvoiceRow,
    due: usize,
    after_restore: bool,
) {
    let body = if after_restore {
        format!(
            "„{}“: Nach der Wiederherstellung {} seit {} offen. Ob vor der Wiederherstellung \
             schon abgerechnet wurde, weiß Klein.Buch nicht (kein Stand der ersetzten \
             Datenbank). Es wurde nichts erstellt. Bitte unter Rechnungen → Wiederkehrende \
             Rechnungen bei der Vorlage „Rückstand …“ wählen: nachholen (nur als Entwurf, kein \
             automatischer Versand) oder ab heute fortsetzen (lässt auch einen heute fälligen \
             Termin aus).",
            tmpl.label,
            if due == 1 {
                "ist ein Termin".to_string()
            } else {
                format!("sind {due} Termine")
            },
            de_date(&tmpl.next_due_date),
        )
    } else {
        format!(
            "„{}“: {due} Termine seit {} sind offen — mehr als Klein.Buch ohne Rückfrage \
             nachholt ({}). Es wurde nichts erstellt. Bitte unter Rechnungen → Wiederkehrende \
             Rechnungen bei der Vorlage „Rückstand …“ wählen: nachholen (höchstens {}, ältere \
             Termine als Entwurf) oder ab heute fortsetzen.",
            tmpl.label,
            de_date(&tmpl.next_due_date),
            catch_up::AUTO_LIMIT,
            catch_up::CONFIRMED_CAP
        )
    };
    template_notice(
        job,
        "Abo-Rechnung: Entscheidung nötig",
        &body,
        "warning",
        &tmpl.id,
        "/recurring-invoices",
        &format!(
            "recurring_invoice_backlog:{}:{}",
            tmpl.id, tmpl.next_due_date
        ),
    )
    .await;
}

async fn catch_up_notice(job: &JobCtx<'_>, template_id: &str, label: &str, drafts: &[String]) {
    let body = format!(
        "„{label}“: {} nachgeholte Termine entstanden als Entwurf ({}). Nachgeholte \
         Rechnungen werden nie automatisch festgeschrieben oder versendet — bitte prüfen, \
         festschreiben und versenden.",
        drafts.len(),
        drafts.join(", ")
    );
    template_notice(
        job,
        "Abo-Rechnungen nachgeholt",
        &body,
        "warning",
        template_id,
        "/invoices",
        &format!(
            "recurring_invoice_catch_up:{template_id}:{}",
            drafts.first().map(String::as_str).unwrap_or("-")
        ),
    )
    .await;
}

async fn template_notice(
    job: &JobCtx<'_>,
    title: &str,
    body: &str,
    severity: &str,
    template_id: &str,
    action_url: &str,
    dedup: &str,
) {
    let n = NewNotification {
        title,
        body,
        severity,
        related_entity_type: Some("recurring_invoice"),
        related_entity_id: Some(template_id),
        action_url: Some(action_url),
        dedup_key: Some(dedup),
        ..Default::default()
    };
    if let Err(e) = notify::store::create(job.pool, n).await {
        tracing::warn!("Abo-Rechnung-Hinweis fehlgeschlagen: {e}");
    }
}

/// Baut die [`InvoiceInput`] aus der Vorlage + dem Perioden-Stichtag (pure).
/// Belegdatum = `today`; Leistungsdatum = `occurrence`; Fälligkeit = `today` +
/// Zahlungsziel. Liefert zusätzlich das Geschäftsjahr (= Jahr des Belegdatums).
fn build_invoice_input(
    template: &RecurringInvoiceRow,
    items: &[RecurringInvoiceItemRow],
    occurrence: NaiveDate,
    today: NaiveDate,
) -> (InvoiceInput, i64) {
    let mut invoice_items: Vec<InvoiceItemInput> = items.iter().map(row_to_item_input).collect();

    if template.service_period_note != 0 {
        if let Some(first) = invoice_items.first_mut() {
            let label = format!("{} {}", month_de(occurrence.month()), occurrence.year());
            // KB-0054: Trägt die erste Position einen Markdown-Body, würde
            // `recompute_markup_descriptions` (commands::invoices) die an
            // `description` angehängte Periode wieder aus dem Markup
            // überschreiben → der Leistungszeitraum fehlte auf PDF + XML.
            // Daher in dem Fall den Hinweis als eigenen Absatz an den Markup-
            // Body hängen; sonst wie bisher inline an `description`.
            match first.description_markup.as_mut() {
                Some(markup) if !markup.trim().is_empty() => {
                    markup.push_str(&format!("\n\nLeistungszeitraum: {label}"));
                }
                _ => {
                    first.description = format!(
                        "{} (Leistungszeitraum: {label})",
                        first.description.trim_end()
                    );
                }
            }
        }
    }

    let due_date = today.checked_add_days(Days::new(template.payment_terms_days.max(0) as u64));

    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: today, // Ausstellungsdatum = Erstellungstag (rechtlich)
        delivery_date: Some(occurrence), // Leistungsdatum = Periodenstichtag
        due_date,
        currency_code: "EUR".to_string(),
        items: invoice_items,
        notes: None,
        payment_note: None,
        pdf_template: template.pdf_template.clone(),
        is_storno_for: None,
        cancel_reason: None,
    };
    (input, today.year() as i64)
}

fn row_to_item_input(r: &RecurringInvoiceItemRow) -> InvoiceItemInput {
    InvoiceItemInput {
        position: r.position as u32,
        description: r.description.clone(),
        quantity: r.quantity,
        unit_code: r.unit_code.clone(),
        unit_price_cents: r.unit_price_cents,
        tax_rate_percent: r.tax_rate_percent,
        tax_category_code: r.tax_category_code.clone(),
        description_title: r.description_title.clone(),
        description_markup: r.description_markup.clone(),
        source_package_id: r.source_package_id.clone(),
        source_package_revision: r.source_package_revision,
    }
}

fn month_de(month: u32) -> &'static str {
    MONTHS_DE
        .get((month.clamp(1, 12) - 1) as usize)
        .copied()
        .unwrap_or("")
}

fn freq_and_day(tmpl: &RecurringInvoiceRow) -> Result<(Frequency, u32)> {
    let freq = Frequency::from_db(&tmpl.frequency).ok_or_else(|| {
        Error::Domain(format!(
            "Abo-Vorlage {} hat ungültige Frequenz '{}'",
            tmpl.id, tmpl.frequency
        ))
    })?;
    let day = tmpl.day_of_period.clamp(1, 31) as u32;
    Ok((freq, day))
}

async fn emit_notice(
    job: &JobCtx<'_>,
    title: &str,
    body: &str,
    severity: &str,
    invoice_id: &str,
    dedup_key: &str,
) {
    // Inbox-Eintrag zur Rechnung (der Server hat keinen weiteren Kanal).
    let n = NewNotification {
        rule_id: None,
        title,
        body,
        severity,
        related_entity_type: Some("invoice"),
        related_entity_id: Some(invoice_id),
        action_url: Some("/invoices"),
        dedup_key: Some(dedup_key),
    };
    if let Err(e) = notify::emit(job.pool, n).await {
        tracing::warn!("Abo-Rechnung-Hinweis fehlgeschlagen: {e}");
    }
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("Ungültiges Datum '{s}': {e}")))
}

fn end_date(tmpl: &RecurringInvoiceRow) -> Option<NaiveDate> {
    tmpl.end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

fn de_date(iso: &str) -> String {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(|d| d.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn template() -> RecurringInvoiceRow {
        RecurringInvoiceRow {
            id: "tmpl-1".into(),
            label: "Wartung Server – Müller GmbH".into(),
            contact_id: "contact-1".into(),
            frequency: "monthly".into(),
            day_of_period: 1,
            next_due_date: "2026-06-01".into(),
            start_date: None,
            end_date: None,
            auto_mode: "draft".into(),
            payment_terms_days: 14,
            pdf_template: "default".into(),
            service_period_note: 1,
            active: 1,
            last_executed_at: None,
            last_invoice_id: None,
            notes: None,
            created_at: "2026-05-24 00:00:00".into(),
            updated_at: "2026-05-24 00:00:00".into(),
        }
    }

    fn item() -> RecurringInvoiceItemRow {
        RecurringInvoiceItemRow {
            id: "it-1".into(),
            recurring_invoice_id: "tmpl-1".into(),
            position: 1,
            description: "Server-Wartung (Pauschale)".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 9_900,
            net_amount_cents: 9_900,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }
    }

    #[test]
    fn month_names_german() {
        assert_eq!(month_de(1), "Januar");
        assert_eq!(month_de(6), "Juni");
        assert_eq!(month_de(12), "Dezember");
    }

    #[test]
    fn build_input_uses_today_as_invoice_date_not_occurrence() {
        let occurrence = d(2026, 3, 1);
        let today = d(2026, 5, 24); // Catch-up: erstellt heute, nicht rückdatiert
        let (input, fy) = build_invoice_input(&template(), &[item()], occurrence, today);
        assert_eq!(input.invoice_date, today, "Belegdatum = Erstellungstag");
        assert_eq!(
            input.delivery_date,
            Some(occurrence),
            "Leistungsdatum = Periodenstichtag"
        );
        assert_eq!(fy, 2026, "Geschäftsjahr = Jahr des Belegdatums");
        assert!(matches!(input.direction, InvoiceDirection::Issued));
        assert_eq!(input.currency_code, "EUR");
    }

    #[test]
    fn build_input_due_date_is_today_plus_terms() {
        let today = d(2026, 5, 24);
        let (input, _) = build_invoice_input(&template(), &[item()], d(2026, 6, 1), today);
        assert_eq!(input.due_date, Some(d(2026, 6, 7)), "heute + 14 Tage");
    }

    #[test]
    fn build_input_keeps_kleinunternehmer_tax_codes() {
        let (input, _) = build_invoice_input(&template(), &[item()], d(2026, 6, 1), d(2026, 5, 24));
        assert_eq!(input.items[0].tax_category_code, "E");
        assert_eq!(input.items[0].tax_rate_percent, 0.0);
    }

    #[test]
    fn build_input_appends_service_period_label() {
        let (input, _) = build_invoice_input(&template(), &[item()], d(2026, 6, 1), d(2026, 5, 24));
        assert!(
            input.items[0]
                .description
                .contains("Leistungszeitraum: Juni 2026"),
            "Periode in der ersten Position: {}",
            input.items[0].description
        );
    }

    #[test]
    fn build_input_no_label_when_disabled() {
        let mut t = template();
        t.service_period_note = 0;
        let (input, _) = build_invoice_input(&t, &[item()], d(2026, 6, 1), d(2026, 5, 24));
        assert_eq!(input.items[0].description, "Server-Wartung (Pauschale)");
    }
}

#[cfg(test)]
mod pipeline_tests {
    //! DB-/Pipeline-Tests für den Abo-Scheduler (KB-0058, Lauf F). `auto_mode =
    //! 'draft'` → kein Java-Sidecar nötig. Die Uhr ist eine [`FixedClock`]
    //! (D-20); Festschreiben/Versand testet `tests/recurring_scheduler_test.rs`
    //! mit Mock-Sidecar.

    use super::*;
    use crate::backup::BackupSession;
    use crate::clock::{Clock, FixedClock};
    use crate::config::Paths;
    use crate::db::repo::seller_profile::SellerProfileInput;
    use crate::db::repo::{contacts, invoices, seller_profile};
    use crate::db::MIGRATOR;
    use crate::domain::contact::{ContactInput, ContactType};
    use crate::domain::invoice::InvoiceItemInput;
    use crate::domain::numbering::DocType;
    use crate::domain::recurring_invoice::RecurringInvoiceInput;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::path::Path;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("klein-buch.sqlite");
        let url = format!("sqlite://{}", db_path.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        (pool, dir)
    }

    fn paths_for(dir: &Path) -> Paths {
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        std::fs::create_dir_all(dir.join("archive")).unwrap();
        Paths {
            data_dir: dir.to_path_buf(),
            db_file: dir.join("klein-buch.sqlite"),
            archive_dir: dir.join("archive"),
            backups_dir: backups,
            inputs_dir: dir.join("inputs"),
            sidecar_dir: dir.join("sidecar"),
            resource_dir: std::path::PathBuf::from("/non/existent/resources"),
            web_dir: std::path::PathBuf::from("/non/existent/web"),
            backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn noon(day: NaiveDate) -> FixedClock {
        FixedClock::at_berlin(day, 12, 0)
    }

    fn unlocked() -> BackupSession {
        let s = BackupSession::default();
        s.set("passphrase-1234".into());
        s
    }

    /// §19-Verkäuferprofil (Kleinunternehmer) — Pflicht für die Draft-Pipeline.
    async fn mk_seller(pool: &SqlitePool) {
        let input = SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Teststr. 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: None,
            vat_id: None,
            email: "test@example.org".into(),
            phone: None,
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: None,
            default_pdf_template: None,
            default_currency: None,
        };
        seller_profile::upsert(pool, &input).await.unwrap();
    }

    async fn mk_contact(pool: &SqlitePool) -> String {
        let input = ContactInput {
            contact_type: ContactType::Customer,
            name: "Müller GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: None,
            tax_number: None,
            street: "Kundenweg 2".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: Some("kunde@example.org".into()),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        };
        contacts::create(pool, &input).await.unwrap().id
    }

    fn ri_item() -> InvoiceItemInput {
        InvoiceItemInput {
            position: 1,
            description: "Server-Wartung (Pauschale)".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 9_900,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }
    }

    fn ri_tmpl(contact_id: &str, next_due: NaiveDate, auto_mode: &str) -> RecurringInvoiceInput {
        RecurringInvoiceInput {
            label: "Wartung Server – Müller GmbH".into(),
            contact_id: contact_id.into(),
            frequency: "monthly".into(),
            day_of_period: 1,
            next_due_date: next_due,
            start_date: None,
            end_date: None,
            auto_mode: auto_mode.into(),
            payment_terms_days: 14,
            pdf_template: "default".into(),
            service_period_note: true,
            notes: None,
            items: vec![ri_item()],
        }
    }

    async fn count(pool: &SqlitePool, sql: &str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
    }

    async fn notice_count(pool: &SqlitePool, prefix: &str) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE ?")
            .bind(format!("{prefix}%"))
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Wartet, bis an der Sperre `waiters` weitere Aufgaben warten (Halter +
    /// Tabelle zählen mit; tokio-Mutexe sind fair, Reihenfolge = Anfrage).
    async fn wait_for_waiters(guard: &tokio::sync::OwnedMutexGuard<()>, waiters: usize) {
        for _ in 0..5_000 {
            if Arc::strong_count(tokio::sync::OwnedMutexGuard::mutex(guard)) >= 2 + waiters {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        panic!("Wartende an der Vorlagen-Sperre erscheinen nicht");
    }

    #[tokio::test]
    async fn process_due_skips_when_locked() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = BackupSession::default(); // gesperrt
        let clock = noon(d(2026, 5, 20));

        recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
            .await
            .unwrap();

        let report = process_due(&JobCtx::new(&pool, &paths, &session, &clock))
            .await
            .unwrap();
        assert!(
            report.skipped_locked,
            "gesperrte Session → Lauf übersprungen"
        );
        assert_eq!(report.created_invoices, 0);
    }

    /// Bis zur Grenze (3) holt der Lauf selbst nach; ältere Termine als Entwurf
    /// mit Sammelhinweis, Belegdatum = heute, Leistungsdatum = Periode.
    #[tokio::test]
    async fn process_due_catches_up_up_to_limit_older_as_drafts() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));

        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 3, 1), "draft"))
                .await
                .unwrap();

        // Heute 2026-05-20 → fällige Stichtage 03-01, 04-01, 05-01 (3); 06-01 Zukunft.
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        let report = process_due(&job).await.unwrap();
        assert!(!report.skipped_locked);
        assert_eq!(report.processed_templates, 1);
        assert_eq!(
            report.created_invoices, 3,
            "drei verpasste Perioden nachgeholt"
        );

        let after = recurring_invoice::get_row(&pool, &detail.template.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.next_due_date, "2026-06-01");
        let last_id = after.last_invoice_id.expect("last_invoice_id gesetzt");

        let inv = invoices::get(&pool, &last_id).await.unwrap().unwrap();
        assert_eq!(
            inv.invoice_date, "2026-05-20",
            "Belegdatum = Erstellungstag"
        );
        assert_eq!(inv.delivery_date.as_deref(), Some("2026-05-01"));
        assert_eq!(inv.fiscal_year, 2026);
        assert_eq!(
            notice_count(&pool, "recurring_invoice_catch_up:").await,
            1,
            "Sammelhinweis zu den älteren Entwürfen"
        );

        // Zweiter Lauf am selben Tag legt nichts Neues an (idempotent über Stichtag).
        let again = process_due(&job).await.unwrap();
        assert_eq!(again.created_invoices, 0);
    }

    /// W14-026: Mehr als die Grenze fällig → nichts erzeugen, Hinweis; nach
    /// Bestätigung („nachholen“) alle Termine, ältere als Entwurf.
    #[tokio::test]
    async fn backlog_over_limit_waits_for_confirmation() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let job = JobCtx::new(&pool, &paths, &session, &clock);

        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2025, 12, 1), "draft"))
                .await
                .unwrap();
        let id = detail.template.id.clone();

        // 12-01 … 05-01 = 6 fällige Termine > Grenze 3
        let report = process_due(&job).await.unwrap();
        assert_eq!(report.created_invoices, 0);
        assert_eq!(report.awaiting_decision, 1);
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert_eq!(notice_count(&pool, "recurring_invoice_backlog:").await, 1);
        // Hinweis nur einmal je Stichtag
        process_due(&job).await.unwrap();
        assert_eq!(notice_count(&pool, "recurring_invoice_backlog:").await, 1);

        let tmpl = recurring_invoice::get_row(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        apply_resume_mode(&pool, &tmpl, ResumeMode::CatchUp, clock.today_berlin())
            .await
            .unwrap();
        let report = process_due(&job).await.unwrap();
        assert_eq!(report.created_invoices, 6, "nach Bestätigung alle sechs");
        let after = recurring_invoice::get_row(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.next_due_date, "2026-06-01");
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM invoices WHERE locked_at IS NULL"
            )
            .await,
            6
        );
        assert!(
            app_settings::get(&pool, &format!("recurring_catch_up_ok:inv:{id}"))
                .await
                .unwrap()
                .is_none(),
            "Bestätigung verbraucht"
        );
    }

    /// „Ab heute fortsetzen“: kein Beleg, Stichtag auf den ersten Termin ab
    /// heute, Audit mit der Zahl ausgelassener Termine.
    #[tokio::test]
    async fn skip_backlog_moves_next_due_without_invoices() {
        let (pool, _dir) = setup_pool().await;
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2025, 12, 1), "issue_send"))
                .await
                .unwrap();
        let tmpl = detail.template;
        apply_resume_mode(&pool, &tmpl, ResumeMode::Skip, d(2026, 5, 20))
            .await
            .unwrap();
        let after = recurring_invoice::get_row(&pool, &tmpl.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.next_due_date, "2026-06-01");
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        let details: String = sqlx::query_scalar(
            "SELECT details_json FROM audit_log WHERE action = 'recurring_invoice.backlog_skipped'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(details.contains("\"skipped\":6"), "{details}");
    }

    /// Zu viele Termine lassen sich nicht bestätigen.
    #[tokio::test]
    async fn catch_up_confirmation_is_capped() {
        let (pool, _dir) = setup_pool().await;
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2023, 1, 1), "draft"))
                .await
                .unwrap();
        let err = apply_resume_mode(&pool, &detail.template, ResumeMode::CatchUp, d(2026, 5, 20))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("zu viele"), "{err}");
    }

    #[tokio::test]
    async fn run_now_creates_one_with_today_invoice_date_and_advances() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));

        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
                .await
                .unwrap();

        let invoice_id = run_now(
            &JobCtx::new(&pool, &paths, &session, &clock),
            &detail.template.id,
        )
        .await
        .unwrap();

        let inv = invoices::get(&pool, &invoice_id).await.unwrap().unwrap();
        assert_eq!(
            inv.invoice_date, "2026-05-20",
            "Belegdatum = heute, nicht der Periodenstichtag 05-01"
        );
        assert_eq!(inv.delivery_date.as_deref(), Some("2026-05-01"));

        let after = recurring_invoice::get_row(&pool, &detail.template.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            after.next_due_date, "2026-06-01",
            "um eine Periode vorgerückt"
        );
    }

    /// Plan W1.4: Tick und „Jetzt erstellen“ gleichzeitig auf derselben fälligen
    /// Periode → genau eine Rechnung, Vorlage genau einmal vorgerückt.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parallel_tick_and_run_now_create_exactly_one_invoice() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));

        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
                .await
                .unwrap();
        let id = detail.template.id.clone();

        let job = JobCtx::new(&pool, &paths, &session, &clock);
        let (tick, now) = tokio::join!(process_due(&job), run_now(&job, &id));
        let tick = tick.unwrap();
        let created = tick.created_invoices + usize::from(now.is_ok());
        assert_eq!(
            created, 1,
            "genau eine Rechnung (tick={tick:?}, run_now={now:?})"
        );

        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 1);
        let after = recurring_invoice::get_row(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.next_due_date, "2026-06-01");
    }

    /// Compare-and-Swap: ein veralteter Stichtag rückt nicht vor.
    #[tokio::test]
    async fn advance_with_stale_due_date_is_rejected() {
        let (pool, _dir) = setup_pool().await;
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
                .await
                .unwrap();
        let id = detail.template.id.clone();
        let err = recurring_invoice::advance(&pool, &id, "2026-04-01", "inv-x", "2026-05-01")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("vorgerückt"), "{err}");
        let row = recurring_invoice::get_row(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.next_due_date, "2026-05-01");
        assert!(row.last_invoice_id.is_none());
    }

    /// W14-025: Entwurf und Vorrücken sind eine Transaktion. Scheitert das
    /// Vorrücken (Termin inzwischen weiter), bleibt weder Entwurf noch
    /// verbrauchte Nummer zurück.
    #[tokio::test]
    async fn failed_advance_rolls_back_draft_and_number() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "issue"))
                .await
                .unwrap();
        let items = recurring_invoice::items_for(&pool, &detail.template.id)
            .await
            .unwrap();
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        // veralteter Termin 04-01 statt 05-01 → CAS scheitert in der Transaktion
        let err = create_occurrence(
            &job,
            &detail.template,
            &items,
            d(2026, 4, 1),
            d(2026, 5, 20),
            AutoMode::Issue,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("vorgerückt"), "{err}");
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert_eq!(
            crate::db::numbering::peek_seq(&pool, DocType::Invoice, 2026)
                .await
                .unwrap(),
            0,
            "keine Nummer verbraucht"
        );
        assert_eq!(
            app_settings::list_prefix(&pool, PENDING_PREFIX)
                .await
                .unwrap()
                .len(),
            0,
            "kein Fortsetzungs-Marker"
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM audit_log WHERE action = 'recurring_invoice.run'"
            )
            .await,
            0
        );
    }

    /// W14-031: Anonymisierter Kunde → kein nummerierter Entwurf, sondern Hinweis.
    #[tokio::test]
    async fn anonymized_contact_creates_no_draft() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
            .await
            .unwrap();
        sqlx::query(
            "UPDATE contacts SET anonymized_at = datetime('now','utc'), archived = 1 WHERE id = ?",
        )
        .bind(&contact_id)
        .execute(&pool)
        .await
        .unwrap();
        let report = process_due(&JobCtx::new(&pool, &paths, &session, &clock))
            .await
            .unwrap();
        assert_eq!(report.created_invoices, 0);
        assert_eq!(report.failed_templates, 1);
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert_eq!(
            crate::db::numbering::peek_seq(&pool, DocType::Invoice, 2026)
                .await
                .unwrap(),
            0
        );
        assert_eq!(notice_count(&pool, "recurring_invoice_blocked:").await, 1);
    }

    /// R1-017: §14c-Prüfung auch im Scheduler — Vorlage mit USt bei §19 erzeugt
    /// keinen Entwurf.
    #[tokio::test]
    async fn vat_template_under_paragraph_19_creates_no_draft() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let mut input = ri_tmpl(&contact_id, d(2026, 5, 1), "draft");
        input.items[0].tax_category_code = "S".into();
        input.items[0].tax_rate_percent = 19.0;
        // am Command-Schutz vorbei direkt über das Repo (Altbestand/Statuswechsel)
        recurring_invoice::create(&pool, &input).await.unwrap();
        let report = process_due(&JobCtx::new(&pool, &paths, &session, &clock))
            .await
            .unwrap();
        assert_eq!(report.created_invoices, 0);
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert_eq!(notice_count(&pool, "recurring_invoice_blocked:").await, 1);
    }

    /// W14-031: Pausieren zwischen zwei Terminen eines Nachhol-Laufs wirkt ab
    /// dem nächsten Termin (Sperre je Termin, frisches Lesen).
    #[tokio::test]
    async fn pause_between_periods_stops_catch_up() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 3, 1), "draft"))
                .await
                .unwrap();
        let id = detail.template.id.clone();

        // Testsperre halten, Lauf starten: er wartet auf die Entscheidung.
        let g1 = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
        let run = {
            let (pool, paths) = (pool.clone(), paths.clone());
            tokio::spawn(async move {
                let session = unlocked();
                let clock = noon(d(2026, 5, 20));
                process_due(&JobCtx::new(&pool, &paths, &session, &clock))
                    .await
                    .unwrap()
            })
        };
        wait_for_waiters(&g1, 1).await;
        // Freigeben und sofort wieder anfragen: der Lauf entscheidet (3 Termine)
        // und stellt sich für Termin 1 hinter uns an.
        drop(g1);
        let g2 = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
        wait_for_waiters(&g2, 1).await;
        // Pausieren stellt sich hinter den Lauf (Termin 1).
        let pauser = {
            let (pool, id) = (pool.clone(), id.clone());
            tokio::spawn(async move {
                let _g = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
                recurring_invoice::set_active(&pool, &id, false)
                    .await
                    .unwrap();
            })
        };
        wait_for_waiters(&g2, 2).await;
        drop(g2);
        pauser.await.unwrap();
        let report = run.await.unwrap();
        assert_eq!(
            report.created_invoices, 1,
            "nur der laufende Termin: {report:?}"
        );
        let after = recurring_invoice::get_row(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.next_due_date, "2026-04-01");
        assert_eq!(after.active, 0);
    }

    /// W14-027: Die Abbruch-Anforderung beendet einen Nachhol-Lauf nach dem
    /// laufenden Termin.
    #[tokio::test]
    async fn stop_request_ends_catch_up_after_current_period() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 3, 1), "draft"))
                .await
                .unwrap();
        let id = detail.template.id.clone();
        static STOP: AtomicBool = AtomicBool::new(false);

        let g1 = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
        let run = {
            let (pool, paths) = (pool.clone(), paths.clone());
            tokio::spawn(async move {
                let session = unlocked();
                let clock = noon(d(2026, 5, 20));
                process_due(&JobCtx::new(&pool, &paths, &session, &clock).with_stop(&STOP))
                    .await
                    .unwrap()
            })
        };
        wait_for_waiters(&g1, 1).await;
        drop(g1);
        let g2 = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
        wait_for_waiters(&g2, 1).await; // Lauf wartet auf Termin 1
        STOP.store(true, Ordering::SeqCst);
        drop(g2);
        let report = run.await.unwrap();
        assert_eq!(report.created_invoices, 1, "{report:?}");
        let after = recurring_invoice::get_row(&pool, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.next_due_date, "2026-04-01");
    }

    #[tokio::test]
    async fn run_now_rejects_future_due() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));

        // next_due 2026-06-01 liegt nach today 2026-05-20 → vorzeitige Erstellung
        // würde ein zukünftiges Periodendatum erzeugen → Fehler.
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 6, 1), "draft"))
                .await
                .unwrap();

        let res = run_now(
            &JobCtx::new(&pool, &paths, &session, &clock),
            &detail.template.id,
        )
        .await;
        assert!(
            res.is_err(),
            "Zukunfts-Stichtag darf nicht vorzeitig erstellt werden"
        );
    }

    /// „Jetzt erstellen“ nach dem Laufzeit-Ende erzeugt nichts.
    #[tokio::test]
    async fn run_now_respects_end_date() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let mut input = ri_tmpl(&contact_id, d(2026, 5, 1), "draft");
        input.end_date = Some(d(2026, 4, 30));
        let detail = recurring_invoice::create(&pool, &input).await.unwrap();
        let err = run_now(
            &JobCtx::new(&pool, &paths, &session, &clock),
            &detail.template.id,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("Laufzeit"), "{err}");
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
    }

    /// Jahreswechsel mit Test-Uhr: 31.12. 23:30 UTC ist in Berlin schon der
    /// 01.01. → Belegdatum und Nummernkreis des neuen Jahres.
    #[tokio::test]
    async fn new_year_in_berlin_uses_new_number_range() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let mut input = ri_tmpl(&contact_id, d(2027, 1, 1), "draft");
        input.day_of_period = 1;
        recurring_invoice::create(&pool, &input).await.unwrap();

        let clock = FixedClock::at(
            chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 12, 31, 22, 59, 0).unwrap(),
        );
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        assert_eq!(
            process_due(&job).await.unwrap().created_invoices,
            0,
            "Berlin: 23:59 am 31.12."
        );
        clock.advance(chrono::Duration::minutes(31)); // 23:30 UTC = 00:30 Berlin
        let report = process_due(&job).await.unwrap();
        assert_eq!(report.created_invoices, 1);
        let number: String = sqlx::query_scalar("SELECT invoice_number FROM invoices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(number, "RE-2027-0001");
        let date: String = sqlx::query_scalar("SELECT invoice_date FROM invoices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(date, "2027-01-01");
    }

    /// ASVS V11.1.5: Uhrsprung um Jahre nach vorn → Pause statt Massenerzeugung.
    #[tokio::test]
    async fn clock_jump_holds_automation() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 6, 1), "draft"))
            .await
            .unwrap();
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        assert_eq!(process_due(&job).await.unwrap().created_invoices, 0);
        clock.set_berlin(d(2036, 5, 20), 12, 0);
        let report = process_due(&job).await.unwrap();
        assert_eq!(
            report.skipped_hold.as_deref(),
            Some(hold::REASON_CLOCK_JUMP)
        );
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert!(run_now(
            &job,
            &recurring_invoice::list(&pool, true).await.unwrap()[0].id
        )
        .await
        .is_err());
        assert_eq!(notice_count(&pool, "scheduler_hold:").await, 1);
        // Freigabe: der Lauf arbeitet wieder (hier: Rückstand > Grenze → Hinweis)
        hold::release(&pool, clock.now_utc()).await.unwrap();
        let report = process_due(&job).await.unwrap();
        assert!(report.skipped_hold.is_none());
        assert_eq!(report.awaiting_decision, 1);
    }

    /// F1d (Race Anonymisierung ↔ Abo-Entwurf): Pausiert eine Anonymisierung
    /// die Vorlage, nachdem die Käuferdaten gelesen wurden, entsteht in der
    /// Transaktion weder Entwurf noch verbrauchte Nummer; der Termin bleibt.
    #[tokio::test]
    async fn template_paused_after_prepare_creates_no_draft() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "issue"))
                .await
                .unwrap();
        let items = recurring_invoice::items_for(&pool, &detail.template.id)
            .await
            .unwrap();
        // Zustand nach der parallel abgeschlossenen Anonymisierung (sie pausiert
        // die Vorlage in derselben Transaktion).
        sqlx::query("UPDATE recurring_invoices SET active = 0 WHERE id = ?")
            .bind(&detail.template.id)
            .execute(&pool)
            .await
            .unwrap();
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        let out = create_occurrence(
            &job,
            &detail.template,
            &items,
            d(2026, 5, 1),
            d(2026, 5, 20),
            AutoMode::Issue,
        )
        .await
        .unwrap();
        assert!(out.is_none(), "kein Entwurf");
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert_eq!(
            crate::db::numbering::peek_seq(&pool, DocType::Invoice, 2026)
                .await
                .unwrap(),
            0,
            "keine Nummer verbraucht"
        );
        assert!(app_settings::list_prefix(&pool, PENDING_PREFIX)
            .await
            .unwrap()
            .is_empty());
        let row = recurring_invoice::get_row(&pool, &detail.template.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.next_due_date, "2026-05-01", "Termin bleibt");
    }

    /// F1d: Mit offener Prüfung nach einer Wiederherstellung erstellt auch
    /// „Jetzt erstellen“ nur einen Entwurf (kein Festschreiben, kein Versand);
    /// danach ist die Prüfung erledigt.
    #[tokio::test]
    async fn run_now_under_restore_review_creates_draft_only() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 20), "issue_send"))
                .await
                .unwrap();
        let id = detail.template.id.clone();
        app_settings::set(
            &pool,
            &catch_up::review_key(Series::Template, &id),
            "2026-05-20",
        )
        .await
        .unwrap();
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        let invoice_id = run_now(&job, &id).await.unwrap();
        let inv = invoices::get(&pool, &invoice_id).await.unwrap().unwrap();
        assert!(inv.locked_at.is_none(), "nur Entwurf");
        assert!(app_settings::list_prefix(&pool, PENDING_PREFIX)
            .await
            .unwrap()
            .is_empty());
        assert!(
            app_settings::get(&pool, &catch_up::review_key(Series::Template, &id))
                .await
                .unwrap()
                .is_none(),
            "Prüfung erledigt (nächster Termin nach dem Restore-Tag)"
        );
    }

    /// F1d: Ein offener Fortsetzungs-Marker wird nie überschrieben — der
    /// begonnene Termin geht sonst still verloren.
    #[tokio::test]
    async fn open_pending_marker_is_never_overwritten() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "issue"))
                .await
                .unwrap();
        let id = detail.template.id.clone();
        let marker = format!("{PENDING_PREFIX}{id}");
        app_settings::set(&pool, &marker, "inv-frueher|2026-04-01|issue")
            .await
            .unwrap();
        let items = recurring_invoice::items_for(&pool, &id).await.unwrap();
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        let err = create_occurrence(
            &job,
            &detail.template,
            &items,
            d(2026, 5, 1),
            d(2026, 5, 20),
            AutoMode::Issue,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("begonnener Termin"), "{err}");
        assert_eq!(count(&pool, "SELECT COUNT(*) FROM invoices").await, 0);
        assert_eq!(
            app_settings::get(&pool, &marker).await.unwrap().as_deref(),
            Some("inv-frueher|2026-04-01|issue"),
            "Marker unverändert"
        );
    }

    /// W14-032 (Lauf F2a): Ein Fortsetzungs-Marker, der auf einen inzwischen
    /// verworfenen Entwurf zeigt, wird ohne Festschreiben und ohne
    /// Fehlerhinweis entfernt.
    #[tokio::test]
    async fn pending_marker_on_voided_draft_is_dropped() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = unlocked();
        let clock = noon(d(2026, 5, 20));
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "issue"))
                .await
                .unwrap();
        let id = detail.template.id.clone();
        let items = recurring_invoice::items_for(&pool, &id).await.unwrap();
        let job = JobCtx::new(&pool, &paths, &session, &clock);
        let (invoice_id, _number) = create_occurrence(
            &job,
            &detail.template,
            &items,
            d(2026, 5, 1),
            d(2026, 5, 20),
            AutoMode::Draft,
        )
        .await
        .unwrap()
        .expect("Entwurf");
        crate::commands::invoices::void_draft_core(
            &pool,
            &invoice_id,
            "Termin doppelt angelegt",
            clock.now_utc(),
        )
        .await
        .unwrap();
        // Marker wie vor dem Verwerfen gelesen (Race-Fall)
        let marker = format!("{PENDING_PREFIX}{id}");
        app_settings::set(&pool, &marker, &format!("{invoice_id}|2026-05-01|issue"))
            .await
            .unwrap();
        resume_pending(&job).await.unwrap();
        assert!(app_settings::get(&pool, &marker).await.unwrap().is_none());
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM invoices WHERE locked_at IS NOT NULL"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM notifications WHERE title LIKE '%fehlgeschlagen%'"
            )
            .await,
            0
        );
    }
}
