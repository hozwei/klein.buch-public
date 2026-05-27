//! Abo-Rechnungen am Stichtag materialisieren (Imperative Shell) — Block RI-2.
//!
//! Erzeugt für fällige Abo-Vorlagen ([`crate::db::repo::recurring_invoice`])
//! echte Ausgangsrechnungen über die **bestehende** Pipeline
//! ([`create_invoice_draft_from_input`] + [`run_lock_pipeline`]). §19-Klausel,
//! Pflichtangaben-Check, XRechnung, PDF, KoSIT, Archiv und Lock kommen dadurch
//! 1:1 aus dem normalen Rechnungsweg — hier nur die Orchestrierung.
//!
//! ## Verhalten (mit Manuel abgestimmt)
//!
//! - **Belegdatum = Erstellungstag (heute), nie rückdatiert** (§14 Abs. 4 Nr. 3
//!   UStG + GoBD: zeitnahe, lückenlose Nummern). Der Leistungszeitraum der
//!   Periode geht ins `delivery_date` (Leistungsdatum) und — bei
//!   `service_period_note` — als Klartext in die erste Positionsbeschreibung.
//! - **Catch-up:** War die App länger zu, wird pro verpasster Periode eine
//!   Rechnung angelegt — alle mit **heutigem** Belegdatum, jeweils mit dem
//!   korrekten Periodendatum als Leistungsdatum. [`CATCH_UP_CAP`] schützt vor
//!   Endlosschleifen; `end_date` stoppt die Reihe.
//! - **Automatik-Stufe je Vorlage** ([`AutoMode`]):
//!   - `draft`      → Entwurf anlegen + Hinweis (prüfungssicher).
//!   - `issue`      → Entwurf anlegen + automatisch festschreiben (volle Pipeline).
//!   - `issue_send` → wie `issue` + automatischer Versand über das Standard-Mail-Konto
//!     (best-effort; Versandfehler/kein Konto → Hinweis, Beleg bleibt festgeschrieben).
//! - **Nummern-Sicherheit:** Die Nummer wird beim Draft-Anlegen vergeben
//!   (`create_invoice_draft_from_input`). Schlägt das spätere Festschreiben fehl,
//!   bleibt der nummerierte Entwurf bestehen (erneut festschreibbar) — keine
//!   Nummernlücke, identisch zum manuell angelegten Entwurf.
//! - **Unlock-Gate + ein Backup pro Burst** wie beim Kosten-Abo.

use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::backup::{self, BackupSession};
use crate::commands::invoices::{create_invoice_draft_from_input, run_lock_pipeline};
use crate::commands::mail::send_invoice_core;
use crate::config::Paths;
use crate::db::models::{RecurringInvoiceItemRow, RecurringInvoiceRow};
use crate::db::repo::{audit_log, mail_accounts, recurring_invoice};
use crate::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use crate::domain::recurring::{compute_next_due_date, Frequency};
use crate::domain::recurring_invoice::AutoMode;
use crate::error::{Error, Result};
use crate::fiscal_year::guard;
use crate::notify::{self, NewNotification};

/// Sicherheits-Obergrenze für Catch-up je Vorlage und Lauf (10 Jahre monatlich
/// = 120). Schützt gegen Endlosschleife, falls die Stichtags-Rechnung wider
/// Erwarten nicht vorrückt.
pub const CATCH_UP_CAP: usize = 120;

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
    /// Anzahl Vorlagen, für die mindestens eine Rechnung erzeugt wurde.
    pub processed_templates: usize,
    /// Gesamtzahl erzeugter Rechnungen (Entwurf oder festgeschrieben).
    pub created_invoices: usize,
}

/// Verarbeitet alle fälligen Abo-Vorlagen zum Stichtag `today` (Europe/Berlin).
pub async fn process_due(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    app: Option<&AppHandle>,
    today: NaiveDate,
) -> Result<ProcessReport> {
    if !session.is_unlocked() {
        tracing::debug!("Abo-Rechnungen übersprungen: Backup-Session gesperrt");
        return Ok(ProcessReport {
            skipped_locked: true,
            ..Default::default()
        });
    }

    let due = recurring_invoice::list_due(pool, &today.to_string()).await?;
    let mut report = ProcessReport::default();

    for tmpl in &due {
        match catch_up(pool, paths, app, tmpl, today).await {
            Ok(0) => {}
            Ok(n) => {
                report.processed_templates += 1;
                report.created_invoices += n;
            }
            // Ein Fehler bei einer Vorlage darf die anderen nicht abbrechen.
            Err(e) => tracing::warn!("Abo-Rechnung {} fehlgeschlagen: {e}", tmpl.id),
        }
    }

    // Ein Backup deckt den gesamten Burst ab (Lock-Event).
    if report.created_invoices > 0 {
        backup::auto_backup_if_unlocked(pool, paths, session, "recurring_invoice.lock")
            .await
            .ok();
    }
    Ok(report)
}

/// Manueller Einzel-Trigger („Jetzt erstellen") für eine fällige Vorlage —
/// unabhängig vom `auto_mode`-Default der Vorlage (die Stufe bestimmt nur, ob
/// festgeschrieben wird). Legt **genau eine** Rechnung für den aktuellen Stichtag
/// an und rückt die Vorlage um eine Periode vor.
pub async fn run_now(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    app: Option<&AppHandle>,
    id: &str,
    today: NaiveDate,
) -> Result<String> {
    let tmpl = recurring_invoice::get_row(pool, id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Abo-Vorlage nicht gefunden: {id}")))?;
    if tmpl.active == 0 {
        return Err(Error::Domain(
            "Pausierte Vorlage: bitte erst fortsetzen, dann erstellen.".into(),
        ));
    }
    let items = recurring_invoice::items_for(pool, id).await?;
    if items.is_empty() {
        return Err(Error::Domain("Vorlage hat keine Positionen.".into()));
    }
    let (freq, day) = freq_and_day(&tmpl)?;
    let occurrence = parse_date(&tmpl.next_due_date)?;
    if occurrence > today {
        return Err(Error::Domain(format!(
            "Vorlage ist erst am {occurrence} fällig — eine vorzeitige Erstellung würde ein zukünftiges Periodendatum erzeugen."
        )));
    }

    let invoice_id = process_one(pool, paths, app, &tmpl, &items, occurrence, today).await?;
    let following = compute_next_due_date(freq, day, occurrence);
    recurring_invoice::advance(pool, &tmpl.id, &invoice_id, &following.to_string()).await?;

    backup::auto_backup_if_unlocked(pool, paths, session, "recurring_invoice.lock")
        .await
        .ok();
    Ok(invoice_id)
}

// ---- intern ----------------------------------------------------------------

/// Legt für eine Vorlage alle bis `today` fälligen Perioden nach. Liefert die
/// Anzahl erzeugter Rechnungen.
async fn catch_up(
    pool: &SqlitePool,
    paths: &Paths,
    app: Option<&AppHandle>,
    tmpl: &RecurringInvoiceRow,
    today: NaiveDate,
) -> Result<usize> {
    let items = recurring_invoice::items_for(pool, &tmpl.id).await?;
    if items.is_empty() {
        return Err(Error::Domain(format!(
            "Abo-Vorlage {} hat keine Positionen",
            tmpl.id
        )));
    }
    let (freq, day) = freq_and_day(tmpl)?;
    let end = tmpl
        .end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let mut next = parse_date(&tmpl.next_due_date)?;
    let mut created = 0usize;

    while next <= today && end.map(|e| next <= e).unwrap_or(true) && created < CATCH_UP_CAP {
        let invoice_id = process_one(pool, paths, app, tmpl, &items, next, today).await?;
        let following = compute_next_due_date(freq, day, next);
        recurring_invoice::advance(pool, &tmpl.id, &invoice_id, &following.to_string()).await?;
        next = following;
        created += 1;
    }

    if created == CATCH_UP_CAP {
        tracing::warn!(
            "Abo-Vorlage {} hat den Catch-up-Cap ({CATCH_UP_CAP}) erreicht — Rest beim nächsten Lauf",
            tmpl.id
        );
    }
    Ok(created)
}

/// Materialisiert genau eine Periode: Draft anlegen, je `auto_mode` festschreiben,
/// auditieren, benachrichtigen. Liefert die Rechnungs-ID (für `advance`).
async fn process_one(
    pool: &SqlitePool,
    paths: &Paths,
    app: Option<&AppHandle>,
    tmpl: &RecurringInvoiceRow,
    items: &[RecurringInvoiceItemRow],
    occurrence: NaiveDate,
    today: NaiveDate,
) -> Result<String> {
    let (input, fiscal_year) = build_invoice_input(tmpl, items, occurrence, today);

    // Draft über den normalen Weg — vergibt die Nummer + persistiert (Transaktion).
    let detail =
        create_invoice_draft_from_input(pool, &tmpl.contact_id, fiscal_year, &input, None).await?;
    let invoice_id = detail.invoice.id.clone();
    let number = detail.invoice.invoice_number.clone();

    audit_log::append(
        pool,
        "recurring_invoice.run",
        "recurring_invoice",
        &tmpl.id,
        Some(&format!(
            r#"{{"template":"{}","invoice_id":"{}","number":"{}","occurrence":"{}","mode":"{}"}}"#,
            esc(&tmpl.id),
            esc(&invoice_id),
            esc(&number),
            occurrence,
            esc(&tmpl.auto_mode)
        )),
    )
    .await?;

    let mode = AutoMode::from_db(&tmpl.auto_mode).unwrap_or(AutoMode::Draft);
    let dedup = format!("recurring_invoice:{}:{}", tmpl.id, occurrence);

    match mode {
        AutoMode::Draft => {
            emit_notice(
                pool,
                app,
                "Abo-Rechnung als Entwurf bereit",
                &format!(
                    "„{}“: Rechnungs-Entwurf {} angelegt — bitte prüfen und festschreiben.",
                    tmpl.label, number
                ),
                "info",
                &invoice_id,
                &dedup,
            )
            .await;
        }
        AutoMode::Issue | AutoMode::IssueSend => {
            // R2-006 (GJ-Guard): Auto-Lock niemals in ein festgeschriebenes
            // GJ. Bei Konflikt bleibt der Draft bestehen (manuell handhabbar)
            // und der User bekommt einen Hinweis — analog zu einem Lock-
            // Pipeline-Fehler, kein harter Tick-Abbruch.
            if guard::is_closed(pool, fiscal_year).await? {
                tracing::warn!(
                    "Abo-Rechnung {number}: Auto-Festschreiben übersprungen — GJ {fiscal_year} ist abgeschlossen"
                );
                emit_notice(
                    pool,
                    app,
                    "Abo-Rechnung: Auto-Festschreiben übersprungen",
                    &format!(
                        "„{}“: Entwurf {} liegt vor, kann aber nicht automatisch festgeschrieben werden, weil das Geschäftsjahr {} bereits abgeschlossen ist. Bitte den Beleg manuell prüfen.",
                        tmpl.label, number, fiscal_year
                    ),
                    "warning",
                    &invoice_id,
                    &dedup,
                )
                .await;
                return Ok(invoice_id);
            }
            match run_lock_pipeline(pool, paths, &invoice_id, "N/A").await {
                Ok(_) => {
                    if matches!(mode, AutoMode::IssueSend) {
                        try_auto_send(pool, paths, app, &tmpl.label, &invoice_id, &number, &dedup)
                            .await;
                    } else {
                        emit_notice(
                            pool,
                            app,
                            "Abo-Rechnung erstellt",
                            &format!(
                                "„{}“: Rechnung {} automatisch festgeschrieben.",
                                tmpl.label, number
                            ),
                            "info",
                            &invoice_id,
                            &dedup,
                        )
                        .await;
                    }
                }
                Err(e) => {
                    // Entwurf bleibt bestehen (nummeriert, erneut festschreibbar);
                    // kein Abbruch der Reihe — Periode gilt als verarbeitet.
                    tracing::warn!(
                        "Abo-Rechnung {number}: automatisches Festschreiben fehlgeschlagen: {e}"
                    );
                    emit_notice(
                        pool,
                        app,
                        "Abo-Rechnung: Festschreiben fehlgeschlagen",
                        &format!(
                            "„{}“: Entwurf {} liegt vor, konnte aber nicht automatisch festgeschrieben werden ({e}). Bitte manuell festschreiben.",
                            tmpl.label, number
                        ),
                        "warning",
                        &invoice_id,
                        &dedup,
                    )
                    .await;
                }
            }
        }
    }

    Ok(invoice_id)
}

/// Auto-Versand für `issue_send` nach erfolgreichem Festschreiben (best-effort).
/// Nutzt das Standard-Mail-Konto; der Empfänger kommt aus dem Buyer-Snapshot bzw.
/// Kontakt. Fehler rollen die festgeschriebene Rechnung NICHT zurück — sie werden
/// nur als Hinweis gemeldet. Genau EIN Hinweis je Periode (gleicher `dedup_key`).
async fn try_auto_send(
    pool: &SqlitePool,
    paths: &Paths,
    app: Option<&AppHandle>,
    label: &str,
    invoice_id: &str,
    number: &str,
    dedup: &str,
) {
    match mail_accounts::get_default(pool).await {
        Ok(Some(account)) => {
            match send_invoice_core(pool, paths, &account.id, invoice_id, None, None, None).await {
                Ok(_) => {
                    emit_notice(
                        pool,
                        app,
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
                    tracing::warn!("Abo-Rechnung {number}: Auto-Versand fehlgeschlagen: {e}");
                    emit_notice(
                        pool,
                        app,
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
                pool,
                app,
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
            tracing::warn!("Abo-Rechnung {number}: Mail-Konto-Abruf fehlgeschlagen: {e}");
            emit_notice(
                pool,
                app,
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
    pool: &SqlitePool,
    _app: Option<&AppHandle>,
    title: &str,
    body: &str,
    severity: &str,
    invoice_id: &str,
    dedup_key: &str,
) {
    // R4-007: Scheduler-Pfad nutzt **Inbox-only** via `notify::store::create`,
    // NICHT `notify::emit`. Begründung: `emit` zieht `os_native::push` →
    // Windows-TaskDialogIndirect, was in Integrationstests crashen kann
    // (G1-NOTIFY-Memory). Konsistent mit `backup/mod.rs::notify_backup_result`
    // (Backup-Ergebnis-Hinweise) und `scheduler/reminders.rs`. OS-Push für
    // diese Inbox-Einträge übernimmt der periodische Reminder-Cron.
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
    if let Err(e) = notify::store::create(pool, n).await {
        tracing::warn!("Abo-Rechnung-Hinweis fehlgeschlagen: {e}");
    }
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
    //! DB-/Pipeline-Tests für den Abo-Scheduler (KB-0058). Bewusst **Lib-Unit-
    //! Tests** statt `tests/`-Integrationstests: nur im `cfg(test)`-Build der Lib
    //! ist `notify::os_native::push` ein No-op (kein WinRT/comctl32-Link). Als
    //! `tests/`-Binary scheitert derselbe Pfad unter Windows beim Laden
    //! (STATUS_ENTRYPOINT_NOT_FOUND, vgl. `notify::os_native`). `auto_mode =
    //! 'draft'` → kein Java-Sidecar nötig.

    use super::*;
    use crate::db::repo::seller_profile::SellerProfileInput;
    use crate::db::repo::{contacts, invoices, seller_profile};
    use crate::db::MIGRATOR;
    use crate::domain::contact::{ContactInput, ContactType};
    use crate::domain::invoice::InvoiceItemInput;
    use crate::domain::recurring_invoice::RecurringInvoiceInput;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::path::Path;
    use std::str::FromStr;

    async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("klein-buch.sqlite");
        let url = format!("sqlite://{}", db_path.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
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
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// §19-Verkäuferprofil (Kleinunternehmer) — Pflicht für die Draft-Pipeline.
    async fn mk_seller(pool: &SqlitePool) {
        let input = SellerProfileInput {
            name: "Wildbach Computerhilfe".into(),
            legal_form: None,
            street: "Teststr. 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: None,
            vat_id: None,
            email: "test@example.de".into(),
            phone: None,
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: true,
            default_pdf_template: None,
            default_currency: None,
            confirm_waive_paragraph_19: None,
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
            email: Some("kunde@example.de".into()),
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

    #[tokio::test]
    async fn process_due_skips_when_locked() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = BackupSession::default(); // gesperrt

        recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
            .await
            .unwrap();

        let report = process_due(&pool, &paths, &session, None, d(2026, 5, 20))
            .await
            .unwrap();
        assert!(
            report.skipped_locked,
            "gesperrte Session → Lauf übersprungen"
        );
        assert_eq!(report.created_invoices, 0);
    }

    #[tokio::test]
    async fn process_due_catches_up_all_missed_periods_as_drafts() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = BackupSession::default();
        session.set("passphrase-1234".into());

        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 3, 1), "draft"))
                .await
                .unwrap();

        // Heute 2026-05-20 → fällige Stichtage 03-01, 04-01, 05-01 (3); 06-01 Zukunft.
        let report = process_due(&pool, &paths, &session, None, d(2026, 5, 20))
            .await
            .unwrap();
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

        // Zuletzt erzeugte Rechnung (Periode 05-01): Belegdatum = heute (nicht
        // rückdatiert), Leistungsdatum = Periodenstichtag, GJ = Jahr des Belegdatums.
        let inv = invoices::get(&pool, &last_id).await.unwrap().unwrap();
        assert_eq!(
            inv.invoice_date, "2026-05-20",
            "Belegdatum = Erstellungstag"
        );
        assert_eq!(inv.delivery_date.as_deref(), Some("2026-05-01"));
        assert_eq!(inv.fiscal_year, 2026);

        // Zweiter Lauf am selben Tag legt nichts Neues an (idempotent über Stichtag).
        let again = process_due(&pool, &paths, &session, None, d(2026, 5, 20))
            .await
            .unwrap();
        assert_eq!(again.created_invoices, 0);
    }

    #[tokio::test]
    async fn run_now_creates_one_with_today_invoice_date_and_advances() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = BackupSession::default();
        session.set("passphrase-1234".into());

        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 5, 1), "draft"))
                .await
                .unwrap();

        let invoice_id = run_now(
            &pool,
            &paths,
            &session,
            None,
            &detail.template.id,
            d(2026, 5, 20),
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

    #[tokio::test]
    async fn run_now_rejects_future_due() {
        let (pool, dir) = setup_pool().await;
        let paths = paths_for(dir.path());
        mk_seller(&pool).await;
        let contact_id = mk_contact(&pool).await;
        let session = BackupSession::default();
        session.set("passphrase-1234".into());

        // next_due 2026-06-01 liegt nach today 2026-05-20 → vorzeitige Erstellung
        // würde ein zukünftiges Periodendatum erzeugen → Fehler.
        let detail =
            recurring_invoice::create(&pool, &ri_tmpl(&contact_id, d(2026, 6, 1), "draft"))
                .await
                .unwrap();

        let res = run_now(
            &pool,
            &paths,
            &session,
            None,
            &detail.template.id,
            d(2026, 5, 20),
        )
        .await;
        assert!(
            res.is_err(),
            "Zukunfts-Stichtag darf nicht vorzeitig erstellt werden"
        );
    }
}
