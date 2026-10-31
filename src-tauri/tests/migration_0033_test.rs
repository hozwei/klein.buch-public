//! Integrationstests für Migration `0033_tax_status_history.sql` (ADR 0042
//! Nr. 1–6, Lauf 2026.11.1, Block V1).
//!
//! Gegen eine Tempfile-SQLite mit dem echten `MIGRATOR`:
//!
//!   * Schema 33, Regel `rule_tax_threshold`.
//!   * `tax_status_changes` ist append-only: kein DELETE, UPDATE nur als
//!     einmaliger Widerruf mit Grund; CHECKs zu Grund, Status, Wirkungsdatum
//!     und Bindung; höchstens ein gültiger Eintrag je Wirkungsdatum.
//!   * Übernahme des Bestands aus dem Profil (Schema 32): für jedes Jahr
//!     2000–2040 dasselbe Ergebnis wie die bisherige Logik
//!     (`is_klein_for_year`), Bindungsende nach ADR 0042 Nr. 2.
//!   * Der Spiegel im Profil folgt der Historie.
//!
//! Testdaten nur mit Dokumentations-Adressen (example.org).

use std::borrow::Cow;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};
use klein_buch_lib::db::repo::tax_status;
use klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::tax_status::{
    verzicht_binding_until, ChangeReason, TaxStatus, YearStatus,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

async fn open(dir: &std::path::Path) -> SqlitePool {
    let url = format!(
        "sqlite://{}",
        dir.join("klein-buch.sqlite").to_string_lossy()
    );
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap()
}

/// Migrator ohne 0033 (DB auf Schema 32).
fn migrator_until_32() -> sqlx::migrate::Migrator {
    let all = &MIGRATOR.migrations;
    let idx = all
        .iter()
        .position(|m| m.description.contains("tax status history"))
        .expect("Migration 0033");
    sqlx::migrate::Migrator {
        migrations: Cow::Owned(all[..idx].to_vec()),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

async fn schema_version(pool: &SqlitePool) -> String {
    sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn setup() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let pool = open(dir.path()).await;
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

fn assert_blocked<T: std::fmt::Debug>(res: sqlx::Result<T>, needle: &str) {
    let err = res.expect_err("muss abbrechen");
    let msg = err.to_string();
    assert!(msg.contains(needle), "erwartet „{needle}“, war: {msg}");
}

async fn insert_raw(
    pool: &SqlitePool,
    id: &str,
    from: &str,
    status: &str,
    reason: &str,
    binding: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO tax_status_changes (id, effective_from, status, reason, binding_until)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(from)
    .bind(status)
    .bind(reason)
    .bind(binding)
    .execute(pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn schema_33_and_threshold_rule_seeded() {
    let (pool, _dir) = setup().await;
    // Seit 0034 (ADR 0042 Nr. 7) steht das Schema höher; 0033 ist darin enthalten.
    const { assert!(EXPECTED_SCHEMA_VERSION >= 33) };
    assert_eq!(
        schema_version(&pool).await,
        EXPECTED_SCHEMA_VERSION.to_string()
    );
    let (rule_type, enabled): (String, i64) = sqlx::query_as(
        "SELECT rule_type, enabled FROM notification_rules WHERE id = 'rule_tax_threshold'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((rule_type.as_str(), enabled), ("custom", 1));
    // Ohne Profil: keine Einträge (ohne Eintrag gilt §19).
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tax_status_changes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn append_only_delete_and_update_blocked_revoke_once() {
    let (pool, _dir) = setup().await;
    insert_raw(
        &pool,
        "a",
        "0001-01-01",
        "kleinunternehmer",
        "initial",
        None,
    )
    .await
    .unwrap();
    insert_raw(
        &pool,
        "b",
        "2026-01-01",
        "regelbesteuerung",
        "verzicht",
        Some("2031-01-01"),
    )
    .await
    .unwrap();

    assert_blocked(
        sqlx::query("DELETE FROM tax_status_changes WHERE id = 'b'")
            .execute(&pool)
            .await,
        "append-only",
    );
    for sql in [
        "UPDATE tax_status_changes SET status = 'kleinunternehmer' WHERE id = 'b'",
        "UPDATE tax_status_changes SET effective_from = '2027-01-01' WHERE id = 'b'",
        "UPDATE tax_status_changes SET note = 'nachträglich' WHERE id = 'b'",
        "UPDATE tax_status_changes SET binding_until = '2030-01-01' WHERE id = 'b'",
    ] {
        assert_blocked(sqlx::query(sql).execute(&pool).await, "nur ein Widerruf");
    }
    // Widerruf ohne Grund: CHECK.
    assert_blocked(
        sqlx::query(
            "UPDATE tax_status_changes SET revoked_at = datetime('now','utc') WHERE id = 'b'",
        )
        .execute(&pool)
        .await,
        "CHECK",
    );
    // Widerruf mit Grund: erlaubt, danach endgültig.
    sqlx::query(
        "UPDATE tax_status_changes SET revoked_at = datetime('now','utc'),
                revoke_reason = 'Irrtum' WHERE id = 'b'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_blocked(
        sqlx::query(
            "UPDATE tax_status_changes SET revoked_at = NULL, revoke_reason = NULL WHERE id = 'b'",
        )
        .execute(&pool)
        .await,
        "nur ein Widerruf",
    );
    // Nach dem Widerruf ist das Wirkungsdatum wieder frei (Teilindex).
    insert_raw(
        &pool,
        "c",
        "2026-01-01",
        "regelbesteuerung",
        "wegfall_vorjahr",
        None,
    )
    .await
    .unwrap();
    assert_blocked(
        insert_raw(
            &pool,
            "d",
            "2026-01-01",
            "regelbesteuerung",
            "wegfall_laufend",
            None,
        )
        .await,
        "UNIQUE",
    );
}

#[tokio::test]
async fn checks_reason_status_date_and_binding() {
    let (pool, _dir) = setup().await;
    // Grund ↔ Status
    assert_blocked(
        insert_raw(
            &pool,
            "x1",
            "2027-01-01",
            "kleinunternehmer",
            "verzicht",
            Some("2032-01-01"),
        )
        .await,
        "CHECK",
    );
    assert_blocked(
        insert_raw(
            &pool,
            "x2",
            "2027-01-01",
            "regelbesteuerung",
            "rueckkehr",
            None,
        )
        .await,
        "CHECK",
    );
    // 1. Januar bei Verzicht, Vorjahr, Rückkehr
    for (reason, status, binding) in [
        ("verzicht", "regelbesteuerung", Some("2031-03-15")),
        ("wegfall_vorjahr", "regelbesteuerung", None),
        ("rueckkehr", "kleinunternehmer", None),
    ] {
        assert_blocked(
            insert_raw(&pool, "x3", "2026-03-15", status, reason, binding).await,
            "CHECK",
        );
    }
    // Laufendes Jahr und Gründungsjahr: beliebiger Tag
    insert_raw(
        &pool,
        "ok1",
        "2026-08-15",
        "regelbesteuerung",
        "wegfall_laufend",
        None,
    )
    .await
    .unwrap();
    insert_raw(
        &pool,
        "ok2",
        "2026-09-02",
        "regelbesteuerung",
        "wegfall_gruendungsjahr",
        None,
    )
    .await
    .unwrap();
    // Bindung: genau fünf Kalenderjahre einschließlich des Wirkungsjahres
    assert_blocked(
        insert_raw(
            &pool,
            "x4",
            "2027-01-01",
            "regelbesteuerung",
            "verzicht",
            Some("2033-01-01"),
        )
        .await,
        "CHECK",
    );
    assert_blocked(
        insert_raw(
            &pool,
            "x5",
            "2027-01-01",
            "regelbesteuerung",
            "verzicht",
            None,
        )
        .await,
        "CHECK",
    );
    assert_blocked(
        insert_raw(
            &pool,
            "x6",
            "2028-01-01",
            "regelbesteuerung",
            "wegfall_vorjahr",
            Some("2033-01-01"),
        )
        .await,
        "CHECK",
    );
    insert_raw(
        &pool,
        "ok3",
        "2027-01-01",
        "regelbesteuerung",
        "verzicht",
        Some("2032-01-01"),
    )
    .await
    .unwrap();
    // Anfangsstand nur „seit Beginn“, kein ungültiges Datum
    assert_blocked(
        insert_raw(
            &pool,
            "x7",
            "2020-01-01",
            "kleinunternehmer",
            "initial",
            None,
        )
        .await,
        "CHECK",
    );
    assert_blocked(
        insert_raw(
            &pool,
            "x8",
            "2026-02-30",
            "regelbesteuerung",
            "wegfall_laufend",
            None,
        )
        .await,
        "CHECK",
    );
    assert_blocked(
        insert_raw(
            &pool,
            "x9",
            "2026-13-01",
            "regelbesteuerung",
            "wegfall_laufend",
            None,
        )
        .await,
        "CHECK",
    );
}

/// Bisherige Logik (`seller_profile::is_klein_for_year` bis Schema 32): Jahre vor
/// dem Jahr des Verzichtsdatums §19, ab dann der Schalter; ohne Profil §19.
fn old_is_klein_for_year(profile: Option<(i64, Option<&str>)>, year: i32) -> bool {
    match profile {
        None => true,
        Some((klein, waived)) => {
            match waived.and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()) {
                Some(d) if year < d.year() => true,
                _ => klein == 1,
            }
        }
    }
}

/// ADR 0042 Nr. 6: Für jeden Bestand (kein Profil, Schalter an/aus, mit und
/// ohne Verzichtsdatum, kaputtes Datum) liefert die Historie nach 0033 für jedes
/// Jahr 2000–2040 dasselbe Ergebnis wie bisher — nie einen Wechsel im Jahr, also
/// auch keine Abweichung in abgeschlossenen Jahren.
#[tokio::test]
async fn backfill_matches_old_result_for_every_year() {
    let cases: [Option<(i64, Option<&str>)>; 7] = [
        None,
        Some((1, None)),
        Some((1, Some("2026-03-15"))),
        Some((0, Some("2026-03-15"))),
        Some((0, Some("2026-12-31"))),
        Some((0, None)),
        Some((0, Some("kaputt"))),
    ];
    for case in cases {
        let dir = tempfile::tempdir().unwrap();
        let pool = open(dir.path()).await;
        migrator_until_32().run(&pool).await.unwrap();
        assert_eq!(schema_version(&pool).await, "32");
        if let Some((klein, waived)) = case {
            sqlx::query(
                "INSERT INTO seller_profile (id, name, street, postal_code, city, email,
                                             is_kleinunternehmer, waived_paragraph_19_since)
                 VALUES (1, 'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt',
                         'kontakt@example.org', ?, ?)",
            )
            .bind(klein)
            .bind(waived)
            .execute(&pool)
            .await
            .unwrap();
        }
        MIGRATOR.run(&pool).await.unwrap();
        assert_eq!(
            schema_version(&pool).await,
            EXPECTED_SCHEMA_VERSION.to_string()
        );

        let hist = tax_status::history(&pool).await.unwrap();
        for year in 2000..=2040 {
            let old = old_is_klein_for_year(case, year);
            let new = hist.year_status(year);
            assert_ne!(new, YearStatus::Wechsel, "{case:?} {year}");
            assert_eq!(new.is_klein(), old, "{case:?} {year}");
        }
        // Bindung nach Nr. 2: Verzicht 2026 → Rückkehr frühestens 01.01.2031.
        let rows = tax_status::list(&pool).await.unwrap();
        match case {
            None => assert!(rows.is_empty()),
            Some((0, Some(w))) if w.starts_with("2026") => {
                assert_eq!(rows.len(), 2, "{rows:?}");
                assert_eq!(
                    (rows[0].effective_from.as_str(), rows[0].reason.as_str()),
                    ("0001-01-01", "initial")
                );
                assert_eq!(rows[0].status, "kleinunternehmer");
                assert_eq!(
                    (
                        rows[1].effective_from.as_str(),
                        rows[1].status.as_str(),
                        rows[1].reason.as_str(),
                        rows[1].binding_until.as_deref()
                    ),
                    (
                        "2026-01-01",
                        "regelbesteuerung",
                        "legacy",
                        Some("2031-01-01")
                    )
                );
                assert_eq!(rows[1].id, "00000000-0000-7000-8000-000000000002");
                assert!(rows[1].note.as_deref().unwrap().contains(w), "{rows:?}");
                assert_eq!(
                    hist.binding_at(NaiveDate::from_ymd_opt(2030, 6, 1).unwrap()),
                    NaiveDate::from_ymd_opt(2031, 1, 1)
                );
            }
            Some(_) => {
                assert_eq!(rows.len(), 1, "{rows:?}");
                assert_eq!(rows[0].id, "00000000-0000-7000-8000-000000000001");
                assert_eq!(rows[0].effective_from, "0001-01-01");
            }
        }
        // Audit der Übernahme je Eintrag.
        let audits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'tax_status.migrated'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audits as usize, rows.len());
    }
}

/// Der Spiegel im Profil folgt der Historie (nur informativ; maßgeblich ist die
/// Historie, ADR 0042 Nr. 1).
#[tokio::test]
async fn mirror_follows_history() {
    let (pool, _dir) = setup().await;
    sqlx::query(
        "INSERT INTO seller_profile (id, name, street, postal_code, city, email)
         VALUES (1, 'Beispiel IT-Service', 'Beispielweg 1', '12345', 'Musterstadt',
                 'kontakt@example.org')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let jan = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    tax_status::insert(
        &pool,
        &tax_status::NewEntry {
            effective_from: jan,
            status: TaxStatus::Regelbesteuerung,
            reason: ChangeReason::Verzicht,
            binding_until: Some(verzicht_binding_until(jan)),
            trigger_document: None,
            note: None,
        },
        today,
    )
    .await
    .unwrap();
    let mirror: (i64, Option<String>) = sqlx::query_as(
        "SELECT is_kleinunternehmer, waived_paragraph_19_since FROM seller_profile WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mirror, (0, Some("2026-01-01".into())));
    // Die ID eines neuen Eintrags ist eine UUIDv7.
    let rows = tax_status::list(&pool).await.unwrap();
    let id = uuid::Uuid::parse_str(&rows[0].id).unwrap();
    assert_eq!(id.get_version_num(), 7);
    let audit: String = sqlx::query_scalar(
        "SELECT details_json FROM audit_log WHERE action = 'tax_status.changed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit.contains("\"reason\":\"verzicht\""), "{audit}");
    assert!(!audit.to_lowercase().contains("passphrase"));
}
