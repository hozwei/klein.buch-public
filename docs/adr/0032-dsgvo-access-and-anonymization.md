# ADR 0032 — DSGVO: Auskunft (Art. 15) + Anonymisierung (Art. 17)

**Status:** Akzeptiert · 2026-05-24 · Blöcke 18/19. Migrationen `0022_contact_anonymization`, `0023_quote_buyer_snapshot`.

## Kontext

Betroffene können nach DSGVO **Auskunft** (Art. 15) über ihre gespeicherten Daten
und **Löschung** (Art. 17) verlangen. Das kollidiert mit der GoBD-Hardline:
ausgestellte Belege müssen **10 Jahre unverändert** aufbewahrt werden — die
darin enthaltenen Empfänger-Daten dürfen also **nicht** gelöscht oder verändert
werden. Beide Pflichten müssen gleichzeitig erfüllt sein.

## Entscheidung

1. **Art. 15 Auskunft = read-only Export.** `dsgvo.export` sammelt alle Daten zu
   einem Kontakt und schreibt ein **Komplett-ZIP** (`auskunft.pdf` lesbar +
   `auskunft.json` maschinenlesbar inkl. SHA-256 + `dokumente/` mit den
   archivierten Originalen + `LIESMICH.txt`). **Keine** Beleg-Mutation, **genau
   ein** Audit-Eintrag.
2. **Art. 17 = Anonymisierung statt Löschung.** Die Kontakt-**Stammdaten** werden
   überschrieben (`name = 'Anonymisiert #<8hex>'`, übrige PII auf NULL,
   `anonymized_at` gesetzt). Eine echte Löschung der Zeile/Belege findet **nicht**
   statt.
3. **Buyer-Snapshots bleiben erhalten.** Die beim Festschreiben in Rechnung/Angebot
   **eingefrorenen** Empfänger-Snapshots (`buyer_*`) werden **nicht** anonymisiert —
   für ausgestellte Belege gewinnt GoBD. Der Schutz liegt im **Repo-Guard**, nicht
   im Immutability-Trigger (der Trigger deckt `buyer_*` bewusst nicht ab; die
   Spannung GoBD↔DSGVO ist damit explizit und nachvollziehbar aufgelöst).
4. **Guard: keine offenen Entwürfe.** Anonymisierung wird abgelehnt, solange ein
   nicht-festgeschriebener (änderbarer) Beleg auf den Kontakt zeigt — sonst stünde
   der anonymisierte Name in einem noch editierbaren Dokument. Zweite
   Anonymisierung wird abgelehnt (kein zweiter Audit-Eintrag).

## Konsequenzen

- DSGVO-Auskunfts- und Löschpflicht **und** GoBD-Aufbewahrung sind gleichzeitig
  erfüllt: lebende Stammdaten verschwinden, der historische Beleg bleibt intakt.
- Anonymisierung ist **einmalig + irreversibel** (kein Rück-Schlüssel) → keine
  Re-Identifizierung über die App.
- Der Snapshot-Schutz im Repo (statt Trigger) ist eine bewusste Ausnahme von der
  „Trigger erzwingt Immutability"-Regel und ist als solche dokumentiert/getestet.

## Alternativen

| Option | Contra |
|---|---|
| Echte Löschung (DELETE Kontakt + Beleg-Daten) | bricht GoBD (10-Jahres-Aufbewahrung, Storno-statt-Löschung) |
| Pseudonymisierung mit Rück-Schlüssel | DSGVO-fragwürdig (Re-Identifizierung bleibt möglich) |
| Buyer-Snapshot mit-anonymisieren | verändert den ausgestellten Beleg → GoBD-Verstoß |
| Auskunft als Live-DB-Dump statt Snapshot-ZIP | nicht reproduzierbar, kein Integritätsnachweis (SHA-256) |

## Referenzen

`domain::dsgvo`, `domain::anonymize`, `commands` (dsgvo export/anonymize),
`db::repo::dsgvo` (`gather`), Migrationen `0022`/`0023`; Tests
`dsgvo_export_test.rs`, `contact_anonymize_test.rs`, `quotes_repo_test.rs`
(Buyer-Snapshot übersteht Anonymisierung); ADR 0006 (GoBD), ADR 0005 (§19).
Review-Befunde KB-0048/0049.
