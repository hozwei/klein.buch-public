//! Datenbank-Schicht (Imperative Shell).
//!
//! - SQLite-Connection-Management via sqlx Pool
//! - Migration-Runner (forward-only)
//! - Schema-Version-Check beim Start
//! - Numbering-Counter (atomar)
//! - GoBD-Triggers werden in Migrationen erzeugt, hier nur dokumentiert
//!   in `triggers.rs`.

pub mod models;
pub mod numbering;
pub mod repo;
pub mod schema_version;
pub mod triggers;

use crate::backup::restore::AppliedInfo;
use crate::{
    config::Paths,
    error::{Error, Result},
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Migrations werden zur Compile-Zeit in die Binary eingebettet.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Hält einen beim App-Start angewendeten Restore zwischen, bis der Pool
/// (nach dem Entsperren) offen ist und der Audit-Eintrag geschrieben werden
/// kann. Der Restore-Swap selbst passiert filesystem-only **vor** dem Öffnen
/// des Pools (Windows-File-Lock), der zugehörige Audit-Eintrag braucht aber den
/// geöffneten Pool — daher dieser Zwischenspeicher im Tauri-State.
#[derive(Default)]
pub struct PendingRestoreAudit(pub Mutex<Option<AppliedInfo>>);

/// Baut ein `PRAGMA key = '…'`-Literal aus einer Passphrase: einfache
/// Anführungszeichen verdoppeln, dann in einfache Quotes fassen. SQLCipher
/// leitet aus dieser Passphrase intern (PBKDF2-HMAC-SHA512) den Seiten-
/// schlüssel ab; der KDF-Salt liegt im DB-Header → selbstbeschreibende,
/// cross-OS-portable Datei (ADR 0035, Amendment 2026-05-24).
fn sqlcipher_key_literal(passphrase: &str) -> String {
    format!("'{}'", passphrase.replace('\'', "''"))
}

/// Öffnet (oder erstellt) den SQLite-Pool für `db_file`.
///
/// - `key = Some(passphrase)` → **SQLCipher**: die Datei ist seitenweise
///   AES-verschlüsselt und nur mit korrekter Passphrase lesbar (ADR 0035 —
///   „kein Passwort, kein Zugriff"). Eine falsche/fehlende Passphrase lässt
///   bereits die erste Query scheitern („file is not a database").
/// - `key = None` → Klartext-DB (Tests + Bestands-/Übergangspfad vor der
///   Verschlüsselungs-Migration in G1-ENC Schritt 3).
///
/// Das `key`-Pragma muss vor jedem anderen Statement laufen; sqlx wendet die
/// via [`SqliteConnectOptions::pragma`] gesetzten Pragmas beim Verbindungs-
/// aufbau zuerst an.
pub async fn open_pool(db_file: &Path, key: Option<&str>) -> Result<SqlitePool> {
    let url = format!("sqlite://{}", db_file.to_string_lossy());

    let mut opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    if let Some(passphrase) = key {
        opts = opts.pragma("key", sqlcipher_key_literal(passphrase));
    }

    let pool: SqlitePool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

/// Liest den 16-Byte-Magic-Header der DB-Datei und entscheidet, ob es eine
/// **unverschlüsselte** SQLite-Datei ist. SQLCipher verschlüsselt auch den
/// Header, sodass eine verschlüsselte DB diesen Magic-String nie trägt.
///
/// Übergangs-Erkennung für G1-ENC Schritt 2: Manuels Bestands-DB ist noch
/// Klartext (Verschlüsselungs-Migration folgt in Schritt 3). Solange das so ist,
/// muss das Entsperren sie ohne Key öffnen und die Passphrase gegen den
/// gespeicherten Verifier prüfen; eine bereits verschlüsselte DB wird mit Key
/// geöffnet (SQLCipher selbst ist dann der Verifier).
pub fn db_file_is_plaintext(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 16];
    match f.read_exact(&mut buf) {
        Ok(()) => &buf == b"SQLite format 3\0",
        Err(_) => false,
    }
}

/// Bereitet das Dateisystem vor (Verzeichnisse + vorgemerkter Restore), **ohne**
/// den DB-Pool zu öffnen. Wird im `setup`-Closure beim App-Start aufgerufen.
///
/// Der Restore-Swap muss vor jedem Pool-Open passieren (Windows-File-Lock); der
/// zugehörige Audit-Eintrag wird im [`PendingRestoreAudit`]-State gepuffert und
/// nach dem Entsperren in [`run_migrations`] geschrieben.
pub fn prepare_filesystem(app: &AppHandle) -> Result<()> {
    let paths = Paths::from_handle(app)?;

    std::fs::create_dir_all(&paths.data_dir)?;
    std::fs::create_dir_all(&paths.archive_dir)?;
    std::fs::create_dir_all(&paths.backups_dir)?;

    // R7-INPUTS: First-Run-Copy vom gebundelten Default-Mirror in den
    // user-editierbaren `paths.inputs_dir`. Idempotent, ohne Ueberschreiben.
    // Muss VOR der DB-Init laufen, damit z.B. afa_tabellen::load oder die
    // PDF-Template-Loader bei jeder Production-Installation eine voll
    // bestueckte `inputs/` vorfinden — sonst crasht das AfA-Formular sofort
    // beim Mount (Bug V2026.5).
    crate::config::ensure_inputs_seeded(app, &paths)?;

    // G1-RESET (ADR 0036): einen vorgemerkten Factory Reset JETZT anwenden — vor
    // dem Öffnen des Pools (race-frei, kein Windows-File-Lock). Er löscht das
    // gesamte data_dir; ein gleichzeitig vorgemerkter Restore ist damit
    // gegenstandslos → früher Rücksprung in den leeren Onboarding-Zustand.
    if crate::backup::factory_reset::apply_pending(&paths)? {
        return Ok(());
    }

    // Block 4: Vorgemerkten Restore JETZT anwenden — vor dem Öffnen des Pools.
    if let Some(info) = crate::backup::restore::apply_pending(&paths)? {
        if let Some(holder) = app.try_state::<PendingRestoreAudit>() {
            *holder.0.lock().expect("PendingRestoreAudit poisoned") = Some(info);
        }
    }
    Ok(())
}

/// Migrationen anwenden (forward-only) + Schema-Version verifizieren + einen
/// ggf. gepufferten Restore-Audit-Eintrag schreiben. Setzt einen offenen Pool
/// voraus (für die verschlüsselte DB nach erfolgreichem `PRAGMA key`).
pub async fn run_migrations(app: &AppHandle, pool: &SqlitePool) -> Result<()> {
    MIGRATOR.run(pool).await?;
    schema_version::check_compatible(pool).await?;

    if let Some(holder) = app.try_state::<PendingRestoreAudit>() {
        let info = holder
            .0
            .lock()
            .expect("PendingRestoreAudit poisoned")
            .take();
        if let Some(info) = info {
            // R1-010 (v2026.5-Re-Review): Restore-Audit-Append-Fehler NICHT
            // mehr schlucken. Die GoBD-relevante Aussage „Restore wurde
            // angewendet" muss audit-bar sein — schlägt der Append fehl
            // (DB-Bug, Trigger-Verletzung), bricht der App-Start ab und der
            // Holder bleibt für den nächsten Start gefüllt.
            crate::db::repo::audit_log::append(
                pool,
                "backup.restore.applied",
                "backup",
                "restore",
                Some(&format!(
                    r#"{{"source":"{}","staged_at":"{}"}}"#,
                    info.source.replace('"', "'"),
                    info.staged_at.replace('"', "'")
                )),
            )
            .await
            .map_err(|e| {
                // Holder wieder befüllen, damit der nächste App-Start den
                // Audit-Eintrag nachholen kann.
                if let Ok(mut guard) = holder.0.lock() {
                    *guard = Some(info.clone());
                }
                Error::Config(format!(
                    "Restore-Audit konnte nicht geschrieben werden ({e}). \
                     Bitte App-Start erneut versuchen; bei Wiederholung Support kontaktieren."
                ))
            })?;
            tracing::info!("Restore angewendet (Quelle: {})", info.source);
        }
    }
    Ok(())
}

/// Öffnet den Pool (mit oder ohne SQLCipher-Key) und initialisiert ihn
/// (Migrationen + Schema-Check + Restore-Audit). Genutzt vom Onboarding
/// (`key = Some` → frische verschlüsselte DB) und vom Entsperren über den
/// Klartext-Übergangspfad (`key = None`).
pub async fn open_and_init(app: &AppHandle, key: Option<&str>) -> Result<SqlitePool> {
    let paths = Paths::from_handle(app)?;
    tracing::info!("Öffne DB: {}", paths.db_file.to_string_lossy());
    let pool = open_pool(&paths.db_file, key).await?;
    run_migrations(app, &pool).await?;
    Ok(pool)
}

/// Billiger Probe-Read: schlägt fehl, wenn der SQLCipher-Key falsch ist (oder
/// die Datei beschädigt). Trennt „falsche Passphrase" (→ `false`) sauber von
/// echten Fehlern beim Entsperren einer verschlüsselten DB.
pub async fn probe_readable(pool: &SqlitePool) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sqlite_master")
        .fetch_one(pool)
        .await
        .is_ok()
}

/// Hängt ein Suffix an einen Pfad an (z. B. `klein-buch.sqlite` + `-wal`).
fn with_db_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    std::path::PathBuf::from(s)
}

/// Entfernt eine Datei, falls vorhanden (No-op sonst).
fn remove_file_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// G1-ENC Schritt 3 — **Kern** der Bestands-Migration ohne Orchestrierung.
///
/// Exportiert die **Klartext**-DB unter `db_file` über `sqlcipher_export` in eine
/// frische SQLCipher-Datei und legt sie per **atomarem Rename** an dieselbe Stelle
/// (ADR 0035 Pt. 5 + Amendment: ein selbstbeschreibendes File, ein atomarer Swap
/// statt zweier Dateien — robust auf dem datenkritischen, lokal nicht testbaren
/// Pfad). Kein SQL-Migrations-File.
///
/// Sicherheits-Reihenfolge: die verschlüsselte Kopie wird **zuerst** erzeugt und
/// mit der Passphrase probe-gelesen — **bevor** die Klartext-Datei angefasst wird.
/// Erst danach werden Klartext-WAL/-SHM entfernt (sonst würde SQLite sie auf die
/// neue verschlüsselte Datei anwenden → Korruption) und die Datei atomar ersetzt.
/// Schlägt etwas vor dem Swap fehl, bleibt die Klartext-DB unverändert.
///
/// Voraussetzung: **kein offener Pool** auf `db_file` (Windows-File-Lock).
/// Idempotent: ist die Datei bereits verschlüsselt (oder fehlt sie), passiert nichts.
pub async fn encrypt_db_file_in_place(db_file: &Path, passphrase: &str) -> Result<()> {
    if !db_file_is_plaintext(db_file) {
        // Bereits verschlüsselt (oder nicht vorhanden) → nichts zu tun.
        return Ok(());
    }

    let tmp = with_db_suffix(db_file, ".enc-migrating");
    // Reste eines abgebrochenen früheren Laufs entfernen.
    remove_file_if_exists(&tmp)?;
    remove_file_if_exists(&with_db_suffix(&tmp, "-wal"))?;
    remove_file_if_exists(&with_db_suffix(&tmp, "-shm"))?;

    // 1. Export: dedizierte Einzel-Verbindung auf die Klartext-Quelle (kein Key).
    //    ATTACH + sqlcipher_export + DETACH müssen auf DERSELBEN Verbindung laufen
    //    (ATTACH-State ist verbindungslokal) → max_connections(1) + acquire().
    {
        let url = format!("sqlite://{}", db_file.to_string_lossy());
        // create_if_missing(true): die Quelle existiert ohnehin, aber das Flag
        // setzt SQLITE_OPEN_CREATE auf der Verbindung — und `ATTACH` erbt die
        // Open-Flags der Hauptverbindung. Ohne CREATE schlägt das Anlegen der
        // neuen verschlüsselten Attach-Datei mit SQLITE_CANTOPEN (Code 14) fehl.
        let opts = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let src = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;
        let mut conn = src.acquire().await?;

        // WAL vollständig in die Hauptdatei schreiben, damit der Export alles sieht.
        let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&mut *conn)
            .await;

        // Neue verschlüsselte DB anhängen (KEY) und komplett hineinexportieren.
        // Dateiname als Bind-Parameter (kein Injection-Risiko); KEY als Literal,
        // analog zum PRAGMA-key-Pfad (sqlcipher_key_literal).
        let attach = format!(
            "ATTACH DATABASE ? AS encrypted KEY {}",
            sqlcipher_key_literal(passphrase)
        );
        sqlx::query(&attach)
            .bind(tmp.to_string_lossy().to_string())
            .execute(&mut *conn)
            .await?;
        sqlx::query("SELECT sqlcipher_export('encrypted')")
            .fetch_all(&mut *conn)
            .await?;
        sqlx::query("DETACH DATABASE encrypted")
            .execute(&mut *conn)
            .await?;

        drop(conn);
        src.close().await;
    }

    // 2. Verschlüsselte Kopie verifizieren, BEVOR die Klartext-Datei ersetzt wird.
    {
        let check = open_pool(&tmp, Some(passphrase)).await?;
        let ok = probe_readable(&check).await;
        check.close().await;
        if !ok {
            remove_file_if_exists(&tmp)?;
            return Err(Error::Backup(
                "Verschlüsselte Kopie ließ sich nicht mit der Passphrase lesen — \
                 Migration abgebrochen, Klartext-DB unverändert."
                    .into(),
            ));
        }
    }

    // 3. Aufräumen + atomar swappen:
    //    - Die Verifikation (open_pool erzwingt WAL) kann tmp-wal/-shm angelegt
    //      haben; alle Daten liegen aber im tmp-Hauptfile (sqlcipher_export
    //      committet im DELETE-Modus). Diese Reste entfernen, damit nach dem
    //      Rename keine fehlbenannten WAL-Dateien zurückbleiben.
    //    - Klartext-WAL/-SHM der Quelle entfernen (sonst würde SQLite sie auf die
    //      neue verschlüsselte Datei anwenden → Korruption).
    remove_file_if_exists(&with_db_suffix(&tmp, "-wal"))?;
    remove_file_if_exists(&with_db_suffix(&tmp, "-shm"))?;
    remove_file_if_exists(&with_db_suffix(db_file, "-wal"))?;
    remove_file_if_exists(&with_db_suffix(db_file, "-shm"))?;
    std::fs::rename(&tmp, db_file)?;
    Ok(())
}

/// G1-ENC Schritt 3 — **Orchestrierung** der Bestands-Migration beim ersten
/// Entsperren/Onboarding einer Klartext-Bestands-DB:
///
/// 1. **Pflicht-Pre-Migration-Backup** (verschlüsselte `.kbk`, Trigger
///    `pre_restore` → never-prune) mit der gerade bestätigten Passphrase —
///    schlägt es fehl, wird **nichts** verschlüsselt (DB bleibt Klartext, intakt).
/// 2. Audit-Marker in die (noch offene) Klartext-DB schreiben — er wandert mit in
///    den Export → lückenloser Audit-Trail.
/// 3. Übergebenen Klartext-Pool schließen, Datei verschlüsseln + atomar swappen.
/// 4. Pool **mit Key** neu öffnen, Schema-Check, Erfolgs-Audit.
///
/// Liefert den neuen, verschlüsselten Pool zurück (vom Aufrufer in den State zu
/// hängen). Ist die DB bereits verschlüsselt, wird der übergebene Pool unverändert
/// zurückgegeben (kein Pre-Backup, kein Swap).
pub async fn migrate_plaintext_to_encrypted(
    app: &AppHandle,
    plaintext_pool: SqlitePool,
    passphrase: &str,
) -> Result<SqlitePool> {
    let paths = Paths::from_handle(app)?;
    if !db_file_is_plaintext(&paths.db_file) {
        return Ok(plaintext_pool);
    }

    // 1. Pflicht-Pre-Migration-Backup. Bei Fehler: Pool schließen + abbrechen,
    //    die Klartext-DB bleibt unverändert (sicher wiederholbar).
    let pre =
        match crate::backup::create_now(&plaintext_pool, &paths, passphrase, "pre_restore").await {
            Ok(outcome) => outcome,
            Err(e) => {
                plaintext_pool.close().await;
                return Err(e);
            }
        };

    // 2. Audit-Marker noch in die Klartext-DB (wird mitexportiert).
    crate::db::repo::audit_log::append(
        &plaintext_pool,
        "encryption.migration.started",
        "db",
        "at_rest",
        Some(&format!(
            r#"{{"pre_migration_backup":"{}"}}"#,
            pre.file_path.replace('"', "'")
        )),
    )
    .await
    .ok();

    // 3. Pool schließen (Windows-File-Lock), dann Datei verschlüsseln + swappen.
    plaintext_pool.close().await;
    encrypt_db_file_in_place(&paths.db_file, passphrase).await?;

    // 4. Verschlüsselt neu öffnen + Schema-Check + Erfolgs-Audit.
    let pool = open_pool(&paths.db_file, Some(passphrase)).await?;
    run_migrations(app, &pool).await?;
    crate::db::repo::audit_log::append(
        &pool,
        "encryption.migration.applied",
        "db",
        "at_rest",
        Some(r#"{"kdf":"sqlcipher-pbkdf2-hmac-sha512"}"#),
    )
    .await
    .ok();
    tracing::info!("Bestands-DB nach SQLCipher migriert (G1-ENC Schritt 3).");
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    /// G1-ENC.1-Akzeptanz: Eine mit Passphrase angelegte DB ist ohne (bzw. mit
    /// falscher) Passphrase nicht lesbar, und die Datei enthält keine Klartext-
    /// Nutzdaten. Grün auf dem Host beweist: SQLCipher ist tatsächlich gelinkt
    /// und `PRAGMA key` greift durch sqlx (Feature-Unification ok).
    #[tokio::test]
    async fn encrypted_db_requires_passphrase() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("enc.sqlite");
        let pass = "correct horse battery staple";

        // Anlegen + schreiben (verschlüsselt).
        {
            let pool = open_pool(&db, Some(pass)).await.expect("open with key");
            sqlx::query("CREATE TABLE t (v TEXT NOT NULL) STRICT")
                .execute(&pool)
                .await
                .expect("create");
            sqlx::query("INSERT INTO t (v) VALUES ('streng-geheim')")
                .execute(&pool)
                .await
                .expect("insert");
            pool.close().await;
        }

        // Korrekte Passphrase → lesbar.
        {
            let pool = open_pool(&db, Some(pass)).await.expect("reopen with key");
            let row = sqlx::query("SELECT v FROM t")
                .fetch_one(&pool)
                .await
                .expect("read back");
            assert_eq!(row.get::<String, _>("v"), "streng-geheim");
            pool.close().await;
        }

        // Ohne Passphrase → nicht lesbar.
        {
            let res: crate::error::Result<()> = async {
                let pool = open_pool(&db, None).await?;
                sqlx::query("SELECT v FROM t").fetch_one(&pool).await?;
                Ok(())
            }
            .await;
            assert!(
                res.is_err(),
                "Klartext-Open einer verschlüsselten DB darf nicht gelingen"
            );
        }

        // Falsche Passphrase → nicht lesbar.
        {
            let res: crate::error::Result<()> = async {
                let pool = open_pool(&db, Some("falsche-passphrase")).await?;
                sqlx::query("SELECT v FROM t").fetch_one(&pool).await?;
                Ok(())
            }
            .await;
            assert!(res.is_err(), "Falsche Passphrase darf die DB nicht öffnen");
        }

        // Rohe Datei: kein Klartext, kein SQLite-Klartext-Header.
        let bytes = std::fs::read(&db).expect("read raw db file");
        assert!(
            !bytes
                .windows(b"streng-geheim".len())
                .any(|w| w == b"streng-geheim"),
            "verschlüsselte DB-Datei enthält Klartext-Nutzdaten"
        );
        assert!(
            !bytes.starts_with(b"SQLite format 3\0"),
            "Datei sieht wie eine unverschlüsselte SQLite-DB aus"
        );
    }

    /// Der Klartext-Pfad (`key = None`) bleibt funktionsfähig — er trägt das
    /// bestehende Bootstrap-Verhalten bis zur Verschlüsselungs-Migration.
    #[tokio::test]
    async fn plaintext_pool_roundtrips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("plain.sqlite");

        let pool = open_pool(&db, None).await.expect("open plaintext");
        sqlx::query("CREATE TABLE t (v TEXT NOT NULL) STRICT")
            .execute(&pool)
            .await
            .expect("create");
        sqlx::query("INSERT INTO t (v) VALUES ('ok')")
            .execute(&pool)
            .await
            .expect("insert");
        pool.close().await;

        // Klartext-DB hat den normalen SQLite-Header.
        let bytes = std::fs::read(&db).expect("read raw db file");
        assert!(
            bytes.starts_with(b"SQLite format 3\0"),
            "unverschlüsselte DB sollte den SQLite-Header tragen"
        );
    }

    /// G1-ENC Schritt 2: Klartext- vs. verschlüsselte DB sicher unterscheiden.
    /// Die Bootstrap-Inversion entscheidet anhand dieses Headers, ob beim
    /// Entsperren mit oder ohne SQLCipher-Key geöffnet wird.
    #[tokio::test]
    async fn detects_plaintext_vs_encrypted_header() {
        let dir = tempfile::tempdir().expect("tempdir");

        // Nicht existierende Datei → kein Klartext (führt zum Onboarding).
        let missing = dir.path().join("missing.sqlite");
        assert!(!db_file_is_plaintext(&missing));

        // Klartext-DB → Magic-Header vorhanden.
        let plain = dir.path().join("plain.sqlite");
        {
            let pool = open_pool(&plain, None).await.expect("open plaintext");
            sqlx::query("CREATE TABLE t (v TEXT) STRICT")
                .execute(&pool)
                .await
                .expect("create");
            pool.close().await;
        }
        assert!(
            db_file_is_plaintext(&plain),
            "Klartext-DB muss erkannt werden"
        );

        // Verschlüsselte DB → Header ist mitverschlüsselt, kein Magic-String.
        let enc = dir.path().join("enc.sqlite");
        {
            let pool = open_pool(&enc, Some("correct horse battery staple"))
                .await
                .expect("open encrypted");
            sqlx::query("CREATE TABLE t (v TEXT) STRICT")
                .execute(&pool)
                .await
                .expect("create");
            pool.close().await;
        }
        assert!(
            !db_file_is_plaintext(&enc),
            "verschlüsselte DB darf nicht als Klartext gelten"
        );
    }

    /// G1-ENC Schritt 3: Eine bestehende Klartext-DB wird in-place nach SQLCipher
    /// migriert. Danach ist die Datei verschlüsselt, mit korrekter Passphrase
    /// lesbar (Daten erhalten), ohne Passphrase nicht — und ein zweiter Aufruf
    /// ist ein No-op (Idempotenz, da bereits verschlüsselt).
    #[tokio::test]
    async fn migrates_plaintext_db_in_place_to_encrypted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("klein-buch.sqlite");
        let pass = "correct horse battery staple";

        // Klartext-DB mit Nutzdaten anlegen.
        {
            let pool = open_pool(&db, None).await.expect("open plaintext");
            sqlx::query("CREATE TABLE t (v TEXT NOT NULL) STRICT")
                .execute(&pool)
                .await
                .expect("create");
            sqlx::query("INSERT INTO t (v) VALUES ('streng-geheim')")
                .execute(&pool)
                .await
                .expect("insert");
            pool.close().await;
        }
        assert!(db_file_is_plaintext(&db), "Ausgangs-DB muss Klartext sein");

        // Migrieren.
        encrypt_db_file_in_place(&db, pass)
            .await
            .expect("encrypt in place");

        // Datei ist jetzt verschlüsselt, Temp-Datei aufgeräumt, kein Klartext mehr.
        assert!(
            !db_file_is_plaintext(&db),
            "DB muss nach Migration verschlüsselt sein"
        );
        assert!(
            !with_db_suffix(&db, ".enc-migrating").exists(),
            "Temp-Datei muss aufgeräumt sein"
        );
        let bytes = std::fs::read(&db).expect("read raw db file");
        assert!(
            !bytes
                .windows(b"streng-geheim".len())
                .any(|w| w == b"streng-geheim"),
            "migrierte DB-Datei enthält noch Klartext-Nutzdaten"
        );

        // Mit korrekter Passphrase lesbar, Daten erhalten.
        {
            let pool = open_pool(&db, Some(pass)).await.expect("reopen encrypted");
            let row = sqlx::query("SELECT v FROM t")
                .fetch_one(&pool)
                .await
                .expect("read back");
            assert_eq!(row.get::<String, _>("v"), "streng-geheim");
            pool.close().await;
        }

        // Ohne Passphrase nicht lesbar.
        {
            let res: crate::error::Result<()> = async {
                let pool = open_pool(&db, None).await?;
                sqlx::query("SELECT v FROM t").fetch_one(&pool).await?;
                Ok(())
            }
            .await;
            assert!(
                res.is_err(),
                "Klartext-Open der migrierten DB darf nicht gelingen"
            );
        }

        // Idempotenz: erneuter Aufruf ist ein No-op (bereits verschlüsselt).
        encrypt_db_file_in_place(&db, pass)
            .await
            .expect("idempotenter No-op");
        assert!(!db_file_is_plaintext(&db));
    }
}
