# ADR 0018 — Rechtsdokumente (AGB/Datenschutz): Versionierung + Angebots-Bundle

**Status:** Akzeptiert · 2026-05-20 · Block 8.

## Kontext

Rechtlich muss nachweisbar bleiben, welche AGB-/Datenschutz-Fassung ein Kunde zum
Angebotszeitpunkt erhielt. Angebote werden immer als **Bundle** (Angebot + AGB +
Datenschutz) ausgegeben — per Druck oder Mail. Spannung: Angebote sind ab `sent`
GoBD-gelockt (ADR 0016), die Verknüpfung darf also kein mutables Quote-Kernfeld
sein. `inputs/` ist nach Block 1 für Maschinen tabu.

## Entscheidung

- **PDF-Upload pro Version** (kein In-App-Editor): `legal_documents` (doc_type
  `agb`/`privacy`, `version` monoton pro Typ, `title`, `archive_entry_id`,
  `is_active`). PDF write-once archiviert (`ArchiveKind::LegalDocument`).
  **Append-only + immutable** (DB-Trigger, kein Löschen, Kernfelder fix); partial
  unique `uq_legal_documents_active` = höchstens eine aktive Version pro Typ.
- **Bindung als eigene append-only Tabelle** `quote_legal_documents`
  (`version`-Snapshot, unique pro (Angebot, doc_type)), gesetzt bei der
  **Bundle-/Versand-Erzeugung** (`prepare_quote_dispatch`), **idempotent** —
  bestehende Bindung bleibt, auch wenn später eine neue Version aktiv wird.
  **Pflicht für Versand**: aktive AGB UND Datenschutz müssen existieren.
- **Angebots-PDF = Plain-PDF** (kein PDF/A-3b — keine E-Rechnung, kein Mustang);
  gemeinsamer `compile_pdf`-Pfad mit der Rechnung. Quote-Template **eingebettet**
  im Binary (mit §19-Marker, besteht `klausel_check`), Override via
  `inputs/pdf-templates/quote.typ` — so bleibt `inputs/` maschinen-tabu, das
  Template wird trotzdem mitgeliefert. Selbe Logik für den Quote-Mailtext.
- **Ausgabe-Form**: Druck = ein zusammengeführtes PDF (`pdf::bundle::merge_pdfs`,
  neue Dep `lopdf 0.40`, Rezept gegen `examples/merge.rs` verifiziert); Mail =
  drei separate Anhänge (Block-5-Multi-Attachment). Das Merge-PDF wird **nicht**
  archiviert (abgeleitet); kanonisch sind Einzel-PDFs + Bindung.

## Konsequenzen

- Lückenloser Nachweis „welche Fassung ging an welches Angebot" ohne die
  GoBD-Lock-Regel für Angebote zu verletzen.
- Neue Cargo-Dependency `lopdf` (PDF-Manipulation). Das Merge-Ergebnis ist kein
  PDF/A — akzeptabel, da reines Druck-Convenience.
- Juristische Bewertung der Texte bleibt extern (anwaltlich) — Klein.Buch liefert
  nur die Mechanik.

## Alternativen

| Option | Contra |
|---|---|
| Legal-Texte im App-Editor → PDF rendern | mehr Aufwand; eingefrorene Upload-Bytes sind der stärkere Nachweis |
| Bindung als Quote-Spalte | mutables Kernfeld auf gelocktem Angebot → GoBD-Verletzung |
| Bundle als ein Typst-Dokument | geht nicht: Legal-Docs sind beliebige Upload-PDFs |
| Quote-Template nach `inputs/` schreiben | verletzt `inputs/`-tabu-Hardline |
| Merged PDF auch für Mail | Manuel will Mail als 3 Anhänge (maschinell trennbar) |

## Referenzen

`db::repo::legal_documents`, `pdf::{bundle,templates,typst_render}`,
`commands::{legal_documents,quotes,mail}`, `0006_legal_documents.sql`;
`memory/klein-buch/block-8-notes.md`.
