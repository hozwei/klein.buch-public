# §14 UStG — Pflichtangaben einer Rechnung

Quellen: https://www.gesetze-im-internet.de/ustg_1980/__14.html ·
https://www.gesetze-im-internet.de/ustdv_1980/__31.html (§ 31 UStDV) ·
https://www.gesetze-im-internet.de/ustdv_1980/__34a.html (§ 34a UStDV)

Klein.Buch prüft die Angaben vor dem Ausstellen (`domain::invoice::validate_for_issue`).

**§ 14 Abs. 4 UStG (Regelfall):**
1. Vollständiger Name und Anschrift von Leistendem und Leistungsempfänger.
2. Steuernummer oder USt-IdNr. des Leistenden.
3. Ausstellungsdatum.
4. Fortlaufende Rechnungsnummer (Klein.Buch: `RE-{JJJJ}-{NNNN}`).
5. Menge und handelsübliche Bezeichnung der Leistung.
6. Zeitpunkt der Leistung — als Datum oder Zeitraum; der Kalendermonat genügt
   (§ 31 Abs. 4 UStDV). Bei einer Rechnung vor der Leistung (Vorausrechnung, Anzahlung) der
   Zeitpunkt der Vereinnahmung, sofern er feststeht und vom Ausstellungsdatum abweicht.
7. Entgelt und jede im Voraus vereinbarte Minderung.
8. Steuersatz und Steuerbetrag bzw. Hinweis auf eine Steuerbefreiung.

**Kleinunternehmer (§ 34a UStDV, seit 2025):** Mindestangaben sind Name und Anschrift von
Leistendem und Empfänger, Steuernummer bzw. USt-IdNr. des Leistenden, Ausstellungsdatum, Menge
und Art der Leistung, Entgelt sowie der Hinweis auf die Steuerbefreiung nach § 19 UStG.
Klein.Buch verlangt trotzdem Rechnungsnummer und Leistungszeitpunkt (GoBD, E-Rechnung).

**Klein.Buch:** Leistungsdatum oder Leistungszeitraum (XRechnung BT-72 bzw. BG-14 mit
BT-73/BT-74); ohne Angabe gilt das Rechnungsdatum. Das Leistungsdatum darf nach dem
Rechnungsdatum liegen (ADR 0042 Nr. 10, 11). Leitweg-ID: BT-10.

Kleinbetragsrechnungen (≤ 250 € brutto, § 33 UStDV) haben weniger Pflichtangaben —
siehe `ust-paragraph-33-kleinbetrag.md`.
