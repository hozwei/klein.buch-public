# ADR 0016 — Angebote: eigener Belegkreis, Festschreiben = Lock → `sent`

**Status:** Akzeptiert · 2026-05-20 · Block 6.

## Kontext

Angebote (`AN-{YYYY}-{NNNN}`) sind keine E-Rechnungen, brauchen aber denselben
GoBD-Schutz wie Rechnungen, sobald sie raus sind. Außerdem soll der Annahme-
Workflow den unterschriebenen Vertrag revisionssicher festhalten. Das Frontend
läuft in WebView2, wo `window.prompt`/`confirm` unzuverlässig sind.

## Entscheidung

- **Eigene Tabellen** `quotes` + `quote_items` (Migration `0005_quotes.sql`),
  eigener Counter `quote`. `domain::quote` ist Functional Core und
  **wiederverwendet** `invoice::{Totals, compute_totals}` + den §19-`assert_no_vat`.
- **Festschreiben = Lock → Status `sent`** (kein separater `issued`-Status für
  Angebote). Ab `locked_at` greift `trg_quotes_immutable` auf den Kernfeldern;
  erlaubt bleiben State-Transitions (sent/accepted/rejected/converted/canceled,
  `pdf_archive_id`, `notes`).
- **Vertrag als echter Datei-Upload**: Bytes gehen per IPC ans Backend und werden
  über `archive::store_bytes` write-once abgelegt + als `attachments`-Eintrag
  (`parent_type='quote'`) verknüpft. Kein Datei-Dialog-Plugin/Capability nötig.
- **`window.prompt`-Verzicht** → Inline-Panels (Annahme/Ablehnung/Storno) im
  Svelte-UI.
- **`quotes.seller_tax_number` nullbar** (konsistent mit `seller_profile` /
  `invoices`; Onboarding ohne Steuernummer + §33-Kleinbetrag).

## Konsequenzen

- Angebote durchlaufen NICHT KoSIT/Mustang (kommen erst bei der Rechnung zum
  Tragen, Block 7/3).
- Storno statt Löschung gilt auch für Angebote (`canceled` + Grund).
- Die Verknüpfung an Rechtsdokumente kommt in Block 8 als eigene append-only
  Tabelle (ADR 0018), nicht als Quote-Kernfeld — eben weil Angebote ab `sent`
  gelockt sind.

## Alternativen

| Option | Contra |
|---|---|
| Angebote als „Rechnung mit Flag" | vermischt E-Rechnungs-Pflichten + Belegkreise |
| Vertrag nur als Datei-Pfad referenzieren | nicht revisionssicher, Pfad kann brechen |
| `window.prompt` für Gründe | in WebView2 unzuverlässig/blockierend |

## Referenzen

`domain::quote`, `db::repo::quotes`, `db::repo::attachments`,
`commands::quotes`, `0005_quotes.sql`; `memory/klein-buch/block-6-notes.md`.
