//! Registry aller Command-Kerne (Plan W4, Handler generiert per `w4_codegen.py`)
//! mit den Zugriffsregeln für den HTTP-Dispatcher (Plan W5.2, ADR 0039).
//!
//! Jeder Eintrag: Name (= Command-Name im Frontend), Weg ([`Route`]), `auth`,
//! `cost`, Art ([`Kind`]), `body_limit` und ein Handler, der das JSON-Argument-
//! Objekt in das Args-Struct des Kerns übersetzt. Über `POST /api/cmd/{name}`
//! erreichbar ist nur, was [`Route::Cmd`] hat (verbindliche Allowlist); alles
//! andere beantwortet der Dispatcher mit 404.

use crate::app_ctx::AppCtx;
use crate::error::{Error, Result};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};

/// Antwort eines Handlers (serialisiertes Ergebnis des Kerns).
pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;
/// Handler: Kontext + JSON-Argumente → JSON-Ergebnis.
pub type Handler = fn(Arc<AppCtx>, Value) -> HandlerFuture;

/// Wer darf aufrufen (Plan §3, W5/W6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// ohne Sitzung
    Public,
    /// gültige Sitzung
    Session,
    /// gültige Sitzung + frische Re-Auth (Prüfung der Frische ab W6)
    Reauth,
}

/// Kostenklasse für Lastgrenzen (Plan W5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    Cheap,
    /// Typst-Render
    Render,
    /// JVM-Start (KoSIT, Mustang)
    Jvm,
    /// Passphrase-Ableitung
    Kdf,
    /// Backup, Restore, Exporte
    Backup,
}

/// Art des Aufrufs (Plan W5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// liest nur
    Read,
    /// ändert Daten; läuft abgekoppelt vom Request, Idempotenz-Schlüssel
    Write,
    /// langer, zustandsändernder Ablauf: `202 + jobId`, Ergebnis über `/api/jobs/{id}`
    Job,
}

/// Über welchen Weg der Kern erreichbar ist (Plan W5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// `POST /api/cmd/{name}` (Allowlist)
    Cmd,
    /// ersetzt durch eine eigene Route (Auth, Upload, Download)
    Replaced(&'static str),
    /// entfällt im Server (Desktop-Funktion)
    Dropped,
    /// im Server-Betrieb gesperrt (E7 Factory Reset, E9 MS-Graph, E11 Drop-Folder)
    Blocked,
}

/// Ein Registry-Eintrag.
pub struct CommandSpec {
    pub name: &'static str,
    pub route: Route,
    pub auth: Auth,
    pub cost: Cost,
    pub kind: Kind,
    pub body_limit: usize,
    pub handler: Handler,
}

/// Standard-Body-Limit für `POST /api/cmd/*` (Plan W5.8).
pub const DEFAULT_BODY_LIMIT: usize = 256 * 1024;

/// Weg im Server (Plan W5.2).
fn route_of(name: &str) -> Route {
    match name {
        "backup_needs_onboarding" | "backup_is_unlocked" => Route::Replaced("GET /api/auth/status"),
        "backup_setup_passphrase" => Route::Replaced("POST /api/auth/setup"),
        "backup_unlock" => Route::Replaced("POST /api/auth/login"),
        "invoices_open_pdf" | "attachments_open" => Route::Replaced("GET /files/archive/{id} (W7)"),
        "backup_set_target" | "backup_set_sftp_target" => {
            Route::Replaced("backup_target_add_directory / backup_target_add_sftp (W7)")
        }
        "backup_restore_preview"
        | "backup_open_folder"
        | "backup_reveal_path"
        | "euer_reveal_path"
        | "invoices_reveal_pdf"
        | "attachments_reveal"
        | "third_party_licenses_path" => Route::Dropped,
        "mail_oauth_status" | "mail_oauth_connect" | "mail_oauth_disconnect" => Route::Blocked,
        _ => Route::Cmd,
    }
}

/// Re-Auth-Pflicht (Plan W6; die Prüfung der Frische folgt in W6).
fn auth_of(name: &str) -> Auth {
    match name {
        "backup_set_target"
        | "backup_set_sftp_target"
        | "backup_target_add_directory"
        | "backup_target_add_sftp"
        | "backup_target_remove"
        | "backup_test_sftp"
        | "backup_test_sftp_saved"
        | "mail_account_test_saved"
        | "backup_restore_apply"
        | "mail_account_create"
        | "mail_account_update"
        | "mail_account_delete"
        | "mail_account_test_connection"
        | "mail_send_test"
        | "migration_export_run"
        | "dsgvo_export"
        | "euer_export_stb_zip"
        | "euer_export_detail_zip"
        // W14: Zahlungsdaten und §19-Status (IBAN auf jeder künftigen Rechnung,
        // auch den automatisch versendeten), unumkehrbare Schritte.
        | "seller_profile_upsert"
        | "payment_accounts_create"
        | "payment_accounts_update"
        | "contacts_anonymize"
        | "fiscal_year_close"
        // Lauf F: Freigabe nach einer Pause der automatischen Belegerzeugung
        // (Uhrsprung, Restore) — danach können Rechnungen hinausgehen.
        | "scheduler_release_hold"
        // Lauf F (D-24): Anheben eines Nummernkreises (dokumentierte Lücke).
        | "numbering_raise"
        // Lauf F (F1d): Freigabe eines wiederhergestellten Entwurfs, dessen
        // Nummer vor dem Restore schon festgeschrieben war.
        | "numbering_restored_draft_confirm" => Auth::Reauth,
        _ => Auth::Session,
    }
}

/// Kostenklasse (Plan W5.8).
fn cost_of(name: &str) -> Cost {
    match name {
        "pdf_template_preview"
        | "package_preview"
        | "package_catalog_render"
        | "package_catalog_send"
        | "quotes_generate_pdf"
        | "quotes_open_bundle"
        | "quotes_issue"
        | "mail_send_quote" => Cost::Render,
        "invoices_lock_and_issue"
        | "invoices_cancel"
        | "recurring_invoices_run_now"
        | "recurring_invoices_run_due_check"
        | "expenses_create_from_einvoice"
        | "expenses_parse_einvoice"
        | "expenses_receipt_xml_text" => Cost::Jvm,
        "backup_create_now"
        | "backup_restore_apply"
        | "migration_export_run"
        | "dsgvo_export"
        | "euer_export_detail_zip"
        | "euer_export_elster"
        | "euer_export_pdf"
        | "euer_export_datev"
        | "euer_export_stb_zip"
        | "archive_integrity_run" => Cost::Backup,
        _ => Cost::Cheap,
    }
}

/// Art des Aufrufs (Plan W5.6). Was nicht ausdrücklich nur liest, gilt als
/// ändernd (abgekoppelt + Idempotenz — für reine Leser unschädlich).
fn kind_of(name: &str) -> Kind {
    match name {
        // Festschreiben/Ausstellen, Storno, Versand, Backup, Restore, Exporte
        "invoices_lock_and_issue"
        | "quotes_issue"
        | "invoices_cancel"
        | "recurring_invoices_run_now"
        | "recurring_invoices_run_due_check"
        | "mail_send_invoice"
        | "mail_send_quote"
        | "package_catalog_send"
        | "backup_create_now"
        | "backup_restore_apply"
        | "euer_export_stb_zip"
        | "dsgvo_export"
        | "migration_export_run" => Kind::Job,
        // legt fällige Abo-Kosten an
        "recurring_run_due_check" => Kind::Write,
        n if n.ends_with("_list")
            || n.ends_with("_get")
            || n.ends_with("_search")
            || n.ends_with("_data")
            || n.ends_with("_info")
            || n.ends_with("_overview")
            || n.ends_with("_history")
            || n.ends_with("_preview")
            || n.ends_with("_count")
            || n.ends_with("_status")
            || n.ends_with("_check") =>
        {
            Kind::Read
        }
        "euer_compute_report"
        | "euer_available_years"
        | "euer_package"
        | "euer_afa_pending"
        | "assets_afa_table"
        | "assets_suggest_method"
        | "depreciation_list_for_year"
        | "email_log_for"
        | "invoices_validate_draft"
        | "quotes_validate_draft"
        | "quotes_legal_bindings"
        | "travel_compute"
        | "expenses_receipt_xml_text" => Kind::Read,
        _ => Kind::Write,
    }
}

fn body_limit_of(_name: &str) -> usize {
    // Seit W7 keine Bytes mehr im JSON (Uploads über POST /api/upload/{art}).
    DEFAULT_BODY_LIMIT
}

impl CommandSpec {
    fn new(name: &'static str, handler: Handler) -> Self {
        Self {
            name,
            route: route_of(name),
            auth: auth_of(name),
            cost: cost_of(name),
            kind: kind_of(name),
            body_limit: body_limit_of(name),
            handler,
        }
    }

    /// Über `POST /api/cmd/{name}` erreichbar?
    pub fn exposed(&self) -> bool {
        self.route == Route::Cmd
    }
}

/// Alle Command-Kerne mit ihren Zugriffsregeln.
pub static REGISTRY: LazyLock<Vec<CommandSpec>> = LazyLock::new(|| {
    ENTRIES
        .iter()
        .map(|(name, handler)| CommandSpec::new(name, *handler))
        .collect()
});

/// Sucht einen Eintrag nach Name (auch nicht erreichbare).
pub fn find(name: &str) -> Option<&'static CommandSpec> {
    REGISTRY.iter().find(|c| c.name == name)
}

/// Sucht einen über `POST /api/cmd/{name}` erreichbaren Eintrag (Allowlist).
pub fn find_exposed(name: &str) -> Option<&'static CommandSpec> {
    find(name).filter(|c| c.exposed())
}

fn parse<T: serde::de::DeserializeOwned>(v: Value) -> Result<T> {
    let v = if v.is_null() {
        Value::Object(Default::default())
    } else {
        v
    };
    serde_json::from_value(v).map_err(|e| Error::BadRequest(e.to_string()))
}

/// Alle Command-Kerne mit Handler, in der Reihenfolge der Quelldateien.
static ENTRIES: &[(&str, Handler)] = &[
    ("assets_list", h::assets_list),
    ("assets_get", h::assets_get),
    ("assets_afa_table", h::assets_afa_table),
    ("assets_suggest_method", h::assets_suggest_method),
    ("assets_create", h::assets_create),
    ("assets_update", h::assets_update),
    ("assets_dispose", h::assets_dispose),
    ("attachments_open", h::attachments_open),
    ("attachments_reveal", h::attachments_reveal),
    ("attachments_list", h::attachments_list),
    ("attachments_add", h::attachments_add),
    ("backup_needs_onboarding", h::backup_needs_onboarding),
    ("backup_is_unlocked", h::backup_is_unlocked),
    ("backup_get_settings", h::backup_get_settings),
    ("backup_set_daily_time", h::backup_set_daily_time),
    ("backup_setup_passphrase", h::backup_setup_passphrase),
    ("backup_unlock", h::backup_unlock),
    ("backup_create_now", h::backup_create_now),
    ("backup_set_target", h::backup_set_target),
    ("backup_test_sftp", h::backup_test_sftp),
    ("backup_set_sftp_target", h::backup_set_sftp_target),
    (
        "backup_target_add_directory",
        h::backup_target_add_directory,
    ),
    ("backup_target_add_sftp", h::backup_target_add_sftp),
    ("backup_target_remove", h::backup_target_remove),
    ("backup_test_sftp_saved", h::backup_test_sftp_saved),
    ("backup_open_folder", h::backup_open_folder),
    ("backup_reveal_path", h::backup_reveal_path),
    ("backup_list", h::backup_list),
    ("backup_log_list", h::backup_log_list),
    ("backup_log_search", h::backup_log_search),
    ("backup_restore_preview", h::backup_restore_preview),
    ("backup_restore_apply", h::backup_restore_apply),
    ("contacts_list", h::contacts_list),
    ("contacts_get", h::contacts_get),
    ("contacts_create", h::contacts_create),
    ("contacts_update", h::contacts_update),
    ("contacts_archive", h::contacts_archive),
    ("contacts_unarchive", h::contacts_unarchive),
    ("contacts_search", h::contacts_search),
    ("contacts_anonymize_check", h::contacts_anonymize_check),
    ("contacts_anonymize", h::contacts_anonymize),
    ("depreciation_accrue_year", h::depreciation_accrue_year),
    ("depreciation_reset_asset", h::depreciation_reset_asset),
    ("depreciation_list_for_year", h::depreciation_list_for_year),
    ("dsgvo_export", h::dsgvo_export),
    ("euer_compute_report", h::euer_compute_report),
    ("euer_available_years", h::euer_available_years),
    ("euer_package", h::euer_package),
    ("euer_export_detail_zip", h::euer_export_detail_zip),
    ("euer_export_elster", h::euer_export_elster),
    ("euer_export_pdf", h::euer_export_pdf),
    ("euer_export_datev", h::euer_export_datev),
    ("euer_export_stb_zip", h::euer_export_stb_zip),
    ("euer_afa_pending", h::euer_afa_pending),
    ("euer_reveal_path", h::euer_reveal_path),
    ("expenses_list", h::expenses_list),
    ("expenses_get", h::expenses_get),
    ("expenses_create", h::expenses_create),
    ("expenses_cancel", h::expenses_cancel),
    ("expenses_set_payment", h::expenses_set_payment),
    ("expenses_parse_einvoice", h::expenses_parse_einvoice),
    (
        "expenses_create_from_einvoice",
        h::expenses_create_from_einvoice,
    ),
    ("expenses_receipt_xml_text", h::expenses_receipt_xml_text),
    ("fiscal_year_overview", h::fiscal_year_overview),
    ("fiscal_year_close", h::fiscal_year_close),
    ("fiscal_year_closed_list", h::fiscal_year_closed_list),
    ("fiscal_year_auto_close_get", h::fiscal_year_auto_close_get),
    ("fiscal_year_auto_close_set", h::fiscal_year_auto_close_set),
    ("invoices_list", h::invoices_list),
    ("invoices_get", h::invoices_get),
    ("invoices_open_pdf", h::invoices_open_pdf),
    ("invoices_reveal_pdf", h::invoices_reveal_pdf),
    ("invoices_create_draft", h::invoices_create_draft),
    ("invoices_validate_draft", h::invoices_validate_draft),
    ("invoices_lock_and_issue", h::invoices_lock_and_issue),
    ("invoices_record_payment", h::invoices_record_payment),
    ("invoices_cancel", h::invoices_cancel),
    ("legal_documents_list", h::legal_documents_list),
    ("legal_documents_upload", h::legal_documents_upload),
    ("legal_documents_activate", h::legal_documents_activate),
    ("legal_documents_deactivate", h::legal_documents_deactivate),
    ("mail_accounts_list", h::mail_accounts_list),
    ("mail_account_create", h::mail_account_create),
    ("mail_account_update", h::mail_account_update),
    ("mail_account_delete", h::mail_account_delete),
    (
        "mail_account_test_connection",
        h::mail_account_test_connection,
    ),
    ("mail_account_test_saved", h::mail_account_test_saved),
    ("mail_oauth_status", h::mail_oauth_status),
    ("mail_oauth_connect", h::mail_oauth_connect),
    ("mail_oauth_disconnect", h::mail_oauth_disconnect),
    ("email_log_list", h::email_log_list),
    ("email_log_for", h::email_log_for),
    ("email_log_search", h::email_log_search),
    ("mail_invoice_preview", h::mail_invoice_preview),
    ("mail_send_invoice", h::mail_send_invoice),
    ("mail_send_test", h::mail_send_test),
    ("mail_quote_preview", h::mail_quote_preview),
    ("mail_send_quote", h::mail_send_quote),
    ("migration_export_run", h::migration_export_run),
    ("notifications_list", h::notifications_list),
    ("notifications_unread_count", h::notifications_unread_count),
    ("notifications_dismiss", h::notifications_dismiss),
    ("notifications_dismiss_all", h::notifications_dismiss_all),
    ("notification_rules_list", h::notification_rules_list),
    (
        "notification_rules_set_enabled",
        h::notification_rules_set_enabled,
    ),
    ("notifications_run_checks", h::notifications_run_checks),
    ("package_categories_list", h::package_categories_list),
    ("package_category_create", h::package_category_create),
    ("package_category_update", h::package_category_update),
    ("package_categories_reorder", h::package_categories_reorder),
    ("packages_list", h::packages_list),
    ("packages_get", h::packages_get),
    ("packages_create", h::packages_create),
    (
        "packages_update_as_new_revision",
        h::packages_update_as_new_revision,
    ),
    ("packages_archive", h::packages_archive),
    ("packages_reactivate", h::packages_reactivate),
    ("package_revisions_list", h::package_revisions_list),
    ("package_revisions_get", h::package_revisions_get),
    ("package_revisions_rollback", h::package_revisions_rollback),
    ("package_preview", h::package_preview),
    ("package_materialize_item", h::package_materialize_item),
    ("package_catalog_render", h::package_catalog_render),
    ("package_catalog_send", h::package_catalog_send),
    ("payment_accounts_list", h::payment_accounts_list),
    ("payment_accounts_create", h::payment_accounts_create),
    ("payment_accounts_update", h::payment_accounts_update),
    (
        "payment_accounts_set_active",
        h::payment_accounts_set_active,
    ),
    ("private_movements_list", h::private_movements_list),
    ("private_movements_get", h::private_movements_get),
    ("private_movements_create", h::private_movements_create),
    ("quotes_list", h::quotes_list),
    ("quotes_get", h::quotes_get),
    ("quotes_attachments_list", h::quotes_attachments_list),
    ("quotes_create_draft", h::quotes_create_draft),
    ("quotes_validate_draft", h::quotes_validate_draft),
    ("quotes_issue", h::quotes_issue),
    ("quotes_accept", h::quotes_accept),
    ("quotes_reject", h::quotes_reject),
    ("quotes_cancel", h::quotes_cancel),
    ("quotes_convert_to_invoice", h::quotes_convert_to_invoice),
    ("quotes_generate_pdf", h::quotes_generate_pdf),
    ("quotes_open_bundle", h::quotes_open_bundle),
    ("quotes_legal_bindings", h::quotes_legal_bindings),
    ("recurring_list", h::recurring_list),
    ("recurring_get", h::recurring_get),
    ("recurring_create", h::recurring_create),
    ("recurring_update", h::recurring_update),
    ("recurring_set_active", h::recurring_set_active),
    ("recurring_run_now", h::recurring_run_now),
    ("recurring_run_due_check", h::recurring_run_due_check),
    ("recurring_resolve_backlog", h::recurring_resolve_backlog),
    ("recurring_invoices_list", h::recurring_invoices_list),
    ("recurring_invoices_get", h::recurring_invoices_get),
    ("recurring_invoices_create", h::recurring_invoices_create),
    ("recurring_invoices_update", h::recurring_invoices_update),
    (
        "recurring_invoices_set_active",
        h::recurring_invoices_set_active,
    ),
    ("recurring_invoices_run_now", h::recurring_invoices_run_now),
    (
        "recurring_invoices_resolve_backlog",
        h::recurring_invoices_resolve_backlog,
    ),
    (
        "recurring_invoices_run_due_check",
        h::recurring_invoices_run_due_check,
    ),
    ("seller_profile_get", h::seller_profile_get),
    ("seller_profile_upsert", h::seller_profile_upsert),
    ("seller_logo_set", h::seller_logo_set),
    ("seller_logo_clear", h::seller_logo_clear),
    ("seller_logo_data", h::seller_logo_data),
    ("pdf_templates_list", h::pdf_templates_list),
    ("pdf_template_preview", h::pdf_template_preview),
    (
        "seller_default_template_set",
        h::seller_default_template_set,
    ),
    ("paragraph_19_info", h::paragraph_19_info),
    ("travel_settings_get", h::travel_settings_get),
    ("travel_settings_set", h::travel_settings_set),
    ("travel_compute", h::travel_compute),
    ("seller_signature_set", h::seller_signature_set),
    ("seller_signature_clear", h::seller_signature_clear),
    ("seller_signature_data", h::seller_signature_data),
    ("quote_signature_get", h::quote_signature_get),
    ("quote_signature_set", h::quote_signature_set),
    ("seller_owner_get", h::seller_owner_get),
    ("seller_owner_set", h::seller_owner_set),
    ("document_terms_get", h::document_terms_get),
    ("document_terms_set", h::document_terms_set),
    ("update_check_get", h::update_check_get),
    ("update_check_set", h::update_check_set),
    ("audit_trail_list", h::audit_trail_list),
    ("archive_integrity_run", h::archive_integrity_run),
    ("archive_integrity_history", h::archive_integrity_history),
    ("app_info", h::app_info),
    ("scheduler_hold_status", h::scheduler_hold_status),
    ("scheduler_release_hold", h::scheduler_release_hold),
    ("numbering_counters_list", h::numbering_counters_list),
    ("numbering_raise", h::numbering_raise),
    (
        "numbering_restored_drafts_list",
        h::numbering_restored_drafts_list,
    ),
    (
        "numbering_restored_draft_confirm",
        h::numbering_restored_draft_confirm,
    ),
    (
        "recurring_restore_review_list",
        h::recurring_restore_review_list,
    ),
    ("third_party_licenses_path", h::third_party_licenses_path),
];

mod h {
    use super::*;

    pub fn assets_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn assets_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn assets_afa_table(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsAfaTableArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_afa_table(&ctx, args).await?,
            )?)
        })
    }

    pub fn assets_suggest_method(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsSuggestMethodArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_suggest_method(&ctx, args).await?,
            )?)
        })
    }

    pub fn assets_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn assets_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn assets_dispose(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::assets::AssetsDisposeArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::assets::assets_dispose(&ctx, args).await?,
            )?)
        })
    }

    pub fn attachments_open(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::attachments::AttachmentsOpenArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::attachments::attachments_open(&ctx, args).await?,
            )?)
        })
    }

    pub fn attachments_reveal(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::attachments::AttachmentsRevealArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::attachments::attachments_reveal(&ctx, args).await?,
            )?)
        })
    }

    pub fn attachments_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::attachments::AttachmentsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::attachments::attachments_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn attachments_add(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::attachments::AttachmentsAddArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::attachments::attachments_add(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_needs_onboarding(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupNeedsOnboardingArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_needs_onboarding(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_is_unlocked(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupIsUnlockedArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_is_unlocked(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_get_settings(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupGetSettingsArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_get_settings(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_setup_passphrase(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupSetupPassphraseArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_setup_passphrase(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_unlock(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupUnlockArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_unlock(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_create_now(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupCreateNowArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_create_now(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_set_target(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupSetTargetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_set_target(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_target_add_directory(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupTargetAddDirectoryArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_target_add_directory(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_target_add_sftp(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupTargetAddSftpArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_target_add_sftp(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_target_remove(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupTargetRemoveArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_target_remove(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_test_sftp_saved(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupTestSftpSavedArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_test_sftp_saved(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_account_test_saved(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailAccountTestSavedArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_account_test_saved(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_test_sftp(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupTestSftpArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_test_sftp(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_set_sftp_target(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupSetSftpTargetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_set_sftp_target(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_open_folder(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupOpenFolderArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_open_folder(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_reveal_path(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupRevealPathArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_reveal_path(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_log_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupLogListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_log_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_log_search(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupLogSearchArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_log_search(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_restore_preview(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupRestorePreviewArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_restore_preview(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_restore_apply(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupRestoreApplyArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_restore_apply(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_archive(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsArchiveArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_archive(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_unarchive(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsUnarchiveArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_unarchive(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_search(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsSearchArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_search(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_anonymize_check(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsAnonymizeCheckArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_anonymize_check(&ctx, args).await?,
            )?)
        })
    }

    pub fn contacts_anonymize(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::contacts::ContactsAnonymizeArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::contacts::contacts_anonymize(&ctx, args).await?,
            )?)
        })
    }

    pub fn depreciation_accrue_year(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::depreciation::DepreciationAccrueYearArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::depreciation::depreciation_accrue_year(&ctx, args).await?,
            )?)
        })
    }

    pub fn depreciation_reset_asset(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::depreciation::DepreciationResetAssetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::depreciation::depreciation_reset_asset(&ctx, args).await?,
            )?)
        })
    }

    pub fn depreciation_list_for_year(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::depreciation::DepreciationListForYearArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::depreciation::depreciation_list_for_year(&ctx, args).await?,
            )?)
        })
    }

    pub fn dsgvo_export(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::dsgvo::DsgvoExportArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::dsgvo::dsgvo_export(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_compute_report(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerComputeReportArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_compute_report(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_available_years(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerAvailableYearsArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_available_years(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_package(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerPackageArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_package(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_export_detail_zip(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerExportDetailZipArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_export_detail_zip(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_export_elster(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerExportElsterArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_export_elster(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_export_pdf(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerExportPdfArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_export_pdf(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_export_datev(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerExportDatevArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_export_datev(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_export_stb_zip(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerExportStbZipArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_export_stb_zip(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_afa_pending(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerAfaPendingArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_afa_pending(&ctx, args).await?,
            )?)
        })
    }

    pub fn euer_reveal_path(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::euer::EuerRevealPathArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::euer::euer_reveal_path(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_cancel(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesCancelArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_cancel(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_set_payment(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesSetPaymentArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_set_payment(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_parse_einvoice(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesParseEinvoiceArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_parse_einvoice(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_create_from_einvoice(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesCreateFromEinvoiceArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_create_from_einvoice(&ctx, args).await?,
            )?)
        })
    }

    pub fn expenses_receipt_xml_text(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::expenses::ExpensesReceiptXmlTextArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::expenses::expenses_receipt_xml_text(&ctx, args).await?,
            )?)
        })
    }

    pub fn fiscal_year_overview(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::fiscal_year::FiscalYearOverviewArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::fiscal_year::fiscal_year_overview(&ctx, args).await?,
            )?)
        })
    }

    pub fn fiscal_year_close(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::fiscal_year::FiscalYearCloseArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::fiscal_year::fiscal_year_close(&ctx, args).await?,
            )?)
        })
    }

    pub fn fiscal_year_closed_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::fiscal_year::FiscalYearClosedListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::fiscal_year::fiscal_year_closed_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn fiscal_year_auto_close_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::fiscal_year::FiscalYearAutoCloseGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::fiscal_year::fiscal_year_auto_close_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn fiscal_year_auto_close_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::fiscal_year::FiscalYearAutoCloseSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::fiscal_year::fiscal_year_auto_close_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_open_pdf(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesOpenPdfArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_open_pdf(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_reveal_pdf(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesRevealPdfArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_reveal_pdf(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_create_draft(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesCreateDraftArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_create_draft(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_validate_draft(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesValidateDraftArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_validate_draft(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_lock_and_issue(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesLockAndIssueArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_lock_and_issue(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_record_payment(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesRecordPaymentArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_record_payment(&ctx, args).await?,
            )?)
        })
    }

    pub fn invoices_cancel(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::invoices::InvoicesCancelArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::invoices::invoices_cancel(&ctx, args).await?,
            )?)
        })
    }

    pub fn legal_documents_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::legal_documents::LegalDocumentsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::legal_documents::legal_documents_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn legal_documents_upload(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::legal_documents::LegalDocumentsUploadArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::legal_documents::legal_documents_upload(&ctx, args).await?,
            )?)
        })
    }

    pub fn legal_documents_activate(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::legal_documents::LegalDocumentsActivateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::legal_documents::legal_documents_activate(&ctx, args).await?,
            )?)
        })
    }

    pub fn legal_documents_deactivate(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::legal_documents::LegalDocumentsDeactivateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::legal_documents::legal_documents_deactivate(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_accounts_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailAccountsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_accounts_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_account_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailAccountCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_account_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_account_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailAccountUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_account_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_account_delete(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailAccountDeleteArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_account_delete(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_account_test_connection(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailAccountTestConnectionArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_account_test_connection(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_oauth_status(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailOauthStatusArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_oauth_status(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_oauth_connect(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailOauthConnectArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_oauth_connect(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_oauth_disconnect(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailOauthDisconnectArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_oauth_disconnect(&ctx, args).await?,
            )?)
        })
    }

    pub fn email_log_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::EmailLogListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::email_log_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn email_log_for(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::EmailLogForArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::email_log_for(&ctx, args).await?,
            )?)
        })
    }

    pub fn email_log_search(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::EmailLogSearchArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::email_log_search(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_invoice_preview(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailInvoicePreviewArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_invoice_preview(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_send_invoice(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailSendInvoiceArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_send_invoice(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_send_test(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailSendTestArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_send_test(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_quote_preview(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailQuotePreviewArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_quote_preview(&ctx, args).await?,
            )?)
        })
    }

    pub fn mail_send_quote(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::mail::MailSendQuoteArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::mail::mail_send_quote(&ctx, args).await?,
            )?)
        })
    }

    pub fn migration_export_run(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::migration_export::MigrationExportRunArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::migration_export::migration_export_run(&ctx, args).await?,
            )?)
        })
    }

    pub fn notifications_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notifications_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn notifications_unread_count(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationsUnreadCountArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notifications_unread_count(&ctx, args).await?,
            )?)
        })
    }

    pub fn notifications_dismiss(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationsDismissArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notifications_dismiss(&ctx, args).await?,
            )?)
        })
    }

    pub fn notifications_dismiss_all(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationsDismissAllArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notifications_dismiss_all(&ctx, args).await?,
            )?)
        })
    }

    pub fn notification_rules_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationRulesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notification_rules_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn notification_rules_set_enabled(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationRulesSetEnabledArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notification_rules_set_enabled(&ctx, args).await?,
            )?)
        })
    }

    pub fn notifications_run_checks(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::notifications::NotificationsRunChecksArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::notifications::notifications_run_checks(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_categories_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageCategoriesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_categories_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_category_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageCategoryCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_category_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_category_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageCategoryUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_category_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_categories_reorder(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageCategoriesReorderArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_categories_reorder(&ctx, args).await?,
            )?)
        })
    }

    pub fn packages_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::packages_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn packages_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagesGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::packages_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn packages_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagesCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::packages_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn packages_update_as_new_revision(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagesUpdateAsNewRevisionArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::packages_update_as_new_revision(&ctx, args).await?,
            )?)
        })
    }

    pub fn packages_archive(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagesArchiveArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::packages_archive(&ctx, args).await?,
            )?)
        })
    }

    pub fn packages_reactivate(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagesReactivateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::packages_reactivate(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_revisions_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageRevisionsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_revisions_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_revisions_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageRevisionsGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_revisions_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_revisions_rollback(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageRevisionsRollbackArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_revisions_rollback(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_preview(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackagePreviewArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_preview(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_materialize_item(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageMaterializeItemArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_materialize_item(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_catalog_render(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageCatalogRenderArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_catalog_render(&ctx, args).await?,
            )?)
        })
    }

    pub fn package_catalog_send(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::packages::PackageCatalogSendArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::packages::package_catalog_send(&ctx, args).await?,
            )?)
        })
    }

    pub fn payment_accounts_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::payment_accounts::PaymentAccountsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::payment_accounts::payment_accounts_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn payment_accounts_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::payment_accounts::PaymentAccountsCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::payment_accounts::payment_accounts_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn payment_accounts_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::payment_accounts::PaymentAccountsUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::payment_accounts::payment_accounts_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn payment_accounts_set_active(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::payment_accounts::PaymentAccountsSetActiveArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::payment_accounts::payment_accounts_set_active(&ctx, args).await?,
            )?)
        })
    }

    pub fn private_movements_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::private_movements::PrivateMovementsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::private_movements::private_movements_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn private_movements_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::private_movements::PrivateMovementsGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::private_movements::private_movements_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn private_movements_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::private_movements::PrivateMovementsCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::private_movements::private_movements_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_attachments_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesAttachmentsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_attachments_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_create_draft(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesCreateDraftArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_create_draft(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_validate_draft(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesValidateDraftArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_validate_draft(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_issue(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesIssueArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_issue(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_accept(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesAcceptArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_accept(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_reject(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesRejectArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_reject(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_cancel(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesCancelArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_cancel(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_convert_to_invoice(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesConvertToInvoiceArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_convert_to_invoice(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_generate_pdf(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesGeneratePdfArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_generate_pdf(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_open_bundle(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesOpenBundleArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_open_bundle(&ctx, args).await?,
            )?)
        })
    }

    pub fn quotes_legal_bindings(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::quotes::QuotesLegalBindingsArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::quotes::quotes_legal_bindings(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_set_active(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringSetActiveArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_set_active(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_run_now(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringRunNowArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_run_now(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_resolve_backlog(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringResolveBacklogArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_resolve_backlog(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_resolve_backlog(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesResolveBacklogArgs =
                parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_resolve_backlog(&ctx, args)
                    .await?,
            )?)
        })
    }

    pub fn scheduler_hold_status(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::SchedulerHoldStatusArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::scheduler_hold_status(&ctx, args).await?,
            )?)
        })
    }

    pub fn scheduler_release_hold(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::SchedulerReleaseHoldArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::scheduler_release_hold(&ctx, args).await?,
            )?)
        })
    }

    pub fn numbering_counters_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::NumberingCountersListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::numbering_counters_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn numbering_raise(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::NumberingRaiseArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::numbering_raise(&ctx, args).await?,
            )?)
        })
    }

    pub fn numbering_restored_drafts_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::NumberingRestoredDraftsListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::numbering_restored_drafts_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn numbering_restored_draft_confirm(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::NumberingRestoredDraftConfirmArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::numbering_restored_draft_confirm(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_restore_review_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringRestoreReviewListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_restore_review_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn backup_set_daily_time(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::backup::BackupSetDailyTimeArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::backup::backup_set_daily_time(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_run_due_check(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring::RecurringRunDueCheckArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring::recurring_run_due_check(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_create(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesCreateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_create(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_update(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesUpdateArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_update(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_set_active(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesSetActiveArgs =
                parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_set_active(&ctx, args)
                    .await?,
            )?)
        })
    }

    pub fn recurring_invoices_run_now(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesRunNowArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::recurring_invoice::recurring_invoices_run_now(&ctx, args).await?,
            )?)
        })
    }

    pub fn recurring_invoices_run_due_check(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::recurring_invoice::RecurringInvoicesRunDueCheckArgs =
                parse(v)?;
            let out =
                crate::commands::recurring_invoice::recurring_invoices_run_due_check(&ctx, args)
                    .await?;
            Ok(serde_json::to_value(out)?)
        })
    }

    pub fn seller_profile_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerProfileGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_profile_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_profile_upsert(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerProfileUpsertArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_profile_upsert(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_logo_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerLogoSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_logo_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_logo_clear(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerLogoClearArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_logo_clear(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_logo_data(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerLogoDataArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_logo_data(&ctx, args).await?,
            )?)
        })
    }

    pub fn pdf_templates_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::PdfTemplatesListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::pdf_templates_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn pdf_template_preview(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::PdfTemplatePreviewArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::pdf_template_preview(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_default_template_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerDefaultTemplateSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_default_template_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn paragraph_19_info(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::Paragraph19InfoArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::paragraph_19_info(&ctx, args).await?,
            )?)
        })
    }

    pub fn travel_settings_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::TravelSettingsGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::travel_settings_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn travel_settings_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::TravelSettingsSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::travel_settings_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn travel_compute(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::TravelComputeArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::travel_compute(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_signature_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerSignatureSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_signature_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_signature_clear(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerSignatureClearArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_signature_clear(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_signature_data(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerSignatureDataArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_signature_data(&ctx, args).await?,
            )?)
        })
    }

    pub fn quote_signature_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::QuoteSignatureGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::quote_signature_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn quote_signature_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::QuoteSignatureSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::quote_signature_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_owner_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerOwnerGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_owner_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn seller_owner_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::SellerOwnerSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::seller_owner_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn document_terms_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::DocumentTermsGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::document_terms_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn document_terms_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::DocumentTermsSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::document_terms_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn update_check_get(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::UpdateCheckGetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::update_check_get(&ctx, args).await?,
            )?)
        })
    }

    pub fn update_check_set(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::settings::UpdateCheckSetArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::settings::update_check_set(&ctx, args).await?,
            )?)
        })
    }

    pub fn audit_trail_list(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::AuditTrailListArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::audit_trail_list(&ctx, args).await?,
            )?)
        })
    }

    pub fn archive_integrity_run(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::ArchiveIntegrityRunArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::archive_integrity_run(&ctx, args).await?,
            )?)
        })
    }

    pub fn archive_integrity_history(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::ArchiveIntegrityHistoryArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::archive_integrity_history(&ctx, args).await?,
            )?)
        })
    }

    pub fn app_info(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::AppInfoArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::app_info(&ctx, args).await,
            )?)
        })
    }

    pub fn third_party_licenses_path(ctx: Arc<AppCtx>, v: Value) -> HandlerFuture {
        Box::pin(async move {
            let args: crate::commands::system::ThirdPartyLicensesPathArgs = parse(v)?;
            Ok(serde_json::to_value(
                crate::commands::system::third_party_licenses_path(&ctx, args).await?,
            )?)
        })
    }
}
