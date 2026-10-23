//! Jobs für lange, zustandsändernde Abläufe (Plan W5.6): Festschreiben,
//! Storno, Versand, Backup, Restore, Exporte.
//!
//! `POST /api/cmd/{name}` antwortet mit `202 {"jobId": …}`; der Ablauf läuft
//! als eigener Task, unabhängig vom Request. `GET /api/jobs/{id}` liefert
//! `running`, `done` (mit Ergebnis) oder `failed` (mit Fehler wie bei einem
//! direkten Aufruf). Höchstens [`MAX_ACTIVE_JOBS`] laufen oder warten
//! gleichzeitig, sonst 503 + `Retry-After`. Das Herunterfahren wartet auf
//! laufende Jobs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Serialize;
use tokio::sync::Notify;
use tokio::time::Instant;
use uuid::Uuid;

use super::errors::ApiError;
use super::state::ServerState;

pub const MAX_ACTIVE_JOBS: usize = 8;
/// Höchstens so viele beendete Jobs bleiben abrufbar (älteste fallen heraus).
pub const MAX_FINISHED_JOBS: usize = 256;
/// So lange bleibt ein beendeter Job abrufbar.
pub const RETENTION: Duration = Duration::from_secs(3600);

/// Sicht auf einen Job (Antwort von `GET /api/jobs/{id}`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum JobView {
    Running {
        id: String,
        name: String,
    },
    Done {
        id: String,
        name: String,
        result: serde_json::Value,
    },
    Failed {
        id: String,
        name: String,
        status: u16,
        error: serde_json::Value,
    },
}

enum JobState {
    Running,
    Done(serde_json::Value),
    Failed(ApiError),
}

struct JobEntry {
    name: &'static str,
    state: JobState,
    finished_at: Option<Instant>,
}

#[derive(Default)]
pub struct JobRegistry {
    inner: Mutex<HashMap<Uuid, JobEntry>>,
    idle: Notify,
}

impl JobRegistry {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, JobEntry>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reserviert einen Platz; über [`MAX_ACTIVE_JOBS`] → 503.
    pub fn reserve(&self, name: &'static str) -> Result<Uuid, ApiError> {
        let now = Instant::now();
        let mut map = self.lock();
        map.retain(|_, j| {
            j.finished_at
                .is_none_or(|t| now.duration_since(t) < RETENTION)
        });
        let mut finished: Vec<(Uuid, Instant)> = map
            .iter()
            .filter_map(|(id, j)| j.finished_at.map(|t| (*id, t)))
            .collect();
        if finished.len() >= MAX_FINISHED_JOBS {
            finished.sort_by_key(|(_, t)| *t);
            for (old, _) in finished.iter().take(finished.len() + 1 - MAX_FINISHED_JOBS) {
                map.remove(old);
            }
        }
        let active = map
            .values()
            .filter(|j| matches!(j.state, JobState::Running))
            .count();
        if active >= MAX_ACTIVE_JOBS {
            return Err(ApiError::busy(10));
        }
        let id = Uuid::new_v4();
        map.insert(
            id,
            JobEntry {
                name,
                state: JobState::Running,
                finished_at: None,
            },
        );
        Ok(id)
    }

    /// Hält das Ergebnis fest.
    pub fn finish(&self, id: Uuid, outcome: Result<serde_json::Value, ApiError>) {
        let mut map = self.lock();
        if let Some(job) = map.get_mut(&id) {
            job.state = match outcome {
                Ok(v) => JobState::Done(v),
                Err(e) => JobState::Failed(e),
            };
            job.finished_at = Some(Instant::now());
        }
        let idle = !map.values().any(|j| matches!(j.state, JobState::Running));
        drop(map);
        if idle {
            self.idle.notify_waiters();
        }
    }

    pub fn view(&self, id: Uuid) -> Option<JobView> {
        let map = self.lock();
        let job = map.get(&id)?;
        let id = id.to_string();
        let name = job.name.to_string();
        Some(match &job.state {
            JobState::Running => JobView::Running { id, name },
            JobState::Done(v) => JobView::Done {
                id,
                name,
                result: v.clone(),
            },
            JobState::Failed(e) => JobView::Failed {
                id,
                name,
                status: e.status.as_u16(),
                error: e.body(),
            },
        })
    }

    pub fn active(&self) -> usize {
        self.lock()
            .values()
            .filter(|j| matches!(j.state, JobState::Running))
            .count()
    }

    /// Wartet, bis kein Job mehr läuft.
    pub async fn wait_idle(&self) {
        loop {
            let notified = self.idle.notified();
            if self.active() == 0 {
                return;
            }
            notified.await;
        }
    }
}

/// `GET /api/jobs/{id}` (Sitzung erforderlich).
pub async fn get_job(
    State(st): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<JobView>, ApiError> {
    st.authenticate(&headers)
        .ok_or_else(ApiError::unauthenticated)?;
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::not_found())?;
    st.jobs.view(id).map(Json).ok_or_else(ApiError::not_found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queue_limit_and_states() {
        let reg = JobRegistry::default();
        let ids: Vec<_> = (0..MAX_ACTIVE_JOBS)
            .map(|_| reg.reserve("backup_create_now").unwrap())
            .collect();
        let err = reg.reserve("backup_create_now").unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(err.retry_after, Some(10));

        reg.finish(ids[0], Ok(serde_json::json!({"ok": true})));
        assert!(matches!(reg.view(ids[0]), Some(JobView::Done { .. })));
        reg.finish(ids[1], Err(ApiError::busy(1)));
        assert!(matches!(
            reg.view(ids[1]),
            Some(JobView::Failed { status: 503, .. })
        ));
        assert!(reg.reserve("x").is_ok(), "Platz wieder frei");

        let waiter = tokio::spawn({
            let reg = Arc::new(reg);
            let r2 = reg.clone();
            async move {
                let ids: Vec<Uuid> = r2.lock().keys().copied().collect();
                for id in ids {
                    r2.finish(id, Ok(serde_json::Value::Null));
                }
                reg.wait_idle().await;
            }
        });
        tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .unwrap()
            .unwrap();
    }
}
