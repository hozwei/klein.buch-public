//! Drop-Folder-Sync (Block PV1-DROP) — **Imperative Shell**.
//!
//! Liest periodisch (Scheduler-Tick + App-Start-Sweep) den konfigurierten
//! Watched-Folder, klassifiziert jede Top-Level-Datei (siehe
//! [`crate::domain::drop_folder`]), schickt XML/PDF durch die *gleiche*
//! Empfangs-Pipeline wie der UI-Import (siehe
//! [`crate::commands::expenses::parse_einvoice_with_paths`] +
//! [`crate::commands::expenses::create_from_einvoice_with`]) und verschiebt
//! die Datei am Ende:
//!
//! - Erfolg: `processed/YYYY-MM/{file}` (Sub-Ordner nach Sync-Datum,
//!   [`crate::domain::drop_folder::processed_subdir`]).
//! - Fehler oder unbekannte Endung: `failed/{file}`; das Original bleibt
//!   liegen — kein Auto-Delete (ADR 0037 D-79; Manuel triagiert manuell).
//! - Versteckte Datei (`.DS_Store`, `Thumbs.db`, `*.tmp`): unangetastet.
//!
//! Per-Datei wird die Inbox bedient (Erfolg vs. Fehler) — Notification-Regeln
//! `rule_drop_folder_import_ok` (Inbox-only, Default off) und
//! `rule_drop_folder_import_failed` (Default on) entscheiden ueber die
//! Sichtbarkeit in der In-App-Inbox.
//!
//! **WICHTIG (Memory `project_g1_notify_backup` + R4-007):** Der Scheduler-Pfad
//! ist strikt **Inbox-only via `notify::store::create`** — kein `notify::emit`,
//! kein `os_native::push`. Sobald ein Integrationstest-Binary die OS-Push-Kette
//! verlinkt, scheitert es beim Laden (`STATUS_ENTRYPOINT_NOT_FOUND`,
//! `TaskDialogIndirect` in comctl32, vgl. ADR 0027 Pt. 5). Wer spaeter einen
//! OS-Toast fuer Fehler will, baut eine separate Reminder-Regel (Polling im
//! UI-Layer), nicht im Scheduler.
//!
//! Polling statt `notify`-Crate (ADR 0037 D-71): OneDrive-Quirks (verzoegerte
//! `read_dir`-Sichtbarkeit, Sync-Phantome) wuerden einen Live-Watcher
//! verwirren; 5-Minuten-Latenz reicht fuer „hier und da mal eine Rechnung".

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use sqlx::SqlitePool;

use crate::backup::BackupSession;
use crate::commands::expenses::{
    create_from_einvoice_with, parse_einvoice_with_paths, EInvoiceCreateArgs,
};
use crate::config::Paths;
use crate::db::repo::app_settings;
use crate::domain::drop_folder::{classify_file, processed_subdir, DropClassification};
use crate::error::{Error, Result};
use crate::notify::{store, NewNotification};

/// Zaehler eines `run_sync`-Laufs. `skipped_disabled` ⇒ Setting off oder kein
/// Pfad konfiguriert (alle anderen Felder = 0).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DropSyncReport {
    pub skipped_disabled: bool,
    /// Importierte E-Rechnungen (XML oder PDF). 1:1 mit angelegten Kosten.
    pub imported: usize,
    /// Datei wanderte nach `failed/` (Pipeline-Fehler ODER `IgnoreOther`).
    pub failed: usize,
    /// Versteckte System-/Sync-Artefakte; weder importiert noch verschoben.
    pub ignored_hidden: usize,
}

/// Liest die zwei Drop-Folder-Settings (`drop_folder_enabled`,
/// `drop_folder_path`) und liefert den Pfad **nur** zurueck, wenn:
/// - der Toggle auf "1" steht,
/// - der Pfad nicht leer ist,
/// - der Pfad als Verzeichnis existiert.
///
/// In allen anderen Faellen `Ok(None)` (kein Fehler) — die Schale ueberspringt
/// den Lauf und protokolliert in `tracing::debug`.
async fn read_settings(pool: &SqlitePool) -> Result<Option<PathBuf>> {
    let enabled = app_settings::get_bool(pool, "drop_folder_enabled", false).await?;
    if !enabled {
        return Ok(None);
    }
    let raw = app_settings::get(pool, "drop_folder_path")
        .await?
        .unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let p = PathBuf::from(trimmed);
    if !p.is_dir() {
        tracing::warn!(
            "drop_folder_path nicht gefunden oder kein Verzeichnis: {} — Sync uebersprungen.",
            p.display()
        );
        return Ok(None);
    }
    Ok(Some(p))
}

/// Ein Sync-Lauf. Idempotent: laeuft die App nach einem Tick erneut, sind die
/// bereits importierten Dateien schon in `processed/` bzw. `failed/` und werden
/// nicht erneut angefasst.
///
/// Keine `AppHandle`-Abhaengigkeit: der Scheduler-Pfad ist Inbox-only (siehe
/// Modul-Doku). UI-Trigger (Settings-Page-Button „Jetzt synchronisieren")
/// und Tick rufen dieselbe Signatur.
pub async fn run_sync(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    today: NaiveDate,
) -> Result<DropSyncReport> {
    let mut report = DropSyncReport::default();

    let Some(root) = read_settings(pool).await? else {
        report.skipped_disabled = true;
        return Ok(report);
    };

    // Ziel-Unterordner deterministisch (komponenten-weise via `join`, NIE als
    // "/"-String — Windows-Separator-Falle, Memory feedback_windows_path_separator_in_tests).
    let processed_rel = processed_subdir(today); // "processed/YYYY-MM"
    let (processed_top, processed_month) = processed_rel
        .split_once('/')
        .expect("processed_subdir liefert immer 'processed/YYYY-MM'");
    let processed_dir = root.join(processed_top).join(processed_month);
    let failed_dir = root.join("failed");

    // Verzeichnisse anlegen (idempotent). Schlaegt das fehl, ist der Lauf
    // ganz dahin — wir geben einen Hard-Error zurueck, statt Files stumm
    // liegenzulassen.
    std::fs::create_dir_all(&processed_dir).map_err(|e| {
        Error::Config(format!(
            "Drop-Folder: 'processed/'-Unterordner konnte nicht angelegt werden ({}): {e}",
            processed_dir.display()
        ))
    })?;
    std::fs::create_dir_all(&failed_dir).map_err(|e| {
        Error::Config(format!(
            "Drop-Folder: 'failed/'-Ordner konnte nicht angelegt werden ({}): {e}",
            failed_dir.display()
        ))
    })?;

    let read_dir = std::fs::read_dir(&root).map_err(|e| {
        Error::Config(format!(
            "Drop-Folder konnte nicht gelesen werden ({}): {e}",
            root.display()
        ))
    })?;

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Drop-Folder: read_dir-Eintrag uebersprungen: {e}");
                continue;
            }
        };
        let path = entry.path();
        // Top-Level only (ADR 0037, kein rekursives Scannen).
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    "Drop-Folder: file_type fuer '{}' nicht lesbar: {e}",
                    path.display()
                );
                continue;
            }
        };
        if !ft.is_file() {
            continue;
        }
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => {
                tracing::warn!(
                    "Drop-Folder: Dateiname ohne gueltiges UTF-8 uebersprungen: {}",
                    path.display()
                );
                continue;
            }
        };

        match classify_file(&file_name) {
            DropClassification::IgnoreHidden => {
                report.ignored_hidden += 1;
            }
            DropClassification::IgnoreOther => {
                // Unbekannte Endung -> Original nach failed/ verschieben.
                if let Err(e) = move_into(&path, &failed_dir, &file_name) {
                    tracing::warn!(
                        "Drop-Folder: Verschieben nach failed/ fehlgeschlagen ({}): {e}",
                        path.display()
                    );
                    continue;
                }
                report.failed += 1;
                inbox_failed(
                    pool,
                    &file_name,
                    "unbekannter Dateityp (nur .xml oder .pdf)",
                )
                .await;
            }
            DropClassification::Xml | DropClassification::Pdf => {
                match import_einvoice_from_file(pool, paths, session, &path, &file_name).await {
                    Ok(expense_id) => {
                        if let Err(e) = move_into(&path, &processed_dir, &file_name) {
                            tracing::warn!(
                                "Drop-Folder: Verschieben nach processed/ fehlgeschlagen ({}): {e}",
                                path.display()
                            );
                            // Import war erfolgreich (Beleg + Archiv stehen) —
                            // der File-Move ist nachgelagert. Wir zaehlen ihn
                            // dennoch als Erfolg; das File bleibt halt liegen.
                        }
                        report.imported += 1;
                        inbox_ok(pool, &file_name, &expense_id).await;
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        if let Err(mv) = move_into(&path, &failed_dir, &file_name) {
                            tracing::warn!(
                                "Drop-Folder: Verschieben nach failed/ fehlgeschlagen ({}): {mv}",
                                path.display()
                            );
                            continue;
                        }
                        // Side-File mit der Fehlermeldung daneben, damit
                        // Manuel die Ursache sieht, ohne ins Audit-Log zu
                        // muessen. Best-effort; Fehler nicht eskalieren.
                        let err_file = failed_dir.join(format!("{file_name}.error.txt"));
                        if let Err(ew) = std::fs::write(&err_file, &err_msg) {
                            tracing::warn!(
                                "Drop-Folder: error.txt nicht schreibbar ({}): {ew}",
                                err_file.display()
                            );
                        }
                        report.failed += 1;
                        inbox_failed(pool, &file_name, &err_msg).await;
                    }
                }
            }
        }
    }

    if report.imported > 0 || report.failed > 0 {
        tracing::info!(
            "Drop-Folder-Sync abgeschlossen: {} importiert, {} fehlerhaft, {} ignoriert.",
            report.imported,
            report.failed,
            report.ignored_hidden
        );
    }
    Ok(report)
}

/// Headless-Pipeline-Funktion: liest eine einzelne Datei und schickt sie durch
/// die *gleichen* Helfer wie der UI-Import. Gibt bei Erfolg die `expense_id`
/// zurueck — der Aufrufer (run_sync) macht das File-Move + Notification.
///
/// Nutzt [`parse_einvoice_with_paths`] (Schritt 1, ohne Persistenz) +
/// [`create_from_einvoice_with`] (Schritt 2, Archiv + Expense + Audit). Es
/// gibt **keine** zweite Pipeline und keinen abgekuerzten Pfad — das ist
/// genau die Anforderung aus ADR 0037 D-74.
pub async fn import_einvoice_from_file(
    pool: &SqlitePool,
    paths: &Paths,
    session: &BackupSession,
    file: &Path,
    file_name: &str,
) -> Result<String> {
    let bytes = std::fs::read(file).map_err(|e| {
        Error::Domain(format!(
            "Datei konnte nicht gelesen werden ({}): {e}",
            file.display()
        ))
    })?;
    let parsed = parse_einvoice_with_paths(paths, bytes.clone(), file_name.to_string()).await?;
    let detail = create_from_einvoice_with(
        pool,
        paths,
        session,
        EInvoiceCreateArgs {
            input: parsed.input,
            fiscal_year: None,
            original_bytes: bytes,
            original_file_name: file_name.to_string(),
            source_format: parsed.source_format,
            validation: parsed.validation,
        },
    )
    .await?;
    Ok(detail.expense.id)
}

/// Verschiebt `src` nach `target_dir/{file_name}`, mit Counter-Suffix bei
/// Namens-Kollision (`{stem}-2.{ext}`, `{stem}-3.{ext}`, …) bis 99. Erst
/// `rename` versuchen (atomar bei gleichem Filesystem); fallback auf
/// `copy` + `remove_file` ueber Filesystem-Grenzen (USB-Drop-Folder, NAS).
fn move_into(src: &Path, target_dir: &Path, file_name: &str) -> std::io::Result<()> {
    let target = unique_target(target_dir, file_name);
    if let Err(rename_err) = std::fs::rename(src, &target) {
        // Cross-device rename schlaegt mit ErrorKind::CrossesDevices oder
        // PermissionDenied fehl — wir kopieren manuell.
        std::fs::copy(src, &target)?;
        std::fs::remove_file(src).map_err(|e| {
            tracing::warn!(
                "Cross-device move: Original konnte nicht geloescht werden ({}): {e}; \
                 ursprueglicher rename-Fehler: {rename_err}",
                src.display()
            );
            e
        })?;
    }
    Ok(())
}

fn unique_target(dir: &Path, file_name: &str) -> PathBuf {
    let primary = dir.join(file_name);
    if !primary.exists() {
        return primary;
    }
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, Some(e)),
        _ => (file_name, None),
    };
    for n in 2..=99 {
        let candidate = match ext {
            Some(e) => dir.join(format!("{stem}-{n}.{e}")),
            None => dir.join(format!("{stem}-{n}")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    // Fallback: zeitstempel-suffix, damit wir nichts ueberschreiben.
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    match ext {
        Some(e) => dir.join(format!("{stem}-{stamp}.{e}")),
        None => dir.join(format!("{stem}-{stamp}")),
    }
}

/// Inbox-only (KEIN `notify::emit`, KEIN OS-Push) — siehe Modul-Doku
/// und Memory `project_g1_notify_backup`. Linkage gegen `os_native::push`
/// wuerde Test-Binaries beim Laden zerstoeren.
async fn inbox_ok(pool: &SqlitePool, file_name: &str, expense_id: &str) {
    let body = format!(
        "Datei \"{file_name}\" wurde aus dem Rechnungs-Eingang als Eingangsbeleg \
         übernommen und im Archiv gesichert."
    );
    let action = format!("/expenses/{expense_id}");
    if let Err(e) = store::create(
        pool,
        NewNotification {
            rule_id: Some("rule_drop_folder_import_ok"),
            title: "Rechnung aus dem Rechnungs-Eingang übernommen",
            body: &body,
            severity: "info",
            related_entity_type: Some("expense"),
            related_entity_id: Some(expense_id),
            action_url: Some(&action),
            dedup_key: None,
        },
    )
    .await
    {
        tracing::warn!("Drop-Folder: import_ok-Notification fehlgeschlagen: {e}");
    }
}

async fn inbox_failed(pool: &SqlitePool, file_name: &str, reason: &str) {
    let body = format!(
        "Datei \"{file_name}\" konnte nicht übernommen werden: {reason}. \
         Die Datei liegt jetzt im Unterordner failed/, bitte prüfen."
    );
    if let Err(e) = store::create(
        pool,
        NewNotification {
            rule_id: Some("rule_drop_folder_import_failed"),
            title: "Rechnungs-Eingang: Übernahme fehlgeschlagen",
            body: &body,
            severity: "warning",
            related_entity_type: None,
            related_entity_id: None,
            action_url: Some("/settings/drop-folder"),
            dedup_key: None,
        },
    )
    .await
    {
        tracing::warn!("Drop-Folder: import_failed-Notification fehlgeschlagen: {e}");
    }
}
