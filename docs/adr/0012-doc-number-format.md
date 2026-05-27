# ADR 0012 — Doc-Number-Format `{TYP}-{YYYY}-{NNNN}` pro Geschäftsjahr

**Status:** Akzeptiert · 2026-05-19 · Block 3. (Decision-Log D-18)

## Kontext

§14 UStG verlangt eindeutige, fortlaufende Rechnungsnummern. Bei einem Solo-
Unternehmer soll die Nummer lesbar sein und pro Jahr + Belegart lückenlos laufen.
Nebenläufige Vergabe darf keine Lücken/Duplikate erzeugen.

## Entscheidung

- Format **`{TYP}-{YYYY}-{NNNN}`** (z. B. `RE-2026-0001`, Storno `ST-2026-0001`),
  Sequenz pro **Geschäftsjahr und Typ**.
- Vergabe atomar in `db::numbering` via `INSERT OR IGNORE` + `UPDATE … RETURNING`
  in einer Transaktion (getestet mit 50 parallelen Allokationen ohne Lücken/Dupes).
- Drafts verbrauchen bereits eine Nummer (gemeinsame Sequenz mit gelockten Belegen);
  ein gelöschter Draft hinterlässt eine GoBD-dokumentierbare Lücke.

## Konsequenzen

- Nummern sind menschenlesbar und jahresbezogen interpretierbar.
- Lückenlosigkeit ist nebenläufigkeitssicher garantiert.
- Beim GJ-Wechsel werden Counter neu initialisiert (Phase 2D `fiscal_year::transition`).

## Referenzen

§14 UStG; `db::numbering`, `domain::numbering::DocType`.
