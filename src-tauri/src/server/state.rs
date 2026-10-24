//! Gemeinsamer Server-Zustand und Zustandsautomat der Instanz (Plan §3, W5.4).
//!
//! `setup` (keine DB) → `locked` (DB vorhanden, kein Pool, kein Scheduler) ↔
//! `unlocked` (Pool offen). Browser-Sitzungen sind davon getrennt (Plan §3):
//! sie entstehen beim Einrichten und Anmelden, enden per Timeout oder Abmelden;
//! „Datenbank sperren“ schließt den Pool und beendet alle Sitzungen.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::headers::Csp;
use super::idempotency::IdempotencyStore;
use super::jobs::JobRegistry;
use super::limits::Admission;
use super::policy::HttpPolicy;
use super::ratelimit::{PublicRateLimiter, RateLimiter};
use super::session::{SessionId, SessionStore};
use super::totp::TotpStore;
use crate::app_ctx::AppCtx;

/// Zustand der Instanz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    Setup,
    Locked,
    Unlocked,
}

impl InstanceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Locked => "locked",
            Self::Unlocked => "unlocked",
        }
    }
}

/// Wie lange der Server nach einem erfolgreichen Restore noch antwortet, damit
/// der Browser das Job-Ergebnis abholen kann, bevor er neu startet.
pub const RESTART_GRACE: Duration = Duration::from_secs(10);
/// Höchstens so lange wartet das Herunterfahren auf laufende Commands und Jobs
/// (W5-STOP-GRACE, W14-025b). Zusammen mit dem Abwarten offener Verbindungen
/// (`http::CONNECTION_DRAIN_TIMEOUT`, 30 s) und dem Schließen des Pools
/// ([`POOL_CLOSE_WAIT`]) bleibt das Herunterfahren unter 70 s — deutlich unter
/// `stop_grace_period: 90s` im Compose-Beispiel. Ein abgebrochenes Backup ist
/// unkritisch (Temp-Datei + rename, Reste räumt der Sweeper).
pub const JOB_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
/// So lange wartet das Herunterfahren auf den laufenden Scheduler-Tick
/// (gleichzeitig mit [`JOB_DRAIN_TIMEOUT`]); die Abbruch-Anforderung steht
/// schon seit dem Signal.
pub const SCHEDULER_SHUTDOWN_WAIT: Duration = Duration::from_secs(25);
/// Höchstens so lange wartet das Herunterfahren auf das Schließen des Pools
/// (WAL-Checkpoint, ausgeliehene Verbindungen).
pub const POOL_CLOSE_WAIT: Duration = Duration::from_secs(5);
/// So lange warten „Datenbank sperren“ und „Passphrase ändern“ auf laufende
/// Aktionen, bevor sie mit 503 abbrechen.
pub const DB_GATE_WAIT: Duration = Duration::from_secs(60);
/// So lange warten „Datenbank sperren“ und „Passphrase ändern“ darauf, dass ein
/// laufender Scheduler-Tick nach dem aktuellen Termin aufhört (W14-027, ADR
/// 0040). Danach sperrt „Datenbank sperren“ trotzdem (der Tick läuft ohne Pool
/// und Passphrase leer); „Passphrase ändern“ bricht mit 503 ab.
pub const SCHEDULER_STOP_WAIT: Duration = Duration::from_secs(10);

/// Setup-Token und ausstehende TOTP-Geheimnisse (Plan W6).
#[derive(Default)]
pub struct SetupState {
    /// 128-bit-Token aus dem Container-Log (pro Start neu, nach Erfolg ungültig).
    token: Option<String>,
    /// Geheimnis der laufenden Einrichtung (an den Token gebunden).
    pending_setup: Option<Vec<u8>>,
    /// Geheimnis einer laufenden TOTP-Neueinrichtung (an die Sitzung gebunden).
    pending_renew: Option<(SessionId, Vec<u8>)>,
}
pub struct ServerState {
    pub ctx: Arc<AppCtx>,
    pub sessions: SessionStore,
    pub jobs: JobRegistry,
    pub idem: IdempotencyStore,
    pub admission: Admission,
    pub csp: Csp,
    /// Serialisiert Einrichten und Anmelden (Zustandswechsel).
    pub transition: tokio::sync::Mutex<()>,
    /// Löst das geordnete Herunterfahren aus (Signal oder Restore).
    pub shutdown: CancellationToken,
    /// Abgekoppelte Tasks (Commands, Jobs, Anmeldung); das Herunterfahren
    /// wartet auf sie.
    pub tasks: TaskTracker,
    restart_pending: AtomicBool,
    /// Ein Restore-Job läuft: keine Änderungen, Scheduler pausiert.
    restore_running: AtomicBool,
    /// Exklusive Sperre auf `/data/.klein-buch.lock` (ein Prozess je Instanz).
    _instance_lock: Option<std::fs::File>,
    /// HTTP-Policy (öffentliche URL, vertraute Proxys, unsicheres HTTP).
    pub policy: HttpPolicy,
    /// Zweiter Faktor (`/data/auth/totp.json`).
    pub totp: TotpStore,
    /// Rate-Limit für Anmeldung, Einrichtung, Re-Auth.
    pub ratelimit: RateLimiter,
    /// Anfragen je Client auf öffentlichen Lese-Routen (`/api/public/info`,
    /// `/legal/`, Plan W12).
    pub public_rate: PublicRateLimiter,
    setup: Mutex<SetupState>,
    /// Lesend gehalten von jedem laufenden Command/Job; schreibend von
    /// „Datenbank sperren“ und „Passphrase ändern“ (warten auf laufende Arbeit).
    pub db_gate: tokio::sync::RwLock<()>,
    /// Verschiebung der TOTP-Uhr in Sekunden (nur Tests).
    totp_clock_offset: AtomicI64,
}

impl ServerState {
    pub fn new(
        ctx: Arc<AppCtx>,
        csp: Csp,
        instance_lock: Option<std::fs::File>,
        policy: HttpPolicy,
        ratelimit: RateLimiter,
    ) -> Arc<Self> {
        let totp = TotpStore::new(&ctx.paths.data_dir);
        Arc::new(Self {
            ctx,
            sessions: SessionStore::default(),
            jobs: JobRegistry::default(),
            idem: IdempotencyStore::default(),
            admission: Admission::default(),
            csp,
            transition: tokio::sync::Mutex::new(()),
            shutdown: CancellationToken::new(),
            tasks: TaskTracker::new(),
            restart_pending: AtomicBool::new(false),
            restore_running: AtomicBool::new(false),
            _instance_lock: instance_lock,
            policy,
            totp,
            ratelimit,
            public_rate: PublicRateLimiter::default(),
            setup: Mutex::new(SetupState::default()),
            db_gate: tokio::sync::RwLock::new(()),
            totp_clock_offset: AtomicI64::new(0),
        })
    }

    fn setup_state(&self) -> std::sync::MutexGuard<'_, SetupState> {
        self.setup.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Braucht die Instanz einen Setup-Token? (keine DB oder kein TOTP)
    pub fn needs_setup_token(&self) -> bool {
        self.instance_state() == InstanceState::Setup || !self.totp.exists()
    }

    /// Erzeugt (falls nötig) einen Setup-Token und gibt ihn als eigene Zeile
    /// auf stderr aus — nur dort, nie über `tracing` (Plan W6, Log-Test).
    pub fn ensure_setup_token(&self) {
        let mut s = self.setup_state();
        if s.token.is_some() {
            return;
        }
        let mut raw = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
        let token = data_encoding::BASE32_NOPAD.encode(&raw);
        eprintln!(
            "\n=== Klein.Buch: Einrichtung nötig ===\nSetup-Token: {token}\n             (gilt bis zum nächsten Neustart; im Browser zusammen mit Passphrase und TOTP eingeben)\n"
        );
        tracing::warn!(
            "Einrichtung nötig — Setup-Token steht als eigene Zeile im Container-Log (stderr)"
        );
        s.token = Some(token);
        s.pending_setup = None;
    }

    /// Prüft den Setup-Token konstantzeitig.
    pub fn setup_token_matches(&self, candidate: &str) -> bool {
        let s = self.setup_state();
        let Some(token) = s.token.as_deref() else {
            return false;
        };
        let candidate = candidate.trim().to_ascii_uppercase();
        token.len() == candidate.len() && bool::from(token.as_bytes().ct_eq(candidate.as_bytes()))
    }

    /// Aktueller Setup-Token (Tests lesen ihn statt des Logs).
    #[doc(hidden)]
    pub fn setup_token_for_tests(&self) -> Option<String> {
        self.setup_state().token.clone()
    }

    /// Ausstehendes Geheimnis der Einrichtung setzen bzw. lesen.
    pub fn set_pending_setup(&self, secret: Vec<u8>) {
        self.setup_state().pending_setup = Some(secret);
    }

    pub fn pending_setup(&self) -> Option<Vec<u8>> {
        self.setup_state().pending_setup.clone()
    }

    /// Einrichtung abgeschlossen: Token und ausstehendes Geheimnis verwerfen.
    pub fn finish_setup(&self) {
        let mut s = self.setup_state();
        s.token = None;
        s.pending_setup = None;
    }

    pub fn set_pending_renew(&self, session: SessionId, secret: Vec<u8>) {
        self.setup_state().pending_renew = Some((session, secret));
    }

    /// Ausstehendes Geheimnis einer TOTP-Neueinrichtung dieser Sitzung.
    pub fn take_pending_renew(&self, session: SessionId) -> Option<Vec<u8>> {
        let mut s = self.setup_state();
        match &s.pending_renew {
            Some((id, _)) if *id == session => s.pending_renew.take().map(|(_, secret)| secret),
            _ => None,
        }
    }

    /// Unix-Zeit für TOTP (in Tests verschiebbar).
    pub fn totp_now(&self) -> u64 {
        let base = super::totp::unix_now() as i64;
        (base + self.totp_clock_offset.load(Ordering::SeqCst)).max(0) as u64
    }

    #[doc(hidden)]
    pub fn shift_totp_clock_for_tests(&self, secs: i64) {
        self.totp_clock_offset.fetch_add(secs, Ordering::SeqCst);
    }

    /// Gültige Sitzung zur Anfrage. Fehlt `totp.json` (nach `totp-reset`),
    /// sind alle Sitzungen ungültig (Plan W6).
    pub fn authenticate(&self, headers: &HeaderMap) -> Option<SessionId> {
        if !self.totp.exists() {
            if !self.sessions.is_empty() {
                tracing::warn!("totp.json fehlt — alle Sitzungen beendet");
                self.sessions.clear();
            }
            return None;
        }
        self.sessions.from_headers(headers)
    }

    pub fn instance_state(&self) -> InstanceState {
        if self.ctx.is_unlocked() {
            InstanceState::Unlocked
        } else if self.ctx.paths.db_file.exists() {
            InstanceState::Locked
        } else {
            InstanceState::Setup
        }
    }

    /// Neustart vorgemerkt (nach Restore) oder Herunterfahren läuft.
    pub fn is_stopping(&self) -> bool {
        self.restart_pending.load(Ordering::SeqCst) || self.shutdown.is_cancelled()
    }

    /// Keine Änderungen annehmen: Restore läuft, Neustart vorgemerkt oder
    /// Herunterfahren.
    pub fn writes_blocked(&self) -> bool {
        self.restore_running.load(Ordering::SeqCst) || self.is_stopping()
    }

    /// Restore beginnt: ab sofort keine Änderungen mehr annehmen (sie gingen
    /// beim Tausch verloren). Der Job pausiert danach den Scheduler.
    /// `false`, wenn schon ein Restore läuft (zweite Anfrage → 503,
    /// Gegenprüfung W7).
    pub fn begin_restore(&self) -> bool {
        self.restore_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Restore nicht angenommen (Job-Platz fehlte): nur die Sperre lösen.
    pub fn cancel_restore(&self) {
        self.restore_running.store(false, Ordering::SeqCst);
    }

    /// Restore gescheitert: Sperre aufheben, Scheduler fortsetzen.
    pub async fn abort_restore(&self) {
        crate::scheduler::tick::resume(&self.ctx).await;
        self.restore_running.store(false, Ordering::SeqCst);
    }

    /// Nach einem erfolgreichen Restore: neue Änderungen ablehnen, kurz
    /// warten (Ergebnis abholbar), dann geordnet herunterfahren. Der Neustart
    /// durch die Container-Verwaltung wendet den Restore vor dem Port-Bind an.
    pub fn request_restart(self: &Arc<Self>) {
        if self.restart_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::info!("Restore vorgemerkt — Server fährt für den Neustart herunter");
        let st = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(RESTART_GRACE).await;
            st.shutdown.cancel();
        });
    }

    /// Nach dem Ende der Annahme-Schleife (W5-STOP-GRACE, W14-025b, ADR 0040):
    /// zuerst den Scheduler anhalten (Abbruch-Anforderung — laufende Abo-Läufe
    /// beenden ihren Termin, der Vollscan die Datei), dann gleichzeitig den
    /// Tick ([`SCHEDULER_SHUTDOWN_WAIT`]) und abgekoppelte Tasks und Jobs
    /// ([`JOB_DRAIN_TIMEOUT`]) abwarten, zuletzt den Pool schließen
    /// (WAL-Checkpoint, [`POOL_CLOSE_WAIT`]). Alle Fristen zusammen liegen unter
    /// der `stop_grace_period` des Compose-Beispiels.
    pub async fn finish_shutdown(&self) {
        crate::scheduler::tick::request_stop(&self.ctx);
        self.tasks.close();
        let (scheduler_stopped, tasks_done) = tokio::join!(
            crate::scheduler::tick::stop_within(&self.ctx, SCHEDULER_SHUTDOWN_WAIT),
            tokio::time::timeout(JOB_DRAIN_TIMEOUT, self.tasks.wait()),
        );
        if !scheduler_stopped {
            tracing::warn!(
                "Herunterfahren: Scheduler-Tick nach {} s noch aktiv — wird abgebrochen",
                SCHEDULER_SHUTDOWN_WAIT.as_secs()
            );
        }
        if tasks_done.is_err() {
            tracing::warn!(
                "Herunterfahren: {} Task(s), davon {} Job(s), nach {} s noch aktiv — werden abgebrochen",
                self.tasks.len(),
                self.jobs.active(),
                JOB_DRAIN_TIMEOUT.as_secs()
            );
        }
        if let Some(pool) = self.ctx.take_pool() {
            if tokio::time::timeout(POOL_CLOSE_WAIT, pool.close())
                .await
                .is_err()
            {
                tracing::warn!("Herunterfahren: Pool schließt verzögert");
            }
        }
        self.ctx.session.clear();
        self.sessions.clear();
    }
}
