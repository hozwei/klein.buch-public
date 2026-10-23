//! Backup-Commands (Block 4) — Brücke Frontend ↔ `backup::*`.
//!
//! Die Passphrase wird ausschließlich als Funktionsargument durchgereicht und in
//! der Session ([`backup::BackupSession`]) gehalten — **nie** in DB/Logs/Audit.

use crate::app_ctx::AppCtx;
use crate::backup::{self, restore, BackupOutcome};
use crate::config::Paths;
use crate::db;
use crate::error::{Error, Result};
use serde::Serialize;
use std::sync::Arc;

/// Status fürs Onboarding/Settings-UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    pub passphrase_set: bool,
    pub unlocked: bool,
    /// Lokaler **Floor**-Ordner (`paths.backups_dir`) — wird IMMER geschrieben
    /// (G1-BKP.4). Das Ziel unten ist die zusätzliche Off-Site-Spiegelung.
    pub floor_path: String,
    /// Pfad, falls das aktuelle (Off-Site-)Ziel ein **Verzeichnis** ist (sonst `None`).
    pub target_path: Option<String>,
    pub default_suggestion: String,
    /// Vom Auto-Detect erkannte Cloud-Ordner (G1-BKP.2) als 1-Klick-Vorschläge.
    pub detected_targets: Vec<backup::target::DetectedTarget>,
    /// Konfiguration, falls das aktuelle Ziel **SFTP** ist (G1-BKP.3) — ohne
    /// Passwort (das lebt im Keychain).
    pub sftp: Option<SftpTargetView>,
}

/// SFTP-Zielkonfiguration fürs UI (kein Passwort).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpTargetView {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub remote_dir: String,
    /// Gepinnter SHA-256-Host-Key-Fingerprint, falls bereits bestätigt.
    pub host_fingerprint: Option<String>,
}

/// Eine Zeile aus `backup_history` für die Liste.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BackupHistoryItem {
    pub id: String,
    pub created_at: String,
    pub target_path: String,
    pub file_size_bytes: i64,
    pub retention_tag: String,
    pub trigger_reason: String,
    pub db_schema_version: i64,
    pub app_version: String,
    pub verified_at: Option<String>,
}

/// Review W2 S0-2/S1-3: Solange ein Restore vorgemerkt ist oder ein Tausch
/// nicht sauber abgeschlossen wurde, weder Onboarding noch Entsperren — sonst
/// entstünde neben `klein-buch.sqlite.rollback` eine neue DB.
pub fn ensure_no_pending_restore(paths: &Paths) -> Result<()> {
    if crate::backup::restore::restore_incomplete(paths) {
        return Err(Error::Backup(
            "Eine Wiederherstellung ist noch nicht abgeschlossen. Bitte die App neu \
             starten; bleibt diese Meldung, ist das Datenverzeichnis zu prüfen \
             (Datei RESTORE_PENDING.json bzw. *.rollback)."
                .into(),
        ));
    }
    Ok(())
}

/// Argumente von [`backup_needs_onboarding`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupNeedsOnboardingArgs {}

/// Onboarding-Gate.
///
/// G1-ENC Schritt 2 (Bootstrap-Inversion): primäres Signal ist die
/// **Datei-Existenz** — der Verifier liegt in der DB, und eine verschlüsselte DB
/// lässt sich vor der Passphrase gar nicht öffnen (chicken-and-egg).
///
/// - **Keine DB** → Onboarding (legt die DB verschlüsselt an).
/// - **Verschlüsselte DB** → kein Onboarding (hat per Konstruktion eine
///   Passphrase) → Entsperren.
/// - **Klartext-DB** (pre-G1-Bestand): Onboarding nur, wenn dort noch **keine**
///   Passphrase gesetzt ist (legitimer Alt-Zustand: DB angelegt, Passphrase nie
///   eingerichtet). Sonst Entsperren. Das verhindert ein Aussperren von
///   Bestands-Installationen, in denen die DB existiert, aber noch kein
///   Verifier geschrieben wurde.
pub async fn backup_needs_onboarding(
    ctx: &Arc<AppCtx>,
    _args: BackupNeedsOnboardingArgs,
) -> Result<bool> {
    let paths = ctx.paths.clone();
    if !paths.db_file.exists() {
        ensure_no_pending_restore(&paths)?;
        return Ok(true);
    }
    if !db::db_file_is_plaintext(&paths.db_file) {
        return Ok(false);
    }
    // Klartext-Übergangs-DB: ohne Key öffnen und prüfen, ob eine Passphrase
    // gesetzt ist. (Kein open_and_init: keine Migrationen für eine reine
    // Status-Abfrage; app_settings existiert auf jeder pre-G1-DB.)
    let pool = db::open_pool(&paths.db_file, None).await?;
    let has = backup::is_passphrase_set(&pool).await.unwrap_or(false);
    pool.close().await;
    Ok(!has)
}

/// Argumente von [`backup_is_unlocked`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupIsUnlockedArgs {}

/// Ist die Session aktuell entsperrt (Passphrase im Memory)?
pub async fn backup_is_unlocked(ctx: &Arc<AppCtx>, _args: BackupIsUnlockedArgs) -> Result<bool> {
    let session = &ctx.session;

    Ok(session.is_unlocked())
}

/// Argumente von [`backup_get_settings`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupGetSettingsArgs {}

pub async fn backup_get_settings(
    ctx: &Arc<AppCtx>,
    _args: BackupGetSettingsArgs,
) -> Result<BackupSettings> {
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let floor_path = ctx.paths.clone().backups_dir.to_string_lossy().to_string();
    // Konfiguriertes Ziel → Directory-Pfad bzw. SFTP-View aufteilen.
    let (target_path, sftp) = match backup::target::get_target(&pool).await? {
        Some(backup::target::BackupTarget::Directory { path }) => (Some(path), None),
        Some(backup::target::BackupTarget::Sftp {
            host,
            port,
            user,
            remote_dir,
            host_fingerprint,
        }) => (
            None,
            Some(SftpTargetView {
                host,
                port,
                user,
                remote_dir,
                host_fingerprint,
            }),
        ),
        None => (None, None),
    };
    Ok(BackupSettings {
        passphrase_set: backup::is_passphrase_set(&pool).await?,
        unlocked: session.is_unlocked(),
        floor_path,
        target_path,
        default_suggestion: backup::target::default_suggestion()
            .to_string_lossy()
            .to_string(),
        detected_targets: backup::target::detect_cloud_targets(),
        sftp,
    })
}

/// Argumente von [`backup_setup_passphrase`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSetupPassphraseArgs {
    pub passphrase: String,
}

/// Onboarding: legt die DB **verschlüsselt** an (SQLCipher, Passphrase = DB-Key,
/// ADR 0035), hängt den Pool in den State, speichert den Verifier, startet den
/// Scheduler und erstellt sofort das erste Backup (PRD Done-Check).
///
/// G1-ENC Schritt 2: Diese Command bekommt **keinen** `SqlitePool`-State mehr —
/// vor dem Onboarding gibt es keinen Pool. Sie öffnet ihn selbst (Passphrase VOR
/// dem DB-Open) und hängt ihn ein.
///
/// G1-ENC Schritt 3: Wird eine bestehende **Klartext**-DB ohne Passphrase
/// adoptiert (legitimer pre-G1-Zustand), so wird sie nach dem Setzen des
/// Verifiers einmalig nach SQLCipher migriert (inkl. Pflicht-Pre-Migration-
/// Backup). Eine frisch verschlüsselt angelegte DB durchläuft das als No-op.
pub async fn backup_setup_passphrase(
    ctx: &Arc<AppCtx>,
    args: BackupSetupPassphraseArgs,
) -> Result<BackupOutcome> {
    let BackupSetupPassphraseArgs { passphrase } = args;
    let session = &ctx.session;

    let paths = ctx.paths.clone();
    ensure_no_pending_restore(&paths)?;
    // R5-005: Passphrase-Floor von 8 auf 16 Zeichen gehoben. Verlust = Total-
    // verlust by design (ADR 0035) — 8-Zeichen-Passwörter sind gegen
    // Argon2id-m=64MB zwar nicht trivial, aber Wörterbuch-/Top-1k-Treffer-
    // fähig. Frontend (`BackupGate.svelte::doSetup`) erzwingt denselben Floor;
    // hier ist die Backend-Backstop für Direkt-Command-Aufrufer.
    if passphrase.chars().count() < 16 {
        return Err(Error::Backup(
            "Passphrase muss mindestens 16 Zeichen haben (Tipp: 3–4 zufällige Wörter)".into(),
        ));
    }

    let pool = if paths.db_file.exists() {
        // Bestehende Klartext-DB ohne Passphrase adoptieren (legitimer pre-G1-
        // Zustand). Sie wird nach dem Setzen des Verifiers unten in Schritt 3
        // verschlüsselt. Eine bereits verschlüsselte oder bereits eingerichtete
        // DB gehört zum Entsperren, nicht zum Onboarding.
        if !db::db_file_is_plaintext(&paths.db_file) {
            return Err(Error::Backup(
                "Datenbank ist bereits verschlüsselt — bitte entsperren statt neu einrichten."
                    .into(),
            ));
        }
        let pool = db::open_and_init(ctx, None).await?;
        if backup::is_passphrase_set(&pool).await? {
            pool.close().await;
            return Err(Error::Backup(
                "Es existiert bereits eine Datenbank — bitte entsperren statt neu einrichten."
                    .into(),
            ));
        }
        pool
    } else {
        // Frische DB verschlüsselt anlegen (Key = Passphrase) + Migrationen.
        db::open_and_init(ctx, Some(passphrase.as_str())).await?
    };

    // Verifier + passphrase_set + Session setzen (im jetzt offenen Pool).
    backup::setup_passphrase(&pool, session, &passphrase).await?;

    // G1-ENC Schritt 3: Eine adoptierte Klartext-Bestands-DB jetzt einmalig
    // verschlüsseln (inkl. Pflicht-Pre-Migration-Backup). Eine frisch
    // verschlüsselt angelegte DB ist hier ein No-op. Erst danach den (nun
    // verschlüsselten) Pool in den State hängen.
    let pool = db::migrate_plaintext_to_encrypted(ctx, pool, &passphrase).await?;
    ctx.set_pool(pool.clone());

    // Scheduler erst jetzt starten (Pool liegt im State).
    crate::scheduler::tick::ensure_started(ctx);

    // Erst-Backup direkt nach Setup (PRD Done-Check).
    backup::create_now(&pool, &paths, &passphrase, "manual").await
}

/// Argumente von [`backup_unlock`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupUnlockArgs {
    pub passphrase: String,
}

/// Entsperrt die App: öffnet den DB-Pool mit der Passphrase und hängt ihn in den
/// State. Liefert `true` bei korrekter Passphrase, `false` bei falscher.
///
/// Zwei Pfade, je nach DB-Format:
/// - **Klartext-Bestands-DB**: ohne Key öffnen, Passphrase gegen den gespeicherten
///   Verifier prüfen. Bei korrekter Passphrase wird die DB **einmalig nach
///   SQLCipher migriert** (G1-ENC Schritt 3, inkl. Pflicht-Pre-Migration-Backup),
///   und der verschlüsselte Pool übernommen.
/// - **Verschlüsselte DB**: mit Key öffnen; ein Probe-Read entscheidet (SQLCipher
///   ist selbst der Verifier). Falscher Key → `false`, ohne den State zu ändern.
///
/// Bei Erfolg: Pool einhängen, Session entsperren, Scheduler starten, ein
/// fälliges Auto-Daily-Backup auslösen.
pub async fn backup_unlock(ctx: &Arc<AppCtx>, args: BackupUnlockArgs) -> Result<bool> {
    let BackupUnlockArgs { passphrase } = args;

    let paths = ctx.paths.clone();
    ensure_no_pending_restore(&paths)?;
    if !paths.db_file.exists() {
        return Err(Error::Backup(
            "Keine Datenbank gefunden — bitte zuerst einrichten.".into(),
        ));
    }

    let pool = if db::db_file_is_plaintext(&paths.db_file) {
        // Klartext-Übergangspfad: Pool ohne Key, Passphrase via Verifier prüfen.
        let pool = db::open_and_init(ctx, None).await?;
        if !backup::verify_passphrase(&pool, &passphrase).await? {
            // R2-026: Pflicht-Audit (Security-Event). Wenn das Schreiben
            // fehlschlägt, lieber zusätzlich loggen und trotzdem die Unlock-
            // Antwort zurückgeben (sonst wird der User bei DB-Lock-Timeout
            // permanent ausgesperrt) — aber den Fehler nicht stillschweigend
            // verschlucken.
            if let Err(e) = crate::db::repo::audit_log::append(
                &pool,
                "backup.unlock.failed",
                "backup",
                "passphrase",
                None,
            )
            .await
            {
                tracing::error!("audit_log.backup.unlock.failed-Schreiben fehlgeschlagen: {e}");
            }
            pool.close().await;
            return Ok(false);
        }
        // G1-ENC Schritt 3: Passphrase ist korrekt → die Klartext-Bestands-DB
        // jetzt einmalig verschlüsseln (inkl. Pflicht-Pre-Migration-Backup) und
        // den verschlüsselten Pool übernehmen. Beim nächsten Start greift der
        // verschlüsselte Zweig unten.
        db::migrate_plaintext_to_encrypted(ctx, pool, &passphrase).await?
    } else {
        let Some(pool) = open_encrypted(&paths, &passphrase).await? else {
            return Ok(false);
        };
        db::run_migrations(ctx, &pool).await?;
        pool
    };
    activate_unlocked(ctx, pool, passphrase).await?;
    Ok(true)
}

/// Öffnet die verschlüsselte DB mit der Passphrase und prüft sie per
/// Probe-Read — ohne Migrationen und ohne den Zustand zu ändern. `None` =
/// falsche Passphrase. Mit falschem Key scheitert meist schon der
/// Verbindungsaufbau (`SQLITE_NOTADB`).
pub async fn open_encrypted(paths: &Paths, passphrase: &str) -> Result<Option<sqlx::SqlitePool>> {
    let pool = match db::open_pool(&paths.db_file, Some(passphrase)).await {
        Ok(pool) => pool,
        Err(e) if db::is_not_a_database(&e) => return Ok(None),
        Err(e) => return Err(e),
    };
    if !db::probe_readable(&pool).await {
        // Falsche Passphrase: DB nicht lesbar → kein Audit möglich.
        pool.close().await;
        return Ok(None);
    }
    Ok(Some(pool))
}

/// Hängt einen geprüften, migrierten Pool ein: Pool, Passphrase der Sitzung,
/// Scheduler, Audit `backup.unlock.ok`.
pub async fn activate_unlocked(
    ctx: &Arc<AppCtx>,
    pool: sqlx::SqlitePool,
    passphrase: String,
) -> Result<()> {
    ctx.set_pool(pool.clone());
    ctx.session.set(passphrase);
    crate::scheduler::tick::ensure_started(ctx);
    // R2-026: Pflicht-Audit (Security-Event), Fehler propagieren.
    crate::db::repo::audit_log::append(&pool, "backup.unlock.ok", "backup", "passphrase", None)
        .await?;

    // „Täglich beim App-Start“: das fällige Tages-Backup legt der erste
    // Scheduler-Tick an (läuft sofort nach `ensure_started`, Plan W2.4). Ein
    // zusätzlicher Aufruf hier erzeugte zwei volle Backups (Review W2 S3).
    Ok(())
}

/// Argumente von [`backup_create_now`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateNowArgs {}

/// Manuelles Backup (Trigger `manual`). Verlangt entsperrte Session.
pub async fn backup_create_now(
    ctx: &Arc<AppCtx>,
    _args: BackupCreateNowArgs,
) -> Result<BackupOutcome> {
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    let passphrase = session
        .get()
        .ok_or_else(|| Error::Backup("Session gesperrt — bitte zuerst entsperren".into()))?;
    backup::create_now(pool, &paths, &passphrase, "manual").await
}

/// Argumente von [`backup_set_target`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSetTargetArgs {
    pub path: String,
}

/// Setzt das Backup-Ziel (Verzeichnis).
pub async fn backup_set_target(ctx: &Arc<AppCtx>, args: BackupSetTargetArgs) -> Result<()> {
    let BackupSetTargetArgs { path } = args;
    let pool = ctx.pool()?;

    backup::target::set_directory_target(&pool, &path).await
}

/// Argumente von [`backup_test_sftp`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTestSftpArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub remote_dir: String,
    pub password: String,
}

/// Testet eine SFTP-Verbindung (G1-BKP.3): Handshake + Auth + Schreibrechte.
/// Liefert den SHA-256-Host-Key-Fingerprint (zum Bestätigen/Pinnen) und ob im
/// Zielordner geschrieben werden konnte. Schreibt **nichts** in die Konfiguration
/// und lädt **kein** Backup hoch.
///
/// Ist `password` leer, wird ein bereits im Keychain hinterlegtes Passwort
/// genutzt (Test eines gespeicherten Ziels ohne erneute Eingabe).
pub async fn backup_test_sftp(
    _ctx: &Arc<AppCtx>,
    args: BackupTestSftpArgs,
) -> Result<backup::sftp::SftpProbe> {
    let BackupTestSftpArgs {
        host,
        port,
        user,
        remote_dir,
        password,
    } = args;

    if host.trim().is_empty() || user.trim().is_empty() {
        return Err(Error::Backup(
            "Host und Benutzer dürfen nicht leer sein.".into(),
        ));
    }
    if port == 0 {
        return Err(Error::Backup("Ungültiger SFTP-Port.".into()));
    }
    let effective = if password.is_empty() {
        backup::sftp::get_password()?.ok_or_else(|| {
            Error::Backup(
                "Kein Passwort eingegeben und keines gespeichert — bitte SFTP-Passwort eingeben."
                    .into(),
            )
        })?
    } else {
        password
    };
    backup::sftp::probe(
        host.trim(),
        port,
        user.trim(),
        &effective,
        remote_dir.trim(),
    )
    .await
}

/// Argumente von [`backup_set_sftp_target`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSetSftpTargetArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub remote_dir: String,
    pub host_fingerprint: String,
    pub password: String,
}

/// Setzt ein SFTP-Backup-Ziel (G1-BKP.3). Speichert Host/Port/User/Ordner +
/// gepinnten Fingerprint als Ziel und legt das Passwort im OS-Keychain ab
/// (**nie** in DB/Log/audit). Ist `password` leer, bleibt ein bereits
/// gespeichertes Passwort erhalten.
///
/// `host_fingerprint` ist Pflicht — er stammt aus [`backup_test_sftp`] und ist
/// der MITM-Schutz: ohne ihn lehnt der spätere Upload ab.
pub async fn backup_set_sftp_target(
    ctx: &Arc<AppCtx>,
    args: BackupSetSftpTargetArgs,
) -> Result<()> {
    let BackupSetSftpTargetArgs {
        host,
        port,
        user,
        remote_dir,
        host_fingerprint,
        password,
    } = args;
    let pool = ctx.pool()?;

    if host.trim().is_empty() || user.trim().is_empty() {
        return Err(Error::Backup(
            "Host und Benutzer dürfen nicht leer sein.".into(),
        ));
    }
    if port == 0 {
        return Err(Error::Backup("Ungültiger SFTP-Port.".into()));
    }
    if host_fingerprint.trim().is_empty() {
        return Err(Error::Backup(
            "Kein Host-Fingerprint — bitte zuerst die Verbindung testen und bestätigen.".into(),
        ));
    }
    // Passwort nur überschreiben, wenn eines eingegeben wurde.
    if !password.is_empty() {
        backup::sftp::set_password(&password)?;
    }
    let target = backup::target::BackupTarget::Sftp {
        host: host.trim().to_string(),
        port,
        user: user.trim().to_string(),
        remote_dir: remote_dir.trim().to_string(),
        host_fingerprint: Some(host_fingerprint.trim().to_string()),
    };
    backup::target::set_target(&pool, &target).await
}

/// Argumente von [`backup_open_folder`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOpenFolderArgs {
    pub path: String,
}

/// Öffnet einen lokalen Backup-**Ordner** im Datei-Explorer/Finder (Floor oder
/// Off-Site-Verzeichnis). Für SFTP-Ziele nicht verfügbar (Remote-Pfad).
pub async fn backup_open_folder(ctx: &Arc<AppCtx>, args: BackupOpenFolderArgs) -> Result<()> {
    let BackupOpenFolderArgs { path } = args;

    let p = path.trim();
    if p.is_empty() || p.starts_with("sftp://") {
        return Err(Error::Backup(
            "Für SFTP-Ziele ist kein lokaler Ordner verfügbar.".into(),
        ));
    }
    ctx.open_path(p)?;
    Ok(())
}

/// Argumente von [`backup_reveal_path`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRevealPathArgs {
    pub path: String,
}

/// Zeigt eine lokale Backup-**Datei** im enthaltenden Ordner (markiert sie) — für
/// die Verlaufs-Zeilen. Für SFTP-Backups nicht verfügbar (erst herunterladen).
pub async fn backup_reveal_path(ctx: &Arc<AppCtx>, args: BackupRevealPathArgs) -> Result<()> {
    let BackupRevealPathArgs { path } = args;

    let p = path.trim();
    if p.is_empty() || p.starts_with("sftp://") {
        return Err(Error::Backup(
            "SFTP-Backups erst herunterladen — kein lokaler Ordner verfügbar.".into(),
        ));
    }
    ctx.reveal(p)?;
    Ok(())
}

/// Argumente von [`backup_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupListArgs {}

/// Backup-Historie (neueste zuerst).
pub async fn backup_list(
    ctx: &Arc<AppCtx>,
    _args: BackupListArgs,
) -> Result<Vec<BackupHistoryItem>> {
    let pool = ctx.pool()?;

    let rows = sqlx::query_as::<_, BackupHistoryItem>(
        "SELECT id, created_at, target_path, file_size_bytes, retention_tag, trigger_reason,
                db_schema_version, app_version, verified_at
         FROM backup_history
         ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await?;
    Ok(rows)
}

/// Argumente von [`backup_log_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupLogListArgs {
    #[serde(default)]
    pub limit: Option<i64>,
}

/// Backup-Protokoll: die jüngsten Einträge (neueste zuerst). G1-LOG / ADR 0034.
pub async fn backup_log_list(
    ctx: &Arc<AppCtx>,
    args: BackupLogListArgs,
) -> Result<Vec<crate::db::models::BackupLogRow>> {
    let BackupLogListArgs { limit } = args;
    let pool = ctx.pool()?;

    let limit = limit.unwrap_or(200).clamp(1, 5000);
    db::repo::backup_log::list(&pool, limit).await
}

/// Argumente von [`backup_log_search`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupLogSearchArgs {
    pub filter: db::repo::backup_log::BackupLogFilter,
}

/// Serverseitige Suche/Filterung über das Backup-Protokoll (Volltext +
/// Zeitfenster + Status/Auslöser/Ziel-Typ). G1-LOG / ADR 0034.
pub async fn backup_log_search(
    ctx: &Arc<AppCtx>,
    args: BackupLogSearchArgs,
) -> Result<Vec<crate::db::models::BackupLogRow>> {
    let BackupLogSearchArgs { filter } = args;
    let pool = ctx.pool()?;

    db::repo::backup_log::search(&pool, &filter).await
}

/// Argumente von [`backup_restore_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestorePreviewArgs {
    pub path: String,
}

/// Restore-Vorschau (Plain-Header, ohne Passphrase).
pub async fn backup_restore_preview(
    _ctx: &Arc<AppCtx>,
    args: BackupRestorePreviewArgs,
) -> Result<restore::RestorePreview> {
    let BackupRestorePreviewArgs { path } = args;

    restore::preview(std::path::Path::new(&path))
}

/// Argumente von [`backup_restore_apply`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreApplyArgs {
    pub path: String,
    pub passphrase: String,
}

/// Restore Phase A: Pre-Restore-Backup, entschlüsseln, ins Staging entpacken,
/// Neustart anfordern.
pub async fn backup_restore_apply(
    ctx: &Arc<AppCtx>,
    args: BackupRestoreApplyArgs,
) -> Result<restore::RestoreReport> {
    let BackupRestoreApplyArgs { path, passphrase } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    restore::apply(
        pool,
        &paths,
        session,
        std::path::Path::new(&path),
        &passphrase,
    )
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
