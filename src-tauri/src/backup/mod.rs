//! Backup-Schicht (Block 4).
//!
//! - `snapshot`: konsistenter SQLite-Snapshot (WAL-Checkpoint + DB-Datei lesen) + Archive-Tree zippen.
//! - `encrypt`: AES-256-GCM mit Argon2id-derived Key aus Passphrase.
//! - `restore`: pre-restore-Backup → verify → apply.
//! - `rotation`: daily/monthly/yearly Retention (GVS).
//! - `target`: konfigurierbares Backup-Ziel (lokale FS, OneDrive, SFTP, etc.).
//! - `sftp`: SFTP-Upload-Arm (russh) mit Host-Key-Pinning + Keychain-Passwort.
//! - `manifest`: JSON-Header mit Versionsdaten + Hash.
//!
//! Dieser `mod.rs` enthält zusätzlich:
//! - [`BackupSession`] — die **Session-Passphrase im Prozess-Memory**
//!   (nie in DB/Logs/audit_log; siehe Backup-Hardline).
//! - Generische `app_settings`-Helfer.
//! - Passphrase-Setup/Unlock via verschlüsseltem Verifier (kein Klartext, kein
//!   Hash in der DB nötig).
//! - [`create_now`] — die Backup-Erstellungs-Pipeline.
//! - [`auto_backup_if_unlocked`] — best-effort Auto-Hook für `lock`-Events.

pub mod encrypt;
pub mod factory_reset;
pub mod manifest;
pub mod restore;
pub mod rotation;
pub mod sftp;
pub mod snapshot;
pub mod target;

use crate::config::Paths;
use crate::db::schema_version::EXPECTED_SCHEMA_VERSION;
use crate::error::{Error, Result};
use chrono::{Local, Utc};
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// app_settings-Keys (Block 4)
// ---------------------------------------------------------------------------

pub const SETTING_PASSPHRASE_SET: &str = "backup_passphrase_set";
pub const SETTING_VERIFIER_SALT: &str = "backup_verifier_salt_hex";
pub const SETTING_VERIFIER_NONCE: &str = "backup_verifier_nonce_hex";
pub const SETTING_VERIFIER_CT: &str = "backup_verifier_ct_hex";

// ---------------------------------------------------------------------------
// Session-Passphrase (nur im Memory)
// ---------------------------------------------------------------------------

/// Hält die Backup-Passphrase **ausschließlich im Prozess-Memory** der
/// laufenden Session. Wird in den Tauri-State gehängt. Niemals serialisiert,
/// niemals in DB/Logs/audit_log.
///
/// Hinweis (Hardening-Backlog): die Passphrase liegt als `String` im Heap.
/// Eine `zeroize`-Wischung beim Drop wäre die nächste Stufe; bewusst nicht in
/// Block 4, um die Dependency-Fläche unverändert zu lassen.
#[derive(Default)]
pub struct BackupSession {
    inner: Mutex<Option<String>>,
}

impl BackupSession {
    pub fn set(&self, passphrase: String) {
        *self.inner.lock().expect("BackupSession mutex poisoned") = Some(passphrase);
    }

    pub fn clear(&self) {
        *self.inner.lock().expect("BackupSession mutex poisoned") = None;
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner
            .lock()
            .expect("BackupSession mutex poisoned")
            .is_some()
    }

    /// Liefert eine Kopie der Passphrase, falls die Session entsperrt ist.
    pub fn get(&self) -> Option<String> {
        self.inner
            .lock()
            .expect("BackupSession mutex poisoned")
            .clone()
    }
}

// ---------------------------------------------------------------------------
// Generische app_settings-Helfer
// ---------------------------------------------------------------------------

pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("value")))
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at)
         VALUES (?, ?, datetime('now','utc'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Wurde die Backup-Passphrase je eingerichtet?
pub async fn is_passphrase_set(pool: &SqlitePool) -> Result<bool> {
    Ok(get_setting(pool, SETTING_PASSPHRASE_SET).await?.as_deref() == Some("true"))
}

/// Onboarding nötig, wenn noch keine Passphrase eingerichtet wurde. Die
/// Onboarding-UI blockiert dann das Erstellen der ersten Rechnung.
pub async fn needs_onboarding(pool: &SqlitePool) -> Result<bool> {
    Ok(!is_passphrase_set(pool).await?)
}

// ---------------------------------------------------------------------------
// Passphrase-Setup + Unlock (Verifier statt Klartext/Hash)
// ---------------------------------------------------------------------------

/// Richtet die Backup-Passphrase erstmalig ein. Speichert einen
/// **verschlüsselten Verifier** (Salt+Nonce+Ciphertext einer Magic-Konstante)
/// in `app_settings` — nicht die Passphrase und keinen Passwort-Hash. Setzt die
/// Session-Passphrase. Idempotenz: erneuter Aufruf bei bereits gesetzter
/// Passphrase ist ein Fehler.
pub async fn setup_passphrase(
    pool: &SqlitePool,
    session: &BackupSession,
    passphrase: &str,
) -> Result<()> {
    // R5-005: Passphrase-Floor von 8 auf 16 (ADR 0035). Beide Verteidigungs-
    // linien (command-Layer und lib-Layer) müssen denselben Floor haben —
    // sonst kann ein Direkt-Aufrufer der lib-API den Frontend-Floor umgehen.
    if passphrase.chars().count() < 16 {
        return Err(Error::Backup(
            "Passphrase muss mindestens 16 Zeichen haben (Tipp: 3–4 zufällige Wörter)".into(),
        ));
    }
    if is_passphrase_set(pool).await? {
        return Err(Error::Backup(
            "Backup-Passphrase ist bereits eingerichtet (nutze Unlock)".into(),
        ));
    }

    let salt = encrypt::random_bytes(encrypt::SALT_LEN);
    let nonce = encrypt::random_bytes(encrypt::NONCE_LEN);
    let key = encrypt::derive_key(passphrase.as_bytes(), &salt)?;
    let nonce_arr = encrypt::nonce_array(&nonce)?;
    let ct = encrypt::encrypt(&key, &nonce_arr, encrypt::VERIFIER_PLAINTEXT)?;

    set_setting(pool, SETTING_VERIFIER_SALT, &manifest::to_hex(&salt)).await?;
    set_setting(pool, SETTING_VERIFIER_NONCE, &manifest::to_hex(&nonce)).await?;
    set_setting(pool, SETTING_VERIFIER_CT, &manifest::to_hex(&ct)).await?;
    set_setting(pool, SETTING_PASSPHRASE_SET, "true").await?;

    session.set(passphrase.to_string());

    crate::db::repo::audit_log::append(
        pool,
        "backup.passphrase.setup",
        "backup",
        "passphrase",
        Some(r#"{"verifier":"stored"}"#),
    )
    .await?;
    Ok(())
}

/// Prüft die Passphrase gegen den gespeicherten Verifier. Bei Erfolg wird die
/// Session entsperrt. Liefert `Ok(true)` bei korrekter, `Ok(false)` bei falscher
/// Passphrase. Schreibt die Passphrase niemals ins Audit-Log.
pub async fn unlock(pool: &SqlitePool, session: &BackupSession, passphrase: &str) -> Result<bool> {
    if verify_passphrase(pool, passphrase).await? {
        session.set(passphrase.to_string());
        crate::db::repo::audit_log::append(pool, "backup.unlock.ok", "backup", "passphrase", None)
            .await
            .ok();
        Ok(true)
    } else {
        crate::db::repo::audit_log::append(
            pool,
            "backup.unlock.failed",
            "backup",
            "passphrase",
            None,
        )
        .await
        .ok();
        Ok(false)
    }
}

/// Pure-ish Verifikation gegen den gespeicherten Verifier (ohne Session-Effekt).
pub async fn verify_passphrase(pool: &SqlitePool, passphrase: &str) -> Result<bool> {
    let salt_hex = get_setting(pool, SETTING_VERIFIER_SALT)
        .await?
        .ok_or_else(|| Error::Backup("keine Passphrase eingerichtet".into()))?;
    let nonce_hex = get_setting(pool, SETTING_VERIFIER_NONCE)
        .await?
        .ok_or_else(|| Error::Backup("Verifier unvollständig".into()))?;
    let ct_hex = get_setting(pool, SETTING_VERIFIER_CT)
        .await?
        .ok_or_else(|| Error::Backup("Verifier unvollständig".into()))?;

    let salt = manifest::from_hex(&salt_hex)?;
    let nonce = encrypt::nonce_array(&manifest::from_hex(&nonce_hex)?)?;
    let ct = manifest::from_hex(&ct_hex)?;

    let key = encrypt::derive_key(passphrase.as_bytes(), &salt)?;
    match encrypt::decrypt(&key, &nonce, &ct) {
        Ok(pt) => Ok(pt == encrypt::VERIFIER_PLAINTEXT),
        Err(_) => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// Backup-Erstellung
// ---------------------------------------------------------------------------

/// Ergebnis eines erstellten Backups. `file_path` ist immer der **lokale Floor**
/// (primäre Kopie). Die Off-Site-Spiegelung (G1-BKP.4) ist best-effort:
/// `mirror_target` = Off-Site-Pfad/URI bei Erfolg, `mirror_error` = Fehlertext bei
/// Fehlschlag (Floor ist dann trotzdem gesichert).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOutcome {
    pub history_id: String,
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub retention_tag: String,
    pub trigger_reason: String,
    pub created_at: String,
    /// Off-Site-Pfad/URI, falls die Spiegelung erfolgreich war (sonst `None`).
    pub mirror_target: Option<String>,
    /// Fehlertext, falls die Off-Site-Spiegelung fehlschlug (sonst `None`).
    pub mirror_error: Option<String>,
}

/// Fügt eine `backup_history`-Zeile ein (Floor oder Off-Site — unterschieden
/// allein am `target_path`; siehe [`rotation`]). `retention_tag` ist für beide
/// Tiers gleich (datumsbasiert); die Tier-Zuordnung passiert erst beim Pruning.
#[allow(clippy::too_many_arguments)]
async fn insert_history_row(
    pool: &SqlitePool,
    id: &str,
    target_path: &str,
    file_hash: &str,
    size_bytes: i64,
    retention_tag: &str,
    trigger_reason: &str,
    app_version: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO backup_history
            (id, target_path, file_hash_sha256, file_size_bytes, is_encrypted,
             retention_tag, trigger_reason, db_schema_version, app_version)
         VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(target_path)
    .bind(file_hash)
    .bind(size_bytes)
    .bind(retention_tag)
    .bind(trigger_reason)
    .bind(EXPECTED_SCHEMA_VERSION)
    .bind(app_version)
    .execute(pool)
    .await?;
    Ok(())
}

/// Pro-Lauf-Backup-Ergebnis-Hinweis (G1-NOTIFY), gated über die abschaltbare
/// Regel `rule_backup_result` (Migration `0027`). Schreibt in die In-App-Inbox
/// (Quelle der Wahrheit, ADR 0027); **kein** OS-Push — `create_now` hat keinen
/// `AppHandle`, die OS-native Eskalation übernimmt die periodische
/// `backup_overdue`-Regel. Best-effort: ein Hinweis-Fehler darf ein Backup nie
/// beeinflussen. Enthält **nie** ein Geheimnis (nur Status/Dateiname).
async fn notify_backup_result(
    pool: &SqlitePool,
    severity: &str,
    title: &str,
    body: &str,
    dedup_key: &str,
) {
    let enabled = crate::notify::rules::get(pool, "rule_backup_result")
        .await
        .ok()
        .flatten()
        .map(|r| r.is_enabled())
        .unwrap_or(false);
    if !enabled {
        return;
    }
    // Direkt in die In-App-Inbox schreiben (NICHT über `notify::emit`): `emit`
    // berührt `os_native::push`, dessen WinRT-Pfad in Integrationstest-Binaries
    // (Lib ohne `cfg(test)`) comctl32/`TaskDialogIndirect` zieht und sie beim
    // Laden scheitern lässt (STATUS_ENTRYPOINT_NOT_FOUND, vgl. ADR 0027 Pt. 5).
    // Der Pro-Lauf-Hinweis ist ohnehin Inbox-only (kein OS-Push) → `store::create`
    // genügt und hält den Backup-Pfad frei vom OS-Notification-Code.
    crate::notify::store::create(
        pool,
        crate::notify::NewNotification {
            rule_id: Some("rule_backup_result"),
            title,
            body,
            severity,
            action_url: Some("/settings/backup-log"),
            dedup_key: Some(dedup_key),
            ..Default::default()
        },
    )
    .await
    .ok();
}

/// Erstellt **jetzt** ein verschlüsseltes Backup: Snapshot → Argon2id-Key →
/// AES-256-GCM → Manifest-Frame → **Floor schreiben** (immer lokal) →
/// `backup_history` → Audit → **Off-Site-Spiegelung** (best-effort, G1-BKP.4) →
/// Rotation. Surfaced Floor-Fehler an den Aufrufer; ein fehlgeschlagener
/// Off-Site-Mirror ist **nicht** fatal (der Floor ist die Sicherheit).
pub async fn create_now(
    pool: &SqlitePool,
    paths: &Paths,
    passphrase: &str,
    trigger_reason: &str,
) -> Result<BackupOutcome> {
    // 1. Konsistenter Content-Snapshot.
    let content = snapshot::build_content_zip(pool, paths).await?;

    // 2. Schlüssel ableiten + verschlüsseln (frische Salt+Nonce pro Backup).
    let salt = encrypt::random_bytes(encrypt::SALT_LEN);
    let nonce = encrypt::random_bytes(encrypt::NONCE_LEN);
    let key = encrypt::derive_key(passphrase.as_bytes(), &salt)?;
    let nonce_arr = encrypt::nonce_array(&nonce)?;
    let ciphertext = encrypt::encrypt(&key, &nonce_arr, &content.zip_bytes)?;

    // 3. Manifest + Datei-Frame.
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let man = manifest::Manifest {
        magic: manifest::MAGIC.to_string(),
        format_version: manifest::FORMAT_VERSION,
        schema_version: EXPECTED_SCHEMA_VERSION,
        app_version: app_version.clone(),
        created_at: created_at.clone(),
        kdf: manifest::KdfParams {
            algo: "argon2id".to_string(),
            m_cost_kib: encrypt::KDF_M_COST_KIB,
            t_cost: encrypt::KDF_T_COST,
            p_cost: encrypt::KDF_P_COST,
        },
        salt_hex: manifest::to_hex(&salt),
        nonce_hex: manifest::to_hex(&nonce),
        content_sha256: content.sha256_hex.clone(),
        content_size_bytes: content.zip_bytes.len() as u64,
    };
    let file_bytes = manifest::frame(&man, &ciphertext)?;
    let file_hash = snapshot::sha256_hex(&file_bytes);
    let size_bytes = file_bytes.len() as i64;
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let file_name = format!("klein-buch-{stamp}.kbk");
    let retention_tag =
        rotation::classify_retention(Local::now().date_naive(), trigger_reason).to_string();

    // 4. Floor schreiben — IMMER lokal (`paths.backups_dir`). Das ist die
    //    primäre, nicht-abschaltbare Sicherheits-Kopie (G1-BKP.4 Tier-Modell).
    let floor_target = target::BackupTarget::Directory {
        path: paths.backups_dir.to_string_lossy().to_string(),
    };
    let floor_path = match target::write_backup(&floor_target, &file_name, &file_bytes).await {
        Ok(p) => p,
        Err(e) => {
            // Floor-Fehlschlag protokollieren (best-effort), dann hart melden:
            // ohne Floor gibt es keine Sicherheits-Kopie (G1-LOG: jeder Lauf wird
            // protokolliert, Erfolg UND Fehlschlag).
            crate::db::repo::backup_log::insert(
                pool,
                &crate::db::repo::backup_log::BackupLogEntry {
                    trigger: trigger_reason.to_string(),
                    target_kind: "local".to_string(),
                    target_label: None,
                    file_name: file_name.clone(),
                    full_path: paths
                        .backups_dir
                        .join(&file_name)
                        .to_string_lossy()
                        .to_string(),
                    size_bytes,
                    status: "failed".to_string(),
                    detail: Some(e.to_string()),
                },
            )
            .await
            .ok();
            notify_backup_result(
                pool,
                "warning",
                "Backup fehlgeschlagen",
                "Die lokale Pflicht-Sicherung konnte nicht geschrieben werden. \
                 Prüfe Speicherplatz und Schreibrechte. Details im Backup-Protokoll.",
                &format!(
                    "backup_result_failed_floor:{}",
                    Local::now().format("%Y-%m-%d")
                ),
            )
            .await;
            return Err(e);
        }
    };

    let history_id = uuid::Uuid::now_v7().to_string();
    insert_history_row(
        pool,
        &history_id,
        &floor_path,
        &file_hash,
        size_bytes,
        &retention_tag,
        trigger_reason,
        &app_version,
    )
    .await?;
    crate::db::repo::audit_log::append(
        pool,
        "backup.created",
        "backup",
        &history_id,
        Some(&format!(
            r#"{{"trigger":"{trigger_reason}","tag":"{retention_tag}","size":{size_bytes}}}"#
        )),
    )
    .await
    .ok();
    // Floor-Erfolg ins Backup-Protokoll (G1-LOG, append-only; nie fatal).
    crate::db::repo::backup_log::insert(
        pool,
        &crate::db::repo::backup_log::BackupLogEntry {
            trigger: trigger_reason.to_string(),
            target_kind: "local".to_string(),
            target_label: None,
            file_name: file_name.clone(),
            full_path: floor_path.clone(),
            size_bytes,
            status: "ok".to_string(),
            detail: None,
        },
    )
    .await
    .ok();

    // 5. Off-Site-Spiegelung (best-effort) — falls ein vom Floor verschiedenes
    //    Ziel konfiguriert ist. Fehlschlag ist NICHT fatal: der Floor ist
    //    gesichert, und ein nicht erreichbares Off-Site-Ziel darf einen
    //    GoBD-festgeschriebenen Vorgang nie zurückrollen. Pre-Restore-Backups
    //    werden nicht gespiegelt (lokaler Sicherheits-Snapshot vor Restore).
    let mut mirror_target = None;
    let mut mirror_error = None;
    if trigger_reason != "pre_restore" {
        if let Some(offsite) = target::offsite_target(pool, &paths.backups_dir).await? {
            match target::write_backup(&offsite, &file_name, &file_bytes).await {
                Ok(offsite_path) => {
                    let mirror_id = uuid::Uuid::now_v7().to_string();
                    insert_history_row(
                        pool,
                        &mirror_id,
                        &offsite_path,
                        &file_hash,
                        size_bytes,
                        &retention_tag,
                        trigger_reason,
                        &app_version,
                    )
                    .await
                    .ok();
                    crate::db::repo::audit_log::append(
                        pool,
                        "backup.mirror.ok",
                        "backup",
                        &mirror_id,
                        Some(&format!(
                            r#"{{"target":"{}"}}"#,
                            offsite_path.replace('\\', "\\\\").replace('"', "'")
                        )),
                    )
                    .await
                    .ok();
                    // Off-Site-Erfolg ins Backup-Protokoll (G1-LOG).
                    let (kind, label) = target::log_meta(&offsite);
                    crate::db::repo::backup_log::insert(
                        pool,
                        &crate::db::repo::backup_log::BackupLogEntry {
                            trigger: trigger_reason.to_string(),
                            target_kind: kind.to_string(),
                            target_label: label,
                            file_name: file_name.clone(),
                            full_path: offsite_path.clone(),
                            size_bytes,
                            status: "ok".to_string(),
                            detail: None,
                        },
                    )
                    .await
                    .ok();
                    mirror_target = Some(offsite_path);
                }
                Err(e) => {
                    tracing::error!("Off-Site-Spiegelung fehlgeschlagen ({trigger_reason}): {e}");
                    crate::db::repo::audit_log::append(
                        pool,
                        "backup.mirror.failed",
                        "backup",
                        &history_id,
                        Some(&format!(
                            r#"{{"error":"{}"}}"#,
                            e.to_string().replace('"', "'")
                        )),
                    )
                    .await
                    .ok();
                    // Off-Site-Fehlschlag ins Backup-Protokoll (G1-LOG) — der für
                    // den Nutzer wichtigste Fall (Ziel offline). Nie fatal.
                    let (kind, label) = target::log_meta(&offsite);
                    crate::db::repo::backup_log::insert(
                        pool,
                        &crate::db::repo::backup_log::BackupLogEntry {
                            trigger: trigger_reason.to_string(),
                            target_kind: kind.to_string(),
                            target_label: label,
                            file_name: file_name.clone(),
                            full_path: target::log_failed_path(&offsite, &file_name),
                            size_bytes,
                            status: "failed".to_string(),
                            detail: Some(e.to_string()),
                        },
                    )
                    .await
                    .ok();
                    notify_backup_result(
                        pool,
                        "warning",
                        "Externe Backup-Spiegelung fehlgeschlagen",
                        &format!(
                            "Die Sicherung auf das externe Ziel schlug fehl: {e}. \
                             Die lokale Pflichtkopie ist gesichert. Details im Backup-Protokoll."
                        ),
                        &format!(
                            "backup_result_failed_offsite:{}",
                            Local::now().format("%Y-%m-%d")
                        ),
                    )
                    .await;
                    mirror_error = Some(e.to_string());
                }
            }
        }
    }

    // 6. Rotation (getiert) — außer direkt vor einem Restore (Pre-Restore nie prunen).
    if trigger_reason != "pre_restore" {
        let _ = rotation::run(pool, &paths.backups_dir).await;
    }

    // 7. Pro-Lauf-Erfolgs-Hinweis NUR bei manuellem Backup (G1-NOTIFY). Auto-Lock-/
    //    Tages-Backups bleiben bei Erfolg lautlos, sonst Hinweis-Spam bei jedem
    //    festgeschriebenen Beleg. Fehlschläge wurden oben (Floor/Off-Site) gemeldet.
    if trigger_reason == "manual" {
        notify_backup_result(
            pool,
            "info",
            "Backup erstellt",
            &format!("Die manuelle Sicherung wurde erfolgreich erstellt: {file_name}."),
            &format!("backup_result_ok:{file_name}"),
        )
        .await;
    }

    Ok(BackupOutcome {
        history_id,
        file_path: floor_path,
        file_name,
        size_bytes,
        retention_tag,
        trigger_reason: trigger_reason.to_string(),
        created_at,
        mirror_target,
        mirror_error,
    })
}

/// Bildet ein auslösendes Event auf eine für `backups.trigger_reason` **gültige**
/// Kategorie ab. Der CHECK erlaubt nur auto_daily/auto_critical/manual/pre_restore;
/// bereits gültige Reasons bleiben, alle Lock-Event-Labels werden `auto_critical`.
fn db_trigger_reason(event: &str) -> &str {
    match event {
        "auto_daily" | "auto_critical" | "manual" | "pre_restore" => event,
        _ => "auto_critical",
    }
}

/// Best-effort Auto-Backup für `lock`-Events (Invoice/Storno/Expense/…).
///
/// - Session nicht entsperrt → kein Backup, nur Audit-Marker, `Ok(None)`.
/// - Session entsperrt → `create_now`. Schlägt das Backup fehl (z. B. Ziel
///   offline), wird das **nicht** zum harten Fehler des auslösenden Vorgangs
///   (eine GoBD-festgeschriebene Rechnung darf nicht zurückrollen, weil ein
///   Backup-Ziel nicht erreichbar war) — der Fehler wird geloggt + auditiert
///   und `Ok(None)` zurückgegeben.
pub async fn auto_backup_if_unlocked(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    event: &str,
) -> Result<Option<BackupOutcome>> {
    let Some(passphrase) = session.get() else {
        crate::db::repo::audit_log::append(
            pool,
            "backup.skipped_locked",
            "backup",
            "auto",
            Some(&format!(r#"{{"event":"{event}"}}"#)),
        )
        .await
        .ok();
        tracing::warn!("Auto-Backup übersprungen: Session nicht entsperrt ({event})");
        return Ok(None);
    };

    // Lock-Event-Label → gültige `backups.trigger_reason`-Kategorie. Das konkrete
    // Event bleibt im Audit-Log (`event`) nachvollziehbar.
    match create_now(pool, paths, &passphrase, db_trigger_reason(event)).await {
        Ok(outcome) => Ok(Some(outcome)),
        Err(e) => {
            tracing::error!("Auto-Backup fehlgeschlagen ({event}): {e}");
            crate::db::repo::audit_log::append(
                pool,
                "backup.failed",
                "backup",
                "auto",
                Some(&format!(
                    r#"{{"event":"{event}","error":"{}"}}"#,
                    e.to_string().replace('"', "'")
                )),
            )
            .await
            .ok();
            Ok(None)
        }
    }
}

/// Auto-Daily beim App-Start, wenn das letzte erfolgreiche Backup älter als
/// 24 h ist (und die Session entsperrt ist).
pub async fn auto_daily_if_due(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
) -> Result<Option<BackupOutcome>> {
    if !session.is_unlocked() {
        return Ok(None);
    }
    let last: Option<String> =
        sqlx::query("SELECT MAX(created_at) AS last FROM backup_history WHERE is_encrypted = 1")
            .fetch_one(pool)
            .await?
            .try_get::<Option<String>, _>("last")
            .unwrap_or(None);

    let due = match last {
        None => true,
        Some(ts) => {
            // created_at ist 'YYYY-MM-DD HH:MM:SS' (UTC). Vergleich gegen jetzt-24h.
            let threshold = (Utc::now() - chrono::Duration::hours(24))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            ts < threshold
        }
    };

    if due {
        auto_backup_if_unlocked(pool, paths, session, "auto_daily").await
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn db_trigger_reason_maps_events_to_valid_check_values() {
        // Gültige Reasons bleiben unverändert.
        assert_eq!(db_trigger_reason("auto_daily"), "auto_daily");
        assert_eq!(db_trigger_reason("auto_critical"), "auto_critical");
        assert_eq!(db_trigger_reason("manual"), "manual");
        assert_eq!(db_trigger_reason("pre_restore"), "pre_restore");
        // Lock-Event-Labels → auto_critical (sonst CHECK-Constraint-Verletzung).
        assert_eq!(db_trigger_reason("invoice.lock"), "auto_critical");
        assert_eq!(db_trigger_reason("storno.create"), "auto_critical");
        assert_eq!(db_trigger_reason("expense.lock"), "auto_critical");
        assert_eq!(db_trigger_reason("recurring.lock"), "auto_critical");
    }

    /// G1-HARDEN.2: **alle** im Code real ausgelösten Lock-Events ergeben einen
    /// gültigen `backups.trigger_reason` UND einen gültigen `backup_history
    /// .retention_tag` — sonst CHECK-Constraint-Verletzung beim Auto-Backup.
    /// Die Liste spiegelt jeden `auto_backup_if_unlocked(..)`-Aufrufer wider
    /// (Stand G1-HARDEN). Der Catch-all in `db_trigger_reason` macht das
    /// bruchsicher; dieser Test friert das Versprechen ein.
    #[test]
    fn all_real_lock_events_map_to_valid_check_sets() {
        // CHECK-Mengen aus Migration 0001 (backup_history).
        const VALID_TRIGGER: &[&str] = &["auto_daily", "auto_critical", "manual", "pre_restore"];
        const VALID_RETENTION: &[&str] = &["daily", "monthly", "yearly", "manual"];

        // Jedes Event-Label, das im Code an `auto_backup_if_unlocked`/`create_now`
        // übergeben wird (grep `auto_backup_if_unlocked`, Stand 2026-05-25).
        let events = [
            "auto_daily",
            "manual",
            "pre_restore",
            "invoice.lock",
            "storno.create",
            "expense.lock",
            "expense.cancel",
            "recurring.lock",
            "recurring_invoice.lock",
            "private_movement.lock",
            "quote.issue",
            "asset.create",
            "asset.update",
            "asset.dispose",
            "depreciation.lock",
            "depreciation.reset",
            "fiscal_year.close",
            // Auch ein unbekanntes/neues Label darf nie den CHECK brechen.
            "some.future.lock",
        ];

        // Verschiedene Datümer abdecken (01.01. → yearly, 1. → monthly, sonst daily).
        let dates = [
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
        ];

        for ev in events {
            let reason = db_trigger_reason(ev);
            assert!(
                VALID_TRIGGER.contains(&reason),
                "Event {ev:?} ergibt ungültigen trigger_reason {reason:?}"
            );
            for d in dates {
                let tag = rotation::classify_retention(d, reason);
                assert!(
                    VALID_RETENTION.contains(&tag),
                    "Event {ev:?} am {d} ergibt ungültigen retention_tag {tag:?}"
                );
            }
        }
    }

    async fn pool_with_settings() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system', action TEXT NOT NULL,
                entity_type TEXT, entity_id TEXT, details_json TEXT) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn setup_then_unlock_roundtrip() {
        let pool = pool_with_settings().await;
        let session = BackupSession::default();
        assert!(needs_onboarding(&pool).await.unwrap());

        // R5-005: Passphrase-Floor 16 — Test-Phrasen entsprechend lang.
        setup_passphrase(&pool, &session, "super-secret-roundtrip-1")
            .await
            .unwrap();
        assert!(!needs_onboarding(&pool).await.unwrap());
        assert!(session.is_unlocked());

        // Verifier darf nicht die Klartext-Passphrase enthalten.
        let ct = get_setting(&pool, SETTING_VERIFIER_CT)
            .await
            .unwrap()
            .unwrap();
        assert!(!ct.contains("super-secret"));

        // Frische Session, korrekt entsperren.
        let s2 = BackupSession::default();
        assert!(unlock(&pool, &s2, "super-secret-roundtrip-1")
            .await
            .unwrap());
        assert!(s2.is_unlocked());

        // Falsche Passphrase.
        let s3 = BackupSession::default();
        assert!(!unlock(&pool, &s3, "falsch-aber-lang-genug").await.unwrap());
        assert!(!s3.is_unlocked());
    }

    #[tokio::test]
    async fn setup_rejects_short_and_double() {
        let pool = pool_with_settings().await;
        let session = BackupSession::default();
        // R5-005: Floor liegt bei 16 — "kurz" (4) bleibt Reject; auch
        // "lang-genug-123" (14) wird jetzt abgelehnt. Erst eine
        // ≥16-Zeichen-Phrase wird akzeptiert.
        assert!(setup_passphrase(&pool, &session, "kurz").await.is_err());
        assert!(
            setup_passphrase(&pool, &session, "lang-genug-123")
                .await
                .is_err(),
            "14 Zeichen sind unter dem 16-Zeichen-Floor"
        );
        setup_passphrase(&pool, &session, "lang-genug-fuer-test")
            .await
            .unwrap();
        assert!(setup_passphrase(&pool, &session, "noch-eine-lang-genug")
            .await
            .is_err());
    }
}
