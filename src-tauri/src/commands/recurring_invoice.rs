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

use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::backup;
use crate::config::Paths;
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

#[tauri::command]
pub async fn recurring_invoices_list(
    pool: State<'_, SqlitePool>,
    include_inactive: Option<bool>,
) -> Result<Vec<RecurringInvoiceRow>> {
    recurring_invoice::list(pool.inner(), include_inactive.unwrap_or(false)).await
}

#[tauri::command]
pub async fn recurring_invoices_get(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<RecurringInvoiceDetail>> {
    recurring_invoice::get(pool.inner(), &id).await
}

// =============================================================================
// Create / Update / Pause
// =============================================================================

#[tauri::command]
pub async fn recurring_invoices_create(
    pool: State<'_, SqlitePool>,
    input: RecurringInvoiceInput,
) -> Result<RecurringInvoiceDetail> {
    validate(&input)?;
    assert_no_vat_when_kleinunternehmer(pool.inner(), &input).await?;
    let detail = recurring_invoice::create(pool.inner(), &input).await?;
    audit_log::append(
        pool.inner(),
        "recurring_invoice.create",
        "recurring_invoice",
        &detail.template.id,
        Some(&audit_details(&detail.template)),
    )
    .await?;
    Ok(detail)
}

#[tauri::command]
pub async fn recurring_invoices_update(
    pool: State<'_, SqlitePool>,
    id: String,
    input: RecurringInvoiceInput,
) -> Result<RecurringInvoiceDetail> {
    validate(&input)?;
    assert_no_vat_when_kleinunternehmer(pool.inner(), &input).await?;
    let detail = recurring_invoice::update(pool.inner(), &id, &input).await?;
    audit_log::append(
        pool.inner(),
        "recurring_invoice.update",
        "recurring_invoice",
        &detail.template.id,
        Some(&audit_details(&detail.template)),
    )
    .await?;
    Ok(detail)
}

#[tauri::command]
pub async fn recurring_invoices_set_active(
    pool: State<'_, SqlitePool>,
    id: String,
    active: bool,
) -> Result<RecurringInvoiceRow> {
    recurring_invoice::set_active(pool.inner(), &id, active).await?;
    audit_log::append(
        pool.inner(),
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
    recurring_invoice::get_row(pool.inner(), &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Vorlage nicht gefunden: {id}")))
}

// =============================================================================
// Auslösen (manuell einzeln + Due-Check für alle)
// =============================================================================

/// „Jetzt erstellen" für eine fällige Vorlage — legt eine Rechnung für den
/// aktuellen Stichtag an (je `auto_mode` Entwurf oder festgeschrieben) und rückt
/// die Vorlage um eine Periode vor. Liefert die Rechnungs-ID (Frontend navigiert
/// dorthin).
#[tauri::command]
pub async fn recurring_invoices_run_now(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
    id: String,
) -> Result<String> {
    let paths = Paths::from_handle(&app)?;
    run_now(
        pool.inner(),
        &paths,
        session.inner(),
        Some(&app),
        &id,
        today_berlin(),
    )
    .await
}

/// Manueller Due-Check für ALLE fälligen Vorlagen (gleiche Logik wie der
/// Scheduler-Tick). Nützlich direkt nach dem Entsperren, ohne auf den nächsten
/// Tick zu warten.
#[tauri::command]
pub async fn recurring_invoices_run_due_check(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, backup::BackupSession>,
) -> Result<ProcessReport> {
    let paths = Paths::from_handle(&app)?;
    process_due(
        pool.inner(),
        &paths,
        session.inner(),
        Some(&app),
        today_berlin(),
    )
    .await
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

/// Heutiges Datum (Europe/Berlin = System-TZ, in Block 0 gepinnt).
fn today_berlin() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
