//! Commands für wiederkehrende Kosten-Abos (Block 10, Lauf F).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::recurring::validate_recurring`]).
//! - CRUD + Pausieren ([`crate::db::repo::recurring`]).
//! - Manuelle/automatische Auto-Anlage von Kosten ([`crate::scheduler::recurring`]).
//! - Audit-Log.
//!
//! ## Abgrenzung
//!
//! Ein Abo ist ein Stammdatum/Template — editierbar und pausierbar (kein
//! GoBD-Beleg). Die daraus erzeugten Kosten sind sofort gelockt (Block 9).

use crate::app_ctx::AppCtx;
use chrono::NaiveDate;
use std::sync::Arc;

use crate::db::models::{ExpenseRow, RecurringSubscriptionRow};
use crate::db::repo::{audit_log, recurring};
use crate::domain::recurring::{self as domain, RecurringInput};
use crate::error::{Error, Result};
use crate::scheduler::catch_up::{self, ResumeMode, Series};
use crate::scheduler::recurring::{apply_resume_mode, process_due, run_now, ProcessReport};
use crate::scheduler::JobCtx;

// =============================================================================
// Read
// =============================================================================

/// Argumente von [`recurring_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringListArgs {
    #[serde(default)]
    pub include_inactive: Option<bool>,
}

pub async fn recurring_list(
    ctx: &Arc<AppCtx>,
    args: RecurringListArgs,
) -> Result<Vec<RecurringSubscriptionRow>> {
    let RecurringListArgs { include_inactive } = args;
    let pool = ctx.pool()?;

    recurring::list(&pool, include_inactive.unwrap_or(false)).await
}

/// Argumente von [`recurring_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringGetArgs {
    pub id: String,
}

pub async fn recurring_get(
    ctx: &Arc<AppCtx>,
    args: RecurringGetArgs,
) -> Result<Option<RecurringSubscriptionRow>> {
    let RecurringGetArgs { id } = args;
    let pool = ctx.pool()?;

    recurring::get(&pool, &id).await
}

// =============================================================================
// Create / Update / Pause
// =============================================================================

/// Argumente von [`recurring_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringCreateArgs {
    pub input: RecurringInput,
}

pub async fn recurring_create(
    ctx: &Arc<AppCtx>,
    args: RecurringCreateArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringCreateArgs { input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    let row = recurring::create(&pool, &input).await?;
    audit_log::append(
        &pool,
        "recurring.create",
        "recurring",
        &row.id,
        Some(serde_json::json!({ "label": &row.label, "frequency": &row.frequency, "auto": row.auto_create_expense == 1 })),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`recurring_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringUpdateArgs {
    pub id: String,
    pub input: RecurringInput,
}

pub async fn recurring_update(
    ctx: &Arc<AppCtx>,
    args: RecurringUpdateArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringUpdateArgs { id, input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    // W14-022: nie gleichzeitig mit einem Lauf auf demselben Abo.
    let _guard = crate::locks::RECURRING_SUBSCRIPTIONS.lock(&id).await;
    let row = recurring::update(&pool, &id, &input).await?;
    // Ein neuer Stichtag überholt eine Bestätigung zum Nachholen.
    catch_up::clear(&pool, Series::Subscription, &id).await?;
    audit_log::append(
        &pool,
        "recurring.update",
        "recurring",
        &row.id,
        Some(serde_json::json!({ "label": &row.label, "frequency": &row.frequency, "auto": row.auto_create_expense == 1 })),
    )
    .await?;
    Ok(row)
}

/// Argumente von [`recurring_set_active`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringSetActiveArgs {
    pub id: String,
    pub active: bool,
    /// Pflicht beim Fortsetzen eines Auto-Abos mit verpassten Terminen
    /// (`next_due_date` vor heute): nachholen oder ab heute fortsetzen (W14-026).
    #[serde(default)]
    pub resume_mode: Option<ResumeMode>,
}

pub async fn recurring_set_active(
    ctx: &Arc<AppCtx>,
    args: RecurringSetActiveArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringSetActiveArgs {
        id,
        active,
        resume_mode,
    } = args;
    let pool = ctx.pool()?;
    let today = ctx.clock().today_berlin();

    // W14-022: unter der Abo-Sperre (kein Lauf dazwischen).
    let _guard = crate::locks::RECURRING_SUBSCRIPTIONS.lock(&id).await;
    let sub = recurring::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))?;
    let mut applied = None;
    if active
        && sub.active == 0
        && sub.auto_create_expense == 1
        && has_backlog(&pool, &sub, today).await?
    {
        let mode = resume_mode.ok_or_else(|| {
            Error::Domain(format!(
                "Das Abo hat verpasste Termine seit {}. Bitte wählen: nachholen oder ab heute \
                 fortsetzen.",
                sub.next_due_date
            ))
        })?;
        apply_resume_mode(&pool, &sub, mode, today).await?;
        applied = Some(mode);
    }
    recurring::set_active(&pool, &id, active).await?;
    audit_log::append(
        &pool,
        if active {
            "recurring.activate"
        } else {
            "recurring.deactivate"
        },
        "recurring",
        &id,
        applied.map(|m| serde_json::json!({ "resume_mode": m })),
    )
    .await?;
    recurring::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))
}

/// Argumente von [`recurring_resolve_backlog`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringResolveBacklogArgs {
    pub id: String,
    pub mode: ResumeMode,
}

/// Entscheidung zu verpassten Terminen eines aktiven Abos (Hinweis
/// „Entscheidung nötig“, W14-026/-034): nachholen (Bestätigung für den
/// nächsten Lauf) oder ab heute fortsetzen.
pub async fn recurring_resolve_backlog(
    ctx: &Arc<AppCtx>,
    args: RecurringResolveBacklogArgs,
) -> Result<RecurringSubscriptionRow> {
    let RecurringResolveBacklogArgs { id, mode } = args;
    let pool = ctx.pool()?;
    let today = ctx.clock().today_berlin();
    let _guard = crate::locks::RECURRING_SUBSCRIPTIONS.lock(&id).await;
    let sub = recurring::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))?;
    if sub.active == 0 {
        return Err(Error::Domain(
            "Das Abo ist pausiert — die Wahl erfolgt beim Fortsetzen.".into(),
        ));
    }
    apply_resume_mode(&pool, &sub, mode, today).await?;
    recurring::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))
}

/// Verpasste Termine (Stichtag vor heute) oder eine offene Prüfung nach einer
/// Wiederherstellung (F1d: auch ein heute fälliger Termin verlangt dann die
/// Wahl)?
async fn has_backlog(
    pool: &sqlx::SqlitePool,
    sub: &RecurringSubscriptionRow,
    today: chrono::NaiveDate,
) -> Result<bool> {
    let Ok(next) = chrono::NaiveDate::parse_from_str(&sub.next_due_date, "%Y-%m-%d") else {
        return Ok(false);
    };
    let review =
        catch_up::restore_review_through(pool, Series::Subscription, &sub.id, &sub.next_due_date)
            .await?
            .is_some();
    // ADR 0042 Nr. 16: dieselbe Regel wie die Rückstand-Vorschau (Kosten-Abos
    // ohne Laufzeit-Ende).
    Ok(domain::has_backlog(next, today, None, review))
}

/// Argumente von [`recurring_restore_review_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringRestoreReviewListArgs {}

/// Reihen mit offener Prüfung nach einer Wiederherstellung (F1d).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReviewList {
    /// IDs der Abo-Rechnungs-Vorlagen.
    pub templates: Vec<String>,
    /// IDs der Kosten-Abos.
    pub subscriptions: Vec<String>,
}

/// Welche Reihen warten nach einer Wiederherstellung auf die Entscheidung
/// „Rückstand …“ (Stand der ersetzten Datenbank unbekannt, F1d)? Die Listen
/// zeigen dafür den Knopf auch bei einem heute fälligen Termin.
pub async fn recurring_restore_review_list(
    ctx: &Arc<AppCtx>,
    _args: RecurringRestoreReviewListArgs,
) -> Result<RestoreReviewList> {
    let pool = ctx.pool()?;
    Ok(RestoreReviewList {
        templates: catch_up::restore_review_ids(&pool, Series::Template).await?,
        subscriptions: catch_up::restore_review_ids(&pool, Series::Subscription).await?,
    })
}

// =============================================================================
// Rückstand-Vorschau (ADR 0042 Nr. 16, V1-11)
// =============================================================================

/// Art der Reihe für [`recurring_backlog_preview`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BacklogSeries {
    /// Abo-Rechnungs-Vorlage.
    Template,
    /// Kosten-Abo.
    Subscription,
}

/// Argumente von [`recurring_backlog_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringBacklogPreviewArgs {
    pub kind: BacklogSeries,
    pub id: String,
}

/// Was „nachholen“ bzw. „ab heute fortsetzen“ mit einer Reihe täte — gerechnet
/// wie `apply_resume_mode` und `has_backlog` (Stand heute; der Server prüft bei
/// der Wahl erneut).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklogPreview {
    /// Nächster offener Termin der Reihe.
    pub next_due_date: NaiveDate,
    /// Verlangt die Reihe die Wahl (wie `has_backlog`)?
    pub backlog: bool,
    /// Fällige Termine bis heute (innerhalb der Laufzeit), gezählt bis
    /// [`catch_up::COUNT_CAP`] — die Zahl, an der „nachholen“ scheitert.
    pub due: usize,
    /// Die ersten fälligen Termine, höchstens [`catch_up::CONFIRMED_CAP`].
    pub dates: Vec<NaiveDate>,
    /// Höchstzahl beim Nachholen ([`catch_up::CONFIRMED_CAP`]).
    pub cap: usize,
    /// Mehr Termine als `cap`: „nachholen“ wird abgelehnt.
    pub exceeds_cap: bool,
    /// Prüfung nach einer Wiederherstellung offen (nachgeholte Rechnungen nur
    /// als Entwurf, „ab heute fortsetzen“ lässt auch den heutigen Termin aus).
    pub after_restore: bool,
    /// Nächster Termin nach „ab heute fortsetzen“.
    pub skip_to: NaiveDate,
    /// So viele Termine lässt „ab heute fortsetzen“ aus.
    pub skipped: usize,
    /// „Ab heute fortsetzen“ führt hinter das Laufzeit-Ende (Vorlage endet).
    pub skip_ends_series: bool,
}

/// Rückstand-Vorschau aus den Daten einer Reihe (pure; dieselben Funktionen
/// und Grenzen wie `apply_resume_mode` und `has_backlog`).
pub fn backlog_preview(
    frequency: domain::Frequency,
    day_of_period: u32,
    next_due: NaiveDate,
    end: Option<NaiveDate>,
    today: NaiveDate,
    after_restore: bool,
) -> BacklogPreview {
    let mut dates = domain::due_occurrences(
        frequency,
        day_of_period,
        next_due,
        today,
        end,
        catch_up::COUNT_CAP,
    );
    let due = dates.len();
    dates.truncate(catch_up::CONFIRMED_CAP);
    let (skip_to, skipped) =
        domain::skip_target(frequency, day_of_period, next_due, today, after_restore);
    BacklogPreview {
        next_due_date: next_due,
        backlog: domain::has_backlog(next_due, today, end, after_restore),
        due,
        dates,
        cap: catch_up::CONFIRMED_CAP,
        exceeds_cap: due > catch_up::CONFIRMED_CAP,
        after_restore,
        skip_to,
        skipped,
        skip_ends_series: end.is_some_and(|e| skip_to > e),
    }
}

/// Testbarer Kern von [`recurring_backlog_preview`] (`today` = Europe/Berlin
/// aus der Uhr des Aufrufers).
pub async fn backlog_preview_core(
    pool: &sqlx::SqlitePool,
    kind: BacklogSeries,
    id: &str,
    today: NaiveDate,
) -> Result<BacklogPreview> {
    let (series, frequency, day, next_due, end) = match kind {
        BacklogSeries::Template => {
            let t = crate::db::repo::recurring_invoice::get_row(pool, id)
                .await?
                .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))?;
            (
                Series::Template,
                t.frequency,
                t.day_of_period,
                t.next_due_date,
                t.end_date,
            )
        }
        BacklogSeries::Subscription => {
            let s = recurring::get(pool, id)
                .await?
                .ok_or_else(|| Error::Domain(format!("Abo nicht gefunden: {id}")))?;
            // Kosten-Abos haben kein Laufzeit-Ende (wie im Lauf).
            (
                Series::Subscription,
                s.frequency,
                s.day_of_period,
                s.next_due_date,
                None,
            )
        }
    };
    let freq = domain::Frequency::from_db(&frequency)
        .ok_or_else(|| Error::Domain(format!("Ungültige Frequenz '{frequency}'")))?;
    let next = parse_iso(&next_due)?;
    let end = end.as_deref().map(parse_iso).transpose()?;
    let after_restore = catch_up::restore_review_through(pool, series, id, &next_due)
        .await?
        .is_some();
    Ok(backlog_preview(
        freq,
        day.clamp(1, 31) as u32,
        next,
        end,
        today,
        after_restore,
    ))
}

/// Rückstand-Vorschau für den Dialog „nachholen oder ab heute fortsetzen“
/// (ADR 0042 Nr. 16, V1-11): Anzahl und Daten der offenen Termine, Obergrenze,
/// Prüfung nach einer Wiederherstellung. Liest nur.
pub async fn recurring_backlog_preview(
    ctx: &Arc<AppCtx>,
    args: RecurringBacklogPreviewArgs,
) -> Result<BacklogPreview> {
    let RecurringBacklogPreviewArgs { kind, id } = args;
    let pool = ctx.pool()?;
    backlog_preview_core(&pool, kind, &id, ctx.clock().today_berlin()).await
}

fn parse_iso(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("Ungültiges Datum '{s}': {e}")))
}

// =============================================================================
// Auslösen (manuell einzeln + Due-Check für alle)
// =============================================================================

/// Argumente von [`recurring_run_now`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringRunNowArgs {
    pub id: String,
}

/// „Jetzt erfassen" für ein fälliges Abo — legt eine Kosten-Position für den
/// aktuellen Stichtag an und rückt das Abo um eine Periode vor. Liefert die
/// erzeugte Kosten-Position (das Frontend navigiert dorthin).
pub async fn recurring_run_now(ctx: &Arc<AppCtx>, args: RecurringRunNowArgs) -> Result<ExpenseRow> {
    let RecurringRunNowArgs { id } = args;
    let pool = ctx.pool()?;
    let clock = ctx.clock();
    run_now(&JobCtx::from_app(ctx, &pool, clock.as_ref()), &id).await
}

/// Argumente von [`recurring_run_due_check`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringRunDueCheckArgs {}

/// Manueller Due-Check für ALLE Auto-Abos (gleiche Logik und Grenzen wie der
/// Scheduler-Tick, Button „Alle fälligen buchen“).
pub async fn recurring_run_due_check(
    ctx: &Arc<AppCtx>,
    _args: RecurringRunDueCheckArgs,
) -> Result<ProcessReport> {
    let pool = ctx.pool()?;
    let clock = ctx.clock();
    process_due(&JobCtx::from_app(ctx, &pool, clock.as_ref())).await
}

// =============================================================================
// Helpers
// =============================================================================

fn validate(input: &RecurringInput) -> Result<()> {
    if let Err(errs) = domain::validate_recurring(input) {
        return Err(Error::Domain(format!(
            "Abo kann nicht gespeichert werden: {}",
            errs.iter()
                .map(domain::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
