# §33 UStDV — Kleinbetragsrechnung

Quelle: https://www.gesetze-im-internet.de/ustdv_1980/__33.html

Rechnungen mit Bruttobetrag **≤ 250 €** dürfen vereinfachte
Pflichtangaben enthalten:

1. Name + Anschrift des Leistenden.
2. Ausstellungsdatum.
3. Menge + Bezeichnung der Leistung.
4. Entgelt + Steuerbetrag in einer Summe (oder bei §19: Hinweis statt
   Steuersatz).
5. Steuersatz oder Hinweis auf Steuerbefreiung.

**Nicht** erforderlich (für Kleinbetragsrechnungen):
- Fortlaufende Rechnungsnummer.
- Steuernummer/USt-IdNr.
- Anschrift des Empfängers.

Klein.Buch erkennt das über `domain::kleinbetragsrechnung::is_applicable(brutto)` und prüft dann
milder (Empfängeranschrift und Steuernummer entfallen; die Rechnungsnummer vergibt Klein.Buch
trotzdem fortlaufend, GoBD). Auch hier gilt:
bei `is_kleinunternehmer = true` → §19-Klausel obligatorisch.
