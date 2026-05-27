# ADR 0010 — EÜR auf Cash-Basis (§4 Abs. 3 EStG)

**Status:** Akzeptiert · 2026-05-19 · Block 1 (Planung), umgesetzt Phase 2C.
(Decision-Log D-12) · **Storno-Behandlung korrigiert durch [ADR 0022](0022-euer-income-recognition-zufluss-abfluss.md) (Block 13).**

> **Korrektur (Block 13):** Die unten genannte Annahme, Stornos verrechneten sich
> über einen negativen `paid_amount` selbst, ist im gebauten Code unmöglich
> (`record_payment` verlangt `amount>0` + verbietet Überzahlung ⇒ ein Storno-Beleg
> trägt nie eine Zahlung). Maßgeblich für die Storno-Wirkung in der EÜR ist
> **ADR 0022**: Storno = negative Einnahme zum Storno-Datum (§11 EStG). Die übrige
> Cash-Basis-Entscheidung dieses ADR bleibt gültig.

## Kontext

§19-Kleinunternehmer ermitteln den Gewinn per Einnahmen-Überschuss-Rechnung, nicht
per Bilanz. Maßgeblich ist das **Zufluss-/Abfluss-Prinzip** (Cash-Basis), nicht die
Rechnungsstellung.

## Entscheidung

- **EÜR strikt Cash-Basis**: Einnahmen am Zahlungsdatum (`invoices.paid_at`),
  Ausgaben am Zahlungsausgang (`expenses.paid_date`).
- Stornos und Privatentnahmen werden aus der EÜR ausgeklammert; AfA wird pro
  Geschäftsjahr aggregiert (eigene Buchungen).
- Geschäftsjahr = Kalenderjahr (v0.1 fix); `fiscal_year` auf allen
  Bewegungstabellen, Doc-Nummern pro GJ (ADR 0012).

## Konsequenzen

- Zahlungserfassung (`record_payment`) ist die EÜR-relevante Aktion, nicht das
  Ausstellen — die UI macht das transparent.
- Offene Forderungen erscheinen nicht in der EÜR, bis sie bezahlt sind.
- AfA bricht mit der reinen Cash-Sicht (periodisierte Abschreibung) — bewusst,
  weil steuerlich vorgeschrieben.

## Referenzen

§4 Abs. 3 EStG; `euer::aggregate` (Phase 2C).
