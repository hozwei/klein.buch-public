//! Commands für wiederkehrende Ausgangsrechnungen (Block RI-3, Lauf F).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::recurring_invoice::validate_recurring_invoice`]).
//! - CRUD + Pausieren ([`crate::db::repo::recurring_invoice`]).
//! - Manuelle/automatische Materialisierung ([`crate::scheduler::recurring_invoice`]).
//! - Audit-Log.
//!
//! ## Abgrenzung
//!
//! Eine Vorlage ist ein Stammdatum/Template — editierbar und pausierbar (kein
//! GoBD-Beleg). Die daraus erzeugte Rechnung ist nach dem Festschreiben
//! unveränderlich (invoices-Trigger). Belegdatum = Erstellungstag.

use crate::app_ctx::AppCtx;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::db::models::{RecurringInvoiceDetail, RecurringInvoiceRow};
use crate::db::repo::{audit_log, recurring_invoice, seller_profile};
use crate::domain::invoice::{compute_totals, InvoiceItemInput};
use crate::domain::kleinunternehmer::{self, ItemVatCheck};
use crate::domain::recurring_invoice::{self as domain, RecurringInvoiceInput};
use crate::error::{Error, Result};
use crate::scheduler::catch_up::ResumeMode;
use crate::scheduler::recurring_invoice::{apply_resume_mode, process_due, run_now, ProcessReport};
use crate::scheduler::JobCtx;

// =============================================================================
// Read
// =============================================================================

/// Argumente von [`recurring_invoices_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesListArgs {
    #[serde(default)]
    pub include_inactive: Option<bool>,
}

pub async fn recurring_invoices_list(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesListArgs,
) -> Result<Vec<RecurringInvoiceRow>> {
    let RecurringInvoicesListArgs { include_inactive } = args;
    let pool = ctx.pool()?;

    recurring_invoice::list(&pool, include_inactive.unwrap_or(false)).await
}

/// Argumente von [`recurring_invoices_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesGetArgs {
    pub id: String,
}

pub async fn recurring_invoices_get(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesGetArgs,
) -> Result<Option<RecurringInvoiceDetail>> {
    let RecurringInvoicesGetArgs { id } = args;
    let pool = ctx.pool()?;

    recurring_invoice::get(&pool, &id).await
}

// =============================================================================
// Create / Update / Pause
// =============================================================================

/// Argumente von [`recurring_invoices_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesCreateArgs {
    pub input: RecurringInvoiceInput,
}

pub async fn recurring_invoices_create(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesCreateArgs,
) -> Result<RecurringInvoiceDetail> {
    let RecurringInvoicesCreateArgs { mut input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    // KB-0046 (F5-fix): Paket-Provenienz auch an Abo-Vorlagen erzwingen.
    crate::commands::packages::enforce_package_provenance(&pool, &mut input.items).await?;
    // R1-S2-NEXTDUE: Eine neue Vorlage beginnt nicht in der Vergangenheit —
    // sonst holte der nächste Lauf ohne Rückfrage Rechnungen für vergangene
    // Zeiträume nach.
    let today = ctx.clock().today_berlin();
    if input.next_due_date < today {
        return Err(Error::Domain(format!(
            "Die erste Fälligkeit ({}) liegt in der Vergangenheit. Bitte heute oder ein späteres \
             Datum wählen; Rechnungen für vergangene Zeiträume bitte einzeln anlegen.",
            input.next_due_date.format("%d.%m.%Y")
        )));
    }
    // ADR 0042 Nr. 3/10: Status am Ende des ersten Abrechnungszeitraums.
    check_items_no_vat(&pool, &input.items, first_status_day(&input)).await?;
    let detail = recurring_invoice::create(&pool, &input).await?;
    audit_log::append(
        &pool,
        "recurring_invoice.create",
        "recurring_invoice",
        &detail.template.id,
        Some(audit_details(&detail.template)),
    )
    .await?;
    Ok(detail)
}

/// Argumente von [`recurring_invoices_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesUpdateArgs {
    pub id: String,
    pub input: RecurringInvoiceInput,
}

pub async fn recurring_invoices_update(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesUpdateArgs,
) -> Result<RecurringInvoiceDetail> {
    let RecurringInvoicesUpdateArgs { id, mut input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    crate::commands::packages::enforce_package_provenance(&pool, &mut input.items).await?;
    check_items_no_vat(&pool, &input.items, first_status_day(&input)).await?;
    // W14-031: nie gleichzeitig mit einem Lauf auf derselben Vorlage.
    let _guard = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
    let detail = recurring_invoice::update(&pool, &id, &input).await?;
    audit_log::append(
        &pool,
        "recurring_invoice.update",
        "recurring_invoice",
        &detail.template.id,
        Some(audit_details(&detail.template)),
    )
    .await?;
    Ok(detail)
}

/// Argumente von [`recurring_invoices_set_active`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesSetActiveArgs {
    pub id: String,
    pub active: bool,
    /// Pflicht beim Fortsetzen einer Vorlage mit verpassten Terminen
    /// (`next_due_date` vor heute): nachholen oder ab heute fortsetzen (W14-026).
    #[serde(default)]
    pub resume_mode: Option<ResumeMode>,
}

pub async fn recurring_invoices_set_active(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesSetActiveArgs,
) -> Result<RecurringInvoiceRow> {
    let RecurringInvoicesSetActiveArgs {
        id,
        active,
        resume_mode,
    } = args;
    let pool = ctx.pool()?;
    let today = ctx.clock().today_berlin();

    // W14-031: unter der Vorlagen-Sperre — ein laufender Termin wird fertig,
    // danach wirkt die Pause.
    let _guard = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
    let tmpl = recurring_invoice::get_row(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))?;
    let mut applied = None;
    if active && tmpl.active == 0 && has_backlog(&pool, &tmpl, today).await? {
        let mode = resume_mode.ok_or_else(|| {
            Error::Domain(format!(
                "Die Vorlage hat verpasste Termine seit {}. Bitte wählen: nachholen (ältere \
                 Termine als Entwurf) oder ab heute fortsetzen.",
                tmpl.next_due_date
            ))
        })?;
        apply_resume_mode(&pool, &tmpl, mode, today).await?;
        applied = Some(mode);
    }
    recurring_invoice::set_active(&pool, &id, active).await?;
    audit_log::append(
        &pool,
        if active {
            "recurring_invoice.activate"
        } else {
            "recurring_invoice.deactivate"
        },
        "recurring_invoice",
        &id,
        applied.map(|m| serde_json::json!({ "resume_mode": m })),
    )
    .await?;
    recurring_invoice::get_row(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))
}

/// Argumente von [`recurring_invoices_resolve_backlog`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesResolveBacklogArgs {
    pub id: String,
    pub mode: ResumeMode,
}

/// Entscheidung zu verpassten Terminen einer aktiven Vorlage (Hinweis
/// „Entscheidung nötig“, W14-026): nachholen (Bestätigung für den nächsten
/// Lauf, ältere Termine als Entwurf) oder ab heute fortsetzen.
pub async fn recurring_invoices_resolve_backlog(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesResolveBacklogArgs,
) -> Result<RecurringInvoiceRow> {
    let RecurringInvoicesResolveBacklogArgs { id, mode } = args;
    let pool = ctx.pool()?;
    let today = ctx.clock().today_berlin();
    let _guard = crate::locks::RECURRING_TEMPLATES.lock(&id).await;
    let tmpl = recurring_invoice::get_row(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))?;
    if tmpl.active == 0 {
        return Err(Error::Domain(
            "Die Vorlage ist pausiert — die Wahl erfolgt beim Fortsetzen.".into(),
        ));
    }
    apply_resume_mode(&pool, &tmpl, mode, today).await?;
    recurring_invoice::get_row(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))
}

/// Verpasste Termine (Stichtag vor heute, innerhalb der Laufzeit) oder eine
/// offene Prüfung nach einer Wiederherstellung (F1d: auch ein heute fälliger
/// Termin verlangt dann die Wahl)?
async fn has_backlog(
    pool: &SqlitePool,
    tmpl: &RecurringInvoiceRow,
    today: chrono::NaiveDate,
) -> Result<bool> {
    let Ok(next) = chrono::NaiveDate::parse_from_str(&tmpl.next_due_date, "%Y-%m-%d") else {
        return Ok(false);
    };
    let end = tmpl
        .end_date
        .as_deref()
        .and_then(|e| chrono::NaiveDate::parse_from_str(e, "%Y-%m-%d").ok());
    let review = crate::scheduler::catch_up::restore_review_through(
        pool,
        crate::scheduler::catch_up::Series::Template,
        &tmpl.id,
        &tmpl.next_due_date,
    )
    .await?
    .is_some();
    // ADR 0042 Nr. 16: dieselbe Regel wie die Rückstand-Vorschau.
    Ok(crate::domain::recurring::has_backlog(
        next, today, end, review,
    ))
}

// =============================================================================
// Auslösen (manuell einzeln + Due-Check für alle)
// =============================================================================

/// Argumente von [`recurring_invoices_run_now`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesRunNowArgs {
    pub id: String,
}

/// „Jetzt erstellen" für eine fällige Vorlage — legt eine Rechnung für den
/// aktuellen Stichtag an (je `auto_mode` Entwurf oder festgeschrieben) und rückt
/// die Vorlage um eine Periode vor. Liefert die Rechnungs-ID (Frontend navigiert
/// dorthin).
pub async fn recurring_invoices_run_now(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesRunNowArgs,
) -> Result<String> {
    let RecurringInvoicesRunNowArgs { id } = args;
    let pool = ctx.pool()?;
    let clock = ctx.clock();
    run_now(&JobCtx::from_app(ctx, &pool, clock.as_ref()), &id).await
}

/// Argumente von [`recurring_invoices_run_due_check`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesRunDueCheckArgs {}

/// Manueller Due-Check für ALLE fälligen Vorlagen (gleiche Logik und Grenzen
/// wie der Scheduler-Tick, Button „Alle fälligen erstellen“). Die Seite löst
/// ihn nicht mehr beim Öffnen aus (W14-030).
pub async fn recurring_invoices_run_due_check(
    ctx: &Arc<AppCtx>,
    _args: RecurringInvoicesRunDueCheckArgs,
) -> Result<ProcessReport> {
    let pool = ctx.pool()?;
    let clock = ctx.clock();
    process_due(&JobCtx::from_app(ctx, &pool, clock.as_ref())).await
}

// =============================================================================
// Helpers
// =============================================================================

/// §19-Status-Tag der ersten Rechnung einer Vorlage (ADR 0042 Nr. 3/10): Ende
/// des ersten Abrechnungszeitraums.
fn first_status_day(input: &RecurringInvoiceInput) -> chrono::NaiveDate {
    crate::domain::recurring::status_day_for(
        &input.frequency,
        input.day_of_period,
        input.next_due_date,
    )
}

fn validate(input: &RecurringInvoiceInput) -> Result<()> {
    if let Err(errs) = domain::validate_recurring_invoice(input) {
        return Err(Error::Domain(format!(
            "Vorlage kann nicht gespeichert werden: {}",
            errs.iter()
                .map(domain::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    Ok(())
}

/// KB-0053 / R1-017: §14c-Schutz für Vorlagen-Positionen — beim Speichern
/// (Defense-in-Depth unter der UI-Sperre) und vor jeder Materialisierung im
/// Scheduler. Bei aktivem §19 dürfen die Positionen keine USt tragen — sonst
/// entstünden nummerierte Entwürfe, die erst beim Festschreiben (`assert_no_vat`
/// in der Lock-Pipeline) scheitern. Nutzt denselben Domain-Check wie der
/// Rechnungsweg (`kleinunternehmer::assert_no_vat`) mit dem §19-Status am
/// Termin `date` (Leistungsdatum der Rechnung, ADR 0042 Nr. 3): ein
/// nachgeholter Termin vor einem Wechsel bleibt §19, einer danach nicht.
///
/// Umgekehrt (KB-0014, F5-fix): Bei Regelbesteuerung darf keine Position die
/// Steuerart E (§19) tragen — eine unter §19 gespeicherte Vorlage erzeugte nach
/// dem Verzicht sonst je Termin einen nummerierten Entwurf, der beim
/// Festschreiben scheitert. Der Scheduler meldet die Vorlage dann als
/// übersprungen (Hinweis, keine Nummer), bis sie umgestellt ist.
pub(crate) async fn check_items_no_vat(
    pool: &SqlitePool,
    items: &[InvoiceItemInput],
    date: chrono::NaiveDate,
) -> Result<()> {
    // Onboarding erzwingt das Verkäuferprofil vor der ersten Rechnung; solange
    // keins existiert, ist der §19-Status nicht bestimmbar → kein Block.
    if seller_profile::get(pool).await?.is_none() {
        return Ok(());
    }
    let is_klein = crate::db::repo::tax_status::is_klein_at(pool, date).await?;
    if !is_klein {
        let mut exempt: Vec<u32> = items
            .iter()
            .filter(|it| it.tax_category_code == kleinunternehmer::EXEMPT_CATEGORY)
            .map(|it| it.position)
            .collect();
        if !exempt.is_empty() {
            exempt.sort_unstable();
            exempt.dedup();
            let list = exempt
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::Domain(format!(
                "Bei Regelbesteuerung (Termin {}) gibt es die Steuerart E (§19) nicht. \
                 Betroffene Position(en): {list}. Bitte die Vorlage auf Steuerart S und den Steuersatz \
                 (z. B. 19 %) umstellen.",
                date.format("%d.%m.%Y")
            )));
        }
    }
    let checks: Vec<ItemVatCheck> = items
        .iter()
        .map(|it| ItemVatCheck {
            position: it.position,
            tax_category_code: it.tax_category_code.as_str(),
            tax_amount_cents: compute_totals(std::slice::from_ref(it)).tax_amount_cents,
            tax_rate_percent: it.tax_rate_percent,
        })
        .collect();
    if let Err(viol) = kleinunternehmer::assert_no_vat(is_klein, &checks) {
        let mut positions: Vec<u32> = viol.iter().map(|v| v.position).collect();
        positions.sort_unstable();
        positions.dedup();
        let list = positions
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::Domain(format!(
            "Im Kleinunternehmer-Modus (§19 UStG) dürfen Abo-Positionen keine Umsatzsteuer ausweisen. \
             Betroffene Position(en): {list}. Bitte Steuer-Kategorie 'E' und 0 % setzen."
        )));
    }
    Ok(())
}

fn audit_details(t: &RecurringInvoiceRow) -> serde_json::Value {
    serde_json::json!({
        "label": &t.label,
        "frequency": &t.frequency,
        "mode": &t.auto_mode,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
