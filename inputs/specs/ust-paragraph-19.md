# §19 UStG — Kleinunternehmer (Fassung ab 1. Januar 2025)

Quellen: https://www.gesetze-im-internet.de/ustg_1980/__19.html ·
BMF-Schreiben vom 18.03.2025 (Abschn. 19.1 ff. UStAE) · § 34a UStDV

**Voraussetzungen (§ 19 Abs. 1 und 2):**
- Umsätze sind steuerfrei, wenn der Gesamtumsatz im Vorjahr **25.000 €** nicht überschritten hat
  und im laufenden Jahr **100.000 €** nicht überschreitet.
- Gesamtumsatz = Summe der steuerbaren Umsätze **nach vereinnahmten Entgelten** (netto), ohne
  Umsätze mit Wirtschaftsgütern des Anlagevermögens und bestimmte steuerfreie Umsätze.
- Gründungsjahr: Es gilt die Grenze von 25.000 € im laufenden Jahr (BMF).

**Wegfall:**
- Vorjahr über 25.000 €: Regelbesteuerung ab 1. Januar.
- Laufendes Jahr über 100.000 € (Gründungsjahr: über 25.000 €): Regelbesteuerung ab dem Umsatz,
  mit dem die Grenze überschritten wird; dieser Umsatz ist schon steuerpflichtig, die vorherigen
  bleiben steuerfrei.
- Kein Bindungszeitraum: Liegen die Voraussetzungen in einem späteren Jahr wieder vor, gilt
  § 19 ab dessen 1. Januar wieder.

**Verzicht (§ 19 Abs. 3):** Erklärung gegenüber dem Finanzamt bis Ende Februar des zweiten auf
das Jahr folgenden Kalenderjahres; wirkt ab Beginn des Jahres, für das er gilt; bindet
**fünf Kalenderjahre** einschließlich des Wirkungsjahres (Verzicht ab 2026 → Rückkehr frühestens
zum 01.01.2031); Widerruf nur mit Wirkung vom Beginn eines Kalenderjahres.

**Folgen:**
- Keine Umsatzsteuer auf Ausgangsrechnungen, kein Vorsteuerabzug.
- Rechnung mit Hinweis auf die Steuerbefreiung (§ 34a UStDV). Klein.Buch druckt immer:
  "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."
- Wer als Kleinunternehmer Umsatzsteuer ausweist, schuldet sie seit 2025 nach **§ 14c Abs. 1**
  UStG (unrichtiger Steuerausweis; gegenüber Endverbrauchern nach BMF nicht, wenn die Leistung
  erbracht wurde). Klein.Buch verhindert den Ausweis: Unter § 19 sind alle USt-Felder gesperrt.
- Leistungen nach § 13b UStG schuldet auch der Kleinunternehmer (siehe `ust-paragraph-13b.md`).

**Klein.Buch:** Der Status steht als Historie mit Wirkungsdatum und Grund
(`tax_status_changes`, ADR 0042 Nr. 1–6). Maßgeblich je Beleg ist das Leistungsdatum (bei einem
Zeitraum dessen Ende, sonst das Rechnungsdatum), bei Kosten das Belegdatum; beim Festschreiben
wird der Status je Beleg eingefroren. Klein.Buch zeigt den Gesamtumsatz und meldet erreichte
Grenzen; den Wechsel trägt der Nutzer ein.
