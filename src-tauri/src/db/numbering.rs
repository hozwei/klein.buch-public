//! Atomare Belegnummer-Allokation pro (`doc_type`, `fiscal_year`).
//!
//! Schicht: **Imperative Shell**. Format-Layer (Prefix-Schema, Zero-Padding)
//! lebt in [`crate::domain::numbering`]; hier nur die Counter-Persistenz.
//!
//! ## Pattern
//!
//! Pro Aufruf wird in einer Transaktion ausgeführt:
//!
//! 1. `INSERT OR IGNORE INTO doc_number_counters (doc_type, fiscal_year, last_value) VALUES (?, ?, 0)`
//!    legt eine Zeile für ein neues Geschäftsjahr an, wenn noch keine existiert.
//! 2. `UPDATE doc_number_counters SET last_value = last_value + 1
//!    WHERE doc_type = ? AND fiscal_year = ? RETURNING last_value`
//!    inkrementiert atomar und liefert den frischen Counter.
//!
//! Die Garantie (lückenlos, eindeutig) kommt aus dem **atomaren**
//! `UPDATE … RETURNING` in einer Schreib-Transaktion (`BEGIN IMMEDIATE` über
//! `db::begin_immediate` bzw. die Transaktion des Aufrufers) und dem
//! **Single-Writer**-Modell von SQLite: Es gibt höchstens einen Schreiber
//! gleichzeitig, weitere Verbindungen des Pools warten bis zum
//! `busy_timeout` (WAL-Modus). Dass der Pool eine Verbindung während einer
//! Transaktion exklusiv hält, reicht dafür nicht — im Server laufen Anfragen
//! parallel über mehrere Verbindungen (KB-0005). Der Test
//! `parallel_allocations_over_several_connections_stay_gapless` prüft das
//! gegen eine Datei-DB mit mehreren Verbindungen.
//!
//! ## GoBD
//!
//! Lückenlose Nummerierung pro Geschäftsjahr ist GoBD-Pflicht. Storno
//! erhält einen **eigenen** Counter (`DocType::StornoInvoice`), damit
//! die Rechnungs-Sequenz weiterhin lückenlos bleibt.
//!
//! ## Forward-only
//!
//! Counter werden niemals dekrementiert. Wenn eine Allokation passiert ist
//! aber die nachfolgende Operation scheitert (z. B. KoSIT-Validierung
//! schlägt fehl), bleibt die Nummer "verbraucht" und produziert eine Lücke.
//! Das ist GoBD-konform dokumentierbar (Storno-Vorgang); siehe
//! `memory/klein-buch/gobd-archivierung.md`.

use crate::{
    domain::numbering::{format, DocType},
    error::Result,
};
use sqlx::{Row, SqliteConnection, SqlitePool};

/// Allokiert die nächste Belegnummer und liefert die formatierte Version.
///
/// Wirft `Error::Db`, wenn die Transaktion scheitert (z. B. Disk-Voll,
/// Lock-Konflikt). Wirft niemals `last_value < 1` — wenn das passiert,
/// ist die DB korrupt.
pub async fn next_number(pool: &SqlitePool, doc_type: DocType, fiscal_year: i32) -> Result<String> {
    let seq = next_seq(pool, doc_type, fiscal_year).await?;
    Ok(format(doc_type, fiscal_year, seq))
}

/// Allokiert nur den Counter (ohne Formatierung). Hauptsächlich für Tests
/// und sehr seltene Sonderfälle, die den Slug direkt brauchen.
pub async fn next_seq(pool: &SqlitePool, doc_type: DocType, fiscal_year: i32) -> Result<u32> {
    let mut tx = crate::db::begin_immediate(pool).await?;
    let seq = next_seq_in(&mut tx, doc_type, fiscal_year).await?;
    tx.commit().await?;
    Ok(seq)
}

/// Wie [`next_number`], aber innerhalb einer laufenden Schreib-Transaktion
/// (`db::begin_immediate`): Nummer, Beleg-INSERT und Audit werden zusammen
/// committet oder zusammen verworfen (Lauf F, W14-022/-025) — eine
/// zurückgerollte Transaktion verbraucht keine Nummer.
pub async fn next_number_in(
    conn: &mut SqliteConnection,
    doc_type: DocType,
    fiscal_year: i32,
) -> Result<String> {
    let seq = next_seq_in(conn, doc_type, fiscal_year).await?;
    Ok(format(doc_type, fiscal_year, seq))
}

/// Counter-Schritt innerhalb einer laufenden Transaktion.
pub async fn next_seq_in(
    conn: &mut SqliteConnection,
    doc_type: DocType,
    fiscal_year: i32,
) -> Result<u32> {
    let slug = doc_type.db_slug();

    // Zeile für (slug, year) sicherstellen — kollidiert sie schon, ignorieren.
    sqlx::query(
        "INSERT OR IGNORE INTO doc_number_counters (doc_type, fiscal_year, last_value)
         VALUES (?, ?, 0)",
    )
    .bind(slug)
    .bind(fiscal_year)
    .execute(&mut *conn)
    .await?;

    // Atomar inkrementieren + neuen Wert zurückgeben.
    let row = sqlx::query(
        "UPDATE doc_number_counters
            SET last_value = last_value + 1
          WHERE doc_type = ? AND fiscal_year = ?
        RETURNING last_value",
    )
    .bind(slug)
    .bind(fiscal_year)
    .fetch_one(&mut *conn)
    .await?;

    let last_value: i64 = row.try_get("last_value")?;
    if last_value < 1 {
        return Err(crate::error::Error::Domain(format!(
            "doc_number_counters.last_value < 1 für ({}, {}): {}",
            slug, fiscal_year, last_value
        )));
    }
    Ok(last_value as u32)
}

/// Stand eines Nummernkreises (eine Zeile von `doc_number_counters`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CounterState {
    pub doc_type: String,
    pub fiscal_year: i64,
    pub last_value: i64,
}

/// Alle Nummernkreise (für Restore-Abgleich und Anzeige), sortiert.
pub async fn snapshot(pool: &SqlitePool) -> Result<Vec<CounterState>> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT doc_type, fiscal_year, last_value FROM doc_number_counters
         ORDER BY fiscal_year DESC, doc_type",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(doc_type, fiscal_year, last_value)| CounterState {
            doc_type,
            fiscal_year,
            last_value,
        })
        .collect())
}

/// Ergebnis einer Anhebung: Nummernkreis mit altem und neuem Stand.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Raised {
    pub doc_type: String,
    pub fiscal_year: i64,
    pub from: i64,
    pub to: i64,
}

/// Hebt einen Nummernkreis innerhalb einer laufenden Schreib-Transaktion
/// **nur aufwärts** auf `last_value` an (Lauf F, D-24). Die nächste Nummer ist
/// danach `last_value + 1`; die übersprungenen Nummern bleiben eine
/// dokumentierte Lücke (GoBD: lieber Lücke als Doppelvergabe, §14 Abs. 4 Nr. 4
/// UStG). Belege werden nicht angefasst. `None` = Stand war schon gleich oder
/// höher (keine Änderung).
pub async fn raise_in(
    conn: &mut SqliteConnection,
    doc_type: &str,
    fiscal_year: i64,
    last_value: i64,
) -> Result<Option<Raised>> {
    let current: Option<i64> = sqlx::query_scalar(
        "SELECT last_value FROM doc_number_counters WHERE doc_type = ? AND fiscal_year = ?",
    )
    .bind(doc_type)
    .bind(fiscal_year)
    .fetch_optional(&mut *conn)
    .await?;
    let from = current.unwrap_or(0);
    if last_value <= from {
        return Ok(None);
    }
    sqlx::query(
        "INSERT INTO doc_number_counters (doc_type, fiscal_year, last_value) VALUES (?, ?, ?)
         ON CONFLICT(doc_type, fiscal_year) DO UPDATE SET last_value = excluded.last_value
         WHERE excluded.last_value > doc_number_counters.last_value",
    )
    .bind(doc_type)
    .bind(fiscal_year)
    .bind(last_value)
    .execute(&mut *conn)
    .await?;
    Ok(Some(Raised {
        doc_type: doc_type.to_string(),
        fiscal_year,
        from,
        to: last_value,
    }))
}

/// Liefert den aktuellen Counter-Stand, ohne ihn zu verändern. Für UI und
/// Tests; nicht für die Allokation benutzen.
pub async fn peek_seq(pool: &SqlitePool, doc_type: DocType, fiscal_year: i32) -> Result<u32> {
    let slug = doc_type.db_slug();
    let row = sqlx::query(
        "SELECT last_value FROM doc_number_counters
         WHERE doc_type = ? AND fiscal_year = ?",
    )
    .bind(slug)
    .bind(fiscal_year)
    .fetch_optional(pool)
    .await?;

    Ok(row
        .map(|r| r.get::<i64, _>("last_value") as u32)
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// In-Memory-DB mit dem Numbering-Counter-Schema. Genügt für diese
    /// Modul-Tests; die produktive Migration läuft separat via
    /// [`crate::db::MIGRATOR`].
    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE doc_number_counters (
                doc_type    TEXT NOT NULL,
                fiscal_year INTEGER NOT NULL,
                last_value  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (doc_type, fiscal_year)
            ) STRICT",
        )
        .execute(&pool)
        .await
        .expect("create table");

        pool
    }

    #[tokio::test]
    async fn first_invoice_in_fiscal_year_is_one() {
        let pool = fresh_pool().await;
        let n = next_number(&pool, DocType::Invoice, 2026).await.unwrap();
        assert_eq!(n, "RE-2026-0001");
    }

    #[tokio::test]
    async fn sequential_invoices_increment_per_year() {
        let pool = fresh_pool().await;
        for expected in 1..=5u32 {
            let n = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
            assert_eq!(n, expected);
        }
        // Anderer GJ → eigener Counter, startet bei 1.
        let n = next_seq(&pool, DocType::Invoice, 2027).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn storno_and_invoice_have_independent_counters() {
        let pool = fresh_pool().await;
        let _ = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        let _ = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        let s1 = next_number(&pool, DocType::StornoInvoice, 2026)
            .await
            .unwrap();
        assert_eq!(s1, "ST-2026-0001");
        // Rechnungs-Sequenz bleibt unberührt
        let i3 = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        assert_eq!(i3, 3);
    }

    #[tokio::test]
    async fn peek_does_not_advance_counter() {
        let pool = fresh_pool().await;
        assert_eq!(peek_seq(&pool, DocType::Invoice, 2026).await.unwrap(), 0);
        let _ = next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        assert_eq!(peek_seq(&pool, DocType::Invoice, 2026).await.unwrap(), 1);
        assert_eq!(peek_seq(&pool, DocType::Invoice, 2026).await.unwrap(), 1);
    }

    /// Pessimistischer Concurrency-Test: 50 parallele Allokationen müssen
    /// eine kontinuierliche, kollisionsfreie Sequenz 1..=50 ergeben.
    #[tokio::test]
    async fn parallel_allocations_have_no_gaps_or_duplicates() {
        let pool = fresh_pool().await;
        let mut handles = Vec::new();
        for _ in 0..50 {
            let p = pool.clone();
            handles.push(tokio::spawn(async move {
                next_seq(&p, DocType::Invoice, 2026).await.unwrap()
            }));
        }
        let mut got: Vec<u32> = Vec::new();
        for h in handles {
            got.push(h.await.unwrap());
        }
        got.sort_unstable();
        let expected: Vec<u32> = (1..=50).collect();
        assert_eq!(got, expected, "parallele Allokation hat Lücken/Duplikate");
    }

    /// KB-0005: wie im Server — Datei-DB im WAL-Modus, mehrere Verbindungen,
    /// `busy_timeout`; parallele Allokationen über verschiedene Verbindungen
    /// ergeben je Nummernkreis eine lückenlose, eindeutige Folge.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parallel_allocations_over_several_connections_stay_gapless() {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::db::open_pool(&dir.path().join("n.sqlite"), None)
            .await
            .unwrap();
        assert!(
            pool.options().get_max_connections() > 1,
            "mehrere Verbindungen"
        );
        sqlx::query(
            "CREATE TABLE doc_number_counters (
                doc_type    TEXT NOT NULL,
                fiscal_year INTEGER NOT NULL,
                last_value  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (doc_type, fiscal_year)
            ) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut set = tokio::task::JoinSet::new();
        for i in 0..60 {
            let p = pool.clone();
            let dt = if i % 3 == 0 {
                DocType::StornoInvoice
            } else {
                DocType::Invoice
            };
            set.spawn(async move { (dt, next_seq(&p, dt, 2026).await.unwrap()) });
        }
        let mut invoices = Vec::new();
        let mut stornos = Vec::new();
        while let Some(r) = set.join_next().await {
            match r.unwrap() {
                (DocType::Invoice, n) => invoices.push(n),
                (_, n) => stornos.push(n),
            }
        }
        invoices.sort_unstable();
        stornos.sort_unstable();
        assert_eq!(invoices, (1..=40).collect::<Vec<u32>>());
        assert_eq!(stornos, (1..=20).collect::<Vec<u32>>());
    }

    /// D-24: Anheben nur aufwärts, nächste Nummer = Stand + 1.
    #[tokio::test]
    async fn raise_only_upwards_then_next_follows() {
        let pool = fresh_pool().await;
        for _ in 0..3 {
            next_seq(&pool, DocType::Invoice, 2026).await.unwrap();
        }
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(raise_in(&mut tx, "invoice", 2026, 2).await.unwrap(), None);
        assert_eq!(raise_in(&mut tx, "invoice", 2026, 3).await.unwrap(), None);
        let r = raise_in(&mut tx, "invoice", 2026, 7)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((r.from, r.to), (3, 7));
        // fehlender Kreis (anderes Jahr) wird angelegt
        let r = raise_in(&mut tx, "storno_invoice", 2025, 4)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((r.from, r.to), (0, 4));
        tx.commit().await.unwrap();
        assert_eq!(
            next_number(&pool, DocType::Invoice, 2026).await.unwrap(),
            "RE-2026-0008"
        );
        assert_eq!(
            next_number(&pool, DocType::StornoInvoice, 2025)
                .await
                .unwrap(),
            "ST-2025-0005"
        );
        let snap = snapshot(&pool).await.unwrap();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].fiscal_year, 2026);
    }

    #[tokio::test]
    async fn all_doc_types_have_independent_counters() {
        let pool = fresh_pool().await;
        for dt in [
            DocType::Quote,
            DocType::Invoice,
            DocType::StornoInvoice,
            DocType::Expense,
            DocType::PrivateMovement,
            DocType::Asset,
        ] {
            assert_eq!(next_seq(&pool, dt, 2026).await.unwrap(), 1);
        }
    }
}
