//! Lauf 2026.11 U4 — neue Lesefelder und der Lese-Command der Warteliste
//! (TASKS.md „Lauf 2026.11 – Stand“, Offene Punkte 1, 3, 5, 9, 13), gegen eine
//! Tempfile-SQLite mit dem echten `MIGRATOR`:
//!
//!   * DATEV-/Steuerberater-Export nehmen Berater- und Mandantennummer an (1);
//!   * `euer_compute_report` trägt §19-Status des Jahres und Snapshot-Abgleich (3);
//!   * `assets_disposal_preview` rechnet wie `assets_dispose`, ohne zu buchen (5);
//!   * `QuoteListItem.lockedAt` trennt verworfene Entwürfe von zurückgezogenen
//!     Angeboten (9);
//!   * `backup_get_settings` liefert Stand und Alter der Off-Site-Kopien (13).
//!
//! Testdaten nur mit Dokumentations-Adressen (example.org).

use std::str::FromStr;
use std::sync::Arc;

use chrono::NaiveDate;
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::{assets as assets_cmd, backup as backup_cmd, euer, quotes};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::MIGRATOR;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

const CONTACT: &str = "01928300-0000-7000-8000-00000000c001";

async fn setup() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = format!(
        "sqlite://{}",
        dir.path().join("klein-buch.sqlite").to_string_lossy()
    );
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO contacts (id, contact_type, name, email, country_code)
         VALUES (?, 'customer', 'Kunde Beispiel', 'kunde@example.org', 'DE')",
    )
    .bind(CONTACT)
    .execute(&pool)
    .await
    .unwrap();
    (pool, dir)
}

fn ctx_at(pool: &SqlitePool, dir: &std::path::Path, day: (i32, u32, u32)) -> Arc<AppCtx> {
    let ctx = AppCtx::for_tests(Paths::under(dir));
    ctx.set_pool(pool.clone());
    ctx.set_clock(Arc::new(klein_buch_lib::clock::FixedClock::at_berlin(
        d(day.0, day.1, day.2),
        12,
        0,
    )));
    ctx
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

// ============================================================================
// (1) DATEV: Berater- und Mandantennummer
// ============================================================================

#[tokio::test]
async fn datev_export_takes_consultant_and_client_number() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 6, 10));

    // Schlüssel wie aus `api.ts` (camelCase, null erlaubt).
    let args: euer::EuerExportDatevArgs = serde_json::from_value(serde_json::json!({
        "fiscalYear": 2026, "skr": "SKR03", "consultantNumber": 29098, "clientNumber": 55003
    }))
    .unwrap();
    let out = euer::euer_export_datev(&ctx, args).await.unwrap();
    let entry = ctx.downloads.take(&out.download.download_id).unwrap();
    let bytes = std::fs::read(&entry.path).unwrap();
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(400)]).to_string();
    assert!(
        head.contains("29098") && head.contains("55003"),
        "DATEV-Kopf: {head}"
    );

    let args: euer::EuerExportStbZipArgs = serde_json::from_value(serde_json::json!({
        "fiscalYear": 2026, "skr": "SKR03", "consultantNumber": null, "clientNumber": null
    }))
    .unwrap();
    assert_eq!((args.consultant_number, args.client_number), (None, None));

    // Außerhalb der DATEV-Wertebereiche → klare Ablehnung.
    let args: euer::EuerExportDatevArgs = serde_json::from_value(serde_json::json!({
        "fiscalYear": 2026, "skr": "SKR03", "consultantNumber": 999
    }))
    .unwrap();
    let err = euer::euer_export_datev(&ctx, args).await.unwrap_err();
    assert!(err.to_string().contains("Beraternummer"), "{err}");
}

// ============================================================================
// (3) EÜR: §19-Status des Jahres und Snapshot-Abgleich
// ============================================================================

async fn lock_year(pool: &SqlitePool, year: i64, income: i64) {
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
                afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (?, ?, 0, 0, ?, 'test', 32)",
    )
    .bind(year)
    .bind(income)
    .bind(income)
    .execute(pool)
    .await
    .unwrap();
}

async fn report(ctx: &Arc<AppCtx>, year: i64) -> serde_json::Value {
    let view = euer::euer_compute_report(ctx, euer::EuerComputeReportArgs { fiscal_year: year })
        .await
        .unwrap();
    serde_json::to_value(view).unwrap()
}

#[tokio::test]
async fn euer_report_carries_paragraph_19_status_and_snapshot_check() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 6, 10));

    // Ohne Profil gilt §19; offenes Jahr ohne Snapshot. Report-Felder bleiben flach.
    let r = report(&ctx, 2026).await;
    assert_eq!(r["isKleinunternehmer"], true, "{r}");
    assert!(r["snapshotCheck"].is_null(), "{r}");
    assert_eq!(r["fiscalYear"], 2026);
    assert_eq!(r["surplusCents"], 0);

    // Verzicht ab 2026: 2026 Regelbesteuerung, 2025 weiter §19.
    sqlx::query(
        "INSERT INTO seller_profile (id, name, street, postal_code, city, tax_number, email,
                                     is_kleinunternehmer, waived_paragraph_19_since)
         VALUES (1, 'Verkäufer', 'Beispielweg 1', '84028', 'Landshut', '123/456/78901',
                 'verkaeufer@example.org', 0, '2026-01-01')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(report(&ctx, 2026).await["isKleinunternehmer"], false);
    assert_eq!(report(&ctx, 2025).await["isKleinunternehmer"], true);

    // Abgeschlossene Jahre: passender bzw. abweichender Snapshot.
    lock_year(&pool, 2025, 0).await;
    lock_year(&pool, 2024, 12_345).await;
    let r = report(&ctx, 2025).await;
    assert_eq!(r["snapshotCheck"]["fiscalYear"], 2025, "{r}");
    assert_eq!(r["snapshotCheck"]["matches"], true, "{r}");
    let r = report(&ctx, 2024).await;
    assert_eq!(r["snapshotCheck"]["matches"], false, "{r}");
    let diffs = r["snapshotCheck"]["differences"].as_array().unwrap();
    assert!(diffs.iter().any(|v| v == "income"), "{r}");

    // Reine Anzeige: weder Audit noch Hinweis (anders als die Exporte).
    assert_eq!(
        scalar(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'euer.snapshot_mismatch'"
        )
        .await,
        0
    );
}

// ============================================================================
// (5) Anlagen: Vorschau des Abgangs
// ============================================================================

async fn mk_asset(pool: &SqlitePool, id: &str, acq: &str, cost: i64, life: f64, book: i64) {
    let year: i64 = acq[..4].parse().unwrap();
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
                             acquisition_fiscal_year, depreciation_method, useful_life_years,
                             book_value_cents)
         VALUES (?, ?, 'Server', ?, ?, ?, 'linear', ?, ?)",
    )
    .bind(id)
    .bind(format!("AV-{year}-000{}", &id[id.len() - 1..]))
    .bind(acq)
    .bind(cost)
    .bind(year)
    .bind(life)
    .bind(book)
    .execute(pool)
    .await
    .unwrap();
}

async fn book_entry(pool: &SqlitePool, asset: &str, year: i64, before: i64, amount: i64) {
    sqlx::query(
        "INSERT INTO depreciation_entries (id, asset_id, fiscal_year, depreciation_amount_cents,
                months_in_year, book_value_before_cents, book_value_after_cents)
         VALUES (?, ?, ?, ?, 12, ?, ?)",
    )
    .bind(format!("{asset}-{year}"))
    .bind(asset)
    .bind(year)
    .bind(amount)
    .bind(before)
    .bind(before - amount)
    .execute(pool)
    .await
    .unwrap();
}

async fn preview(
    ctx: &Arc<AppCtx>,
    id: &str,
    date: (i32, u32, u32),
) -> klein_buch_lib::error::Result<assets_cmd::AssetDisposalPreview> {
    assets_cmd::assets_disposal_preview(
        ctx,
        assets_cmd::AssetsDisposalPreviewArgs {
            id: id.into(),
            disposal_date: d(date.0, date.1, date.2),
        },
    )
    .await
}

async fn dispose(ctx: &Arc<AppCtx>, id: &str, date: (i32, u32, u32)) -> i64 {
    let detail = assets_cmd::assets_dispose(
        ctx,
        assets_cmd::AssetsDisposeArgs {
            args: assets_cmd::AssetDisposeArgs {
                asset_id: id.into(),
                disposal_date: d(date.0, date.1, date.2),
                disposal_type: "scrap".into(),
                proceeds_cents: 0,
            },
        },
    )
    .await
    .unwrap();
    detail.asset.disposal_residual_book_value_cents.unwrap()
}

/// Nachholen der Vorjahre und anteilige AfA: die Vorschau bucht nichts und
/// trifft den Restbuchwert, den der Abgang danach festhält.
#[tokio::test]
async fn disposal_preview_matches_dispose_with_catch_up() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 7, 1));
    mk_asset(&pool, "as1", "2025-01-01", 120_000, 4.0, 120_000).await;
    let audits = "SELECT COUNT(*) FROM audit_log";
    let audit_before = scalar(&pool, audits).await;

    let p = preview(&ctx, "as1", (2026, 6, 30)).await.unwrap();
    assert_eq!(p.current_book_value_cents, 120_000);
    assert_eq!(p.catch_up_years, vec![2025]);
    assert_eq!(p.catch_up_depreciation_cents, 30_000);
    assert!(p.replaced_years.is_empty());
    assert_eq!(p.book_value_before_cents, 90_000);
    assert_eq!(
        (p.disposal_year_depreciation_cents, p.disposal_year_months),
        (15_000, 6)
    );
    assert_eq!(p.residual_book_value_cents, 75_000);
    assert_eq!(p.depreciation_to_date_cents, 45_000);
    assert_eq!(p.blocked_reason, None);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["residualBookValueCents"], 75_000);
    assert_eq!(json["disposalDate"], "2026-06-30");

    // Liest nur.
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM depreciation_entries").await,
        0
    );
    assert_eq!(scalar(&pool, audits).await, audit_before);

    assert_eq!(
        dispose(&ctx, "as1", (2026, 6, 30)).await,
        p.residual_book_value_cents
    );
    let err = preview(&ctx, "as1", (2026, 6, 30)).await.unwrap_err();
    assert!(err.to_string().contains("bereits veräußert"), "{err}");
}

/// Offene AfA ab dem Abgangsjahr wird ersetzt (wie F3b); Vorschau = Abgang.
#[tokio::test]
async fn disposal_preview_matches_dispose_replacing_later_years() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 12, 15));
    mk_asset(&pool, "as2", "2024-01-01", 120_000, 3.0, 1).await;
    for (year, before) in [(2024, 120_000), (2025, 80_000), (2026, 40_000)] {
        book_entry(&pool, "as2", year, before, 40_000.min(before - 1)).await;
    }
    let p = preview(&ctx, "as2", (2025, 10, 15)).await.unwrap();
    assert_eq!(p.replaced_years, vec![2025, 2026]);
    assert_eq!(p.replaced_depreciation_cents, 40_000 + 39_999);
    assert_eq!(p.book_value_before_cents, 80_000);
    assert_eq!(p.disposal_year_depreciation_cents, 33_333);
    assert_eq!(p.residual_book_value_cents, 46_667);
    assert_eq!(p.blocked_reason, None);
    assert_eq!(dispose(&ctx, "as2", (2025, 10, 15)).await, 46_667);
}

/// Gründe, aus denen der Abgang ablehnen würde, stehen in `blockedReason`
/// (gleiche Texte wie die Ablehnung); die Zahlen bleiben lesbar.
#[tokio::test]
async fn disposal_preview_reports_blocking_reasons() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2027, 3, 1));
    mk_asset(&pool, "as3", "2024-01-01", 120_000, 3.0, 40_000).await;
    book_entry(&pool, "as3", 2024, 120_000, 40_000).await;
    book_entry(&pool, "as3", 2025, 80_000, 40_000).await;

    let future = preview(&ctx, "as3", (2027, 3, 2)).await.unwrap();
    assert!(
        future
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("Zukunft"),
        "{future:?}"
    );
    let before = preview(&ctx, "as3", (2023, 12, 31)).await.unwrap();
    assert!(
        before
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("vor dem Anschaffungsdatum"),
        "{before:?}"
    );

    lock_year(&pool, 2026, 0).await;
    // Abgang im abgeschlossenen Jahr selbst.
    let closed = preview(&ctx, "as3", (2026, 5, 1)).await.unwrap();
    assert!(
        closed
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("Das Geschäftsjahr 2026 ist abgeschlossen (festgeschrieben)"),
        "{closed:?}"
    );
    // Abgang vor einem abgeschlossenen Jahr, das die Anlage im Bestand führt.
    let conflict = preview(&ctx, "as3", (2025, 6, 1)).await.unwrap();
    let reason = conflict.blocked_reason.clone().unwrap();
    assert!(
        reason.contains("2026 ist abgeschlossen") && reason.contains("frühestens 2027"),
        "{reason}"
    );
    let err = assets_cmd::assets_dispose(
        &ctx,
        assets_cmd::AssetsDisposeArgs {
            args: assets_cmd::AssetDisposeArgs {
                asset_id: "as3".into(),
                disposal_date: d(2025, 6, 1),
                disposal_type: "scrap".into(),
                proceeds_cents: 0,
            },
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().ends_with(&reason), "{err} / {reason}");
    // Abgang nach dem letzten Abschluss geht.
    let ok = preview(&ctx, "as3", (2027, 2, 1)).await.unwrap();
    assert_eq!(ok.blocked_reason, None, "{ok:?}");
}

// ============================================================================
// (9) Angebote: verworfen vs. zurückgezogen
// ============================================================================

async fn mk_quote(pool: &SqlitePool, id: &str, number: &str) {
    sqlx::query(
        "INSERT INTO quotes (
            id, quote_number, fiscal_year, quote_date, valid_until, contact_id,
            seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents
         ) VALUES (?, ?, 2026, '2026-05-20', '2026-06-20', ?,
            'Verkäufer', 'Beispielweg 1', '84028', 'Landshut', 10000, 10000)",
    )
    .bind(id)
    .bind(number)
    .bind(CONTACT)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn quote_list_tells_discarded_drafts_from_withdrawn_quotes() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 6, 10));
    mk_quote(&pool, "q1", "AN-2026-0001").await;
    mk_quote(&pool, "q2", "AN-2026-0002").await;
    klein_buch_lib::db::repo::quotes::issue(&pool, "q2")
        .await
        .unwrap();
    for id in ["q1", "q2"] {
        klein_buch_lib::db::repo::quotes::cancel(&pool, id, "Kunde hat abgesagt")
            .await
            .unwrap();
    }
    let rows = quotes::quotes_list(&ctx, quotes::QuotesListArgs { filter: None })
        .await
        .unwrap();
    let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap().clone();
    assert_eq!(row("q1").status, "canceled");
    assert_eq!(row("q1").locked_at, None, "verworfener Entwurf");
    assert_eq!(row("q2").status, "canceled");
    assert!(row("q2").locked_at.is_some(), "zurückgezogenes Angebot");
    let json = serde_json::to_value(row("q2")).unwrap();
    assert!(json["lockedAt"].is_string(), "{json}");
}

// ============================================================================
// (13) Off-Site-Alter für die Übersicht
// ============================================================================

async fn settings(ctx: &Arc<AppCtx>) -> backup_cmd::BackupSettings {
    backup_cmd::backup_get_settings(ctx, backup_cmd::BackupGetSettingsArgs {})
        .await
        .unwrap()
}

#[tokio::test]
async fn backup_settings_report_offsite_copy_age() {
    let (pool, dir) = setup().await;
    let ctx = ctx_at(&pool, dir.path(), (2026, 6, 10));
    // Ohne Ziel: keine Aussage.
    let s = settings(&ctx).await;
    assert_eq!(s.offsite_status, "missing");
    assert_eq!((s.offsite_last_copy_at, s.offsite_age_days), (None, None));

    klein_buch_lib::backup::set_setting(
        &pool,
        "backup_targets",
        r#"[{"id":"a","label":"NAS","kind":"directory","path":"nas/kb"},
            {"id":"b","label":"USB","kind":"directory","path":"usb/kb"}]"#,
    )
    .await
    .unwrap();
    // NAS scheiterte zuletzt; die letzte gute Kopie steht nur im Protokoll.
    klein_buch_lib::backup::set_setting(
        &pool,
        "backup_offsite_results",
        r#"{"a":{"ok":false,"at":"2026-06-09T02:30:00+02:00","error":"offline"},
            "b":{"ok":true,"at":"2026-06-08T02:30:00+02:00","error":null}}"#,
    )
    .await
    .unwrap();
    for (created, status) in [
        ("2026-05-31 00:30:00", "ok"),
        ("2026-06-01 00:30:00", "ok"),
        ("2026-06-09 00:30:00", "failed"),
    ] {
        sqlx::query(
            "INSERT INTO backup_log (id, created_at, trigger, target_kind, target_label,
                    file_name, full_path, size_bytes, status)
             VALUES (?, ?, 'auto_daily', 'directory', 'NAS', 'klein-buch.kbk',
                     '/backup-targets/nas/kb/klein-buch.kbk', 1, ?)",
        )
        .bind(format!("log-{created}"))
        .bind(created)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
    }
    let s = settings(&ctx).await;
    assert_eq!(s.offsite_status, "failing");
    let nas = s.targets.iter().find(|t| t.id == "a").unwrap();
    assert_eq!(
        nas.last_copy_at.as_deref(),
        Some("2026-06-01T02:30:00+02:00")
    );
    let usb = s.targets.iter().find(|t| t.id == "b").unwrap();
    assert_eq!(
        usb.last_copy_at.as_deref(),
        Some("2026-06-08T02:30:00+02:00")
    );
    assert_eq!(
        s.offsite_last_copy_at.as_deref(),
        Some("2026-06-01T02:30:00+02:00")
    );
    assert_eq!(s.offsite_age_days, Some(9));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["offsiteAgeDays"], 9, "{json}");
    assert!(json["targets"][0].get("lastCopyAt").is_some(), "{json}");

    // Ein neues Ziel ohne Kopie: Stand unbekannt.
    klein_buch_lib::backup::set_setting(
        &pool,
        "backup_targets",
        r#"[{"id":"a","label":"NAS","kind":"directory","path":"nas/kb"},
            {"id":"b","label":"USB","kind":"directory","path":"usb/kb"},
            {"id":"c","label":"Neu","kind":"directory","path":"neu/kb"}]"#,
    )
    .await
    .unwrap();
    let s = settings(&ctx).await;
    let neu = s.targets.iter().find(|t| t.id == "c").unwrap();
    assert_eq!(neu.last_copy_at, None);
    assert_eq!((s.offsite_last_copy_at, s.offsite_age_days), (None, None));
}
