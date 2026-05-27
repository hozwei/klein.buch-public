# ELSTER — Anlage EÜR: Zeilen-Zuordnung (Block 14a)

Referenz für die **ELSTER-Ausfüllhilfe** (`euer::elster_csv`). Zielgruppe ist der
**selbst steuernde §19-Kleinunternehmer**, der seine EÜR in *Mein ELSTER*
abgibt.

> **Wichtig:** ELSTER bietet **keinen CSV-Import** für die Anlage EÜR. Eine
> direkte elektronische Übermittlung ginge nur über die zertifizierte
> **ERiC-Schnittstelle** (eigener, späterer Block). Dieses Modul erzeugt deshalb
> eine **Ausfüllhilfe** zum Abtippen ins Online-Formular plus eine CSV fürs
> Archiv/Excel — keine Einreichung ans Finanzamt.

> **Steuerberater-Caveat:** Die Zuordnung ist ein **Vorschlag**. Sie ist vor der
> Abgabe gegenzuprüfen — insbesondere AfA (Zeile 33/36), Kfz (Zeile 68) und
> Raumkosten (Zeile 39). Klein.Buch ist ein Werkzeug, kein Steuerberater.

## Rechtsgrundlage / Quellen

- Anlage EÜR (Gewinnermittlung nach **§ 4 Abs. 3 EStG**); elektronische Abgabe
  nach **§ 60 Abs. 4 EStDV**.
- Zeilennummern nach der **Anleitung zur Anlage EÜR 2025** (ELSTER/BMF).
  Stand der Recherche: 2026-05-21. Das Formular 2026 erscheint erst später im
  Jahr; bei Wechsel die Zeilen erneut prüfen.
- Quellen:
  - ELSTER, „Anleitung zur Anlage EÜR 2025":
    <https://www.elster.de/eportal/helpGlobal?themaGlobal=help_euer_ufa_77_2025>
  - BMF, Anlage EÜR 2025 (Formular):
    <https://www.bundesfinanzministerium.de/Content/DE/Downloads/BMF_Schreiben/Steuerarten/Einkommensteuer/2025-08-29-anlage-EUER-2025.pdf>

## Leitlinie: Default = gesetzliches Minimum

Es werden **nur Positionen mit Betrag ≠ 0** ausgegeben. Die Anleitung verlangt
ausdrücklich, nicht betroffene Zeilen *nicht* auszufüllen (auch nicht mit 0,00).
Summen-Zeilen (23, Betriebsausgaben-Summe, Gewinn) sind nur **Kontrollsummen**
zum Abgleich — in *Mein ELSTER* berechnet das Formular sie selbst.

## Betriebseinnahmen

| Zeile | Position | Klein.Buch-Quelle |
|------:|----------|-------------------|
| **12** | Betriebseinnahmen als umsatzsteuerlicher Kleinunternehmer (§ 19 Abs. 1 UStG) | `invoice_income − storno_refunds` (Brutto, cash-basis) |
| 15 | Umsatzsteuerpflichtige Betriebseinnahmen (netto) | nur bei Regelbesteuerung (`is_kleinunternehmer = false`) — USt-Aufteilung manuell prüfen |
| **19** | Veräußerung oder Entnahme von Anlagevermögen | `disposal_proceeds` (Verkaufserlös) |
| 23 | *Summe der Betriebseinnahmen* (Kontrollsumme) | `total_income` |

Storno-Erstattungen mindern Zeile 12 im Jahr des Storno-Belegs (Cash-Basis,
keine Rückwirkung — siehe ADR 0022).

## Betriebsausgaben — Kategorie → Zeile

Kategorien ohne eindeutige amtliche Zeile landen bewusst im **Auffangposten
Zeile 60** („Übrige unbeschränkt abziehbare Betriebsausgaben, soweit nicht in den
Zeilen 24 bis 59 berücksichtigt") — das ist immer zulässig.

| Klein.Buch-Kategorie | Zeile | Position |
|----------------------|------:|----------|
| `goods` (Wareneinkauf) | 27 | Waren, Rohstoffe und Hilfsstoffe einschl. Nebenkosten |
| `services` (Fremdleistungen) | 29 | Bezogene Leistungen |
| `rent` (Miete / Raumkosten) | 39 | Raumkosten und sonstige Grundstücksaufwendungen |
| `travel` (Reisekosten) | 44 | Übernachtungs- und Reisenebenkosten bei Geschäftsreisen |
| `vehicle` (Kfz / Fahrzeug) | 68 | Kraftfahrzeugkosten und andere Fahrtkosten |
| `office`, `software`, `hardware`, `communications`, `insurance`, `training`, `fees`, `marketing`, `other` | 60 | Übrige unbeschränkt abziehbare Betriebsausgaben |

Hinweise für die STB-Prüfung: `travel` bündelt Übernachtung/Verpflegung/Fahrt
(amtlich getrennt: Zeile 44/64/68–73); `vehicle` ist Kfz-Sammelposten (amtlich
Zeile 68–73); `software`/`hardware` als laufende Kosten → 60, als Anschaffung →
Anlage/AfA.

## AfA & Anlagenabgang

Die Jahres-AfA wird über die `depreciation_method` der Anlage aufgeteilt
(`db::repo::euer::depreciation_split_for_year`):

| Methode | Zeile | Position |
|---------|------:|----------|
| `linear`, `computer_special_2021` | 33 | AfA auf bewegliche Wirtschaftsgüter |
| `gwg_sofort` | 36 | Geringwertige Wirtschaftsgüter (§ 6 Abs. 2 EStG) |
| (Restbuchwert verkaufter/entsorgter Anlagen) | 38 | Restbuchwert der ausgeschiedenen Anlagegüter (`disposal_book_value`) |

Vereinfachung (STB prüfen): Software-AfA gehört amtlich auf Zeile 32 (immaterielle
Wirtschaftsgüter), Gebäude auf Zeile 31 — beides wird hier mangels Unterscheidung
auf Zeile 33 geführt.

## Vollständiges EÜR-Paket (gesetzlicher Anspruch)

Die Selbst-Abgabe-Ausgabe bildet das komplette EÜR-Paket ab, nicht nur die
Formularzeilen-Summen:

1. **Anlage EÜR** — die Formularzeilen-Summen (oben, `euer::elster_csv`).
2. **Anlage AVEÜR (Anlageverzeichnis)** — gesetzl. Pflicht bei Anlagevermögen
   (§ 4 Abs. 3 Satz 5 EStG): je Anlage AK/HK, Methode, Nutzungsdauer,
   betrieblicher Anteil, **AfA des Jahres**, Restwert Jahresanfang/-ende, Abgang.
   Quelle: `db::repo::euer::aveeur_items` (LEFT JOIN `depreciation_entries` auf das
   Jahr; ohne gebuchte AfA fällt der Restwert auf den aktuellen Buchwert zurück).
3. **Einzelaufstellung** (GoBD-Einzelaufzeichnung, prüfungssicher):
   - Einnahmen — jeder Zahlungseingang mit Datum, Rechnungsnr., Kunde,
     **Beschreibung** (aus den Rechnungspositionen zusammengefasst), Betrag
     (`income_detail`; Teilzahlungen je eigene Zeile, dem Zuflussjahr zugeordnet).
   - Storno-Erstattungen (`storno_detail`).
   - Ausgaben — jede bezahlte Kostenposition mit Datum, Beleg-Nr., Lieferant,
     Kategorie, **Beschreibung**, Brutto (`expense_detail`).
   - Veräußerungen (`disposal_detail`).

Die Einzelposten-Loader werden von **Block 14b (DATEV-Buchungsstapel)** mitgenutzt.

## Export-Formate

- **ELSTER-Summen-CSV** (`euer_export_elster`): `Zeile;Position;Betrag;Art`
  (`Art` ∈ {Eingabe, Kontrollsumme}), UTF-8 **mit BOM**, Semikolon, Dezimalkomma,
  vorangestellte `#`-Kommentarzeile mit Geschäftsjahr + Quelle.
- **Einzelaufstellung-ZIP** (`euer_export_detail_zip`): `einnahmen.csv`,
  `ausgaben.csv`, `anlageverzeichnis.csv` (jeweils BOM/Semikolon/Dezimalkomma,
  mit Summenzeile).
- **PDF „Anlage EÜR {Jahr}"** (Schritt 2, Typst) — eigenständiges Dokument ohne
  App-Chrome, enthält Anlage EÜR + AVEÜR + Einzelaufstellung; wiederverwendbar im
  Steuerberater-Paket (14c).

Grenze: die **elektronische Übermittlung** ans Finanzamt (amtlicher Datensatz
nach § 60 Abs. 4 EStDV via ERiC) ist nicht enthalten — eigener späterer Block.
