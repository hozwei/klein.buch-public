# §13b UStG — Reverse-Charge

Quelle: https://www.gesetze-im-internet.de/ustg_1980/__13b.html

Steuerschuldnerschaft des Leistungsempfängers — der Empfänger
führt die Umsatzsteuer ab statt des Leistenden. Relevant u.a. bei:

- Bauleistungen zwischen Bauunternehmen.
- Auslandsdienstleistungen aus dem EU-Ausland (Reverse-Charge auf
  Empfängerseite).
- Lieferungen von Edelmetallen, Mobilfunkgeräten ab bestimmten Schwellen.

**Für §19-Kleinunternehmer:** Reverse-Charge auf der Eingangsseite ist
relevant (Empfang einer §13b-Rechnung). Auf der Ausgangsseite tritt
§13b für Kleinunternehmer selten auf — sie schulden keine USt.

Klein.Buch (Phase 2B, Block 9): Eingangs-Rechnungen mit Reverse-Charge
bekommen ein Checkbox-Flag `is_reverse_charge`. EÜR-Aggregation
berücksichtigt diese korrekt — der Empfänger hat zwar USt zu erheben
(bei Regelbesteuerung), aber bei §19-Status ist auch das gegenstandslos.
