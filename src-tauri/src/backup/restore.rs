//! Restore-Wizard (Block 4) — verify-then-apply, zweiphasig.
//!
//! Warum zweiphasig? Während der Server läuft, hält der sqlx-Pool die
//! SQLite-Datei (inkl. WAL) offen; ein Tausch der Datei unter einer offenen
//! Verbindung ist nicht sicher. Daher:
//!
//! 1. **Phase A — [`apply`]** (Server läuft, Änderungen gesperrt):
//!    Pre-Restore-Backup erstellen, Backup-Datei entschlüsseln + Hash
//!    verifizieren, Inhalt in ein Staging-Verzeichnis entpacken, die
//!    Nummernkreise der laufenden DB festhalten (D-24), eine **Marker-Datei**
//!    schreiben und einen Neustart anfordern.
//! 2. **Phase B — [`apply_pending`]** (Serverstart, **vor** dem Öffnen des
//!    Pools): Staging-DB über die echte DB legen, Archive ersetzen, Marker
//!    löschen. Das erste Entsperren danach (`db::run_migrations`) schreibt den
//!    Audit, setzt die Pause der automatischen Abläufe (W14-023), hebt die
//!    Nummernkreise an und gleicht die Backup-Historie ab (D-22).
//!
//! Die Marker-Datei liegt im Dateisystem (nicht in der DB!), weil die DB selbst
//! ausgetauscht wird.
//!
//! GoBD/§-Hinweise:
//! - **`inputs/` bleibt tabu** — beim Restore werden nur maschinen-verwaltete
//!   Daten ersetzt (DB + `archive/` + ab Format 2 `data/branding`, also Logo und
//!   Unterschrift). `inputs/` wird **nicht** überschrieben (menschen-gepflegt).
//!   Backups im Format 1 enthalten kein `data/branding`; dann bleibt es
//!   unverändert und der Restore-Report sagt das (Plan W1.5).
//! - **Pre-Restore-Backup ist Pflicht** und nutzt die *aktuelle* Session-
//!   Passphrase, sodass der Vor-Restore-Stand jederzeit wiederherstellbar ist.

use crate::backup::{create_now, encrypt, manifest, snapshot, BackupSession};
use crate::config::Paths;
use crate::db::schema_version::{EXPECTED_SCHEMA_VERSION, MIN_RESTORE_SCHEMA_VERSION};
use crate::error::{Error, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

/// Name der Restore-Marker-Datei im `data_dir`.
pub const RESTORE_MARKER: &str = "RESTORE_PENDING.json";
/// Name des Staging-Verzeichnisses im `data_dir`.
pub const RESTORE_STAGING: &str = "restore-staging";

/// Anzeige-Vorschau eines Backup-Files (ohne Passphrase).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub file_path: String,
    pub created_at: String,
    pub app_version: String,
    pub format_version: u32,
    pub schema_version: i32,
    pub current_schema_version: i32,
    pub compatible: bool,
    pub content_size_bytes: u64,
}

/// Ergebnis von Phase A.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub requires_restart: bool,
    /// Dateiname des Pre-Restore-Backups (im lokalen Backup-Ordner).
    pub pre_restore_backup: String,
    /// Quelle ohne Serverpfad (Upload-Dateiname bzw. Backup aus der Historie).
    pub source: String,
    pub staged_at: String,
    /// Format-Version des eingespielten Backups.
    pub format_version: u32,
    /// Schema-Version des Backups (ältere werden beim Entsperren migriert).
    pub schema_version: i32,
    /// `true`, wenn Logo und Unterschrift (`data/branding`) mit eingespielt werden.
    pub branding_restored: bool,
    /// Hinweis für die Oberfläche (z. B. Format-1-Backup ohne Logo/Unterschrift).
    pub notice: Option<String>,
}

/// Hinweis bei Backups im Format 1 (ohne `data/branding`).
pub const NOTICE_V1_NO_BRANDING: &str = "Dieses Backup stammt aus einer älteren Version \
     (Format 1) und enthält kein Logo und keine Unterschrift. Die aktuell gespeicherten \
     bleiben unverändert.";

/// Liest den Plain-Header eines Backup-Files für die Anzeige im Wizard.
pub fn preview(file_path: &Path) -> Result<RestorePreview> {
    let bytes = fs::read(file_path)?;
    let m = manifest::read_manifest_only(&bytes)?;
    Ok(RestorePreview {
        file_path: file_path.to_string_lossy().to_string(),
        created_at: m.created_at,
        app_version: m.app_version,
        format_version: m.format_version,
        schema_version: m.schema_version,
        current_schema_version: EXPECTED_SCHEMA_VERSION,
        compatible: ensure_restorable_schema(m.schema_version).is_ok(),
        content_size_bytes: m.content_size_bytes,
    })
}

/// Entschlüsselt + verifiziert ein Backup-File mit der gegebenen Passphrase.
/// Liefert (Manifest, Content-ZIP-Bytes). Schlägt fehl bei falscher Passphrase
/// (GCM-Auth) oder bei Content-Hash-Mismatch (Manipulation).
pub fn decrypt_backup(
    file_bytes: &[u8],
    passphrase: &str,
) -> Result<(manifest::Manifest, Vec<u8>)> {
    let (m, ct) = manifest::unframe(file_bytes)?;
    let salt = manifest::from_hex(&m.salt_hex)?;
    let nonce = encrypt::nonce_array(&manifest::from_hex(&m.nonce_hex)?)?;
    let key = encrypt::derive_key(passphrase.as_bytes(), &salt)?;
    let content = encrypt::decrypt(&key, &nonce, &ct)?;
    let actual = snapshot::sha256_hex(&content);
    if actual != m.content_sha256 {
        return Err(Error::Backup(
            "Content-Hash stimmt nicht — Backup beschädigt".into(),
        ));
    }
    Ok((m, content))
}

/// Grenzen für Phase A (Plan W7): Gesamtgröße beim Entpacken und freier Platz.
#[derive(Debug, Clone, Copy)]
pub struct RestoreLimits {
    /// Höchstens so viele Bytes werden entpackt (`KLEIN_BUCH_MAX_RESTORE_BYTES`).
    pub max_bytes: u64,
    /// So viel muss nach dem Entpacken frei bleiben (`KLEIN_BUCH_MIN_FREE_BYTES`).
    pub min_free_bytes: u64,
}

impl Default for RestoreLimits {
    fn default() -> Self {
        let l = crate::uploads::UploadLimits::default();
        Self {
            max_bytes: l.max_restore_bytes,
            min_free_bytes: l.min_free_bytes,
        }
    }
}

/// Wie [`apply`] gescheitert ist: bei falscher Passphrase darf derselbe Upload
/// erneut versucht werden, sonst wird er verworfen (Plan W7).
pub fn is_wrong_passphrase(err: &Error) -> bool {
    matches!(err, Error::Crypto(_))
}

/// Phase A (Plan W7): Manifest und Format prüfen → Schema `30 ≤ s ≤ EXPECTED`
/// → entschlüsseln (Passphrase-Prüfung vor jedem Schreiben) →
/// Pre-Restore-Backup mit der aktuellen Passphrase → Platz prüfen → gehärtet
/// ins Staging entpacken → gestagte DB mit der Backup-Passphrase öffnen und
/// ihre Schema-Version gegen das Manifest prüfen → Audit → Marker. Scheitert
/// ein Schritt nach dem Anlegen des Stagings, wird es gelöscht. `label`
/// beschreibt die Quelle ohne Serverpfad (Audit, Marker, Bericht).
pub async fn apply(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    file_path: &Path,
    label: &str,
    passphrase: &str,
    limits: RestoreLimits,
) -> Result<RestoreReport> {
    let current = session.get().ok_or_else(|| {
        Error::Backup("Vor dem Restore bitte die Instanz entsperren (aktuelle Passphrase).".into())
    })?;
    // Ein schon vorgemerkter Restore würde sonst sein Staging verlieren
    // (Gegenprüfung W7).
    if paths.data_dir.join(RESTORE_MARKER).exists() || restore_incomplete(paths) {
        return Err(Error::Backup(
            "Ein Restore ist bereits vorgemerkt — der Server startet neu.".into(),
        ));
    }

    let file_bytes = fs::read(file_path)?;
    let header = manifest::read_manifest_only(&file_bytes)?;
    manifest::ensure_supported_format(header.format_version)?;
    let branding_restored = header.format_version >= 2;
    ensure_restorable_schema(header.schema_version)?;

    // Entschlüsseln + Content-Hash prüfen — vor dem Pre-Restore-Backup, damit
    // eine falsche Passphrase nichts schreibt. Argon2id/AES außerhalb der
    // Runtime, KDF begrenzt (Plan W2.6).
    let content_zip = {
        let pass = zeroize::Zeroizing::new(passphrase.to_string());
        crate::limits::run_blocking(&crate::limits::KDF, move || {
            decrypt_backup(&file_bytes, &pass).map(|(_m, content)| content)
        })
        .await?
    };

    // Pflicht-Audit (R2-026, Gegenprüfung W7): VOR dem Pre-Restore-Backup, damit
    // der Eintrag in genau dieser Sicherung des ersetzten Stands steht (die
    // alte DB selbst wird beim Tausch verworfen). Scheitert er, passiert nichts.
    crate::db::repo::audit_log::append(
        pool,
        "backup.restore.requested",
        "backup",
        "restore",
        Some(
            &serde_json::json!({
                "source": label,
                "schema": header.schema_version,
                "format": header.format_version,
            })
            .to_string(),
        ),
    )
    .await?;

    // Pre-Restore-Backup (Pflicht) mit der aktuellen Session-Passphrase.
    let pre = create_now(pool, paths, &current, "pre_restore").await?;

    // Platz: entpackte Größe laut Verzeichnis ≤ Limit und ≤ (frei − Reserve)/2
    // — der Tausch beim Neustart kopiert das Staging noch einmal
    // (Gegenprüfung W7).
    let declared = declared_size(&content_zip)?;
    let free = crate::uploads::free_bytes(&paths.data_dir)?;
    let budget = limits
        .max_bytes
        .min(free.saturating_sub(limits.min_free_bytes) / 2);
    if declared > budget {
        return Err(Error::Backup(format!(
            "Backup zu groß für den Restore: {declared} Bytes entpackt, erlaubt {budget} \
             (KLEIN_BUCH_MAX_RESTORE_BYTES / freier Platz)."
        )));
    }

    let staging = paths.data_dir.join(RESTORE_STAGING);
    reset_dir(&staging)?;
    let staged = stage(
        content_zip,
        &staging,
        paths,
        header.format_version,
        header.schema_version,
        passphrase,
        budget,
    )
    .await;
    if let Err(e) = staged {
        remove_staging(&staging).await;
        return Err(e);
    }

    // Nummernkreise der laufenden DB (Lauf F, D-24): nach dem Tausch hebt das
    // erste Entsperren die Zähler der eingespielten DB auf diese Stände an,
    // falls sie höher sind — bereits vergebene Nummern werden nie erneut
    // vergeben. Seit der Annahme des Restores sind Änderungen gesperrt; die
    // Stände bleiben bis zum Tausch gültig.
    let counters = match crate::db::numbering::snapshot(pool).await {
        Ok(c) => c,
        Err(e) => {
            remove_staging(&staging).await;
            return Err(e);
        }
    };
    // F1d: nächste Termine der Abo-Reihen (bereits abgerechnete Perioden
    // werden nach dem Restore nicht erneut abgerechnet) und Nummern
    // festgeschriebener Belege (wiederhergestellte Entwürfe mit diesen Nummern
    // werden vorgemerkt), siehe `backup::after_restore`.
    let series = match crate::backup::after_restore::snapshot_series(pool).await {
        Ok(s) => s,
        Err(e) => {
            remove_staging(&staging).await;
            return Err(e);
        }
    };
    let locked_numbers = match crate::backup::after_restore::snapshot_locked_numbers(pool).await {
        Ok(n) => n,
        Err(e) => {
            remove_staging(&staging).await;
            return Err(e);
        }
    };

    // Marker vorbereiten (Dateisystem, nicht DB). Er wird erst nach dem
    // Pflicht-Audit an seinen Platz gebracht (Review W2 S1-6): scheitert der
    // Audit, meldet der Restore einen Fehler und wird beim Neustart auch NICHT
    // angewendet.
    let staged_at = crate::clock::now_utc()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let marker = paths.data_dir.join(RESTORE_MARKER);
    let marker_tmp = paths.data_dir.join(format!("{RESTORE_MARKER}.tmp"));
    let info = serde_json::json!({
        "staging": staging.to_string_lossy(),
        "source": label,
        "staged_at": staged_at,
        "pre_restore_backup": pre.file_name,
        "format_version": header.format_version,
        "schema_version": header.schema_version,
        "backup_created_at": header.created_at,
        "counters": counters,
        "series": series,
        "locked_numbers": locked_numbers,
    });
    let written = (|| -> Result<()> {
        use std::io::Write;
        let mut f = fs::File::create(&marker_tmp)?;
        f.write_all(&serde_json::to_vec_pretty(&info)?)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&marker_tmp);
        remove_staging(&staging).await;
        return Err(e);
    }

    // Der Pflicht-Audit steht schon vor dem Pre-Restore-Backup; nach dem
    // Tausch schreibt das erste Entsperren `backup.restore.applied` in die
    // wiederhergestellte DB (aus RESTORE_APPLIED.json, übersteht Neustarts).
    if let Err(e) = fs::rename(&marker_tmp, &marker) {
        let _ = fs::remove_file(&marker_tmp);
        remove_staging(&staging).await;
        return Err(e.into());
    }

    Ok(RestoreReport {
        requires_restart: true,
        pre_restore_backup: pre.file_name,
        source: label.to_string(),
        staged_at,
        format_version: header.format_version,
        schema_version: header.schema_version,
        branding_restored,
        notice: (!branding_restored).then(|| NOTICE_V1_NO_BRANDING.to_string()),
    })
}

/// Schema-Version eines Backups im zulässigen Bereich (E15)?
pub fn ensure_restorable_schema(schema: i32) -> Result<()> {
    if schema > EXPECTED_SCHEMA_VERSION {
        return Err(Error::Backup(format!(
            "Backup hat Schema-Version {schema}, diese Version kennt höchstens \
             {EXPECTED_SCHEMA_VERSION}. Restore abgebrochen — bitte zuerst Klein.Buch aktualisieren."
        )));
    }
    if schema < MIN_RESTORE_SCHEMA_VERSION {
        return Err(Error::Backup(format!(
            "Backup hat Schema-Version {schema}; eingespielt werden Backups ab Version \
             {MIN_RESTORE_SCHEMA_VERSION} (Klein.Buch v2026.5). Restore abgebrochen."
        )));
    }
    Ok(())
}

/// Summe der entpackten Größen laut ZIP-Verzeichnis (Vorabprüfung; das
/// Entpacken zählt die tatsächlich gelesenen Bytes).
fn declared_size(zip_bytes: &[u8]) -> Result<u64> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        total = total.saturating_add(archive.by_index_raw(i)?.size());
    }
    Ok(total)
}

/// Angewendete Migrationen der gestagten DB müssen mit denen dieser Version
/// übereinstimmen (Version + Prüfsumme), sonst scheitert das Entsperren nach
/// dem Tausch mit `VersionMismatch` (Gegenprüfung W7).
async fn check_migration_checksums(pool: &SqlitePool) -> Result<()> {
    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations WHERE success = 1")
            .fetch_all(pool)
            .await?;
    for (version, checksum) in applied {
        let known = crate::db::MIGRATOR
            .migrations
            .iter()
            .find(|m| m.version == version);
        match known {
            Some(m) if m.checksum.as_ref() == checksum.as_slice() => {}
            _ => {
                return Err(Error::Backup(format!(
                    "Migration {version} im Backup passt nicht zu dieser Version \
                     (Prüfsumme) — Restore abgebrochen."
                )))
            }
        }
    }
    Ok(())
}

/// Staging nach einem Fehler in Phase A entfernen. Ein eben geschlossener
/// SQLite-Pool kann `-wal`/`-shm` noch kurz selbst löschen bzw. halten
/// (CI-Fund W8) — daher einige Versuche.
async fn remove_staging(staging: &Path) {
    for _ in 0..20 {
        let _ = clear_dir_force(staging);
        let _ = fs::remove_dir(staging);
        if !staging.exists() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    tracing::warn!(
        "Restore-Staging ließ sich nicht vollständig entfernen (wird beim nächsten Serverstart aufgeräumt)"
    );
}

/// Entpacken + gestagte DB prüfen (blockierende Teile außerhalb der Runtime).
async fn stage(
    content_zip: Vec<u8>,
    staging: &Path,
    paths: &Paths,
    format_version: u32,
    schema_version: i32,
    passphrase: &str,
    budget: u64,
) -> Result<()> {
    {
        let staging = staging.to_path_buf();
        let limits = ExtractLimits {
            max_total: budget,
            ..ExtractLimits::default()
        };
        tokio::task::spawn_blocking(move || {
            extract_zip(&content_zip, &staging, format_version, &limits)
        })
        .await
        .map_err(|e| Error::Backup(format!("Entpacken abgebrochen: {e}")))??;
    }
    let staged_db = staging.join("klein-buch.sqlite");
    if !staged_db.is_file() {
        return Err(Error::Backup(
            "Backup enthält keine klein-buch.sqlite — Restore abgebrochen".into(),
        ));
    }
    if crate::db::db_file_is_plaintext(&staged_db) {
        return Err(Error::Backup(
            "Backup enthält eine unverschlüsselte Datenbank — wird nicht eingespielt \
             (ADR 0035/0039)."
                .into(),
        ));
    }
    // Gestagte DB mit der Backup-Passphrase (= deren DB-Key) öffnen und prüfen.
    let pool = crate::db::open_pool(&staged_db, Some(passphrase)).await?;
    let checked = async {
        if !crate::db::probe_readable(&pool).await {
            return Err(Error::Backup(
                "Die Datenbank im Backup lässt sich mit der Backup-Passphrase nicht lesen.".into(),
            ));
        }
        let found = crate::db::schema_version::read(&pool)
            .await?
            .ok_or_else(|| {
                Error::Backup("Die Datenbank im Backup hat keine Schema-Version.".into())
            })?;
        if found != schema_version {
            return Err(Error::Backup(format!(
                "Schema-Version der Datenbank im Backup ({found}) passt nicht zum \
                 Manifest ({schema_version}) — Restore abgebrochen."
            )));
        }
        ensure_restorable_schema(found)?;
        // Gegenprüfung W7: Was das Entsperren später prüft, schon hier — sonst
        // wäre die Instanz nach dem Tausch dauerhaft gesperrt.
        crate::server::startup::check_archive_paths(&pool, paths)
            .await
            .map_err(|_| {
                Error::Backup(format!(
                    "Die Archivpfade im Backup passen nicht zu {} (Backup einer Instanz mit \
                 anderem Datenverzeichnis oder der Desktop-Fassung) — Restore abgebrochen.",
                    paths.archive_dir.display()
                ))
            })?;
        check_migration_checksums(&pool).await
    }
    .await;
    pool.close().await;
    checked
}

/// Nach dem Restore (Plan W7): Werden PDF-Vorlagen verwendet, die nicht in
/// `/inputs` liegen (die Vorlagen sind nicht Teil des Backups)? Geprüft werden
/// die Standard-Vorlage im Verkäuferprofil, offene Rechnungs- und
/// Angebotsentwürfe und aktive Abo-Rechnungen. Fehlende → Inbox-Warnung mit
/// Namen. Liefert die fehlenden Namen (best effort, Fehler nur im Log).
pub async fn warn_missing_templates(
    paths: &Paths,
    pool: &SqlitePool,
    staged_at: &str,
) -> Vec<String> {
    let available: std::collections::BTreeSet<String> =
        match crate::pdf::templates::list_templates(&paths.inputs_dir) {
            Ok(list) => list.into_iter().map(|t| t.name).collect(),
            Err(e) => {
                tracing::warn!("Vorlagen nach dem Restore nicht prüfbar: {e}");
                return Vec::new();
            }
        };
    let referenced: Vec<String> = match sqlx::query_scalar(
        "SELECT default_pdf_template FROM seller_profile
         UNION SELECT pdf_template FROM invoices WHERE locked_at IS NULL
         UNION SELECT pdf_template FROM quotes WHERE locked_at IS NULL
         UNION SELECT pdf_template FROM recurring_invoices WHERE active = 1",
    )
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Vorlagen nach dem Restore nicht prüfbar: {e}");
            return Vec::new();
        }
    };
    let missing: Vec<String> = referenced
        .into_iter()
        .filter(|n| !n.trim().is_empty() && !available.contains(n))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if !missing.is_empty() {
        let names = missing.join(", ");
        tracing::warn!("Nach dem Restore fehlen PDF-Vorlagen in /inputs: {names}");
        let body = format!(
            "Diese PDF-Vorlagen werden verwendet, liegen aber nicht im Vorlagen-Ordner \
             (/inputs/pdf-templates): {names}. Die Vorlagen sind nicht Teil des Backups — \
             bitte wieder dort ablegen, sonst lassen sich betroffene Belege nicht erzeugen."
        );
        crate::notify::store::create(
            pool,
            crate::notify::NewNotification {
                title: "PDF-Vorlagen nach dem Restore fehlen",
                body: &body,
                severity: "warning",
                action_url: Some("/settings/pdf-templates"),
                dedup_key: Some(&format!("restore_missing_templates:{staged_at}")),
                ..Default::default()
            },
        )
        .await
        .ok();
    }
    missing
}

/// Info über einen angewandten Restore (für nachträgliches Audit nach Pool-Open).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AppliedInfo {
    pub source: String,
    pub staged_at: String,
    /// `true`, wenn `data/branding` ersetzt wurde (Backup-Format ≥ 2).
    pub branding_restored: bool,
    /// Dateiname des Pre-Restore-Backups im lokalen Backup-Ordner.
    #[serde(default)]
    pub pre_restore_backup: String,
    #[serde(default)]
    pub schema_version: Option<i32>,
    /// Erstellungszeit des eingespielten Backups (Manifest, UTC ISO-8601).
    #[serde(default)]
    pub backup_created_at: Option<String>,
    /// Nummernkreise der ersetzten DB (Lauf F, D-24); leer bei Markern aus
    /// älteren Versionen und bei einer frisch eingerichteten Instanz.
    #[serde(default)]
    pub counters: Vec<crate::db::numbering::CounterState>,
    /// Nächste Termine der Abo-Reihen der ersetzten DB (F1d); leer bei einer
    /// frisch eingerichteten Instanz.
    #[serde(default)]
    pub series: Vec<crate::backup::after_restore::SeriesState>,
    /// Nummern der festgeschriebenen Rechnungen, Stornos und Angebote der
    /// ersetzten DB (F1d).
    #[serde(default)]
    pub locked_numbers: Vec<String>,
}

/// Datei im `data_dir`: angewendeter Restore, dessen Audit noch aussteht
/// (Gegenprüfung W7 — übersteht Neustarts bis zum ersten Entsperren).
pub const RESTORE_APPLIED: &str = "RESTORE_APPLIED.json";

/// Liest einen angewendeten, noch nicht auditierten Restore.
pub fn read_applied(paths: &Paths) -> Option<AppliedInfo> {
    let bytes = fs::read(paths.data_dir.join(RESTORE_APPLIED)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Staging und Marker-Temp eines abgebrochenen Phase-A-Laufs entfernen —
/// nur, wenn weder ein Marker noch ein Tausch-Zustand existiert.
pub fn clean_stale_staging(paths: &Paths) {
    if paths.data_dir.join(RESTORE_MARKER).exists() || swap_state_path(paths).exists() {
        return;
    }
    let staging = paths.data_dir.join(RESTORE_STAGING);
    if staging.exists() {
        let _ = clear_dir_force(&staging);
        let _ = fs::remove_dir(&staging);
    }
    let _ = fs::remove_file(paths.data_dir.join(format!("{RESTORE_MARKER}.tmp")));
}

/// Nach dem geschriebenen Audit: Datei entfernen.
pub fn clear_applied(paths: &Paths) {
    let _ = fs::remove_file(paths.data_dir.join(RESTORE_APPLIED));
}

/// Suffix für die zur-Seite-bewegten Originale beim Phase-B-Swap (R4-001).
const DB_ROLLBACK_SUFFIX: &str = ".rollback";
/// Verzeichnis-Name für das zur-Seite-bewegte alte Archive.
const ARCHIVE_ROLLBACK_DIR: &str = "archive.rollback";
/// Verzeichnis-Name für das zur-Seite-bewegte alte `data/branding` (Plan W1.5).
const BRANDING_ROLLBACK_DIR: &str = "branding.rollback";

/// Zustandsdatei des Phase-B-Tauschs (Review W2 S0-2). Sie sagt
/// [`recover_pending_rollback`] nach einem Abbruch eindeutig, ob die Originale
/// zurückgeholt oder nur noch aufgeräumt werden müssen — statt das aus der
/// Existenz von Ordnern abzuleiten (ein beim Serverstart neu angelegtes leeres
/// `archive/` sah sonst wie „Tausch fertig“ aus und das echte Archiv in
/// `archive.rollback` wurde gelöscht).
pub const RESTORE_SWAP_STATE: &str = "RESTORE_SWAP_STATE.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapPhase {
    /// Originale werden gerade beiseitegelegt.
    Moving,
    /// Originale liegen unter `.rollback`, die Kopie aus dem Staging läuft.
    Moved,
    /// Kopie vollständig; nur Aufräumen steht aus.
    Copied,
}

impl SwapPhase {
    fn as_str(self) -> &'static str {
        match self {
            SwapPhase::Moving => "moving",
            SwapPhase::Moved => "moved",
            SwapPhase::Copied => "copied",
        }
    }
}

fn swap_state_path(paths: &Paths) -> PathBuf {
    paths.data_dir.join(RESTORE_SWAP_STATE)
}

fn write_swap_state(paths: &Paths, phase: SwapPhase) -> Result<()> {
    use std::io::Write;
    let target = swap_state_path(paths);
    let tmp = paths.data_dir.join(format!("{RESTORE_SWAP_STATE}.tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(
            serde_json::json!({ "phase": phase.as_str() })
                .to_string()
                .as_bytes(),
        )?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &target)?;
    sync_dir(&paths.data_dir);
    Ok(())
}

fn read_swap_state(paths: &Paths) -> Option<SwapPhase> {
    let raw = fs::read(swap_state_path(paths)).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    match v.get("phase")?.as_str()? {
        "moving" => Some(SwapPhase::Moving),
        "moved" => Some(SwapPhase::Moved),
        "copied" => Some(SwapPhase::Copied),
        _ => None,
    }
}

fn clear_swap_state(paths: &Paths) {
    let _ = fs::remove_file(swap_state_path(paths));
    sync_dir(&paths.data_dir);
}

fn sync_dir(dir: &Path) {
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

/// Test-Hook: lässt den Tausch gezielt scheitern (nur `cfg(test)`).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjectAt {
    /// Stufe 1 beim n-ten Paar (0 = DB, 1 = Archiv, 2 = Branding).
    Stage1(usize),
    /// Stufe 2 nach dem Kopieren der DB.
    Stage2,
}

#[cfg(test)]
thread_local! {
    static INJECT: std::cell::Cell<Option<InjectAt>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn injected(at: InjectAt) -> Result<()> {
    if INJECT.with(|i| i.get()) == Some(at) {
        return Err(std::io::Error::other("injizierter Fehler (Test)").into());
    }
    Ok(())
}

#[cfg(not(test))]
fn injected_stage1(_index: usize) -> Result<()> {
    Ok(())
}
#[cfg(test)]
fn injected_stage1(index: usize) -> Result<()> {
    injected(InjectAt::Stage1(index))
}
#[cfg(not(test))]
fn injected_stage2() -> Result<()> {
    Ok(())
}
#[cfg(test)]
fn injected_stage2() -> Result<()> {
    injected(InjectAt::Stage2)
}

/// Paare (Original, beiseitegelegte Fassung, ist Verzeichnis) des Tauschs.
fn swap_pairs(paths: &Paths, with_branding: bool) -> Vec<(PathBuf, PathBuf, bool)> {
    let mut v = vec![
        (
            paths.db_file.clone(),
            with_suffix(&paths.db_file, DB_ROLLBACK_SUFFIX),
            false,
        ),
        (
            paths.archive_dir.clone(),
            paths.data_dir.join(ARCHIVE_ROLLBACK_DIR),
            true,
        ),
    ];
    if with_branding {
        v.push((
            crate::branding::branding_dir(&paths.data_dir),
            paths.data_dir.join(BRANDING_ROLLBACK_DIR),
            true,
        ));
    }
    v
}

fn remove_path(path: &Path, is_dir: bool) -> Result<()> {
    if is_dir {
        clear_dir_force(path)?;
        if path.exists() {
            fs::remove_dir(path)?;
        }
    } else {
        remove_if_exists(path)?;
    }
    Ok(())
}

/// Legt `path` unter `<path>.verworfen-<uuid>` beiseite (nichts wird gelöscht).
fn quarantine(path: &Path) -> Result<PathBuf> {
    let target = with_suffix(
        path,
        &format!(".verworfen-{}", uuid::Uuid::now_v7().simple()),
    );
    fs::rename(path, &target)?;
    tracing::warn!(
        "Restore-Recovery: {} beiseitegelegt nach {}",
        path.display(),
        target.display()
    );
    Ok(target)
}

/// `true`, solange ein Restore vorgemerkt ist oder ein Tausch nicht sauber
/// abgeschlossen wurde. Dann darf weder Onboarding noch Entsperren laufen.
pub fn restore_incomplete(paths: &Paths) -> bool {
    paths.data_dir.join(RESTORE_MARKER).exists()
        || swap_state_path(paths).exists()
        || swap_pairs(paths, true).iter().any(|(_, rb, _)| rb.exists())
}

/// **R4-001 / Review W2 S0-2:** Heilt die Reste eines unterbrochenen Tauschs.
///
/// Wird zu Beginn von `apply_pending` aufgerufen — und damit **vor** jedem
/// Pool-Open und **vor** dem Anlegen von `archive/` in
/// `db::prepare_filesystem`. Maßgeblich ist die Zustandsdatei
/// [`RESTORE_SWAP_STATE`]:
///
/// - `copied`: Kopie war vollständig → nur die `.rollback`-Fassungen löschen.
/// - `moving`/`moved`: Tausch unvollständig → Teilkopien (stammen aus dem noch
///   vorhandenen Staging) entfernen, Originale aus `.rollback` zurückholen.
///   Der Marker bleibt; `apply_pending` startet den Tausch neu.
/// - keine Zustandsdatei (älterer Stand): nichts löschen. Fehlt das Original,
///   kommt die `.rollback`-Fassung zurück; sind beide da, wird die
///   `.rollback`-Fassung beiseitegelegt (`.verworfen-<uuid>`).
///
/// `pub`, weil `tests/r4_review_test.rs` die Funktion direkt aufruft.
pub fn recover_pending_rollback(paths: &Paths) -> Result<()> {
    let pairs = swap_pairs(paths, true);
    match read_swap_state(paths) {
        Some(SwapPhase::Copied) => {
            for (_, rb, is_dir) in &pairs {
                if rb.exists() {
                    remove_path(rb, *is_dir)?;
                }
            }
            clear_swap_state(paths);
        }
        Some(SwapPhase::Moving) | Some(SwapPhase::Moved) => {
            for (orig, rb, is_dir) in &pairs {
                if rb.exists() {
                    if orig.exists() {
                        remove_path(orig, *is_dir)?;
                    }
                    fs::rename(rb, orig)?;
                }
            }
            clear_swap_state(paths);
        }
        None => {
            for (orig, rb, _) in &pairs {
                if rb.exists() {
                    if orig.exists() {
                        quarantine(rb)?;
                    } else {
                        if let Some(parent) = orig.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::rename(rb, orig)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Phase B: wird beim Serverstart **vor** dem Öffnen des Pools aufgerufen. Wenn ein
/// Marker existiert, wird die Staging-DB über die echte DB gelegt und das Archive
/// (ab Backup-Format 2 auch `data/branding`) ersetzt. `inputs/` bleibt
/// unangetastet (Hardline).
///
/// Ablauf (R4-001, Review W2 S0-2): Zustand `moving` → Originale per `rename`
/// beiseite (scheitert ein Schritt, werden die schon verschobenen sofort
/// zurückgeholt) → Zustand `moved` → Kopie aus dem Staging mit `fsync`, Archiv
/// wieder write-once → Zustand `copied` → `.rollback`, Marker, Staging und
/// Zustand löschen. Scheitert die Kopie, kommen die Originale zurück; gelingt
/// das nicht vollständig, bleibt der Zustand `moved` für den nächsten Start.
pub fn apply_pending(paths: &Paths) -> Result<Option<AppliedInfo>> {
    recover_pending_rollback(paths)?;

    let marker = paths.data_dir.join(RESTORE_MARKER);
    if !marker.exists() {
        return Ok(None);
    }

    let info: serde_json::Value = serde_json::from_slice(&fs::read(&marker)?)?;
    let staging = PathBuf::from(
        info.get("staging")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Backup("Restore-Marker ohne staging-Pfad".into()))?,
    );
    let source = info
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let staged_at = info
        .get("staged_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // Marker ohne Angabe stammen aus Format 1 (vor Plan W1.5).
    let format_version = info
        .get("format_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let restore_branding = format_version >= 2;
    let pre_restore_backup = info
        .get("pre_restore_backup")
        .and_then(|v| v.as_str())
        .map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let schema_version = info
        .get("schema_version")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);
    let backup_created_at = info
        .get("backup_created_at")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let counters: Vec<crate::db::numbering::CounterState> = info
        .get("counters")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let series: Vec<crate::backup::after_restore::SeriesState> = info
        .get("series")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let locked_numbers: Vec<String> = info
        .get("locked_numbers")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let staged_db = staging.join("klein-buch.sqlite");
    if !staged_db.is_file() {
        return Err(Error::Backup(
            "Restore-Staging unvollständig (keine DB) — Marker bleibt für Diagnose".into(),
        ));
    }

    let pairs = swap_pairs(paths, restore_branding);

    // Stufe 1 — Originale beiseite. WAL/SHM gehören zur alten DB; ohne sie ist
    // die `.rollback`-DB bei einem Rückfall ein sauberer (ggf. älterer) Stand.
    write_swap_state(paths, SwapPhase::Moving)?;
    remove_if_exists(&with_suffix(&paths.db_file, "-wal"))?;
    remove_if_exists(&with_suffix(&paths.db_file, "-shm"))?;
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (index, (orig, rb, is_dir)) in pairs.iter().enumerate() {
        if !orig.exists() {
            continue;
        }
        let step = (|| -> Result<()> {
            injected_stage1(index)?;
            if rb.exists() {
                remove_path(rb, *is_dir)?;
            }
            fs::rename(orig, rb)?;
            Ok(())
        })();
        if let Err(e) = step {
            let mut undone = true;
            for (o, r) in moved.iter().rev() {
                if fs::rename(r, o).is_err() {
                    undone = false;
                }
            }
            if undone {
                clear_swap_state(paths);
            }
            return Err(e);
        }
        moved.push((orig.clone(), rb.clone()));
    }
    write_swap_state(paths, SwapPhase::Moved)?;

    // Stufe 2 — Kopie aus dem Staging.
    let swap_result: Result<()> = (|| {
        if let Some(parent) = paths.db_file.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_file_synced(&staged_db, &paths.db_file)?;
        injected_stage2()?;

        let staged_archive = staging.join("archive");
        if staged_archive.is_dir() {
            fs::create_dir_all(&paths.archive_dir)?;
            copy_dir_recursive(&staged_archive, &paths.archive_dir)?;
            // Plan W2.7: `fs::copy` legt beschreibbare Dateien an — das Archiv
            // muss danach wieder write-once (`0o400`) sein. Ein Fehler hier löst
            // denselben Rollback aus wie jeder Kopierfehler.
            set_tree_readonly(&paths.archive_dir)?;
        }
        // Format ≥ 2: `data/branding` exakt auf den Backup-Stand (auch leer).
        if restore_branding {
            let branding = crate::branding::branding_dir(&paths.data_dir);
            fs::create_dir_all(&branding)?;
            let staged_branding = staging.join("branding");
            if staged_branding.is_dir() {
                copy_dir_recursive(&staged_branding, &branding)?;
            }
        }
        sync_dir(&paths.data_dir);
        Ok(())
    })();

    if let Err(e) = swap_result {
        // Rückfall: Teilkopien entfernen, Originale zurückholen. Marker +
        // Staging bleiben für einen späteren Neuversuch.
        let mut restored = true;
        for (orig, rb, is_dir) in &pairs {
            if orig.exists() && remove_path(orig, *is_dir).is_err() {
                restored = false;
            }
            if rb.exists() && fs::rename(rb, orig).is_err() {
                restored = false;
            }
        }
        if restored {
            clear_swap_state(paths);
        }
        return Err(e);
    }
    write_swap_state(paths, SwapPhase::Copied)?;

    // Angewendet: Angaben für den Audit dauerhaft ablegen, bevor der Marker
    // verschwindet (Gegenprüfung W7, übersteht Neustarts).
    let applied = AppliedInfo {
        source,
        staged_at,
        branding_restored: restore_branding,
        pre_restore_backup,
        schema_version,
        backup_created_at,
        counters,
        series,
        locked_numbers,
    };
    {
        use std::io::Write;
        let tmp = paths.data_dir.join(format!("{RESTORE_APPLIED}.tmp"));
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(&applied)?)?;
        f.sync_all()?;
        fs::rename(&tmp, paths.data_dir.join(RESTORE_APPLIED))?;
    }

    // Stufe 3 — Aufräumen.
    for (_, rb, is_dir) in &pairs {
        if rb.exists() {
            let _ = remove_path(rb, *is_dir);
        }
    }
    let _ = fs::remove_file(&marker);
    let _ = clear_dir_force(&staging);
    let _ = fs::remove_dir(&staging);
    clear_swap_state(paths);

    Ok(Some(applied))
}

// ---------------------------------------------------------------------------
// Datei-Helfer
// ---------------------------------------------------------------------------

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        make_writable(path)?;
        fs::remove_file(path)?;
    }
    Ok(())
}

fn make_writable(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(path, perms);
    }
    Ok(())
}

/// Leert ein Verzeichnis rekursiv, inkl. read-only-Dateien (Archive-Files sind
/// `0o400`/read-only). Entfernt das Verzeichnis selbst nicht.
///
/// `pub(crate)`, weil der Factory Reset (G1-RESET, ADR 0036) denselben
/// force-rekursiven Lösch-Mechanismus für den `data_dir`-Nuke nutzt.
pub(crate) fn clear_dir_force(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut stack = vec![dir.to_path_buf()];
    let mut dirs_to_remove = Vec::new();
    while let Some(d) = stack.pop() {
        let entries = match fs::read_dir(&d) {
            Ok(it) => it,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p.clone());
                dirs_to_remove.push(p);
            } else {
                make_writable(&p)?;
                // Eine Datei kann zwischen Auflisten und Löschen verschwinden
                // (z. B. `-wal`/`-shm` eines eben geschlossenen SQLite-Pools).
                match fs::remove_file(&p) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    // Tiefste zuerst entfernen.
    dirs_to_remove.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs_to_remove {
        let _ = fs::remove_dir(&d);
    }
    Ok(())
}

fn reset_dir(dir: &Path) -> Result<()> {
    clear_dir_force(dir)?;
    let _ = fs::remove_dir(dir);
    fs::create_dir_all(dir)?;
    Ok(())
}

/// Setzt jede Datei unter `dir` schreibgeschützt (`0o400` unter Unix), mit
/// derselben Funktion wie beim Archivieren ([`crate::archive::store`]).
fn set_tree_readonly(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            set_tree_readonly(&path)?;
        } else {
            crate::archive::store::set_readonly(&path)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            fs::create_dir_all(&dst)?;
            copy_dir_recursive(&src, &dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            copy_file_synced(&src, &dst)?;
        }
    }
    Ok(())
}

/// `fs::copy` + `fsync` der Zieldatei (Review W2 S2-6): erst danach werden die
/// `.rollback`-Fassungen gelöscht — ein Stromausfall darf keine halbe DB ohne
/// Rückfallkopie hinterlassen.
fn copy_file_synced(src: &Path, dst: &Path) -> Result<()> {
    fs::copy(src, dst)?;
    fs::OpenOptions::new().read(true).open(dst)?.sync_all()?;
    Ok(())
}

/// Grenzen beim Entpacken eines Content-ZIP (Plan W7, Zip-Bomben-Schutz).
#[derive(Debug, Clone, Copy)]
pub struct ExtractLimits {
    /// Höchstens so viele Bytes insgesamt.
    pub max_total: u64,
    /// Höchstens so viele Einträge.
    pub max_entries: usize,
    /// Höchstens dieses Verhältnis entpackt : gepackt je Eintrag …
    pub max_ratio: u64,
    /// … plus dieser Sockel (kleine, gut komprimierbare Dateien).
    pub ratio_slack: u64,
}

impl Default for ExtractLimits {
    fn default() -> Self {
        Self {
            max_total: crate::uploads::DEFAULT_MAX_RESTORE_BYTES,
            max_entries: 200_000,
            max_ratio: 200,
            ratio_slack: 1024 * 1024,
        }
    }
}

/// Erlaubte Einträge: `klein-buch.sqlite`, `archive/**`, `branding/**`; im
/// Format 1 zusätzlich `inputs/branding/**` (wird übersprungen, Logo und
/// Unterschrift bleiben dann unverändert). `Some(true)` = entpacken,
/// `Some(false)` = überspringen, `None` = abbrechen.
fn entry_action(rel: &Path, is_dir: bool, format_version: u32) -> Option<bool> {
    let parts: Vec<&std::ffi::OsStr> = rel.iter().collect();
    match parts.first().and_then(|p| p.to_str()) {
        Some("klein-buch.sqlite") if parts.len() == 1 && !is_dir => Some(true),
        Some("archive") | Some("branding") => Some(true),
        Some("inputs")
            if format_version < 2 && parts.get(1).and_then(|p| p.to_str()) == Some("branding") =>
        {
            Some(false)
        }
        Some("inputs") if format_version < 2 && parts.len() == 1 && is_dir => Some(false),
        _ => None,
    }
}

/// Entpackt einen Content-ZIP gehärtet nach `dest` (Plan W7): Pfade nur über
/// `enclosed_name()` (kein `..`, kein absoluter Pfad), nur erlaubte Namen
/// ([`entry_action`]), keine Symlinks, keine doppelten Dateien, höchstens
/// `max_entries` Einträge, gelesene Bytes per `take` begrenzt (gesamt und je
/// Eintrag nach Kompressionsverhältnis). Liefert die geschriebenen Bytes.
pub fn extract_zip(
    zip_bytes: &[u8],
    dest: &Path,
    format_version: u32,
    limits: &ExtractLimits,
) -> Result<u64> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
    if archive.len() > limits.max_entries {
        return Err(Error::Backup(format!(
            "Backup-ZIP hat {} Einträge (höchstens {}) — Restore abgebrochen",
            archive.len(),
            limits.max_entries
        )));
    }
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if file.is_symlink() {
            return Err(Error::Backup(format!(
                "Symlink im Backup-ZIP: {name} — Restore abgebrochen"
            )));
        }
        // Review W2 S1-5 / Plan W7: Komponenten prüfen statt Zeichenfolgen —
        // „Angebot v2..pdf“ ist legitim, `../x`, `/x` oder `C:\x` nicht.
        // `enclosed_name()` lässt `a/../b` zu (bleibt im Ziel, unterläuft aber
        // die Allowlist) — daher nur normale Bestandteile.
        let rel = file
            .enclosed_name()
            .filter(|p| {
                p.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            })
            .filter(|_| !name.starts_with('\\') && !name.contains(':'))
            .ok_or_else(|| Error::Backup(format!("unsicherer Pfad im Backup-ZIP: {name}")))?;
        let is_dir = file.is_dir();
        match entry_action(&rel, is_dir, format_version) {
            Some(true) => {}
            Some(false) => continue,
            None => {
                return Err(Error::Backup(format!(
                    "unerwarteter Eintrag im Backup-ZIP: {name} — Restore abgebrochen"
                )))
            }
        }
        let out_path = dest.join(&rel);
        if is_dir {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let by_ratio = file
            .compressed_size()
            .saturating_mul(limits.max_ratio)
            .saturating_add(limits.ratio_slack);
        let remaining = limits.max_total.saturating_sub(total);
        let cap = by_ratio.min(remaining);
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out_path)
            .map_err(|e| Error::Backup(format!("Eintrag {name} nicht anlegbar (doppelt?): {e}")))?;
        let written = std::io::copy(&mut (&mut file).take(cap.saturating_add(1)), &mut out)?;
        if written > cap {
            drop(out);
            let _ = fs::remove_file(&out_path);
            return Err(Error::Backup(if cap == remaining {
                format!(
                    "Backup entpackt größer als erlaubt ({} Bytes) — Restore abgebrochen",
                    limits.max_total
                )
            } else {
                format!(
                    "Eintrag {name} entpackt sich über das Verhältnis {}:1 — Restore abgebrochen",
                    limits.max_ratio
                )
            }));
        }
        total = total.saturating_add(written);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup;
    use tempfile::TempDir;

    fn paths_for(dir: &Path) -> Paths {
        Paths {
            data_dir: dir.to_path_buf(),
            db_file: dir.join("klein-buch.sqlite"),
            archive_dir: dir.join("archive"),
            backups_dir: dir.join("backups"),
            inputs_dir: dir.join("inputs"),
            sidecar_dir: dir.join("sidecar"),
            resource_dir: std::path::PathBuf::from("/non/existent/resources"),
            web_dir: std::path::PathBuf::from("/non/existent/web"),
            backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
        }
    }

    /// Grenzen für Tests (kleines `/tmp`).
    const TEST_LIMITS: RestoreLimits = RestoreLimits {
        max_bytes: 1 << 30,
        min_free_bytes: 0,
    };

    /// Test-DB wie im Betrieb verschlüsselt (SQLCipher, Key = Passphrase).
    async fn pool_for_backup(db_file: &Path) -> SqlitePool {
        let pool = crate::db::open_pool(db_file, Some("passphrase-1234567890"))
            .await
            .unwrap();
        for ddl in [
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
            "INSERT INTO app_settings (key, value) VALUES ('schema_version', '30')",
            "CREATE TABLE audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system', action TEXT NOT NULL,
                entity_type TEXT, entity_id TEXT, details_json TEXT) STRICT",
            "CREATE TABLE backup_history (id TEXT PRIMARY KEY NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now','utc')),
                target_path TEXT NOT NULL, file_hash_sha256 TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL, is_encrypted INTEGER NOT NULL DEFAULT 1,
                retention_tag TEXT NOT NULL, trigger_reason TEXT NOT NULL,
                db_schema_version INTEGER NOT NULL, app_version TEXT NOT NULL,
                verified_at TEXT) STRICT",
            "CREATE TABLE marker (id INTEGER PRIMARY KEY, v TEXT) STRICT",
            "INSERT INTO marker (v) VALUES ('original-data')",
            // Nummernkreise (Phase A hält sie für das Anheben fest, D-24)
            "CREATE TABLE doc_number_counters (doc_type TEXT NOT NULL, fiscal_year INTEGER NOT NULL,
                last_value INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (doc_type, fiscal_year)) STRICT",
            // Abo-Termine und festgeschriebene Nummern (Phase A, F1d)
            "CREATE TABLE recurring_invoices (id TEXT PRIMARY KEY NOT NULL,
                next_due_date TEXT NOT NULL) STRICT",
            "CREATE TABLE recurring_subscriptions (id TEXT PRIMARY KEY NOT NULL,
                next_due_date TEXT NOT NULL) STRICT",
            "CREATE TABLE invoices (id TEXT PRIMARY KEY NOT NULL, invoice_number TEXT NOT NULL,
                locked_at TEXT) STRICT",
            "CREATE TABLE quotes (id TEXT PRIMARY KEY NOT NULL, quote_number TEXT NOT NULL,
                locked_at TEXT) STRICT",
            // für die Prüfungen in Phase A (Archivpfade, Migrationen)
            "CREATE TABLE archive_entries (id TEXT PRIMARY KEY NOT NULL, file_path TEXT NOT NULL)",
            "CREATE TABLE _sqlx_migrations (version INTEGER PRIMARY KEY, checksum BLOB NOT NULL,
                success INTEGER NOT NULL)",
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn backup_then_decrypt_roundtrip() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let outcome = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();
        let file_bytes = std::fs::read(&outcome.file_path).unwrap();

        // Korrekte Passphrase → Content entschlüsselbar, enthält DB.
        let (_m, content) = decrypt_backup(&file_bytes, "passphrase-1234567890").unwrap();
        let staging = dir.path().join("stg");
        extract_zip(
            &content,
            &staging,
            manifest::FORMAT_VERSION,
            &ExtractLimits::default(),
        )
        .unwrap();
        assert!(staging.join("klein-buch.sqlite").is_file());

        // Falsche Passphrase → Fehler.
        assert!(decrypt_backup(&file_bytes, "falsch").is_err());
    }

    #[tokio::test]
    async fn tampered_backup_fails() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let outcome = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();
        let mut file_bytes = std::fs::read(&outcome.file_path).unwrap();
        // Letztes Byte (im Ciphertext-Bereich) kippen → GCM-Auth schlägt an.
        let last = file_bytes.len() - 1;
        file_bytes[last] ^= 0x01;
        assert!(decrypt_backup(&file_bytes, "passphrase-1234567890").is_err());
    }

    #[test]
    fn extract_zip_rejects_traversal() {
        // Manuell einen ZIP mit bösartigem Pfad bauen.
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = zip::write::SimpleFileOptions::default();
        use std::io::Write;
        zw.start_file("../evil.txt", opts).unwrap();
        zw.write_all(b"x").unwrap();
        let bytes = zw.finish().unwrap().into_inner();
        let dir = TempDir::new().unwrap();
        assert!(extract_zip(
            &bytes,
            dir.path(),
            manifest::FORMAT_VERSION,
            &ExtractLimits::default()
        )
        .is_err());
    }

    #[test]
    fn apply_pending_swaps_db_and_archive() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.data_dir).unwrap();
        fs::create_dir_all(paths.archive_dir.join("2025")).unwrap();
        // Alte DB + altes Archive.
        fs::write(&paths.db_file, b"OLD-DB").unwrap();
        fs::write(paths.archive_dir.join("2025/old.pdf"), b"old").unwrap();

        // Staging mit neuer DB + neuem Archive.
        let staging = paths.data_dir.join(RESTORE_STAGING);
        fs::create_dir_all(staging.join("archive/2026")).unwrap();
        fs::write(staging.join("klein-buch.sqlite"), b"NEW-DB").unwrap();
        fs::write(staging.join("archive/2026/new.pdf"), b"new").unwrap();

        // Marker.
        let marker = paths.data_dir.join(RESTORE_MARKER);
        let info = serde_json::json!({"staging": staging.to_string_lossy(), "source":"x.kbk", "staged_at":"t"});
        fs::write(&marker, serde_json::to_vec(&info).unwrap()).unwrap();

        let applied = apply_pending(&paths).unwrap();
        assert!(applied.is_some());
        assert_eq!(fs::read(&paths.db_file).unwrap(), b"NEW-DB");
        assert!(paths.archive_dir.join("2026/new.pdf").is_file());
        assert!(!paths.archive_dir.join("2025/old.pdf").exists());
        assert!(!marker.exists());
    }

    #[test]
    fn apply_pending_noop_without_marker() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.data_dir).unwrap();
        assert!(apply_pending(&paths).unwrap().is_none());
    }

    /// G1-HARDEN.1: voller Backup→Restore-Roundtrip über die echte Pipeline
    /// (`apply` → Neustart-Simulation → `apply_pending`) mit nicht-trivialem
    /// Datenvolumen (1000 Zeilen). Verifiziert: (a) der **identische** Stand
    /// wird wiederhergestellt, (b) das **Pre-Restore-Backup wird erzwungen**
    /// erstellt.
    #[tokio::test]
    async fn full_restore_roundtrip_restores_identical_state() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        // Nicht-triviales Volumen.
        sqlx::query("CREATE TABLE bulk (id INTEGER PRIMARY KEY, payload TEXT NOT NULL) STRICT")
            .execute(&pool)
            .await
            .unwrap();
        for i in 0..1000i64 {
            sqlx::query("INSERT INTO bulk (id, payload) VALUES (?, ?)")
                .bind(i)
                .bind(format!("row-{i}"))
                .execute(&pool)
                .await
                .unwrap();
        }

        // Original-Stand sichern (manuelles Backup).
        let original = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();

        // Stand verändern — genau das soll der Restore zurückdrehen.
        sqlx::query("UPDATE marker SET v = 'mutated'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM bulk WHERE id >= 500")
            .execute(&pool)
            .await
            .unwrap();

        // Restore Phase A — verlangt entsperrte Session (für das Pre-Restore-Backup).
        let session = backup::BackupSession::default();
        session.set("passphrase-1234567890".to_string());
        let report = apply(
            &pool,
            &paths,
            &session,
            Path::new(&original.file_path),
            "test.kbk",
            "passphrase-1234567890",
            TEST_LIMITS,
        )
        .await
        .unwrap();

        // Pre-Restore-Backup wurde erzwungen erstellt + Staging/Marker liegen vor.
        assert!(report.requires_restart);
        assert!(
            paths.backups_dir.join(&report.pre_restore_backup).is_file(),
            "Pre-Restore-Backup muss als Datei existieren"
        );
        assert!(paths.data_dir.join(RESTORE_MARKER).exists());
        assert!(paths
            .data_dir
            .join(RESTORE_STAGING)
            .join("klein-buch.sqlite")
            .is_file());

        // Neustart simulieren: Pool schließen, dann Phase B (vor Pool-Open).
        pool.close().await;
        drop(pool);
        assert!(apply_pending(&paths).unwrap().is_some());

        // DB neu öffnen (ohne DDL) → Stand muss IDENTISCH zum Original sein.
        let pool2 = crate::db::open_pool(&paths.db_file, Some("passphrase-1234567890"))
            .await
            .unwrap();
        let v: String = sqlx::query_scalar("SELECT v FROM marker")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(
            v, "original-data",
            "Restore muss den Original-Stand herstellen"
        );
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bulk")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(n, 1000, "alle 1000 Zeilen müssen wiederhergestellt sein");
    }

    /// Baut aus einem aktuellen Backup ein Backup im alten Format 1 nach
    /// (`inputs/branding/` statt `branding/`, `formatVersion` 1).
    fn rewrite_as_format_v1(file: &Path, passphrase: &str) -> PathBuf {
        use std::io::Write;
        let bytes = fs::read(file).unwrap();
        let (mut m, content) = decrypt_backup(&bytes, passphrase).unwrap();
        let mut src = zip::ZipArchive::new(Cursor::new(content)).unwrap();
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for i in 0..src.len() {
            let mut e = src.by_index(i).unwrap();
            let name = e.name().to_string();
            let new_name = match name.strip_prefix("branding/") {
                Some(rest) => format!("inputs/branding/{rest}"),
                None => name,
            };
            let mut buf = Vec::new();
            e.read_to_end(&mut buf).unwrap();
            zw.start_file(new_name, opts).unwrap();
            zw.write_all(&buf).unwrap();
        }
        let zip_bytes = zw.finish().unwrap().into_inner();
        let salt = encrypt::random_bytes(encrypt::SALT_LEN);
        let nonce = encrypt::random_bytes(encrypt::NONCE_LEN);
        let key = encrypt::derive_key(passphrase.as_bytes(), &salt).unwrap();
        let ct =
            encrypt::encrypt(&key, &encrypt::nonce_array(&nonce).unwrap(), &zip_bytes).unwrap();
        m.format_version = 1;
        m.salt_hex = manifest::to_hex(&salt);
        m.nonce_hex = manifest::to_hex(&nonce);
        m.content_sha256 = snapshot::sha256_hex(&zip_bytes);
        m.content_size_bytes = zip_bytes.len() as u64;
        let out = file.with_file_name("format-v1.kbk");
        fs::write(&out, manifest::frame(&m, &ct).unwrap()).unwrap();
        out
    }

    /// Plan W1.5: Logo und Unterschrift (`data/branding`) überstehen
    /// Backup → Restore; ein nach dem Backup gelöschtes Bild kommt zurück,
    /// ein später hinzugekommenes verschwindet (exakter Backup-Stand).
    #[tokio::test]
    async fn restore_roundtrip_includes_logo_and_signature() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        let branding = crate::branding::branding_dir(&paths.data_dir);
        fs::create_dir_all(&branding).unwrap();
        fs::write(branding.join("logo.png"), b"LOGO-ORIGINAL").unwrap();
        fs::write(branding.join("signature.png"), b"SIG-ORIGINAL").unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let original = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();

        // Stand verändern: Logo ersetzt, Unterschrift gelöscht, neue Datei.
        fs::write(branding.join("logo.png"), b"LOGO-NEU").unwrap();
        fs::remove_file(branding.join("signature.png")).unwrap();
        fs::write(branding.join("logo.svg"), b"<svg/>").unwrap();

        let session = backup::BackupSession::default();
        session.set("passphrase-1234567890".to_string());
        let report = apply(
            &pool,
            &paths,
            &session,
            Path::new(&original.file_path),
            "test.kbk",
            "passphrase-1234567890",
            TEST_LIMITS,
        )
        .await
        .unwrap();
        assert_eq!(report.format_version, manifest::FORMAT_VERSION);
        assert!(report.branding_restored);
        assert!(report.notice.is_none());

        pool.close().await;
        let applied = apply_pending(&paths).unwrap().unwrap();
        assert!(applied.branding_restored);
        assert_eq!(
            fs::read(branding.join("logo.png")).unwrap(),
            b"LOGO-ORIGINAL"
        );
        assert_eq!(
            fs::read(branding.join("signature.png")).unwrap(),
            b"SIG-ORIGINAL"
        );
        assert!(!branding.join("logo.svg").exists());
        assert!(!paths.data_dir.join(BRANDING_ROLLBACK_DIR).exists());
    }

    /// Plan W1.5: ein Backup im Format 1 wird weiter eingespielt; `data/branding`
    /// bleibt dabei unverändert und der Report sagt das.
    #[tokio::test]
    async fn restore_of_format_v1_backup_keeps_branding() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        let branding = crate::branding::branding_dir(&paths.data_dir);
        fs::create_dir_all(&branding).unwrap();
        fs::write(branding.join("logo.png"), b"LOGO-IM-BACKUP").unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let v2 = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();
        let v1 = rewrite_as_format_v1(Path::new(&v2.file_path), "passphrase-1234567890");
        assert_eq!(preview(&v1).unwrap().format_version, 1);

        fs::write(branding.join("logo.png"), b"LOGO-AKTUELL").unwrap();

        let session = backup::BackupSession::default();
        session.set("passphrase-1234567890".to_string());
        let report = apply(
            &pool,
            &paths,
            &session,
            &v1,
            "v1.kbk",
            "passphrase-1234567890",
            TEST_LIMITS,
        )
        .await
        .unwrap();
        assert_eq!(report.format_version, 1);
        assert!(!report.branding_restored);
        assert_eq!(report.notice.as_deref(), Some(NOTICE_V1_NO_BRANDING));

        pool.close().await;
        let applied = apply_pending(&paths).unwrap().unwrap();
        assert!(!applied.branding_restored);
        assert_eq!(
            fs::read(branding.join("logo.png")).unwrap(),
            b"LOGO-AKTUELL"
        );
        assert!(
            !paths.inputs_dir.join("branding").exists(),
            "inputs/ bleibt tabu"
        );
    }

    /// Plan W2.7: nach Backup → Restore hat keine Datei unter `archive/` ein
    /// Schreibbit (write-once auch nach dem Kopieren aus dem Staging).
    #[cfg(unix)]
    #[tokio::test]
    async fn restored_archive_files_are_read_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        fs::create_dir_all(paths.archive_dir.join("2026/invoices/pdf")).unwrap();
        let f = paths.archive_dir.join("2026/invoices/pdf/RE-2026-0001.pdf");
        fs::write(&f, b"pdf").unwrap();
        crate::archive::store::set_readonly(&f).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let original = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();
        let session = backup::BackupSession::default();
        session.set("passphrase-1234567890".to_string());
        apply(
            &pool,
            &paths,
            &session,
            Path::new(&original.file_path),
            "test.kbk",
            "passphrase-1234567890",
            TEST_LIMITS,
        )
        .await
        .unwrap();
        pool.close().await;
        apply_pending(&paths).unwrap().unwrap();

        let mut checked = 0;
        let mut stack = vec![paths.archive_dir.clone()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let mode = fs::metadata(&p).unwrap().permissions().mode();
                    assert_eq!(
                        mode & 0o222,
                        0,
                        "{} hat ein Schreibbit ({mode:o})",
                        p.display()
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 1);
    }

    /// Baut einen Tausch-Ausgangszustand: Original-DB/-Archiv/-Branding und ein
    /// Staging (Format 2) mit Marker.
    fn swap_fixture(dir: &Path) -> Paths {
        let paths = paths_for(dir);
        fs::create_dir_all(paths.archive_dir.join("2025")).unwrap();
        fs::write(&paths.db_file, b"ORIGINAL-DB").unwrap();
        fs::write(paths.archive_dir.join("2025/alt.pdf"), b"alt").unwrap();
        let branding = crate::branding::branding_dir(&paths.data_dir);
        fs::create_dir_all(&branding).unwrap();
        fs::write(branding.join("logo.png"), b"ALT-LOGO").unwrap();
        let staging = paths.data_dir.join(RESTORE_STAGING);
        fs::create_dir_all(staging.join("archive/2026")).unwrap();
        fs::create_dir_all(staging.join("branding")).unwrap();
        fs::write(staging.join("klein-buch.sqlite"), b"NEUE-DB").unwrap();
        fs::write(staging.join("archive/2026/neu.pdf"), b"neu").unwrap();
        fs::write(staging.join("branding/logo.png"), b"NEU-LOGO").unwrap();
        let info = serde_json::json!({
            "staging": staging.to_string_lossy(), "source": "x.kbk",
            "staged_at": "t", "format_version": 2
        });
        fs::write(paths.data_dir.join(RESTORE_MARKER), info.to_string()).unwrap();
        paths
    }

    fn assert_original_state(paths: &Paths) {
        assert_eq!(fs::read(&paths.db_file).unwrap(), b"ORIGINAL-DB");
        assert_eq!(
            fs::read(paths.archive_dir.join("2025/alt.pdf")).unwrap(),
            b"alt"
        );
        assert!(!paths.archive_dir.join("2026/neu.pdf").exists());
        let branding = crate::branding::branding_dir(&paths.data_dir);
        assert_eq!(fs::read(branding.join("logo.png")).unwrap(), b"ALT-LOGO");
    }

    /// Review W2 S0-2: scheitert Stufe 1 beim Branding (nachdem DB und Archiv
    /// schon beiseitegelegt waren), werden beide sofort zurückgeholt; der Marker
    /// bleibt, ein späterer Lauf ohne Fehler wendet den Restore an.
    #[test]
    fn stage1_failure_restores_originals_immediately() {
        let dir = TempDir::new().unwrap();
        let paths = swap_fixture(dir.path());
        INJECT.with(|i| i.set(Some(InjectAt::Stage1(2))));
        let err = apply_pending(&paths).unwrap_err();
        INJECT.with(|i| i.set(None));
        assert!(err.to_string().contains("injiziert"), "{err}");
        assert_original_state(&paths);
        assert!(!swap_state_path(&paths).exists());
        assert!(
            paths.data_dir.join(RESTORE_MARKER).exists(),
            "Marker bleibt"
        );

        // Neustart wie in prepare_filesystem: Recovery + Tausch laufen sauber.
        let applied = apply_pending(&paths).unwrap();
        assert!(applied.is_some());
        assert_eq!(fs::read(&paths.db_file).unwrap(), b"NEUE-DB");
        assert!(!restore_incomplete(&paths));
    }

    /// Review W2 S0-2: scheitert Stufe 2 (Kopie), kommen die Originale zurück.
    #[test]
    fn stage2_failure_rolls_back_to_originals() {
        let dir = TempDir::new().unwrap();
        let paths = swap_fixture(dir.path());
        INJECT.with(|i| i.set(Some(InjectAt::Stage2)));
        let err = apply_pending(&paths).unwrap_err();
        INJECT.with(|i| i.set(None));
        assert!(err.to_string().contains("injiziert"), "{err}");
        assert_original_state(&paths);
        assert!(
            restore_incomplete(&paths),
            "Marker liegt noch → kein Onboarding/Entsperren"
        );
    }

    /// Review W2 S0-2: Absturz mitten in Stufe 2 (Zustand `moved`, Teilkopie)
    /// und beim Neustart ein frisch angelegtes leeres `archive/` — das Recovery
    /// holt das echte Archiv zurück, statt `archive.rollback` zu löschen.
    #[test]
    fn crash_in_stage2_recovers_real_archive_even_with_fresh_empty_dir() {
        let dir = TempDir::new().unwrap();
        let paths = swap_fixture(dir.path());
        // Absturz simulieren: Originale beiseite, Zustand `moved`, Teilkopie.
        write_swap_state(&paths, SwapPhase::Moved).unwrap();
        for (orig, rb, _) in swap_pairs(&paths, true) {
            fs::rename(&orig, &rb).unwrap();
        }
        fs::write(&paths.db_file, b"HALBE-DB").unwrap();
        fs::create_dir_all(&paths.archive_dir).unwrap(); // leer, wie ein Neustart

        recover_pending_rollback(&paths).unwrap();
        assert_original_state(&paths);
        assert!(!swap_state_path(&paths).exists());
    }

    /// Ohne Zustandsdatei (älterer Stand) wird nichts gelöscht: liegt beides vor,
    /// wandert die `.rollback`-Fassung nach `.verworfen-<uuid>`.
    #[test]
    fn recover_without_state_never_deletes() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.archive_dir).unwrap();
        let rb = paths.data_dir.join(ARCHIVE_ROLLBACK_DIR);
        fs::create_dir_all(&rb).unwrap();
        fs::write(rb.join("wichtig.pdf"), b"x").unwrap();
        recover_pending_rollback(&paths).unwrap();
        assert!(!rb.exists());
        let kept: Vec<_> = fs::read_dir(&paths.data_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("archive.rollback.verworfen-"))
            .collect();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(paths.data_dir.join(&kept[0]).join("wichtig.pdf").exists());
    }

    /// Review W2 S1-5: ein Anhang mit `..` im Namen blockiert den Restore nicht;
    /// echte Pfad-Traversal bleibt abgelehnt.
    #[test]
    fn extract_zip_allows_double_dots_in_file_names() {
        use std::io::Write;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = zip::write::SimpleFileOptions::default();
        zw.start_file("archive/2026/attachments/Angebot v2..pdf", opts)
            .unwrap();
        zw.write_all(b"x").unwrap();
        let bytes = zw.finish().unwrap().into_inner();
        let dir = TempDir::new().unwrap();
        extract_zip(
            &bytes,
            dir.path(),
            manifest::FORMAT_VERSION,
            &ExtractLimits::default(),
        )
        .unwrap();
        assert!(dir
            .path()
            .join("archive/2026/attachments/Angebot v2..pdf")
            .is_file());

        for evil in ["../evil.txt", "archive/../../evil.txt", "/etc/evil"] {
            let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
            zw.start_file(evil, opts).unwrap();
            zw.write_all(b"x").unwrap();
            let bytes = zw.finish().unwrap().into_inner();
            assert!(
                extract_zip(
                    &bytes,
                    dir.path(),
                    manifest::FORMAT_VERSION,
                    &ExtractLimits::default()
                )
                .is_err(),
                "{evil}"
            );
        }
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(bytes).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    /// Plan W7: Allowlist, Eintragszahl, Gesamtgröße, Symlinks, Doppelte,
    /// Format-1-Branding übersprungen.
    #[test]
    fn extract_zip_hardening() {
        let limits = ExtractLimits::default();
        let ok = zip_of(&[
            ("klein-buch.sqlite", b"db"),
            ("archive/2026/a.pdf", b"a"),
            ("branding/logo.png", b"l"),
        ]);
        let dir = TempDir::new().unwrap();
        assert_eq!(extract_zip(&ok, dir.path(), 2, &limits).unwrap(), 4);

        // fremde Namen, auch Unterordner der DB
        for evil in [
            "inputs/branding/x.png",
            "klein-buch.sqlite/x",
            "data/x",
            "C:x",
            "archive/x/../../evil",
        ] {
            let dir = TempDir::new().unwrap();
            let bytes = zip_of(&[(evil, b"x")]);
            assert!(
                extract_zip(&bytes, dir.path(), 2, &limits).is_err(),
                "{evil}"
            );
        }
        // Format 1: inputs/branding wird übersprungen
        let dir = TempDir::new().unwrap();
        let v1 = zip_of(&[
            ("klein-buch.sqlite", b"db"),
            ("inputs/branding/logo.png", b"l"),
        ]);
        extract_zip(&v1, dir.path(), 1, &limits).unwrap();
        assert!(!dir.path().join("inputs").exists());

        // zu viele Einträge
        let few = ExtractLimits {
            max_entries: 2,
            ..limits
        };
        let dir = TempDir::new().unwrap();
        let err = extract_zip(&ok, dir.path(), 2, &few).unwrap_err();
        assert!(err.to_string().contains("Einträge"), "{err}");

        // Gesamtgröße
        let small = ExtractLimits {
            max_total: 3,
            ..limits
        };
        let dir = TempDir::new().unwrap();
        let err = extract_zip(&ok, dir.path(), 2, &small).unwrap_err();
        assert!(err.to_string().contains("größer als erlaubt"), "{err}");

        // doppelte Datei
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = zip::write::SimpleFileOptions::default();
        {
            use std::io::Write;
            zw.start_file("archive/a.pdf", opts).unwrap();
            zw.write_all(b"1").unwrap();
            zw.start_file("archive/b.pdf", opts).unwrap();
            zw.write_all(b"2").unwrap();
        }
        let mut bytes = zw.finish().unwrap().into_inner();
        // zweiten Namen auf den ersten umbiegen (lokaler Kopf + Verzeichnis)
        let needle = b"archive/b.pdf";
        let mut i = 0;
        while i + needle.len() <= bytes.len() {
            if &bytes[i..i + needle.len()] == needle {
                bytes[i + 8] = b'a';
            }
            i += 1;
        }
        let dir = TempDir::new().unwrap();
        // zip fasst doppelte Namen beim Öffnen zusammen; sonst greift
        // `create_new`. In keinem Fall wird eine Datei überschrieben.
        let _ = extract_zip(&bytes, dir.path(), 2, &limits);
        let files: Vec<_> = std::fs::read_dir(dir.path().join("archive"))
            .map(|it| it.flatten().collect())
            .unwrap_or_default();
        assert!(files.len() <= 1, "{files:?}");

        // Symlink
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        zw.add_symlink("archive/link", "/etc/passwd", opts).unwrap();
        let bytes = zw.finish().unwrap().into_inner();
        let dir = TempDir::new().unwrap();
        let err = extract_zip(&bytes, dir.path(), 2, &limits).unwrap_err();
        assert!(err.to_string().contains("Symlink"), "{err}");
    }

    #[test]
    fn unsupported_format_version_is_rejected() {
        assert!(manifest::ensure_supported_format(1).is_ok());
        assert!(manifest::ensure_supported_format(2).is_ok());
        assert!(manifest::ensure_supported_format(0).is_err());
        assert!(manifest::ensure_supported_format(3).is_err());
    }

    /// G1-HARDEN.1: ohne entsperrte Session bricht der Restore ab — das
    /// Pre-Restore-Backup (mit der aktuellen Passphrase) ist nicht erstellbar,
    /// also darf nicht restauriert werden.
    #[tokio::test]
    async fn apply_rejects_when_session_locked() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        fs::create_dir_all(&paths.backups_dir).unwrap();
        let pool = pool_for_backup(&paths.db_file).await;

        let original = backup::create_now(&pool, &paths, "passphrase-1234567890", "manual")
            .await
            .unwrap();

        // Session NICHT entsperrt.
        let session = backup::BackupSession::default();
        let err = apply(
            &pool,
            &paths,
            &session,
            Path::new(&original.file_path),
            "test.kbk",
            "passphrase-1234567890",
            TEST_LIMITS,
        )
        .await
        .unwrap_err();
        assert!(
            format!("{err}").contains("entsperr"),
            "erwartete Entsperr-Hinweis, got: {err}"
        );
        // Kein Marker geschrieben — der Restore wurde sauber abgebrochen.
        assert!(!paths.data_dir.join(RESTORE_MARKER).exists());
    }
}
