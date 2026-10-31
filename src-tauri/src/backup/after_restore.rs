//! Abgleich nach einer Wiederherstellung (Lauf F, Teil F1d; ADR 0040 Nr. 11,
//! 12 und Entscheidungen 32–35).
//!
//! Phase A des Restores ([`crate::backup::restore::apply`]) hält neben den
//! Nummernkreisen (D-24) zwei weitere Stände der laufenden Datenbank im
//! Restore-Marker fest — ab der Annahme des Restores sind Änderungen gesperrt,
//! die Stände bleiben bis zum Tausch gültig:
//!
//! - die nächsten Termine aller Abo-Rechnungs-Vorlagen und Kosten-Abos
//!   ([`snapshot_series`]),
//! - die Nummern aller festgeschriebenen Rechnungen, Stornos und Angebote
//!   ([`snapshot_locked_numbers`]).
//!
//! Das erste Entsperren nach dem Tausch gleicht die eingespielte Datenbank in
//! der Transaktion des Restore-Audits damit ab ([`apply_in`]):
//!
//! 1. **Termine nur vorwärts.** Steht eine Reihe im eingespielten Stand auf
//!    einem älteren Termin als vor dem Restore, rückt sie auf den Stand vor dem
//!    Restore vor (Compare-and-Swap, Audit `recurring_invoice.advanced_after_restore`
//!    bzw. `recurring.advanced_after_restore` mit den ausgelassenen Terminen).
//!    Bereits abgerechnete Zeiträume werden so nicht erneut in Rechnung
//!    gestellt oder gebucht.
//!    Der Hinweis nennt vorgerückte Kosten-Abos getrennt: Ihre gebuchten Kosten
//!    sind mit dem ersetzten Stand verworfen und von Hand nachzutragen (F3b).
//! 2. **Prüfung ohne bekannten Stand.** Kennt Klein.Buch den Stand einer Reihe
//!    nicht (Disaster Recovery in eine leere Instanz, Reihe vor dem Restore
//!    gelöscht) und ist ein Termin bis heute offen, verlangt die Reihe die
//!    Entscheidung „Rückstand …“ bzw. beim Fortsetzen einer pausierten Reihe
//!    (Marker, siehe [`crate::scheduler::catch_up`]) — nur Vorlagen innerhalb
//!    ihrer Laufzeit und Kosten-Abos mit Auto-Buchung, für alle anderen erzeugt
//!    kein Lauf etwas (F3b).
//! 3. **Marker aus dem Backup verwerfen.** Fortsetzungs-Marker begonnener
//!    Abo-Rechnungen (`recurring_invoice_pending:*`) und Bestätigungen zum
//!    Nachholen (`recurring_catch_up_ok:*`) stammen aus dem alten Stand; sie
//!    würden sonst festschreiben und versenden bzw. bis zu 24 Termine ohne
//!    Rückfrage nachholen. Audit `recurring.markers_cleared_after_restore`.
//! 4. **Entwürfe mit bereits vergebener Nummer.** Ein Entwurf des eingespielten
//!    Stands, dessen Nummer vor dem Restore schon festgeschrieben (und
//!    womöglich versandt) war, wird vorgemerkt
//!    (`restored_number_conflict:<art>:<id>`, Audit
//!    `numbering.restored_draft_flagged`). Festschreiben ist erst möglich,
//!    nachdem der Inhaber mit Re-Auth bestätigt hat, dass der Entwurf dem
//!    versandten Original entspricht ([`confirm_conflict`]) — sonst gäbe es zwei
//!    verschiedene Belege mit derselben Nummer (§14 Abs. 4 Nr. 4 UStG).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::db::repo::{app_settings, audit_log};
use crate::error::{Error, Result};
use crate::scheduler::catch_up::{self, Series};

/// Art einer Reihe im Restore-Marker.
pub const KIND_TEMPLATE: &str = "invoice";
/// Art einer Reihe im Restore-Marker.
pub const KIND_SUBSCRIPTION: &str = "subscription";

/// Nächster Termin einer Reihe der ersetzten Datenbank.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesState {
    /// [`KIND_TEMPLATE`] oder [`KIND_SUBSCRIPTION`].
    pub kind: String,
    pub id: String,
    pub next_due_date: String,
}

/// Nächste Termine aller Abo-Rechnungs-Vorlagen und Kosten-Abos (Phase A).
pub async fn snapshot_series(pool: &SqlitePool) -> Result<Vec<SeriesState>> {
    let mut out = Vec::new();
    for (kind, table) in [
        (KIND_TEMPLATE, "recurring_invoices"),
        (KIND_SUBSCRIPTION, "recurring_subscriptions"),
    ] {
        let rows: Vec<(String, String)> = sqlx::query_as(&format!(
            "SELECT id, next_due_date FROM {table} ORDER BY id"
        ))
        .fetch_all(pool)
        .await?;
        out.extend(rows.into_iter().map(|(id, next_due_date)| SeriesState {
            kind: kind.to_string(),
            id,
            next_due_date,
        }));
    }
    Ok(out)
}

/// Nummern aller festgeschriebenen Rechnungen (auch Stornos) und Angebote der
/// laufenden Datenbank (Phase A). Kosten, Privatbewegungen und Anlagen fehlen
/// bewusst: Sie entstehen festgeschrieben bzw. ihre Nummern verlassen die
/// Instanz nicht.
pub async fn snapshot_locked_numbers(pool: &SqlitePool) -> Result<Vec<String>> {
    let mut numbers: Vec<String> = sqlx::query_scalar(
        "SELECT invoice_number FROM invoices WHERE locked_at IS NOT NULL
         UNION SELECT quote_number FROM quotes WHERE locked_at IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    numbers.sort();
    Ok(numbers)
}

/// IDs derselben festgeschriebenen Belege wie [`snapshot_locked_numbers`]
/// (Phase A, Lauf F3b). Damit erkennt das erste Entsperren, ob ein Beleg der
/// eingespielten Datenbank unter einer Nummer der ersetzten ein **anderer**
/// Datensatz ist ([`foreign_numbers`]) — dann stammt die ersetzte Datenbank
/// von einer anderen Instanz.
pub async fn snapshot_locked_ids(pool: &SqlitePool) -> Result<Vec<String>> {
    let mut ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM invoices WHERE locked_at IS NOT NULL
         UNION SELECT id FROM quotes WHERE locked_at IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    ids.sort();
    Ok(ids)
}

/// Nummern festgeschriebener Belege der eingespielten Datenbank (`restored`:
/// Nummer, ID), die es in der ersetzten Datenbank unter derselben Nummer, aber
/// als anderen Datensatz gab (Lauf F3b, pure). Ein älteres Backup derselben
/// Instanz enthält nur Belege, die auch die ersetzte DB unverändert hatte
/// (festgeschriebene Belege ändern sich nie). Ohne IDs im Marker (Marker vor
/// F3b) bleibt die Prüfung leer.
pub fn foreign_numbers(
    restored: &[(String, String)],
    locked_numbers: &[String],
    locked_ids: &[String],
) -> Vec<String> {
    if locked_ids.is_empty() {
        return Vec::new();
    }
    let numbers: BTreeSet<&str> = locked_numbers.iter().map(String::as_str).collect();
    let ids: BTreeSet<&str> = locked_ids.iter().map(String::as_str).collect();
    let mut out: Vec<String> = restored
        .iter()
        .filter(|(number, id)| numbers.contains(number.as_str()) && !ids.contains(id.as_str()))
        .map(|(number, _)| number.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// [`foreign_numbers`] gegen die eingespielte Datenbank (in der Transaktion
/// des Restore-Abschlusses).
pub async fn foreign_numbers_in(
    conn: &mut SqliteConnection,
    locked_numbers: &[String],
    locked_ids: &[String],
) -> Result<Vec<String>> {
    if locked_ids.is_empty() {
        return Ok(Vec::new());
    }
    let restored: Vec<(String, String)> = sqlx::query_as(
        "SELECT invoice_number, id FROM invoices WHERE locked_at IS NOT NULL
         UNION ALL SELECT quote_number, id FROM quotes WHERE locked_at IS NOT NULL",
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(foreign_numbers(&restored, locked_numbers, locked_ids))
}

/// Vorgerückte Reihe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advanced {
    pub kind: &'static str,
    pub id: String,
    pub label: String,
    pub from: String,
    pub to: String,
}

/// Reihe, die auf die Entscheidung wartet (Stand unbekannt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub kind: &'static str,
    pub id: String,
    pub label: String,
    pub next_due: String,
    /// Pausiert: Die Wahl fällt beim Fortsetzen (F3b), nicht über „Rückstand …“.
    pub paused: bool,
}

/// Wiederhergestellter Entwurf mit vor dem Restore festgeschriebener Nummer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// `invoice` (auch Storno) oder `quote`.
    pub kind: String,
    pub id: String,
    pub number: String,
}

/// Ergebnis des Abgleichs.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub advanced: Vec<Advanced>,
    pub review: Vec<Review>,
    /// Nummern der Entwürfe begonnener Abo-Rechnungen (Marker verworfen).
    pub pending_drafts: Vec<String>,
    /// Verworfene Bestätigungen zum Nachholen.
    pub confirmations_cleared: usize,
    pub conflicts: Vec<Conflict>,
}

/// Gleicht die eingespielte Datenbank ab (in der Transaktion des
/// Restore-Audits, siehe Moduldoku). `today` = Tag der Wiederherstellung
/// (Europe/Berlin, injizierte Uhr).
pub async fn apply_in(
    conn: &mut SqliteConnection,
    series: &[SeriesState],
    locked_numbers: &[String],
    today: NaiveDate,
) -> Result<Outcome> {
    let mut out = Outcome::default();
    let known: BTreeMap<(&str, &str), &str> = series
        .iter()
        .map(|s| ((s.kind.as_str(), s.id.as_str()), s.next_due_date.as_str()))
        .collect();

    // Je Reihe: Laufzeitende (nur Vorlagen), aktiv, und ob ein Lauf für sie
    // überhaupt Belege erzeugt (Vorlagen immer, Kosten-Abos nur mit
    // Auto-Buchung) — F3b, für die Prüfung ohne bekannten Stand.
    for (kind, query, series_kind, action) in [
        (
            KIND_TEMPLATE,
            "SELECT id, label, next_due_date, end_date, active, 1
               FROM recurring_invoices ORDER BY id",
            Series::Template,
            "recurring_invoice",
        ),
        (
            KIND_SUBSCRIPTION,
            "SELECT id, label, next_due_date, NULL, active, auto_create_expense
               FROM recurring_subscriptions ORDER BY id",
            Series::Subscription,
            "recurring",
        ),
    ] {
        let rows: Vec<(String, String, String, Option<String>, i64, i64)> =
            sqlx::query_as(query).fetch_all(&mut *conn).await?;
        for (id, label, next_due, end_date, active, books) in rows {
            let before: Option<String> = known.get(&(kind, id.as_str())).map(|s| s.to_string());
            match before {
                Some(before) => {
                    // Nur vorwärts: ISO-Daten vergleichen lexikografisch richtig.
                    if parse(&before).is_none() || parse(&next_due).is_none() {
                        continue;
                    }
                    if before > next_due {
                        if kind == KIND_TEMPLATE {
                            crate::db::repo::recurring_invoice::advance_in(
                                &mut *conn, &id, &next_due, None, &before,
                            )
                            .await?;
                        } else {
                            crate::db::repo::recurring::advance_in(
                                &mut *conn, &id, &next_due, None, &before,
                            )
                            .await?;
                        }
                        audit_log::append_in(
                            &mut *conn,
                            &format!("{action}.advanced_after_restore"),
                            action,
                            &id,
                            Some(serde_json::json!({
                                "label": label,
                                "from": next_due,
                                "to": before,
                            })),
                        )
                        .await?;
                        out.advanced.push(Advanced {
                            kind,
                            id,
                            label,
                            from: next_due,
                            to: before,
                        });
                    }
                }
                None => {
                    // Nur Reihen, für die ein Lauf etwas erzeugen würde und
                    // die Listen „Rückstand …“ bzw. das Fortsetzen eine Wahl
                    // anbieten (F3b): Vorlagen innerhalb der Laufzeit,
                    // Kosten-Abos mit Auto-Buchung. Für eine abgelaufene
                    // Vorlage blieb der Marker sonst für immer stehen, für ein
                    // Abo ohne Auto-Buchung gab es keinen Handlungsweg.
                    let due = parse(&next_due).filter(|d| *d <= today);
                    let within = end_date
                        .as_deref()
                        .and_then(parse)
                        .is_none_or(|end| due.is_some_and(|d| d <= end));
                    if due.is_some() && within && books == 1 {
                        catch_up::set_restore_review_in(&mut *conn, series_kind, &id, today)
                            .await?;
                        audit_log::append_in(
                            &mut *conn,
                            &format!("{action}.restore_review"),
                            action,
                            &id,
                            Some(serde_json::json!({
                                "label": label,
                                "next_due": next_due,
                                "restored_on": today.to_string(),
                                "paused": active == 0,
                            })),
                        )
                        .await?;
                        out.review.push(Review {
                            kind,
                            id,
                            label,
                            next_due,
                            paused: active == 0,
                        });
                    }
                }
            }
        }
    }

    // Marker aus dem Backup verwerfen (Fortsetzen und Bestätigungen). Präfix
    // exakt vergleichen (`_` wäre in LIKE ein Platzhalter).
    let pending_prefix = crate::scheduler::recurring_invoice::PENDING_PREFIX;
    let pending: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, value FROM app_settings WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key",
    )
    .bind(pending_prefix)
    .fetch_all(&mut *conn)
    .await?;
    for (_, value) in &pending {
        if let Some(invoice_id) = value.split('|').next() {
            let number: Option<String> =
                sqlx::query_scalar("SELECT invoice_number FROM invoices WHERE id = ?")
                    .bind(invoice_id)
                    .fetch_optional(&mut *conn)
                    .await?;
            out.pending_drafts.extend(number);
        }
    }
    let confirmations =
        sqlx::query("DELETE FROM app_settings WHERE substr(key, 1, length(?1)) = ?1")
            .bind(catch_up::CONFIRM_PREFIX)
            .execute(&mut *conn)
            .await?
            .rows_affected();
    sqlx::query("DELETE FROM app_settings WHERE substr(key, 1, length(?1)) = ?1")
        .bind(pending_prefix)
        .execute(&mut *conn)
        .await?;
    out.confirmations_cleared = confirmations as usize;
    if !pending.is_empty() || confirmations > 0 {
        audit_log::append_in(
            &mut *conn,
            "recurring.markers_cleared_after_restore",
            "recurring",
            "restore",
            Some(serde_json::json!({
                "pending": pending.len(),
                "pending_drafts": out.pending_drafts,
                "confirmations": confirmations,
            })),
        )
        .await?;
    }

    // Entwürfe mit vor dem Restore festgeschriebener Nummer vormerken.
    let locked: BTreeSet<&str> = locked_numbers.iter().map(String::as_str).collect();
    if !locked.is_empty() {
        let drafts: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT 'invoice', id, invoice_number FROM invoices
              WHERE locked_at IS NULL AND voided_at IS NULL
             UNION ALL SELECT 'quote', id, quote_number FROM quotes
              WHERE locked_at IS NULL AND status = 'draft'
             ORDER BY 3",
        )
        .fetch_all(&mut *conn)
        .await?;
        for (kind, id, number) in drafts {
            if !locked.contains(number.as_str()) {
                continue;
            }
            app_settings::set_in(&mut *conn, &conflict_key(&kind, &id), &number).await?;
            audit_log::append_in(
                &mut *conn,
                "numbering.restored_draft_flagged",
                &kind,
                &id,
                Some(serde_json::json!({ "number": number })),
            )
            .await?;
            out.conflicts.push(Conflict { kind, id, number });
        }
    }
    Ok(out)
}

fn parse(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

fn de_date(iso: &str) -> String {
    parse(iso)
        .map(|d| d.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|| iso.to_string())
}

/// Hinweis „Abo-Termine nach der Wiederherstellung“ (nach dem Commit; nur wenn
/// etwas vorgerückt, zu prüfen oder verworfen wurde).
pub async fn notice(pool: &SqlitePool, staged_at: &str, outcome: &Outcome) {
    let Some((body, severity)) = notice_body(outcome) else {
        return;
    };
    let dedup = format!("restore_recurring:{staged_at}");
    if let Err(e) = crate::notify::store::create(
        pool,
        crate::notify::NewNotification {
            title: "Abo-Termine nach der Wiederherstellung",
            body: &body,
            severity,
            action_url: Some("/recurring-invoices"),
            dedup_key: Some(&dedup),
            ..Default::default()
        },
    )
    .await
    {
        tracing::warn!("Hinweis zu den Abo-Terminen nicht angelegt: {e}");
    }
}

/// Text und Schwere des Hinweises „Abo-Termine nach der Wiederherstellung“
/// (pure); `None`, wenn nichts zu melden ist.
///
/// Vorgerückte Abo-Rechnungen und Kosten-Abos stehen getrennt (F3b): Eine
/// Rechnung ging hinaus, ihr Termin darf nicht erneut abgerechnet werden.
/// Gebuchte Kosten dagegen hatten keine Außenwirkung und sind mit dem
/// ersetzten Stand verworfen — sie fehlen in Kosten und EÜR und sind von Hand
/// nachzutragen. Pausierte Reihen ohne bekannten Stand entscheidet der Inhaber
/// beim Fortsetzen, nicht über „Rückstand …“.
pub fn notice_body(outcome: &Outcome) -> Option<(String, &'static str)> {
    let mut parts = Vec::new();
    let advanced = |kind: &str| {
        outcome
            .advanced
            .iter()
            .filter(|a| a.kind == kind)
            .map(|a| format!("„{}“ {} → {}", a.label, de_date(&a.from), de_date(&a.to)))
            .collect::<Vec<_>>()
    };
    let templates = advanced(KIND_TEMPLATE);
    if !templates.is_empty() {
        parts.push(format!(
            "Abo-Rechnungen: Diese Vorlagen standen im eingespielten Stand auf einem älteren \
             Termin. Klein.Buch hat sie auf den Stand vor der Wiederherstellung vorgerückt, damit \
             bereits abgerechnete Zeiträume nicht erneut in Rechnung gestellt werden: {}. Die \
             Rechnungen dieser Termine fehlen in dieser Datenbank (Lücke im Nummernkreis, siehe \
             Audit-Log). Wurde ein Termin vor der Wiederherstellung doch nicht abgerechnet, bitte \
             von Hand erstellen.",
            templates.join(", ")
        ));
    }
    let subscriptions = advanced(KIND_SUBSCRIPTION);
    if !subscriptions.is_empty() {
        parts.push(format!(
            "Kosten-Abos: Diese Abos standen im eingespielten Stand auf einem älteren Termin und \
             wurden auf den Stand vor der Wiederherstellung vorgerückt: {}. Die zu diesen \
             Terminen gebuchten Kosten wurden mit dem ersetzten Stand verworfen und fehlen jetzt \
             in Kosten und EÜR. Bitte anhand des Kontoauszugs unter „Kosten“ von Hand erfassen; \
             die Abos buchen erst ab dem nächsten Termin weiter.",
            subscriptions.join(", ")
        ));
    }
    if !outcome.review.is_empty() {
        let list = |paused: bool| {
            outcome
                .review
                .iter()
                .filter(|r| r.paused == paused)
                .map(|r| format!("„{}“ seit {}", r.label, de_date(&r.next_due)))
                .collect::<Vec<_>>()
        };
        let mut text = String::from(
            "Bei diesen Reihen kennt Klein.Buch den Stand vor der Wiederherstellung nicht \
             (Wiederherstellung in eine neue Instanz oder Reihe zuvor gelöscht). Ob ihre offenen \
             Termine schon abgerechnet wurden, prüfst Du im Mail-Ausgang bzw. auf dem \
             Kontoauszug.",
        );
        let active = list(false);
        if !active.is_empty() {
            text.push_str(&format!(
                " Danach in der Liste „Rückstand …“ wählen: {} — nachholen (Rechnungen nur als \
                 Entwurf) oder ab heute fortsetzen (lässt auch einen heute fälligen Termin aus).",
                active.join(", ")
            ));
        }
        let paused = list(true);
        if !paused.is_empty() {
            text.push_str(&format!(
                " Pausiert, die Wahl fällt beim Fortsetzen: {}.",
                paused.join(", ")
            ));
        }
        text.push_str(" Bis dahin entsteht für sie nichts automatisch.");
        parts.push(text);
    }
    if !outcome.pending_drafts.is_empty() {
        parts.push(format!(
            "Begonnene Abo-Rechnungen aus dem Backup werden nicht automatisch festgeschrieben oder \
             versendet: {} — bitte prüfen.",
            outcome.pending_drafts.join(", ")
        ));
    }
    if outcome.confirmations_cleared > 0 {
        parts.push(
            "Bestätigungen zum Nachholen aus dem Backup gelten nicht mehr; größere Rückstände \
             fragt Klein.Buch erneut nach."
                .to_string(),
        );
    }
    if parts.is_empty() {
        return None;
    }
    let severity = if outcome.advanced.is_empty() && outcome.review.is_empty() {
        "warning"
    } else {
        "urgent"
    };
    Some((parts.join("\n\n"), severity))
}

// ---------------------------------------------------------------------------
// Entwürfe mit vor dem Restore festgeschriebener Nummer
// ---------------------------------------------------------------------------

/// Präfix der vorgemerkten Entwürfe (`restored_number_conflict:<art>:<id>` =
/// Nummer).
pub const CONFLICT_PREFIX: &str = "restored_number_conflict:";

fn conflict_key(kind: &str, id: &str) -> String {
    format!("{CONFLICT_PREFIX}{kind}:{id}")
}

/// Verhindert das Festschreiben eines vorgemerkten Entwurfs (Rechnung, Storno,
/// Angebot), bis der Inhaber den Abgleich mit dem versandten Original
/// bestätigt hat.
pub async fn ensure_no_conflict(pool: &SqlitePool, kind: &str, id: &str) -> Result<()> {
    if let Some(number) = app_settings::get(pool, &conflict_key(kind, id)).await? {
        return Err(Error::Domain(format!(
            "Die Nummer {number} war vor der Wiederherstellung bereits festgeschrieben — dieser \
             Entwurf ist ein älterer Stand des Belegs. Bitte den Entwurf an das versandte Original \
             anpassen und unter Einstellungen → Backups → Nummernkreise bestätigen; erst dann \
             lässt er sich festschreiben (sonst gäbe es zwei verschiedene Belege mit derselben \
             Nummer)."
        )));
    }
    Ok(())
}

/// Alle vorgemerkten Entwürfe (für die Oberfläche).
pub async fn list_conflicts(pool: &SqlitePool) -> Result<Vec<Conflict>> {
    let mut out = Vec::new();
    for (key, number) in app_settings::list_prefix(pool, CONFLICT_PREFIX).await? {
        let rest = key.trim_start_matches(CONFLICT_PREFIX);
        if let Some((kind, id)) = rest.split_once(':') {
            out.push(Conflict {
                kind: kind.to_string(),
                id: id.to_string(),
                number,
            });
        }
    }
    out.sort_by(|a, b| a.number.cmp(&b.number));
    Ok(out)
}

/// Stand der Restore-Vormerkung eines Belegs (ADR 0042 Nr. 16, Warteliste 8),
/// aus derselben Quelle wie [`list_conflicts`] und [`reflag_confirmed_in`]:
/// `Flagged`, solange der Schlüssel steht; `Confirmed`, wenn die Bestätigung
/// im Audit steht und nicht wieder vorgemerkt ist — dann merkt das nächste
/// Speichern den Entwurf neu vor. Sonst `None`.
pub async fn draft_restore_state(
    pool: &SqlitePool,
    kind: &str,
    id: &str,
) -> Result<Option<crate::db::models::RestoreConflictState>> {
    use crate::db::models::RestoreConflictState;
    if app_settings::get(pool, &conflict_key(kind, id))
        .await?
        .is_some()
    {
        return Ok(Some(RestoreConflictState::Flagged));
    }
    let confirmed: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM audit_log
          WHERE action = 'numbering.restored_draft_confirmed'
            AND entity_type = ? AND entity_id = ?
          LIMIT 1",
    )
    .bind(kind)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(confirmed.map(|_| RestoreConflictState::Confirmed))
}

/// Lauf F8: Merkt einen schon bestätigten Entwurf nach einer Änderung wieder
/// vor (in der Transaktion der Änderung). Die Bestätigung galt dem Stand, der
/// dem versandten Original entsprach; danach geänderte Preise oder Empfänger
/// ergäben zwei verschiedene Belege mit derselben Nummer (§ 14 Abs. 4 Nr. 4
/// UStG). Ohne Bestätigung im Audit (kein Restore-Konflikt) geschieht nichts.
pub async fn reflag_confirmed_in(
    conn: &mut SqliteConnection,
    kind: &str,
    id: &str,
    number: &str,
) -> Result<()> {
    let confirmed: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM audit_log
          WHERE action = 'numbering.restored_draft_confirmed'
            AND entity_type = ? AND entity_id = ?
          LIMIT 1",
    )
    .bind(kind)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    if confirmed.is_none() {
        return Ok(());
    }
    let key = conflict_key(kind, id);
    let flagged: Option<String> =
        sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?")
            .bind(&key)
            .fetch_optional(&mut *conn)
            .await?;
    if flagged.is_some() {
        return Ok(());
    }
    app_settings::set_in(&mut *conn, &key, number).await?;
    audit_log::append_in(
        &mut *conn,
        "numbering.restored_draft_reflagged",
        kind,
        id,
        Some(serde_json::json!({ "number": number })),
    )
    .await?;
    Ok(())
}

/// Bestätigt den Abgleich eines vorgemerkten Entwurfs mit dem versandten
/// Original (Re-Auth im Command): Audit `numbering.restored_draft_confirmed`
/// mit Begründung, danach lässt er sich wie gewohnt festschreiben.
pub async fn confirm_conflict(pool: &SqlitePool, kind: &str, id: &str, reason: &str) -> Result<()> {
    let key = conflict_key(kind, id);
    let mut tx = crate::db::begin_immediate(pool).await?;
    let number: Option<String> = sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?")
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(number) = number else {
        return Err(Error::Domain(
            "Für diesen Beleg ist nichts zu bestätigen.".into(),
        ));
    };
    sqlx::query("DELETE FROM app_settings WHERE key = ?")
        .bind(&key)
        .execute(&mut *tx)
        .await?;
    audit_log::append_in(
        &mut tx,
        "numbering.restored_draft_confirmed",
        kind,
        id,
        Some(serde_json::json!({ "number": number, "reason": reason })),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    /// F3b: Derselbe Beleg (gleiche ID) ist kein fremder Stand; dieselbe Nummer
    /// mit anderer ID schon; Nummern, die die ersetzte DB nicht kannte, zählen
    /// hier nicht (die prüft der Zählerstand). Marker ohne IDs: keine Prüfung.
    #[test]
    fn foreign_numbers_needs_same_number_with_other_id() {
        let numbers = vec![s("RE-2026-0001"), s("RE-2026-0002")];
        let ids = vec![s("id-1"), s("id-2")];
        let same = vec![(s("RE-2026-0001"), s("id-1"))];
        assert!(foreign_numbers(&same, &numbers, &ids).is_empty());
        let other = vec![
            (s("RE-2026-0001"), s("id-x")),
            (s("RE-2026-0002"), s("id-2")),
            (s("RE-2026-0003"), s("id-3")),
        ];
        assert_eq!(
            foreign_numbers(&other, &numbers, &ids),
            vec![s("RE-2026-0001")]
        );
        assert!(foreign_numbers(&other, &numbers, &[]).is_empty());
    }

    fn advanced(kind: &'static str, label: &str) -> Advanced {
        Advanced {
            kind,
            id: s("a"),
            label: s(label),
            from: s("2026-10-01"),
            to: s("2026-11-01"),
        }
    }

    fn review(label: &str, paused: bool) -> Review {
        Review {
            kind: KIND_TEMPLATE,
            id: s(label),
            label: s(label),
            next_due: s("2026-10-01"),
            paused,
        }
    }

    /// F3b: Vorgerückte Kosten-Abos nennen die verworfenen Kosten und das
    /// Nachtragen von Hand, getrennt von den Abo-Rechnungen; pausierte Reihen
    /// ohne bekannten Stand verweisen aufs Fortsetzen, nicht auf „Rückstand …“.
    #[test]
    fn notice_separates_costs_and_paused_series() {
        assert!(notice_body(&Outcome::default()).is_none());

        let outcome = Outcome {
            advanced: vec![
                advanced(KIND_TEMPLATE, "Wartung"),
                advanced(KIND_SUBSCRIPTION, "Hosting"),
            ],
            ..Default::default()
        };
        let (body, severity) = notice_body(&outcome).unwrap();
        assert_eq!(severity, "urgent");
        let (inv, cost) = body.split_once("\n\n").unwrap();
        assert!(
            inv.starts_with("Abo-Rechnungen:") && inv.contains("„Wartung“"),
            "{body}"
        );
        assert!(!inv.contains("Hosting"), "{body}");
        assert!(
            cost.starts_with("Kosten-Abos:") && cost.contains("„Hosting“"),
            "{body}"
        );
        assert!(
            cost.contains("verworfen") && cost.contains("von Hand erfassen"),
            "{body}"
        );
        assert!(cost.contains("01.10.2026 → 01.11.2026"), "{body}");

        let only_paused = Outcome {
            review: vec![review("Pausiert", true)],
            ..Default::default()
        };
        let (body, _) = notice_body(&only_paused).unwrap();
        assert!(body.contains("beim Fortsetzen: „Pausiert“"), "{body}");
        assert!(!body.contains("Rückstand"), "{body}");

        let mixed = Outcome {
            review: vec![review("Aktiv", false), review("Pausiert", true)],
            ..Default::default()
        };
        let (body, _) = notice_body(&mixed).unwrap();
        assert!(
            body.contains("„Rückstand …“ wählen: „Aktiv“ seit 01.10.2026"),
            "{body}"
        );
        assert!(
            body.contains("beim Fortsetzen: „Pausiert“ seit 01.10.2026"),
            "{body}"
        );
    }
}
