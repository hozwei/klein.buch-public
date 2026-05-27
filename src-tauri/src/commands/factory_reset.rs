//! Factory-Reset-Commands (G1-RESET, ADR 0036) — Imperative Shell.
//!
//! Die **eine** sanktionierte Total-Löschung: setzt die gesamte lokale Instanz
//! (DB + Archiv + lokale Backups + alle maschinen-verwalteten Daten unter
//! `data_dir`) auf den Onboarding-Zustand zurück. Selektives Beleg-Löschen bleibt
//! verboten (GoBD-Hardline, Storno statt Löschung).
//!
//! **Zweiphasig** (race-frei, wie der Restore): Dieser Command (Phase A) prüft nur
//! die Freigabe-Bedingungen und schreibt einen **Marker** ins `data_dir`, dann
//! Neustart. Der eigentliche Datei-Nuke + Keychain-Wipe läuft beim nächsten Start
//! **vor** dem Pool-Open ([`crate::backup::factory_reset::apply_pending`]). So wird
//! der DB-Pool **nicht** im laufenden Betrieb geschlossen — sonst liefen parallele
//! Commands in „closed pool".
//!
//! Mehrstufige Absicherung (prüfungssicherer Default, ADR 0036 Pt. 2):
//! 1. **Passphrase-Verifikation** (Master-Passphrase, ADR 0035).
//! 2. **Tipp-Bestätigung** (`LÖSCHEN`) — reine Domain-Prüfung
//!    ([`crate::domain::factory_reset`]).
//! 3. **GoBD-Gating:** bestehen festgeschriebene Belege, ist Export **oder** eine
//!    getippte Aufbewahrungs-Quittung Pflicht.
//!
//! Scope = **nur lokal** (ADR 0036 Pt. 3): Off-Site-Backups (Cloud-Ordner / SFTP)
//! bleiben unangetastet. [`factory_reset_check`] nennt das Off-Site-Ziel, damit das
//! UI vorab darauf hinweisen kann.

use crate::backup;
use crate::config::Paths;
use crate::domain::factory_reset as fr;
use crate::error::{Error, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

/// Pre-Flight-Status für das Reset-UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryResetCheck {
    /// Anzahl festgeschriebener, aufbewahrungspflichtiger Belege. `> 0` ⇒ Export
    /// oder Aufbewahrungs-Quittung Pflicht (G1-RESET.3).
    pub locked_documents: i64,
    /// Ist ein Off-Site-Ziel konfiguriert (bleibt nach dem Reset bestehen)?
    pub has_off_site_target: bool,
    /// Menschlich lesbares Off-Site-Label (Pfad bzw. `sftp://…`), falls vorhanden.
    pub off_site_label: Option<String>,
    /// Exaktes Tipp-Bestätigungswort fürs UI ([`fr::CONFIRM_WORD`]).
    pub confirm_word: String,
    /// Exakter Wortlaut der Aufbewahrungs-Quittung fürs UI ([`fr::RETENTION_RECEIPT`]).
    pub retention_receipt_text: String,
}

/// Pre-Flight: liefert Belege-Zählung + Off-Site-Hinweis + die exakten
/// Bestätigungstexte fürs Reset-UI. Verlangt eine entsperrte Session (Pool im
/// State).
#[tauri::command]
pub async fn factory_reset_check(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
) -> Result<FactoryResetCheck> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    let locked = count_locked_documents(pool).await?;
    let off_site = backup::target::offsite_target(pool, &paths.backups_dir).await?;
    let (has_off_site_target, off_site_label) = match &off_site {
        Some(t) => (true, Some(off_site_label_of(t))),
        None => (false, None),
    };
    Ok(FactoryResetCheck {
        locked_documents: locked,
        has_off_site_target,
        off_site_label,
        confirm_word: fr::CONFIRM_WORD.to_string(),
        retention_receipt_text: fr::RETENTION_RECEIPT.to_string(),
    })
}

/// Führt den Factory Reset aus: prüft die Freigabe, merkt den Reset vor und
/// **startet die App neu**. Der eigentliche Nuke läuft beim Neustart vor dem
/// Pool-Open.
///
/// Der Neustart ist ein **echter Prozess-Neustart** ([`AppHandle::restart`]).
/// `restart()` divergiert (`-> !`); auf dem Erfolgspfad kehrt die Command daher
/// nicht zum Frontend zurück. Fehler (falsche Passphrase, Gating) entstehen
/// **vor** dem Neustart und werden normal ans UI propagiert.
#[tauri::command]
pub async fn factory_reset(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    passphrase: String,
    confirm_word: String,
    export_confirmed: bool,
    retention_receipt: String,
) -> Result<()> {
    let paths = Paths::from_handle(&app)?;
    factory_reset_request(
        pool.inner(),
        &paths,
        &passphrase,
        &confirm_word,
        export_confirmed,
        &retention_receipt,
    )
    .await?;
    // Sauberer Neustart → Phase B (apply_pending) löscht vor dem Pool-Open.
    app.restart()
}

/// Testbarer Kern (Phase A, ohne `AppHandle`/Neustart): verifiziert die
/// Passphrase, prüft Tipp-Wort + GoBD-Gating und schreibt den Reset-Marker (mit
/// den später zu löschenden Keychain-Service-IDs). Löscht **nichts** und schließt
/// den Pool **nicht** — der Nuke passiert beim nächsten Start (race-frei).
pub async fn factory_reset_request(
    pool: &SqlitePool,
    paths: &Paths,
    passphrase: &str,
    confirm_word: &str,
    export_confirmed: bool,
    retention_receipt: &str,
) -> Result<()> {
    // 1. Master-Passphrase verifizieren (ADR 0035). Falsch → kein Marker.
    if !backup::verify_passphrase(pool, passphrase).await? {
        return Err(Error::Backup(
            "Falsches Daten-Passwort — Zurücksetzen abgebrochen.".into(),
        ));
    }

    // 2. Tipp-Wort + GoBD-Gating (pure). Fehler → kein Marker.
    let locked = count_locked_documents(pool).await?;
    fr::check_reset_allowed(&fr::ResetRequest {
        confirm_word,
        export_confirmed,
        retention_receipt,
        locked_documents: locked,
    })
    .map_err(Error::Backup)?;

    // 3. Keychain-Service-IDs der Mail-Konten in den Marker legen (nach dem
    //    DB-Nuke nicht mehr ermittelbar) und den Reset vormerken.
    let mail_services = collect_mail_services(pool).await;
    backup::factory_reset::request(paths, &mail_services)?;

    tracing::warn!(
        "Factory Reset vorgemerkt (festgeschriebene Belege: {locked}). \
         Nuke beim nächsten Start."
    );
    Ok(())
}

/// Zählt festgeschriebene (aufbewahrungspflichtige) Belege über alle
/// `locked_at`-Tabellen — der GoBD-Gating-Indikator (G1-RESET.3).
async fn count_locked_documents(pool: &SqlitePool) -> Result<i64> {
    let n: i64 = sqlx::query_scalar(
        "SELECT
            (SELECT COUNT(*) FROM invoices             WHERE locked_at IS NOT NULL)
          + (SELECT COUNT(*) FROM quotes               WHERE locked_at IS NOT NULL)
          + (SELECT COUNT(*) FROM expenses             WHERE locked_at IS NOT NULL)
          + (SELECT COUNT(*) FROM depreciation_entries WHERE locked_at IS NOT NULL)
          + (SELECT COUNT(*) FROM private_movements    WHERE locked_at IS NOT NULL)",
    )
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Sammelt die Keychain-Service-IDs der Mail-Konten (für die Secret-Löschung).
/// Best-effort: liefert bei einem DB-Fehler eine leere Liste — der Reset darf
/// daran nicht scheitern.
async fn collect_mail_services(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT keychain_service_id FROM mail_accounts
          WHERE keychain_service_id IS NOT NULL AND TRIM(keychain_service_id) != ''",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Menschlich lesbares Off-Site-Label (kein Geheimnis).
fn off_site_label_of(t: &backup::target::BackupTarget) -> String {
    match t {
        backup::target::BackupTarget::Directory { path } => path.clone(),
        backup::target::BackupTarget::Sftp {
            host,
            port,
            user,
            remote_dir,
            ..
        } => {
            let rd = remote_dir.trim().trim_matches('/');
            if rd.is_empty() {
                format!("sftp://{user}@{host}:{port}/")
            } else {
                format!("sftp://{user}@{host}:{port}/{rd}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::factory_reset_request;
    use crate::backup::{self, factory_reset::RESET_MARKER, BackupSession};
    use crate::config::Paths;
    use crate::domain::factory_reset as fr;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::path::Path;
    use tempfile::TempDir;

    fn paths_for(dir: &Path) -> Paths {
        Paths {
            data_dir: dir.to_path_buf(),
            db_file: dir.join("klein-buch.sqlite"),
            archive_dir: dir.join("archive"),
            backups_dir: dir.join("backups"),
            inputs_dir: dir.join("inputs"),
            sidecar_dir: dir.join("sidecar"),
        }
    }

    /// File-basierter Pool + das Minimal-Schema, das Phase A liest.
    async fn setup_pool(db_file: &Path) -> SqlitePool {
        let opts = SqliteConnectOptions::new()
            .filename(db_file)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        for ddl in [
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now','utc'))) STRICT",
            "CREATE TABLE audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL DEFAULT (datetime('now','utc')),
                actor TEXT NOT NULL DEFAULT 'system', action TEXT NOT NULL,
                entity_type TEXT, entity_id TEXT, details_json TEXT) STRICT",
            "CREATE TABLE invoices (id TEXT PRIMARY KEY, locked_at TEXT) STRICT",
            "CREATE TABLE quotes (id TEXT PRIMARY KEY, locked_at TEXT) STRICT",
            "CREATE TABLE expenses (id TEXT PRIMARY KEY, locked_at TEXT) STRICT",
            "CREATE TABLE depreciation_entries (id TEXT PRIMARY KEY, locked_at TEXT) STRICT",
            "CREATE TABLE private_movements (id TEXT PRIMARY KEY, locked_at TEXT) STRICT",
            "CREATE TABLE mail_accounts (id TEXT PRIMARY KEY, keychain_service_id TEXT) STRICT",
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn arm_passphrase(pool: &SqlitePool) -> BackupSession {
        let session = BackupSession::default();
        backup::setup_passphrase(pool, &session, "passphrase-1234567890")
            .await
            .unwrap();
        session
    }

    #[tokio::test]
    async fn wrong_passphrase_aborts_without_marker() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.data_dir).unwrap();
        let pool = setup_pool(&paths.db_file).await;
        let _session = arm_passphrase(&pool).await;

        let res = factory_reset_request(&pool, &paths, "falsch", fr::CONFIRM_WORD, true, "").await;
        assert!(res.is_err(), "falsche Passphrase muss abbrechen");
        assert!(
            !paths.data_dir.join(RESET_MARKER).exists(),
            "kein Marker bei falscher Passphrase"
        );
        assert!(paths.db_file.exists(), "Phase A löscht nichts");
    }

    #[tokio::test]
    async fn locked_documents_gate_blocks_without_export_or_receipt() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.data_dir).unwrap();
        let pool = setup_pool(&paths.db_file).await;
        sqlx::query("INSERT INTO invoices (id, locked_at) VALUES ('i1', '2026-01-01')")
            .execute(&pool)
            .await
            .unwrap();
        let _session = arm_passphrase(&pool).await;

        let blocked = factory_reset_request(
            &pool,
            &paths,
            "passphrase-1234567890",
            fr::CONFIRM_WORD,
            false,
            "",
        )
        .await;
        assert!(blocked.is_err(), "Gating muss ohne Export/Quittung blocken");
        assert!(!paths.data_dir.join(RESET_MARKER).exists());
    }

    #[tokio::test]
    async fn success_writes_marker_with_mail_services_without_deleting() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        std::fs::create_dir_all(&paths.data_dir).unwrap();
        let pool = setup_pool(&paths.db_file).await;
        sqlx::query(
            "INSERT INTO mail_accounts (id, keychain_service_id)
             VALUES ('a', 'kleinbuch::mail::a')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Festgeschriebener Beleg + Export bestätigt → erlaubt.
        sqlx::query("INSERT INTO invoices (id, locked_at) VALUES ('i1', '2026-01-01')")
            .execute(&pool)
            .await
            .unwrap();
        let _session = arm_passphrase(&pool).await;

        factory_reset_request(
            &pool,
            &paths,
            "passphrase-1234567890",
            fr::CONFIRM_WORD,
            true,
            "",
        )
        .await
        .expect("Phase A muss gelingen");

        // Marker geschrieben, Daten noch da (Nuke kommt erst beim Neustart).
        let marker = paths.data_dir.join(RESET_MARKER);
        assert!(marker.exists(), "Marker muss geschrieben sein");
        assert!(paths.db_file.exists(), "Phase A löscht die DB nicht");
        let body = std::fs::read_to_string(&marker).unwrap();
        assert!(
            body.contains("kleinbuch::mail::a"),
            "Marker trägt die Keychain-Service-IDs"
        );
    }
}
