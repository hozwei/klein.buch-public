# §19 UStG — Kleinunternehmerregelung

Quelle: https://www.gesetze-im-internet.de/ustg_1980/__19.html

Stand 2026: Vorjahresumsatz max. **25.000 €**, laufendes Jahr max.
**100.000 €** (Wachstumschancengesetz). Bei Überschreitung im laufenden
Jahr → ab dem Umsatz, der die Grenze überschreitet, Regelbesteuerung.
Kein rückwirkender Wechsel innerhalb des Jahres.

**Konsequenzen:**
- Keine Umsatzsteuer auf Ausgangsrechnungen.
- Kein Vorsteuerabzug aus Eingangsrechnungen.
- Pflichthinweis auf Rechnungen: "Gemäß §19 UStG wird keine Umsatzsteuer
  ausgewiesen."

**Verzicht (§19 Abs. 2 UStG):** freiwilliger Übergang zur Regelbesteuerung
ist möglich. Bindungsfrist **5 Jahre**. Klein.Buch's Settings-Toggle
zeigt einen Warn-Dialog bei Aktivierung und speichert
`seller_profile.waived_paragraph_19_since` als Stichtag.

**§14c-Schutz:** Wer als Kleinunternehmer fälschlich USt ausweist,
schuldet die ausgewiesene Steuer (§14c Abs. 2 UStG). Klein.Buch
verhindert das in der UI aktiv: bei `is_kleinunternehmer = true` sind
alle USt-Felder gesperrt und `tax_amount` ist fest 0.
