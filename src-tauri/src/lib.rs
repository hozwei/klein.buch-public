//! Klein.Buch — Lokale EÜR-Buchhaltung für deutsche §19-Kleinunternehmer.
//!
//! Architektur: Functional Core (domain, einvoice::{parser,generator},
//! pdf::klausel_check, depreciation::compute, euer::aggregate) +
//! Imperative Shell (commands, db, archive, mail, scheduler, sidecar bridges,
//! backup, migration_export).

pub mod archive;
pub mod assets;
pub mod backup;
pub mod branding;
pub mod commands;
pub mod config;
pub mod db;
pub mod depreciation;
pub mod domain;
pub mod einvoice;
pub mod error;
pub mod euer;
pub mod fiscal_year;
pub mod mail;
pub mod migration_export;
pub mod notify;
pub mod pdf;
pub mod scheduler;

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging: Default INFO, klein_buch_lib DEBUG, override via RUST_LOG.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,klein_buch_lib=debug"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!("Klein.Buch v{} startet", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            // Block 4: Session-Passphrase-Halter (nur Memory) in den State.
            app.manage(backup::BackupSession::default());
            // G1-ENC Schritt 2 (Bootstrap-Inversion): Zwischenspeicher für einen
            // beim Start angewendeten Restore-Audit + Einmal-Guard für den
            // Scheduler. Der DB-Pool wird hier NICHT mehr geöffnet — er entsteht
            // erst nach Passphrase-Eingabe (Onboarding/Unlock), weil eine
            // verschlüsselte DB ohne Passphrase nicht zu öffnen ist.
            app.manage(db::PendingRestoreAudit::default());
            app.manage(scheduler::tick::SchedulerStarted::default());

            let handle = app.handle().clone();
            // Nur Dateisystem vorbereiten (Verzeichnisse + vorgemerkter Restore)
            // — ohne Pool. Onboarding/Unlock öffnen den Pool danach.
            tauri::async_runtime::spawn(async move {
                if let Err(err) = db::prepare_filesystem(&handle) {
                    tracing::error!("Dateisystem-Vorbereitung fehlgeschlagen: {err:#}");
                    // Kein exit(1): das Backup-Gate (Onboarding/Unlock) macht den
                    // Fehler sichtbar, statt die App stumm zu beenden.
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::contacts::contacts_list,
            commands::contacts::contacts_get,
            commands::contacts::contacts_create,
            commands::contacts::contacts_update,
            commands::contacts::contacts_archive,
            commands::contacts::contacts_unarchive,
            commands::contacts::contacts_search,
            commands::contacts::contacts_anonymize,
            commands::contacts::contacts_anonymize_check,
            commands::settings::seller_profile_get,
            commands::settings::seller_profile_upsert,
            commands::settings::seller_logo_set,
            commands::settings::seller_logo_clear,
            commands::settings::seller_logo_data,
            commands::settings::pdf_templates_list,
            commands::settings::pdf_template_preview,
            commands::settings::seller_default_template_set,
            commands::settings::paragraph_19_info,
            // Block P1: Anfahrtsrechner
            commands::settings::travel_settings_get,
            commands::settings::travel_settings_set,
            commands::settings::travel_compute,
            // Block „Angebots-Signatur"
            commands::settings::seller_signature_set,
            commands::settings::seller_signature_clear,
            commands::settings::seller_signature_data,
            commands::settings::quote_signature_get,
            commands::settings::quote_signature_set,
            commands::settings::seller_owner_get,
            commands::settings::seller_owner_set,
            commands::settings::document_terms_get,
            commands::settings::document_terms_set,
            // Block PV1-DROP: Drop-Folder fuer eingehende E-Rechnungen
            commands::settings::drop_folder_settings_get,
            commands::settings::drop_folder_settings_set,
            commands::settings::drop_folder_sync_now,
            // Block 3b: Invoice-Pipeline
            commands::invoices::invoices_list,
            commands::invoices::invoices_get,
            commands::invoices::invoices_create_draft,
            commands::invoices::invoices_validate_draft,
            commands::invoices::invoices_lock_and_issue,
            commands::invoices::invoices_record_payment,
            commands::invoices::invoices_cancel,
            // Block 3c: PDF-Zugriff
            commands::invoices::invoices_open_pdf,
            commands::invoices::invoices_reveal_pdf,
            commands::invoices::invoices_archive_paths,
            // Block 4: Backup + Restore
            commands::backup::backup_needs_onboarding,
            commands::backup::backup_is_unlocked,
            commands::backup::backup_get_settings,
            commands::backup::backup_setup_passphrase,
            commands::backup::backup_unlock,
            commands::backup::backup_create_now,
            commands::backup::backup_set_target,
            commands::backup::backup_test_sftp,
            commands::backup::backup_set_sftp_target,
            commands::backup::backup_list,
            commands::backup::backup_log_list,
            commands::backup::backup_log_search,
            commands::backup::backup_open_folder,
            commands::backup::backup_reveal_path,
            commands::backup::backup_restore_preview,
            commands::backup::backup_restore_apply,
            // Block 4: Migrations-Export
            commands::migration_export::migration_export_run,
            // Block 5: Mail-Versand (SMTP)
            commands::mail::mail_accounts_list,
            commands::mail::mail_account_create,
            commands::mail::mail_account_update,
            commands::mail::mail_account_delete,
            commands::mail::mail_account_test_connection,
            commands::mail::mail_send_test,
            commands::mail::mail_invoice_preview,
            commands::mail::mail_send_invoice,
            commands::mail::mail_quote_preview,
            commands::mail::mail_send_quote,
            // Block 16: OAuth Microsoft Exchange Online
            commands::mail::mail_oauth_status,
            commands::mail::mail_oauth_connect,
            commands::mail::mail_oauth_disconnect,
            // Block 16b: E-Mail-Versandprotokoll
            commands::mail::email_log_list,
            commands::mail::email_log_for,
            commands::mail::email_log_search,
            // Block 6: Angebote (CRUD + Annahme-Workflow)
            commands::quotes::quotes_list,
            commands::quotes::quotes_get,
            commands::quotes::quotes_attachments_list,
            commands::quotes::quotes_create_draft,
            commands::quotes::quotes_validate_draft,
            commands::quotes::quotes_issue,
            commands::quotes::quotes_accept,
            commands::quotes::quotes_reject,
            commands::quotes::quotes_cancel,
            // Block 7: Angebot → Rechnung-Konvertierung
            commands::quotes::quotes_convert_to_invoice,
            // Block 8: Angebots-PDF + Bundle + Rechtsdokumente
            commands::quotes::quotes_generate_pdf,
            commands::quotes::quotes_open_bundle,
            commands::quotes::quotes_legal_bindings,
            commands::legal_documents::legal_documents_list,
            commands::legal_documents::legal_documents_upload,
            commands::legal_documents::legal_documents_activate,
            commands::legal_documents::legal_documents_deactivate,
            // Block 18: DSGVO-Auskunft (Art. 15) — read-only Export
            commands::dsgvo::dsgvo_export,
            // Block 6: Anhänge öffnen/anzeigen
            commands::attachments::attachments_open,
            commands::attachments::attachments_reveal,
            // Block 9: Anhänge generisch hochladen/listen
            commands::attachments::attachments_add,
            commands::attachments::attachments_list,
            // Block 9: Zahlungs-Konten
            commands::payment_accounts::payment_accounts_list,
            commands::payment_accounts::payment_accounts_ensure_defaults,
            commands::payment_accounts::payment_accounts_create,
            commands::payment_accounts::payment_accounts_update,
            commands::payment_accounts::payment_accounts_set_active,
            // Block 9: Kosten
            commands::expenses::expenses_list,
            commands::expenses::expenses_get,
            commands::expenses::expenses_create,
            commands::expenses::expenses_cancel,
            commands::expenses::expenses_set_payment,
            // Block 11: E-Rechnung-Empfang
            commands::expenses::expenses_parse_einvoice,
            commands::expenses::expenses_create_from_einvoice,
            // PV1-A5: Roh-XML-Viewer für empfangene E-Rechnungen
            commands::expenses::expenses_receipt_xml_text,
            // Block 9: Privatbewegungen
            commands::private_movements::private_movements_list,
            commands::private_movements::private_movements_get,
            commands::private_movements::private_movements_create,
            // Block 10: Wiederkehrende Abos + Scheduler
            commands::recurring::recurring_list,
            commands::recurring::recurring_get,
            commands::recurring::recurring_create,
            commands::recurring::recurring_update,
            commands::recurring::recurring_set_active,
            commands::recurring::recurring_run_now,
            commands::recurring::recurring_run_due_check,
            // Block 12: Anlagenverzeichnis + AfA
            commands::assets::assets_list,
            commands::assets::assets_get,
            commands::assets::assets_afa_table,
            commands::assets::assets_suggest_method,
            commands::assets::assets_create,
            commands::assets::assets_update,
            commands::assets::assets_dispose,
            commands::depreciation::depreciation_accrue_year,
            commands::depreciation::depreciation_reset_asset,
            commands::depreciation::depreciation_list_for_year,
            // Block 13: EÜR-Auswertung (Cash-Basis)
            commands::euer::euer_compute_report,
            commands::euer::euer_available_years,
            // Block 14a: ELSTER-Ausfüllhilfe + Einzelaufstellung + AVEÜR (Anlage EÜR)
            commands::euer::euer_package,
            commands::euer::euer_export_elster,
            commands::euer::euer_export_detail_zip,
            commands::euer::euer_export_pdf,
            commands::euer::euer_export_datev,
            commands::euer::euer_export_stb_zip,
            commands::euer::euer_afa_pending,
            commands::euer::euer_reveal_path,
            // Block 15: Notifications + Reminder-Regeln
            commands::notifications::notifications_list,
            commands::notifications::notifications_unread_count,
            commands::notifications::notifications_dismiss,
            commands::notifications::notifications_dismiss_all,
            commands::notifications::notification_rules_list,
            commands::notifications::notification_rules_set_enabled,
            commands::notifications::notifications_run_checks,
            // Block 15: Geschäftsjahr-Abschluss (GJ-Lock) + Auto-AfA-Schalter
            commands::fiscal_year::fiscal_year_overview,
            commands::fiscal_year::fiscal_year_close,
            commands::fiscal_year::fiscal_year_closed_list,
            commands::fiscal_year::fiscal_year_auto_close_get,
            commands::fiscal_year::fiscal_year_auto_close_set,
            // Block 15: Audit-Trail-Einsicht + manueller Integrity-Check
            commands::system::audit_trail_list,
            commands::system::archive_integrity_run,
            commands::system::archive_integrity_history,
            // Block G2-DOC.3.5: „Über"-Dialog (App-Info + Drittanbieter-Lizenzen)
            commands::system::app_info,
            commands::system::third_party_licenses_path,
            // Block G1-RESET: Factory Reset (ADR 0036)
            commands::factory_reset::factory_reset_check,
            commands::factory_reset::factory_reset,
            // Block P2: Paket-Katalog (Kategorien + Pakete + append-only Revisionen)
            commands::packages::package_categories_list,
            commands::packages::package_category_create,
            commands::packages::package_category_update,
            commands::packages::package_categories_reorder,
            commands::packages::packages_list,
            commands::packages::packages_get,
            commands::packages::packages_create,
            commands::packages::packages_update_as_new_revision,
            commands::packages::packages_archive,
            commands::packages::packages_reactivate,
            commands::packages::package_revisions_list,
            commands::packages::package_revisions_get,
            commands::packages::package_revisions_rollback,
            commands::packages::package_preview,
            // Block P3: Paket-Position in Angebot/Rechnung materialisieren.
            commands::packages::package_materialize_item,
            // Block P4: Paket-Katalog-Broschüre (kein §14-Beleg).
            commands::packages::package_catalog_render,
            commands::packages::package_catalog_send,
            // Block RI: Wiederkehrende Ausgangsrechnungen (Abo-Rechnungen)
            commands::recurring_invoice::recurring_invoices_list,
            commands::recurring_invoice::recurring_invoices_get,
            commands::recurring_invoice::recurring_invoices_create,
            commands::recurring_invoice::recurring_invoices_update,
            commands::recurring_invoice::recurring_invoices_set_active,
            commands::recurring_invoice::recurring_invoices_run_now,
            commands::recurring_invoice::recurring_invoices_run_due_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
