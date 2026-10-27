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

## Amendment 2026-10-24 — Schema-Prüfung im Server-Betrieb (Lauf F, ADR 0040)

**Status:** Akzeptiert. Präzisiert den Satz „`EXPECTED_SCHEMA_VERSION`-Check beim
Start; App startet bei Mismatch nicht“ für den Server-Betrieb (ADR 0039): Der
Server startet immer gesperrt (E3). Geprüft wird beim **Entsperren**, bevor
Pool, Passphrase und Scheduler eingehängt werden (`db::run_migrations`: zuerst
der Vergleich der gespeicherten Schema-Version, erst dann der Migrations-Runner
und `schema_version::check_compatible`). Ist die Datenbank neuer als das
Programm, scheitert das Entsperren mit dem Hinweis auf das Image der passenden
Version, und die Instanz bleibt gesperrt (Down-Migration-Schutz; Test
`db::tests::newer_database_is_rejected_on_unlock`).
Ist sie älter, entsteht zuerst das Pflicht-Backup, dann wird vorwärts migriert
(ADR 0039 E15, D-R8-19). PRD-Revision 9 löst die PRD-Stellen (G13, D-49, §6.1) entsprechend
ab.

## Amendment 2026-10-25 — Rechenregeln für Cent-Beträge (Lauf F, Block F5a)

Gerechnet wird in `i64` (`domain::money`); `Menge × Einzelpreis` und
`Netto × Satz` bleiben als dokumentierte Ausnahme `f64` mit einmaliger
kaufmännischer Rundung (die Menge ist kein Geld, höchstens 4 Nachkommastellen).
Die USt eines Belegs entsteht je (Steuerart, Satz) aus der Nettosumme (EN 16931
BR-CO-17), AfA und betrieblicher Anteil sind ganzzahlig (i128, half-up).
Domänengrenzen (Menge, Einzelpreis, Satz, Summen) und `overflow-checks` im
Release-Profil des eigenen Crates schließen stille Überläufe aus. Einzelheiten:
ADR 0039 Nr. 134. Nachtrag (Lauf F5-fix): Dieselbe Obergrenze von 1 Mrd. € gilt
für jeden einzeln erfassten Betrag (Kosten, Zahlung, Anschaffung, Abgangserlös,
Privatbewegung, Kosten-Abo; `money::MAX_AMOUNT_CENTS`, ADR 0039 Nr. 137).
