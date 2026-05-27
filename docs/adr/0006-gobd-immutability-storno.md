# ADR 0006 — GoBD: Immutability via DB-Trigger, Storno statt Löschung

**Status:** Akzeptiert · 2026-05-19 · Block 3. (Decision-Log D-11, D-22)

## Kontext

GoBD (§147 AO) verlangt Unveränderbarkeit, Nachvollziehbarkeit und 10 Jahre
Aufbewahrung. Eine fehlerhafte, bereits festgeschriebene Rechnung darf nicht
editiert oder gelöscht werden.

## Entscheidung

- **DB-Trigger erzwingen Immutability**: nach `locked_at` sind Kernfelder
  (Nummer, Datum, Beträge, Kontakt, GJ, §19-Flag, Direction) per
  `trg_invoices_immutable` gesperrt. `status`/`sent_at`/Zahlungsfelder bleiben
  änderbar (Versand, Zahlungseingang).
- **Storno = neuer Beleg** (`ST-{YYYY}-{NNNN}`, `is_storno_for`), Original wird
  `status='canceled'` markiert, nie gelöscht.
- **Archive write-once**: SHA-256 beim Schreiben, Re-Hash beim Lesen
  (Tamper-Detection), Datei read-only.
- **Audit-Log append-only** (Trigger gegen Update/Delete).
- **Kein Löschen-UI**; Soft-Delete (`archived`) nur für Stammdaten.

## Konsequenzen

- Korrekturen laufen ausschließlich über Storno + Neurechnung.
- Datenmenge wächst monoton (10-Jahre-Aufbewahrung); akzeptabel für Single-User.
- DSGVO-Löschwünsche werden später (Block 19) über Anonymisierung statt Löschung
  gelöst, im Spannungsfeld zur AO-Aufbewahrung (juristisch zu prüfen).

## Referenzen

GoBD, §147 AO; `0001_init.sql` (Trigger).
