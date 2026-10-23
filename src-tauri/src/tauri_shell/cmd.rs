//! GENERIERT (Plan W4, `w4_codegen.py`): dünne Tauri-Wrapper um die
//! Command-Kerne in `crate::commands`. Entfällt mit Tauri in W5.
//! Sonderfälle (Opener, Neustart) stehen von Hand in `tauri_shell::special`.

pub mod assets {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::assets::afa_tabellen::{self, AfaTabellen};
    use crate::backup;
    use crate::commands::assets::*;
    use crate::config::Paths;
    use crate::db::models::AssetDetail;
    use crate::db::numbering;
    use crate::db::repo::{assets, audit_log, depreciation as depreciation_repo, expenses};
    use crate::domain::asset::{
        self, business_book_value_start_cents, AssetInput, DepreciationMethod, DisposalType,
        MethodSuggestion,
    };
    use crate::domain::depreciation::{compute_disposal_year_partial, DepreciationAsset};
    use crate::domain::numbering::DocType;
    use crate::error::{Error, Result};
    use crate::fiscal_year::guard;
    use chrono::{Datelike, NaiveDate};
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn assets_list(
        ctx: State<'_, Arc<AppCtx>>,
        filter: Option<assets::ListFilter>,
    ) -> Result<Vec<crate::db::models::AssetListItem>> {
        crate::commands::assets::assets_list(&ctx, AssetsListArgs { filter }).await
    }

    #[tauri::command]
    pub async fn assets_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<AssetDetail>> {
        crate::commands::assets::assets_get(&ctx, AssetsGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn assets_afa_table(ctx: State<'_, Arc<AppCtx>>) -> Result<AfaTabellen> {
        crate::commands::assets::assets_afa_table(&ctx, AssetsAfaTableArgs {}).await
    }

    #[tauri::command]
    pub async fn assets_suggest_method(
        ctx: State<'_, Arc<AppCtx>>,
        expense_category: Option<String>,
        acquisition_cost_cents: i64,
    ) -> Result<MethodSuggestion> {
        crate::commands::assets::assets_suggest_method(
            &ctx,
            AssetsSuggestMethodArgs {
                expense_category,
                acquisition_cost_cents,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn assets_create(
        ctx: State<'_, Arc<AppCtx>>,
        input: AssetInput,
    ) -> Result<AssetDetail> {
        crate::commands::assets::assets_create(&ctx, AssetsCreateArgs { input }).await
    }

    #[tauri::command]
    pub async fn assets_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        input: AssetInput,
    ) -> Result<AssetDetail> {
        crate::commands::assets::assets_update(&ctx, AssetsUpdateArgs { id, input }).await
    }

    #[tauri::command]
    pub async fn assets_dispose(
        ctx: State<'_, Arc<AppCtx>>,
        args: AssetDisposeArgs,
    ) -> Result<AssetDetail> {
        crate::commands::assets::assets_dispose(&ctx, AssetsDisposeArgs { args }).await
    }
}

pub mod attachments {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::{self, ArchiveKind};
    use crate::commands::attachments::*;
    use crate::config::Paths;
    use crate::db::models::AttachmentView;
    use crate::db::repo::{attachments, audit_log};
    use crate::error::{Error, Result};
    use chrono::Datelike;
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn attachments_open(
        ctx: State<'_, Arc<AppCtx>>,
        archive_entry_id: String,
    ) -> Result<()> {
        crate::commands::attachments::attachments_open(
            &ctx,
            AttachmentsOpenArgs { archive_entry_id },
        )
        .await
    }

    #[tauri::command]
    pub async fn attachments_reveal(
        ctx: State<'_, Arc<AppCtx>>,
        archive_entry_id: String,
    ) -> Result<()> {
        crate::commands::attachments::attachments_reveal(
            &ctx,
            AttachmentsRevealArgs { archive_entry_id },
        )
        .await
    }

    #[tauri::command]
    pub async fn attachments_list(
        ctx: State<'_, Arc<AppCtx>>,
        parent_type: String,
        parent_id: String,
    ) -> Result<Vec<AttachmentView>> {
        crate::commands::attachments::attachments_list(
            &ctx,
            AttachmentsListArgs {
                parent_type,
                parent_id,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn attachments_add(
        ctx: State<'_, Arc<AppCtx>>,
        args: AddAttachmentArgs,
    ) -> Result<Vec<AttachmentView>> {
        crate::commands::attachments::attachments_add(&ctx, AttachmentsAddArgs { args }).await
    }
}

pub mod backup {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup::{self, restore, BackupOutcome, BackupSession};
    use crate::commands::backup::*;
    use crate::config::Paths;
    use crate::db;
    use crate::error::{Error, Result};
    use serde::Serialize;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn backup_needs_onboarding(ctx: State<'_, Arc<AppCtx>>) -> Result<bool> {
        crate::commands::backup::backup_needs_onboarding(&ctx, BackupNeedsOnboardingArgs {}).await
    }

    #[tauri::command]
    pub async fn backup_is_unlocked(ctx: State<'_, Arc<AppCtx>>) -> Result<bool> {
        crate::commands::backup::backup_is_unlocked(&ctx, BackupIsUnlockedArgs {}).await
    }

    #[tauri::command]
    pub async fn backup_get_settings(ctx: State<'_, Arc<AppCtx>>) -> Result<BackupSettings> {
        crate::commands::backup::backup_get_settings(&ctx, BackupGetSettingsArgs {}).await
    }

    #[tauri::command]
    pub async fn backup_setup_passphrase(
        ctx: State<'_, Arc<AppCtx>>,
        passphrase: String,
    ) -> Result<BackupOutcome> {
        crate::commands::backup::backup_setup_passphrase(
            &ctx,
            BackupSetupPassphraseArgs { passphrase },
        )
        .await
    }

    #[tauri::command]
    pub async fn backup_unlock(ctx: State<'_, Arc<AppCtx>>, passphrase: String) -> Result<bool> {
        crate::commands::backup::backup_unlock(&ctx, BackupUnlockArgs { passphrase }).await
    }

    #[tauri::command]
    pub async fn backup_create_now(ctx: State<'_, Arc<AppCtx>>) -> Result<BackupOutcome> {
        crate::commands::backup::backup_create_now(&ctx, BackupCreateNowArgs {}).await
    }

    #[tauri::command]
    pub async fn backup_set_target(ctx: State<'_, Arc<AppCtx>>, path: String) -> Result<()> {
        crate::commands::backup::backup_set_target(&ctx, BackupSetTargetArgs { path }).await
    }

    #[tauri::command]
    pub async fn backup_test_sftp(
        ctx: State<'_, Arc<AppCtx>>,
        host: String,
        port: u16,
        user: String,
        remote_dir: String,
        password: String,
    ) -> Result<backup::sftp::SftpProbe> {
        crate::commands::backup::backup_test_sftp(
            &ctx,
            BackupTestSftpArgs {
                host,
                port,
                user,
                remote_dir,
                password,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn backup_set_sftp_target(
        ctx: State<'_, Arc<AppCtx>>,
        host: String,
        port: u16,
        user: String,
        remote_dir: String,
        host_fingerprint: String,
        password: String,
    ) -> Result<()> {
        crate::commands::backup::backup_set_sftp_target(
            &ctx,
            BackupSetSftpTargetArgs {
                host,
                port,
                user,
                remote_dir,
                host_fingerprint,
                password,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn backup_open_folder(ctx: State<'_, Arc<AppCtx>>, path: String) -> Result<()> {
        crate::commands::backup::backup_open_folder(&ctx, BackupOpenFolderArgs { path }).await
    }

    #[tauri::command]
    pub async fn backup_reveal_path(ctx: State<'_, Arc<AppCtx>>, path: String) -> Result<()> {
        crate::commands::backup::backup_reveal_path(&ctx, BackupRevealPathArgs { path }).await
    }

    #[tauri::command]
    pub async fn backup_list(ctx: State<'_, Arc<AppCtx>>) -> Result<Vec<BackupHistoryItem>> {
        crate::commands::backup::backup_list(&ctx, BackupListArgs {}).await
    }

    #[tauri::command]
    pub async fn backup_log_list(
        ctx: State<'_, Arc<AppCtx>>,
        limit: Option<i64>,
    ) -> Result<Vec<crate::db::models::BackupLogRow>> {
        crate::commands::backup::backup_log_list(&ctx, BackupLogListArgs { limit }).await
    }

    #[tauri::command]
    pub async fn backup_log_search(
        ctx: State<'_, Arc<AppCtx>>,
        filter: db::repo::backup_log::BackupLogFilter,
    ) -> Result<Vec<crate::db::models::BackupLogRow>> {
        crate::commands::backup::backup_log_search(&ctx, BackupLogSearchArgs { filter }).await
    }

    #[tauri::command]
    pub async fn backup_restore_preview(
        ctx: State<'_, Arc<AppCtx>>,
        path: String,
    ) -> Result<restore::RestorePreview> {
        crate::commands::backup::backup_restore_preview(&ctx, BackupRestorePreviewArgs { path })
            .await
    }

    #[tauri::command]
    pub async fn backup_restore_apply(
        ctx: State<'_, Arc<AppCtx>>,
        path: String,
        passphrase: String,
    ) -> Result<restore::RestoreReport> {
        crate::commands::backup::backup_restore_apply(
            &ctx,
            BackupRestoreApplyArgs { path, passphrase },
        )
        .await
    }
}

pub mod contacts {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::commands::contacts::*;
    use crate::db::models::ContactRow;
    use crate::db::repo::{audit_log, contacts};
    use crate::domain::contact::ContactInput;
    use crate::error::{Error, Result};
    use serde::Serialize;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn contacts_list(
        ctx: State<'_, Arc<AppCtx>>,
        include_archived: Option<bool>,
    ) -> Result<Vec<ContactRow>> {
        crate::commands::contacts::contacts_list(&ctx, ContactsListArgs { include_archived }).await
    }

    #[tauri::command]
    pub async fn contacts_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<ContactRow>> {
        crate::commands::contacts::contacts_get(&ctx, ContactsGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn contacts_create(
        ctx: State<'_, Arc<AppCtx>>,
        input: ContactInput,
    ) -> Result<ContactRow> {
        crate::commands::contacts::contacts_create(&ctx, ContactsCreateArgs { input }).await
    }

    #[tauri::command]
    pub async fn contacts_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        input: ContactInput,
    ) -> Result<ContactRow> {
        crate::commands::contacts::contacts_update(&ctx, ContactsUpdateArgs { id, input }).await
    }

    #[tauri::command]
    pub async fn contacts_archive(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::contacts::contacts_archive(&ctx, ContactsArchiveArgs { id }).await
    }

    #[tauri::command]
    pub async fn contacts_unarchive(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::contacts::contacts_unarchive(&ctx, ContactsUnarchiveArgs { id }).await
    }

    #[tauri::command]
    pub async fn contacts_search(
        ctx: State<'_, Arc<AppCtx>>,
        query: String,
        include_archived: Option<bool>,
    ) -> Result<Vec<ContactRow>> {
        crate::commands::contacts::contacts_search(
            &ctx,
            ContactsSearchArgs {
                query,
                include_archived,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn contacts_anonymize_check(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<AnonymizeCheck> {
        crate::commands::contacts::contacts_anonymize_check(&ctx, ContactsAnonymizeCheckArgs { id })
            .await
    }

    #[tauri::command]
    pub async fn contacts_anonymize(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<ContactRow> {
        crate::commands::contacts::contacts_anonymize(&ctx, ContactsAnonymizeArgs { id }).await
    }
}

pub mod depreciation {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup;
    use crate::commands::depreciation::*;
    use crate::config::Paths;
    use crate::db::models::{AssetDetail, DepreciationEntryRow};
    use crate::db::repo::{assets, depreciation as depreciation_repo};
    use crate::depreciation::accrue_yearly::{self, AccrueReport};
    use crate::error::{Error, Result};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn depreciation_accrue_year(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
    ) -> Result<AccrueReport> {
        crate::commands::depreciation::depreciation_accrue_year(
            &ctx,
            DepreciationAccrueYearArgs { fiscal_year },
        )
        .await
    }

    #[tauri::command]
    pub async fn depreciation_reset_asset(
        ctx: State<'_, Arc<AppCtx>>,
        asset_id: String,
    ) -> Result<AssetDetail> {
        crate::commands::depreciation::depreciation_reset_asset(
            &ctx,
            DepreciationResetAssetArgs { asset_id },
        )
        .await
    }

    #[tauri::command]
    pub async fn depreciation_list_for_year(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
    ) -> Result<Vec<DepreciationEntryRow>> {
        crate::commands::depreciation::depreciation_list_for_year(
            &ctx,
            DepreciationListForYearArgs { fiscal_year },
        )
        .await
    }
}

pub mod dsgvo {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::commands::dsgvo::*;
    use crate::config::Paths;
    use crate::db::repo::{audit_log, dsgvo as dsgvo_repo, seller_profile};
    use crate::domain::dsgvo::{self, ControllerInfo, RawData, RawInvoice, RawQuote};
    use crate::error::{Error, Result};
    use crate::pdf::{templates, typst_render};
    use serde::Serialize;
    use sqlx::SqlitePool;
    use std::io::Write;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn dsgvo_export(
        ctx: State<'_, Arc<AppCtx>>,
        contact_id: String,
    ) -> Result<DsgvoExportResult> {
        crate::commands::dsgvo::dsgvo_export(&ctx, DsgvoExportArgs { contact_id }).await
    }
}

pub mod euer {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive;
    use crate::commands::euer::*;
    use crate::config::Paths;
    use crate::db::models::SellerProfileRow;
    use crate::db::repo::{audit_log, euer as euer_repo, seller_profile};
    use crate::domain::kleinunternehmer::{self, KleinunternehmerStatus, HINWEIS_TEXT};
    use crate::error::{Error, Result};
    use crate::euer::aggregate::{self, EuerReport};
    use crate::euer::datev_csv::{self, DatevHeader, Skr};
    use crate::euer::detail::{
        self, AveeurItem, DisposalItem, ExpenseItem, IncomeItem, StornoItem,
    };
    use crate::euer::elster_csv::{self, ElsterForm};
    use crate::pdf::{templates, typst_render};
    use chrono::Datelike;
    use serde::Serialize;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn euer_compute_report(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
    ) -> Result<EuerReport> {
        crate::commands::euer::euer_compute_report(&ctx, EuerComputeReportArgs { fiscal_year })
            .await
    }

    #[tauri::command]
    pub async fn euer_available_years(ctx: State<'_, Arc<AppCtx>>) -> Result<Vec<i64>> {
        crate::commands::euer::euer_available_years(&ctx, EuerAvailableYearsArgs {}).await
    }

    #[tauri::command]
    pub async fn euer_package(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
    ) -> Result<EuerPackage> {
        crate::commands::euer::euer_package(&ctx, EuerPackageArgs { fiscal_year }).await
    }

    #[tauri::command]
    pub async fn euer_export_detail_zip(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
        target_path: String,
    ) -> Result<EuerDetailExport> {
        crate::commands::euer::euer_export_detail_zip(
            &ctx,
            EuerExportDetailZipArgs {
                fiscal_year,
                target_path,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn euer_export_elster(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
        target_path: String,
    ) -> Result<ElsterExportResult> {
        crate::commands::euer::euer_export_elster(
            &ctx,
            EuerExportElsterArgs {
                fiscal_year,
                target_path,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn euer_export_pdf(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
        target_path: String,
    ) -> Result<EuerPdfExport> {
        crate::commands::euer::euer_export_pdf(
            &ctx,
            EuerExportPdfArgs {
                fiscal_year,
                target_path,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn euer_export_datev(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
        skr: String,
        target_path: String,
    ) -> Result<DatevExport> {
        crate::commands::euer::euer_export_datev(
            &ctx,
            EuerExportDatevArgs {
                fiscal_year,
                skr,
                target_path,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn euer_export_stb_zip(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
        skr: String,
        target_path: String,
    ) -> Result<StbExport> {
        crate::commands::euer::euer_export_stb_zip(
            &ctx,
            EuerExportStbZipArgs {
                fiscal_year,
                skr,
                target_path,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn euer_afa_pending(
        ctx: State<'_, Arc<AppCtx>>,
        fiscal_year: i64,
    ) -> Result<AfaPending> {
        crate::commands::euer::euer_afa_pending(&ctx, EuerAfaPendingArgs { fiscal_year }).await
    }

    #[tauri::command]
    pub async fn euer_reveal_path(ctx: State<'_, Arc<AppCtx>>, path: String) -> Result<()> {
        crate::commands::euer::euer_reveal_path(&ctx, EuerRevealPathArgs { path }).await
    }
}

pub mod expenses {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::{self, ArchiveKind};
    use crate::backup;
    use crate::commands::attachments::{guess_mime, sanitize_filename};
    use crate::commands::expenses::*;
    use crate::config::Paths;
    use crate::db::models::{ExpenseDetail, ExpenseListItem, ExpenseRow};
    use crate::db::numbering;
    use crate::db::repo::{audit_log, contacts, expenses};
    use crate::domain::expense::{self, ExpenseInput};
    use crate::domain::numbering::DocType;
    use crate::einvoice::parser::{self, ParsedEInvoice};
    use crate::einvoice::types::ValidationSummary;
    use crate::einvoice::{mustang_bridge, validator};
    use crate::error::{Error, Result};
    use crate::fiscal_year::guard;
    use chrono::{Datelike, NaiveDate};
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn expenses_list(
        ctx: State<'_, Arc<AppCtx>>,
        filter: Option<expenses::ListFilter>,
    ) -> Result<Vec<ExpenseListItem>> {
        crate::commands::expenses::expenses_list(&ctx, ExpensesListArgs { filter }).await
    }

    #[tauri::command]
    pub async fn expenses_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<ExpenseDetail>> {
        crate::commands::expenses::expenses_get(&ctx, ExpensesGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn expenses_create(
        ctx: State<'_, Arc<AppCtx>>,
        args: ExpenseCreateArgs,
    ) -> Result<ExpenseDetail> {
        crate::commands::expenses::expenses_create(&ctx, ExpensesCreateArgs { args }).await
    }

    #[tauri::command]
    pub async fn expenses_cancel(
        ctx: State<'_, Arc<AppCtx>>,
        args: ExpenseCancelArgs,
    ) -> Result<ExpenseRow> {
        crate::commands::expenses::expenses_cancel(&ctx, ExpensesCancelArgs { args }).await
    }

    #[tauri::command]
    pub async fn expenses_set_payment(
        ctx: State<'_, Arc<AppCtx>>,
        args: ExpenseSetPaymentArgs,
    ) -> Result<ExpenseRow> {
        crate::commands::expenses::expenses_set_payment(&ctx, ExpensesSetPaymentArgs { args }).await
    }

    #[tauri::command]
    pub async fn expenses_parse_einvoice(
        ctx: State<'_, Arc<AppCtx>>,
        file_bytes: Vec<u8>,
        file_name: String,
    ) -> Result<EInvoiceParseResult> {
        crate::commands::expenses::expenses_parse_einvoice(
            &ctx,
            ExpensesParseEinvoiceArgs {
                file_bytes,
                file_name,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn expenses_create_from_einvoice(
        ctx: State<'_, Arc<AppCtx>>,
        args: EInvoiceCreateArgs,
    ) -> Result<ExpenseDetail> {
        crate::commands::expenses::expenses_create_from_einvoice(
            &ctx,
            ExpensesCreateFromEinvoiceArgs { args },
        )
        .await
    }

    #[tauri::command]
    pub async fn expenses_receipt_xml_text(
        ctx: State<'_, Arc<AppCtx>>,
        expense_id: String,
    ) -> Result<Option<XmlViewerPayload>> {
        crate::commands::expenses::expenses_receipt_xml_text(
            &ctx,
            ExpensesReceiptXmlTextArgs { expense_id },
        )
        .await
    }
}

pub mod factory_reset {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup;
    use crate::commands::factory_reset::*;
    use crate::config::Paths;
    use crate::domain::factory_reset as fr;
    use crate::error::{Error, Result};
    use serde::Serialize;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn factory_reset_check(ctx: State<'_, Arc<AppCtx>>) -> Result<FactoryResetCheck> {
        crate::commands::factory_reset::factory_reset_check(&ctx, FactoryResetCheckArgs {}).await
    }

    #[tauri::command]
    pub async fn factory_reset(
        ctx: State<'_, Arc<AppCtx>>,
        passphrase: String,
        confirm_word: String,
        export_confirmed: bool,
        retention_receipt: String,
    ) -> Result<()> {
        crate::commands::factory_reset::factory_reset(
            &ctx,
            FactoryResetArgs {
                passphrase,
                confirm_word,
                export_confirmed,
                retention_receipt,
            },
        )
        .await
    }
}

pub mod fiscal_year {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup::BackupSession;
    use crate::commands::fiscal_year::*;
    use crate::config::Paths;
    use crate::db::repo::{app_settings, euer as euer_repo};
    use crate::error::Result;
    use crate::euer::aggregate::aggregate;
    use crate::fiscal_year::{lock, transition};
    use chrono::Datelike;
    use serde::Serialize;
    use sqlx::SqlitePool;
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn fiscal_year_overview(ctx: State<'_, Arc<AppCtx>>) -> Result<FiscalYearOverview> {
        crate::commands::fiscal_year::fiscal_year_overview(&ctx, FiscalYearOverviewArgs {}).await
    }

    #[tauri::command]
    pub async fn fiscal_year_close(
        ctx: State<'_, Arc<AppCtx>>,
        year: i64,
    ) -> Result<lock::FiscalYearLock> {
        crate::commands::fiscal_year::fiscal_year_close(&ctx, FiscalYearCloseArgs { year }).await
    }

    #[tauri::command]
    pub async fn fiscal_year_closed_list(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Vec<lock::FiscalYearLock>> {
        crate::commands::fiscal_year::fiscal_year_closed_list(&ctx, FiscalYearClosedListArgs {})
            .await
    }

    #[tauri::command]
    pub async fn fiscal_year_auto_close_get(ctx: State<'_, Arc<AppCtx>>) -> Result<bool> {
        crate::commands::fiscal_year::fiscal_year_auto_close_get(
            &ctx,
            FiscalYearAutoCloseGetArgs {},
        )
        .await
    }

    #[tauri::command]
    pub async fn fiscal_year_auto_close_set(
        ctx: State<'_, Arc<AppCtx>>,
        enabled: bool,
    ) -> Result<()> {
        crate::commands::fiscal_year::fiscal_year_auto_close_set(
            &ctx,
            FiscalYearAutoCloseSetArgs { enabled },
        )
        .await
    }
}

pub mod invoices {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::{self, ArchiveKind};
    use crate::backup;
    use crate::commands::invoices::*;
    use crate::config::Paths;
    use crate::db::models::{InvoiceDetail, InvoiceListItem, InvoiceRow};
    use crate::db::numbering;
    use crate::db::repo::audit_log;
    use crate::db::repo::{contacts, invoices, seller_profile};
    use crate::domain::invoice::{
        self, BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
    };
    use crate::domain::numbering::DocType;
    use crate::domain::storno::{build_storno_input, OriginalInvoiceView, OriginalItemView};
    use crate::einvoice::generator;
    use crate::einvoice::mustang_bridge;
    use crate::einvoice::types::ValidationStatus;
    use crate::einvoice::validator;
    use crate::error::{Error, Result};
    use crate::pdf::{klausel_check, templates, typst_render};
    use chrono::NaiveDate;
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn invoices_list(
        ctx: State<'_, Arc<AppCtx>>,
        filter: Option<invoices::ListFilter>,
    ) -> Result<Vec<InvoiceListItem>> {
        crate::commands::invoices::invoices_list(&ctx, InvoicesListArgs { filter }).await
    }

    #[tauri::command]
    pub async fn invoices_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<InvoiceDetail>> {
        crate::commands::invoices::invoices_get(&ctx, InvoicesGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn invoices_open_pdf(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::invoices::invoices_open_pdf(&ctx, InvoicesOpenPdfArgs { id }).await
    }

    #[tauri::command]
    pub async fn invoices_reveal_pdf(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::invoices::invoices_reveal_pdf(&ctx, InvoicesRevealPdfArgs { id }).await
    }

    #[tauri::command]
    pub async fn invoices_create_draft(
        ctx: State<'_, Arc<AppCtx>>,
        args: CreateDraftArgs,
    ) -> Result<InvoiceDetail> {
        crate::commands::invoices::invoices_create_draft(&ctx, InvoicesCreateDraftArgs { args })
            .await
    }

    #[tauri::command]
    pub async fn invoices_validate_draft(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Vec<ValidationIssueDto>> {
        crate::commands::invoices::invoices_validate_draft(&ctx, InvoicesValidateDraftArgs { id })
            .await
    }

    #[tauri::command]
    pub async fn invoices_lock_and_issue(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        buyer_reference: Option<String>,
    ) -> Result<LockResponse> {
        crate::commands::invoices::invoices_lock_and_issue(
            &ctx,
            InvoicesLockAndIssueArgs {
                id,
                buyer_reference,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn invoices_record_payment(
        ctx: State<'_, Arc<AppCtx>>,
        args: RecordPaymentArgs,
    ) -> Result<InvoiceRow> {
        crate::commands::invoices::invoices_record_payment(&ctx, InvoicesRecordPaymentArgs { args })
            .await
    }

    #[tauri::command]
    pub async fn invoices_cancel(
        ctx: State<'_, Arc<AppCtx>>,
        args: CancelArgs,
    ) -> Result<CancelResponse> {
        crate::commands::invoices::invoices_cancel(&ctx, InvoicesCancelArgs { args }).await
    }
}

pub mod legal_documents {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::{self, ArchiveKind};
    use crate::commands::legal_documents::*;
    use crate::config::Paths;
    use crate::db::models::LegalDocumentRow;
    use crate::db::repo::{audit_log, legal_documents};
    use crate::error::{Error, Result};
    use chrono::Datelike;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;
    use uuid::Uuid;

    #[tauri::command]
    pub async fn legal_documents_list(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Vec<LegalDocumentRow>> {
        crate::commands::legal_documents::legal_documents_list(&ctx, LegalDocumentsListArgs {})
            .await
    }

    #[tauri::command]
    pub async fn legal_documents_upload(
        ctx: State<'_, Arc<AppCtx>>,
        doc_type: String,
        title: Option<String>,
        file_bytes: Vec<u8>,
        file_name: String,
    ) -> Result<LegalDocumentRow> {
        crate::commands::legal_documents::legal_documents_upload(
            &ctx,
            LegalDocumentsUploadArgs {
                doc_type,
                title,
                file_bytes,
                file_name,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn legal_documents_activate(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::legal_documents::legal_documents_activate(
            &ctx,
            LegalDocumentsActivateArgs { id },
        )
        .await
    }

    #[tauri::command]
    pub async fn legal_documents_deactivate(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::legal_documents::legal_documents_deactivate(
            &ctx,
            LegalDocumentsDeactivateArgs { id },
        )
        .await
    }
}

pub mod mail {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive;
    use crate::commands::mail::*;
    use crate::config::Paths;
    use crate::db::models::MailAccountRow;
    use crate::db::repo::{
        audit_log, contacts, email_log, invoices, mail_accounts, quotes, seller_profile,
    };
    use crate::domain::kleinunternehmer;
    use crate::error::{Error, Result};
    use crate::mail::templates::RenderedMail;
    use crate::mail::{keyring, oauth_ms, smtp, templates};
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn mail_accounts_list(ctx: State<'_, Arc<AppCtx>>) -> Result<Vec<MailAccountRow>> {
        crate::commands::mail::mail_accounts_list(&ctx, MailAccountsListArgs {}).await
    }

    #[tauri::command]
    pub async fn mail_account_create(
        ctx: State<'_, Arc<AppCtx>>,
        input: mail_accounts::MailAccountInput,
        password: Option<String>,
    ) -> Result<MailAccountRow> {
        crate::commands::mail::mail_account_create(&ctx, MailAccountCreateArgs { input, password })
            .await
    }

    #[tauri::command]
    pub async fn mail_account_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        input: mail_accounts::MailAccountInput,
        password: Option<String>,
    ) -> Result<MailAccountRow> {
        crate::commands::mail::mail_account_update(
            &ctx,
            MailAccountUpdateArgs {
                id,
                input,
                password,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn mail_account_delete(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::mail::mail_account_delete(&ctx, MailAccountDeleteArgs { id }).await
    }

    #[tauri::command]
    pub async fn mail_account_test_connection(
        ctx: State<'_, Arc<AppCtx>>,
        args: TestConnectionArgs,
    ) -> Result<()> {
        crate::commands::mail::mail_account_test_connection(
            &ctx,
            MailAccountTestConnectionArgs { args },
        )
        .await
    }

    #[tauri::command]
    pub async fn mail_oauth_status(
        ctx: State<'_, Arc<AppCtx>>,
        account_id: String,
    ) -> Result<OauthStatus> {
        crate::commands::mail::mail_oauth_status(&ctx, MailOauthStatusArgs { account_id }).await
    }

    #[tauri::command]
    pub async fn mail_oauth_connect(
        ctx: State<'_, Arc<AppCtx>>,
        account_id: String,
    ) -> Result<OauthStatus> {
        crate::commands::mail::mail_oauth_connect(&ctx, MailOauthConnectArgs { account_id }).await
    }

    #[tauri::command]
    pub async fn mail_oauth_disconnect(
        ctx: State<'_, Arc<AppCtx>>,
        account_id: String,
    ) -> Result<()> {
        crate::commands::mail::mail_oauth_disconnect(&ctx, MailOauthDisconnectArgs { account_id })
            .await
    }

    #[tauri::command]
    pub async fn email_log_list(
        ctx: State<'_, Arc<AppCtx>>,
        limit: Option<i64>,
    ) -> Result<Vec<crate::db::models::EmailLogRow>> {
        crate::commands::mail::email_log_list(&ctx, EmailLogListArgs { limit }).await
    }

    #[tauri::command]
    pub async fn email_log_for(
        ctx: State<'_, Arc<AppCtx>>,
        kind: String,
        id: String,
    ) -> Result<Vec<crate::db::models::EmailLogRow>> {
        crate::commands::mail::email_log_for(&ctx, EmailLogForArgs { kind, id }).await
    }

    #[tauri::command]
    pub async fn email_log_search(
        ctx: State<'_, Arc<AppCtx>>,
        filter: email_log::EmailLogFilter,
    ) -> Result<Vec<crate::db::models::EmailLogRow>> {
        crate::commands::mail::email_log_search(&ctx, EmailLogSearchArgs { filter }).await
    }

    #[tauri::command]
    pub async fn mail_invoice_preview(
        ctx: State<'_, Arc<AppCtx>>,
        invoice_id: String,
    ) -> Result<RenderedMail> {
        crate::commands::mail::mail_invoice_preview(&ctx, MailInvoicePreviewArgs { invoice_id })
            .await
    }

    #[tauri::command]
    pub async fn mail_send_invoice(
        ctx: State<'_, Arc<AppCtx>>,
        args: SendInvoiceArgs,
    ) -> Result<SendResult> {
        crate::commands::mail::mail_send_invoice(&ctx, MailSendInvoiceArgs { args }).await
    }

    #[tauri::command]
    pub async fn mail_send_test(
        ctx: State<'_, Arc<AppCtx>>,
        account_id: String,
        to: String,
    ) -> Result<()> {
        crate::commands::mail::mail_send_test(&ctx, MailSendTestArgs { account_id, to }).await
    }

    #[tauri::command]
    pub async fn mail_quote_preview(
        ctx: State<'_, Arc<AppCtx>>,
        quote_id: String,
    ) -> Result<RenderedMail> {
        crate::commands::mail::mail_quote_preview(&ctx, MailQuotePreviewArgs { quote_id }).await
    }

    #[tauri::command]
    pub async fn mail_send_quote(
        ctx: State<'_, Arc<AppCtx>>,
        args: SendQuoteArgs,
    ) -> Result<SendQuoteResult> {
        crate::commands::mail::mail_send_quote(&ctx, MailSendQuoteArgs { args }).await
    }
}

pub mod migration_export {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::commands::migration_export::*;
    use crate::config::Paths;
    use crate::db::repo::audit_log;
    use crate::error::Result;
    use crate::migration_export::export::{self, ExportReport};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn migration_export_run(
        ctx: State<'_, Arc<AppCtx>>,
        target_path: String,
    ) -> Result<ExportReport> {
        crate::commands::migration_export::migration_export_run(
            &ctx,
            MigrationExportRunArgs { target_path },
        )
        .await
    }
}

pub mod notifications {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::commands::notifications::*;
    use crate::error::Result;
    use crate::notify::{rules, store, NotificationRule};
    use crate::scheduler::{integrity_check_cron, reminders};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn notifications_list(
        ctx: State<'_, Arc<AppCtx>>,
        include_dismissed: bool,
    ) -> Result<Vec<store::Notification>> {
        crate::commands::notifications::notifications_list(
            &ctx,
            NotificationsListArgs { include_dismissed },
        )
        .await
    }

    #[tauri::command]
    pub async fn notifications_unread_count(ctx: State<'_, Arc<AppCtx>>) -> Result<i64> {
        crate::commands::notifications::notifications_unread_count(
            &ctx,
            NotificationsUnreadCountArgs {},
        )
        .await
    }

    #[tauri::command]
    pub async fn notifications_dismiss(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::notifications::notifications_dismiss(&ctx, NotificationsDismissArgs { id })
            .await
    }

    #[tauri::command]
    pub async fn notifications_dismiss_all(ctx: State<'_, Arc<AppCtx>>) -> Result<u64> {
        crate::commands::notifications::notifications_dismiss_all(
            &ctx,
            NotificationsDismissAllArgs {},
        )
        .await
    }

    #[tauri::command]
    pub async fn notification_rules_list(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Vec<NotificationRule>> {
        crate::commands::notifications::notification_rules_list(&ctx, NotificationRulesListArgs {})
            .await
    }

    #[tauri::command]
    pub async fn notification_rules_set_enabled(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        enabled: bool,
    ) -> Result<()> {
        crate::commands::notifications::notification_rules_set_enabled(
            &ctx,
            NotificationRulesSetEnabledArgs { id, enabled },
        )
        .await
    }

    #[tauri::command]
    pub async fn notifications_run_checks(ctx: State<'_, Arc<AppCtx>>) -> Result<usize> {
        crate::commands::notifications::notifications_run_checks(
            &ctx,
            NotificationsRunChecksArgs {},
        )
        .await
    }
}

pub mod packages {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::commands::packages::*;
    use crate::config::Paths;
    use crate::db::models::{PackageCategoryRow, PackageRevisionRow, PackageRow, SellerProfileRow};
    use crate::db::repo::{audit_log, contacts, mail_accounts, packages, seller_profile};
    use crate::domain::invoice::SellerView;
    use crate::domain::kleinunternehmer;
    use crate::domain::package::{self, PackageRevisionInput};
    use crate::error::{Error, Result};
    use crate::pdf::{templates, typst_render};
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn package_categories_list(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Vec<PackageCategoryRow>> {
        crate::commands::packages::package_categories_list(&ctx, PackageCategoriesListArgs {}).await
    }

    #[tauri::command]
    pub async fn package_category_create(
        ctx: State<'_, Arc<AppCtx>>,
        name: String,
    ) -> Result<PackageCategoryRow> {
        crate::commands::packages::package_category_create(&ctx, PackageCategoryCreateArgs { name })
            .await
    }

    #[tauri::command]
    pub async fn package_category_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        name: String,
    ) -> Result<()> {
        crate::commands::packages::package_category_update(
            &ctx,
            PackageCategoryUpdateArgs { id, name },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_categories_reorder(
        ctx: State<'_, Arc<AppCtx>>,
        ordered_ids: Vec<String>,
    ) -> Result<()> {
        crate::commands::packages::package_categories_reorder(
            &ctx,
            PackageCategoriesReorderArgs { ordered_ids },
        )
        .await
    }

    #[tauri::command]
    pub async fn packages_list(ctx: State<'_, Arc<AppCtx>>) -> Result<Vec<PackageRow>> {
        crate::commands::packages::packages_list(&ctx, PackagesListArgs {}).await
    }

    #[tauri::command]
    pub async fn packages_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<PackageRow>> {
        crate::commands::packages::packages_get(&ctx, PackagesGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn packages_create(
        ctx: State<'_, Arc<AppCtx>>,
        category_id: Option<String>,
        name: String,
        revision: PackageRevisionInput,
    ) -> Result<PackageRow> {
        crate::commands::packages::packages_create(
            &ctx,
            PackagesCreateArgs {
                category_id,
                name,
                revision,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn packages_update_as_new_revision(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        category_id: Option<String>,
        name: String,
        revision: PackageRevisionInput,
    ) -> Result<PackageRow> {
        crate::commands::packages::packages_update_as_new_revision(
            &ctx,
            PackagesUpdateAsNewRevisionArgs {
                id,
                category_id,
                name,
                revision,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn packages_archive(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::packages::packages_archive(&ctx, PackagesArchiveArgs { id }).await
    }

    #[tauri::command]
    pub async fn packages_reactivate(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::packages::packages_reactivate(&ctx, PackagesReactivateArgs { id }).await
    }

    #[tauri::command]
    pub async fn package_revisions_list(
        ctx: State<'_, Arc<AppCtx>>,
        package_id: String,
    ) -> Result<Vec<PackageRevisionRow>> {
        crate::commands::packages::package_revisions_list(
            &ctx,
            PackageRevisionsListArgs { package_id },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_revisions_get(
        ctx: State<'_, Arc<AppCtx>>,
        package_id: String,
        revision: i64,
    ) -> Result<Option<PackageRevisionRow>> {
        crate::commands::packages::package_revisions_get(
            &ctx,
            PackageRevisionsGetArgs {
                package_id,
                revision,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_revisions_rollback(
        ctx: State<'_, Arc<AppCtx>>,
        package_id: String,
        to_revision: i64,
    ) -> Result<PackageRow> {
        crate::commands::packages::package_revisions_rollback(
            &ctx,
            PackageRevisionsRollbackArgs {
                package_id,
                to_revision,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_preview(
        ctx: State<'_, Arc<AppCtx>>,
        title: String,
        body_markup: String,
        default_unit_price_cents: i64,
    ) -> Result<()> {
        crate::commands::packages::package_preview(
            &ctx,
            PackagePreviewArgs {
                title,
                body_markup,
                default_unit_price_cents,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_materialize_item(
        ctx: State<'_, Arc<AppCtx>>,
        package_id: String,
    ) -> Result<MaterializedPackageItem> {
        crate::commands::packages::package_materialize_item(
            &ctx,
            PackageMaterializeItemArgs { package_id },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_catalog_render(
        ctx: State<'_, Arc<AppCtx>>,
        package_ids: Vec<String>,
    ) -> Result<()> {
        crate::commands::packages::package_catalog_render(
            &ctx,
            PackageCatalogRenderArgs { package_ids },
        )
        .await
    }

    #[tauri::command]
    pub async fn package_catalog_send(
        ctx: State<'_, Arc<AppCtx>>,
        args: SendPackageCatalogArgs,
    ) -> Result<SendPackageCatalogResult> {
        crate::commands::packages::package_catalog_send(&ctx, PackageCatalogSendArgs { args }).await
    }
}

pub mod payment_accounts {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::commands::payment_accounts::*;
    use crate::db::models::PaymentAccountRow;
    use crate::db::repo::payment_accounts::PaymentAccountInput;
    use crate::db::repo::{audit_log, payment_accounts};
    use crate::error::Result;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn payment_accounts_list(
        ctx: State<'_, Arc<AppCtx>>,
        include_inactive: Option<bool>,
    ) -> Result<Vec<PaymentAccountRow>> {
        crate::commands::payment_accounts::payment_accounts_list(
            &ctx,
            PaymentAccountsListArgs { include_inactive },
        )
        .await
    }

    #[tauri::command]
    pub async fn payment_accounts_ensure_defaults(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Vec<PaymentAccountRow>> {
        crate::commands::payment_accounts::payment_accounts_ensure_defaults(
            &ctx,
            PaymentAccountsEnsureDefaultsArgs {},
        )
        .await
    }

    #[tauri::command]
    pub async fn payment_accounts_create(
        ctx: State<'_, Arc<AppCtx>>,
        input: PaymentAccountInput,
    ) -> Result<PaymentAccountRow> {
        crate::commands::payment_accounts::payment_accounts_create(
            &ctx,
            PaymentAccountsCreateArgs { input },
        )
        .await
    }

    #[tauri::command]
    pub async fn payment_accounts_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        input: PaymentAccountInput,
    ) -> Result<PaymentAccountRow> {
        crate::commands::payment_accounts::payment_accounts_update(
            &ctx,
            PaymentAccountsUpdateArgs { id, input },
        )
        .await
    }

    #[tauri::command]
    pub async fn payment_accounts_set_active(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        active: bool,
    ) -> Result<()> {
        crate::commands::payment_accounts::payment_accounts_set_active(
            &ctx,
            PaymentAccountsSetActiveArgs { id, active },
        )
        .await
    }
}

pub mod private_movements {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::{self, ArchiveKind};
    use crate::backup;
    use crate::commands::attachments::{guess_mime, sanitize_filename};
    use crate::commands::private_movements::*;
    use crate::config::Paths;
    use crate::db::models::{PrivateMovementListItem, PrivateMovementRow};
    use crate::db::numbering;
    use crate::db::repo::{audit_log, private_movements};
    use crate::domain::numbering::DocType;
    use crate::domain::private_movement::{self, PrivateMovementInput};
    use crate::error::{Error, Result};
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn private_movements_list(
        ctx: State<'_, Arc<AppCtx>>,
        filter: Option<private_movements::ListFilter>,
    ) -> Result<Vec<PrivateMovementListItem>> {
        crate::commands::private_movements::private_movements_list(
            &ctx,
            PrivateMovementsListArgs { filter },
        )
        .await
    }

    #[tauri::command]
    pub async fn private_movements_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<PrivateMovementRow>> {
        crate::commands::private_movements::private_movements_get(
            &ctx,
            PrivateMovementsGetArgs { id },
        )
        .await
    }

    #[tauri::command]
    pub async fn private_movements_create(
        ctx: State<'_, Arc<AppCtx>>,
        args: PrivateMovementCreateArgs,
    ) -> Result<PrivateMovementRow> {
        crate::commands::private_movements::private_movements_create(
            &ctx,
            PrivateMovementsCreateArgs { args },
        )
        .await
    }
}

pub mod quotes {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::{self, ArchiveKind};
    use crate::backup;
    use crate::commands::quotes::*;
    use crate::config::Paths;
    use crate::db::models::{
        AttachmentView, ContactRow, InvoiceDetail, QuoteDetail, QuoteLegalDocumentView,
        QuoteListItem, QuoteRow,
    };
    use crate::db::numbering;
    use crate::db::repo::audit_log;
    use crate::db::repo::invoices::{BuyerSnapshot, SellerSnapshot};
    use crate::db::repo::{attachments, contacts, legal_documents, quotes, seller_profile};
    use crate::domain::invoice::{BuyerView, SellerView};
    use crate::domain::numbering::DocType;
    use crate::domain::quote::{self, QuoteBuyerView, QuoteInput, QuoteItemInput};
    use crate::error::{Error, Result};
    use crate::fiscal_year::guard;
    use crate::pdf::typst_render::QuoteRenderInput;
    use crate::pdf::{bundle, klausel_check, templates, typst_render};
    use chrono::NaiveDate;
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::fs;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn quotes_list(
        ctx: State<'_, Arc<AppCtx>>,
        filter: Option<quotes::ListFilter>,
    ) -> Result<Vec<QuoteListItem>> {
        crate::commands::quotes::quotes_list(&ctx, QuotesListArgs { filter }).await
    }

    #[tauri::command]
    pub async fn quotes_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<QuoteDetail>> {
        crate::commands::quotes::quotes_get(&ctx, QuotesGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn quotes_attachments_list(
        ctx: State<'_, Arc<AppCtx>>,
        quote_id: String,
    ) -> Result<Vec<AttachmentView>> {
        crate::commands::quotes::quotes_attachments_list(
            &ctx,
            QuotesAttachmentsListArgs { quote_id },
        )
        .await
    }

    #[tauri::command]
    pub async fn quotes_create_draft(
        ctx: State<'_, Arc<AppCtx>>,
        args: CreateDraftArgs,
    ) -> Result<QuoteDetail> {
        crate::commands::quotes::quotes_create_draft(&ctx, QuotesCreateDraftArgs { args }).await
    }

    #[tauri::command]
    pub async fn quotes_validate_draft(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Vec<ValidationIssueDto>> {
        crate::commands::quotes::quotes_validate_draft(&ctx, QuotesValidateDraftArgs { id }).await
    }

    #[tauri::command]
    pub async fn quotes_issue(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<QuoteRow> {
        crate::commands::quotes::quotes_issue(&ctx, QuotesIssueArgs { id }).await
    }

    #[tauri::command]
    pub async fn quotes_accept(
        ctx: State<'_, Arc<AppCtx>>,
        args: AcceptArgs,
    ) -> Result<QuoteDetail> {
        crate::commands::quotes::quotes_accept(&ctx, QuotesAcceptArgs { args }).await
    }

    #[tauri::command]
    pub async fn quotes_reject(ctx: State<'_, Arc<AppCtx>>, args: RejectArgs) -> Result<QuoteRow> {
        crate::commands::quotes::quotes_reject(&ctx, QuotesRejectArgs { args }).await
    }

    #[tauri::command]
    pub async fn quotes_cancel(ctx: State<'_, Arc<AppCtx>>, args: CancelArgs) -> Result<QuoteRow> {
        crate::commands::quotes::quotes_cancel(&ctx, QuotesCancelArgs { args }).await
    }

    #[tauri::command]
    pub async fn quotes_convert_to_invoice(
        ctx: State<'_, Arc<AppCtx>>,
        args: ConvertArgs,
    ) -> Result<InvoiceDetail> {
        crate::commands::quotes::quotes_convert_to_invoice(
            &ctx,
            QuotesConvertToInvoiceArgs { args },
        )
        .await
    }

    #[tauri::command]
    pub async fn quotes_generate_pdf(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<QuoteRow> {
        crate::commands::quotes::quotes_generate_pdf(&ctx, QuotesGeneratePdfArgs { id }).await
    }

    #[tauri::command]
    pub async fn quotes_open_bundle(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<()> {
        crate::commands::quotes::quotes_open_bundle(&ctx, QuotesOpenBundleArgs { id }).await
    }

    #[tauri::command]
    pub async fn quotes_legal_bindings(
        ctx: State<'_, Arc<AppCtx>>,
        quote_id: String,
    ) -> Result<Vec<QuoteLegalDocumentView>> {
        crate::commands::quotes::quotes_legal_bindings(&ctx, QuotesLegalBindingsArgs { quote_id })
            .await
    }
}

pub mod recurring {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup;
    use crate::commands::recurring::*;
    use crate::config::Paths;
    use crate::db::models::{ExpenseRow, RecurringSubscriptionRow};
    use crate::db::repo::{audit_log, recurring};
    use crate::domain::recurring::{self as domain, RecurringInput};
    use crate::error::{Error, Result};
    use crate::scheduler::recurring::{process_due, run_now, ProcessReport};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn recurring_list(
        ctx: State<'_, Arc<AppCtx>>,
        include_inactive: Option<bool>,
    ) -> Result<Vec<RecurringSubscriptionRow>> {
        crate::commands::recurring::recurring_list(&ctx, RecurringListArgs { include_inactive })
            .await
    }

    #[tauri::command]
    pub async fn recurring_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<RecurringSubscriptionRow>> {
        crate::commands::recurring::recurring_get(&ctx, RecurringGetArgs { id }).await
    }

    #[tauri::command]
    pub async fn recurring_create(
        ctx: State<'_, Arc<AppCtx>>,
        input: RecurringInput,
    ) -> Result<RecurringSubscriptionRow> {
        crate::commands::recurring::recurring_create(&ctx, RecurringCreateArgs { input }).await
    }

    #[tauri::command]
    pub async fn recurring_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        input: RecurringInput,
    ) -> Result<RecurringSubscriptionRow> {
        crate::commands::recurring::recurring_update(&ctx, RecurringUpdateArgs { id, input }).await
    }

    #[tauri::command]
    pub async fn recurring_set_active(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        active: bool,
    ) -> Result<RecurringSubscriptionRow> {
        crate::commands::recurring::recurring_set_active(
            &ctx,
            RecurringSetActiveArgs { id, active },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_run_now(ctx: State<'_, Arc<AppCtx>>, id: String) -> Result<ExpenseRow> {
        crate::commands::recurring::recurring_run_now(&ctx, RecurringRunNowArgs { id }).await
    }

    #[tauri::command]
    pub async fn recurring_run_due_check(ctx: State<'_, Arc<AppCtx>>) -> Result<ProcessReport> {
        crate::commands::recurring::recurring_run_due_check(&ctx, RecurringRunDueCheckArgs {}).await
    }
}

pub mod recurring_invoice {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup;
    use crate::commands::recurring_invoice::*;
    use crate::config::Paths;
    use crate::db::models::{RecurringInvoiceDetail, RecurringInvoiceRow};
    use crate::db::repo::{audit_log, recurring_invoice, seller_profile};
    use crate::domain::invoice::compute_totals;
    use crate::domain::kleinunternehmer::{self, ItemVatCheck, KleinunternehmerStatus};
    use crate::domain::recurring_invoice::{self as domain, RecurringInvoiceInput};
    use crate::error::{Error, Result};
    use crate::scheduler::recurring_invoice::{process_due, run_now, ProcessReport};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn recurring_invoices_list(
        ctx: State<'_, Arc<AppCtx>>,
        include_inactive: Option<bool>,
    ) -> Result<Vec<RecurringInvoiceRow>> {
        crate::commands::recurring_invoice::recurring_invoices_list(
            &ctx,
            RecurringInvoicesListArgs { include_inactive },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_invoices_get(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<Option<RecurringInvoiceDetail>> {
        crate::commands::recurring_invoice::recurring_invoices_get(
            &ctx,
            RecurringInvoicesGetArgs { id },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_invoices_create(
        ctx: State<'_, Arc<AppCtx>>,
        input: RecurringInvoiceInput,
    ) -> Result<RecurringInvoiceDetail> {
        crate::commands::recurring_invoice::recurring_invoices_create(
            &ctx,
            RecurringInvoicesCreateArgs { input },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_invoices_update(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        input: RecurringInvoiceInput,
    ) -> Result<RecurringInvoiceDetail> {
        crate::commands::recurring_invoice::recurring_invoices_update(
            &ctx,
            RecurringInvoicesUpdateArgs { id, input },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_invoices_set_active(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
        active: bool,
    ) -> Result<RecurringInvoiceRow> {
        crate::commands::recurring_invoice::recurring_invoices_set_active(
            &ctx,
            RecurringInvoicesSetActiveArgs { id, active },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_invoices_run_now(
        ctx: State<'_, Arc<AppCtx>>,
        id: String,
    ) -> Result<String> {
        crate::commands::recurring_invoice::recurring_invoices_run_now(
            &ctx,
            RecurringInvoicesRunNowArgs { id },
        )
        .await
    }

    #[tauri::command]
    pub async fn recurring_invoices_run_due_check(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<ProcessReport> {
        crate::commands::recurring_invoice::recurring_invoices_run_due_check(
            &ctx,
            RecurringInvoicesRunDueCheckArgs {},
        )
        .await
    }
}

pub mod settings {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::backup::BackupSession;
    use crate::branding;
    use crate::commands::settings::*;
    use crate::config::Paths;
    use crate::db::models::SellerProfileRow;
    use crate::db::repo::seller_profile::{self, SellerProfileInput};
    use crate::domain::kleinunternehmer::{hinweis_text, waiver_deadline};
    use crate::error::{Error, Result};
    use serde::{Deserialize, Serialize};
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn seller_profile_get(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Option<SellerProfileRow>> {
        crate::commands::settings::seller_profile_get(&ctx, SellerProfileGetArgs {}).await
    }

    #[tauri::command]
    pub async fn seller_profile_upsert(
        ctx: State<'_, Arc<AppCtx>>,
        input: SellerProfileInput,
    ) -> Result<SellerProfileRow> {
        crate::commands::settings::seller_profile_upsert(&ctx, SellerProfileUpsertArgs { input })
            .await
    }

    #[tauri::command]
    pub async fn seller_logo_set(
        ctx: State<'_, Arc<AppCtx>>,
        file_bytes: Vec<u8>,
        file_name: String,
    ) -> Result<SellerProfileRow> {
        crate::commands::settings::seller_logo_set(
            &ctx,
            SellerLogoSetArgs {
                file_bytes,
                file_name,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn seller_logo_clear(ctx: State<'_, Arc<AppCtx>>) -> Result<SellerProfileRow> {
        crate::commands::settings::seller_logo_clear(&ctx, SellerLogoClearArgs {}).await
    }

    #[tauri::command]
    pub async fn seller_logo_data(ctx: State<'_, Arc<AppCtx>>) -> Result<Option<String>> {
        crate::commands::settings::seller_logo_data(&ctx, SellerLogoDataArgs {}).await
    }

    #[tauri::command]
    pub async fn pdf_templates_list(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<Vec<crate::pdf::templates::TemplateMeta>> {
        crate::commands::settings::pdf_templates_list(&ctx, PdfTemplatesListArgs {}).await
    }

    #[tauri::command]
    pub async fn pdf_template_preview(ctx: State<'_, Arc<AppCtx>>, name: String) -> Result<()> {
        crate::commands::settings::pdf_template_preview(&ctx, PdfTemplatePreviewArgs { name }).await
    }

    #[tauri::command]
    pub async fn seller_default_template_set(
        ctx: State<'_, Arc<AppCtx>>,
        name: String,
    ) -> Result<SellerProfileRow> {
        crate::commands::settings::seller_default_template_set(
            &ctx,
            SellerDefaultTemplateSetArgs { name },
        )
        .await
    }

    #[tauri::command]
    pub async fn paragraph_19_info(ctx: State<'_, Arc<AppCtx>>) -> Result<Paragraph19Info> {
        crate::commands::settings::paragraph_19_info(&ctx, Paragraph19InfoArgs {}).await
    }

    #[tauri::command]
    pub async fn travel_settings_get(ctx: State<'_, Arc<AppCtx>>) -> Result<TravelSettings> {
        crate::commands::settings::travel_settings_get(&ctx, TravelSettingsGetArgs {}).await
    }

    #[tauri::command]
    pub async fn travel_settings_set(
        ctx: State<'_, Arc<AppCtx>>,
        cost_per_km_cents: i64,
        round_trip_default: bool,
    ) -> Result<()> {
        crate::commands::settings::travel_settings_set(
            &ctx,
            TravelSettingsSetArgs {
                cost_per_km_cents,
                round_trip_default,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn travel_compute(
        ctx: State<'_, Arc<AppCtx>>,
        km: f64,
        round_trip: bool,
    ) -> Result<crate::domain::travel::TravelLine> {
        crate::commands::settings::travel_compute(&ctx, TravelComputeArgs { km, round_trip }).await
    }

    #[tauri::command]
    pub async fn seller_signature_set(
        ctx: State<'_, Arc<AppCtx>>,
        file_bytes: Vec<u8>,
        file_name: String,
    ) -> Result<Option<String>> {
        crate::commands::settings::seller_signature_set(
            &ctx,
            SellerSignatureSetArgs {
                file_bytes,
                file_name,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn seller_signature_clear(ctx: State<'_, Arc<AppCtx>>) -> Result<()> {
        crate::commands::settings::seller_signature_clear(&ctx, SellerSignatureClearArgs {}).await
    }

    #[tauri::command]
    pub async fn seller_signature_data(ctx: State<'_, Arc<AppCtx>>) -> Result<Option<String>> {
        crate::commands::settings::seller_signature_data(&ctx, SellerSignatureDataArgs {}).await
    }

    #[tauri::command]
    pub async fn quote_signature_get(ctx: State<'_, Arc<AppCtx>>) -> Result<bool> {
        crate::commands::settings::quote_signature_get(&ctx, QuoteSignatureGetArgs {}).await
    }

    #[tauri::command]
    pub async fn quote_signature_set(ctx: State<'_, Arc<AppCtx>>, enabled: bool) -> Result<()> {
        crate::commands::settings::quote_signature_set(&ctx, QuoteSignatureSetArgs { enabled })
            .await
    }

    #[tauri::command]
    pub async fn seller_owner_get(ctx: State<'_, Arc<AppCtx>>) -> Result<Option<String>> {
        crate::commands::settings::seller_owner_get(&ctx, SellerOwnerGetArgs {}).await
    }

    #[tauri::command]
    pub async fn seller_owner_set(
        ctx: State<'_, Arc<AppCtx>>,
        owner_name: Option<String>,
    ) -> Result<()> {
        crate::commands::settings::seller_owner_set(&ctx, SellerOwnerSetArgs { owner_name }).await
    }

    #[tauri::command]
    pub async fn document_terms_get(ctx: State<'_, Arc<AppCtx>>) -> Result<DocumentTerms> {
        crate::commands::settings::document_terms_get(&ctx, DocumentTermsGetArgs {}).await
    }

    #[tauri::command]
    pub async fn document_terms_set(
        ctx: State<'_, Arc<AppCtx>>,
        quote_valid_days: i64,
        invoice_due_days: i64,
    ) -> Result<()> {
        crate::commands::settings::document_terms_set(
            &ctx,
            DocumentTermsSetArgs {
                quote_valid_days,
                invoice_due_days,
            },
        )
        .await
    }

    #[tauri::command]
    pub async fn drop_folder_settings_get(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<DropFolderSettings> {
        crate::commands::settings::drop_folder_settings_get(&ctx, DropFolderSettingsGetArgs {})
            .await
    }

    #[tauri::command]
    pub async fn drop_folder_settings_set(
        ctx: State<'_, Arc<AppCtx>>,
        args: DropFolderSettings,
    ) -> Result<DropFolderSettings> {
        crate::commands::settings::drop_folder_settings_set(
            &ctx,
            DropFolderSettingsSetArgs { args },
        )
        .await
    }

    #[tauri::command]
    pub async fn drop_folder_sync_now(ctx: State<'_, Arc<AppCtx>>) -> Result<DropFolderSyncResult> {
        crate::commands::settings::drop_folder_sync_now(&ctx, DropFolderSyncNowArgs {}).await
    }
}

pub mod system {
    #![allow(unused_imports)]
    use crate::app_ctx::AppCtx;
    use crate::archive::integrity_check::{self, IntegrityCheckSummary};
    use crate::commands::system::*;
    use crate::db::repo::audit_log::{self, AuditEntry};
    use crate::db::schema_version::EXPECTED_SCHEMA_VERSION;
    use crate::error::{Error, Result};
    use serde::Serialize;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tauri::State;

    #[tauri::command]
    pub async fn audit_trail_list(
        ctx: State<'_, Arc<AppCtx>>,
        limit: Option<i64>,
    ) -> Result<Vec<AuditEntry>> {
        crate::commands::system::audit_trail_list(&ctx, AuditTrailListArgs { limit }).await
    }

    #[tauri::command]
    pub async fn archive_integrity_run(
        ctx: State<'_, Arc<AppCtx>>,
    ) -> Result<IntegrityCheckSummary> {
        crate::commands::system::archive_integrity_run(&ctx, ArchiveIntegrityRunArgs {}).await
    }

    #[tauri::command]
    pub async fn archive_integrity_history(
        ctx: State<'_, Arc<AppCtx>>,
        limit: Option<i64>,
    ) -> Result<Vec<IntegrityCheckRow>> {
        crate::commands::system::archive_integrity_history(
            &ctx,
            ArchiveIntegrityHistoryArgs { limit },
        )
        .await
    }

    #[tauri::command]
    pub async fn app_info(ctx: State<'_, Arc<AppCtx>>) -> Result<AppInfo> {
        Ok(crate::commands::system::app_info(&ctx, AppInfoArgs {}).await)
    }

    #[tauri::command]
    pub async fn third_party_licenses_path(ctx: State<'_, Arc<AppCtx>>) -> Result<String> {
        crate::commands::system::third_party_licenses_path(&ctx, ThirdPartyLicensesPathArgs {})
            .await
    }
}
