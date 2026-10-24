//! Tauri-Commands für wiederkehrende Ausgangsrechnungen (Block RI-3).
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
use crate::domain::invoice::compute_totals;
use crate::domain::kleinunternehmer::{self, ItemVatCheck, KleinunternehmerStatus};
use crate::domain::recurring_invoice::{self as domain, RecurringInvoiceInput};
use crate::error::{Error, Result};
use crate::scheduler::recurring_invoice::{process_due, run_now, ProcessReport};

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
    let RecurringInvoicesCreateArgs { input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    assert_no_vat_when_kleinunternehmer(&pool, &input).await?;
    let detail = recurring_invoice::create(&pool, &input).await?;
    audit_log::append(
        &pool,
        "recurring_invoice.create",
        "recurring_invoice",
        &detail.template.id,
        Some(&audit_details(&detail.template)),
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
    let RecurringInvoicesUpdateArgs { id, input } = args;
    let pool = ctx.pool()?;

    validate(&input)?;
    assert_no_vat_when_kleinunternehmer(&pool, &input).await?;
    let detail = recurring_invoice::update(&pool, &id, &input).await?;
    audit_log::append(
        &pool,
        "recurring_invoice.update",
        "recurring_invoice",
        &detail.template.id,
        Some(&audit_details(&detail.template)),
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
}

pub async fn recurring_invoices_set_active(
    ctx: &Arc<AppCtx>,
    args: RecurringInvoicesSetActiveArgs,
) -> Result<RecurringInvoiceRow> {
    let RecurringInvoicesSetActiveArgs { id, active } = args;
    let pool = ctx.pool()?;

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
        None,
    )
    .await?;
    recurring_invoice::get_row(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))
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
    let session = &ctx.session;

    let paths = ctx.paths.clone();
    run_now(&pool, &paths, session, ctx.notifier(), &id, today_berlin()).await
}

/// Argumente von [`recurring_invoices_run_due_check`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringInvoicesRunDueCheckArgs {}

/// Manueller Due-Check für ALLE fälligen Vorlagen (gleiche Logik wie der
/// Scheduler-Tick). Nützlich direkt nach dem Entsperren, ohne auf den nächsten
/// Tick zu warten.
pub async fn recurring_invoices_run_due_check(
    ctx: &Arc<AppCtx>,
    _args: RecurringInvoicesRunDueCheckArgs,
) -> Result<ProcessReport> {
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let paths = ctx.paths.clone();
    process_due(&pool, &paths, session, ctx.notifier(), today_berlin()).await
}

// =============================================================================
// Helpers
// =============================================================================

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

/// KB-0053: §14c-Schutz beim Speichern der Vorlage (Defense-in-Depth unter der
/// UI-Sperre). Bei aktivem §19 dürfen die Vorlagen-Positionen keine USt tragen —
/// sonst entstünden daraus bei jeder Materialisierung USt-behaftete Entwürfe, die
/// erst beim Festschreiben (`assert_no_vat` in der Lock-Pipeline) scheitern.
/// Nutzt denselben Domain-Check wie der Rechnungsweg (`kleinunternehmer::assert_no_vat`).
async fn assert_no_vat_when_kleinunternehmer(
    pool: &SqlitePool,
    input: &RecurringInvoiceInput,
) -> Result<()> {
    // Onboarding erzwingt das Verkäuferprofil vor der ersten Rechnung; solange
    // keins existiert, ist der §19-Status nicht bestimmbar → kein Block.
    let Some(seller) = seller_profile::get(pool).await? else {
        return Ok(());
    };
    let status = KleinunternehmerStatus {
        is_kleinunternehmer: seller.is_kleinunternehmer != 0,
        waived_since: seller
            .waived_paragraph_19_since
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
    };
    let checks: Vec<ItemVatCheck> = input
        .items
        .iter()
        .map(|it| ItemVatCheck {
            position: it.position,
            tax_category_code: it.tax_category_code.as_str(),
            tax_amount_cents: compute_totals(std::slice::from_ref(it)).tax_amount_cents,
            tax_rate_percent: it.tax_rate_percent,
        })
        .collect();
    if let Err(viol) = kleinunternehmer::assert_no_vat(&status, &checks) {
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

fn audit_details(t: &RecurringInvoiceRow) -> String {
    format!(
        r#"{{"label":"{}","frequency":"{}","mode":"{}"}}"#,
        esc(&t.label),
        esc(&t.frequency),
        esc(&t.auto_mode)
    )
}

/// Heutiges Datum in Europe/Berlin (`crate::clock`, Plan W1.8).
fn today_berlin() -> chrono::NaiveDate {
    crate::clock::today_berlin()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
