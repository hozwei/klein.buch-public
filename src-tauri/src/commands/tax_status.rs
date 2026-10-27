//! Commands der §19-Statushistorie (ADR 0042 Nr. 1–6).
//!
//! - [`tax_status_overview`]: Historie, Status heute, Bindung, frühestmögliche
//!   Rückkehr, Gesamtumsatz von Vorjahr und laufendem Jahr, Grenzhinweise.
//! - [`tax_status_at`]: Status an einem Datum (für Editoren: Leistungsdatum,
//!   sonst Belegdatum, ADR 0042 Nr. 3).
//! - [`tax_status_change_preview`]: Prüfung und Folgen eines geplanten Eintrags,
//!   ohne zu schreiben.
//! - [`tax_status_change_add`] / [`tax_status_change_revoke`]: Eintrag bzw.
//!   Widerruf mit frischer Re-Auth (Registry), Audit und Spiegel in derselben
//!   Transaktion; danach Hinweise in der Inbox (betroffene festgeschriebene
//!   Rechnungen, Entwürfe und Abo-Vorlagen, aktivierte Kosten, Umsatzgrenzen).
//!
//! Den Status ändert Klein.Buch nie selbst (ADR 0042 Nr. 5): Ein automatischer
//! Wechsel würde ohne Wissen des Nutzers Rechnungen mit Umsatzsteuer erzeugen.

use crate::app_ctx::AppCtx;
use crate::db::repo::tax_status::{self as repo, TaxStatusChangeRow};
use crate::domain::kleinunternehmer::hinweis_text;
use crate::domain::tax_status::{
    self, first_difference, ChangeReason, Grenzhinweis, NewChange, TaxStatus, TaxStatusHistory,
};
use crate::error::{Error, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

fn de(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

fn eur(cents: i64) -> String {
    crate::euer::detail::eur_grouped(cents)
}

// ============================================================================
// Übersicht
// ============================================================================

/// Argumente von [`tax_status_overview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusOverviewArgs {}

/// Gesamtumsatz nach vereinnahmten Entgelten (§ 19 Abs. 2 UStG).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Turnover {
    pub previous_year: i32,
    pub previous_year_cents: i64,
    pub current_year: i32,
    pub current_year_cents: i64,
}

/// Antwort von [`tax_status_overview`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusOverview {
    /// Alle Einträge (auch widerrufene), nach Wirkungsdatum.
    pub entries: Vec<TaxStatusChangeRow>,
    pub today: String,
    pub current_status: TaxStatus,
    /// Laufende Bindung nach einem Verzicht: erster Tag der möglichen Rückkehr.
    pub binding_until: Option<String>,
    /// Frühestmögliche Rückkehr zu §19 (nur bei Regelbesteuerung heute).
    pub earliest_return: Option<String>,
    /// Jüngster gültiger Eintrag: nur er lässt sich widerrufen.
    pub revocable_id: Option<String>,
    pub turnover: Turnover,
    pub threshold_hints: Vec<Grenzhinweis>,
    pub hinweis_text: &'static str,
}

/// Grenzprüfung für `today` aus den erfassten Zahlungen.
async fn threshold_hints(
    pool: &SqlitePool,
    hist: &TaxStatusHistory,
    today: NaiveDate,
) -> Result<(Turnover, Vec<Grenzhinweis>)> {
    let year = today.year();
    let posten = repo::turnover_items(pool, &[year - 1, year]).await?;
    let earlier = repo::has_turnover_before(pool, year - 1).await?;
    let turnover = Turnover {
        previous_year: year - 1,
        previous_year_cents: tax_status::gesamtumsatz(&posten, year - 1),
        current_year: year,
        current_year_cents: tax_status::gesamtumsatz(&posten, year),
    };
    let hints = tax_status::grenzpruefung(hist, &posten, today, earlier);
    Ok((turnover, hints))
}

pub async fn tax_status_overview(
    ctx: &Arc<AppCtx>,
    _args: TaxStatusOverviewArgs,
) -> Result<TaxStatusOverview> {
    let pool = ctx.pool()?;
    let today = crate::clock::berlin_date(ctx.clock().now_utc());
    let entries = repo::list(&pool).await?;
    let hist = repo::history_of(&entries)?;
    let (turnover, threshold_hints) = threshold_hints(&pool, &hist, today).await?;
    Ok(TaxStatusOverview {
        today: today.to_string(),
        current_status: hist.status_at(today),
        binding_until: hist.binding_at(today).map(|d| d.to_string()),
        earliest_return: hist.earliest_return(today).map(|d| d.to_string()),
        revocable_id: hist.last().map(|e| e.id.clone()),
        entries,
        turnover,
        threshold_hints,
        hinweis_text: hinweis_text(),
    })
}

/// Argumente von [`tax_status_at`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusAtArgs {
    /// Leistungsdatum, sonst Rechnungs-/Angebots-/Belegdatum (JJJJ-MM-TT).
    pub date: NaiveDate,
}

/// Antwort von [`tax_status_at`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusAt {
    pub date: String,
    pub status: TaxStatus,
    pub is_kleinunternehmer: bool,
}

pub async fn tax_status_at(ctx: &Arc<AppCtx>, args: TaxStatusAtArgs) -> Result<TaxStatusAt> {
    let pool = ctx.pool()?;
    let status = repo::history(&pool).await?.status_at(args.date);
    Ok(TaxStatusAt {
        date: args.date.to_string(),
        status,
        is_kleinunternehmer: status.is_klein(),
    })
}

// ============================================================================
// Eintrag: Eingabe, Prüfung, Folgen
// ============================================================================

/// Ein geplanter Eintrag.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusChangeInput {
    /// Wirkungsdatum; beim Anfangsstand ohne Bedeutung (seit Beginn).
    pub effective_from: Option<NaiveDate>,
    pub status: TaxStatus,
    pub reason: ChangeReason,
    /// Beleg, mit dem eine Grenze überschritten wurde (laufendes Jahr,
    /// Gründungsjahr).
    #[serde(default)]
    pub trigger_document: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn clean(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl TaxStatusChangeInput {
    fn to_change(&self) -> Result<NewChange> {
        let effective_from = if self.reason == ChangeReason::Initial {
            tax_status::anfang()
        } else {
            self.effective_from
                .ok_or_else(|| Error::Domain("Bitte ein Wirkungsdatum angeben.".into()))?
        };
        if clean(&self.trigger_document).is_some_and(|s| s.chars().count() > 100) {
            return Err(Error::Domain(
                "Die Belegnummer ist zu lang (höchstens 100 Zeichen).".into(),
            ));
        }
        if clean(&self.note).is_some_and(|s| s.chars().count() > 2000) {
            return Err(Error::Domain(
                "Die Notiz ist zu lang (höchstens 2.000 Zeichen).".into(),
            ));
        }
        Ok(NewChange {
            effective_from,
            status: self.status,
            reason: self.reason,
        })
    }
}

/// Festgeschriebene Rechnung, deren eingefrorener Status vom Status nach der
/// Änderung abweicht (ADR 0042 Nr. 4).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AffectedInvoice {
    pub id: String,
    pub number: String,
    /// Maßgebliches Datum (Leistungsdatum, sonst Rechnungsdatum).
    pub document_date: String,
    /// Beim Festschreiben eingefroren.
    pub locked_as_kleinunternehmer: bool,
    /// Status am maßgeblichen Datum nach der Änderung.
    pub now_kleinunternehmer: bool,
}

/// Folgen einer Änderung der Historie.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeConsequences {
    /// Erster Tag mit anderem Status; `None` = keine Wirkung.
    pub first_difference: Option<String>,
    /// Festgeschriebene Rechnungen mit abweichendem Status (werden nicht
    /// geändert, nur gemeldet).
    pub affected_invoices: Vec<AffectedInvoice>,
    /// Offene Rechnungs- und Angebotsentwürfe, deren Datum betroffen ist (sie
    /// übernehmen beim Festschreiben den neuen Status).
    pub affected_drafts: Vec<String>,
    /// Aktive Abo-Vorlagen, deren Positionen nicht zum Status am nächsten
    /// Termin passen.
    pub recurring_templates: Vec<String>,
    /// Aus Kosten übernommene Anlagen mit unpassenden Anschaffungskosten.
    pub capitalized_assets: Vec<String>,
    /// Offene Geschäftsjahre bis heute, deren Auswertungen sich ändern können.
    pub reexport_years: Vec<i32>,
}

/// Ergebnis von [`tax_status_change_preview`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePreview {
    pub ok: bool,
    pub errors: Vec<String>,
    /// Hinweise, die den Eintrag nicht sperren (ADR 0042 Nr. 4 und 5):
    /// Erklärungsfrist des Verzichts abgelaufen, Vorjahresumsatz vor einer
    /// Rückkehr über 25.000 €.
    pub warnings: Vec<String>,
    pub binding_until: Option<String>,
    pub consequences: ChangeConsequences,
}

/// Ergebnis von [`tax_status_change_add`] bzw. [`tax_status_change_revoke`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeResult {
    pub entry: TaxStatusChangeRow,
    pub consequences: ChangeConsequences,
}

fn relevant_date(delivery: Option<&str>, document: &str) -> Result<NaiveDate> {
    repo::parse_document_date(delivery.unwrap_or(document), "Beleg")
}

/// Folgen des Übergangs `old` → `new` (ADR 0042 Nr. 4). Lesend.
pub async fn consequences(
    pool: &SqlitePool,
    old: &TaxStatusHistory,
    new: &TaxStatusHistory,
    today: NaiveDate,
) -> Result<ChangeConsequences> {
    let Some(first) = first_difference(old, new) else {
        return Ok(ChangeConsequences::default());
    };
    let mut out = ChangeConsequences {
        first_difference: Some(first.to_string()),
        ..Default::default()
    };

    // Festgeschriebene Rechnungen (ohne Stornos und stornierte Originale; ein
    // Storno folgt stets seinem Original).
    let locked: Vec<(String, String, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT id, invoice_number, delivery_date, invoice_date, is_kleinunternehmer
           FROM invoices
          WHERE direction = 'issued' AND locked_at IS NOT NULL
            AND is_storno_for IS NULL AND status != 'canceled'
          ORDER BY invoice_date, invoice_number",
    )
    .fetch_all(pool)
    .await?;
    for (id, number, delivery, invoice_date, frozen) in locked {
        let date = relevant_date(delivery.as_deref(), &invoice_date)?;
        if date < first {
            continue;
        }
        let now = new.is_klein_at(date);
        if now != (frozen == 1) {
            out.affected_invoices.push(AffectedInvoice {
                id,
                number,
                document_date: date.to_string(),
                locked_as_kleinunternehmer: frozen == 1,
                now_kleinunternehmer: now,
            });
        }
    }

    // Offene Entwürfe: Status am Datum ändert sich.
    let drafts: Vec<(String, Option<String>, String)> = sqlx::query_as(
        "SELECT invoice_number, delivery_date, invoice_date FROM invoices
          WHERE direction = 'issued' AND locked_at IS NULL AND voided_at IS NULL
            AND is_storno_for IS NULL
         UNION ALL
         SELECT quote_number, NULL, quote_date FROM quotes WHERE status = 'draft'
          ORDER BY 1",
    )
    .fetch_all(pool)
    .await?;
    for (number, delivery, date) in drafts {
        let d = relevant_date(delivery.as_deref(), &date)?;
        if d >= first && old.is_klein_at(d) != new.is_klein_at(d) {
            out.affected_drafts.push(number);
        }
    }

    // Aktive Abo-Vorlagen: Positionen passen nicht zum Status am nächsten Termin.
    let templates: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT r.label, r.next_due_date, i.tax_category_code, i.position
           FROM recurring_invoices r
           JOIN recurring_invoice_items i ON i.recurring_invoice_id = r.id
          WHERE r.active = 1
          ORDER BY r.label, i.position",
    )
    .fetch_all(pool)
    .await?;
    for (label, next_due, code, _) in templates {
        let d = repo::parse_document_date(&next_due, "Abo-Vorlage")?;
        let klein = new.is_klein_at(d);
        let exempt = code == crate::domain::kleinunternehmer::EXEMPT_CATEGORY;
        if klein != exempt && !out.recurring_templates.contains(&label) {
            out.recurring_templates.push(label);
        }
    }

    out.capitalized_assets = crate::commands::assets::capitalized_cost_mismatches_with(pool, new)
        .await?
        .into_iter()
        .map(|m| m.asset_number)
        .collect();

    let mut conn = pool.acquire().await?;
    let closed = repo::closed_years_in(&mut conn).await?;
    out.reexport_years = (first.year()..=today.year())
        .filter(|y| !closed.contains(y))
        .collect();
    Ok(out)
}

fn validation_messages(errs: &[tax_status::ChangeError]) -> Vec<String> {
    errs.iter().map(tax_status::message).collect()
}

/// Argumente von [`tax_status_change_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusChangePreviewArgs {
    pub input: TaxStatusChangeInput,
}

/// Warnungen zu einem geplanten Eintrag (ADR 0042 Nr. 4 und 5); sperren nicht.
async fn change_warnings(
    pool: &SqlitePool,
    change: &NewChange,
    today: NaiveDate,
) -> Result<Vec<String>> {
    let turnover_before = if change.reason == ChangeReason::Rueckkehr {
        let year = change.effective_from.year() - 1;
        let posten = repo::turnover_items(pool, &[year]).await?;
        Some(tax_status::gesamtumsatz(&posten, year))
    } else {
        None
    };
    Ok(tax_status::change_warnings(change, today, turnover_before)
        .iter()
        .map(tax_status::warning_message)
        .collect())
}

/// Prüft einen geplanten Eintrag und listet seine Folgen, ohne zu schreiben.
pub async fn tax_status_change_preview(
    ctx: &Arc<AppCtx>,
    args: TaxStatusChangePreviewArgs,
) -> Result<ChangePreview> {
    let pool = ctx.pool()?;
    let today = crate::clock::berlin_date(ctx.clock().now_utc());
    let change = args.input.to_change()?;
    let mut conn = pool.acquire().await?;
    let hist = repo::history_in(&mut conn).await?;
    let closed = repo::closed_years_in(&mut conn).await?;
    let capitalized = repo::capitalized_input_tax_in(&mut conn, &closed).await?;
    drop(conn);
    match tax_status::validate_change_with(&hist, &change, today, &closed, &capitalized) {
        Ok(binding) => {
            let next = hist.after_change(&change, binding);
            Ok(ChangePreview {
                ok: true,
                errors: Vec::new(),
                warnings: change_warnings(&pool, &change, today).await?,
                binding_until: binding.map(|d| d.to_string()),
                consequences: consequences(&pool, &hist, &next, today).await?,
            })
        }
        Err(errs) => Ok(ChangePreview {
            ok: false,
            errors: validation_messages(&errs),
            warnings: Vec::new(),
            binding_until: None,
            consequences: ChangeConsequences::default(),
        }),
    }
}

/// Argumente von [`tax_status_change_add`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusChangeAddArgs {
    pub input: TaxStatusChangeInput,
}

/// Trägt einen Wechsel des Steuerstatus ein (ADR 0042 Nr. 1–4). Re-Auth über
/// die Registry; Eintrag, Audit und Spiegel in einer Transaktion unter
/// [`crate::locks::SELLER_PROFILE`]; ein Jahresabschluss wartet, bis der
/// Eintrag geschrieben ist.
pub async fn tax_status_change_add(
    ctx: &Arc<AppCtx>,
    args: TaxStatusChangeAddArgs,
) -> Result<ChangeResult> {
    let pool = ctx.pool()?;
    let today = crate::clock::berlin_date(ctx.clock().now_utc());
    let input = args.input;
    let change = input.to_change()?;
    let trigger_document = clean(&input.trigger_document);
    let note = clean(&input.note);

    let _profile = crate::locks::SELLER_PROFILE.lock("1").await;
    let permit = crate::fiscal_year::guard::booking_permit().await;
    let mut tx = crate::db::begin_immediate(&pool).await?;
    let hist = repo::history_in(&mut tx).await?;
    let closed = repo::closed_years_in(&mut tx).await?;
    let capitalized = repo::capitalized_input_tax_in(&mut tx, &closed).await?;
    let binding = tax_status::validate_change_with(&hist, &change, today, &closed, &capitalized)
        .map_err(|errs| {
            Error::Domain(format!(
                "Der Steuerstatus lässt sich so nicht eintragen: {}",
                validation_messages(&errs).join(" ")
            ))
        })?;
    let id = repo::insert_in(
        &mut tx,
        &repo::NewEntry {
            effective_from: change.effective_from,
            status: change.status,
            reason: change.reason,
            binding_until: binding,
            trigger_document: trigger_document.as_deref(),
            note: note.as_deref(),
        },
    )
    .await?;
    repo::refresh_mirror_in(&mut tx, today).await?;
    tx.commit().await?;
    drop(permit);

    let next = repo::history(&pool).await?;
    let consequences = consequences(&pool, &hist, &next, today).await?;
    notify_after_change(&pool, ChangeAction::Add, &id, &consequences, today).await;
    let entry = repo::list(&pool)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| Error::Domain("Eintrag nach dem Speichern nicht gefunden.".into()))?;
    Ok(ChangeResult {
        entry,
        consequences,
    })
}

/// Argumente von [`tax_status_change_revoke`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxStatusChangeRevokeArgs {
    pub id: String,
    pub reason: String,
}

/// Widerruft den jüngsten gültigen Eintrag mit Grund (ADR 0042 Nr. 1). Nicht,
/// wenn sich dadurch ein abgeschlossenes Geschäftsjahr änderte. Festgeschriebene
/// Rechnungen unter dem widerrufenen Status bleiben, wie sie sind; Klein.Buch
/// meldet sie zur Prüfung.
pub async fn tax_status_change_revoke(
    ctx: &Arc<AppCtx>,
    args: TaxStatusChangeRevokeArgs,
) -> Result<ChangeResult> {
    let pool = ctx.pool()?;
    let today = crate::clock::berlin_date(ctx.clock().now_utc());
    let reason = args.reason.trim().to_string();
    if reason.is_empty() {
        return Err(Error::Domain(tax_status::message(
            &tax_status::ChangeError::ReasonMissing,
        )));
    }
    if reason.chars().count() > 2000 {
        return Err(Error::Domain(
            "Der Grund ist zu lang (höchstens 2.000 Zeichen).".into(),
        ));
    }

    let _profile = crate::locks::SELLER_PROFILE.lock("1").await;
    let permit = crate::fiscal_year::guard::booking_permit().await;
    let mut tx = crate::db::begin_immediate(&pool).await?;
    let rows = repo::list_in(&mut tx).await?;
    if let Some(r) = rows.iter().find(|r| r.id == args.id) {
        if r.revoked_at.is_some() {
            return Err(Error::Domain(tax_status::message(
                &tax_status::ChangeError::AlreadyRevoked,
            )));
        }
    }
    let hist = repo::history_of(&rows)?;
    let closed = repo::closed_years_in(&mut tx).await?;
    let capitalized = repo::capitalized_input_tax_in(&mut tx, &closed).await?;
    tax_status::validate_revoke_with(&hist, &args.id, &closed, &capitalized).map_err(|e| {
        Error::Domain(format!(
            "Der Eintrag lässt sich nicht widerrufen: {}",
            tax_status::message(&e)
        ))
    })?;
    repo::revoke_in(&mut tx, &args.id, &reason).await?;
    repo::refresh_mirror_in(&mut tx, today).await?;
    tx.commit().await?;
    drop(permit);

    let next = hist.after_revoke(&args.id);
    let consequences = consequences(&pool, &hist, &next, today).await?;
    notify_after_change(&pool, ChangeAction::Revoke, &args.id, &consequences, today).await;
    let entry = repo::list(&pool)
        .await?
        .into_iter()
        .find(|r| r.id == args.id)
        .ok_or_else(|| Error::Domain("Eintrag nach dem Widerruf nicht gefunden.".into()))?;
    Ok(ChangeResult {
        entry,
        consequences,
    })
}

// ============================================================================
// Hinweise
// ============================================================================

/// Auslöser der Hinweise: Eintrag oder Widerruf desselben Eintrags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeAction {
    Add,
    Revoke,
}

impl ChangeAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Revoke => "revoke",
        }
    }
}

/// Hinweise nach einem Eintrag oder Widerruf (ADR 0042 Nr. 4 und 5).
/// Best-effort: ein Fehler beim Hinweis macht den Eintrag nicht rückgängig.
/// Der Dedup-Schlüssel nennt die Aktion: Eintrag und Widerruf desselben
/// Eintrags melden je eigene Belege (sonst verwürfe `INSERT OR IGNORE` den
/// Hinweis nach dem Widerruf).
async fn notify_after_change(
    pool: &SqlitePool,
    action: ChangeAction,
    entry_id: &str,
    c: &ChangeConsequences,
    today: NaiveDate,
) {
    if !c.affected_invoices.is_empty() {
        let label = |klein: bool| {
            if klein {
                "als Kleinunternehmer (ohne Umsatzsteuer)"
            } else {
                "mit Regelbesteuerung"
            }
        };
        let lines: Vec<String> = c
            .affected_invoices
            .iter()
            .map(|a| {
                let d = NaiveDate::parse_from_str(&a.document_date, "%Y-%m-%d")
                    .map(de)
                    .unwrap_or_else(|_| a.document_date.clone());
                format!(
                    "{} (Leistung {d}): festgeschrieben {}, nach dem Steuerstatus {}.",
                    a.number,
                    label(a.locked_as_kleinunternehmer),
                    label(a.now_kleinunternehmer)
                )
            })
            .collect();
        let body = format!(
            "Der geänderte Steuerstatus wirkt auf schon festgeschriebene Rechnungen. Klein.Buch \
             ändert sie nicht. Kläre mit deiner Steuerberatung, ob eine Berichtigung (§ 31 Abs. 5 \
             UStDV) oder Storno und Neuausstellung nötig ist. {}",
            lines.join(" ")
        );
        let key = format!("tax_status_affected:{}:{entry_id}", action.as_str());
        let n = crate::notify::NewNotification {
            title: "Festgeschriebene Rechnungen nach Steuerstatus-Wechsel prüfen",
            body: &body,
            severity: "warning",
            related_entity_type: Some("invoice"),
            action_url: Some("/invoices"),
            dedup_key: Some(&key),
            ..Default::default()
        };
        if let Err(e) = crate::notify::emit(pool, n).await {
            tracing::warn!("Hinweis zu betroffenen Rechnungen nicht angelegt: {e}");
        }
    }

    let mut parts: Vec<String> = Vec::new();
    if !c.affected_drafts.is_empty() {
        parts.push(format!(
            "Entwürfe, die beim Festschreiben den neuen Status übernehmen (Positionen prüfen): {}.",
            c.affected_drafts.join(", ")
        ));
    }
    if !c.recurring_templates.is_empty() {
        parts.push(format!(
            "Abo-Vorlagen, deren Steuerart nicht zum Status am nächsten Termin passt (Vorlage \
             anpassen, sonst entsteht dort keine Rechnung): {}.",
            c.recurring_templates.join(", ")
        ));
    }
    // Nur rückwirkende Änderungen ändern schon vorhandene Auswertungen.
    let retro = c
        .first_difference
        .as_deref()
        .and_then(|f| NaiveDate::parse_from_str(f, "%Y-%m-%d").ok())
        .is_some_and(|f| f <= today);
    let past_years: Vec<String> = c.reexport_years.iter().map(|y| y.to_string()).collect();
    if retro && !past_years.is_empty() {
        parts.push(format!(
            "Auswertungen der offenen Geschäftsjahre {} (EÜR, ELSTER-Ausfüllhilfe, DATEV-Stapel, \
             Steuerberater-Paket) können sich geändert haben; schon weitergegebene Exporte neu \
             erstellen.",
            past_years.join(", ")
        ));
    }
    if !parts.is_empty() {
        let body = parts.join(" ");
        let key = format!("tax_status_followups:{}:{entry_id}", action.as_str());
        let n = crate::notify::NewNotification {
            title: "Folgen des Steuerstatus-Wechsels",
            body: &body,
            severity: "info",
            action_url: Some("/settings/seller"),
            dedup_key: Some(&key),
            ..Default::default()
        };
        if let Err(e) = crate::notify::emit(pool, n).await {
            tracing::warn!("Hinweis zu den Folgen des Statuswechsels nicht angelegt: {e}");
        }
    }

    crate::commands::assets::notify_capitalized_status_change(pool).await;
    if let Err(e) = notify_thresholds(pool, today).await {
        tracing::warn!("Grenzprüfung nach §19-Wechsel fehlgeschlagen: {e}");
    }
}

/// Hinweis zu erreichten oder überschrittenen Umsatzgrenzen (ADR 0042 Nr. 5).
/// Regel `rule_tax_threshold` (Migration 0033); je Grenze, Jahr und Stufe
/// höchstens ein Hinweis. Läuft täglich im Scheduler und nach jedem Eintrag.
pub async fn notify_thresholds(pool: &SqlitePool, today: NaiveDate) -> Result<usize> {
    let rule = crate::notify::rules::get(pool, "rule_tax_threshold").await?;
    let Some(rule) = rule.filter(|r| r.is_enabled()) else {
        return Ok(0);
    };
    let hist = repo::history(pool).await?;
    let (_, hints) = threshold_hints(pool, &hist, today).await?;
    let mut created = 0usize;
    for h in &hints {
        let (title, what) = match (h.kind, h.exceeded) {
            (tax_status::Grenzart::Vorjahr, true) => (
                "Vorjahresumsatz über 25.000 €",
                format!(
                    "Der Gesamtumsatz {} lag nach den in Klein.Buch erfassten Zahlungen bei {} und \
                     damit über {}. Dann gilt §19 ab dem 1. Januar {} nicht mehr (§ 19 Abs. 1 \
                     UStG), ohne Bindung; Vorschlag: Wegfall wegen des Vorjahres zum 01.01.{}.",
                    h.year,
                    eur(h.turnover_cents),
                    eur(h.limit_cents),
                    h.year + 1,
                    h.year + 1
                ),
            ),
            (tax_status::Grenzart::Vorjahr, false) => (
                "Vorjahresumsatz hat 25.000 € erreicht",
                format!(
                    "Der Gesamtumsatz {} lag bei {} und hat die Grenze von {} erreicht, aber nicht \
                     überschritten; §19 gilt weiter.",
                    h.year,
                    eur(h.turnover_cents),
                    eur(h.limit_cents)
                ),
            ),
            (kind, exceeded) => {
                let (title, limit_text) = match (kind, exceeded) {
                    (tax_status::Grenzart::Gruendungsjahr, true) => (
                        "Umsatz im Gründungsjahr über 25.000 €",
                        "im Gründungsjahr (§ 19 Abs. 2 UStG)",
                    ),
                    (tax_status::Grenzart::Gruendungsjahr, false) => (
                        "Umsatz im Gründungsjahr hat 25.000 € erreicht",
                        "im Gründungsjahr (§ 19 Abs. 2 UStG)",
                    ),
                    (_, true) => (
                        "Umsatz im laufenden Jahr über 100.000 €",
                        "im laufenden Jahr (§ 19 Abs. 1 UStG)",
                    ),
                    (_, false) => (
                        "Umsatz im laufenden Jahr hat 100.000 € erreicht",
                        "im laufenden Jahr (§ 19 Abs. 1 UStG)",
                    ),
                };
                let mut text = format!(
                    "Der Gesamtumsatz {} liegt nach den erfassten Zahlungen bei {}; die Grenze \
                     {limit_text} beträgt {}.",
                    h.year,
                    eur(h.turnover_cents),
                    eur(h.limit_cents)
                );
                if let (Some(doc), Some(from)) =
                    (h.trigger_document.as_deref(), h.suggested_effective_from)
                {
                    text.push_str(&format!(
                        " Überschritten mit {doc}; dieser Umsatz ist schon steuerpflichtig, die \
                         vorherigen bleiben steuerfrei. Vorschlag: Wegfall zum {} mit {doc} als \
                         auslösendem Beleg.",
                        de(from)
                    ));
                }
                if h.presumed {
                    text.push_str(
                        " Klein.Buch kennt keinen Umsatz aus früheren Jahren und vermutet deshalb \
                         ein Gründungsjahr; trifft das nicht zu, gilt die Grenze von 100.000 €.",
                    );
                }
                (title, text)
            }
        };
        let body = format!(
            "{what} Klein.Buch ändert den Steuerstatus nicht selbst: Liegen die Voraussetzungen \
             vor, trage den Wechsel unter Einstellungen → Meine Firmendaten → Steuerstatus ein. \
             Den Gesamtumsatz kennt Klein.Buch nur aus den hier erfassten Rechnungen und \
             Zahlungen; Umsätze mit Anlagevermögen zählen nicht mit."
        );
        let key = format!(
            "tax_threshold:{}:{}:{}",
            h.kind.as_str(),
            h.year,
            if h.exceeded { "ueber" } else { "erreicht" }
        );
        let n = crate::notify::NewNotification {
            rule_id: Some(&rule.id),
            title,
            body: &body,
            severity: if h.exceeded { "warning" } else { "info" },
            action_url: Some("/settings/seller"),
            dedup_key: Some(&key),
            ..Default::default()
        };
        if crate::notify::emit(pool, n).await?.is_some() {
            created += 1;
        }
    }
    Ok(created)
}

/// Kurzfassung des Jahresstatus für Texte (Steuerberater-Paket, EÜR-PDF).
pub fn year_status_text(hist: &TaxStatusHistory, year: i32) -> String {
    let segs = hist.segments(year);
    if segs.len() == 1 {
        return segs[0].status.label().to_string();
    }
    segs.iter()
        .map(|s| format!("{}–{}: {}", de(s.from), de(s.to), s.status.label()))
        .collect::<Vec<_>>()
        .join("; ")
}
