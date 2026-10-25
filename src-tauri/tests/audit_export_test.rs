//! Lauf F, Block F2b — Audit der Exporte, Snapshot-Abgleich, Audit-JSON und
//! Akteur.
//!
//! * R2-016 / ASVS 8.3.5: jeder EÜR-Export (PDF, DATEV, ELSTER,
//!   Einzelaufstellung, Steuerberater-Paket) schreibt genau einen
//!   Audit-Eintrag mit Geschäftsjahr, Dateiname und Snapshot-Status — ohne
//!   Inhalte.
//! * KB-0031: Weicht die Live-Rechnung eines abgeschlossenen Jahres vom
//!   Festschreibungs-Snapshot ab, melden Anzeige und Exporte das (Audit
//!   `euer.snapshot_mismatch`, Hinweis einmal je Jahr, Block in LIESMICH).
//! * W14-010 / R2-S2-AUDITJSON: alle `details_json` sind gültiges JSON, auch mit
//!   Anführungs- und Steuerzeichen in Nutzereingaben.
//! * R1-S3-ACTOR: Commands über die Sitzung schreiben `owner`, direkte
//!   Aufrufe ohne Kontext `system`.
//!
//! Tempfile-DB mit den echten Migrationen; nur Dokumentations-Adressen.

mod server_support;

use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::app_ctx::AppCtx;
use klein_buch_lib::commands::{euer as euer_cmd, expenses as expenses_cmd};
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::numbering;
use klein_buch_lib::db::repo::expenses as expenses_repo;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::domain::numbering::DocType;
use klein_buch_lib::fiscal_year::lock;
use server_support::{call, test_server, PASS};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

struct Env {
    pool: SqlitePool,
    ctx: Arc<AppCtx>,
    _dir: tempfile::TempDir,
}

fn paths_for(dir: &Path) -> Paths {
    let data = dir.join("data");
    for d in ["archive", "backups", "tmp"] {
        std::fs::create_dir_all(data.join(d)).unwrap();
    }
    std::fs::create_dir_all(dir.join("inputs")).unwrap();
    Paths {
        data_dir: data.clone(),
        db_file: data.join("klein-buch.sqlite"),
        archive_dir: data.join("archive"),
        backups_dir: data.join("backups"),
        inputs_dir: dir.join("inputs"),
        sidecar_dir: dir.join("sidecar"),
        resource_dir: std::path::PathBuf::from("/non/existent/resources"),
        web_dir: std::path::PathBuf::from("/non/existent/web"),
        backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
    }
}

async fn setup() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_for(dir.path());
    let url = format!("sqlite://{}", paths.db_file.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    let ctx = AppCtx::for_tests(paths);
    ctx.set_pool(pool.clone());
    // Session bleibt gesperrt (keine Auto-Backups je Buchung); nur der
    // Abschluss entsperrt sie kurz.
    Env {
        pool,
        ctx,
        _dir: dir,
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn expense_input(date: NaiveDate, gross: i64, description: &str) -> ExpenseInput {
    ExpenseInput {
        expense_date: date,
        paid_date: Some(date),
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "Bürobedarf Beispiel".into(),
        vendor_invoice_number: None,
        category: "office".into(),
        description: description.into(),
        net_amount_cents: gross,
        tax_amount_cents: 0,
        gross_amount_cents: gross,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    }
}

async fn create_expense(env: &Env, date: NaiveDate, gross: i64, description: &str) -> String {
    let args: expenses_cmd::ExpensesCreateArgs = serde_json::from_value(serde_json::json!({
        "args": { "input": expense_input(date, gross, description) }
    }))
    .unwrap();
    expenses_cmd::expenses_create(&env.ctx, args)
        .await
        .unwrap()
        .expense
        .id
}

async fn audits(pool: &SqlitePool, action: &str) -> Vec<serde_json::Value> {
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = ? ORDER BY id")
            .bind(action)
            .fetch_all(pool)
            .await
            .unwrap();
    rows.iter()
        .map(|r| serde_json::from_str(r).expect("gültiges JSON"))
        .collect()
}

async fn invalid_details(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log
          WHERE details_json IS NOT NULL AND json_valid(details_json) = 0",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

fn args<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap()
}

/// R2-016 / ASVS 8.3.5 und KB-0031.
#[tokio::test]
async fn exports_are_audited_and_flag_snapshot_mismatch() {
    // Abschluss (Argon2id-Backup) und Typst-Renders nicht parallel zur
    // Einrichtung im Server-Test desselben Binaries.
    let _serial = server_support::kdf_serial().await;
    let env = setup().await;
    let now = klein_buch_lib::clock::today_berlin();
    let closed = now.year() - 1;
    create_expense(&env, d(closed, 3, 1), 2_000, "Papier").await;
    env.ctx.session.set(PASS.into());
    lock::close_year(
        &env.pool,
        &env.ctx.paths,
        &env.ctx.session,
        closed as i64,
        klein_buch_lib::clock::berlin_to_utc(now, 12, 0),
    )
    .await
    .unwrap();
    env.ctx.session.clear();
    let y = serde_json::json!({ "fiscalYear": closed });
    let ys = serde_json::json!({ "fiscalYear": closed, "skr": "SKR03" });

    // Abgeschlossenes Jahr ohne Abweichung: je Export genau ein Eintrag.
    let elster = euer_cmd::euer_export_elster(&env.ctx, args(y.clone()))
        .await
        .unwrap();
    assert!(elster.snapshot_check.as_ref().unwrap().matches);
    let datev = euer_cmd::euer_export_datev(&env.ctx, args(ys.clone()))
        .await
        .unwrap();
    assert!(datev.snapshot_check.unwrap().matches);
    euer_cmd::euer_export_detail_zip(&env.ctx, args(y.clone()))
        .await
        .unwrap();
    euer_cmd::euer_export_pdf(&env.ctx, args(y.clone()))
        .await
        .unwrap();
    euer_cmd::euer_export_stb_zip(&env.ctx, args(ys.clone()))
        .await
        .unwrap();
    for (action, file) in [
        ("euer.elster_export", format!("EUER-{closed}-ELSTER.csv")),
        ("euer.datev_export", format!("DATEV-{closed}-SKR03.csv")),
        (
            "euer.detail_export",
            format!("EUER-{closed}-Einzelaufstellung.zip"),
        ),
        ("euer.pdf_export", format!("EUER-{closed}.pdf")),
        ("euer.stb_export", format!("Steuerberater-{closed}.zip")),
    ] {
        let rows = audits(&env.pool, action).await;
        assert_eq!(rows.len(), 1, "{action}: genau ein Eintrag");
        assert_eq!(rows[0]["fiscal_year"], closed, "{action}");
        assert_eq!(rows[0]["file_name"], file.as_str(), "{action}");
        assert_eq!(rows[0]["snapshot"], "match", "{action}");
        assert!(
            !rows[0].to_string().contains("Papier"),
            "{action}: keine Inhalte im Audit"
        );
    }
    assert!(audits(&env.pool, "euer.snapshot_mismatch").await.is_empty());

    // Ein offenes Jahr trägt den Status `open`.
    euer_cmd::euer_export_elster(
        &env.ctx,
        args(serde_json::json!({ "fiscalYear": now.year() })),
    )
    .await
    .unwrap();
    let rows = audits(&env.pool, "euer.elster_export").await;
    assert_eq!(rows[1]["snapshot"], "open");

    // Eingriff am Bestand des geschlossenen Jahres (am Guard vorbei).
    {
        let mut tx = klein_buch_lib::db::begin_immediate(&env.pool)
            .await
            .unwrap();
        let number = numbering::next_number_in(&mut tx, DocType::Expense, closed)
            .await
            .unwrap();
        expenses_repo::insert_in(
            &mut tx,
            &expense_input(d(closed, 7, 1), 500, "Nachträglich"),
            &number,
            closed as i64,
            None,
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let package = euer_cmd::euer_package(&env.ctx, args(y.clone()))
        .await
        .unwrap();
    let check = package.snapshot_check.expect("abgeschlossenes Jahr");
    assert!(!check.matches);
    assert_eq!(check.differences, vec!["expenses", "surplus"]);
    assert_eq!(check.snapshot.expense_cents, 2_000);
    assert_eq!(check.live.expense_cents, 2_500);
    // Die Anzeige allein schreibt nichts.
    assert!(audits(&env.pool, "euer.snapshot_mismatch").await.is_empty());

    // Lauf F2c: ein verworfener Rechnungs-Entwurf des Jahres (Nummernlücke).
    sqlx::query(
        "INSERT INTO contacts (id, contact_type, name, email, country_code)
         VALUES ('c-1', 'customer', 'Kunde Beispiel', 'kunde@example.org', 'DE')",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invoices (id, invoice_number, fiscal_year, direction, invoice_date,
            contact_id, seller_name, seller_street, seller_postal_code, seller_city,
            net_amount_cents, gross_amount_cents)
         VALUES ('i-void', ?, ?, 'issued', ?, 'c-1', 'V', 'W', '12345', 'O', 4200, 4200)",
    )
    .bind(format!("RE-{closed}-0007"))
    .bind(closed)
    .bind(format!("{closed}-05-01"))
    .execute(&env.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET voided_at = ?, void_reason = 'Doppelt angelegt'
          WHERE id = 'i-void'",
    )
    .bind(format!("{closed}-05-02 10:00:00"))
    .execute(&env.pool)
    .await
    .unwrap();

    let stb = euer_cmd::euer_export_stb_zip(&env.ctx, args(ys.clone()))
        .await
        .unwrap();
    assert!(!stb.snapshot_check.as_ref().unwrap().matches);
    let entry = env
        .ctx
        .downloads
        .take(&stb.download.download_id)
        .expect("Download");
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&entry.path).unwrap()).unwrap();
    let mut readme = String::new();
    zip.by_name(&format!("EUER-{closed}/LIESMICH.txt"))
        .unwrap()
        .read_to_string(&mut readme)
        .unwrap();
    assert!(
        readme.starts_with("ACHTUNG — Abweichung vom Festschreibungs-Snapshot"),
        "{readme}"
    );
    // F2c: verworfene Nummer erklärt (LIESMICH + CSV, im Manifest)
    assert!(
        readme.contains("1 Nummer(n) dieses Jahres haben keinen festgeschriebenen Beleg")
            && readme.contains("nicht-festgeschriebene-nummern.csv"),
        "{readme}"
    );
    let mut unissued = String::new();
    zip.by_name(&format!("EUER-{closed}/nicht-festgeschriebene-nummern.csv"))
        .unwrap()
        .read_to_string(&mut unissued)
        .unwrap();
    assert!(
        unissued.contains(&format!(
            "RE-{closed}-0007;Rechnung;{closed}-05-01;verworfen (nie ausgestellt);"
        )) && unissued.contains("Doppelt angelegt;42,00"),
        "{unissued}"
    );
    let mut manifest = String::new();
    zip.by_name(&format!("EUER-{closed}/manifest.json"))
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    assert!(manifest.contains("nicht-festgeschriebene-nummern.csv"));
    // F2c: auch die ELSTER-Hilfe im Paket trägt den Hinweis
    let mut elster_in_zip = String::new();
    zip.by_name(&format!("EUER-{closed}/elster-ausfuellhilfe.csv"))
        .unwrap()
        .read_to_string(&mut elster_in_zip)
        .unwrap();
    assert!(
        elster_in_zip.contains("# ACHTUNG: Abweichung vom Jahresabschluss"),
        "{elster_in_zip}"
    );
    drop(zip);

    // F2c: Einzel-Exporte tragen den Vermerk in der Datei
    let elster = euer_cmd::euer_export_elster(&env.ctx, args(y.clone()))
        .await
        .unwrap();
    let entry = env
        .ctx
        .downloads
        .take(&elster.download.download_id)
        .expect("Download");
    let csv = std::fs::read_to_string(&entry.path).unwrap();
    assert!(
        csv.lines()
            .nth(1)
            .is_some_and(|l| l.starts_with("# ACHTUNG: Abweichung vom Jahresabschluss")),
        "{csv}"
    );
    let detail = euer_cmd::euer_export_detail_zip(&env.ctx, args(y.clone()))
        .await
        .unwrap();
    let entry = env
        .ctx
        .downloads
        .take(&detail.download.download_id)
        .expect("Download");
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&entry.path).unwrap()).unwrap();
    let mut note = String::new();
    zip.by_name("HINWEIS-ABWEICHUNG-JAHRESABSCHLUSS.txt")
        .unwrap()
        .read_to_string(&mut note)
        .unwrap();
    assert!(note.starts_with("ACHTUNG — Abweichung"), "{note}");
    // Das PDF rendert mit dem Hinweiskasten (Inhalt prüft der Unit-Test
    // `fiscal_year::snapshot::tests::pdf_block_*`).
    euer_cmd::euer_export_pdf(&env.ctx, args(y.clone()))
        .await
        .unwrap();

    let mismatches = audits(&env.pool, "euer.snapshot_mismatch").await;
    assert_eq!(mismatches.len(), 4, "je Export ein Eintrag");
    assert_eq!(mismatches[0]["export"], "euer.stb_export");
    assert_eq!(mismatches[0]["snapshot"]["expenseCents"], 2_000);
    assert_eq!(mismatches[0]["live"]["expenseCents"], 2_500);
    let stb_rows = audits(&env.pool, "euer.stb_export").await;
    assert_eq!(stb_rows.last().unwrap()["snapshot"], "mismatch");
    let notices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE dedup_key = ?")
        .bind(format!("euer_snapshot_mismatch:{closed}"))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(notices, 1, "ein Hinweis je Jahr");

    // Übersicht der Geschäftsjahre meldet die Abweichung ebenfalls.
    let overview = klein_buch_lib::commands::fiscal_year::fiscal_year_overview(
        &env.ctx,
        args(serde_json::json!({})),
    )
    .await
    .unwrap();
    let year = overview
        .years
        .iter()
        .find(|s| s.fiscal_year == closed as i64)
        .unwrap();
    assert_eq!(year.snapshot_matches, Some(false));
    assert_eq!(invalid_details(&env.pool).await, 0);
}

/// W14-010: Nutzereingaben mit Anführungs- und Steuerzeichen ergeben gültige
/// Audit-Details mit unverändertem Inhalt; direkte Aufrufe ohne Kontext
/// schreiben den Akteur `system`.
#[tokio::test]
async fn audit_details_survive_quotes_and_control_characters() {
    let env = setup().await;
    let now = klein_buch_lib::clock::today_berlin();
    let id = create_expense(&env, now, 1_000, "Papier").await;
    let reason = "Doppelt \"erfasst\"\n\tsiehe Beleg \\ 4711 \u{7}";
    let cancel: expenses_cmd::ExpensesCancelArgs =
        args(serde_json::json!({ "args": { "expenseId": id, "reason": reason } }));
    expenses_cmd::expenses_cancel(&env.ctx, cancel)
        .await
        .unwrap();
    let rows = audits(&env.pool, "expense.cancel").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["reason"], reason.trim());

    let name = "Paket \"Premium\"\twith \\backslash";
    let pkg: klein_buch_lib::commands::packages::PackagesCreateArgs = args(serde_json::json!({
        "name": name,
        "revision": {
            "title": "Premium",
            "bodyMarkup": "Alles drin",
            "defaultUnitPriceCents": 10_000,
            "unitCode": "C62",
            "taxCategoryCode": "E",
            "note": null
        }
    }));
    klein_buch_lib::commands::packages::packages_create(&env.ctx, pkg)
        .await
        .unwrap();
    let rows = audits(&env.pool, "package.create").await;
    assert_eq!(rows[0]["name"], name);

    assert_eq!(invalid_details(&env.pool).await, 0);
    let actors: Vec<String> = sqlx::query_scalar("SELECT DISTINCT actor FROM audit_log")
        .fetch_all(&env.pool)
        .await
        .unwrap();
    assert_eq!(actors, vec!["system".to_string()]);
}

/// R1-S3-ACTOR: über die Sitzung (Dispatcher) schreibt der Inhaber; die
/// Einrichtung ebenso.
#[tokio::test]
async fn commands_over_the_session_write_actor_owner() {
    let srv = test_server();
    let cookie = server_support::setup(&srv).await;
    let reply = call(
        &srv.app,
        &cookie,
        "package_category_create",
        serde_json::json!({ "name": "Kategorie \"A\"" }),
    )
    .await;
    assert_eq!(reply.status, axum::http::StatusCode::OK, "{}", reply.text());
    let pool = srv.state.ctx.pool().unwrap();
    let actor: String =
        sqlx::query_scalar("SELECT actor FROM audit_log WHERE action = 'package_category.create'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(actor, "owner");
    let setup_actor: String =
        sqlx::query_scalar("SELECT actor FROM audit_log WHERE action = 'auth.setup.ok'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(setup_actor, "owner");
    assert_eq!(invalid_details(&pool).await, 0);
}
