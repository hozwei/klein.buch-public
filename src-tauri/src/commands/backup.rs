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
    /// (G1-BKP.4). Die Ziele unten sind die zusätzlichen Off-Site-Kopien.
    pub floor_path: String,
    /// Wurzel der Verzeichnis-Ziele (`/backup-targets`, Plan W7).
    pub targets_root: String,
    /// Unterordner der Wurzel zur Auswahl (eingebundene Volumes).
    pub available_dirs: Vec<String>,
    /// Konfigurierte Off-Site-Ziele (ohne Passwörter).
    pub targets: Vec<TargetView>,
    /// `missing` | `failing` | `ok` (Plan W7).
    pub offsite_status: String,
}

/// Off-Site-Ziel fürs UI (kein Passwort).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetView {
    pub id: String,
    pub label: String,
    /// `directory` | `sftp`
    pub kind: String,
    /// Verzeichnis-Ziel: Ordner relativ zur Wurzel.
    pub directory: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub remote_dir: Option<String>,
    /// Gepinnter SHA-256-Host-Key-Fingerprint.
    pub host_fingerprint: Option<String>,
    /// Ergebnis des letzten Laufs.
    pub last_result: Option<backup::target::TargetResult>,
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
/// - **DB vorhanden** → kein Onboarding (sie ist per Konstruktion
///   verschlüsselt; der Klartext-Übergangspfad ist mit W6 entfallen, eine
///   Klartext-DB verweigert schon den Serverstart).
pub async fn backup_needs_onboarding(
    ctx: &Arc<AppCtx>,
    _args: BackupNeedsOnboardingArgs,
) -> Result<bool> {
    let paths = ctx.paths.clone();
    if !paths.db_file.exists() {
        ensure_no_pending_restore(&paths)?;
        return Ok(true);
    }
    Ok(false)
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

    let paths = &ctx.paths;
    let root = &paths.backup_targets_dir;
    let results = backup::target::load_results(&pool).await?;
    let targets = backup::target::list_targets(&pool, root)
        .await?
        .into_iter()
        .map(|t| {
            let last_result = results.get(&t.id).cloned();
            match t.target {
                backup::target::BackupTarget::Directory { path } => TargetView {
                    id: t.id,
                    label: t.label,
                    kind: "directory".into(),
                    directory: Some(path),
                    host: None,
                    port: None,
                    user: None,
                    remote_dir: None,
                    host_fingerprint: None,
                    last_result,
                },
                backup::target::BackupTarget::Sftp {
                    host,
                    port,
                    user,
                    remote_dir,
                    host_fingerprint,
                } => TargetView {
                    id: t.id,
                    label: t.label,
                    kind: "sftp".into(),
                    directory: None,
                    host: Some(host),
                    port: Some(port),
                    user: Some(user),
                    remote_dir: Some(remote_dir),
                    host_fingerprint,
                    last_result,
                },
            }
        })
        .collect();
    Ok(BackupSettings {
        passphrase_set: backup::is_passphrase_set(&pool).await?,
        unlocked: session.is_unlocked(),
        floor_path: paths.backups_dir.to_string_lossy().to_string(),
        targets_root: root.to_string_lossy().to_string(),
        available_dirs: backup::target::available_dirs(root),
        targets,
        offsite_status: backup::target::offsite_status(&pool, root)
            .await?
            .to_string(),
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
/// Seit W6 nur noch für eine frische Instanz (keine DB); das Adoptieren einer
/// Klartext-Bestands-DB ist entfallen (E17).
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

    if paths.db_file.exists() {
        return Err(Error::Backup(
            "Es existiert bereits eine Datenbank — bitte entsperren statt neu einrichten.".into(),
        ));
    }
    // Frische DB verschlüsselt anlegen (Key = Passphrase) + Migrationen.
    let pool = db::open_and_init(ctx, Some(passphrase.as_str())).await?;

    // Verifier + passphrase_set + Session setzen (im jetzt offenen Pool).
    backup::setup_passphrase(&pool, session, &passphrase).await?;

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
/// Die DB wird mit Key geöffnet; ein Probe-Read entscheidet (SQLCipher ist
/// selbst der Verifier). Falscher Key → `false`, ohne den State zu ändern.
/// Eine Klartext-DB wird seit W6 nicht mehr geöffnet (E17).
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

    if db::db_file_is_plaintext(&paths.db_file) {
        // Der Klartext-Übergangspfad ist mit W6 entfallen (ADR 0039, E17).
        return Err(Error::Backup(
            "Unverschlüsselte Datenbank — wird nicht geöffnet (ADR 0035/0039).".into(),
        ));
    }
    let Some(pool) = open_encrypted(&paths, &passphrase).await? else {
        return Ok(false);
    };
    migrate_with_backup(ctx, &pool, &passphrase).await?;
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

/// Fehler des Pflicht-Backups vor einer Migration (Plan W7): keine Migration,
/// die Datenbank bleibt gesperrt.
pub const PRE_MIGRATION_FAILED: &str = "Pflicht-Backup vor der Datenbank-Migration fehlgeschlagen";

/// Migrationen mit Pflicht-Backup (ADR 0039 E15, Plan W7): Ist die
/// Schema-Version der geöffneten DB älter als `EXPECTED`, entsteht vorher ein
/// Backup `manual` mit der alten Version, das vollständig gelesen und geprüft
/// wird. Scheitert es, wird nicht migriert (Fehler, Instanz bleibt gesperrt;
/// Log + Inbox-Hinweis). Vor der Migration Audit `backup.pre_migration`.
/// Frische oder aktuelle Datenbanken migrieren ohne Backup.
pub async fn migrate_with_backup(
    ctx: &Arc<AppCtx>,
    pool: &sqlx::SqlitePool,
    passphrase: &str,
) -> Result<()> {
    use crate::db::schema_version::EXPECTED_SCHEMA_VERSION;
    let Some(from) = db::schema_version::read(pool)
        .await?
        .filter(|v| *v < EXPECTED_SCHEMA_VERSION)
    else {
        return db::run_migrations(ctx, pool).await;
    };
    let backup = async {
        let outcome = backup::create_now(pool, &ctx.paths, passphrase, "manual").await?;
        backup::verify_backup_file(std::path::Path::new(&outcome.file_path), passphrase).await?;
        Ok::<_, Error>(outcome)
    }
    .await;
    let outcome = match backup {
        Ok(outcome) => outcome,
        Err(e) => {
            tracing::error!(
                "{PRE_MIGRATION_FAILED} (Schema {from} → {EXPECTED_SCHEMA_VERSION}): {e} — \
                 keine Migration, die Datenbank bleibt gesperrt"
            );
            let body = format!(
                "Vor der Umstellung der Datenbank von Version {from} auf \
                 {EXPECTED_SCHEMA_VERSION} ließ sich kein Backup schreiben. Es wurde nichts \
                 umgestellt. Prüfe Speicherplatz und Schreibrechte von /data (Server-Log)."
            );
            crate::notify::store::create(
                pool,
                crate::notify::NewNotification {
                    title: "Datenbank-Migration abgebrochen",
                    body: &body,
                    severity: "urgent",
                    action_url: Some("/settings/backup-log"),
                    dedup_key: Some(&format!("pre_migration_failed:{from}")),
                    ..Default::default()
                },
            )
            .await
            .ok();
            return Err(Error::Backup(format!("{PRE_MIGRATION_FAILED}: {e}")));
        }
    };
    // Audit noch in der alten DB, vor der Migration (übersteht sie; auch ein
    // Abbruch mittendrin ist dann dokumentiert, Gegenprüfung W7).
    crate::db::repo::audit_log::append(
        pool,
        "backup.pre_migration",
        "backup",
        &outcome.history_id,
        Some(&format!(
            r#"{{"from":{from},"to":{EXPECTED_SCHEMA_VERSION},"backup_id":"{}"}}"#,
            outcome.history_id
        )),
    )
    .await?;
    db::run_migrations(ctx, pool).await?;
    tracing::info!(
        "Datenbank von Schema {from} auf {EXPECTED_SCHEMA_VERSION} migriert (Backup {})",
        outcome.file_name
    );
    Ok(())
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
    // Nach „Datenbank sperren“ (W6) ist der Scheduler pausiert.
    crate::scheduler::tick::resume(ctx).await;
    crate::scheduler::tick::ensure_started(ctx);
    // R2-026: Pflicht-Audit (Security-Event), Fehler propagieren.
    crate::db::repo::audit_log::append(&pool, "backup.unlock.ok", "backup", "passphrase", None)
        .await?;
    // Hinweise des Seeds von /inputs (Plan W11) in die Inbox.
    if let Err(e) = crate::seed::deliver_notices(&pool, &ctx.paths.inputs_dir).await {
        tracing::warn!("Seed-Hinweise nicht übernommen: {e}");
    }

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
#[allow(dead_code)] // nicht mehr freigegeben, entfällt in W11
pub struct BackupSetTargetArgs {
    pub path: String,
}

/// Ersetzt durch [`backup_target_add_directory`] (Plan W7, nicht mehr
/// freigegeben; entfällt in W11).
pub async fn backup_set_target(_ctx: &Arc<AppCtx>, args: BackupSetTargetArgs) -> Result<()> {
    let BackupSetTargetArgs { path: _ } = args;
    Err(Error::Domain(
        "Ersetzt durch „Backup-Ziel hinzufügen“ (mehrere Ziele, Plan W7).".into(),
    ))
}

/// Argumente von [`backup_target_add_directory`].
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTargetAddDirectoryArgs {
    pub label: String,
    /// Ordner relativ zur Wurzel `/backup-targets` (z. B. `nas/klein-buch`).
    pub directory: String,
}

/// Neues Verzeichnis-Ziel unter `/backup-targets` (Plan W7, Re-Auth). Der
/// Ordner wird sofort aufgelöst und geprüft (angelegt, falls er fehlt).
pub async fn backup_target_add_directory(
    ctx: &Arc<AppCtx>,
    args: BackupTargetAddDirectoryArgs,
) -> Result<backup::target::TargetEntry> {
    let BackupTargetAddDirectoryArgs { label, directory } = args;
    let pool = ctx.pool()?;
    let root = &ctx.paths.backup_targets_dir;
    // Normalisiert speichern (`nas/./kb` → `nas/kb`), Dubletten am
    // aufgelösten Pfad erkennen und einmal schreiben/löschen (Gegenprüfung W7).
    if directory.trim().starts_with('/') {
        backup::target::check_relative(directory.trim())?;
    }
    let directory = directory
        .trim()
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/");
    let resolved = backup::target::resolve_directory(root, &directory)?;
    for existing in backup::target::list_targets(&pool, root).await? {
        if let backup::target::BackupTarget::Directory { path } = &existing.target {
            if backup::target::resolve_directory(root, path).ok().as_ref() == Some(&resolved) {
                return Err(Error::Domain(format!(
                    "Dieser Ordner ist schon als Ziel „{}“ eingerichtet.",
                    existing.label
                )));
            }
        }
    }
    let probe = resolved.join(".klein-buch-write-test");
    std::fs::write(&probe, b"ok")
        .and_then(|_| std::fs::remove_file(&probe))
        .map_err(|e| Error::Domain(format!("Im Ordner kann nicht geschrieben werden: {e}")))?;
    let entry = backup::target::TargetEntry {
        id: uuid::Uuid::now_v7().to_string(),
        label: clean_label(&label, &directory),
        target: backup::target::BackupTarget::Directory { path: directory },
    };
    add_target(&pool, root, entry).await
}

fn clean_label(label: &str, fallback: &str) -> String {
    let l: String = label
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    if l.is_empty() {
        fallback.to_string()
    } else {
        l
    }
}

async fn add_target(
    pool: &sqlx::SqlitePool,
    root: &std::path::Path,
    entry: backup::target::TargetEntry,
) -> Result<backup::target::TargetEntry> {
    let mut targets = backup::target::list_targets(pool, root).await?;
    if targets.iter().any(|t| t.target == entry.target) {
        return Err(Error::Domain(
            "Dieses Backup-Ziel ist schon eingerichtet.".into(),
        ));
    }
    targets.push(entry.clone());
    backup::target::save_targets(pool, &targets).await?;
    Ok(entry)
}

/// Argumente von [`backup_target_remove`].
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTargetRemoveArgs {
    pub id: String,
}

/// Entfernt ein Off-Site-Ziel (Plan W7, Re-Auth); vorhandene Dateien dort
/// bleiben liegen, das SFTP-Passwort wird gelöscht.
pub async fn backup_target_remove(ctx: &Arc<AppCtx>, args: BackupTargetRemoveArgs) -> Result<()> {
    let BackupTargetRemoveArgs { id } = args;
    let pool = ctx.pool()?;
    let root = &ctx.paths.backup_targets_dir;
    let mut targets = backup::target::list_targets(&pool, root).await?;
    let before = targets.len();
    let removed: Vec<_> = targets.iter().filter(|t| t.id == id).cloned().collect();
    targets.retain(|t| t.id != id);
    if targets.len() == before {
        return Err(Error::Domain(format!("Backup-Ziel nicht gefunden: {id}")));
    }
    backup::target::save_targets(&pool, &targets).await?;
    backup::target::store_results(&pool, &targets, &[]).await?;
    if removed
        .iter()
        .any(|t| matches!(t.target, backup::target::BackupTarget::Sftp { .. }))
    {
        crate::secrets::delete(&pool, &crate::secrets::sftp_key(&id)).await?;
    }
    Ok(())
}

/// Argumente von [`backup_test_sftp`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTestSftpArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub remote_dir: String,
    /// Pflicht: nie ein gespeichertes Passwort an einen frei gewählten Host
    /// (Plan W8; gespeicherte Ziele testet `backup_test_sftp_saved`).
    pub password: String,
}

/// Testet eine neue SFTP-Verbindung (G1-BKP.3): Handshake + Auth +
/// Schreibrechte. Liefert den SHA-256-Host-Key-Fingerprint zum Bestätigen und
/// merkt ihn 10 min für (Host, Port) — nur dieser lässt sich speichern
/// (Plan W8). Port-Allowlist und Kontingent wie alle Verbindungstests.
pub async fn backup_test_sftp(
    ctx: &Arc<AppCtx>,
    args: BackupTestSftpArgs,
) -> Result<backup::sftp::SftpProbe> {
    let BackupTestSftpArgs {
        host,
        port,
        user,
        remote_dir,
        password,
    } = args;
    let host = host.trim().to_string();
    if host.is_empty() || user.trim().is_empty() {
        return Err(Error::Backup(
            "Host und Benutzer dürfen nicht leer sein.".into(),
        ));
    }
    if password.is_empty() {
        return Err(Error::Backup("Bitte das SFTP-Passwort eingeben.".into()));
    }
    ctx.net_tests
        .check_port(crate::nettest::TestKind::Sftp, port)?;
    ctx.net_tests.admit()?;
    let probe = backup::sftp::probe(&host, port, user.trim(), &password, remote_dir.trim())
        .await
        .map_err(|e| crate::nettest::connect_failed(&host, port, &e))?;
    ctx.net_tests
        .remember_probe(&host, port, &probe.fingerprint);
    Ok(probe)
}

/// Argumente von [`backup_test_sftp_saved`].
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTestSftpSavedArgs {
    pub id: String,
}

/// Testet ein gespeichertes SFTP-Ziel mit seinem gespeicherten Passwort und
/// prüft den gepinnten Host-Fingerprint (Plan W8, nur per ID).
pub async fn backup_test_sftp_saved(
    ctx: &Arc<AppCtx>,
    args: BackupTestSftpSavedArgs,
) -> Result<backup::sftp::SftpProbe> {
    let BackupTestSftpSavedArgs { id } = args;
    let pool = ctx.pool()?;
    let entry = backup::target::list_targets(&pool, &ctx.paths.backup_targets_dir)
        .await?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| Error::Domain(format!("Backup-Ziel nicht gefunden: {id}")))?;
    let backup::target::BackupTarget::Sftp {
        host,
        port,
        user,
        remote_dir,
        host_fingerprint,
    } = entry.target
    else {
        return Err(Error::Domain("Kein SFTP-Ziel.".into()));
    };
    let password = crate::secrets::get(&pool, &crate::secrets::sftp_key(&id))
        .await?
        .ok_or_else(|| {
            Error::Backup("Kein SFTP-Passwort gespeichert — Ziel neu anlegen.".into())
        })?;
    ctx.net_tests
        .check_port(crate::nettest::TestKind::Sftp, port)?;
    ctx.net_tests.admit()?;
    let probe = backup::sftp::probe(&host, port, &user, &password, &remote_dir)
        .await
        .map_err(|e| crate::nettest::connect_failed(&host, port, &e))?;
    if host_fingerprint.as_deref() != Some(probe.fingerprint.as_str()) {
        return Err(Error::Backup(format!(
            "SFTP-Host-Schlüssel weicht vom gespeicherten Fingerprint ab (erhalten {}) — \
             möglicher Man-in-the-Middle. Ziel prüfen und neu anlegen.",
            probe.fingerprint
        )));
    }
    Ok(probe)
}

/// Argumente von [`backup_set_sftp_target`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // nicht mehr freigegeben, entfällt in W11
pub struct BackupSetSftpTargetArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub remote_dir: String,
    pub host_fingerprint: String,
    pub password: String,
}

/// Setzt ein SFTP-Backup-Ziel (G1-BKP.3). Speichert Host/Port/User/Ordner +
/// gepinnten Fingerprint als Ziel (Desktop-Fassung). Im Server ersetzt durch
/// [`backup_target_add_sftp`].
///
/// `host_fingerprint` ist Pflicht — er stammt aus [`backup_test_sftp`] und ist
/// der MITM-Schutz: ohne ihn lehnt der spätere Upload ab.
pub async fn backup_set_sftp_target(
    _ctx: &Arc<AppCtx>,
    _args: BackupSetSftpTargetArgs,
) -> Result<()> {
    Err(Error::Domain(
        "Ersetzt durch „Backup-Ziel hinzufügen“ (mehrere Ziele, Plan W7).".into(),
    ))
}

/// Argumente von [`backup_target_add_sftp`].
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTargetAddSftpArgs {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub remote_dir: String,
    pub host_fingerprint: String,
    pub password: String,
}

/// Neues SFTP-Ziel (Plan W7, Re-Auth). `host_fingerprint` ist Pflicht (aus
/// [`backup_test_sftp`], MITM-Schutz); das Passwort liegt je Ziel in der
/// verschlüsselten DB (`secrets`), **nie** im Ziel-JSON, Log oder Audit.
pub async fn backup_target_add_sftp(
    ctx: &Arc<AppCtx>,
    args: BackupTargetAddSftpArgs,
) -> Result<backup::target::TargetEntry> {
    let BackupTargetAddSftpArgs {
        label,
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
    if password.is_empty() {
        return Err(Error::Backup("Bitte das SFTP-Passwort eingeben.".into()));
    }
    ctx.net_tests
        .check_port(crate::nettest::TestKind::Sftp, port)?;
    // Plan W8: nur der Fingerprint, den die letzte Probe für (Host, Port)
    // geliefert hat (kein frei eingegebener Pin).
    if ctx
        .net_tests
        .probed_fingerprint(host.trim(), port)
        .as_deref()
        != Some(host_fingerprint.trim())
    {
        return Err(Error::Backup(
            "Der Fingerprint stammt nicht aus dem letzten Verbindungstest für diesen Server — \
             bitte die Verbindung erneut testen."
                .into(),
        ));
    }
    let id = uuid::Uuid::now_v7().to_string();
    let entry = backup::target::TargetEntry {
        id: id.clone(),
        label: clean_label(&label, host.trim()),
        target: backup::target::BackupTarget::Sftp {
            host: host.trim().to_string(),
            port,
            user: user.trim().to_string(),
            remote_dir: remote_dir.trim().to_string(),
            host_fingerprint: Some(host_fingerprint.trim().to_string()),
        },
    };
    crate::secrets::set(&pool, &crate::secrets::sftp_key(&id), &password).await?;
    match add_target(&pool, &ctx.paths.backup_targets_dir, entry).await {
        Ok(entry) => Ok(entry),
        Err(e) => {
            crate::secrets::delete(&pool, &crate::secrets::sftp_key(&id))
                .await
                .ok();
            Err(e)
        }
    }
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
    /// `.kbk` aus `POST /api/upload/restore` (Plan W7) …
    #[serde(default)]
    pub upload_id: Option<String>,
    /// … oder ein lokales Backup aus der Historie (`backup_history.id`).
    #[serde(default)]
    pub backup_id: Option<String>,
    /// Passphrase des Backups (danach die Passphrase der Instanz).
    pub passphrase: String,
}

/// Restore Phase A (Plan W7): Quelle ist ein Upload oder ein lokales Backup
/// aus der Historie (nur Dateien im lokalen Backup-Ordner); danach Neustart.
/// Der Upload wird nach Erfolg und nach jedem Fehler außer einer falschen
/// Passphrase verworfen.
pub async fn backup_restore_apply(
    ctx: &Arc<AppCtx>,
    args: BackupRestoreApplyArgs,
) -> Result<restore::RestoreReport> {
    let BackupRestoreApplyArgs {
        upload_id,
        backup_id,
        passphrase,
    } = args;
    let pool = ctx.pool()?;
    let upload_id = upload_id.filter(|s| !s.is_empty());
    let backup_id = backup_id.filter(|s| !s.is_empty());
    let (path, label) = match (&upload_id, &backup_id) {
        (Some(id), None) => {
            let (path, _size, name) = ctx
                .uploads
                .path_of(id, &[crate::uploads::UploadKind::Restore])?;
            (path, format!("Upload {name}"))
        }
        (None, Some(id)) => history_backup_path(&pool, &ctx.paths, id).await?,
        _ => {
            return Err(Error::BadRequest(
                "Genau eine Quelle angeben: uploadId oder backupId.".into(),
            ))
        }
    };
    let limits = ctx.uploads.limits();
    let result = restore::apply(
        &pool,
        &ctx.paths,
        &ctx.session,
        &path,
        &label,
        &passphrase,
        restore::RestoreLimits {
            max_bytes: limits.max_restore_bytes,
            min_free_bytes: limits.min_free_bytes,
        },
    )
    .await;
    if let Some(id) = upload_id.as_deref() {
        let keep = matches!(&result, Err(e) if restore::is_wrong_passphrase(e));
        if !keep {
            ctx.uploads.consume(id);
        }
    }
    result
}

/// Pfad eines lokalen Backups aus der Historie — nur unter `backups_dir`
/// (aufgelöst per `canonicalize`, keine Symlinks nach draußen).
async fn history_backup_path(
    pool: &sqlx::SqlitePool,
    paths: &Paths,
    id: &str,
) -> Result<(std::path::PathBuf, String)> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT target_path, created_at FROM backup_history WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let (target, created_at) =
        row.ok_or_else(|| Error::Domain(format!("Backup nicht gefunden: {id}")))?;
    let not_local = || {
        Error::Domain(
            "Nur lokale Backups aus dem Backup-Ordner lassen sich direkt einspielen — \
             andere bitte herunterladen und hochladen."
                .into(),
        )
    };
    let root = paths.backups_dir.canonicalize().map_err(|_| not_local())?;
    let real = std::path::Path::new(&target)
        .canonicalize()
        .map_err(|_| not_local())?;
    if !real.starts_with(&root) || !real.is_file() {
        return Err(not_local());
    }
    Ok((real, format!("Backup vom {created_at}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
