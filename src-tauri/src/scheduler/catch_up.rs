//! Nachhol-Regeln für Abo-Läufe (Lauf F, W14-026, ADR 0040 Nr. 3).
//!
//! Nach einer längeren Sperre, einer Reaktivierung oder einem Uhrsprung sind
//! mehrere Termine einer Reihe fällig. Klein.Buch holt sie sichtbar und
//! begrenzt nach:
//!
//! - Bis [`AUTO_LIMIT`] fällige Termine je Reihe holt der Lauf selbst nach.
//!   Bei Abo-Rechnungen folgt nur der **jüngste** Termin der Automatik-Stufe
//!   der Vorlage (festschreiben, ggf. versenden); ältere Termine entstehen als
//!   **Entwurf** — nachgeholte Rechnungen gehen nie automatisch hinaus. Ein
//!   Sammelhinweis nennt die Entwürfe.
//! - Mehr fällige Termine: der Lauf erzeugt nichts, sondern legt einen Hinweis
//!   an („Entscheidung nötig“). Der Inhaber wählt „nachholen“ (Bestätigung,
//!   höchstens [`CONFIRMED_CAP`] Termine) oder „ab heute fortsetzen“.
//! - Die Bestätigung steht als Marker in `app_settings`
//!   (`recurring_catch_up_ok:<art>:<id>` = Tag der Bestätigung): Sie gilt für
//!   alle Termine bis zu diesem Tag — auch wenn ein Lauf abbricht und der
//!   nächste fortsetzt — und fällt weg, sobald der nächste Termin danach liegt.
//!   Termine **nach** dem Tag der Bestätigung folgen wieder den normalen Regeln
//!   (bis [`AUTO_LIMIT`] selbst, sonst neue Entscheidung; F1d).
//! - **Prüfung nach einer Wiederherstellung (F1d):** Kennt Klein.Buch den Stand
//!   einer Reihe vor dem Restore nicht (Disaster Recovery in eine leere
//!   Instanz, Reihe in der ersetzten Datenbank gelöscht), steht ein Marker
//!   `recurring_restore_review:<art>:<id>` = Tag der Wiederherstellung. Jeder
//!   offene Termin bis zu diesem Tag verlangt dann die Entscheidung —
//!   unabhängig von [`AUTO_LIMIT`]; nachgeholte Rechnungen entstehen nur als
//!   Entwurf, und „ab heute fortsetzen“ lässt auch den heutigen Termin aus (er
//!   kann vor dem Ausfall schon abgerechnet worden sein).
//!
//! Belegdatum-Regeln bleiben unverändert (GoBD/§14 UStG): Abo-Rechnungen tragen
//! das Erstellungsdatum, der Abrechnungszeitraum ist ihr Leistungszeitraum (ADR
//! 0042 Nr. 10); Abo-Kosten tragen den Stichtag als Belegdatum.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::repo::app_settings;
use crate::error::Result;

/// So viele fällige Termine je Reihe holt ein Lauf ohne Rückfrage nach.
pub const AUTO_LIMIT: usize = 3;
/// Höchstzahl nachzuholender Termine nach ausdrücklicher Bestätigung.
pub const CONFIRMED_CAP: usize = 24;
/// Zählgrenze für fällige Termine (Schutz vor Endlosschleifen).
pub const COUNT_CAP: usize = 1000;

/// Wahl beim Fortsetzen einer pausierten Reihe bzw. bei einem Rückstand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResumeMode {
    /// Verpasste Termine nachholen (bestätigt).
    CatchUp,
    /// Verpasste Termine auslassen, ab dem ersten Termin ab heute fortsetzen.
    Skip,
}

/// Art der Reihe (für den Marker-Schlüssel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Series {
    /// Kosten-Abo (`recurring_subscriptions`).
    Subscription,
    /// Abo-Rechnungs-Vorlage (`recurring_invoices`).
    Template,
}

/// Präfix der Bestätigungen (`recurring_catch_up_ok:<art>:<id>`).
pub const CONFIRM_PREFIX: &str = "recurring_catch_up_ok:";
/// Präfix der Prüfung nach einer Wiederherstellung
/// (`recurring_restore_review:<art>:<id>` = Tag der Wiederherstellung).
pub const REVIEW_PREFIX: &str = "recurring_restore_review:";

impl Series {
    /// Kürzel im Marker-Schlüssel.
    pub fn kind(self) -> &'static str {
        match self {
            Series::Subscription => "sub",
            Series::Template => "inv",
        }
    }
}

fn marker_key(series: Series, id: &str) -> String {
    format!("{CONFIRM_PREFIX}{}:{id}", series.kind())
}

/// Schlüssel der Prüfung nach einer Wiederherstellung.
pub fn review_key(series: Series, id: &str) -> String {
    format!("{REVIEW_PREFIX}{}:{id}", series.kind())
}

/// Tag, bis zu dem der Inhaber das Nachholen bestätigt hat — `Some`, solange
/// `next_due` noch im bestätigten Zeitraum liegt (bis einschließlich Tag der
/// Bestätigung).
pub async fn confirmed_through(
    pool: &SqlitePool,
    series: Series,
    id: &str,
    next_due: &str,
) -> Result<Option<NaiveDate>> {
    marker_through(pool, &marker_key(series, id), next_due).await
}

/// Tag der Wiederherstellung, wenn für die Reihe die Prüfung nach einem
/// Restore offen ist und `next_due` nicht danach liegt (F1d).
pub async fn restore_review_through(
    pool: &SqlitePool,
    series: Series,
    id: &str,
    next_due: &str,
) -> Result<Option<NaiveDate>> {
    marker_through(pool, &review_key(series, id), next_due).await
}

async fn marker_through(pool: &SqlitePool, key: &str, next_due: &str) -> Result<Option<NaiveDate>> {
    let Some(through) = app_settings::get(pool, key).await? else {
        return Ok(None);
    };
    Ok(match (parse(&through), parse(next_due)) {
        (Some(through), Some(next)) if next <= through => Some(through),
        _ => None,
    })
}

/// Setzt die Prüfung nach einer Wiederherstellung (in der Restore-Transaktion).
pub async fn set_restore_review_in(
    conn: &mut sqlx::SqliteConnection,
    series: Series,
    id: &str,
    day: NaiveDate,
) -> Result<()> {
    app_settings::set_in(conn, &review_key(series, id), &day.to_string()).await
}

/// Entfernt die Prüfung nach einer Wiederherstellung (Entscheidung getroffen).
pub async fn clear_restore_review(pool: &SqlitePool, series: Series, id: &str) -> Result<()> {
    app_settings::delete(pool, &review_key(series, id)).await
}

/// Reihen einer Art mit offener Prüfung nach einer Wiederherstellung (für
/// die Listen der Oberfläche).
pub async fn restore_review_ids(pool: &SqlitePool, series: Series) -> Result<Vec<String>> {
    let prefix = format!("{REVIEW_PREFIX}{}:", series.kind());
    Ok(app_settings::list_prefix(pool, &prefix)
        .await?
        .into_iter()
        .map(|(k, _)| k.trim_start_matches(prefix.as_str()).to_string())
        .collect())
}

/// Bestätigt das Nachholen aller Termine bis einschließlich `through` (heute).
pub async fn confirm(
    pool: &SqlitePool,
    series: Series,
    id: &str,
    through: NaiveDate,
) -> Result<()> {
    app_settings::set(pool, &marker_key(series, id), &through.to_string()).await
}

/// Entfernt eine Bestätigung (überholt, z. B. „ab heute fortsetzen“ oder ein
/// neuer Stichtag).
pub async fn clear(pool: &SqlitePool, series: Series, id: &str) -> Result<()> {
    app_settings::delete(pool, &marker_key(series, id)).await
}

/// Entfernt Bestätigung und Restore-Prüfung, sobald der nächste Termin nach
/// dem jeweiligen Tag liegt (alles Bestätigte bzw. zu Prüfende erledigt).
pub async fn clear_if_done(
    pool: &SqlitePool,
    series: Series,
    id: &str,
    next_due: &str,
) -> Result<()> {
    for key in [marker_key(series, id), review_key(series, id)] {
        if let Some(through) = app_settings::get(pool, &key).await? {
            let done = match (parse(&through), parse(next_due)) {
                (Some(through), Some(next)) => next > through,
                _ => true,
            };
            if done {
                app_settings::delete(pool, &key).await?;
            }
        }
    }
    Ok(())
}

fn parse(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

/// Was ein Lauf mit einer Reihe tut (pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Nichts fällig.
    Nothing,
    /// Bis zu `n` Termine selbst nachholen.
    Run { n: usize },
    /// Zu viele Termine ohne Bestätigung: nur Hinweis.
    AwaitDecision { due: usize },
}

/// Plan aus der Zahl fälliger Termine und der Bestätigung (pure).
pub fn plan(due: usize, confirmed: bool) -> Plan {
    match due {
        0 => Plan::Nothing,
        n if n <= AUTO_LIMIT => Plan::Run { n },
        n if confirmed => Plan::Run {
            n: n.min(CONFIRMED_CAP),
        },
        n => Plan::AwaitDecision { due: n },
    }
}

/// Plan mit bestätigtem Zeitraum und Restore-Prüfung (pure, F1d).
///
/// - `due`: fällige Termine bis heute.
/// - `within_confirmed`: davon im bestätigten Zeitraum (`None` = keine
///   gültige Bestätigung). Die Bestätigung deckt nur Termine bis zu ihrem Tag;
///   spätere Termine folgen den normalen Regeln: bis [`AUTO_LIMIT`] werden sie
///   mitgenommen, bei mehr verlangt der nächste Lauf nach dem bestätigten
///   Abschnitt eine neue Entscheidung.
/// - `review`: Prüfung nach einer Wiederherstellung offen — ohne Bestätigung
///   wartet die Reihe auf die Entscheidung, unabhängig von [`AUTO_LIMIT`].
pub fn plan_for(due: usize, within_confirmed: Option<usize>, review: bool) -> Plan {
    if due == 0 {
        return Plan::Nothing;
    }
    match within_confirmed.filter(|w| *w > 0) {
        Some(within) => {
            let within = within.min(due);
            let after = due - within;
            let extra = if after <= AUTO_LIMIT { after } else { 0 };
            Plan::Run {
                n: within.min(CONFIRMED_CAP) + extra,
            }
        }
        None if review => Plan::AwaitDecision { due },
        None => plan(due, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_respects_limits() {
        assert_eq!(plan(0, false), Plan::Nothing);
        assert_eq!(plan(1, false), Plan::Run { n: 1 });
        assert_eq!(plan(AUTO_LIMIT, false), Plan::Run { n: AUTO_LIMIT });
        assert_eq!(
            plan(AUTO_LIMIT + 1, false),
            Plan::AwaitDecision {
                due: AUTO_LIMIT + 1
            }
        );
        assert_eq!(plan(AUTO_LIMIT + 1, true), Plan::Run { n: AUTO_LIMIT + 1 });
        assert_eq!(plan(500, true), Plan::Run { n: CONFIRMED_CAP });
    }

    /// F1d: Die Bestätigung deckt nur Termine bis zu ihrem Tag; die
    /// Restore-Prüfung verlangt ohne Bestätigung immer die Entscheidung.
    #[test]
    fn plan_for_limits_confirmation_and_review() {
        assert_eq!(plan_for(0, Some(3), true), Plan::Nothing);
        // ohne Bestätigung wie plan()
        assert_eq!(plan_for(2, None, false), Plan::Run { n: 2 });
        assert_eq!(plan_for(5, None, false), Plan::AwaitDecision { due: 5 });
        // Restore-Prüfung: auch ein einzelner Termin wartet
        assert_eq!(plan_for(1, None, true), Plan::AwaitDecision { due: 1 });
        // 5 bestätigt, 5 danach (> AUTO_LIMIT) → nur die bestätigten
        assert_eq!(plan_for(10, Some(5), false), Plan::Run { n: 5 });
        // 5 bestätigt, 2 danach → beide mit
        assert_eq!(plan_for(7, Some(5), false), Plan::Run { n: 7 });
        // bestätigt mit Prüfung: Lauf (Entwürfe erzwingt der Aufrufer)
        assert_eq!(plan_for(2, Some(2), true), Plan::Run { n: 2 });
        // Obergrenze der Bestätigung bleibt
        assert_eq!(
            plan_for(40, Some(30), false),
            Plan::Run { n: CONFIRMED_CAP }
        );
    }

    #[test]
    fn resume_mode_json() {
        let m: ResumeMode = serde_json::from_str("\"catchUp\"").unwrap();
        assert_eq!(m, ResumeMode::CatchUp);
        let m: ResumeMode = serde_json::from_str("\"skip\"").unwrap();
        assert_eq!(m, ResumeMode::Skip);
    }
}
