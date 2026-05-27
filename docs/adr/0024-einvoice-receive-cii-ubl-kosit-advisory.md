# ADR 0024 — E-Rechnung-Empfang: CII+UBL-Parser, KoSIT beratend, Import als Kosten

**Status:** Akzeptiert · 2026-05-21 · Block 11. Keine Migration (Schema v9).

## Kontext

Seit 01.01.2025 gilt die **E-Rechnung-Empfangspflicht** (§14 UStG, BMF
15.10.2024): inländische B2B-Empfänger müssen strukturierte E-Rechnungen
annehmen können. Eingehende **XRechnung** (zwei Syntaxen: UN/CEFACT **CII** und
OASIS **UBL**) und **ZUGFeRD** (PDF/A-3 mit eingebettetem CII-XML) müssen
gelesen, geprüft, archiviert und als Kosten erfassbar sein. Anders als beim
**Ausstellen** (dort ist der KoSIT-Validator ein Hard-Block, weil wir nichts
Fehlerhaftes verschicken dürfen) gilt beim **Empfang**: eine Rechnung, die der
Lieferant uns rechtlich wirksam geschickt hat, müssen wir verbuchen können —
auch wenn ihr XML einen KoSIT-Regelverstoß hat.

## Entscheidung

1. **`einvoice::parser`** liest beide XRechnung-Syntaxen (**CII + UBL**) und
   extrahiert das eingebettete CII-XML aus ZUGFeRD-PDFs.
2. **KoSIT-Validierung ist beim Empfang beratend, nie blockierend** — das
   Ergebnis wird angezeigt, verhindert aber den Import nicht.
3. **Import → normale Kostenposition** (`paid_date = None`, Default-Kategorie
   `other`): die empfangene Rechnung wird zu einer regulären Ausgabe, die der
   Mensch prüft, kategorisiert und bei Zahlung mit `paid_date` versieht.
4. **Original = Beleg:** die Originaldatei (XML bzw. ZUGFeRD-PDF) wird
   write-once als `ReceivedEinvoice` archiviert (GoBD: Aufbewahrung im
   Originalformat, unverändert).

## Konsequenzen

- Empfang funktioniert **immer**, unabhängig vom KoSIT-Urteil — die gesetzliche
  Annahmepflicht wird erfüllt, ohne dass Lieferanten-Fehler den Workflow
  blockieren.
- Das archivierte Original ist der prüfungsfeste Beleg; die DB-Kostenposition
  ist die auswertbare Projektion davon.
- Die Kostenposition fließt nach Zahlung über die normale EÜR-Cash-Basis ein
  (ADR 0022) — kein Sonderpfad.

## Alternativen

| Option | Contra |
|---|---|
| Import bei KoSIT-Fail blockieren | verhindert das Verbuchen einer rechtlich wirksam empfangenen Rechnung — fachlich falsch |
| Eigene Empfangs-Tabelle statt Kosten | dupliziert die Ausgaben-Logik (EÜR, DATEV, Festschreibung) ohne Mehrwert |
| Nur ZUGFeRD, kein reines XRechnung-XML | reine XRechnung (CII/UBL ohne PDF) ist zulässig und kommt vor → muss unterstützt werden |

## Referenzen

`einvoice::parser` (CII + UBL + ZUGFeRD-Extract), Import-Mapping in
`commands`/`db::repo::expenses`, `archive` (`ReceivedEinvoice`), Frontend
`routes/expenses/import`; ADR 0007 (CII statt UBL beim **Ausstellen**),
ADR 0019 (Kosten), ADR 0022 (Cash-Basis). Commit `block-11` (`1218eb07`),
Tag `v0.1.0-phase2b`.
