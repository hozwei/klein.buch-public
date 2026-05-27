//! Tabellen-strukturierter JSON-Dump (Block 4).
//!
//! Liest jede Anwendungstabelle dynamisch (`SELECT *`) und serialisiert sie als
//! JSON-Array von Objekten. Spaltentypen werden über `type_info()` der STRICT-
//! Tabellen gemappt (TEXT→string, INTEGER→number, REAL→number, BLOB→hex).
//!
//! Dient dem **Vendor-Lock-in-Schutz**: der komplette Datenbestand bleibt auch
//! ohne Klein.Buch les- und migrierbar.

use crate::error::{Error, Result};
use serde_json::{Map, Value};
use sqlx::sqlite::SqliteConnection;
use sqlx::{Column, Row, SqlitePool, ValueRef};

/// Listet alle Anwendungstabellen (ohne interne `sqlite_*`), alphabetisch.
pub async fn list_tables(pool: &SqlitePool) -> Result<Vec<String>> {
    let mut conn = pool.acquire().await?;
    list_tables_in_conn(&mut conn).await
}

/// Zeilenzahl einer Tabelle.
pub async fn row_count(pool: &SqlitePool, table: &str) -> Result<i64> {
    let mut conn = pool.acquire().await?;
    row_count_in_conn(&mut conn, table).await
}

/// Dumped eine Tabelle als JSON-Array von Objekten (`serde_json::Value`).
pub async fn dump_table_value(pool: &SqlitePool, table: &str) -> Result<Value> {
    let mut conn = pool.acquire().await?;
    dump_table_value_in_conn(&mut conn, table).await
}

/// Dumped eine Tabelle als hübsch formatierten JSON-String.
pub async fn dump_table_json(pool: &SqlitePool, table: &str) -> Result<String> {
    let v = dump_table_value(pool, table).await?;
    serde_json::to_string_pretty(&v).map_err(Error::from)
}

/// Liefert die `CREATE`-Statements aller Tabellen (für die ERD-Doku).
pub async fn create_statements(pool: &SqlitePool) -> Result<Vec<(String, String)>> {
    let mut conn = pool.acquire().await?;
    create_statements_in_conn(&mut conn).await
}

// ---------------------------------------------------------------------------
// R4-006: Connection-Varianten für TX-Snapshot
// ---------------------------------------------------------------------------
//
// Diese Funktionen nehmen eine vorhandene `&mut SqliteConnection`, damit der
// Migration-Export-Pfad alle Tabellen-Dumps + ERD-Lookups innerhalb einer
// **einzigen** `BEGIN IMMEDIATE`-Transaktion durchziehen kann. Damit ist der
// Export ein konsistenter Point-in-Time-Snapshot (kein "Tabelle A ist vor
// Tabelle B beim Insert", was bei großer DB unter Last echte Inkonsistenz
// erzeugen würde).

/// Listet alle Anwendungstabellen über eine bestehende Verbindung.
pub async fn list_tables_in_conn(conn: &mut SqliteConnection) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("name"))
        .collect())
}

/// Zeilenzahl einer Tabelle über eine bestehende Verbindung.
pub async fn row_count_in_conn(conn: &mut SqliteConnection, table: &str) -> Result<i64> {
    assert_safe_identifier(table)?;
    let row = sqlx::query(&format!("SELECT COUNT(*) AS n FROM \"{table}\""))
        .fetch_one(&mut *conn)
        .await?;
    Ok(row.get::<i64, _>("n"))
}

/// Dumped eine Tabelle als JSON-Array über eine bestehende Verbindung.
pub async fn dump_table_value_in_conn(conn: &mut SqliteConnection, table: &str) -> Result<Value> {
    assert_safe_identifier(table)?;
    let rows = sqlx::query(&format!("SELECT * FROM \"{table}\""))
        .fetch_all(&mut *conn)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut obj = Map::new();
        for col in row.columns() {
            let name = col.name().to_string();
            let ord = col.ordinal();
            let raw = row.try_get_raw(ord)?;
            let value = if raw.is_null() {
                Value::Null
            } else if let Ok(i) = row.try_get::<i64, _>(ord) {
                Value::from(i)
            } else if let Ok(f) = row.try_get::<f64, _>(ord) {
                Value::from(f)
            } else if let Ok(s) = row.try_get::<String, _>(ord) {
                Value::String(s)
            } else if let Ok(b) = row.try_get::<Vec<u8>, _>(ord) {
                Value::String(format!("hex:{}", crate::backup::manifest::to_hex(&b)))
            } else {
                Value::Null
            };
            obj.insert(name, value);
        }
        out.push(Value::Object(obj));
    }
    Ok(Value::Array(out))
}

/// Dumped eine Tabelle als JSON-String über eine bestehende Verbindung.
pub async fn dump_table_json_in_conn(conn: &mut SqliteConnection, table: &str) -> Result<String> {
    let v = dump_table_value_in_conn(conn, table).await?;
    serde_json::to_string_pretty(&v).map_err(Error::from)
}

/// `CREATE`-Statements aller Tabellen über eine bestehende Verbindung.
pub async fn create_statements_in_conn(
    conn: &mut SqliteConnection,
) -> Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT name, sql FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND sql IS NOT NULL
         ORDER BY name",
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get::<String, _>("name"), r.get::<String, _>("sql")))
        .collect())
}

/// Liefert den in `archive_entries` für `file_path` gespeicherten SHA-256-Hash
/// — wenn ein Eintrag existiert. Sonst `None`. Genutzt vom Migration-Export
/// zur Tamper-Detektion (R4-004): die Datei wird gelesen, neu gehasht und
/// gegen den Soll-Hash verglichen.
pub async fn archive_hash_for_path(
    conn: &mut SqliteConnection,
    file_path: &str,
) -> Result<Option<String>> {
    let v: Option<String> =
        sqlx::query_scalar("SELECT file_hash_sha256 FROM archive_entries WHERE file_path = ?")
            .bind(file_path)
            .fetch_optional(&mut *conn)
            .await?;
    Ok(v)
}

fn assert_safe_identifier(ident: &str) -> Result<()> {
    if ident.is_empty() || !ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::Backup(format!("unsicherer Tabellenname: {ident}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn demo_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE contacts (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL,
                amount_cents INTEGER NOT NULL, rate REAL, note TEXT) STRICT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO contacts (id, name, amount_cents, rate, note)
             VALUES ('c1','Müller GmbH', 12345, 1.5, NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn lists_tables_and_counts() {
        let pool = demo_pool().await;
        let tables = list_tables(&pool).await.unwrap();
        assert_eq!(tables, vec!["contacts".to_string()]);
        assert_eq!(row_count(&pool, "contacts").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn dumps_typed_values() {
        let pool = demo_pool().await;
        let v = dump_table_value(&pool, "contacts").await.unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let row = &arr[0];
        assert_eq!(row["id"], Value::from("c1"));
        assert_eq!(row["name"], Value::from("Müller GmbH"));
        assert_eq!(row["amount_cents"], Value::from(12345_i64));
        assert_eq!(row["rate"], Value::from(1.5_f64));
        assert_eq!(row["note"], Value::Null);
    }

    #[tokio::test]
    async fn rejects_bad_identifier() {
        let pool = demo_pool().await;
        assert!(row_count(&pool, "contacts; DROP TABLE x").await.is_err());
    }
}
