# ADR 0017 — Angebot → Rechnung: Konvertierung nur aus `accepted`, erzeugt Draft

**Status:** Akzeptiert · 2026-05-20 · Block 7.

## Kontext

Aus einem Angebot soll eine Rechnung werden. Offene Fragen: Aus welchem Status
ist das zulässig? Entsteht direkt eine festgeschriebene Rechnung oder eine Draft?
Wie bleibt der Belegnummern-/Snapshot-/Audit-Code DRY?

## Entscheidung

- **Konvertierung nur aus `status='accepted'`** (Hard-Rule Manuel): erst das
  angenommene/unterschriebene Angebot wird zur Rechnung. Andere Status (inkl.
  bereits `converted`) werden abgelehnt — blockt auch Doppel-Konvertierung.
- **Erzeugt eine Rechnungs-Draft**, keine festgeschriebene Rechnung. Das
  Festschreiben läuft danach über die **eine** Lock-Pipeline
  (`invoices_lock_and_issue`, ADR 0007/0008) — kein Issue-Duplikat, §19/§14/KoSIT
  greifen unverändert. Positionen sind im Konvertierungs-Schritt anpassbar.
- **DRY-Helper** `commands::invoices::create_invoice_draft_from_input`, genutzt von
  direkter Neuanlage UND Konvertierung; setzt `derived_from_quote_id`.
  `domain::quote::convert_to_invoice` ist pure (1:1-Item-Mapping inkl. §19-E/0).
  `quotes::mark_converted` (Guard `accepted → converted`) ist durch
  `trg_quotes_immutable` erlaubt (status/converted_* nicht in der Kernfeld-Liste).

## Konsequenzen

- Rückverfolgbarkeit: Rechnung kennt ihr Ursprungsangebot
  (`invoices.derived_from_quote_id`), Angebot kennt seine Rechnung
  (`converted_invoice_id`).
- Keine separate Draft-Invoice-Edit-Route nötig — Anpassungen passieren im
  Convert-Schritt.

## Alternativen

| Option | Contra |
|---|---|
| Direkt festgeschriebene Rechnung erzeugen | dupliziert die Lock-Pipeline, umgeht Validierung |
| Konvertierung aus `sent` erlauben | Rechnung ohne Kundenzustimmung — fachlich falsch |
| Eigener Convert-Code statt Helper | Drift zur Neuanlage (Snapshots/Audit/Nummer) |

## Referenzen

`domain::quote::convert_to_invoice`, `commands::quotes::quotes_convert_to_invoice`,
`create_invoice_draft_from_input`; `memory/klein-buch/block-7-notes.md`.
