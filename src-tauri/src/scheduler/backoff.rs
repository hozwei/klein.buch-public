//! Persistierter Backoff nach Fehlschlägen (Lauf F, ADR 0040).
//!
//! Ein Job, der scheitert, soll nicht bei jedem 5-Minuten-Tick erneut laufen
//! (Last, Log- und Hinweisflut), und das Warten muss einen Neustart überstehen.
//! Der Zeitpunkt des letzten Fehlschlags steht deshalb in `app_settings`
//! (UTC, DB-Format) unter einem Schlüssel je Job.

use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;

use crate::clock;
use crate::db::repo::app_settings;
use crate::error::Result;

/// `true`, solange seit dem letzten Fehlschlag weniger als `wait` vergangen
/// ist. Ein Fehlschlag „in der Zukunft“ (Uhr zurückgestellt) blockiert nicht.
pub async fn blocked(
    pool: &SqlitePool,
    key: &str,
    now: DateTime<Utc>,
    wait: Duration,
) -> Result<bool> {
    let Some(last) = app_settings::get(pool, key)
        .await?
        .as_deref()
        .and_then(clock::db_utc)
    else {
        return Ok(false);
    };
    Ok(last <= now && now - last < wait)
}

/// Merkt den Fehlschlag zum Zeitpunkt `now`.
pub async fn record_failure(pool: &SqlitePool, key: &str, now: DateTime<Utc>) -> Result<()> {
    app_settings::set(pool, key, &clock::to_db_utc(now)).await
}

/// Erfolg: Backoff aufheben.
pub async fn clear(pool: &SqlitePool, key: &str) -> Result<()> {
    app_settings::delete(pool, key).await
}
