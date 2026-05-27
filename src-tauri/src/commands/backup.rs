//! Backup-Commands (Block 4) — Brücke Frontend ↔ `backup::*`.
//!
//! Die Passphrase wird ausschließlich als Funktionsargument durchgereicht und in
//! der Session ([`backup::BackupSession`]) gehalten — **nie** in DB/Logs/Audit.

use crate::backup::{self, restore, BackupOutcome, BackupSession};
use crate::config::Paths;
use crate::db;
use crate::error::{Error, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

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
#[tauri::command]
pub async fn backup_needs_onboarding(app: AppHandle) -> Result<bool> {
    let paths = Paths::from_handle(&app)?;
    if !paths.db_file.exists() {
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

/// Ist die Session aktuell entsperrt (Passphrase im Memory)?
#[tauri::command]
pub async fn backup_is_unlocked(session: State<'_, BackupSession>) -> Result<bool> {
    Ok(session.is_unlocked())
}

#[tauri::command]
pub async fn backup_get_settings(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, BackupSession>,
) -> Result<BackupSettings> {
    let floor_path = Paths::from_handle(&app)?
        .backups_dir
        .to_string_lossy()
        .to_string();
    // Konfiguriertes Ziel → Directory-Pfad bzw. SFTP-View aufteilen.
    let (target_path, sftp) = match backup::target::get_target(pool.inner()).await? {
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
        passphrase_set: backup::is_passphrase_set(pool.inner()).await?,
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
#[tauri::command]
pub async fn backup_setup_passphrase(
    app: AppHandle,
    session: State<'_, BackupSession>,
    passphrase: String,
) -> Result<BackupOutcome> {
    let paths = Paths::from_handle(&app)?;
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
        let pool = db::open_and_init(&app, None).await?;
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
        db::open_and_init(&app, Some(passphrase.as_str())).await?
    };

    // Verifier + passphrase_set + Session setzen (im jetzt offenen Pool).
    backup::setup_passphrase(&pool, session.inner(), &passphrase).await?;

    // G1-ENC Schritt 3: Eine adoptierte Klartext-Bestands-DB jetzt einmalig
    // verschlüsseln (inkl. Pflicht-Pre-Migration-Backup). Eine frisch
    // verschlüsselt angelegte DB ist hier ein No-op. Erst danach den (nun
    // verschlüsselten) Pool in den State hängen.
    let pool = db::migrate_plaintext_to_encrypted(&app, pool, &passphrase).await?;
    app.manage(pool.clone());

    // Scheduler erst jetzt starten (Pool liegt im State).
    crate::scheduler::tick::ensure_started(&app);

    // Erst-Backup direkt nach Setup (PRD Done-Check).
    backup::create_now(&pool, &paths, &passphrase, "manual").await
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
#[tauri::command]
pub async fn backup_unlock(
    app: AppHandle,
    session: State<'_, BackupSession>,
    passphrase: String,
) -> Result<bool> {
    let paths = Paths::from_handle(&app)?;
    if !paths.db_file.exists() {
        return Err(Error::Backup(
            "Keine Datenbank gefunden — bitte zuerst einrichten.".into(),
        ));
    }

    let pool = if db::db_file_is_plaintext(&paths.db_file) {
        // Klartext-Übergangspfad: Pool ohne Key, Passphrase via Verifier prüfen.
        let pool = db::open_and_init(&app, None).await?;
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
        db::migrate_plaintext_to_encrypted(&app, pool, &passphrase).await?
    } else {
        // Verschlüsselte DB: mit Key öffnen, Probe-Read als Verifier.
        let pool = db::open_pool(&paths.db_file, Some(passphrase.as_str())).await?;
        if !db::probe_readable(&pool).await {
            // Falsche Passphrase: DB nicht lesbar → kein Audit möglich.
            pool.close().await;
            return Ok(false);
        }
        db::run_migrations(&app, &pool).await?;
        pool
    };

    app.manage(pool.clone());
    session.set(passphrase);
    crate::scheduler::tick::ensure_started(&app);
    // R2-026: Pflicht-Audit (Security-Event), Fehler propagieren.
    crate::db::repo::audit_log::append(&pool, "backup.unlock.ok", "backup", "passphrase", None)
        .await?;

    // „Täglich beim App-Start": fälliges Auto-Daily-Backup auslösen.
    let _ = backup::auto_daily_if_due(&pool, &paths, session.inner()).await;
    Ok(true)
}

/// Manuelles Backup (Trigger `manual`). Verlangt entsperrte Session.
#[tauri::command]
pub async fn backup_create_now(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, BackupSession>,
) -> Result<BackupOutcome> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    let passphrase = session
        .get()
        .ok_or_else(|| Error::Backup("Session gesperrt — bitte zuerst entsperren".into()))?;
    backup::create_now(pool, &paths, &passphrase, "manual").await
}

/// Setzt das Backup-Ziel (Verzeichnis).
#[tauri::command]
pub async fn backup_set_target(pool: State<'_, SqlitePool>, path: String) -> Result<()> {
    backup::target::set_directory_target(pool.inner(), &path).await
}

/// Testet eine SFTP-Verbindung (G1-BKP.3): Handshake + Auth + Schreibrechte.
/// Liefert den SHA-256-Host-Key-Fingerprint (zum Bestätigen/Pinnen) und ob im
/// Zielordner geschrieben werden konnte. Schreibt **nichts** in die Konfiguration
/// und lädt **kein** Backup hoch.
///
/// Ist `password` leer, wird ein bereits im Keychain hinterlegtes Passwort
/// genutzt (Test eines gespeicherten Ziels ohne erneute Eingabe).
#[tauri::command]
pub async fn backup_test_sftp(
    host: String,
    port: u16,
    user: String,
    remote_dir: String,
    password: String,
) -> Result<backup::sftp::SftpProbe> {
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

/// Setzt ein SFTP-Backup-Ziel (G1-BKP.3). Speichert Host/Port/User/Ordner +
/// gepinnten Fingerprint als Ziel und legt das Passwort im OS-Keychain ab
/// (**nie** in DB/Log/audit). Ist `password` leer, bleibt ein bereits
/// gespeichertes Passwort erhalten.
///
/// `host_fingerprint` ist Pflicht — er stammt aus [`backup_test_sftp`] und ist
/// der MITM-Schutz: ohne ihn lehnt der spätere Upload ab.
#[tauri::command]
pub async fn backup_set_sftp_target(
    pool: State<'_, SqlitePool>,
    host: String,
    port: u16,
    user: String,
    remote_dir: String,
    host_fingerprint: String,
    password: String,
) -> Result<()> {
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
    backup::target::set_target(pool.inner(), &target).await
}

/// Öffnet einen lokalen Backup-**Ordner** im Datei-Explorer/Finder (Floor oder
/// Off-Site-Verzeichnis). Für SFTP-Ziele nicht verfügbar (Remote-Pfad).
#[tauri::command]
pub async fn backup_open_folder(app: AppHandle, path: String) -> Result<()> {
    let p = path.trim();
    if p.is_empty() || p.starts_with("sftp://") {
        return Err(Error::Backup(
            "Für SFTP-Ziele ist kein lokaler Ordner verfügbar.".into(),
        ));
    }
    app.opener()
        .open_path(p.to_string(), None::<&str>)
        .map_err(|e| Error::Backup(format!("Ordner konnte nicht geöffnet werden: {e}")))?;
    Ok(())
}

/// Zeigt eine lokale Backup-**Datei** im enthaltenden Ordner (markiert sie) — für
/// die Verlaufs-Zeilen. Für SFTP-Backups nicht verfügbar (erst herunterladen).
#[tauri::command]
pub async fn backup_reveal_path(app: AppHandle, path: String) -> Result<()> {
    let p = path.trim();
    if p.is_empty() || p.starts_with("sftp://") {
        return Err(Error::Backup(
            "SFTP-Backups erst herunterladen — kein lokaler Ordner verfügbar.".into(),
        ));
    }
    app.opener()
        .reveal_item_in_dir(p)
        .map_err(|e| Error::Backup(format!("Ordner konnte nicht geöffnet werden: {e}")))?;
    Ok(())
}

/// Backup-Historie (neueste zuerst).
#[tauri::command]
pub async fn backup_list(pool: State<'_, SqlitePool>) -> Result<Vec<BackupHistoryItem>> {
    let rows = sqlx::query_as::<_, BackupHistoryItem>(
        "SELECT id, created_at, target_path, file_size_bytes, retention_tag, trigger_reason,
                db_schema_version, app_version, verified_at
         FROM backup_history
         ORDER BY created_at DESC",
    )
    .fetch_all(pool.inner())
    .await?;
    Ok(rows)
}

/// Backup-Protokoll: die jüngsten Einträge (neueste zuerst). G1-LOG / ADR 0034.
#[tauri::command]
pub async fn backup_log_list(
    pool: State<'_, SqlitePool>,
    limit: Option<i64>,
) -> Result<Vec<crate::db::models::BackupLogRow>> {
    let limit = limit.unwrap_or(200).clamp(1, 5000);
    db::repo::backup_log::list(pool.inner(), limit).await
}

/// Serverseitige Suche/Filterung über das Backup-Protokoll (Volltext +
/// Zeitfenster + Status/Auslöser/Ziel-Typ). G1-LOG / ADR 0034.
#[tauri::command]
pub async fn backup_log_search(
    pool: State<'_, SqlitePool>,
    filter: db::repo::backup_log::BackupLogFilter,
) -> Result<Vec<crate::db::models::BackupLogRow>> {
    db::repo::backup_log::search(pool.inner(), &filter).await
}

/// Restore-Vorschau (Plain-Header, ohne Passphrase).
#[tauri::command]
pub async fn backup_restore_preview(path: String) -> Result<restore::RestorePreview> {
    restore::preview(std::path::Path::new(&path))
}

/// Restore Phase A: Pre-Restore-Backup, entschlüsseln, ins Staging entpacken,
/// Neustart anfordern.
#[tauri::command]
pub async fn backup_restore_apply(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, BackupSession>,
    path: String,
    passphrase: String,
) -> Result<restore::RestoreReport> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    restore::apply(
        pool,
        &paths,
        session.inner(),
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
