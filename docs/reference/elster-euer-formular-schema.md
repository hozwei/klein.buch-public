# ELSTER — Anlage EÜR: Zeilen- und Kennzahl-Zuordnung

Referenz für die **ELSTER-Ausfüllhilfe** (`euer::elster_csv`). Zielgruppe ist der
**selbst steuernde §19-Kleinunternehmer**, der seine EÜR in *Mein ELSTER*
abgibt. Stand: Lauf F5b, 25.10.2026 (ADR 0039 Nr. 135).

> **Wichtig:** ELSTER bietet **keinen CSV-Import** für die Anlage EÜR. Eine
> direkte elektronische Übermittlung ginge nur über die zertifizierte
> **ERiC-Schnittstelle**. Dieses Modul erzeugt deshalb eine **Ausfüllhilfe** zum
> Abtippen ins Online-Formular plus eine CSV fürs Archiv/Excel — keine
> Einreichung ans Finanzamt.

> **Steuerberater-Caveat:** Die Zuordnung ist ein **Vorschlag**. Sie ist vor der
> Abgabe gegenzuprüfen — insbesondere AfA, Kfz und Raumkosten. Klein.Buch ist ein
> Werkzeug, kein Steuerberater.

## Rechtsgrundlage / Quellen

- Anlage EÜR (Gewinnermittlung nach **§ 4 Abs. 3 EStG**); elektronische Abgabe
  nach **§ 60 Abs. 4 EStDV**; amtliche Vordrucke per BMF-Schreiben.
- Am Vordruck geprüft (Text der BMF-PDFs, 25.10.2026):
  - **Anlage EÜR 2024** — BMF-Schreiben vom 02.09.2024.
  - **Anlage EÜR 2025** — BMF-Schreiben vom 29.08.2025
    (<https://www.bundesfinanzministerium.de/Content/DE/Downloads/BMF_Schreiben/Steuerarten/Einkommensteuer/2025-08-29-anlage-EUER-2025.pdf>).
  - **Anlage EÜR 2026** — BMF-Schreiben vom 01.09.2026, GZ IV C 6 - S 2142/00024/010/001
    (<https://www.bundesfinanzministerium.de/Content/DE/Downloads/BMF_Schreiben/Steuerarten/Einkommensteuer/2026-09-01-anlage-EUER-2026.pdf>).
- Anleitung zur Anlage EÜR (in denselben Schreiben): Zeile 12 (Kleinunternehmer
  brutto, vor dem Übergang ausgeführte Umsätze bleiben regelbesteuert), Zeilen
  15–17 (netto, Umsatzsteuer gesondert), „Betriebsausgaben“ (netto mit Vorsteuer
  in Zeile 57; Kleinunternehmer brutto), Zeile 60 (Auffang „soweit nicht in den
  Zeilen 24 bis 59 berücksichtigt“).

**Kennzahlen (Kz) sind über die Jahre gleich**, die Zeilennummern nicht. Die
Ausfüllhilfe führt beides. Für Jahre ohne geprüften Vordruck (vor 2024, nach
2026) gilt der nächstgelegene Vordruck mit einem Hinweis „Kennzahlen maßgeblich“.
Erscheint ein neuer Vordruck, wird `Layout` in `euer::elster_csv` ergänzt.

## Leitlinie: Default = gesetzliches Minimum

Es werden **nur Positionen mit Betrag ≠ 0** ausgegeben. Die Anleitung verlangt
ausdrücklich, nicht betroffene Zeilen *nicht* auszufüllen (auch nicht mit 0,00).
Summen-Zeilen (Summe Betriebseinnahmen, Summe Betriebsausgaben, Gewinn/Verlust)
sind **Kontrollsummen** — in *Mein ELSTER* berechnet das Formular sie selbst.

## Betriebseinnahmen (Zeilen in allen Jahren gleich)

| Zeile | Kz | Position | Klein.Buch-Quelle |
|------:|---:|----------|-------------------|
| 12 | 111 | Betriebseinnahmen als umsatzsteuerlicher Kleinunternehmer | Zahlungen auf §19-Rechnungen − deren Storno-Erstattungen (brutto) |
| 15 | 112 | Umsatzsteuerpflichtige Betriebseinnahmen (netto) | Rechnungen ohne §19, Steuerart S mit Satz > 0, Nettoanteil der Zahlung |
| 16 | 103 | Umsatzsteuerfreie / nicht steuerbare Einnahmen, 0 %, § 13b UStG | Rechnungen ohne §19, Steuerart Z/AE/K/G/O |
| 17 | 140 | Vereinnahmte Umsatzsteuer | Steueranteil der Zahlungen auf Rechnungen ohne §19 |
| 19 | 102 | Veräußerung oder Entnahme von Anlagevermögen | `disposal_proceeds` |
| 23 | 159 | *Summe Betriebseinnahmen* (Kontrollsumme) | Summe der eingetragenen Zeilen |

Maßgeblich für Zeile 12 bzw. 15–17 ist der beim Festschreiben **eingefrorene
§19-Status der Rechnung**, nicht der Status des Zahlungsjahres. Eine Zahlung wird
im Verhältnis der Brutto-Gruppensummen auf die Steuergruppen der Rechnung
verteilt (`euer::detail::split_payment`). Storno-Erstattungen mindern die Zeile
des Originals im Jahr des Storno-Belegs (Cash-Basis, ADR 0022). Übersteigen sie
die Einnahmen des Jahres, wird die Zeile nicht negativ: der Überhang ist als
Rückzahlung von Betriebseinnahmen eine Betriebsausgabe (§ 11 Abs. 2 EStG) und
steht in Kz 183.

## Betriebsausgaben — Kategorie → Kennzahl

| Klein.Buch-Kategorie | Kz | Zeile 2024/2025 | Zeile 2026 | Position |
|----------------------|---:|----:|----:|----------|
| `goods` | 100 | 27 | 29 | Waren, Rohstoffe und Hilfsstoffe |
| `services` | 110 | 29 | 30 | Bezogene Fremdleistungen |
| `rent` | 150 | 39 | 40 | Miete/Pacht für Geschäftsräume |
| `communications` | 280 | 43 | 44 | Aufwendungen für Telekommunikation |
| `travel` | 221 | 44 | 45 | Übernachtungs- und Reisenebenkosten |
| `training` | 281 | 45 | 46 | Fortbildungskosten |
| `insurance` | 223 | 49 | 50 | Beiträge, Gebühren, Abgaben und Versicherungen |
| `software` | 228 | 50 | 51 | Laufende EDV-Kosten |
| `office` | 229 | 51 | 52 | Arbeitsmittel (Bürobedarf, Porto, Fachliteratur) |
| `marketing` | 224 | 54 | 55 | Werbekosten |
| `vehicle` | 146 | 70 | 71 | Kfz: sonstige tatsächliche Fahrtkosten |
| `hardware`, `fees`, `other` | 183 | 60 | 61 | Übrige unbeschränkt abziehbare Betriebsausgaben |
| (Vorsteuer, nur Regelbesteuerung) | 185 | 57 | 58 | Gezahlte abziehbare Vorsteuer |

Hinweise für die Steuerberater-Prüfung: `travel` bündelt Übernachtung und Fahrt
(amtlich getrennt: Zeile 44 bzw. 45 und Kfz-/Fahrtkosten), `vehicle` ist
Kfz-Sammelposten (amtlich Leasing, Steuern/Versicherungen, sonstige Kosten),
`fees` mischt Bankspesen und Gebühren. Aktivierte Kosten (Anlage aus einer
Kostenposition) sind keine Betriebsausgabe; sie wirken über die AfA.

Bei **Regelbesteuerung** im Zeitpunkt des Leistungsbezugs (Jahr des Belegdatums,
ADR 0039 Nr. 148) stehen die Ausgaben netto in ihrer Zeile und die Belegsteuer als
Vorsteuer in Kz 185, bei §19 brutto; das Jahr der Ausgabe ist das Zahlungsjahr.
Einen Wegfall von §19 mitten im Jahr (Überschreiten der 100.000-€-Grenze, § 19 Abs. 1
UStG) bildet Klein.Buch nicht ab: Wird dafür der Schalter umgelegt, stehen im
Wechseljahr auch Kosten vor dem Überschreiten netto mit Vorsteuer (ADR 0039
Nr. 148, Bekannte Grenze (b); STB prüfen).

## AfA & Anlagenabgang

| Quelle | Kz | Zeile 2024/2025 | Zeile 2026 |
|--------|---:|----:|----:|
| `linear`, `computer_special_2021` — AfA auf bewegliche Wirtschaftsgüter | 130 | 33 | 34 |
| `gwg_sofort` — geringwertige Wirtschaftsgüter (§ 6 Abs. 2 EStG) | 132 | 36 | 37 |
| Restbuchwert ausgeschiedener Anlagegüter (`disposal_book_value`) | 135 | 38 | 39 |

Vereinfachung (STB prüfen): Software-AfA gehört amtlich zu den immateriellen
Wirtschaftsgütern (Kz 131), Gebäude zu Kz 136 — beides wird mangels
Unterscheidung bei Kz 130 geführt.

## Kontrollsummen

| Position | Kz | Zeile 2024/2025 | Zeile 2026 |
|----------|---:|----:|----:|
| Summe Betriebseinnahmen | 159 | 23 | 23 |
| Summe Betriebsausgaben | 199 | 75 | 76 |
| Steuerpflichtiger Gewinn/Verlust | 219 | 97 | 98 |

Verluste stehen mit Minus in der Kontrollsumme Gewinn/Verlust.

## Grenzen bei Regelbesteuerung

Nicht abgebildet (Hinweis in CSV, PDF, Export-Seite und LIESMICH): an das
Finanzamt gezahlte bzw. erstattete Umsatzsteuer (Kz 186/141), Umsatzsteuer auf
Anlagenverkäufe und Entnahmen, nicht abziehbare Vorsteuer, Vorsteuer aus
Anlagen, die nicht aus einer erfassten Kostenposition übernommen wurden (seit
Lauf F8b zählt die Vorsteuer aktivierter Kosten in Kz 185 des Zahlungsjahres),
die Umsatzsteuer-Voranmeldung.

## Vollständiges EÜR-Paket (gesetzlicher Anspruch)

1. **Anlage EÜR** — die Formularzeilen (oben, `euer::elster_csv`).
2. **Anlage AVEÜR (Anlageverzeichnis)** — Pflicht bei Anlagevermögen
   (§ 4 Abs. 3 Satz 5 EStG): je Anlage AK/HK, Methode, Nutzungsdauer,
   betrieblicher Anteil, **AfA des Jahres**, Restwert Jahresanfang/-ende, Abgang.
   Quelle: `db::repo::euer::aveeur_items`.
3. **Einzelaufstellung** (GoBD-Einzelaufzeichnung): Einnahmen (Teilzahlungen je
   Zeile, dem Zuflussjahr zugeordnet), Storno-Erstattungen, Ausgaben (ohne
   aktivierte Kosten), Veräußerungen.

## Export-Formate

- **ELSTER-CSV** (`euer_export_elster`): `Zeile;Kennzahl;Position;Betrag;Art`
  (`Art` ∈ {Eingabe, Kontrollsumme}), UTF-8 **mit BOM** (deutsches Excel),
  Semikolon, Dezimalkomma, vorangestellte `#`-Kommentarzeilen (Geschäftsjahr,
  Vordruck, Hinweise). Abtipp- und Archivdatei, kein Importformat.
- **Einzelaufstellung-ZIP** (`euer_export_detail_zip`): `einnahmen.csv`,
  `ausgaben.csv`, `anlageverzeichnis.csv` — bewusst ohne Manifest (Arbeitsliste
  ohne Original-Belege; die Übergabe ist das Steuerberater-Paket).
- **PDF „Anlage EÜR {Jahr}"** (Typst) mit Zeile, Kz, Hinweisen, AVEÜR und
  Einzelaufstellung; Datumsangaben TT.MM.JJJJ.

Grenze: die **elektronische Übermittlung** ans Finanzamt (amtlicher Datensatz
nach § 60 Abs. 4 EStDV via ERiC) ist nicht enthalten.
