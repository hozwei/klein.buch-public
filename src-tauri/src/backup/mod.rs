//! Backup-Schicht (Block 4).
//!
//! - `snapshot`: konsistenter SQLite-Snapshot (WAL-Checkpoint + DB-Datei lesen) + Archive-Tree zippen.
//! - `encrypt`: AES-256-GCM mit Argon2id-derived Key aus Passphrase.
//! - `restore`: pre-restore-Backup → verify → apply.
//! - `rotation`: daily/monthly/yearly Retention (GVS).
//! - `target`: Backup-Ziele (Verzeichnisse unter `/backup-targets`, SFTP; mehrere, W7).
//! - `sftp`: SFTP-Upload-Arm (russh) mit Host-Key-Pinning; Passwort im SecretStore (W8).
//! - `manifest`: JSON-Header mit Versionsdaten + Hash.
//!
//! Dieser `mod.rs` enthält zusätzlich:
//! - [`BackupSession`] — die **Session-Passphrase im Prozess-Memory**
//!   (nie in DB/Logs/audit_log; siehe Backup-Hardline).
//! - Generische `app_settings`-Helfer.
//! - Passphrase-Setup/Unlock via verschlüsseltem Verifier (kein Klartext, kein
//!   Hash in der DB nötig).
//! - [`create_now`] — die Backup-Erstellungs-Pipeline.
//! - [`request_auto_backup`] — Lock-Events: Vermerk + Backup im Hintergrund
//!   (Scheduler, Lauf F3a); [`auto_backup_at`] — sofort (Tick, Nachholen).

pub mod after_restore;
pub mod encrypt;
pub mod manifest;
pub mod restore;
pub mod rotation;
pub mod sftp;
pub mod snapshot;
pub mod target;

use crate::config::Paths;
use crate::db::schema_version::EXPECTED_SCHEMA_VERSION;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::sync::Mutex;

/// Serialisiert [`create_now`] (Plan W2.3).
static BACKUP_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Lauf F8: Wartet höchstens `wait` darauf, dass kein Backup läuft, und hält
/// danach die Backup-Sperre. „Passphrase ändern“ tauscht die DB-Datei aus; ein
/// Backup, dessen Snapshot-Verbindung noch auf der alten Datei offen ist,
/// löschte beim Schließen sonst `-wal`/`-shm` des neuen Pools (SQLite-Sperren
/// gelten je Inode). `None` = Frist abgelaufen.
pub async fn exclusive_against_backups(
    wait: std::time::Duration,
) -> Option<tokio::sync::MutexGuard<'static, ()>> {
    tokio::time::timeout(wait, BACKUP_LOCK.lock()).await.ok()
}

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
/// laufenden Session (im `AppCtx`). Niemals serialisiert, niemals in
/// DB/Logs/audit_log. Beim Ersetzen und Löschen wird der alte Wert per
/// `zeroize` überschrieben (Plan W6: „Datenbank sperren“, Passphrase ändern).
///
/// Trägt außerdem das Wecksignal für ausstehende Lock-Backups (Lauf F3a,
/// W5-AUTOBACKUP-SYNC): [`request_auto_backup`] vermerkt das Backup und weckt
/// den Scheduler, der es außerhalb der Antwort des Commands anlegt.
#[derive(Default)]
pub struct BackupSession {
    inner: Mutex<Option<zeroize::Zeroizing<String>>>,
    backup_wake: tokio::sync::Notify,
}

impl BackupSession {
    /// Sperre ohne Panik bei Poisoning (KB-0034): der Inhalt (die Passphrase)
    /// ist nach einer Panik eines anderen Halters weiter gültig.
    fn guard(&self) -> std::sync::MutexGuard<'_, Option<zeroize::Zeroizing<String>>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Übernimmt die Passphrase (ohne Kopie); der bisherige Wert wird beim
    /// Ersetzen überschrieben.
    pub fn set(&self, passphrase: String) {
        // Der alte `Zeroizing`-Wert überschreibt sich beim Drop.
        *self.guard() = Some(zeroize::Zeroizing::new(passphrase));
    }

    /// Sperren/Herunterfahren: Passphrase überschreiben und verwerfen.
    pub fn clear(&self) {
        drop(self.guard().take());
    }

    pub fn is_unlocked(&self) -> bool {
        self.guard().is_some()
    }

    /// Liefert eine Kopie der Passphrase, falls die Session entsperrt ist —
    /// als `Zeroizing`: auch die Kopie wird beim Drop überschrieben (W14-015).
    /// Aufrufer reichen sie nur als `&str` weiter, ohne weitere Klone.
    pub fn get(&self) -> Option<zeroize::Zeroizing<String>> {
        self.guard().clone()
    }

    /// Weckt den Scheduler für ein ausstehendes Lock-Backup. Mehrere Signale
    /// vor dem Aufwachen zählen als eines (zusammengefasst).
    pub fn wake_backup(&self) {
        self.backup_wake.notify_one();
    }

    /// Wartet auf [`Self::wake_backup`] (Scheduler-Schleife).
    pub async fn backup_wake_requested(&self) {
        self.backup_wake.notified().await;
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

// ---------------------------------------------------------------------------
// Passphrase-Setup + Prüfung (Verifier statt Klartext/Hash)
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

    store_verifier(pool, passphrase).await?;
    session.set(passphrase.to_string());

    crate::db::repo::audit_log::append(
        pool,
        "backup.passphrase.setup",
        "backup",
        "passphrase",
        Some(serde_json::json!({"verifier":"stored"})),
    )
    .await?;
    Ok(())
}

/// Schreibt den Verifier (Salt + Nonce + Ciphertext einer Magic-Konstante) für
/// `passphrase` — bei der Einrichtung und nach „Passphrase ändern“ (W6).
pub async fn store_verifier(pool: &SqlitePool, passphrase: &str) -> Result<()> {
    let salt = encrypt::random_bytes(encrypt::SALT_LEN);
    let nonce = encrypt::random_bytes(encrypt::NONCE_LEN);
    let key = encrypt::derive_key_async(passphrase, &salt).await?;
    let nonce_arr = encrypt::nonce_array(&nonce)?;
    let ct = encrypt::encrypt(&key, &nonce_arr, encrypt::VERIFIER_PLAINTEXT)?;

    set_setting(pool, SETTING_VERIFIER_SALT, &manifest::to_hex(&salt)).await?;
    set_setting(pool, SETTING_VERIFIER_NONCE, &manifest::to_hex(&nonce)).await?;
    set_setting(pool, SETTING_VERIFIER_CT, &manifest::to_hex(&ct)).await?;
    set_setting(pool, SETTING_PASSPHRASE_SET, "true").await?;
    Ok(())
}

/// Prüft die Passphrase gegen den gespeicherten Verifier (ohne Session-Effekt;
/// das Entsperren der Instanz samt Audit läuft über `server::auth`, ADR 0039
/// Nr. 141).
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

    let key = encrypt::derive_key_async(passphrase, &salt).await?;
    match encrypt::decrypt(&key, &nonce, &ct) {
        Ok(pt) => Ok(pt == encrypt::VERIFIER_PLAINTEXT),
        Err(_) => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// Backup-Erstellung
// ---------------------------------------------------------------------------

/// Ergebnis eines erstellten Backups. `file_path` ist immer der **lokale Floor**
/// (primäre Kopie). Die Off-Site-Spiegelung (G1-BKP.4, Plan W7) ist
/// best-effort und je Ziel in `offsite` beschrieben.
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
    /// Ergebnis je Off-Site-Ziel (Plan W7, mehrere Ziele).
    pub offsite: Vec<OffsiteResult>,
}

/// Ergebnis der Spiegelung auf ein Off-Site-Ziel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OffsiteResult {
    pub target_id: String,
    pub label: String,
    /// Pfad/URI bei Erfolg.
    pub location: Option<String>,
    /// Fehlertext bei Fehlschlag.
    pub error: Option<String>,
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
    db_schema_version: i32,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO backup_history
            (id, target_path, file_hash_sha256, file_size_bytes, is_encrypted,
             retention_tag, trigger_reason, db_schema_version, app_version, created_at)
         VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(target_path)
    .bind(file_hash)
    .bind(size_bytes)
    .bind(retention_tag)
    .bind(trigger_reason)
    .bind(db_schema_version)
    .bind(app_version)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Pro-Lauf-Backup-Ergebnis-Hinweis (G1-NOTIFY), gated über die abschaltbare
/// Regel `rule_backup_result` (Migration `0027`). Schreibt in die In-App-Inbox
/// (Quelle der Wahrheit und einziger Kanal, ADR 0027). Best-effort: ein
/// Hinweis-Fehler darf ein Backup nie beeinflussen. Enthält **nie** ein
/// Geheimnis (nur Status/Dateiname).
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
    // In die In-App-Inbox; Dedup über `dedup_key` (`notify::emit` ist dasselbe).
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

/// Protokolliert einen gescheiterten Lauf vor bzw. beim Schreiben des Floors
/// (G1-LOG, W2-SNAP-LOG): Zeile `failed` im append-only `backup_log` (Ziel
/// `local`, geplanter Dateiname und Pfad) und Inbox-Hinweis über die Regel
/// `rule_backup_result` (Dedup über `dedup_key`). Best-effort.
#[allow(clippy::too_many_arguments)]
async fn record_local_failure(
    pool: &SqlitePool,
    trigger_reason: &str,
    file_name: &str,
    full_path: &str,
    size_bytes: i64,
    detail: String,
    body: &str,
    dedup_key: &str,
) {
    crate::db::repo::backup_log::insert(
        pool,
        &crate::db::repo::backup_log::BackupLogEntry {
            trigger: trigger_reason.to_string(),
            target_kind: "local".to_string(),
            target_label: None,
            file_name: file_name.to_string(),
            full_path: full_path.to_string(),
            size_bytes,
            status: "failed".to_string(),
            detail: Some(detail),
        },
    )
    .await
    .ok();
    notify_backup_result(pool, "warning", "Backup fehlgeschlagen", body, dedup_key).await;
}

/// Verschlüsselt einen Content-ZIP zu einer `.kbk`-Datei (Manifest-Frame +
/// AES-256-GCM, Schlüssel per Argon2id mit frischem Salt/Nonce; Format 3 mit
/// dem Kopf als AAD). Blockierend (KDF) — aus async-Code über
/// `limits::run_blocking` aufrufen. Kopiert den Inhalt einmal; intern nutzt
/// [`create_with`] die Variante ohne Kopie ([`seal_backup_at`]).
pub fn seal_backup(
    zip_bytes: &[u8],
    content_sha256: &str,
    passphrase: &str,
    schema_version: i32,
) -> Result<(manifest::Manifest, Vec<u8>)> {
    seal_backup_at(
        zip_bytes.to_vec(),
        content_sha256,
        passphrase,
        schema_version,
        crate::clock::now_utc(),
        manifest::FORMAT_VERSION,
    )
}

/// Wie [`seal_backup`], mit vorgegebenem Erstellungszeitpunkt im Manifest
/// (injizierte Uhr, Lauf F) und **in place** (Lauf F3a): Der Puffer des
/// Content-ZIP wird zur fertigen Datei — Kopf davor, Inhalt verschlüsselt,
/// Tag dahinter. Speicherbedarf etwa einmal die Backup-Größe statt dreimal
/// (ZIP, Ciphertext, Frame).
///
/// `format_version` (Lauf F3c): Format 3 bindet den Kopf als AAD; Format 2
/// (gleicher Inhalt, leere AAD wie bis v2026.10) nur für das Pflicht-Backup
/// vor einer Migration, solange die Vorversion Format 3 nicht liest
/// ([`manifest::readable_format_for_schema`]).
pub fn seal_backup_at(
    zip_bytes: Vec<u8>,
    content_sha256: &str,
    passphrase: &str,
    schema_version: i32,
    created_at: DateTime<Utc>,
    format_version: u32,
) -> Result<(manifest::Manifest, Vec<u8>)> {
    manifest::ensure_writable_format(format_version)?;
    let salt = encrypt::random_bytes(encrypt::SALT_LEN);
    let nonce = encrypt::random_bytes(encrypt::NONCE_LEN);
    let key = zeroize::Zeroizing::new(encrypt::derive_key(passphrase.as_bytes(), &salt)?);
    let man = manifest::Manifest {
        magic: manifest::MAGIC.to_string(),
        format_version,
        schema_version,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        kdf: manifest::KdfParams {
            algo: "argon2id".to_string(),
            m_cost_kib: encrypt::KDF_M_COST_KIB,
            t_cost: encrypt::KDF_T_COST,
            p_cost: encrypt::KDF_P_COST,
        },
        salt_hex: manifest::to_hex(&salt),
        nonce_hex: manifest::to_hex(&nonce),
        content_sha256: content_sha256.to_string(),
        content_size_bytes: zip_bytes.len() as u64,
    };
    // Kopf (= AAD ab Format 3, W14-020) vor den Inhalt schieben: Platz für Kopf
    // und Tag reservieren, dann im selben Puffer verschieben (keine Kopie).
    let header = manifest::header_bytes(&man)?;
    let mut buf = zip_bytes;
    let content_len = buf.len();
    buf.reserve_exact(header.len() + encrypt::TAG_LEN);
    buf.resize(content_len + header.len(), 0);
    buf.copy_within(0..content_len, header.len());
    buf[..header.len()].copy_from_slice(&header);
    let (head, body) = buf.split_at_mut(header.len());
    // Format 2: ohne AAD wie `encrypt::encrypt` bis v2026.10 (Tag am Ende).
    let aad: &[u8] = if manifest::binds_header(format_version) {
        head
    } else {
        &[]
    };
    let tag = encrypt::encrypt_in_place_detached(&key, &encrypt::nonce_array(&nonce)?, aad, body)?;
    buf.extend_from_slice(&tag);
    Ok((man, buf))
}

/// Trägt eine vorhandene `.kbk` im lokalen Backup-Ordner in `backup_history`
/// ein (Pre-Restore-Backup nach dem Tausch, Gegenprüfung W7; verwaiste
/// Backups nach einem Restore, D-22). Schema-Version und Erstellungszeit aus
/// dem Manifest; Aufbewahrungsklasse `manual` (nie automatisch gelöscht).
/// Unbekannte oder fremde Namen werden ignoriert. `true` = neu eingetragen.
pub async fn record_existing_backup(
    pool: &SqlitePool,
    backups_dir: &std::path::Path,
    file_name: &str,
    trigger_reason: &str,
) -> Result<bool> {
    if file_name.is_empty() || file_name.contains(['/', '\\']) || !file_name.ends_with(".kbk") {
        return Ok(false);
    }
    let path = backups_dir.join(file_name);
    let target = path.to_string_lossy().to_string();
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM backup_history WHERE target_path = ?")
            .bind(&target)
            .fetch_one(pool)
            .await?;
    if exists > 0 {
        return Ok(false);
    }
    let bytes = tokio::fs::read(&path).await?;
    let header = manifest::read_manifest_only(&bytes)?;
    let created_at = crate::clock::db_utc(&header.created_at)
        .map(crate::clock::to_db_utc)
        .unwrap_or_else(|| crate::clock::to_db_utc(crate::clock::now_utc()));
    insert_history_row(
        pool,
        &uuid::Uuid::now_v7().to_string(),
        &target,
        &snapshot::sha256_hex(&bytes),
        bytes.len() as i64,
        "manual",
        trigger_reason,
        &header.app_version,
        header.schema_version,
        &created_at,
    )
    .await?;
    Ok(true)
}

/// Gleicht nach einem Restore die Historie mit dem lokalen Backup-Ordner ab
/// (D-22): `klein-buch-*.kbk` ohne Zeile (entstanden nach dem Stand des
/// eingespielten Backups) werden als `manual` übernommen — nie automatisch
/// gelöscht, in der Restore-Liste wählbar. Zeilen des Floors, deren Datei
/// fehlt (seit dem Backup wegrotiert), werden entfernt. Liefert
/// `(übernommen, entfernt)`; Audit `backup.history_rescanned`.
pub async fn rescan_floor(
    pool: &SqlitePool,
    backups_dir: &std::path::Path,
) -> Result<(usize, usize)> {
    let mut names: Vec<String> = match std::fs::read_dir(backups_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("klein-buch-") && n.ends_with(".kbk"))
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    let mut added = 0usize;
    for name in &names {
        match record_existing_backup(pool, backups_dir, name, "manual").await {
            Ok(true) => added += 1,
            Ok(false) => {}
            // Unlesbare oder fremde Datei: nicht übernehmen, Abgleich läuft weiter.
            Err(e) => tracing::warn!("Backup {name} nicht in die Historie übernommen: {e}"),
        }
    }
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, target_path FROM backup_history")
        .fetch_all(pool)
        .await?;
    let mut removed = 0usize;
    for (id, target) in rows {
        if rotation::is_floor_path(&target, backups_dir)
            && matches!(std::fs::metadata(&target), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            sqlx::query("DELETE FROM backup_history WHERE id = ?")
                .bind(&id)
                .execute(pool)
                .await?;
            removed += 1;
        }
    }
    if added > 0 || removed > 0 {
        crate::db::repo::audit_log::append(
            pool,
            "backup.history_rescanned",
            "backup",
            "floor",
            Some(serde_json::json!({ "added": added, "removed": removed })),
        )
        .await?;
    }
    Ok((added, removed))
}

/// Liest eine geschriebene `.kbk` und prüft sie vollständig (Entschlüsseln +
/// Content-Hash). Für das Pflicht-Backup vor einer Migration (Plan W7).
pub async fn verify_backup_file(path: &std::path::Path, passphrase: &str) -> Result<()> {
    let bytes = tokio::fs::read(path).await?;
    let pass = zeroize::Zeroizing::new(passphrase.to_string());
    crate::limits::run_blocking(&crate::limits::KDF, move || {
        restore::decrypt_backup_owned(bytes, &pass).map(|_| ())
    })
    .await
}

/// Spiegelung eines neuen Backups auf die Off-Site-Ziele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    /// Auf alle Ziele — außer während einer Pause nach einer Wiederherstellung
    /// (D-23): dann nur der lokale Floor, bis der Inhaber freigibt.
    Auto,
    /// Nur der lokale Floor (Pflicht-Backup vor der Migration eines gerade
    /// eingespielten Stands, dessen Pause noch nicht gesetzt ist; D-23).
    FloorOnly,
}

/// Optionen für [`create_with`].
#[derive(Debug, Clone, Copy)]
pub struct CreateOptions {
    pub mirror: Mirror,
    /// Erstellungszeitpunkt (Manifest, Dateiname, Historie, Klasse) — aus der
    /// injizierten Uhr des Aufrufers (Lauf F, D-04/D-20).
    pub now: DateTime<Utc>,
    /// Backup-Format, in der Regel [`manifest::FORMAT_VERSION`]. Das
    /// Pflicht-Backup vor einer Migration nutzt das Format, das die Vorversion
    /// liest (Lauf F3c, [`manifest::readable_format_for_schema`]).
    pub format_version: u32,
}

/// Fehltext der ausgesetzten Spiegelung (Ergebnis je Ziel).
pub const MIRROR_SUSPENDED: &str =
    "Ausgesetzt bis zur Freigabe nach der Wiederherstellung (nur lokale Kopie).";

/// Erstellt **jetzt** ein verschlüsseltes Backup (Systemuhr, Spiegelung wie
/// konfiguriert), siehe [`create_with`].
pub async fn create_now(
    pool: &SqlitePool,
    paths: &Paths,
    passphrase: &str,
    trigger_reason: &str,
) -> Result<BackupOutcome> {
    create_with(
        pool,
        paths,
        passphrase,
        trigger_reason,
        CreateOptions {
            mirror: Mirror::Auto,
            now: crate::clock::now_utc(),
            format_version: manifest::FORMAT_VERSION,
        },
    )
    .await
}

/// Erstellt ein verschlüsseltes Backup: Snapshot → Argon2id-Key →
/// AES-256-GCM → Manifest-Frame → **Floor schreiben** (immer lokal) →
/// `backup_history` → Audit → **Off-Site-Spiegelung** (best-effort, G1-BKP.4) →
/// Rotation. Surfaced Floor-Fehler an den Aufrufer; ein fehlgeschlagener
/// Off-Site-Mirror ist **nicht** fatal (der Floor ist die Sicherheit).
///
/// Lauf F: Während einer Pause nach einer Wiederherstellung (`scheduler_hold =
/// restore`) oder mit [`Mirror::FloorOnly`] wird nicht gespiegelt (D-23,
/// Audit `backup.mirror.skipped`); während jeder Pause ruht die Rotation
/// (keine Löschung nach einem Uhrsprung oder auf einer Probe-Instanz).
pub async fn create_with(
    pool: &SqlitePool,
    paths: &Paths,
    passphrase: &str,
    trigger_reason: &str,
    opts: CreateOptions,
) -> Result<BackupOutcome> {
    // Plan W2.3: Backups laufen nacheinander (Auto-Backup nach Lock, Tages-Backup
    // im Tick, manuell, Pre-Restore) — nie zwei gleichzeitig.
    let _serial = BACKUP_LOCK.lock().await;
    let now_berlin = opts.now.with_timezone(&chrono_tz::Europe::Berlin);
    let day = now_berlin.format("%Y-%m-%d").to_string();
    // Dateiname vorab — auch die Fehlerzeile eines gescheiterten Snapshots nennt
    // ihn (W2-SNAP-LOG). Millisekunden + Kurz-UUID: zwei Backups in derselben
    // Sekunde ergeben zwei Dateien (Plan W2.3).
    let stamp = now_berlin.format("%Y%m%d-%H%M%S%3f").to_string();
    let short = uuid::Uuid::now_v7().simple().to_string();
    let file_name = format!("klein-buch-{stamp}-{}.kbk", &short[short.len() - 8..]);
    let floor_file = paths
        .backups_dir
        .join(&file_name)
        .to_string_lossy()
        .to_string();
    // Vor dem Snapshot: Stand der vermerkten Lock-Ereignisse (alles bis hier
    // Festgeschriebene steht im Snapshot).
    let seq_at_start = pending_seq(pool).await;

    // 1.–3. Snapshot, Schema-Version, Verschlüsselung — mit gemeinsamer
    //       Fehlerbehandlung (W2-SNAP-LOG, Lauf F3a): ein Fehlschlag steht wie
    //       ein gescheiterter Floor im Backup-Protokoll und erzeugt einen
    //       Hinweis (Dedup je Tag), nicht nur Log und Audit.
    #[allow(clippy::type_complexity)]
    let sealed: Result<(manifest::Manifest, Vec<u8>, i32, Vec<snapshot::Skipped>)> = async {
        // Konsistenter Content-Snapshot. Die DB ist mit derselben Passphrase
        // verschlüsselt (eine Passphrase-Kette, ADR 0035); Klartext nur in Tests.
        let db_key = (!crate::db::db_file_is_plaintext(&paths.db_file)).then_some(passphrase);
        let content = snapshot::build_content_zip(paths, db_key).await?;
        // Schema-Version der gesicherten DB — vor einer Migration älter als
        // `EXPECTED` (Pflicht-Backup vor Migration, Plan W7).
        let schema_version = crate::db::schema_version::read(pool)
            .await?
            .unwrap_or(EXPECTED_SCHEMA_VERSION);
        // Verschlüsseln in place (Argon2id + AES außerhalb der Runtime, KDF
        // begrenzt, Plan W2.6) mit dem Kopf als AAD (Format 3; Format 2 nur
        // vor einer Migration, Lauf F3c).
        let pass = zeroize::Zeroizing::new(passphrase.to_string());
        let snapshot::Content {
            zip_bytes,
            sha256_hex,
            skipped,
        } = content;
        let (now, format) = (opts.now, opts.format_version);
        let (man, bytes) = crate::limits::run_blocking(&crate::limits::KDF, move || {
            seal_backup_at(zip_bytes, &sha256_hex, &pass, schema_version, now, format)
        })
        .await?;
        Ok((man, bytes, schema_version, skipped))
    }
    .await;
    let (man, file_bytes, schema_version, skipped) = match sealed {
        Ok(sealed) => sealed,
        Err(e) => {
            tracing::error!("Backup ({trigger_reason}) nicht erstellt: {e}");
            record_local_failure(
                pool,
                trigger_reason,
                &file_name,
                &floor_file,
                0,
                format!("Snapshot/Verschlüsselung: {e}"),
                "Die Sicherung konnte nicht erstellt werden (Snapshot der Datenbank, \
                 Lesen des Archivs oder Verschlüsselung). Details im Backup-Protokoll.",
                &format!("backup_result_failed_create:{day}"),
            )
            .await;
            return Err(e);
        }
    };
    let app_version = man.app_version.clone();
    let created_at = man.created_at.clone();
    let history_created_at = crate::clock::to_db_utc(opts.now);
    let file_hash = snapshot::sha256_hex(&file_bytes);
    let size_bytes = file_bytes.len() as i64;
    // Eine Datei für alle Ziele, ohne Kopie je Ziel (Lauf F3a).
    let file_bytes = std::sync::Arc::new(file_bytes);
    let retention_tag =
        rotation::classify_retention(now_berlin.date_naive(), trigger_reason).to_string();
    // Pause der automatischen Belegerzeugung (Lauf F): Grund `restore` setzt
    // die Spiegelung aus (D-23), jede Pause die Rotation.
    let hold = crate::scheduler::hold::current(pool).await.ok().flatten();
    let mirror_suspended = opts.mirror == Mirror::FloorOnly
        || hold
            .as_ref()
            .is_some_and(|h| h.reason == crate::scheduler::hold::REASON_RESTORE);

    // 4. Floor schreiben — IMMER lokal (`paths.backups_dir`). Das ist die
    //    primäre, nicht-abschaltbare Sicherheits-Kopie (G1-BKP.4 Tier-Modell).
    let floor_target = target::BackupTarget::Directory {
        path: paths.backups_dir.to_string_lossy().to_string(),
    };
    let floor_path = match target::write_backup_shared(
        &floor_target,
        None,
        &file_name,
        file_bytes.clone(),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            // Floor-Fehlschlag protokollieren (best-effort), dann hart melden:
            // ohne Floor gibt es keine Sicherheits-Kopie (G1-LOG: jeder Lauf wird
            // protokolliert, Erfolg UND Fehlschlag).
            record_local_failure(
                pool,
                trigger_reason,
                &file_name,
                &floor_file,
                size_bytes,
                e.to_string(),
                "Die lokale Pflicht-Sicherung konnte nicht geschrieben werden. \
                 Prüfe Speicherplatz und Schreibrechte. Details im Backup-Protokoll.",
                &format!("backup_result_failed_floor:{day}"),
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
        schema_version,
        &history_created_at,
    )
    .await?;
    crate::db::repo::audit_log::append(
        pool,
        "backup.created",
        "backup",
        &history_id,
        Some(serde_json::json!({
            "trigger": trigger_reason,
            "tag": retention_tag,
            "size": size_bytes,
            "skipped": skipped.len(),
        })),
    )
    .await
    .ok();
    // Ein ausstehendes Lock-Backup ist damit erledigt, sofern seit dem Beginn
    // dieses Backups kein weiteres Ereignis vermerkt wurde (F1d, Lauf F3a:
    // Zähler statt Zeitstempel). Ein während des Backups festgeschriebener
    // Beleg hält den Vermerk stehen.
    sqlx::query(
        "DELETE FROM app_settings WHERE key IN (?1, ?2)
           AND COALESCE((SELECT value FROM app_settings WHERE key = ?2), '') = ?3",
    )
    .bind(PENDING_KEY)
    .bind(PENDING_SEQ_KEY)
    .bind(&seq_at_start)
    .execute(pool)
    .await
    .ok();
    // Floor-Erfolg ins Backup-Protokoll (G1-LOG, append-only; nie fatal).
    // Lauf F3c: nicht gesicherte Einträge (Links, verschwundene Archivdateien)
    // stehen im Detail und als Hinweis — ein Backup wird nie still gekürzt.
    let incomplete = incomplete_detail(&skipped);
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
            detail: incomplete.clone(),
        },
    )
    .await
    .ok();
    if let Some(detail) = &incomplete {
        tracing::warn!("Backup {file_name} unvollständig: {detail}");
        notify_backup_result(
            pool,
            "warning",
            "Backup unvollständig",
            &format!(
                "Die Sicherung {file_name} enthält nicht alles: {detail}. Klein.Buch \
                 sichert nur echte Dateien im Archiv; einen ausgelagerten Ordner bitte \
                 als Ordner zurückholen oder als Volume einbinden, eine fehlende \
                 Archivdatei über die Integritätsprüfung klären. Details im \
                 Backup-Protokoll."
            ),
            &format!("backup_result_incomplete:{day}"),
        )
        .await;
    }

    // 5. Off-Site-Spiegelung auf jedes Ziel (Plan W7, best-effort). Fehlschlag
    //    ist NICHT fatal: der Floor ist gesichert, und ein nicht erreichbares
    //    Ziel darf einen GoBD-festgeschriebenen Vorgang nie zurückrollen. Jedes
    //    Ziel wird einzeln protokolliert (`backup_log`, Audit, Ergebnis für den
    //    Status `offsite`); Fehler und fehlende Ziele erzeugen einen
    //    Inbox-Hinweis.
    let mut offsite = Vec::new();
    // Seit W7 auch das Pre-Restore-Backup (einzige vollständige Kopie des
    // ersetzten Stands, Gegenprüfung W7); nur die Rotation spart es aus.
    let targets = target::list_targets(pool, &paths.backup_targets_dir).await?;
    if mirror_suspended && !targets.is_empty() {
        // D-23: Eine wiederhergestellte (oder Probe-)Instanz schreibt keinen
        // älteren Datenstand mit neuerem Zeitstempel neben die echten Backups.
        crate::db::repo::audit_log::append(
            pool,
            "backup.mirror.skipped",
            "backup",
            &history_id,
            Some(serde_json::json!({
                "reason": "restore_hold",
                "targets": targets.len(),
                "trigger": trigger_reason,
            })),
        )
        .await
        .ok();
        tracing::info!(
            "Off-Site-Spiegelung ausgesetzt (Pause nach Wiederherstellung) — nur lokale Kopie"
        );
        for entry in &targets {
            offsite.push(OffsiteResult {
                target_id: entry.id.clone(),
                label: entry.label.clone(),
                location: None,
                error: Some(MIRROR_SUSPENDED.to_string()),
            });
        }
    } else {
        if targets.is_empty() {
            // Dedup je Kalenderwoche: solange kein Ziel eingerichtet ist,
            // kommt der Hinweis wöchentlich wieder.
            let week = now_berlin.format("%G-W%V").to_string();
            crate::notify::store::create(
                pool,
                crate::notify::NewNotification {
                    title: "Kein externes Backup-Ziel",
                    body: "Backups liegen nur im lokalen Datenverzeichnis. Richte unter \
                           „Backups“ mindestens ein externes Ziel ein (Ordner unter \
                           /backup-targets oder SFTP) — sonst ist bei einem Ausfall des \
                           Servers alles weg.",
                    severity: "warning",
                    action_url: Some("/settings/backup"),
                    dedup_key: Some(&format!("backup_offsite_missing:{week}")),
                    ..Default::default()
                },
            )
            .await
            .ok();
        }
        let mut run = Vec::new();
        for entry in &targets {
            let (kind, label) = target::log_meta(entry);
            // Gesamtgrenze je Ziel: ein hängendes Ziel staut nicht alle
            // weiteren Backups (Gegenprüfung W7).
            let limit =
                std::time::Duration::from_secs(60 + file_bytes.len() as u64 / (1024 * 1024));
            let password = match &entry.target {
                target::BackupTarget::Sftp { .. } => {
                    crate::secrets::get(pool, &crate::secrets::sftp_key(&entry.id))
                        .await
                        .ok()
                        .flatten()
                }
                target::BackupTarget::Directory { .. } => None,
            };
            let written = match target::resolve(entry, &paths.backup_targets_dir) {
                Ok(resolved) => match tokio::time::timeout(
                    limit,
                    target::write_backup_shared(
                        &resolved,
                        password.as_deref().map(String::as_str),
                        &file_name,
                        file_bytes.clone(),
                    ),
                )
                .await
                {
                    Ok(r) => r.map_err(|e| (e, resolved)),
                    Err(_) => Err((
                        Error::Backup(format!("Zeitüberschreitung nach {} s", limit.as_secs())),
                        resolved,
                    )),
                },
                Err(e) => Err((e, entry.target.clone())),
            };
            // Zeitpunkt aus der injizierten Uhr des Aufrufers (F1d), nicht
            // der Systemuhr.
            let at = now_berlin.to_rfc3339();
            match written {
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
                        schema_version,
                        &history_created_at,
                    )
                    .await
                    .ok();
                    crate::db::repo::audit_log::append(
                        pool,
                        "backup.mirror.ok",
                        "backup",
                        &mirror_id,
                        Some(serde_json::json!({ "target": entry.id, "location": offsite_path })),
                    )
                    .await
                    .ok();
                    crate::db::repo::backup_log::insert(
                        pool,
                        &crate::db::repo::backup_log::BackupLogEntry {
                            trigger: trigger_reason.to_string(),
                            target_kind: kind.to_string(),
                            target_label: label.clone(),
                            file_name: file_name.clone(),
                            full_path: offsite_path.clone(),
                            size_bytes,
                            status: "ok".to_string(),
                            detail: None,
                        },
                    )
                    .await
                    .ok();
                    run.push((
                        entry.id.clone(),
                        target::TargetResult {
                            ok: true,
                            at,
                            error: None,
                        },
                    ));
                    offsite.push(OffsiteResult {
                        target_id: entry.id.clone(),
                        label: entry.label.clone(),
                        location: Some(offsite_path),
                        error: None,
                    });
                }
                Err((e, failed_target)) => {
                    tracing::error!(
                        "Off-Site-Spiegelung auf „{}“ fehlgeschlagen ({trigger_reason}): {e}",
                        entry.label
                    );
                    crate::db::repo::audit_log::append(
                        pool,
                        "backup.mirror.failed",
                        "backup",
                        &history_id,
                        Some(serde_json::json!({ "target": entry.id, "error": e.to_string() })),
                    )
                    .await
                    .ok();
                    crate::db::repo::backup_log::insert(
                        pool,
                        &crate::db::repo::backup_log::BackupLogEntry {
                            trigger: trigger_reason.to_string(),
                            target_kind: kind.to_string(),
                            target_label: label.clone(),
                            file_name: file_name.clone(),
                            full_path: target::log_failed_path(&failed_target, &file_name),
                            size_bytes,
                            status: "failed".to_string(),
                            detail: Some(e.to_string()),
                        },
                    )
                    .await
                    .ok();
                    let body = format!(
                        "Die Sicherung auf das externe Ziel „{}“ schlug fehl: {e}. Die lokale \
                         Pflichtkopie und die übrigen Ziele sind gesichert. Details im \
                         Backup-Protokoll.",
                        entry.label
                    );
                    crate::notify::store::create(
                        pool,
                        crate::notify::NewNotification {
                            title: "Externe Backup-Spiegelung fehlgeschlagen",
                            body: &body,
                            severity: "warning",
                            action_url: Some("/settings/backup-log"),
                            dedup_key: Some(&format!(
                                "backup_result_failed_offsite:{}:{}",
                                entry.id,
                                now_berlin.format("%Y-%m-%d")
                            )),
                            ..Default::default()
                        },
                    )
                    .await
                    .ok();
                    run.push((
                        entry.id.clone(),
                        target::TargetResult {
                            ok: false,
                            at,
                            error: Some(e.to_string()),
                        },
                    ));
                    offsite.push(OffsiteResult {
                        target_id: entry.id.clone(),
                        label: entry.label.clone(),
                        location: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }
        target::store_results(pool, &targets, &run).await.ok();
    }

    // 6. Rotation (getiert) — außer direkt vor einem Restore (Pre-Restore nie
    //    prunen), bei reiner Floor-Kopie und während einer Pause (Lauf F: ein
    //    Uhrsprung oder der Stand einer Probe-Instanz darf keine echten
    //    Backups wegrotieren).
    if trigger_reason != "pre_restore" && opts.mirror == Mirror::Auto && hold.is_none() {
        let _ = rotation::run(
            pool,
            &paths.backups_dir,
            &paths.backup_targets_dir,
            opts.now,
        )
        .await;
    } else if hold.is_some() {
        tracing::info!("Backup-Rotation ruht während der Pause der automatischen Abläufe");
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
        offsite,
    })
}

/// Höchstens so viele nicht gesicherte Einträge nennt das Detail einzeln.
const INCOMPLETE_LISTED: usize = 5;

/// Detail für das Backup-Protokoll, wenn der Snapshot Einträge nicht sichern
/// konnte (Lauf F3c); `None` = vollständig.
fn incomplete_detail(skipped: &[snapshot::Skipped]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let listed: Vec<String> = skipped
        .iter()
        .take(INCOMPLETE_LISTED)
        .map(ToString::to_string)
        .collect();
    let more = skipped.len().saturating_sub(INCOMPLETE_LISTED);
    let tail = if more > 0 {
        format!(" und {more} weitere")
    } else {
        String::new()
    };
    let count = match skipped.len() {
        1 => "1 Eintrag".to_string(),
        n => format!("{n} Einträge"),
    };
    Some(format!(
        "Unvollständig: {count} nicht gesichert — {}{tail}",
        listed.join(", ")
    ))
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

/// Lock-Ereignis (Backup-Hardline; Lauf F3a W5-AUTOBACKUP-SYNC, ADR 0040
/// Nr. 50): Das Festschreiben (Rechnung, Storno, Kosten, Privatbewegung,
/// Anlage, AfA, Jahresabschluss, Angebot) wartet nicht mehr auf das Backup.
///
/// - Session entsperrt → Vermerk „Backup ausstehend“ dauerhaft in der DB
///   ([`mark_pending`]), Audit `backup.requested` mit dem Ereignis, dann wird
///   der Scheduler geweckt ([`BackupSession::wake_backup`]). Er legt das Backup
///   kurz darauf außerhalb der Antwort an; mehrere Ereignisse in kurzer Folge
///   ergeben ein Backup, und „Datenbank sperren“, Passphrase-Wechsel, Restore
///   und Herunterfahren warten wie beim Tick auf ein laufendes Backup. Fällt es
///   aus (Sperren, Neustart), holt es der erste Tick danach nach
///   ([`pending_backup_if_due`]).
/// - Lässt sich der Vermerk nicht schreiben, läuft das Backup sofort (wie
///   bisher) — ohne Vermerk wäre es sonst nicht gesichert.
/// - Session gesperrt → kein Backup, Audit `backup.skipped_locked`, Vermerk.
///
/// Ein Fehler wird nie zum Fehler des auslösenden Vorgangs (eine
/// festgeschriebene Rechnung darf nicht zurückrollen, weil ein Backup-Ziel
/// nicht erreichbar war).
pub async fn request_auto_backup(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    event: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    if !session.is_unlocked() {
        // Wie bisher: Audit `backup.skipped_locked`, Vermerk, kein Backup.
        return auto_backup_at(pool, paths, session, event, now)
            .await
            .map(|_| ());
    }
    match mark_pending(pool, now).await {
        Ok(()) => {
            crate::db::repo::audit_log::append(
                pool,
                "backup.requested",
                "backup",
                "auto",
                Some(serde_json::json!({ "event": event })),
            )
            .await
            .ok();
            session.wake_backup();
            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                "Vermerk „Backup ausstehend“ nicht gesetzt ({event}): {e} — Backup sofort"
            );
            auto_backup_at(pool, paths, session, event, now)
                .await
                .map(|_| ())
        }
    }
}

/// Auto-Backup **sofort** für ein Lock-Ereignis (Scheduler: gesammeltes
/// Backup am Ende des Ticks, geweckter Lauf nach [`request_auto_backup`],
/// Nachholen; Rückfall ohne Vermerk), mit dem Zeitpunkt der injizierten Uhr
/// des Aufrufers.
///
/// - Session nicht entsperrt → kein Backup, Audit-Marker und Vermerk,
///   `Ok(None)`.
/// - Session entsperrt → [`create_with`]. Schlägt das Backup fehl (z. B. Ziel
///   offline), wird das **nicht** zum harten Fehler — der Fehler wird geloggt,
///   auditiert und steht im Backup-Protokoll; `Ok(None)`.
pub async fn auto_backup_at(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    event: &str,
    now: DateTime<Utc>,
) -> Result<Option<BackupOutcome>> {
    let Some(passphrase) = session.get() else {
        crate::db::repo::audit_log::append(
            pool,
            "backup.skipped_locked",
            "backup",
            "auto",
            Some(serde_json::json!({ "event": event })),
        )
        .await
        .ok();
        // F1d: Das erste Entsperren holt es nach (best-effort; nach dem
        // Sperren ist der Pool womöglich schon zu — dann steht der Vermerk
        // aus dem Sammeln des Ticks).
        mark_pending(pool, now).await.ok();
        tracing::warn!("Auto-Backup übersprungen: Session nicht entsperrt ({event})");
        return Ok(None);
    };

    // Lock-Event-Label → gültige `backups.trigger_reason`-Kategorie. Das konkrete
    // Event bleibt im Audit-Log (`event`) nachvollziehbar.
    let opts = CreateOptions {
        mirror: Mirror::Auto,
        now,
        format_version: manifest::FORMAT_VERSION,
    };
    match create_with(pool, paths, &passphrase, db_trigger_reason(event), opts).await {
        Ok(outcome) => Ok(Some(outcome)),
        Err(e) => {
            tracing::error!("Auto-Backup fehlgeschlagen ({event}): {e}");
            crate::db::repo::audit_log::append(
                pool,
                "backup.failed",
                "backup",
                "auto",
                Some(serde_json::json!({ "event": event, "error": e.to_string() })),
            )
            .await
            .ok();
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Ausstehendes Lock-Backup (Lauf F, F1d)
// ---------------------------------------------------------------------------

/// `app_settings`: Zeitpunkt (UTC) des ältesten noch nicht gesicherten
/// Festschreibens. Gesetzt bei jedem Lock-Ereignis ([`request_auto_backup`]),
/// wenn der Tick ein Lock-Backup sammelt oder ein Lock-Backup mangels
/// Passphrase ausfällt; entfernt von einem erfolgreichen Backup, während
/// dessen kein weiteres Ereignis vermerkt wurde ([`PENDING_SEQ_KEY`]).
/// Übersteht „Datenbank sperren“ und das Herunterfahren.
pub const PENDING_KEY: &str = "backup_pending_since";
/// `app_settings`: Zähler der vermerkten Lock-Ereignisse (Lauf F3a). Ein
/// Backup liest ihn vor dem Snapshot und löscht den Vermerk danach nur, wenn
/// er unverändert ist — ein während des Backups festgeschriebener Beleg bleibt
/// vermerkt. Unabhängig von der Uhr (Uhrsprung, injizierte Testuhr).
pub const PENDING_SEQ_KEY: &str = "backup_pending_seq";
/// `app_settings`: letzter Fehlschlag beim Nachholen (Backoff, UTC).
pub const PENDING_BACKOFF_KEY: &str = "backup_pending_last_failure_at";

/// Vermerkt ein ausstehendes Lock-Backup: der älteste Zeitpunkt bleibt
/// ([`PENDING_KEY`]), der Zähler ([`PENDING_SEQ_KEY`]) steigt; eine
/// Anweisung, also atomar.
pub async fn mark_pending(pool: &SqlitePool, now: DateTime<Utc>) -> Result<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?3), (?2, '1')
         ON CONFLICT(key) DO UPDATE
            SET value = CAST(CAST(app_settings.value AS INTEGER) + 1 AS TEXT),
                updated_at = datetime('now','utc')
          WHERE app_settings.key = ?2",
    )
    .bind(PENDING_KEY)
    .bind(PENDING_SEQ_KEY)
    .bind(crate::clock::to_db_utc(now))
    .execute(pool)
    .await?;
    Ok(())
}

/// Steht ein Lock-Backup aus ([`PENDING_KEY`])? Für das Nachholen vor dem
/// Sperren und Herunterfahren (Lauf F3c).
pub async fn is_backup_pending(pool: &SqlitePool) -> Result<bool> {
    Ok(crate::db::repo::app_settings::get(pool, PENDING_KEY)
        .await?
        .is_some())
}

/// Stand des Zählers vor dem Snapshot (leer = kein Zähler, z. B. Vermerk aus
/// F1 oder keiner).
async fn pending_seq(pool: &SqlitePool) -> String {
    crate::db::repo::app_settings::get(pool, PENDING_SEQ_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Vom Scheduler geweckter Lauf nach [`request_auto_backup`] (Lauf F3a):
/// legt das ausstehende Backup an, sofern der Vermerk noch steht (ein Tick
/// kann es schon erledigt haben). Ohne Backoff — jedes Lock-Ereignis
/// verdient einen Versuch, wie früher das synchrone Backup; ein Fehlschlag
/// setzt aber den Backoff des Nachholens im Tick. `Ok(Some)` = erstellt.
pub async fn requested_backup_now(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    now: DateTime<Utc>,
) -> Result<Option<BackupOutcome>> {
    if !session.is_unlocked()
        || crate::db::repo::app_settings::get(pool, PENDING_KEY)
            .await?
            .is_none()
    {
        return Ok(None);
    }
    let outcome = auto_backup_at(pool, paths, session, "backup.requested", now).await?;
    match &outcome {
        Some(_) => crate::scheduler::backoff::clear(pool, PENDING_BACKOFF_KEY).await?,
        None => crate::scheduler::backoff::record_failure(pool, PENDING_BACKOFF_KEY, now).await?,
    }
    Ok(outcome)
}

/// Holt ein ausstehendes Lock-Backup nach (erster Tick nach Entsperren oder
/// Serverstart), unabhängig vom Tages-Vermerk. Nach einem Fehlschlag
/// frühestens nach [`DAILY_RETRY_AFTER_HOURS`] erneut (persistiert).
/// `Ok(Some)` = erstellt.
pub async fn pending_backup_if_due(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    now: DateTime<Utc>,
) -> Result<Option<BackupOutcome>> {
    if !session.is_unlocked()
        || crate::db::repo::app_settings::get(pool, PENDING_KEY)
            .await?
            .is_none()
    {
        return Ok(None);
    }
    if crate::scheduler::backoff::blocked(
        pool,
        PENDING_BACKOFF_KEY,
        now,
        chrono::Duration::hours(DAILY_RETRY_AFTER_HOURS),
    )
    .await?
    {
        return Ok(None);
    }
    let outcome = auto_backup_at(pool, paths, session, "backup.pending", now).await?;
    match &outcome {
        Some(_) => crate::scheduler::backoff::clear(pool, PENDING_BACKOFF_KEY).await?,
        None => crate::scheduler::backoff::record_failure(pool, PENDING_BACKOFF_KEY, now).await?,
    }
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// Tages-Backup, kalenderfest (Lauf F, D-04)
// ---------------------------------------------------------------------------

/// `app_settings`: Uhrzeit des Tages-Backups (`HH:MM`, Europe/Berlin).
pub const DAILY_TIME_KEY: &str = "backup_daily_time";
/// Standard-Uhrzeit des Tages-Backups (nachts, außerhalb der Arbeitszeit).
pub const DAILY_DEFAULT_TIME: &str = "02:30";
/// `app_settings`: Berliner Datum des zuletzt erstellten Tages-Backups
/// (`YYYY-MM-DD`). Unabhängig von der Historie, die die Rotation ausdünnt.
pub const DAILY_LAST_ON_KEY: &str = "backup_daily_last_on";
/// `app_settings`: letzter Fehlschlag des Tages-Backups (Backoff, UTC).
pub const DAILY_BACKOFF_KEY: &str = "backup_daily_last_failure_at";
/// Wartezeit nach einem fehlgeschlagenen Tages-Backup (Review W2 S2-3).
pub const DAILY_RETRY_AFTER_HOURS: i64 = 1;

/// Liest `HH:MM` (00:00–23:59).
pub fn parse_daily_time(s: &str) -> Option<chrono::NaiveTime> {
    let t = s.trim();
    let (h, m) = t.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    chrono::NaiveTime::from_hms_opt(h.parse().ok()?, m.parse().ok()?, 0)
}

/// Eingestellte Uhrzeit des Tages-Backups (Standard [`DAILY_DEFAULT_TIME`]).
pub async fn daily_time(pool: &SqlitePool) -> Result<chrono::NaiveTime> {
    let stored = crate::db::repo::app_settings::get(pool, DAILY_TIME_KEY).await?;
    Ok(stored
        .as_deref()
        .and_then(parse_daily_time)
        .or_else(|| parse_daily_time(DAILY_DEFAULT_TIME))
        .unwrap_or(chrono::NaiveTime::MIN))
}

/// Zuletzt fälliger Slot (pure, F1d): heute, wenn die eingestellte Uhrzeit
/// erreicht ist, sonst gestern. Am Tag der Zeitumstellung gilt die Wanduhr:
/// eine übersprungene Uhrzeit (29.03. 02:30) ist ab 03:00 erreicht.
pub fn daily_slot(now_berlin: chrono::NaiveDateTime, at: chrono::NaiveTime) -> chrono::NaiveDate {
    let today = now_berlin.date();
    if now_berlin.time() >= at {
        today
    } else {
        today.pred_opt().unwrap_or(today)
    }
}

/// Pure Fälligkeit: genau ein Tages-Backup je Slot (Berliner Kalendertag ab
/// der eingestellten Uhrzeit). Fällig, solange für den zuletzt fälligen Slot
/// ([`daily_slot`]) noch keines entstand — so holt der erste Tick nach einem
/// verpassten Slot (Uhrzeit kurz vor Mitternacht zwischen zwei Ticks, Sperre
/// über Tage, Anmeldung vor der Uhrzeit) genau eines nach (F1d). Eine doppelte
/// Uhrzeit (25.10. 02:30) löst nur einmal aus. Steht das gemerkte Datum nach
/// heute (Uhr zurückgestellt), zählt es nicht als erledigt.
pub fn daily_due(
    now_berlin: chrono::NaiveDateTime,
    last_on: Option<chrono::NaiveDate>,
    at: chrono::NaiveTime,
) -> bool {
    let slot = daily_slot(now_berlin, at);
    match last_on {
        None => true,
        Some(last) => last < slot || last > now_berlin.date(),
    }
}

/// Tages-Backup (Trigger `auto_daily`), wenn für den zuletzt fälligen Slot
/// (Europe/Berlin, eingestellte Uhrzeit) noch keines entstand (D-04, F1d).
/// Gemerkt wird der Slot. Nach einem Fehlschlag frühestens nach
/// [`DAILY_RETRY_AFTER_HOURS`] erneut (persistiert). `Ok(Some)` = erstellt,
/// `Ok(None)` = nicht fällig, gesperrt oder fehlgeschlagen (protokolliert).
pub async fn auto_daily_if_due(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    now: DateTime<Utc>,
) -> Result<Option<BackupOutcome>> {
    if !session.is_unlocked() {
        return Ok(None);
    }
    let now_berlin = now.with_timezone(&chrono_tz::Europe::Berlin);
    let last_on = match crate::db::repo::app_settings::get(pool, DAILY_LAST_ON_KEY).await? {
        Some(v) => chrono::NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").ok(),
        // Bestand vor Lauf F: Datum aus der Historie (jüngstes `auto_daily`).
        None => {
            let ts: Option<String> = sqlx::query_scalar(
                "SELECT MAX(created_at) FROM backup_history WHERE trigger_reason = 'auto_daily'",
            )
            .fetch_one(pool)
            .await?;
            ts.as_deref()
                .and_then(crate::clock::db_utc_to_berlin)
                .map(|b| b.date_naive())
        }
    };
    let at = daily_time(pool).await?;
    if !daily_due(now_berlin.naive_local(), last_on, at) {
        return Ok(None);
    }
    let slot = daily_slot(now_berlin.naive_local(), at);
    if crate::scheduler::backoff::blocked(
        pool,
        DAILY_BACKOFF_KEY,
        now,
        chrono::Duration::hours(DAILY_RETRY_AFTER_HOURS),
    )
    .await?
    {
        return Ok(None);
    }
    let outcome = auto_backup_at(pool, paths, session, "auto_daily", now).await?;
    match &outcome {
        Some(_) => {
            mark_daily_done(pool, slot).await?;
            crate::scheduler::backoff::clear(pool, DAILY_BACKOFF_KEY).await?;
        }
        // Session war entsperrt (oben geprüft) → `None` heißt fehlgeschlagen.
        None => crate::scheduler::backoff::record_failure(pool, DAILY_BACKOFF_KEY, now).await?,
    }
    Ok(outcome)
}

/// Merkt `day` als Tag mit erledigtem Tages-Backup (auch das Erst-Backup der
/// Einrichtung zählt, D-12).
pub async fn mark_daily_done(pool: &SqlitePool, day: chrono::NaiveDate) -> Result<()> {
    crate::db::repo::app_settings::set(pool, DAILY_LAST_ON_KEY, &day.format("%Y-%m-%d").to_string())
        .await
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
    /// Die Liste spiegelt jeden `request_auto_backup(..)`-Aufrufer wider
    /// (Stand G1-HARDEN). Der Catch-all in `db_trigger_reason` macht das
    /// bruchsicher; dieser Test friert das Versprechen ein.
    #[test]
    fn all_real_lock_events_map_to_valid_check_sets() {
        // CHECK-Mengen aus Migration 0001 (backup_history).
        const VALID_TRIGGER: &[&str] = &["auto_daily", "auto_critical", "manual", "pre_restore"];
        const VALID_RETENTION: &[&str] = &["daily", "monthly", "yearly", "manual"];

        // Jedes Event-Label, das im Code an `request_auto_backup`/`create_now`
        // übergeben wird (grep `request_auto_backup`, Stand 2026-10-25).
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
            // Nachholen und geweckter Lauf (F1d, Lauf F3a).
            "backup.pending",
            "backup.requested",
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

    /// KB-0034: Nach der Panik eines Halters der Sitzungs-Sperre liefern
    /// `is_unlocked`/`get`/`set`/`clear` weiter Werte statt zu paniken.
    #[test]
    fn backup_session_survives_poisoned_mutex() {
        let session = std::sync::Arc::new(BackupSession::default());
        session.set("passphrase-1234567890".into());
        let s = session.clone();
        let _ = std::thread::spawn(move || {
            let _guard = s.inner.lock().unwrap();
            panic!("Halter stirbt mit Sperre");
        })
        .join();
        assert!(session.inner.is_poisoned());
        assert!(session.is_unlocked());
        assert_eq!(
            session.get().as_deref().map(String::as_str),
            Some("passphrase-1234567890")
        );
        session.clear();
        assert!(!session.is_unlocked());
        session.set("andere-passphrase-123".into());
        assert_eq!(
            session.get().as_deref().map(String::as_str),
            Some("andere-passphrase-123")
        );
    }

    /// Lauf F3a: Der Vermerk „Backup ausstehend“ wird nur von einem Backup
    /// gelöscht, während dessen kein weiteres Lock-Ereignis vermerkt wurde
    /// (Zähler, unabhängig von der Uhr).
    #[tokio::test]
    async fn pending_marker_survives_events_during_backup() {
        let dir = tempfile::TempDir::new().unwrap();
        let data = dir.path().join("data");
        let paths = Paths {
            data_dir: data.clone(),
            db_file: data.join("klein-buch.sqlite"),
            archive_dir: data.join("archive"),
            backups_dir: data.join("backups"),
            inputs_dir: dir.path().join("inputs"),
            sidecar_dir: dir.path().join("sidecar"),
            resource_dir: std::path::PathBuf::from("/non/existent/resources"),
            web_dir: std::path::PathBuf::from("/non/existent/web"),
            backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
        };
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = crate::db::open_pool(&paths.db_file, None).await.unwrap();
        crate::db::MIGRATOR.run(&pool).await.unwrap();
        let get = |k: &'static str| {
            let pool = pool.clone();
            async move { crate::db::repo::app_settings::get(&pool, k).await.unwrap() }
        };
        // Uhr läuft rückwärts: der Zeitpunkt spielt für die Deckung keine Rolle.
        let later = chrono::TimeZone::with_ymd_and_hms(&Utc, 2027, 1, 1, 0, 0, 0).unwrap();
        let earlier = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 1, 1, 0, 0, 0).unwrap();
        mark_pending(&pool, later).await.unwrap();
        mark_pending(&pool, earlier).await.unwrap();
        assert_eq!(get(PENDING_SEQ_KEY).await.as_deref(), Some("2"));
        assert_eq!(
            get(PENDING_KEY).await,
            Some(crate::clock::to_db_utc(later)),
            "der älteste Vermerk bleibt"
        );
        // Ereignis „während“ des Backups: Zähler ändert sich nach dem Lesen.
        let seq_before = pending_seq(&pool).await;
        mark_pending(&pool, earlier).await.unwrap();
        sqlx::query(
            "DELETE FROM app_settings WHERE key IN (?1, ?2)
               AND COALESCE((SELECT value FROM app_settings WHERE key = ?2), '') = ?3",
        )
        .bind(PENDING_KEY)
        .bind(PENDING_SEQ_KEY)
        .bind(&seq_before)
        .execute(&pool)
        .await
        .unwrap();
        assert!(get(PENDING_KEY).await.is_some(), "Vermerk bleibt stehen");
        // Ein Backup mit der injizierten (früheren) Uhr deckt alles ab.
        create_with(
            &pool,
            &paths,
            "passphrase-1234567890",
            "auto_critical",
            CreateOptions {
                mirror: Mirror::Auto,
                now: earlier,
                format_version: manifest::FORMAT_VERSION,
            },
        )
        .await
        .unwrap();
        assert!(get(PENDING_KEY).await.is_none());
        assert!(get(PENDING_SEQ_KEY).await.is_none());
    }

    #[tokio::test]
    async fn setup_then_verify_roundtrip() {
        let pool = pool_with_settings().await;
        let session = BackupSession::default();
        assert!(!is_passphrase_set(&pool).await.unwrap());

        // R5-005: Passphrase-Floor 16 — Test-Phrasen entsprechend lang.
        setup_passphrase(&pool, &session, "super-secret-roundtrip-1")
            .await
            .unwrap();
        assert!(is_passphrase_set(&pool).await.unwrap());
        assert!(session.is_unlocked());

        // Verifier darf nicht die Klartext-Passphrase enthalten.
        let ct = get_setting(&pool, SETTING_VERIFIER_CT)
            .await
            .unwrap()
            .unwrap();
        assert!(!ct.contains("super-secret"));

        // Richtige und falsche Passphrase gegen den Verifier.
        assert!(verify_passphrase(&pool, "super-secret-roundtrip-1")
            .await
            .unwrap());
        assert!(!verify_passphrase(&pool, "falsch-aber-lang-genug")
            .await
            .unwrap());
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

    /// Plan W2.3: zwei Backups in derselben Sekunde (auch gleichzeitig
    /// angestoßen) ergeben zwei Dateien und zwei `backup_log`-Zeilen.
    #[tokio::test]
    async fn two_backups_in_same_second_give_two_files_and_log_rows() {
        let dir = tempfile::TempDir::new().unwrap();
        let data = dir.path().join("data");
        let paths = Paths {
            data_dir: data.clone(),
            db_file: data.join("klein-buch.sqlite"),
            archive_dir: data.join("archive"),
            backups_dir: data.join("backups"),
            inputs_dir: dir.path().join("inputs"),
            sidecar_dir: dir.path().join("sidecar"),
            resource_dir: std::path::PathBuf::from("/non/existent/resources"),
            web_dir: std::path::PathBuf::from("/non/existent/web"),
            backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
        };
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = crate::db::open_pool(&paths.db_file, None).await.unwrap();
        crate::db::MIGRATOR.run(&pool).await.unwrap();

        let pass = "passphrase-1234567890";
        let (a, b) = tokio::join!(
            create_now(&pool, &paths, pass, "manual"),
            create_now(&pool, &paths, pass, "manual"),
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_ne!(a.file_path, b.file_path);
        let files = std::fs::read_dir(&paths.backups_dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .map(|e| e.file_name().to_string_lossy().ends_with(".kbk"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(files, 2);
        let rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backup_log WHERE target_kind = 'local' AND status = 'ok'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rows, 2);
    }
}
