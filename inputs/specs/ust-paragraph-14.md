# §14 UStG — Pflichtangaben einer Rechnung

§14 Abs. 4 UStG definiert die Pflichtangaben. Klein.Buch's
`domain::invoice::validate()` (Block 3) prüft live, bevor eine
Rechnung in den `issued`-Status übergeht.

Quelle: https://www.gesetze-im-internet.de/ustg_1980/__14.html

Pflichtangaben:
1. Vollständiger Name + Anschrift Leistender + Leistungsempfänger.
2. Steuernummer oder USt-IdNr des Leistenden.
3. Ausstellungsdatum.
4. Fortlaufende Rechnungsnummer (RE-{YYYY}-{NNNN}).
5. Menge + handelsübliche Bezeichnung der Leistung.
6. Zeitpunkt der Lieferung/Leistung (Leistungsdatum).
7. Entgelt + jeden im Voraus vereinbarten Rabatt.
8. Anzuwendender Steuersatz, ggf. Hinweis Steuerbefreiung.
9. Bei §19: Hinweis "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."

Kleinbetragsrechnungen (≤ 250 € brutto, §33 UStDV) haben reduzierte
Pflichtangaben — siehe `ust-paragraph-33-kleinbetrag.md`.
