//! Lauf F — Start- und Zeitlogik der Abo-Läufe gegen eine echte Tempfile-DB
//! (alle Migrationen, GoBD-Trigger) mit injizierter Test-Uhr (D-20).
//!
//! Abgedeckt: Sperre/CAS/Transaktion der Kosten-Abos (W14-022, ASVS V11.1.6),
//! Atomarität und Fortsetzen nach Abbruch (W14-025), Nachhol-Grenze und
//! Bestätigung (W14-026), abgeschlossenes Geschäftsjahr (W14-034), Pause bei
//! Uhrsprung (ASVS V11.1.5), Modus „Erinnerung“ (D-01), Job-Isolation im Tick
//! (KB-0045), Auto-AfA atomar mit Backoff (D-03), Standard-Zahlungskonten
//! (D-14), Anonymisierung pausiert Abos (W14-031), DST/Monatswechsel; ein Backup
//! je Tick und erster Tick ohne Vollscan (D-19), Abbruch-Anforderung (W14-025b);
//! keine Empfängeradresse im WARN-Log beim gescheiterten Auto-Versand (W14-016).

use chrono::{NaiveDate, TimeZone, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use klein_buch_lib::backup::BackupSession;
use klein_buch_lib::clock::{Clock, FixedClock};
use klein_buch_lib::commands::invoices as inv_cmd;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{
    app_settings, contacts, payment_accounts, recurring, recurring_invoice, seller_profile,
};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};
use klein_buch_lib::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
use klein_buch_lib::domain::numbering::DocType;
use klein_buch_lib::domain::recurring::RecurringInput;
use klein_buch_lib::domain::recurring_invoice::RecurringInvoiceInput;
use klein_buch_lib::scheduler::{self, hold, JobCtx};

const TEMPLATE_TEST: &str = r#"// §19-KLAUSEL-BLOCK: REQUIRED
#set page(paper: "a4")
#let data = json(bytes(sys.inputs.at("data-json")))
= Rechnung #data.invoice.number
Empfänger: #data.buyer.name \
#data.invoice.gross_amount
#data.kleinunternehmer.hinweis_text
"#;

struct Env {
    pool: SqlitePool,
    paths: Paths,
    session: BackupSession,
    _tmp: tempfile::TempDir,
}

async fn setup() -> Env {
    static MOCK_ENV: std::sync::Once = std::sync::Once::new();
    MOCK_ENV.call_once(|| std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1"));
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let paths = Paths {
        data_dir: data.clone(),
        db_file: data.join("klein-buch.sqlite"),
        archive_dir: data.join("archive"),
        backups_dir: data.join("backups"),
        inputs_dir: tmp.path().join("inputs"),
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
        resource_dir: PathBuf::from("/non/existent/resources"),
        web_dir: PathBuf::from("/non/existent/web"),
        backup_targets_dir: PathBuf::from("/backup-targets"),
    };
    std::fs::create_dir_all(&paths.archive_dir).unwrap();
    std::fs::create_dir_all(&paths.backups_dir).unwrap();
    std::fs::create_dir_all(paths.inputs_dir.join("pdf-templates")).unwrap();
    std::fs::write(
        paths.inputs_dir.join("pdf-templates").join("default.typ"),
        TEMPLATE_TEST,
    )
    .unwrap();
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
    let session = BackupSession::default();
    session.set("passphrase-1234567890".into());
    Env {
        pool,
        paths,
        session,
        _tmp: tmp,
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn noon(day: NaiveDate) -> FixedClock {
    FixedClock::at_berlin(day, 12, 0)
}

fn job<'a>(env: &'a Env, clock: &'a dyn Clock) -> JobCtx<'a> {
    JobCtx::new(&env.pool, &env.paths, &env.session, clock)
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn notice_bodies(pool: &SqlitePool) -> String {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT title, body FROM notifications ORDER BY triggered_at")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.into_iter()
        .map(|(t, b)| format!("{t}: {b}"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn notices(pool: &SqlitePool, prefix: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE ?")
        .bind(format!("{prefix}%"))
        .fetch_one(pool)
        .await
        .unwrap()
}

fn sub_input(label: &str, day: i64, next_due: NaiveDate, auto: bool) -> RecurringInput {
    RecurringInput {
        label: label.into(),
        vendor_contact_id: None,
        frequency: "monthly".into(),
        day_of_period: day,
        next_due_date: next_due,
        expected_amount_cents: 1_190,
        category: "software".into(),
        description_template: format!("{label} — Abo"),
        auto_create_expense: auto,
        reverse_charge_13b_default: false,
    }
}

async fn close_year(pool: &SqlitePool, year: i64) {
    sqlx::query(
        "INSERT INTO fiscal_year_locks (fiscal_year, income_total_cents, expense_total_cents,
            afa_total_cents, surplus_cents, app_version, schema_version)
         VALUES (?, 0, 0, 0, 0, 'test', 31)",
    )
    .bind(year)
    .execute(pool)
    .await
    .unwrap();
}

async fn mk_seller(pool: &SqlitePool) {
    seller_profile::upsert(
        pool,
        &seller_profile::SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "rechnung@example.org".into(),
            phone: Some("+49 000 000000".into()),
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: None,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
        },
    )
    .await
    .unwrap();
}

async fn mk_contact(pool: &SqlitePool) -> String {
    contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Both,
            name: "Kunde GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: None,
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: Some("kunde@example.org".into()),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: None,
        },
    )
    .await
    .unwrap()
    .id
}

fn item() -> InvoiceItemInput {
    InvoiceItemInput {
        position: 1,
        description: "Server-Wartung (Pauschale)".into(),
        quantity: 1.0,
        unit_code: "C62".into(),
        unit_price_cents: 9_900,
        tax_rate_percent: 0.0,
        tax_category_code: "E".into(),
        description_title: None,
        description_markup: None,
        source_package_id: None,
        source_package_revision: None,
    }
}

fn tmpl_input(contact: &str, next_due: NaiveDate, mode: &str) -> RecurringInvoiceInput {
    RecurringInvoiceInput {
        label: "Wartung".into(),
        contact_id: contact.into(),
        frequency: "monthly".into(),
        day_of_period: 1,
        next_due_date: next_due,
        start_date: None,
        end_date: None,
        auto_mode: mode.into(),
        payment_terms_days: 14,
        pdf_template: "default".into(),
        service_period_note: true,
        notes: None,
        items: vec![item()],
    }
}

// ---- Kosten-Abos: Sperre, CAS, Transaktion (W14-022) ------------------------

/// Tick und „Buchen“ gleichzeitig → genau eine Kosten-Position, Stichtag genau
/// einmal vorgerückt.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_tick_and_run_now_book_exactly_once() {
    let env = setup().await;
    let sub = recurring::create(&env.pool, &sub_input("Hosting", 1, d(2026, 5, 1), true))
        .await
        .unwrap();
    let clock = noon(d(2026, 5, 20));
    let job = job(&env, &clock);
    let (tick, now) = tokio::join!(
        scheduler::recurring::process_due(&job),
        scheduler::recurring::run_now(&job, &sub.id)
    );
    let created = tick.unwrap().created_expenses + usize::from(now.is_ok());
    assert_eq!(created, 1, "genau eine Buchung (run_now={now:?})");
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 1);
    let after = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-06-01");
}

/// Ein fehlerhaftes Abo bricht den Lauf nicht ab; das andere wird gebucht, das
/// Backup entsteht, das fehlerhafte bekommt einen Hinweis.
#[tokio::test]
async fn broken_subscription_does_not_stop_others() {
    let env = setup().await;
    let good = recurring::create(&env.pool, &sub_input("Gut", 1, d(2026, 5, 1), true))
        .await
        .unwrap();
    let bad = recurring::create(&env.pool, &sub_input("Kaputt", 1, d(2026, 4, 1), true))
        .await
        .unwrap();
    // ungültiges Datum, sortiert aber vor „heute“ → gelistet, Parsen scheitert
    sqlx::query("UPDATE recurring_subscriptions SET next_due_date = '2026-02-30' WHERE id = ?")
        .bind(&bad.id)
        .execute(&env.pool)
        .await
        .unwrap();
    let clock = noon(d(2026, 5, 20));
    let report = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(report.created_expenses, 1);
    assert_eq!(report.failed_subscriptions, 1);
    let after = recurring::get(&env.pool, &good.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-06-01");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_critical'"
        )
        .await,
        1,
        "Backup für die gebuchte Position trotz Teilfehler"
    );
    assert_eq!(notices(&env.pool, "recurring_failed:").await, 1);
}

/// W14-025: Abbruch zwischen Erzeugen und Vorrücken (hier: Vorrücken scheitert
/// per Trigger) → weder Beleg noch verbrauchte Nummer noch Audit.
#[tokio::test]
async fn failure_while_advancing_rolls_back_expense_and_number() {
    let env = setup().await;
    let sub = recurring::create(&env.pool, &sub_input("Hosting", 1, d(2026, 5, 1), true))
        .await
        .unwrap();
    sqlx::query(
        "CREATE TRIGGER test_block_advance BEFORE UPDATE OF next_due_date ON recurring_subscriptions
         BEGIN SELECT RAISE(ABORT, 'Test: Abbruch beim Vorrücken'); END",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let clock = noon(d(2026, 5, 20));
    let err = scheduler::recurring::run_now(&job(&env, &clock), &sub.id)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Abbruch"), "{err}");
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 0);
    assert_eq!(
        klein_buch_lib::db::numbering::peek_seq(&env.pool, DocType::Expense, 2026)
            .await
            .unwrap(),
        0,
        "keine Nummer verbraucht"
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action LIKE 'recurring.%' OR action = 'expense.create'"
        )
        .await,
        0
    );
    sqlx::query("DROP TRIGGER test_block_advance")
        .execute(&env.pool)
        .await
        .unwrap();
    // Der nächste Lauf bucht den Termin genau einmal.
    let report = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(report.created_expenses, 1);
    let number: String = sqlx::query_scalar("SELECT expense_number FROM expenses")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(number, "KO-2026-0001", "lückenlos");
}

// ---- Nachholen (W14-026) und abgeschlossenes Jahr (W14-034) -----------------

#[tokio::test]
async fn subscription_backlog_waits_for_decision_and_skips_closed_year_on_confirmation() {
    let env = setup().await;
    close_year(&env.pool, 2025).await;
    let sub = recurring::create(&env.pool, &sub_input("Miete", 1, d(2025, 10, 1), true))
        .await
        .unwrap();
    let clock = noon(d(2026, 5, 20));
    // 2025-10 … 2026-05 = 8 Termine > Grenze 3 → nichts, Hinweis
    let report = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(report.created_expenses, 0);
    assert_eq!(report.awaiting_decision, 1);
    assert_eq!(notices(&env.pool, "recurring_backlog:").await, 1);

    let row = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    scheduler::recurring::apply_resume_mode(
        &env.pool,
        &row,
        scheduler::catch_up::ResumeMode::CatchUp,
        d(2026, 5, 20),
    )
    .await
    .unwrap();
    let report = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(report.created_expenses, 5, "2026-01 … 2026-05: {report:?}");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'recurring.period_skipped'"
        )
        .await,
        3,
        "2025-10 … 2025-12 übersprungen (GJ abgeschlossen)"
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM expenses WHERE fiscal_year = 2025"
        )
        .await,
        0
    );
    let after = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-06-01");
}

/// Ohne Bestätigung: Termin im abgeschlossenen Jahr → ein Hinweis, kein Fehler
/// bei jedem Tick, nichts gebucht.
#[tokio::test]
async fn closed_year_occurrence_notices_once() {
    let env = setup().await;
    close_year(&env.pool, 2025).await;
    recurring::create(&env.pool, &sub_input("Miete", 1, d(2025, 12, 1), true))
        .await
        .unwrap();
    let clock = noon(d(2026, 1, 10));
    for _ in 0..2 {
        let report = scheduler::recurring::process_due(&job(&env, &clock))
            .await
            .unwrap();
        assert_eq!(report.created_expenses, 0);
        assert_eq!(report.failed_subscriptions, 0);
    }
    assert_eq!(notices(&env.pool, "recurring_closed_year:").await, 1);
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 0);
}

/// „Ab heute fortsetzen“ beim Fortsetzen eines pausierten Abos.
#[tokio::test]
async fn skip_resume_moves_subscription_forward() {
    let env = setup().await;
    let sub = recurring::create(&env.pool, &sub_input("Miete", 1, d(2026, 1, 1), true))
        .await
        .unwrap();
    scheduler::recurring::apply_resume_mode(
        &env.pool,
        &sub,
        scheduler::catch_up::ResumeMode::Skip,
        d(2026, 5, 20),
    )
    .await
    .unwrap();
    let after = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-06-01");
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 0);
}

// ---- Test-Uhr: Zeitumstellung, Monatswechsel -------------------------------

#[tokio::test]
async fn dst_switch_days_book_on_berlin_midnight() {
    let env = setup().await;
    let march = recurring::create(&env.pool, &sub_input("März", 29, d(2026, 3, 29), true))
        .await
        .unwrap();
    // 28.03. 23:59 MEZ = 22:59 UTC → noch nicht fällig
    let clock = FixedClock::at(Utc.with_ymd_and_hms(2026, 3, 28, 22, 59, 0).unwrap());
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 0);
    // 29.03. 00:01 MEZ = 23:01 UTC am Vortag → fällig
    clock.advance(chrono::Duration::minutes(2));
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 1);
    let date: String = sqlx::query_scalar("SELECT expense_date FROM expenses")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(date, "2026-03-29");
    let after = recurring::get(&env.pool, &march.id).await.unwrap().unwrap();
    assert_eq!(after.next_due_date, "2026-04-29");

    // Ende der Sommerzeit: 25.10. 00:00 MESZ = 24.10. 22:00 UTC
    recurring::create(&env.pool, &sub_input("Oktober", 25, d(2026, 10, 25), true))
        .await
        .unwrap();
    clock.set(Utc.with_ymd_and_hms(2026, 10, 24, 21, 59, 0).unwrap());
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 0, "23:59 MESZ am 24.10.");
    clock.set(Utc.with_ymd_and_hms(2026, 10, 24, 22, 1, 0).unwrap());
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    // „März“ hat seit 29.04. sechs offene Termine (> Grenze) → nur Hinweis;
    // „Oktober“ wird zum Berliner Tageswechsel gebucht.
    assert_eq!(r.created_expenses, 1, "{r:?}");
    assert_eq!(r.awaiting_decision, 1, "{r:?}");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM expenses WHERE expense_date = '2026-10-25'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn month_end_day_31_clamps_and_recovers() {
    let env = setup().await;
    let sub = recurring::create(&env.pool, &sub_input("Ultimo", 31, d(2026, 1, 31), true))
        .await
        .unwrap();
    for (day, expected_next) in [
        (d(2026, 1, 31), "2026-02-28"),
        (d(2026, 2, 28), "2026-03-31"),
        (d(2026, 3, 31), "2026-04-30"),
    ] {
        let clock = noon(day);
        let r = scheduler::recurring::process_due(&job(&env, &clock))
            .await
            .unwrap();
        assert_eq!(r.created_expenses, 1, "{day}");
        let after = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
        assert_eq!(after.next_due_date, expected_next, "{day}");
    }
}

// ---- Pause bei unplausibler Uhr (ASVS V11.1.5) -----------------------------

#[tokio::test]
async fn clock_backwards_holds_until_release() {
    let env = setup().await;
    recurring::create(&env.pool, &sub_input("Hosting", 1, d(2026, 5, 1), true))
        .await
        .unwrap();
    let clock = noon(d(2026, 4, 20));
    scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    // Uhr springt zwei Tage zurück
    clock.set_berlin(d(2026, 4, 18), 12, 0);
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(
        r.skipped_hold.as_deref(),
        Some(hold::REASON_CLOCK_BACKWARDS)
    );
    // auch nach Korrektur der Uhr bleibt die Pause bis zur Freigabe
    clock.set_berlin(d(2026, 5, 20), 12, 0);
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert!(r.skipped_hold.is_some());
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 0);
    let released = hold::release(&env.pool, clock.now_utc()).await.unwrap();
    assert_eq!(
        released.map(|h| h.reason).as_deref(),
        Some(hold::REASON_CLOCK_BACKWARDS)
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'scheduler.hold_released'"
        )
        .await,
        1
    );
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 1);
}

// ---- Modus „Erinnerung“ (D-01) ---------------------------------------------

#[tokio::test]
async fn reminder_mode_creates_one_notice_per_due_date() {
    let env = setup().await;
    let sub = recurring::create(
        &env.pool,
        &sub_input("Versicherung", 1, d(2026, 5, 1), false),
    )
    .await
    .unwrap();
    let n = scheduler::reminders::run(&env.pool, d(2026, 4, 30), Path::new("/backup-targets"))
        .await
        .unwrap();
    assert_eq!(
        notices(&env.pool, "recurring_due:").await,
        0,
        "noch nicht fällig ({n})"
    );
    for _ in 0..2 {
        scheduler::reminders::run(&env.pool, d(2026, 5, 20), Path::new("/backup-targets"))
            .await
            .unwrap();
    }
    assert_eq!(notices(&env.pool, "recurring_due:").await, 1);
    // Auto-Lauf bucht im Modus Erinnerung nichts
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 0);
    // Buchen rückt vor; der nächste Hinweis kommt erst zum nächsten Stichtag
    scheduler::recurring::run_now(&job(&env, &clock), &sub.id)
        .await
        .unwrap();
    scheduler::reminders::run(&env.pool, d(2026, 5, 21), Path::new("/backup-targets"))
        .await
        .unwrap();
    assert_eq!(notices(&env.pool, "recurring_due:").await, 1);
}

/// Lauf F2a: Migration 0032 seedet `rule_recurring_due`; der Hinweis trägt die
/// Regel, und eine abgeschaltete Regel erzeugt keinen Hinweis.
#[tokio::test]
async fn reminder_mode_follows_the_seeded_rule() {
    let env = setup().await;
    recurring::create(&env.pool, &sub_input("Domain", 1, d(2026, 5, 1), false))
        .await
        .unwrap();
    klein_buch_lib::notify::rules::set_enabled(&env.pool, "rule_recurring_due", false)
        .await
        .unwrap();
    scheduler::reminders::run(&env.pool, d(2026, 5, 20), Path::new("/backup-targets"))
        .await
        .unwrap();
    assert_eq!(notices(&env.pool, "recurring_due:").await, 0, "Regel aus");
    klein_buch_lib::notify::rules::set_enabled(&env.pool, "rule_recurring_due", true)
        .await
        .unwrap();
    scheduler::reminders::run(&env.pool, d(2026, 5, 20), Path::new("/backup-targets"))
        .await
        .unwrap();
    let rule: Option<String> = sqlx::query_scalar(
        "SELECT rule_id FROM notifications WHERE dedup_key LIKE 'recurring_due:%'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(rule.as_deref(), Some("rule_recurring_due"));
}

// ---- Tick: Job-Isolation (KB-0045) -----------------------------------------

#[tokio::test]
async fn failing_job_does_not_stop_following_jobs() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 5, 1), "draft"))
        .await
        .unwrap();
    // Job 1 (Kosten-Abos) scheitert als Ganzes: Tabelle weg.
    sqlx::query("ALTER TABLE recurring_subscriptions RENAME TO recurring_subscriptions_kaputt")
        .execute(&env.pool)
        .await
        .unwrap();
    assert!(
        scheduler::recurring::process_due(&job(&env, &noon(d(2026, 5, 20))))
            .await
            .is_err()
    );
    let clock = noon(d(2026, 5, 20));
    scheduler::tick::run_jobs(&job(&env, &clock)).await.unwrap();
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM invoices").await,
        1,
        "Job 5 (Abo-Rechnungen) lief trotz Fehler in Job 1"
    );
    assert!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'"
        )
        .await
            == 1,
        "Job 0 (Tages-Backup) lief"
    );
}

// ---- Tick: ein Backup je Tick (D-19) und Tages-Backup (D-04) ---------------

/// D-19: Ein fälliges Kosten-Abo und eine fällige Abo-Rechnung (Festschreiben)
/// im selben Tick ergeben genau **ein** Backup. Vor der Tages-Backup-Uhrzeit
/// ist es ein Lock-Backup, danach deckt das Tages-Backup die Lock-Ereignisse
/// mit ab (Backup-Hardline erfüllt, keine drei Voll-Backups).
#[tokio::test]
async fn one_backup_per_tick_covers_all_lock_events() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    recurring::create(&env.pool, &sub_input("Hosting", 15, d(2026, 5, 15), true))
        .await
        .unwrap();
    recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 5, 1), "issue"))
        .await
        .unwrap();
    // 01:00 — vor dem Tages-Backup (Standard 02:30); das des Vortags lief.
    klein_buch_lib::backup::mark_daily_done(&env.pool, d(2026, 5, 19))
        .await
        .unwrap();
    let clock = FixedClock::at_berlin(d(2026, 5, 20), 1, 0);
    scheduler::tick::run_jobs(&job(&env, &clock)).await.unwrap();
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 1);
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM invoices WHERE locked_at IS NOT NULL"
        )
        .await,
        1,
        "{}",
        notice_bodies(&env.pool).await
    );
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM backup_history").await,
        1,
        "genau ein Backup für beide Läufe"
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_critical'"
        )
        .await,
        1
    );
    // Nächster Termin beider Reihen, mittags: das Tages-Backup deckt ab.
    let clock = noon(d(2026, 6, 16));
    scheduler::tick::run_jobs(&job(&env, &clock)).await.unwrap();
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 2);
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM backup_history").await,
        2,
        "wieder genau eines"
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM backup_history WHERE trigger_reason = 'auto_daily'"
        )
        .await,
        1
    );
}

/// D-19: Der erste Tick nach dem Entsperren lässt den Integritäts-Vollscan
/// aus; der nächste holt ihn nach.
#[tokio::test]
async fn first_tick_after_unlock_skips_integrity_scan() {
    let env = setup().await;
    let clock = noon(d(2026, 5, 20));
    scheduler::tick::run_jobs(&job(&env, &clock).with_first_after_unlock(true))
        .await
        .unwrap();
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM archive_integrity_checks").await,
        0
    );
    scheduler::tick::run_jobs(&job(&env, &clock)).await.unwrap();
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM archive_integrity_checks WHERE finished_at IS NOT NULL"
        )
        .await,
        1
    );
}

// ---- Abo-Rechnungen mit Festschreiben (Mock-Sidecar) -----------------------

/// Log-Puffer für [`auto_send_failure_keeps_recipient_out_of_warn_log`].
#[derive(Clone, Default)]
struct LogBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = LogBuffer;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// SMTP-Gegenstelle auf 127.0.0.1, die jeden Empfänger mit seiner Adresse in
/// der Antwort ablehnt (wie viele echte Mailserver).
async fn rejecting_smtp_server() -> u16 {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                let _ = write.write_all(b"220 smtp.example.org ESMTP\r\n").await;
                while let Ok(Some(line)) = lines.next_line().await {
                    let upper = line.to_ascii_uppercase();
                    let reply = if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                        "250 smtp.example.org\r\n".to_string()
                    } else if upper.starts_with("RCPT TO:") {
                        let addr = line[8..].trim().to_string();
                        format!("550 5.1.1 {addr}: Recipient address rejected\r\n")
                    } else if upper.starts_with("QUIT") {
                        let _ = write.write_all(b"221 Bye\r\n").await;
                        break;
                    } else {
                        "250 OK\r\n".to_string()
                    };
                    if write.write_all(reply.as_bytes()).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

/// W14-016 (Lauf F4c): Scheitert der Auto-Versand einer Abo-Rechnung, nennt
/// die WARN-Zeile nur Rechnungsnummer und Fehlerart — die Empfängeradresse aus
/// der Antwort des Mailservers steht nur auf DEBUG, im Hinweis und im
/// E-Mail-Protokoll.
#[tokio::test]
async fn auto_send_failure_keeps_recipient_out_of_warn_log() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let port = rejecting_smtp_server().await;
    let mail_templates = env.paths.inputs_dir.join("mail-templates");
    std::fs::create_dir_all(&mail_templates).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../inputs/mail-templates/invoice-de.txt"),
        mail_templates.join("invoice-de.txt"),
    )
    .unwrap();
    klein_buch_lib::db::repo::mail_accounts::create(
        &env.pool,
        &klein_buch_lib::db::repo::mail_accounts::MailAccountInput {
            label: "Test".into(),
            auth_type: "smtp_password".into(),
            smtp_host: Some("127.0.0.1".into()),
            smtp_port: Some(i64::from(port)),
            smtp_user: None,
            smtp_use_tls: false,
            from_email: "rechnung@example.org".into(),
            from_name: "Beispiel IT-Service".into(),
            is_default: true,
        },
    )
    .await
    .unwrap();
    recurring_invoice::create(
        &env.pool,
        &tmpl_input(&contact, d(2026, 5, 1), "issue_send"),
    )
    .await
    .unwrap();
    let buffer = LogBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(buffer.clone())
        .finish();
    let clock = noon(d(2026, 5, 20));
    {
        // current_thread-Runtime: alle Ereignisse des Laufs auf diesem Thread.
        let _guard = tracing::subscriber::set_default(subscriber);
        let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
            .await
            .unwrap();
        assert_eq!(r.created_invoices, 1, "{r:?}");
    }
    let text = String::from_utf8_lossy(&buffer.0.lock().unwrap()).into_owned();
    let warn = text
        .lines()
        .find(|l| l.contains(" WARN ") && l.contains("Auto-Versand fehlgeschlagen"))
        .unwrap_or_else(|| panic!("WARN-Zeile fehlt:\n{text}"));
    assert!(warn.contains("Fehlerart mail"), "{warn}");
    for line in text
        .lines()
        .filter(|l| l.contains(" WARN ") || l.contains(" INFO "))
    {
        assert!(
            !line.contains("kunde@example.org"),
            "Empfänger im Log: {line}"
        );
    }
    assert!(
        text.lines()
            .any(|l| l.contains(" DEBUG ") && l.contains("kunde@example.org")),
        "Volltext auf DEBUG:\n{text}"
    );
    assert!(
        notice_bodies(&env.pool).await.contains("kunde@example.org"),
        "Hinweis nennt den Fehler:\n{}",
        notice_bodies(&env.pool).await
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM email_log WHERE status = 'failed' AND error LIKE '%kunde@example.org%'"
        )
        .await,
        1
    );
}

/// W14-026: drei fällige Termine im Modus „Auto + Versand“ → nur der jüngste
/// wird festgeschrieben (und ginge an den Versand), ältere bleiben Entwurf.
#[tokio::test]
async fn catch_up_locks_only_latest_period() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    recurring_invoice::create(
        &env.pool,
        &tmpl_input(&contact, d(2026, 3, 1), "issue_send"),
    )
    .await
    .unwrap();
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_invoices, 3, "{r:?}");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM invoices WHERE locked_at IS NOT NULL"
        )
        .await,
        1,
        "nur der jüngste Termin festgeschrieben; Hinweise:\n{}",
        notice_bodies(&env.pool).await
    );
    let latest: Option<String> =
        sqlx::query_scalar("SELECT delivery_date FROM invoices WHERE locked_at IS NOT NULL")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(latest.as_deref(), Some("2026-05-01"));
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM invoices WHERE sent_at IS NOT NULL"
        )
        .await,
        0,
        "ohne Mail-Konto kein Versand; nachgeholte Termine nie"
    );
    assert_eq!(notices(&env.pool, "recurring_invoice_catch_up:").await, 1);
    assert!(
        app_settings::list_prefix(&env.pool, "recurring_invoice_pending:")
            .await
            .unwrap()
            .is_empty(),
        "kein offener Fortsetzungs-Marker"
    );
}

/// W14-025: Abbruch nach dem Entwurf (Marker gesetzt, Vorlage vorgerückt) → der
/// nächste Lauf schreibt den vorhandenen Entwurf fest statt einen zweiten
/// anzulegen.
#[tokio::test]
async fn interrupted_issue_run_resumes_without_second_invoice() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let detail =
        recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 5, 1), "issue"))
            .await
            .unwrap();
    let id = detail.template.id.clone();
    let today = d(2026, 5, 20);
    // Zustand nach einem Abbruch zwischen Entwurf und Festschreiben nachbauen.
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: today,
        delivery_date: Some(d(2026, 5, 1)),
        due_date: Some(d(2026, 6, 3)),
        currency_code: "EUR".into(),
        items: vec![item()],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    let draft = inv_cmd::create_invoice_draft_on(&env.pool, &contact, &input, None, today)
        .await
        .unwrap();
    let invoice_id = draft.invoice.id.clone();
    recurring_invoice::advance(&env.pool, &id, "2026-05-01", &invoice_id, "2026-06-01")
        .await
        .unwrap();
    app_settings::set(
        &env.pool,
        &format!("recurring_invoice_pending:{id}"),
        &format!("{invoice_id}|2026-05-01|issue"),
    )
    .await
    .unwrap();

    let clock = noon(today);
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.resumed, 1, "{r:?}");
    assert_eq!(r.created_invoices, 0);
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM invoices").await, 1);
    let locked: Option<String> = sqlx::query_scalar("SELECT locked_at FROM invoices WHERE id = ?")
        .bind(&invoice_id)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(
        locked.is_some(),
        "vorhandener Entwurf festgeschrieben; Hinweise:\n{}",
        notice_bodies(&env.pool).await
    );
    assert!(
        app_settings::get(&env.pool, &format!("recurring_invoice_pending:{id}"))
            .await
            .unwrap()
            .is_none()
    );
    // zweiter Lauf: nichts mehr zu tun
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!((r.resumed, r.created_invoices), (0, 0));
}

// ---- Anonymisierung pausiert (W14-031) --------------------------------------

#[tokio::test]
async fn anonymize_pauses_templates_and_subscriptions() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let tmpl = recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 6, 1), "draft"))
        .await
        .unwrap();
    let mut s = sub_input("Lieferant", 1, d(2026, 6, 1), true);
    s.vendor_contact_id = Some(contact.clone());
    let sub = recurring::create(&env.pool, &s).await.unwrap();
    klein_buch_lib::commands::contacts::anonymize_core(&env.pool, &contact)
        .await
        .unwrap();
    let t = recurring_invoice::get_row(&env.pool, &tmpl.template.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.active, 0);
    let s = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(s.active, 0);
    let details: String =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'contact.anonymize'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert!(
        details.contains("\"pausedRecurringInvoices\":1"),
        "{details}"
    );
    assert!(
        details.contains("\"pausedRecurringSubscriptions\":1"),
        "{details}"
    );
}

// ---- Standard-Zahlungskonten (D-14) -----------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn payment_defaults_are_seeded_once_and_atomically() {
    let env = setup().await;
    let (a, b) = tokio::join!(
        payment_accounts::ensure_defaults(&env.pool),
        payment_accounts::ensure_defaults(&env.pool)
    );
    assert!(a.unwrap() ^ b.unwrap(), "genau ein Aufruf legt an");
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM payment_accounts").await,
        2
    );

    let env = setup().await;
    let (a, b) = tokio::join!(
        payment_accounts::seed_defaults_once(&env.pool),
        payment_accounts::seed_defaults_once(&env.pool)
    );
    assert!(a.unwrap() ^ b.unwrap());
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM payment_accounts").await,
        2
    );
    // Auch nach Deaktivieren aller Konten: nie wieder anlegen.
    sqlx::query("UPDATE payment_accounts SET active = 0, is_default = 0")
        .execute(&env.pool)
        .await
        .unwrap();
    assert!(!payment_accounts::seed_defaults_once(&env.pool)
        .await
        .unwrap());
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM payment_accounts").await,
        2
    );
}

// ---- Auto-AfA atomar, Backoff, Idempotenz (D-03, KB-0045) ------------------

async fn mk_asset(pool: &SqlitePool, number: &str, acquisition_date: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO assets (id, asset_number, label, acquisition_date, acquisition_cost_cents,
            acquisition_fiscal_year, depreciation_method, useful_life_years, business_share_percent,
            book_value_cents)
         VALUES (?, ?, 'Notebook', ?, 300000, 2025, 'linear', 3.0, 100.0, 300000)",
    )
    .bind(&id)
    .bind(number)
    .bind(acquisition_date)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn auto_afa_books_once_atomically() {
    let env = setup().await;
    let asset = mk_asset(&env.pool, "AV-2025-0001", "2025-01-15").await;
    let clock = noon(d(2026, 1, 10));
    scheduler::depreciation_year_close::run(&job(&env, &clock))
        .await
        .unwrap();
    scheduler::depreciation_year_close::run(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM depreciation_entries").await,
        1,
        "Marker: einmal je Jahr"
    );
    let (after, book): (i64, i64) = sqlx::query_as(
        "SELECT e.book_value_after_cents, a.book_value_cents
           FROM depreciation_entries e JOIN assets a ON a.id = e.asset_id WHERE a.id = ?",
    )
    .bind(&asset)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(after, book, "Restbuchwert = Stand nach der Buchung");
}

/// Scheitert das Fortschreiben des Restbuchwerts, wird auch der Eintrag
/// zurückgerollt; der Fehler erzeugt einen Hinweis und einen Backoff.
#[tokio::test]
async fn auto_afa_failure_rolls_back_and_backs_off() {
    let env = setup().await;
    mk_asset(&env.pool, "AV-2025-0001", "2025-01-15").await;
    sqlx::query(
        "CREATE TRIGGER test_block_book_value BEFORE UPDATE OF book_value_cents ON assets
         BEGIN SELECT RAISE(ABORT, 'Test: Restbuchwert gesperrt'); END",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let clock = noon(d(2026, 1, 10));
    assert!(scheduler::depreciation_year_close::run(&job(&env, &clock))
        .await
        .is_err());
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM depreciation_entries").await,
        0
    );
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'depreciation.accrue'"
        )
        .await,
        0
    );
    assert_eq!(notices(&env.pool, "afa_auto_failed:2025").await, 1);
    // Innerhalb des Backoffs kein neuer Versuch (kein Fehler)
    clock.advance(chrono::Duration::hours(1));
    scheduler::depreciation_year_close::run(&job(&env, &clock))
        .await
        .unwrap();
    // Nach dem Backoff erneut — jetzt ohne Sperre erfolgreich
    sqlx::query("DROP TRIGGER test_block_book_value")
        .execute(&env.pool)
        .await
        .unwrap();
    clock.advance(chrono::Duration::hours(6));
    scheduler::depreciation_year_close::run(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM depreciation_entries").await,
        1
    );
    assert!(
        app_settings::get(&env.pool, scheduler::depreciation_year_close::BACKOFF_KEY)
            .await
            .unwrap()
            .is_none()
    );
}

// ---- Commands: erste Fälligkeit, Fortsetzen mit Wahl (R1-S2-NEXTDUE, W14-026) ----

async fn ctx_for(env: &Env, clock: FixedClock) -> std::sync::Arc<klein_buch_lib::app_ctx::AppCtx> {
    let ctx = klein_buch_lib::app_ctx::AppCtx::for_tests(env.paths.clone());
    ctx.set_pool(env.pool.clone());
    ctx.session.set("passphrase-1234567890".into());
    ctx.set_clock(std::sync::Arc::new(clock));
    ctx
}

#[tokio::test]
async fn new_template_must_not_start_in_the_past() {
    use klein_buch_lib::commands::recurring_invoice as cmd;
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let ctx = ctx_for(&env, noon(d(2026, 5, 20))).await;
    let err = cmd::recurring_invoices_create(
        &ctx,
        cmd::RecurringInvoicesCreateArgs {
            input: tmpl_input(&contact, d(2026, 5, 1), "issue_send"),
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Vergangenheit"), "{err}");
    // heute ist erlaubt
    cmd::recurring_invoices_create(
        &ctx,
        cmd::RecurringInvoicesCreateArgs {
            input: tmpl_input(&contact, d(2026, 5, 20), "draft"),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn resuming_overdue_template_requires_a_choice() {
    use klein_buch_lib::commands::recurring_invoice as cmd;
    use klein_buch_lib::scheduler::catch_up::ResumeMode;
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let tmpl = recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 1, 1), "issue"))
        .await
        .unwrap();
    let id = tmpl.template.id.clone();
    recurring_invoice::set_active(&env.pool, &id, false)
        .await
        .unwrap();
    let ctx = ctx_for(&env, noon(d(2026, 5, 20))).await;
    let args = |mode: Option<ResumeMode>| cmd::RecurringInvoicesSetActiveArgs {
        id: id.clone(),
        active: true,
        resume_mode: mode,
    };
    let err = cmd::recurring_invoices_set_active(&ctx, args(None))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("verpasste Termine"), "{err}");
    let row = cmd::recurring_invoices_set_active(&ctx, args(Some(ResumeMode::Skip)))
        .await
        .unwrap();
    assert_eq!(row.active, 1);
    assert_eq!(row.next_due_date, "2026-06-01");
    // Entscheidung bei einer pausierten Vorlage nur über „Fortsetzen“
    recurring_invoice::set_active(&env.pool, &id, false)
        .await
        .unwrap();
    assert!(cmd::recurring_invoices_resolve_backlog(
        &ctx,
        cmd::RecurringInvoicesResolveBacklogArgs {
            id: id.clone(),
            mode: ResumeMode::CatchUp,
        },
    )
    .await
    .is_err());
}

#[tokio::test]
async fn resuming_overdue_auto_subscription_requires_a_choice_reminder_does_not() {
    use klein_buch_lib::commands::recurring as cmd;
    use klein_buch_lib::scheduler::catch_up::ResumeMode;
    let env = setup().await;
    let auto = recurring::create(&env.pool, &sub_input("Auto", 1, d(2026, 1, 1), true))
        .await
        .unwrap();
    let reminder = recurring::create(&env.pool, &sub_input("Erinnerung", 1, d(2026, 1, 1), false))
        .await
        .unwrap();
    for id in [&auto.id, &reminder.id] {
        recurring::set_active(&env.pool, id, false).await.unwrap();
    }
    let ctx = ctx_for(&env, noon(d(2026, 5, 20))).await;
    let err = cmd::recurring_set_active(
        &ctx,
        cmd::RecurringSetActiveArgs {
            id: auto.id.clone(),
            active: true,
            resume_mode: None,
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("verpasste Termine"), "{err}");
    let row = cmd::recurring_set_active(
        &ctx,
        cmd::RecurringSetActiveArgs {
            id: auto.id.clone(),
            active: true,
            resume_mode: Some(ResumeMode::CatchUp),
        },
    )
    .await
    .unwrap();
    assert_eq!(row.active, 1);
    assert_eq!(
        row.next_due_date, "2026-01-01",
        "Nachholen bestätigt, Termin bleibt"
    );
    // bestätigt: der nächste Lauf bucht alle fünf Termine
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 5, "{r:?}");
    // Modus Erinnerung: Fortsetzen ohne Wahl (es wird nichts automatisch gebucht)
    let row = cmd::recurring_set_active(
        &ctx,
        cmd::RecurringSetActiveArgs {
            id: reminder.id.clone(),
            active: true,
            resume_mode: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(row.active, 1);
}

/// Eine Bestätigung gilt über einen abgebrochenen Lauf hinaus: Der nächste Lauf
/// setzt das Nachholen fort, statt erneut nachzufragen.
#[tokio::test]
async fn confirmed_catch_up_survives_interrupted_run() {
    let env = setup().await;
    let sub = recurring::create(&env.pool, &sub_input("Miete", 1, d(2025, 12, 1), true))
        .await
        .unwrap();
    scheduler::recurring::apply_resume_mode(
        &env.pool,
        &sub,
        scheduler::catch_up::ResumeMode::CatchUp,
        d(2026, 5, 20),
    )
    .await
    .unwrap();
    // Der zweite Termin scheitert (Abbruch mitten im Nachholen).
    sqlx::query(
        "CREATE TRIGGER test_block_second BEFORE UPDATE OF next_due_date ON recurring_subscriptions
         WHEN NEW.next_due_date > '2026-01-01'
         BEGIN SELECT RAISE(ABORT, 'Test: Abbruch'); END",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(
        (r.created_expenses, r.failed_subscriptions),
        (1, 1),
        "{r:?}"
    );
    sqlx::query("DROP TRIGGER test_block_second")
        .execute(&env.pool)
        .await
        .unwrap();
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 5, "Rest ohne neue Rückfrage: {r:?}");
    assert_eq!(r.awaiting_decision, 0);
    assert!(
        app_settings::get(&env.pool, &format!("recurring_catch_up_ok:sub:{}", sub.id))
            .await
            .unwrap()
            .is_none()
    );
}

/// W14-025b: Eine Abbruch-Anforderung (Herunterfahren, Sperren) beendet den
/// Tick vor dem nächsten Termin bzw. Job — kein Beleg, kein Tages-Backup,
/// kein Vollscan.
#[tokio::test]
async fn stop_request_ends_tick_before_work() {
    let env = setup().await;
    recurring::create(&env.pool, &sub_input("Hosting", 1, d(2026, 5, 1), true))
        .await
        .unwrap();
    let stop = std::sync::atomic::AtomicBool::new(true);
    let clock = noon(d(2026, 5, 20));
    scheduler::tick::run_jobs(&job(&env, &clock).with_stop(&stop))
        .await
        .unwrap();
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM expenses").await, 0);
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM backup_history").await,
        0
    );
    assert_eq!(
        count(&env.pool, "SELECT COUNT(*) FROM archive_integrity_checks").await,
        0
    );
}

// ---- Prüferbefunde F1d ------------------------------------------------------

/// F1d: Die Bestätigung „nachholen“ deckt nur Termine bis zu ihrem Tag. Wird
/// danach monatelang nicht gebucht (Instanz gesperrt), folgen die späteren
/// Termine wieder der Nachhol-Grenze (mehr als drei → neue Entscheidung).
#[tokio::test]
async fn confirmation_covers_only_dates_up_to_its_day() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let sub = recurring::create(&env.pool, &sub_input("Miete", 5, d(2025, 10, 5), true))
        .await
        .unwrap();
    let tmpl = recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2025, 11, 1), "issue"))
        .await
        .unwrap();
    // Bestätigung am 05.03.2026: Kosten 10/2025–03/2026 (6), Rechnungen
    // 11/2025–03/2026 (5).
    let confirmed_on = d(2026, 3, 5);
    let row = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    scheduler::recurring::apply_resume_mode(
        &env.pool,
        &row,
        scheduler::catch_up::ResumeMode::CatchUp,
        confirmed_on,
    )
    .await
    .unwrap();
    let trow = recurring_invoice::get_row(&env.pool, &tmpl.template.id)
        .await
        .unwrap()
        .unwrap();
    scheduler::recurring_invoice::apply_resume_mode(
        &env.pool,
        &trow,
        scheduler::catch_up::ResumeMode::CatchUp,
        confirmed_on,
    )
    .await
    .unwrap();
    // Entsperrt erst am 20.08.2026: nur die bestätigten Termine.
    let clock = noon(d(2026, 8, 20));
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_expenses, 6, "{r:?}");
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_invoices, 5, "{r:?}");
    assert_eq!(
        count(
            &env.pool,
            "SELECT COUNT(*) FROM invoices WHERE locked_at IS NOT NULL"
        )
        .await,
        0,
        "keiner der nachgeholten Termine ist der jüngste — nur Entwürfe"
    );
    let s = recurring::get(&env.pool, &sub.id).await.unwrap().unwrap();
    assert_eq!(s.next_due_date, "2026-04-05");
    let t = recurring_invoice::get_row(&env.pool, &tmpl.template.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.next_due_date, "2026-04-01");
    // Danach: fünf weitere Termine → neue Entscheidung, nichts gebucht.
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!((r.created_expenses, r.awaiting_decision), (0, 1), "{r:?}");
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!((r.created_invoices, r.awaiting_decision), (0, 1), "{r:?}");
    assert!(
        app_settings::list_prefix(&env.pool, "recurring_catch_up_ok:")
            .await
            .unwrap()
            .is_empty(),
        "Bestätigungen erledigt"
    );
}

/// F1d: Scheitert nach zwei gebuchten Terminen der dritte, nennen Hinweis und
/// Dedup-Schlüssel den gescheiterten Termin, nicht den ersten des Laufs.
#[tokio::test]
async fn failure_notice_names_failed_occurrence() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let sub = recurring::create(&env.pool, &sub_input("Hosting", 1, d(2026, 3, 1), true))
        .await
        .unwrap();
    let tmpl = recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 3, 1), "draft"))
        .await
        .unwrap();
    for sql in [
        "CREATE TRIGGER test_fail_expense BEFORE INSERT ON expenses
         WHEN NEW.expense_date = '2026-05-01'
         BEGIN SELECT RAISE(ABORT, 'Test: Mai scheitert'); END",
        "CREATE TRIGGER test_fail_invoice BEFORE INSERT ON invoices
         WHEN NEW.delivery_date = '2026-05-01'
         BEGIN SELECT RAISE(ABORT, 'Test: Mai scheitert'); END",
    ] {
        sqlx::query(sql).execute(&env.pool).await.unwrap();
    }
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(
        (r.created_expenses, r.failed_subscriptions),
        (2, 1),
        "{r:?}"
    );
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!((r.created_invoices, r.failed_templates), (2, 1), "{r:?}");
    assert_eq!(
        notices(
            &env.pool,
            &format!("recurring_failed:{}:2026-05-01", sub.id)
        )
        .await,
        1,
        "{}",
        notice_bodies(&env.pool).await
    );
    assert_eq!(
        notices(
            &env.pool,
            &format!("recurring_invoice_failed:{}:2026-05-01", tmpl.template.id)
        )
        .await,
        1
    );
    assert_eq!(notices(&env.pool, "recurring_failed:%2026-03-01").await, 0);
    let bodies = notice_bodies(&env.pool).await;
    assert!(bodies.contains("Termin 01.05.2026"), "{bodies}");
    assert!(!bodies.contains("Termin 01.03.2026"), "{bodies}");
}

/// F1d: „Jetzt erstellen“ setzt einen nach einem Abbruch begonnenen Termin
/// derselben Vorlage zuerst fort, statt seinen Fortsetzungs-Marker zu
/// überschreiben — die frühere Rechnung bleibt nicht still als Entwurf liegen.
#[tokio::test]
async fn run_now_resumes_interrupted_occurrence_first() {
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let detail =
        recurring_invoice::create(&env.pool, &tmpl_input(&contact, d(2026, 5, 1), "issue"))
            .await
            .unwrap();
    let id = detail.template.id.clone();
    // Abbruch nach dem Entwurf für den Mai-Termin (Marker gesetzt, vorgerückt).
    let created_on = d(2026, 5, 2);
    let input = InvoiceInput {
        direction: InvoiceDirection::Issued,
        invoice_date: created_on,
        delivery_date: Some(d(2026, 5, 1)),
        due_date: Some(d(2026, 5, 16)),
        currency_code: "EUR".into(),
        items: vec![item()],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    let draft = inv_cmd::create_invoice_draft_on(&env.pool, &contact, &input, None, created_on)
        .await
        .unwrap();
    let earlier = draft.invoice.id.clone();
    recurring_invoice::advance(&env.pool, &id, "2026-05-01", &earlier, "2026-06-01")
        .await
        .unwrap();
    app_settings::set(
        &env.pool,
        &format!("recurring_invoice_pending:{id}"),
        &format!("{earlier}|2026-05-01|issue"),
    )
    .await
    .unwrap();
    // Instanz bis Juni gestoppt; nach dem Entsperren vor dem ersten Tick
    // klickt der Inhaber „Jetzt erstellen“.
    let clock = noon(d(2026, 6, 20));
    let newer = scheduler::recurring_invoice::run_now(&job(&env, &clock), &id)
        .await
        .unwrap();
    assert_ne!(newer, earlier);
    for inv in [&earlier, &newer] {
        let locked: Option<String> =
            sqlx::query_scalar("SELECT locked_at FROM invoices WHERE id = ?")
                .bind(inv)
                .fetch_one(&env.pool)
                .await
                .unwrap();
        assert!(
            locked.is_some(),
            "{inv} festgeschrieben; Hinweise:\n{}",
            notice_bodies(&env.pool).await
        );
    }
    assert!(
        app_settings::get(&env.pool, &format!("recurring_invoice_pending:{id}"))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM invoices").await, 2);
}

/// F5-fix (S2, KB-0014): Eine unter §19 gespeicherte Vorlage (Steuerart E)
/// erzeugt nach dem Wechsel zur Regelbesteuerung keinen nummerierten Entwurf,
/// der beim Festschreiben scheitert: der Termin wird übersprungen (Hinweis,
/// keine Nummer, Vorlage bleibt stehen), bis die Vorlage umgestellt ist.
/// Speichern mit E bei Regelbesteuerung lehnt der Server ab.
#[tokio::test]
async fn exempt_template_after_switch_to_regular_taxation_is_blocked() {
    use klein_buch_lib::commands::recurring_invoice as cmd;
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let tmpl = recurring_invoice::create(
        &env.pool,
        &tmpl_input(&contact, d(2026, 5, 1), "issue_send"),
    )
    .await
    .unwrap();
    // Verzicht auf §19 (Regelbesteuerung ab 01.01.2026) nach dem Speichern der
    // Vorlage — Eintrag in der Statushistorie (ADR 0042).
    {
        use klein_buch_lib::db::repo::tax_status;
        use klein_buch_lib::domain::tax_status::{verzicht_binding_until, ChangeReason, TaxStatus};
        tax_status::insert(
            &env.pool,
            &tax_status::NewEntry {
                effective_from: d(2026, 1, 1),
                status: TaxStatus::Regelbesteuerung,
                reason: ChangeReason::Verzicht,
                binding_until: Some(verzicht_binding_until(d(2026, 1, 1))),
                trigger_document: None,
                note: None,
            },
            d(2026, 5, 20),
        )
        .await
        .unwrap();
    }
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_invoices, 0, "{r:?}");
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM invoices").await, 0);
    assert_eq!(
        count(
            &env.pool,
            "SELECT COALESCE(SUM(last_value), 0) FROM doc_number_counters"
        )
        .await,
        0,
        "keine Nummer verbraucht"
    );
    assert_eq!(notices(&env.pool, "recurring_invoice_blocked:").await, 1);
    let bodies = notice_bodies(&env.pool).await;
    assert!(bodies.contains("Steuerart S"), "{bodies}");
    let t = recurring_invoice::get_row(&env.pool, &tmpl.template.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.next_due_date, "2026-05-01", "Termin bleibt offen");

    // Ein zweiter Lauf am selben Tag: kein weiterer Hinweis, kein Entwurf.
    scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(notices(&env.pool, "recurring_invoice_blocked:").await, 1);
    assert_eq!(count(&env.pool, "SELECT COUNT(*) FROM invoices").await, 0);

    // Speichern mit E bei Regelbesteuerung wird abgelehnt; mit S/19 % geht es.
    let ctx = ctx_for(&env, noon(d(2026, 5, 20))).await;
    let err = cmd::recurring_invoices_update(
        &ctx,
        cmd::RecurringInvoicesUpdateArgs {
            id: tmpl.template.id.clone(),
            input: tmpl_input(&contact, d(2026, 5, 1), "issue_send"),
        },
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("Steuerart E"), "{err}");
    let mut fixed = tmpl_input(&contact, d(2026, 5, 1), "draft");
    fixed.items[0].tax_category_code = "S".into();
    fixed.items[0].tax_rate_percent = 19.0;
    cmd::recurring_invoices_update(
        &ctx,
        cmd::RecurringInvoicesUpdateArgs {
            id: tmpl.template.id.clone(),
            input: fixed,
        },
    )
    .await
    .unwrap();
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_invoices, 1, "{r:?}");
}

/// ADR 0042 Nr. 3: Nachgeholte Abo-Termine tragen den §19-Status ihres
/// Leistungsdatums. Wegfall im laufenden Jahr ab 15.04.2026: Die Termine 01.03.
/// und 01.04. entstehen als §19-Rechnungen; der Termin 01.05. fiele unter die
/// Regelbesteuerung — mit Steuerart E entsteht dort keine Rechnung (Hinweis,
/// keine Nummer), der Termin bleibt offen.
#[tokio::test]
async fn catch_up_occurrences_follow_status_at_their_date() {
    use klein_buch_lib::db::repo::tax_status;
    use klein_buch_lib::domain::tax_status::{ChangeReason, TaxStatus};
    let env = setup().await;
    mk_seller(&env.pool).await;
    let contact = mk_contact(&env.pool).await;
    let tmpl = recurring_invoice::create(
        &env.pool,
        &tmpl_input(&contact, d(2026, 3, 1), "issue_send"),
    )
    .await
    .unwrap();
    tax_status::insert(
        &env.pool,
        &tax_status::NewEntry {
            effective_from: d(2026, 4, 15),
            status: TaxStatus::Regelbesteuerung,
            reason: ChangeReason::WegfallLaufend,
            binding_until: None,
            trigger_document: Some("RE-2026-0042"),
            note: None,
        },
        d(2026, 5, 20),
    )
    .await
    .unwrap();
    let clock = noon(d(2026, 5, 20));
    let r = scheduler::recurring_invoice::process_due(&job(&env, &clock))
        .await
        .unwrap();
    assert_eq!(r.created_invoices, 2, "{r:?}");
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT delivery_date, is_kleinunternehmer FROM invoices ORDER BY delivery_date",
    )
    .fetch_all(&env.pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (Some("2026-03-01".into()), 1),
            (Some("2026-04-01".into()), 1)
        ]
    );
    assert_eq!(notices(&env.pool, "recurring_invoice_blocked:").await, 1);
    assert!(
        notice_bodies(&env.pool).await.contains("Termin 01.05.2026"),
        "{}",
        notice_bodies(&env.pool).await
    );
    let t = recurring_invoice::get_row(&env.pool, &tmpl.template.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.next_due_date, "2026-05-01", "Termin bleibt offen");
}
