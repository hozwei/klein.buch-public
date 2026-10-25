//! Pause der automatischen Belegerzeugung (Lauf F, ADR 0040 Nr. 4/5).
//!
//! Solange eine Pause besteht, erzeugen weder der Tick noch die manuellen
//! Auslöser Abo-Rechnungen, Abo-Kosten oder die Auto-AfA. Backups,
//! Integritätsprüfung und Hinweise laufen weiter. Aufheben kann nur der Inhaber
//! im Browser (`scheduler_release_hold`, Re-Auth, Audit `scheduler.hold_released`).
//!
//! Gründe (`app_settings.scheduler_hold`):
//! - `clock_backwards` — die Uhr steht mehr als [`BACKWARD_TOLERANCE`] vor dem
//!   zuletzt gesehenen Zeitpunkt (Uhr zurückgestellt, NTP-Fehler).
//! - `clock_jump` — seit dem zuletzt gesehenen Zeitpunkt sind mehr als
//!   [`FORWARD_LIMIT_DAYS`] Tage vergangen (Uhrsprung nach vorn, VM-Snapshot,
//!   oder die Instanz war über ein Jahr nicht entsperrt). ASVS V11.1.5.
//! - `restore` — nach einer Wiederherstellung (gesetzt beim Anwenden des
//!   Restores; Lauf F, W14-023).
//!
//! „Zuletzt gesehen“ (`scheduler_last_seen_at`) schreibt jeder ungebremste
//! Lauf höchstens einmal je Stunde fort. Die Pause steht in der Datenbank und
//! übersteht damit Neustart und Sperren.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::clock;
use crate::db::repo::app_settings;
use crate::error::Result;
use crate::notify::{self, NewNotification};

/// Schlüssel: Grund der Pause (fehlt = keine Pause).
pub const KEY_REASON: &str = "scheduler_hold";
/// Schlüssel: Beginn der Pause (UTC, DB-Format).
pub const KEY_SINCE: &str = "scheduler_hold_since";
/// Schlüssel: zuletzt von einem ungebremsten Lauf gesehene Zeit (UTC).
pub const KEY_LAST_SEEN: &str = "scheduler_last_seen_at";

/// So weit darf die Uhr hinter dem zuletzt gesehenen Zeitpunkt stehen.
pub const BACKWARD_TOLERANCE: Duration = Duration::hours(1);
/// Mehr Tage seit dem zuletzt gesehenen Zeitpunkt gelten als Uhrsprung.
pub const FORWARD_LIMIT_DAYS: i64 = 400;
/// Fortschreiben von „zuletzt gesehen“ höchstens so oft.
const LAST_SEEN_STEP: Duration = Duration::hours(1);

pub const REASON_CLOCK_BACKWARDS: &str = "clock_backwards";
pub const REASON_CLOCK_JUMP: &str = "clock_jump";
pub const REASON_RESTORE: &str = "restore";

/// Bestehende Pause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hold {
    pub reason: String,
    pub since: Option<String>,
}

impl Hold {
    /// Erklärung für Hinweis, Banner und Fehlermeldung.
    pub fn message(&self) -> String {
        reason_text(&self.reason).to_string()
    }
}

/// Status für die Oberfläche (`scheduler_hold_status`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldStatus {
    pub held: bool,
    pub reason: Option<String>,
    pub since: Option<String>,
    pub message: Option<String>,
}

/// Ergebnis der Uhr-Prüfung (pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockVerdict {
    Plausible,
    Backwards,
    Jump,
}

/// Bewertet `now` gegen den zuletzt gesehenen Zeitpunkt (pure).
pub fn assess_clock(last_seen: Option<DateTime<Utc>>, now: DateTime<Utc>) -> ClockVerdict {
    let Some(last) = last_seen else {
        return ClockVerdict::Plausible;
    };
    if now < last - BACKWARD_TOLERANCE {
        ClockVerdict::Backwards
    } else if now > last + Duration::days(FORWARD_LIMIT_DAYS) {
        ClockVerdict::Jump
    } else {
        ClockVerdict::Plausible
    }
}

/// Deutsche Erklärung je Grund.
pub fn reason_text(reason: &str) -> &'static str {
    match reason {
        REASON_CLOCK_BACKWARDS => {
            "Die Uhr des Servers steht vor dem Zeitpunkt des letzten Laufs. Bis zur Klärung \
             erzeugt Klein.Buch keine Abo-Rechnungen, Abo-Kosten und Abschreibungen automatisch. \
             Bitte Uhrzeit und Zeitsynchronisation des Servers prüfen und danach freigeben."
        }
        REASON_CLOCK_JUMP => {
            "Seit dem letzten Lauf sind laut Server-Uhr mehr als 400 Tage vergangen. Das deutet \
             auf eine falsch gestellte Uhr hin (oder die Instanz war über ein Jahr gesperrt). Bis \
             zur Freigabe erzeugt Klein.Buch keine Abo-Rechnungen, Abo-Kosten und Abschreibungen \
             automatisch. Bitte Datum und Uhrzeit prüfen und danach freigeben."
        }
        REASON_RESTORE => {
            "Nach der Wiederherstellung eines Backups sind automatische Abo-Rechnungen, Abo-Kosten \
             und Abschreibungen angehalten, und Backups gehen nur in den lokalen Ordner (keine \
             Spiegelung auf die externen Ziele). Bitte Rechnungen, Kosten, Nummernkreise sowie \
             Abo-Vorlagen und ihre nächsten Termine prüfen. Freigeben nur, wenn dies die \
             produktive Instanz ist — eine Probe-Instanz nie freigeben."
        }
        _ => {
            "Automatische Abo-Läufe und Abschreibungen sind angehalten. Bitte prüfen und freigeben."
        }
    }
}

/// Aktuelle Pause (ohne die Uhr zu prüfen).
pub async fn current(pool: &SqlitePool) -> Result<Option<Hold>> {
    let Some(reason) = app_settings::get(pool, KEY_REASON)
        .await?
        .filter(|r| !r.trim().is_empty())
    else {
        return Ok(None);
    };
    let since = app_settings::get(pool, KEY_SINCE).await?;
    Ok(Some(Hold { reason, since }))
}

/// Status für die Oberfläche.
pub async fn status(pool: &SqlitePool) -> Result<HoldStatus> {
    Ok(match current(pool).await? {
        Some(h) => HoldStatus {
            held: true,
            message: Some(h.message()),
            reason: Some(h.reason),
            since: h.since,
        },
        None => HoldStatus {
            held: false,
            reason: None,
            since: None,
            message: None,
        },
    })
}

/// Setzt eine Pause (idempotent: eine bestehende Pause bleibt mit ihrem Grund
/// stehen) und legt einen Hinweis an.
pub async fn set(pool: &SqlitePool, reason: &str, now: DateTime<Utc>) -> Result<Hold> {
    if let Some(existing) = current(pool).await? {
        return Ok(existing);
    }
    let since = clock::to_db_utc(now);
    app_settings::set(pool, KEY_REASON, reason).await?;
    app_settings::set(pool, KEY_SINCE, &since).await?;
    crate::db::repo::audit_log::append(
        pool,
        "scheduler.hold_set",
        "scheduler",
        "hold",
        Some(serde_json::json!({ "reason": reason, "since": since })),
    )
    .await?;
    let hold = Hold {
        reason: reason.to_string(),
        since: Some(since.clone()),
    };
    notice(pool, &hold).await;
    tracing::warn!("Automatische Belegerzeugung angehalten (Grund: {reason})");
    Ok(hold)
}

/// Setzt die Pause nach einer Wiederherstellung innerhalb der Transaktion des
/// Restore-Audits (W14-023): Grund `restore` ersetzt eine aus dem Backup
/// mitgebrachte Pause (er ist der strengere — er setzt auch die Off-Site-
/// Spiegelung aus, D-23). Audit `scheduler.hold_set`. Den Hinweis legt der
/// Aufrufer nach dem Commit mit [`notice`] an.
pub async fn set_restore_in(conn: &mut sqlx::SqliteConnection, now: DateTime<Utc>) -> Result<Hold> {
    let since = clock::to_db_utc(now);
    app_settings::set_in(&mut *conn, KEY_REASON, REASON_RESTORE).await?;
    app_settings::set_in(&mut *conn, KEY_SINCE, &since).await?;
    crate::db::repo::audit_log::append_in(
        &mut *conn,
        "scheduler.hold_set",
        "scheduler",
        "hold",
        Some(serde_json::json!({ "reason": REASON_RESTORE, "since": since })),
    )
    .await?;
    Ok(Hold {
        reason: REASON_RESTORE.to_string(),
        since: Some(since),
    })
}

/// Hinweis zur Pause (einmal je Pause).
pub async fn notice(pool: &SqlitePool, hold: &Hold) {
    let body = hold.message();
    let dedup = format!(
        "scheduler_hold:{}:{}",
        hold.reason,
        hold.since.as_deref().unwrap_or("-")
    );
    if let Err(e) = notify::store::create(
        pool,
        NewNotification {
            title: "Automatische Abo-Läufe angehalten",
            body: &body,
            severity: "urgent",
            action_url: Some("/notifications"),
            dedup_key: Some(&dedup),
            ..Default::default()
        },
    )
    .await
    {
        tracing::warn!("Hinweis zur Pause nicht angelegt: {e}");
    }
}

/// Hebt die Pause auf und setzt „zuletzt gesehen“ auf `now` (sonst bremste die
/// Uhr-Prüfung sofort wieder). Liefert den bisherigen Grund (`None` = es gab
/// keine Pause).
pub async fn release(pool: &SqlitePool, now: DateTime<Utc>) -> Result<Option<Hold>> {
    let previous = current(pool).await?;
    let mut tx = crate::db::begin_immediate(pool).await?;
    sqlx::query("DELETE FROM app_settings WHERE key IN (?, ?)")
        .bind(KEY_REASON)
        .bind(KEY_SINCE)
        .execute(&mut *tx)
        .await?;
    app_settings::set_in(&mut tx, KEY_LAST_SEEN, &clock::to_db_utc(now)).await?;
    if let Some(h) = &previous {
        crate::db::repo::audit_log::append_in(
            &mut tx,
            "scheduler.hold_released",
            "scheduler",
            "hold",
            Some(serde_json::json!({
                "reason": h.reason,
                "since": h.since,
                "released_at": clock::to_db_utc(now),
            })),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(previous)
}

/// Prüft vor jeder automatischen Belegerzeugung: besteht eine Pause, oder ist
/// die Uhr unplausibel (dann wird die Pause hier gesetzt)? `None` = Lauf darf
/// arbeiten; „zuletzt gesehen“ wird dabei fortgeschrieben.
pub async fn gate(pool: &SqlitePool, now: DateTime<Utc>) -> Result<Option<Hold>> {
    if let Some(h) = current(pool).await? {
        return Ok(Some(h));
    }
    let last_seen = app_settings::get(pool, KEY_LAST_SEEN)
        .await?
        .as_deref()
        .and_then(clock::db_utc);
    match assess_clock(last_seen, now) {
        ClockVerdict::Plausible => {
            if last_seen.is_none_or(|l| now - l >= LAST_SEEN_STEP) {
                app_settings::set(pool, KEY_LAST_SEEN, &clock::to_db_utc(now)).await?;
            }
            Ok(None)
        }
        ClockVerdict::Backwards => Ok(Some(set(pool, REASON_CLOCK_BACKWARDS, now).await?)),
        ClockVerdict::Jump => Ok(Some(set(pool, REASON_CLOCK_JUMP, now).await?)),
    }
}

/// Fehlermeldung für manuelle Auslöser während einer Pause.
pub fn blocked_error(hold: &Hold) -> crate::error::Error {
    crate::error::Error::Domain(format!(
        "Automatische Abo-Läufe sind angehalten: {} (Freigabe unter „Hinweise“ bzw. im Banner oben).",
        hold.message()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn clock_assessment() {
        let last = Some(t(2026, 10, 1, 12));
        assert_eq!(
            assess_clock(None, t(2036, 1, 1, 0)),
            ClockVerdict::Plausible
        );
        assert_eq!(
            assess_clock(last, t(2026, 10, 1, 12)),
            ClockVerdict::Plausible
        );
        // bis eine Stunde zurück: noch plausibel (Toleranz)
        assert_eq!(
            assess_clock(last, t(2026, 10, 1, 11)),
            ClockVerdict::Plausible
        );
        assert_eq!(
            assess_clock(last, t(2026, 10, 1, 10)),
            ClockVerdict::Backwards
        );
        // 400 Tage nach vorn: noch plausibel, danach Sprung
        let ok = t(2026, 10, 1, 12) + Duration::days(400);
        assert_eq!(assess_clock(last, ok), ClockVerdict::Plausible);
        assert_eq!(
            assess_clock(last, ok + Duration::hours(1)),
            ClockVerdict::Jump
        );
        assert_eq!(assess_clock(last, t(2036, 10, 1, 12)), ClockVerdict::Jump);
    }
}
