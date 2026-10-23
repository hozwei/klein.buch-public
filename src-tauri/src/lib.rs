//! Klein.Buch — Lokale EÜR-Buchhaltung für deutsche §19-Kleinunternehmer.
//!
//! Architektur: Functional Core (domain, einvoice::{parser,generator},
//! pdf::klausel_check, depreciation::compute, euer::aggregate) +
//! Imperative Shell (commands, db, archive, mail, scheduler, sidecar bridges,
//! backup, migration_export).

pub mod app_ctx;
pub mod archive;
pub mod assets;
pub mod backup;
pub mod branding;
pub mod clock;
pub mod commands;
pub mod config;
pub mod db;
pub mod depreciation;
pub mod domain;
pub mod einvoice;
pub mod error;
pub mod euer;
pub mod fiscal_year;
pub mod limits;
pub mod locks;
pub mod mail;
pub mod migration_export;
pub mod notify;
pub mod pdf;
pub mod scheduler;
pub mod secrets;
pub mod tauri_shell;

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging: Default INFO, klein_buch_lib DEBUG, override via RUST_LOG.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,klein_buch_lib=debug"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tracing::info!("Klein.Buch v{} startet", env!("CARGO_PKG_VERSION"));

    // Plan W1.3: Sidecar-Mock nur in Debug-/Test-Builds; ein Release-Build mit
    // gesetztem Schalter verweigert den Start.
    let mock_env = std::env::var(einvoice::SIDECAR_MOCK_ENV).ok();
    if einvoice::mock_policy(cfg!(debug_assertions), mock_env.as_deref())
        == einvoice::MockPolicy::RefuseStart
    {
        tracing::error!(
            "{} ist in Release-Builds nicht erlaubt — Start abgebrochen.",
            einvoice::SIDECAR_MOCK_ENV
        );
        std::process::exit(1);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use std::sync::Arc;
            use tauri::Manager;
            // Plan W4: ein `AppCtx` statt einzelner Tauri-States (Pool, Session,
            // Restore-Audit, Scheduler-Guard). Der DB-Pool entsteht erst nach
            // Passphrase-Eingabe (Onboarding/Unlock), weil eine verschlüsselte DB
            // ohne Passphrase nicht zu öffnen ist (G1-ENC, Bootstrap-Inversion).
            let paths = tauri_shell::paths_from_handle(app.handle())?;
            let ctx = Arc::new(app_ctx::AppCtx::new(
                paths,
                Arc::new(tauri_shell::TauriNotifier(app.handle().clone())),
                Arc::new(secrets::KeyringSecretStore),
                Arc::new(tauri_shell::special::TauriDesktop(app.handle().clone())),
                app.config().identifier.clone(),
            ));
            // Nur Dateisystem vorbereiten (Verzeichnisse + vorgemerkter Restore)
            // — ohne Pool. Synchron (Review W2 S1-3): der Restore-Tausch muss
            // fertig sein, bevor das Frontend entsperren kann.
            if let Err(err) = db::prepare_filesystem(&ctx) {
                tracing::error!("Dateisystem-Vorbereitung fehlgeschlagen: {err:#}");
                // Kein exit(1): das Backup-Gate (Onboarding/Unlock) macht den
                // Fehler sichtbar, statt die App stumm zu beenden.
            }
            app.manage(ctx);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_shell::cmd::assets::assets_list,
            tauri_shell::cmd::assets::assets_get,
            tauri_shell::cmd::assets::assets_afa_table,
            tauri_shell::cmd::assets::assets_suggest_method,
            tauri_shell::cmd::assets::assets_create,
            tauri_shell::cmd::assets::assets_update,
            tauri_shell::cmd::assets::assets_dispose,
            tauri_shell::cmd::attachments::attachments_open,
            tauri_shell::cmd::attachments::attachments_reveal,
            tauri_shell::cmd::attachments::attachments_list,
            tauri_shell::cmd::attachments::attachments_add,
            tauri_shell::cmd::backup::backup_needs_onboarding,
            tauri_shell::cmd::backup::backup_is_unlocked,
            tauri_shell::cmd::backup::backup_get_settings,
            tauri_shell::cmd::backup::backup_setup_passphrase,
            tauri_shell::cmd::backup::backup_unlock,
            tauri_shell::cmd::backup::backup_create_now,
            tauri_shell::cmd::backup::backup_set_target,
            tauri_shell::cmd::backup::backup_test_sftp,
            tauri_shell::cmd::backup::backup_set_sftp_target,
            tauri_shell::cmd::backup::backup_open_folder,
            tauri_shell::cmd::backup::backup_reveal_path,
            tauri_shell::cmd::backup::backup_list,
            tauri_shell::cmd::backup::backup_log_list,
            tauri_shell::cmd::backup::backup_log_search,
            tauri_shell::cmd::backup::backup_restore_preview,
            tauri_shell::cmd::backup::backup_restore_apply,
            tauri_shell::cmd::contacts::contacts_list,
            tauri_shell::cmd::contacts::contacts_get,
            tauri_shell::cmd::contacts::contacts_create,
            tauri_shell::cmd::contacts::contacts_update,
            tauri_shell::cmd::contacts::contacts_archive,
            tauri_shell::cmd::contacts::contacts_unarchive,
            tauri_shell::cmd::contacts::contacts_search,
            tauri_shell::cmd::contacts::contacts_anonymize_check,
            tauri_shell::cmd::contacts::contacts_anonymize,
            tauri_shell::cmd::depreciation::depreciation_accrue_year,
            tauri_shell::cmd::depreciation::depreciation_reset_asset,
            tauri_shell::cmd::depreciation::depreciation_list_for_year,
            tauri_shell::cmd::dsgvo::dsgvo_export,
            tauri_shell::cmd::euer::euer_compute_report,
            tauri_shell::cmd::euer::euer_available_years,
            tauri_shell::cmd::euer::euer_package,
            tauri_shell::cmd::euer::euer_export_detail_zip,
            tauri_shell::cmd::euer::euer_export_elster,
            tauri_shell::cmd::euer::euer_export_pdf,
            tauri_shell::cmd::euer::euer_export_datev,
            tauri_shell::cmd::euer::euer_export_stb_zip,
            tauri_shell::cmd::euer::euer_afa_pending,
            tauri_shell::cmd::euer::euer_reveal_path,
            tauri_shell::cmd::expenses::expenses_list,
            tauri_shell::cmd::expenses::expenses_get,
            tauri_shell::cmd::expenses::expenses_create,
            tauri_shell::cmd::expenses::expenses_cancel,
            tauri_shell::cmd::expenses::expenses_set_payment,
            tauri_shell::cmd::expenses::expenses_parse_einvoice,
            tauri_shell::cmd::expenses::expenses_create_from_einvoice,
            tauri_shell::cmd::expenses::expenses_receipt_xml_text,
            tauri_shell::cmd::factory_reset::factory_reset_check,
            tauri_shell::cmd::factory_reset::factory_reset,
            tauri_shell::cmd::fiscal_year::fiscal_year_overview,
            tauri_shell::cmd::fiscal_year::fiscal_year_close,
            tauri_shell::cmd::fiscal_year::fiscal_year_closed_list,
            tauri_shell::cmd::fiscal_year::fiscal_year_auto_close_get,
            tauri_shell::cmd::fiscal_year::fiscal_year_auto_close_set,
            tauri_shell::cmd::invoices::invoices_list,
            tauri_shell::cmd::invoices::invoices_get,
            tauri_shell::cmd::invoices::invoices_open_pdf,
            tauri_shell::cmd::invoices::invoices_reveal_pdf,
            tauri_shell::cmd::invoices::invoices_create_draft,
            tauri_shell::cmd::invoices::invoices_validate_draft,
            tauri_shell::cmd::invoices::invoices_lock_and_issue,
            tauri_shell::cmd::invoices::invoices_record_payment,
            tauri_shell::cmd::invoices::invoices_cancel,
            tauri_shell::cmd::legal_documents::legal_documents_list,
            tauri_shell::cmd::legal_documents::legal_documents_upload,
            tauri_shell::cmd::legal_documents::legal_documents_activate,
            tauri_shell::cmd::legal_documents::legal_documents_deactivate,
            tauri_shell::cmd::mail::mail_accounts_list,
            tauri_shell::cmd::mail::mail_account_create,
            tauri_shell::cmd::mail::mail_account_update,
            tauri_shell::cmd::mail::mail_account_delete,
            tauri_shell::cmd::mail::mail_account_test_connection,
            tauri_shell::cmd::mail::mail_oauth_status,
            tauri_shell::cmd::mail::mail_oauth_connect,
            tauri_shell::cmd::mail::mail_oauth_disconnect,
            tauri_shell::cmd::mail::email_log_list,
            tauri_shell::cmd::mail::email_log_for,
            tauri_shell::cmd::mail::email_log_search,
            tauri_shell::cmd::mail::mail_invoice_preview,
            tauri_shell::cmd::mail::mail_send_invoice,
            tauri_shell::cmd::mail::mail_send_test,
            tauri_shell::cmd::mail::mail_quote_preview,
            tauri_shell::cmd::mail::mail_send_quote,
            tauri_shell::cmd::migration_export::migration_export_run,
            tauri_shell::cmd::notifications::notifications_list,
            tauri_shell::cmd::notifications::notifications_unread_count,
            tauri_shell::cmd::notifications::notifications_dismiss,
            tauri_shell::cmd::notifications::notifications_dismiss_all,
            tauri_shell::cmd::notifications::notification_rules_list,
            tauri_shell::cmd::notifications::notification_rules_set_enabled,
            tauri_shell::cmd::notifications::notifications_run_checks,
            tauri_shell::cmd::packages::package_categories_list,
            tauri_shell::cmd::packages::package_category_create,
            tauri_shell::cmd::packages::package_category_update,
            tauri_shell::cmd::packages::package_categories_reorder,
            tauri_shell::cmd::packages::packages_list,
            tauri_shell::cmd::packages::packages_get,
            tauri_shell::cmd::packages::packages_create,
            tauri_shell::cmd::packages::packages_update_as_new_revision,
            tauri_shell::cmd::packages::packages_archive,
            tauri_shell::cmd::packages::packages_reactivate,
            tauri_shell::cmd::packages::package_revisions_list,
            tauri_shell::cmd::packages::package_revisions_get,
            tauri_shell::cmd::packages::package_revisions_rollback,
            tauri_shell::cmd::packages::package_preview,
            tauri_shell::cmd::packages::package_materialize_item,
            tauri_shell::cmd::packages::package_catalog_render,
            tauri_shell::cmd::packages::package_catalog_send,
            tauri_shell::cmd::payment_accounts::payment_accounts_list,
            tauri_shell::cmd::payment_accounts::payment_accounts_ensure_defaults,
            tauri_shell::cmd::payment_accounts::payment_accounts_create,
            tauri_shell::cmd::payment_accounts::payment_accounts_update,
            tauri_shell::cmd::payment_accounts::payment_accounts_set_active,
            tauri_shell::cmd::private_movements::private_movements_list,
            tauri_shell::cmd::private_movements::private_movements_get,
            tauri_shell::cmd::private_movements::private_movements_create,
            tauri_shell::cmd::quotes::quotes_list,
            tauri_shell::cmd::quotes::quotes_get,
            tauri_shell::cmd::quotes::quotes_attachments_list,
            tauri_shell::cmd::quotes::quotes_create_draft,
            tauri_shell::cmd::quotes::quotes_validate_draft,
            tauri_shell::cmd::quotes::quotes_issue,
            tauri_shell::cmd::quotes::quotes_accept,
            tauri_shell::cmd::quotes::quotes_reject,
            tauri_shell::cmd::quotes::quotes_cancel,
            tauri_shell::cmd::quotes::quotes_convert_to_invoice,
            tauri_shell::cmd::quotes::quotes_generate_pdf,
            tauri_shell::cmd::quotes::quotes_open_bundle,
            tauri_shell::cmd::quotes::quotes_legal_bindings,
            tauri_shell::cmd::recurring::recurring_list,
            tauri_shell::cmd::recurring::recurring_get,
            tauri_shell::cmd::recurring::recurring_create,
            tauri_shell::cmd::recurring::recurring_update,
            tauri_shell::cmd::recurring::recurring_set_active,
            tauri_shell::cmd::recurring::recurring_run_now,
            tauri_shell::cmd::recurring::recurring_run_due_check,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_list,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_get,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_create,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_update,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_set_active,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_run_now,
            tauri_shell::cmd::recurring_invoice::recurring_invoices_run_due_check,
            tauri_shell::cmd::settings::seller_profile_get,
            tauri_shell::cmd::settings::seller_profile_upsert,
            tauri_shell::cmd::settings::seller_logo_set,
            tauri_shell::cmd::settings::seller_logo_clear,
            tauri_shell::cmd::settings::seller_logo_data,
            tauri_shell::cmd::settings::pdf_templates_list,
            tauri_shell::cmd::settings::pdf_template_preview,
            tauri_shell::cmd::settings::seller_default_template_set,
            tauri_shell::cmd::settings::paragraph_19_info,
            tauri_shell::cmd::settings::travel_settings_get,
            tauri_shell::cmd::settings::travel_settings_set,
            tauri_shell::cmd::settings::travel_compute,
            tauri_shell::cmd::settings::seller_signature_set,
            tauri_shell::cmd::settings::seller_signature_clear,
            tauri_shell::cmd::settings::seller_signature_data,
            tauri_shell::cmd::settings::quote_signature_get,
            tauri_shell::cmd::settings::quote_signature_set,
            tauri_shell::cmd::settings::seller_owner_get,
            tauri_shell::cmd::settings::seller_owner_set,
            tauri_shell::cmd::settings::document_terms_get,
            tauri_shell::cmd::settings::document_terms_set,
            tauri_shell::cmd::settings::drop_folder_settings_get,
            tauri_shell::cmd::settings::drop_folder_settings_set,
            tauri_shell::cmd::settings::drop_folder_sync_now,
            tauri_shell::cmd::system::audit_trail_list,
            tauri_shell::cmd::system::archive_integrity_run,
            tauri_shell::cmd::system::archive_integrity_history,
            tauri_shell::cmd::system::app_info,
            tauri_shell::cmd::system::third_party_licenses_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
