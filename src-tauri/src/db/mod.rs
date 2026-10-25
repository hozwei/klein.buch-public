//! Datenbank-Schicht (Imperative Shell).
//!
//! - SQLite-Connection-Management via sqlx Pool
//! - Migration-Runner (forward-only)
//! - Schema-Version-Check beim Entsperren (vor dem Einhängen des Pools)
//! - Numbering-Counter (atomar)
//! - GoBD-Triggers werden in Migrationen erzeugt, hier nur dokumentiert
//!   in `triggers.rs`.

pub mod models;
pub mod numbering;
pub mod repo;
pub mod schema_version;
pub mod triggers;

use crate::backup::restore::AppliedInfo;
use crate::error::{Error, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

/// Migrations werden zur Compile-Zeit in die Binary eingebettet.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Hält einen beim Serverstart angewendeten Restore zwischen, bis der Pool
/// (nach dem Entsperren) offen ist und der Audit-Eintrag geschrieben werden
/// kann. Der Restore-Swap selbst passiert filesystem-only **vor** dem Öffnen
/// des Pools (eine offene Verbindung hielte die alte Datei samt WAL weiter
/// fest), der zugehörige Audit-Eintrag braucht aber den geöffneten Pool —
/// daher dieser Zwischenspeicher im [`crate::app_ctx::AppCtx`].
#[derive(Default)]
pub struct PendingRestoreAudit(pub Mutex<Option<AppliedInfo>>);

/// Baut ein `PRAGMA key = '…'`-Literal aus einer Passphrase: einfache
/// Anführungszeichen verdoppeln, dann in einfache Quotes fassen. SQLCipher
/// leitet aus dieser Passphrase intern (PBKDF2-HMAC-SHA512) den Seiten-
/// schlüssel ab; der KDF-Salt liegt im DB-Header → selbstbeschreibende,
/// cross-OS-portable Datei (ADR 0035, Amendment 2026-05-24).
pub(crate) fn sqlcipher_key_literal(passphrase: &str) -> String {
    format!("'{}'", passphrase.replace('\'', "''"))
}

/// Startet eine Schreib-Transaktion mit `BEGIN IMMEDIATE` (Plan W2.5).
///
/// Ein normales `BEGIN` (deferred) nimmt die Schreibsperre erst beim ersten
/// Schreiben; liest die Transaktion vorher (Lesen-dann-Schreiben, z. B.
/// Festschreiben, Zahlung, Nummernkreis) und hat inzwischen eine andere
/// Verbindung geschrieben, scheitert das Hochstufen unter WAL sofort mit
/// `SQLITE_BUSY` — der `busy_timeout` greift dort nicht. `IMMEDIATE` holt die
/// Sperre am Anfang und wartet dabei bis zum `busy_timeout`.
pub async fn begin_immediate(
    pool: &SqlitePool,
) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>> {
    Ok(pool.begin_with("BEGIN IMMEDIATE").await?)
}

/// Wartezeit auf eine belegte Datenbank, bevor `SQLITE_BUSY` gemeldet wird
/// (Plan W2.5).
pub const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

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
/// Marker eines vorgemerkten Factory Resets der Desktop-Fassung (ADR 0036);
/// im Server verweigert er den Start (E7).
const FACTORY_RESET_MARKER: &str = "FACTORY_RESET_PENDING.json";

/// Initialisiert das von SQLCipher genutzte OpenSSL **ohne** Aufräumen per
/// `atexit`: sqlx schließt Verbindungen in eigenen Threads, die beim
/// Prozessende noch laufen können; das `atexit`-Aufräumen von OpenSSL gab dann
/// „double free or corruption“ (CI-Fund W10, Testbinary nach allen grünen
/// Tests). Muss vor dem ersten SQLCipher-Zugriff laufen; mehrfach aufrufbar.
pub fn init_crypto() {
    openssl_sys::init();
}

pub async fn open_pool(db_file: &Path, key: Option<&str>) -> Result<SqlitePool> {
    init_crypto();
    let url = format!("sqlite://{}", db_file.to_string_lossy());

    // Statement-Logging aus (Review W2 S0-3, Plan W5.9): sqlx führt
    // `PRAGMA key = '<Passphrase>'` als normales Statement aus und loggt es auf
    // DEBUG bzw. als „slow statement“ (ab 1 s, z. B. PBKDF2 beim ersten Lesen)
    // auf WARN — mit der Passphrase im Text.
    use sqlx::ConnectOptions as _;
    let mut opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(BUSY_TIMEOUT)
        .log_statements(log::LevelFilter::Off)
        .log_slow_statements(log::LevelFilter::Off, std::time::Duration::from_secs(3600));

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
/// den DB-Pool zu öffnen. Wird beim Serverstart vor dem Port-Bind aufgerufen
/// (`server::startup::run_checks`).
///
/// Der Restore-Swap muss vor jedem Pool-Open passieren: SQLite verträgt keinen
/// Tausch der Datei unter einer offenen Verbindung (sie arbeitete über ihren
/// Dateideskriptor und die WAL-/SHM-Dateien mit der alten DB weiter). Der
/// zugehörige Audit-Eintrag wird im [`PendingRestoreAudit`]-State gepuffert und
/// nach dem Entsperren in [`run_migrations`] geschrieben.
pub fn prepare_filesystem(ctx: &crate::app_ctx::AppCtx) -> Result<()> {
    let paths = &ctx.paths;

    // `archive/` und `backups/` erst NACH dem Restore-Recovery anlegen (Review
    // W2 S0-2): ein leeres neues `archive/` neben `archive.rollback` sah für ein
    // älteres Recovery wie „Tausch fertig“ aus.
    std::fs::create_dir_all(&paths.data_dir)?;

    // R7-INPUTS: First-Run-Copy vom gebundelten Default-Mirror in den
    // user-editierbaren `paths.inputs_dir`. Idempotent, ohne Ueberschreiben.
    // Muss VOR der DB-Init laufen, damit z.B. afa_tabellen::load oder die
    // PDF-Template-Loader bei jeder Production-Installation eine voll
    // bestueckte `inputs/` vorfinden — sonst crasht das AfA-Formular sofort
    // beim Mount (Bug V2026.5).
    crate::config::ensure_inputs_seeded(paths)?;

    // E7 (ADR 0036/0039, Plan W5): Factory Reset ist im Server-Betrieb
    // abgeschaltet. Ein vorgemerkter Reset (Marker der Desktop-Fassung oder von
    // außen angelegt) wird nie angewendet — der Start bricht ab, nichts wird
    // gelöscht. Der Reset-Code selbst ist seit W11 entfernt.
    let reset_marker = paths.data_dir.join(FACTORY_RESET_MARKER);
    if reset_marker.exists() {
        return Err(Error::Config(format!(
            "{} gefunden — Factory Reset ist im Server-Betrieb abgeschaltet (E7). \
             Start abgebrochen, nichts gelöscht; Datei prüfen und entfernen.",
            reset_marker.display()
        )));
    }

    // Block 4: Vorgemerkten Restore JETZT anwenden — vor dem Öffnen des Pools.
    // Reste eines abgebrochenen Phase-A-Laufs (Staging ohne Marker und ohne
    // Tausch-Zustand) entfernen (Gegenprüfung W7).
    crate::backup::restore::clean_stale_staging(paths);
    // Ein angewendeter Restore, dessen Audit noch aussteht, steht in
    // RESTORE_APPLIED.json (übersteht Neustarts, Gegenprüfung W7).
    let applied = match crate::backup::restore::apply_pending(paths)? {
        Some(info) => Some(info),
        None => crate::backup::restore::read_applied(paths),
    };
    if let Some(info) = applied {
        *ctx.pending_restore
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(info);
    }
    std::fs::create_dir_all(&paths.archive_dir)?;
    std::fs::create_dir_all(&paths.backups_dir)?;
    Ok(())
}

/// Migrationen anwenden (forward-only) + Schema-Version verifizieren +
/// Fremdschlüssel prüfen + einen ggf. gepufferten Restore abschließen
/// ([`finish_restore`]). Setzt einen offenen Pool voraus (für die
/// verschlüsselte DB nach erfolgreichem `PRAGMA key`).
///
/// Prüfungen (R1-012, Lauf F2c): Stehen Migrationen an, prüft
/// [`precheck_before_migration`] die Datenbank **vor** der Umstellung —
/// ein Altfehler lehnt ab, solange die Datenbank noch auf dem alten Schema
/// steht (das vorige Image öffnet sie weiter). Nach den Migrationen prüft
/// [`foreign_key_violations`] erneut; ein Treffer dort lehnt ebenfalls ab, die
/// Meldung sagt dann ehrlich, dass die Umstellung schon gespeichert ist.
pub async fn run_migrations(ctx: &crate::app_ctx::AppCtx, pool: &SqlitePool) -> Result<()> {
    let pending = pending_migration_from(pool).await?;
    if let Some(from) = pending {
        precheck_before_migration(pool, from).await?;
    }
    run_migrations_prechecked(ctx, pool, pending).await
}

/// Schema-Version einer Datenbank, auf der Migrationen anstehen (`None`: frische
/// oder aktuelle Datenbank). Eine **neuere** Datenbank lehnt sie ab
/// (Down-Migration-Schutz, Schema-Hardline, Lauf F D-17/D-18): sie geht gar
/// nicht erst an den Migrations-Runner, der nur „Migration fehlt“ meldete,
/// sondern scheitert mit dem Hinweis auf das passende Image.
pub async fn pending_migration_from(pool: &SqlitePool) -> Result<Option<i32>> {
    match schema_version::read(pool).await? {
        Some(v) if v > schema_version::EXPECTED_SCHEMA_VERSION => {
            schema_version::check_compatible(pool).await?;
            // check_compatible lehnt eine neuere Version immer ab.
            Err(Error::Config(format!(
                "Schema-Version {v} ist neuer als {}",
                schema_version::EXPECTED_SCHEMA_VERSION
            )))
        }
        Some(v) if v < schema_version::EXPECTED_SCHEMA_VERSION => Ok(Some(v)),
        _ => Ok(None),
    }
}

/// Wie [`run_migrations`], wenn der Aufrufer [`precheck_before_migration`]
/// schon ausgeführt hat (`commands::backup::migrate_with_backup`: vor dem
/// Pflicht-Backup). `pending` = Schema vor der Umstellung (`None`: keine
/// Migration offen) — steuert nur die Meldung der Prüfung danach.
pub async fn run_migrations_prechecked(
    ctx: &crate::app_ctx::AppCtx,
    pool: &SqlitePool,
    pending: Option<i32>,
) -> Result<()> {
    if let Err(e) = MIGRATOR.run(pool).await {
        // Vorab-Prüfung einer Migration (benannter CHECK „Migration 00NN
        // abgebrochen: …“, z. B. 0032): die Transaktion dieser Migration ist
        // zurückgerollt — klare Meldung statt Fehlertext des Runners
        // (ADR 0039 Nr. 124). Die Rust-Vorabprüfung fängt den Fall für 0032
        // schon vor der ersten Migration ab; das hier ist die zweite Linie.
        let text = e.to_string();
        if let Some(pos) = text.find("CHECK constraint failed: Migration ") {
            let reason = &text[pos + "CHECK constraint failed: ".len()..];
            tracing::error!("{INTEGRITY_REFUSED}: {reason}");
            return Err(Error::Config(format!("{INTEGRITY_REFUSED}: {reason}")));
        }
        return Err(e.into());
    }
    schema_version::check_compatible(pool).await?;
    let violations = foreign_key_violations(pool).await?;
    if !violations.is_empty() {
        // F3b: Ein früherer Versuch kann die Umstellung schon gespeichert haben
        // (Pflicht-Backup, Migration, dann diese Prüfung gescheitert). Dann
        // steht hier keine Migration mehr an, die Meldung muss aber weiter
        // „nach der Umstellung“ sagen — sonst schickte sie zum vorigen Image,
        // das die neuere Datenbank ablehnt.
        let pending = match pending {
            Some(from) => Some(from),
            None => unfinished_migration_from(pool).await.ok().flatten(),
        };
        let msg = integrity_message_after(pending, &violations);
        log_foreign_key_violations("Entsperren", &violations);
        tracing::error!("{msg}");
        return Err(Error::Config(msg));
    }

    {
        let holder = &ctx.pending_restore;
        let info = holder
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(info) = info {
            if let Err(e) = finish_restore(ctx, pool, &info).await {
                // R1-010: Die GoBD-relevante Aussage „Restore wurde angewendet"
                // muss im Audit stehen. Scheitert die Transaktion, bricht das
                // Entsperren ab und der Holder bleibt für den nächsten Versuch
                // gefüllt (RESTORE_APPLIED.json bleibt ohnehin liegen).
                if let Ok(mut guard) = holder.0.lock() {
                    *guard = Some(info.clone());
                }
                return Err(Error::Config(format!(
                    "Wiederherstellung konnte nicht protokolliert werden ({e}) — bitte erneut \
                     anmelden; bleibt der Fehler, das Server-Log prüfen (Handbuch: Fehlersuche \
                     im Container)."
                )));
            }
        }
    }
    // Standard-Zahlungskonten einmalig und transaktional (Lauf F, D-14) — bei
    // der Einrichtung bzw. beim ersten Entsperren nach dem Update, nicht mehr
    // beim Öffnen der Einstellungsseite.
    if let Err(e) = crate::db::repo::payment_accounts::seed_defaults_once(pool).await {
        tracing::warn!("Standard-Zahlungskonten nicht angelegt: {e}");
    }
    Ok(())
}

/// Anfang der Meldung, mit der das Entsperren wegen einer Prüfung der
/// Datenbank abbricht: verletzte Fremdschlüssel (R1-012) oder eine
/// gescheiterte Vorab-Prüfung einer Migration (0032). Das Entsperren
/// antwortet damit 409 `db_integrity_failed` (`server::auth`). Nur für das
/// Entsperren — die Restore-Prüfung (Phase A) meldet mit eigenem Kontext.
pub const INTEGRITY_REFUSED: &str = "Datenbank-Prüfung beim Entsperren fehlgeschlagen";

/// Kennzeichen in der Meldung, wenn die Prüfung erst **nach** einer schon
/// gespeicherten Umstellung anschlägt (Lauf F2c): Dann stimmt „nichts
/// verändert“ nicht mehr, `server::auth` antwortet mit anderem Text.
pub const INTEGRITY_AFTER_MIGRATION: &str = "nach der Umstellung";

/// Ein verletzter Fremdschlüssel aus `PRAGMA foreign_key_check`: Tabelle,
/// rowid und Zieltabelle — keine Inhalte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeyViolation {
    pub table: String,
    pub rowid: Option<i64>,
    pub parent: String,
}

impl std::fmt::Display for ForeignKeyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rowid {
            Some(r) => write!(f, "{} (rowid {r}) → {}", self.table, self.parent),
            None => write!(f, "{} (rowid -) → {}", self.table, self.parent),
        }
    }
}

/// Prüft die referenzielle Integrität der ganzen Datenbank
/// (`PRAGMA foreign_key_check`, R1-012) und liefert die Befunde — ohne zu
/// loggen oder zu urteilen; die Aufrufer (Entsperren vor und nach der
/// Umstellung, Restore Phase A) melden mit ihrem Kontext (Lauf F2c). Die
/// Migrationen 0002 und 0003 bauten Tabellen um (0003 mit
/// `PRAGMA foreign_keys = OFF`); ein dabei, in einem eingespielten Backup oder
/// durch eine Bearbeitung von außen entstandener verwaister Verweis fiele sonst
/// erst bei einem späteren Schreibzugriff (oder nie) auf.
pub async fn foreign_key_violations(pool: &SqlitePool) -> Result<Vec<ForeignKeyViolation>> {
    use sqlx::Row;
    let rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await?;
    rows.iter()
        .map(|row| {
            Ok(ForeignKeyViolation {
                table: row.try_get(0)?,
                rowid: row.try_get(1)?,
                parent: row.try_get(2)?,
            })
        })
        .collect()
}

/// Schreibt höchstens 20 Befunde ins Log, mit dem Kontext der Prüfung
/// (z. B. „Entsperren“, „Restore-Prüfung“).
pub fn log_foreign_key_violations(context: &str, violations: &[ForeignKeyViolation]) {
    for v in violations.iter().take(20) {
        tracing::error!(
            "Verletzter Fremdschlüssel ({context}): Tabelle {}, rowid {}, Verweis auf {}",
            v.table,
            v.rowid.map_or_else(|| "-".to_string(), |r| r.to_string()),
            v.parent
        );
    }
}

/// Prüfung vor einer Umstellung (Lauf F2c): verletzte Fremdschlüssel und —
/// für die Umstellung auf Schema 32 — mehrere festgeschriebene Stornos zu
/// einem Original (dieselbe Bedingung wie der benannte CHECK in 0032). Ein
/// Befund lehnt ab, **bevor** eine Migration etwas speichert: die Datenbank
/// bleibt auf Schema `from`, das vorige Image öffnet sie weiter, ein Restore
/// bleibt möglich. Billig gegenüber dem Pflicht-Backup, das danach entsteht.
pub async fn precheck_before_migration(pool: &SqlitePool, from: i32) -> Result<()> {
    let violations = foreign_key_violations(pool).await?;
    if !violations.is_empty() {
        log_foreign_key_violations(
            &format!("vor der Umstellung von Schema {from}"),
            &violations,
        );
        let msg = format!(
            "{INTEGRITY_REFUSED}: {} verletzte(r) Fremdschlüssel vor der Umstellung, z. B. {}. \
             Die Datenbank steht unverändert auf Schema {from}; es wurde nichts umgestellt \
             (Rückweg: Image der vorigen Version; Handbuch: Fehlersuche im Container).",
            violations.len(),
            violations[0]
        );
        tracing::error!("{msg}");
        return Err(Error::Config(msg));
    }
    if from < 32 && duplicate_locked_stornos(pool).await? > 0 {
        let msg = format!(
            "{INTEGRITY_REFUSED}: Migration 0032 abgebrochen: Zu mindestens einer Rechnung gibt \
             es mehr als einen festgeschriebenen Storno-Beleg. Die Datenbank steht unverändert \
             auf Schema {from}; es wurde nichts umgestellt (Handbuch: Fehlersuche im Container)."
        );
        tracing::error!("{msg}");
        return Err(Error::Config(msg));
    }
    Ok(())
}

/// Meldung für verletzte Fremdschlüssel **nach** den Migrationen. Mit
/// `pending = Some(from)` ist die Umstellung schon gespeichert (das sagt die
/// Meldung, samt Rückweg über das Backup davor); ohne Umstellung wurden die
/// Daten nicht angefasst.
pub fn integrity_message_after(pending: Option<i32>, violations: &[ForeignKeyViolation]) -> String {
    let first = violations
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();
    match pending {
        Some(from) => format!(
            "{INTEGRITY_REFUSED}: {} verletzte(r) Fremdschlüssel {INTEGRITY_AFTER_MIGRATION} von \
             Schema {from} auf {}, z. B. {first}. Die Umstellung ist gespeichert; das Backup davor \
             (Schema {from}) liegt im Backup-Ordner. Rückweg: neue Instanz mit dem vorigen Image \
             und dieses Backup einspielen (Handbuch: Fehlersuche im Container).",
            violations.len(),
            schema_version::EXPECTED_SCHEMA_VERSION
        ),
        None => format!(
            "{INTEGRITY_REFUSED}: {} verletzte(r) Fremdschlüssel, z. B. {first}. Die Daten wurden \
             nicht angefasst (Handbuch: Fehlersuche im Container).",
            violations.len()
        ),
    }
}

/// Schema vor einer Umstellung, nach der noch kein Entsperren gelungen ist
/// (Lauf F3b): das jüngste Audit `backup.pre_migration` auf das aktuelle
/// Schema, ohne `backup.unlock.ok` danach. `None`, wenn es keines gibt.
/// Grundlage für die ehrliche Meldung „nach der Umstellung“ bei jedem
/// weiteren Versuch, nicht nur beim ersten.
pub async fn unfinished_migration_from(pool: &SqlitePool) -> Result<Option<i32>> {
    let details: Option<Option<String>> = sqlx::query_scalar(
        "SELECT details_json FROM audit_log
          WHERE action = 'backup.pre_migration'
            AND id > COALESCE(
                (SELECT MAX(id) FROM audit_log WHERE action = 'backup.unlock.ok'), 0)
          ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    let Some(json) = details.flatten() else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    let to = value.get("to").and_then(serde_json::Value::as_i64);
    let from = value.get("from").and_then(serde_json::Value::as_i64);
    Ok(match (from, to) {
        (Some(from), Some(to)) if to == i64::from(schema_version::EXPECTED_SCHEMA_VERSION) => {
            i32::try_from(from).ok()
        }
        _ => None,
    })
}

/// Zahl der Rechnungen mit mehr als einem festgeschriebenen Storno-Beleg —
/// dieselbe Bedingung wie die Vorab-Prüfung der Migration 0032 (dort in SQL).
/// Phase A eines Restores lehnt solche Backups ab, statt die Instanz nach dem
/// Tausch gesperrt zu lassen (ADR 0039 Nr. 124).
pub async fn duplicate_locked_stornos(pool: &SqlitePool) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
            SELECT is_storno_for FROM invoices
             WHERE is_storno_for IS NOT NULL AND locked_at IS NOT NULL
             GROUP BY is_storno_for HAVING COUNT(*) > 1
         )",
    )
    .fetch_one(pool)
    .await?)
}

/// Schließt einen beim Serverstart angewendeten Restore im ersten Entsperren
/// ab (Lauf F, ADR 0040):
///
/// 1. In **einer** Transaktion: Audit `backup.restore.applied`, Pause der
///    automatischen Belegerzeugung mit Grund `restore` (W14-023 — sie steht in
///    der eingespielten DB und übersteht damit Neustart und Sperren), das
///    Anheben der Nummernkreise auf die Stände der ersetzten DB, wo diese
///    höher waren (D-24, Audit `numbering.raised_after_restore`), und der
///    Abgleich der Abo-Termine, Marker und Entwürfe
///    ([`crate::backup::after_restore::apply_in`], F1d).
/// 2. Danach (best-effort): Hinweise zur Pause, zu den Nummernkreisen und den
///    Abo-Terminen, Pre-Restore-Backup und verwaiste Backups in die Historie
///    (D-22), Warnung zu fehlenden PDF-Vorlagen, `RESTORE_APPLIED.json`
///    entfernen.
async fn finish_restore(
    ctx: &crate::app_ctx::AppCtx,
    pool: &SqlitePool,
    info: &AppliedInfo,
) -> Result<()> {
    let now = ctx.clock().now_utc();
    let today = ctx.clock().today_berlin();
    let mut tx = begin_immediate(pool).await?;
    // F3b: Passt die eingespielte DB zur ersetzten? Höhere oder dort fehlende
    // Zählerstände bzw. dieselbe Belegnummer als anderer Datensatz heißen:
    // Die ersetzte DB war eine andere oder neu eingerichtete Instanz — Nummern
    // nach dem Backup können anderswo vergeben worden sein (vor dem Anheben
    // lesen, sonst sähe jeder angehobene Kreis gleich aus).
    let restored_counters = numbering::snapshot_in(&mut tx).await?;
    let ahead = numbering::counters_ahead(&info.counters, &restored_counters);
    let foreign_docs = crate::backup::after_restore::foreign_numbers_in(
        &mut tx,
        &info.locked_numbers,
        &info.locked_ids,
    )
    .await?;
    // Reihenfolge im Audit: Restore angewendet → je angehobenem Kreis →
    // Pause gesetzt.
    let mut raised = Vec::new();
    for c in &info.counters {
        if let Some(r) =
            numbering::raise_in(&mut tx, &c.doc_type, c.fiscal_year, c.last_value).await?
        {
            raised.push(r);
        }
    }
    crate::db::repo::audit_log::append_in(
        &mut tx,
        "backup.restore.applied",
        "backup",
        "restore",
        Some(serde_json::json!({
            "source": info.source,
            "staged_at": info.staged_at,
            "branding_restored": info.branding_restored,
            "pre_restore_backup": info.pre_restore_backup,
            "schema": info.schema_version,
            "backup_created_at": info.backup_created_at,
            "counters_raised": raised.len(),
            "counters_ahead": ahead.len(),
            "foreign_documents": foreign_docs,
        })),
    )
    .await?;
    for r in &raised {
        crate::db::repo::audit_log::append_in(
            &mut tx,
            "numbering.raised_after_restore",
            "numbering",
            &format!("{}:{}", r.doc_type, r.fiscal_year),
            Some(serde_json::json!({
                "doc_type": r.doc_type,
                "fiscal_year": r.fiscal_year,
                "from": r.from,
                "to": r.to,
            })),
        )
        .await?;
    }
    let followup =
        crate::backup::after_restore::apply_in(&mut tx, &info.series, &info.locked_numbers, today)
            .await?;
    let hold = crate::scheduler::hold::set_restore_in(&mut tx, now).await?;
    tx.commit().await?;
    tracing::info!(
        "Restore angewendet (Quelle: {}); automatische Abläufe pausiert bis zur Freigabe \
         ({} Abo-Reihe(n) vorgerückt, {} zu prüfen, {} Entwurf/Entwürfe vorgemerkt)",
        info.source,
        followup.advanced.len(),
        followup.review.len(),
        followup.conflicts.len()
    );

    crate::scheduler::hold::notice(pool, &hold).await;
    numbering_notice(
        pool,
        info,
        &NumberingState {
            raised: &raised,
            ahead: &ahead,
            foreign_docs: &foreign_docs,
        },
        &followup.conflicts,
    )
    .await;
    crate::backup::after_restore::notice(pool, &info.staged_at, &followup).await;
    // Das Pre-Restore-Backup stand nur in der verworfenen DB — hier in die
    // Historie übernehmen (Restore aus der Liste möglich).
    if let Err(e) = crate::backup::record_existing_backup(
        pool,
        &ctx.paths.backups_dir,
        &info.pre_restore_backup,
        "pre_restore",
    )
    .await
    {
        tracing::warn!("Pre-Restore-Backup nicht in die Historie übernommen: {e}");
    }
    // D-22: Backups zwischen eingespieltem Stand und Restore übernehmen.
    match crate::backup::rescan_floor(pool, &ctx.paths.backups_dir).await {
        Ok((added, removed)) if added > 0 || removed > 0 => tracing::info!(
            "Backup-Historie abgeglichen: {added} übernommen, {removed} fehlende entfernt"
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("Backup-Historie nicht abgeglichen: {e}"),
    }
    crate::backup::restore::warn_missing_templates(&ctx.paths, pool, &info.staged_at).await;
    crate::backup::restore::clear_applied(&ctx.paths);
    Ok(())
}

/// Nummernkreise im Restore-Abschluss: angehobene Kreise und die Befunde, dass
/// die ersetzte Datenbank nicht zum eingespielten Backup passt (F3b).
struct NumberingState<'a> {
    raised: &'a [numbering::Raised],
    /// Kreise der eingespielten DB über dem Stand der ersetzten (oder dort
    /// fehlend), siehe [`numbering::counters_ahead`].
    ahead: &'a [numbering::CounterState],
    /// Belegnummern, die in beiden Datenbanken verschiedene Datensätze tragen,
    /// siehe [`crate::backup::after_restore::foreign_numbers`].
    foreign_docs: &'a [String],
}

/// Gemeinsamer Teil der Texte „Nummern könnten anderswo vergeben sein“.
const NUMBERING_RAISE_ADVICE: &str = "Wurden danach außerhalb dieser Instanz weitere \
     Rechnungen, Stornos, Kosten oder andere Belege nummeriert (etwa auf einem ausgefallenen \
     Server), vergibt Klein.Buch diese Nummern sonst erneut. Prüfe den Mail-Ausgang bzw. die \
     Korrespondenz und hebe die betroffenen Nummernkreise unter Einstellungen → Backup → \
     Nummernkreise auf die zuletzt vergebene Nummer an (die Lücke ist dokumentiert, eine \
     Doppelvergabe wäre unzulässig).";

/// Hinweis zu den Nummernkreisen nach einem Restore (D-24). Angehobene Kreise
/// werden genannt; ohne Stände der ersetzten DB (Disaster Recovery in eine
/// frische Instanz) erklärt er, wie nachträglich vergebene Nummern zu
/// berücksichtigen sind; mit Ständen, aber ohne Anhebung, meldet er knapp
/// „unverändert“ (F1d). Vorgemerkte Entwürfe mit bereits festgeschriebener
/// Nummer werden immer genannt (F1d).
async fn numbering_notice(
    pool: &SqlitePool,
    info: &AppliedInfo,
    state: &NumberingState<'_>,
    conflicts: &[crate::backup::after_restore::Conflict],
) {
    let backup_date = info
        .backup_created_at
        .as_deref()
        .and_then(crate::clock::db_utc_to_berlin)
        .map(|b| b.format("%d.%m.%Y %H:%M").to_string())
        .unwrap_or_else(|| "unbekannt".into());
    let (body, severity) =
        numbering_notice_body(&backup_date, !info.counters.is_empty(), state, conflicts);
    let dedup = format!("restore_numbering:{}", info.staged_at);
    if let Err(e) = crate::notify::store::create(
        pool,
        crate::notify::NewNotification {
            title: "Nummernkreise nach der Wiederherstellung",
            body: &body,
            severity,
            action_url: Some("/settings/backup"),
            dedup_key: Some(&dedup),
            ..Default::default()
        },
    )
    .await
    {
        tracing::warn!("Hinweis zu den Nummernkreisen nicht angelegt: {e}");
    }
}

/// Text und Schwere des Nummernkreis-Hinweises (pure, F1d/F3b). Fälle:
///
/// - keine Stände der ersetzten DB (leere Instanz, Marker vor D-24) →
///   Anleitung zum Anheben (dringend);
/// - eingespielte Stände über denen der ersetzten DB oder dort fehlend, bzw.
///   dieselbe Belegnummer als anderer Datensatz → fremder Stand: dieselbe
///   Anleitung mit Begründung, angehobene Kreise zusätzlich genannt
///   (dringend) — „keine Lücke“ wäre hier falsch, der ausgefallene Server
///   kann nach dem Backup weiter nummeriert haben;
/// - Stände gleich (nichts angehoben, nichts voraus) → „keine Lücke, keine
///   Doppelvergabe“ (Info);
/// - sonst Liste der angehobenen Kreise (dringend).
///
/// Vorgemerkte Entwürfe machen jeden Fall dringend.
fn numbering_notice_body(
    backup_date: &str,
    has_snapshot: bool,
    state: &NumberingState<'_>,
    conflicts: &[crate::backup::after_restore::Conflict],
) -> (String, &'static str) {
    let raised_list = || {
        state
            .raised
            .iter()
            .map(|r| {
                format!(
                    "{} {} von {} auf {}",
                    label(&r.doc_type),
                    r.fiscal_year,
                    r.from,
                    r.to
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let foreign = !state.ahead.is_empty() || !state.foreign_docs.is_empty();
    let mut severity = "urgent";
    let mut body = if !has_snapshot {
        format!("Das eingespielte Backup hat den Stand vom {backup_date}. {NUMBERING_RAISE_ADVICE}")
    } else if foreign {
        let mut reasons = Vec::new();
        if !state.ahead.is_empty() {
            let list = state
                .ahead
                .iter()
                .take(10)
                .map(|c| format!("{} {}: {}", label(&c.doc_type), c.fiscal_year, c.last_value))
                .collect::<Vec<_>>()
                .join(", ");
            reasons.push(format!(
                "Im Backup stehen Nummernkreise höher als in der ersetzten Datenbank ({list})"
            ));
        }
        if !state.foreign_docs.is_empty() {
            let list = state
                .foreign_docs
                .iter()
                .take(10)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            reasons.push(format!(
                "Belege mit denselben Nummern sind dort andere Datensätze ({list})"
            ));
        }
        let mut text = format!(
            "Das eingespielte Backup hat den Stand vom {backup_date}. Es stammt nicht aus dieser \
             Instanz oder ist neuer als die ersetzte Datenbank: {}. Klein.Buch kennt deshalb die \
             Nummern nicht, die nach dem Backup vergeben wurden. {NUMBERING_RAISE_ADVICE}",
            reasons.join("; ")
        );
        if !state.raised.is_empty() {
            text.push_str(&format!(
                " Auf die Stände der ersetzten Datenbank angehoben wurden: {}.",
                raised_list()
            ));
        }
        text
    } else if state.raised.is_empty() {
        severity = "info";
        format!(
            "Das eingespielte Backup hat den Stand vom {backup_date}. Die Nummernkreise standen \
             dort schon auf denselben Ständen wie vor der Wiederherstellung — es fehlen keine \
             Nummern, und keine wird doppelt vergeben."
        )
    } else {
        format!(
            "Das eingespielte Backup hat den Stand vom {backup_date}. Danach vergebene Nummern \
             werden nicht erneut vergeben: Die Nummernkreise wurden auf die Stände vor der \
             Wiederherstellung angehoben ({}). Die dazwischen liegenden Belege fehlen in \
             dieser Datenbank — die Lücke ist im Audit-Log dokumentiert.",
            raised_list()
        )
    };
    if !conflicts.is_empty() {
        severity = "urgent";
        let list = conflicts
            .iter()
            .map(|c| c.number.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        body.push_str(&format!(
            "\n\nAchtung: Diese Entwürfe tragen Nummern, die vor der Wiederherstellung bereits \
             festgeschrieben waren und vermutlich versandt wurden: {list}. Sie sind ein älterer \
             Stand dieser Belege. Klein.Buch schreibt sie erst fest, wenn Du sie an das versandte \
             Original angepasst und unter Einstellungen → Backups → Nummernkreise bestätigt hast \
             (sonst gäbe es zwei verschiedene Belege mit derselben Nummer)."
        ));
    }
    (body, severity)
}

/// Anzeigename eines Nummernkreises.
pub fn label(doc_type: &str) -> &'static str {
    match doc_type {
        "invoice" => "Rechnungen (RE)",
        "storno_invoice" => "Stornos (ST)",
        "quote" => "Angebote (AN)",
        "expense" => "Kosten (KO)",
        "private_movement" => "Privatbewegungen (PV)",
        "asset" => "Anlagen (AV)",
        _ => "Nummernkreis",
    }
}

/// Öffnet den Pool (mit oder ohne SQLCipher-Key) und initialisiert ihn
/// (Migrationen + Schema-Check + Restore-Audit). Genutzt vom Onboarding
/// (`key = Some` → frische verschlüsselte DB) und vom Entsperren über den
/// Klartext-Übergangspfad (`key = None`).
pub async fn open_and_init(ctx: &crate::app_ctx::AppCtx, key: Option<&str>) -> Result<SqlitePool> {
    let paths = &ctx.paths;
    tracing::info!("Öffne DB: {}", paths.db_file.to_string_lossy());
    let pool = open_pool(&paths.db_file, key).await?;
    run_migrations(ctx, &pool).await?;
    Ok(pool)
}

/// `SQLITE_NOTADB` (26): mit falschem SQLCipher-Key scheitert schon der
/// Verbindungsaufbau (die Pragmas beim Connect lesen die Datei), nicht erst der
/// Probe-Read. Gilt beim Entsperren als „falsche Passphrase“.
pub fn is_not_a_database(err: &Error) -> bool {
    matches!(err, Error::Db(sqlx::Error::Database(db)) if db.code().as_deref() == Some("26"))
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

/// Entfernt eine Datei, falls vorhanden (No-op sonst). Ohne vorherige
/// `exists()`-Prüfung (Lauf F, LE-CI-FLAKE): SQLite löscht `-wal`/`-shm` beim
/// Schließen der letzten Verbindung selbst, und das Schließen kann nachlaufen —
/// zwischen Prüfung und Löschen verschwand die Datei, `remove_file` meldete
/// `NotFound` (Bestands-Migration, Passphrase-Wechsel).
pub(crate) fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => Ok(other?),
    }
}

/// G1-ENC Schritt 3 — Klartext-DB in eine SQLCipher-DB überführen; **nur noch
/// für Tests** dieses Moduls. Seit W6 gibt es keinen produktiven
/// Klartext-Übergangspfad mehr (ADR 0039 Nr. 57, E17); der gemeinsame Kern des
/// Neuverschlüsselns ist [`reencrypt_db_file`] (auch für [`rekey_db_file`]).
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
/// Voraussetzung: **kein offener Pool** auf `db_file` (eine offene Verbindung
/// arbeitete nach dem Tausch mit der alten Datei weiter).
/// Idempotent: ist die Datei bereits verschlüsselt (oder fehlt sie), passiert nichts.
#[cfg(test)]
pub(crate) async fn encrypt_db_file_in_place(db_file: &Path, passphrase: &str) -> Result<()> {
    if !db_file_is_plaintext(db_file) {
        // Bereits verschlüsselt (oder nicht vorhanden) → nichts zu tun.
        return Ok(());
    }
    reencrypt_db_file(db_file, None, passphrase).await
}

/// Verschlüsselt eine SQLCipher-DB mit neuer Passphrase (Plan W6 „Passphrase
/// ändern“): Export per `ATTACH … KEY` + `sqlcipher_export` in eine
/// Temp-Datei, Probe-Read mit der neuen Passphrase, dann atomarer Tausch. Kein
/// `PRAGMA rekey` (WAL, offene Verbindungen). Voraussetzung: kein offener
/// Pool auf `db_file`. Scheitert etwas vor dem Tausch, bleibt die Datei mit der
/// alten Passphrase unverändert.
pub async fn rekey_db_file(db_file: &Path, old: &str, new: &str) -> Result<()> {
    if !db_file.exists() || db_file_is_plaintext(db_file) {
        return Err(Error::Backup(
            "Neu verschlüsseln nur für eine vorhandene SQLCipher-Datenbank.".into(),
        ));
    }
    reencrypt_db_file(db_file, Some(old), new).await
}

/// Gemeinsamer Kern: Quelle ohne (`from_key = None`) oder mit Schlüssel öffnen,
/// in eine mit `passphrase` verschlüsselte Kopie exportieren, prüfen, tauschen.
async fn reencrypt_db_file(db_file: &Path, from_key: Option<&str>, passphrase: &str) -> Result<()> {
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
        use sqlx::ConnectOptions as _;
        let mut opts = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            // `ATTACH … KEY '<Passphrase>'` darf nicht im Log landen (S0-3).
            .log_statements(log::LevelFilter::Off)
            .log_slow_statements(log::LevelFilter::Off, std::time::Duration::from_secs(3600));
        if let Some(key) = from_key {
            opts = opts.pragma("key", sqlcipher_key_literal(key));
        }
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
        // Probe-Read und Integritätsprüfung der neuen Datei (Review W6).
        let ok = probe_readable(&check).await
            && sqlx::query_scalar::<_, String>("PRAGMA quick_check")
                .fetch_one(&check)
                .await
                .is_ok_and(|r| r == "ok");
        check.close().await;
        if !ok {
            remove_file_if_exists(&tmp)?;
            return Err(Error::Backup(
                "Verschlüsselte Kopie ließ sich nicht mit der Passphrase lesen — \
                 abgebrochen, Datenbank unverändert."
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
    // Inhalt auf die Platte, dann atomar tauschen und das Verzeichnis sichern.
    std::fs::File::open(&tmp)?.sync_all()?;
    std::fs::rename(&tmp, db_file)?;
    if let Some(dir) = db_file.parent() {
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter(doc_type: &str, fiscal_year: i64, last_value: i64) -> numbering::CounterState {
        numbering::CounterState {
            doc_type: doc_type.into(),
            fiscal_year,
            last_value,
        }
    }

    /// Lauf F3b: „keine Lücke“ nur bei gleichen Ständen; ein fremder Stand
    /// (höherer Zähler im Backup oder dieselbe Nummer als anderer Datensatz)
    /// bekommt die dringende Anleitung zum Anheben, auch neben angehobenen
    /// Kreisen.
    #[test]
    fn numbering_notice_info_only_for_identical_states() {
        let none = NumberingState {
            raised: &[],
            ahead: &[],
            foreign_docs: &[],
        };
        let (body, severity) = numbering_notice_body("30.09.2026 10:00", true, &none, &[]);
        assert_eq!(severity, "info");
        assert!(body.contains("keine wird doppelt vergeben"), "{body}");

        let (body, severity) = numbering_notice_body("30.09.2026 10:00", false, &none, &[]);
        assert_eq!(severity, "urgent");
        assert!(body.contains("Nummernkreise unter Einstellungen"), "{body}");

        let ahead = [counter("invoice", 2026, 10)];
        let foreign = NumberingState {
            raised: &[],
            ahead: &ahead,
            foreign_docs: &[],
        };
        let (body, severity) = numbering_notice_body("30.09.2026 10:00", true, &foreign, &[]);
        assert_eq!(severity, "urgent");
        assert!(!body.contains("keine wird doppelt vergeben"), "{body}");
        assert!(body.contains("Rechnungen (RE) 2026: 10"), "{body}");
        assert!(body.contains("Nummernkreise unter Einstellungen"), "{body}");

        let raised = [numbering::Raised {
            doc_type: "expense".into(),
            fiscal_year: 2026,
            from: 3,
            to: 5,
        }];
        let docs = ["RE-2026-0001".to_string()];
        let mixed = NumberingState {
            raised: &raised,
            ahead: &[],
            foreign_docs: &docs,
        };
        let (body, severity) = numbering_notice_body("30.09.2026 10:00", true, &mixed, &[]);
        assert_eq!(severity, "urgent");
        assert!(body.contains("andere Datensätze (RE-2026-0001)"), "{body}");
        assert!(body.contains("Kosten (KO) 2026 von 3 auf 5"), "{body}");
        assert!(body.contains("Nummernkreise unter Einstellungen"), "{body}");
    }

    /// Lauf F (LE-CI-FLAKE): fehlende Datei ist kein Fehler; eine Datei, die
    /// ein paralleler Schreiber anlegt und wieder entfernt, auch nicht.
    #[test]
    fn remove_file_if_exists_tolerates_vanished_files() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.sqlite-wal");
        remove_file_if_exists(&f).unwrap();
        std::fs::write(&f, b"x").unwrap();
        remove_file_if_exists(&f).unwrap();
        assert!(!f.exists());
        let racer = {
            let f = f.clone();
            std::thread::spawn(move || {
                for _ in 0..500 {
                    let _ = std::fs::write(&f, b"x");
                    let _ = std::fs::remove_file(&f);
                }
            })
        };
        for _ in 0..500 {
            remove_file_if_exists(&f).unwrap();
        }
        racer.join().unwrap();
        // Ein Verzeichnis ist kein „fehlt“: der Fehler kommt durch.
        let d = dir.path().join("dir");
        std::fs::create_dir(&d).unwrap();
        assert!(remove_file_if_exists(&d).is_err());
    }
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

    /// Schema-Hardline im Server-Betrieb (Lauf F, D-18; ADR 0003 Amendment
    /// 2026-10-24): Eine Datenbank, die neuer ist als das Programm, lässt sich
    /// nicht entsperren — weder mit einer unbekannten angewendeten Migration
    /// noch mit einer höheren `schema_version`. `run_migrations` ist der
    /// Schritt des Entsperrens vor dem Einhängen von Pool und Scheduler.
    #[tokio::test]
    async fn newer_database_is_rejected_on_unlock() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ctx = crate::app_ctx::AppCtx::for_tests(crate::config::Paths::under(dir.path()));
        std::fs::create_dir_all(&ctx.paths.data_dir).unwrap();
        let pool = open_pool(&ctx.paths.db_file, None).await.expect("open");
        run_migrations(&ctx, &pool).await.expect("frische DB");

        // höhere schema_version (alle Migrationen bekannt)
        sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'schema_version'")
            .bind((schema_version::EXPECTED_SCHEMA_VERSION + 1).to_string())
            .execute(&pool)
            .await
            .unwrap();
        let err = run_migrations(&ctx, &pool).await.unwrap_err();
        assert!(
            matches!(err, Error::SchemaMismatch { found, .. } if found == schema_version::EXPECTED_SCHEMA_VERSION + 1),
            "{err}"
        );

        // wie eine echte neuere DB: zusätzlich eine unbekannte, bereits
        // angewendete Migration — derselbe verständliche Hinweis, nicht die
        // Meldung des Migrations-Runners
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (99999, 'aus einer neueren Version', 1, x'00', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let err = run_migrations(&ctx, &pool).await.unwrap_err();
        assert!(matches!(err, Error::SchemaMismatch { .. }), "{err}");
        assert!(err.to_string().contains("Image"), "{err}");

        // nur die unbekannte Migration (Versionsstand gleich): abgelehnt
        sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'schema_version'")
            .bind(schema_version::EXPECTED_SCHEMA_VERSION.to_string())
            .execute(&pool)
            .await
            .unwrap();
        assert!(run_migrations(&ctx, &pool).await.is_err());
        pool.close().await;
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
