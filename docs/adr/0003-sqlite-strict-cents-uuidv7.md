# ADR 0003 — SQLite (WAL + STRICT), Integer-Cents, UUIDv7

**Status:** Akzeptiert · 2026-05-19 · Block 1. (Decision-Log D-04, D-16, D-17)

## Kontext

Local-first Single-User braucht eine eingebettete, transaktionssichere DB. Geld
darf nie als Float repräsentiert werden (Rundungsdrift). Primärschlüssel sollen
ohne zentrale Sequenz vergebbar und zeitlich sortierbar sein.

## Entscheidung

- **SQLite** im **WAL**-Modus, alle Tabellen **`STRICT`** (Typ-Enforcement seit
  SQLite 3.37), `PRAGMA foreign_keys = ON`.
- **Geld als `INTEGER`-Cents (i64)** durchgängig — nie Float.
- **UUIDv7** als Primärschlüssel (zeit-sortierbar, dezentral vergebbar).
- **Forward-only Migrationen** mit `EXPECTED_SCHEMA_VERSION`-Check beim Start;
  App startet bei Mismatch nicht (Down-Migration-Schutz).

## Konsequenzen

- Determinismus bei Beträgen; Formatierung erst an der UI-Grenze.
- STRICT verhindert stille Typ-Coercion (z. B. Text in numerischer Spalte).
- UUIDv7 erlaubt sinnvolle Default-Sortierung nach Anlagezeit ohne extra Feld.
- Schema-Änderungen sind immer additiv/forward; ein Rückschritt erfordert
  Restore aus Backup (ADR 0009), nicht Down-Migration.

## Alternativen

| Option | Contra |
|---|---|
| Postgres | Server-Prozess, gegen local-first |
| Float/Decimal-Text für Geld | Drift bzw. Parsing-Overhead, fehleranfällig |
| Auto-Increment-PK | Nicht dezentral, verrät Mengen, schlecht für Merge/Backup |
