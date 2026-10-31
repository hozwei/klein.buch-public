# DATEV-Buchungsstapel (EXTF) — Format + Konten-Mapping

Referenz für den DATEV-Export (`euer::datev_csv`). Zielgruppe: die **Übergabe an
den Steuerberater**, der den Buchungsstapel in DATEV Rechnungswesen über die
Stapelverarbeitung importiert. Stand: Lauf F5b, 25.10.2026 (ADR 0039 Nr. 135).

> **Steuerberater-Caveat:** Alle Konten sind ein **Vorschlag** und vor der
> Verbuchung zu prüfen/anzupassen. Klein.Buch ist ein Werkzeug, kein
> Steuerberater. Der Buchungsstapel sollte vom Steuerberater einmal **testweise
> importiert** werden (DATEV ist die maßgebliche Validierung).

## Format (DATEV-Formatbeschreibung „Buchungsstapel“, Version 700)

- **Vorlauf (Kopfzeile, 31 Felder):** `"EXTF";700;21;"Buchungsstapel";12;<Erzeugt am>;…`
  - Kennzeichen `EXTF` (Export aus Fremdsoftware), Versionsnummer `700`,
    Datenkategorie `21` (Buchungsstapel), Formatname `Buchungsstapel`,
    Formatversion `12` (vom Steuerberater importiert).
  - Feld 11/12 **Berater-/Mandantennummer:** standardmäßig leer — beim Import
    über die Stapelverarbeitung gilt der geöffnete Mandant. Optional über die
    Export-Commands (`consultantNumber` 1001–9999999, `clientNumber` 1–99999;
    andere Werte werden abgelehnt).
  - WJ-Beginn = `{Jahr}0101`, Datum von/bis = `{Jahr}0101`/`{Jahr}1231`,
    Sachkontenlänge `4`, Buchungstyp `1`, Festschreibung `0`, WKZ `EUR`.
  - Feld 27 **Sachkontenrahmen** `"03"` bzw. `"04"`.
- **Spaltenüberschrift + Buchungszeilen:** die führenden Standard-Spalten
  (positionsbasiert): Umsatz (ohne S/H-Kz), Soll/Haben-Kennzeichen, WKZ Umsatz,
  Kurs, Basis-Umsatz, WKZ Basis-Umsatz, Konto, Gegenkonto, BU-Schlüssel,
  Belegdatum, Belegfeld 1, Belegfeld 2, Skonto, Buchungstext.
- **Kodierung:** CP1252 (Codepage 1252) mit vollständiger Tabelle (27 belegte Zeichen in
  0x80–0x9F nach WHATWG „windows-1252“); C1-Steuerzeichen und nicht darstellbare
  Zeichen werden `?`. CRLF-Zeilenenden, Dezimal-**Komma**, kein Tausendertrenner.
  Belegdatum **TTMM**, Umsatz immer positiv (Richtung über Konto/Gegenkonto +
  S/H). Steuerzeichen in Textfeldern werden Leerzeichen (einzeilige Felder).
  Belegfeld 1 max. 36, Belegfeld 2 max. 12, Buchungstext max. 60 Zeichen.

## Buchungslogik (Cash-Basis)

Das Soll/Haben-Kennzeichen bezieht sich auf „Konto“. Bei jeder Geldbewegung
ist **„Konto“ das Geldkonto** und **„Gegenkonto“ das Sachkonto**; ein
BU-Schlüssel gehört zum Gegenkonto (DATEV-Formatbeschreibung Buchungsstapel:
Feld „Gegenkonto (ohne BU-Schlüssel)“, der Schlüssel wurde früher dem
Gegenkonto vorangestellt). So steht nie ein Steuerschlüssel an einem
Geldkonto (Lauf F, F5-fix, 25.10.2026).

| Vorgang | Konto | S/H | Gegenkonto |
|---|---|---|---|
| Einnahme (Zahlungseingang) | Geldkonto | S | Erlöskonto (§19 bzw. je Steuergruppe) |
| Gegenbuchung („Zahlung stornieren“, ADR 0042 Nr. 9) | Geldkonto der Zahlung | H | Erlöskonto der Zahlung; Belegdatum und Belegfeld 1 der stornierten Zahlung, Buchungstext „Storno Zahlung …“ |
| Storno-Erstattung | Geldkonto | H | Erlöskonto des Originals |
| Ausgabe (bezahlte Kosten) | Geldkonto | H | Aufwandskonto je Kategorie (ggf. mit BU-Schlüssel) |
| Aktivierte Kosten (Anlage aus Kostenposition) | Geldkonto | H | Anlagekonto (ggf. mit BU-Schlüssel) |
| Anlagen-Verkauf | Bank | S | Erlöse Anlagenverkauf (ohne USt) |
| Privatentnahme | Geldkonto | H | Privatentnahmen |
| Privateinlage | Geldkonto | S | Privateinlagen |
| Anlagen-Abgang | Anlagenabgang Restbuchwert | S | Anlagekonto |
| AfA (31.12.) | Abschreibungsaufwand | S | Anlagekonto (direkte AfA) |

Ein **negativer Anteil** einer Steuergruppe (z. B. Gutschrift-Position in einer
anderen Gruppe als der Hauptumsatz) wird als Gegenbuchung mit umgekehrtem
Kennzeichen ausgegeben, nicht weggelassen; die Summe der Geldkonto-Bewegungen
einer Zahlung ist damit gleich dem Zahlungsbetrag (Bankabstimmung). Nullanteile
entfallen.

Ein Importtest beim Steuerberater (DATEV Kanzlei-Rechnungswesen bzw. dessen
Prüfprogramm) mit einer Beispieldatei steht aus; bis dahin gilt die Zuordnung
als Vorschlag wie die Kontenwahl.

## Geldkonten (R2-011)

| Zahlungskonto-Typ | SKR03 | SKR04 |
|---|---|---|
| `bank` (und ohne Angabe) | 1200 Bank | 1800 Bank |
| `cash` | 1000 Kasse | 1600 Kasse |
| `paypal`, `stripe`, `other` | 1360 Geldtransit | 1460 Geldtransit |

Das Konto eines Zahlungseingangs steht im Zahlungsbuch `invoice_payments`
(`account_id`, Feld „Zahlungs-Konto“ beim Erfassen; Schema 34), bei Kosten in
`paid_from_account_id`, bei Privatbewegungen in `account_id`. Eine Gegenbuchung
trägt das Konto der stornierten Zahlung. Eine Storno-Erstattung nimmt das Konto
des letzten nicht stornierten Zahlungseingangs des Originals mit Konto, sonst
das Konto der Rechnung bzw. Bank; ihr Betrag ist die Summe des Zahlungsbuchs
(Zahlungen minus Gegenbuchungen).

## Konten (SKR03 / SKR04), am DATEV-Kontenrahmen geprüft

| Zweck | SKR03 | SKR04 |
|---|---|---|
| Erlöse als Kleinunternehmer (§ 19 UStG) | 8195 | 4185 |
| Erlöse 19 % USt (Automatik, Regelbesteuerung) | 8400 | 4400 |
| Erlöse 7 % USt (Automatik) | 8300 | 4300 |
| Steuerfreie innergem. Lieferungen § 4 Nr. 1b (Steuerart K) | 8125 | 4125 |
| Steuerfreie Umsätze § 4 Nr. 1a (Steuerart G) | 8120 | 4120 |
| Erlöse ohne Automatik (übrige Steuerarten, andere Sätze netto) | 8200 | 4200 |
| Umsatzsteuer (Sätze ohne Automatikkonto) | 1770 | 3800 |
| Abziehbare Vorsteuer (Sätze ohne BU-Schlüssel) | 1570 | 1400 |
| Erlöse Anlagenverkauf ohne USt, bei Buchgewinn | 8829 | 4849 |
| Erlöse Anlagenverkauf ohne USt, bei Buchverlust | 8800 | 6889 |
| Anlagenabgang Restbuchwert, bei Buchgewinn | 2315 | 4855 |
| Anlagenabgang Restbuchwert, bei Buchverlust | 2310 | 6895 |
| AfA Abschreibungsaufwand | 4830 | 6220 |
| GWG-Sofortabschreibung | 4855 | 6260 |
| Anlagekonto (Büroeinrichtung) | 0420 | 0650 |
| GWG-Anlagekonto | 0480 | 0670 |
| Privatentnahmen allgemein | 1800 | 2100 |
| Privateinlagen | 1890 | 2180 |

Die Konten 8820/4845 (Anlagenverkauf **19 % USt**) sind Automatikkonten und
werden für §19 nicht verwendet.

### Kosten je Kategorie → Aufwandskonto (ohne Steuerautomatik)

| Kategorie | SKR03 | SKR04 |
|---|---|---|
| goods (Wareneingang) | 3200 | 5200 |
| services (Fremdleistungen) | 3100 | 5900 |
| office (Bürobedarf) | 4930 | 6815 |
| travel (Reisekosten Unternehmer) | 4670 | 6670 |
| communications (Telefon) | 4920 | 6805 |
| vehicle (Laufende Kfz-Betriebskosten) | 4530 | 6530 |
| rent (Miete unbewegliche Wirtschaftsgüter) | 4210 | 6310 |
| insurance (Versicherungen) | 4360 | 6400 |
| training (Fortbildungskosten) | 4945 | 6821 |
| fees (Nebenkosten des Geldverkehrs) | 4970 | 6855 |
| marketing (Werbekosten) | 4600 | 6600 |
| software (Wartungskosten für Hard- und Software) | 4806 | 6495 |
| hardware (Sonstiger Betriebsbedarf) | 4980 | 6850 |
| other / unbekannt (Sonstige betriebliche Aufwendungen) | 4900 | 6300 |

### Regelbesteuerung

- **Erlöse** je Steuergruppe der Rechnung (Zahlung anteilig verteilt): 19 % und
  7 % auf die Automatikkonten mit Bruttobetrag; Steuerart K/G auf die
  steuerfreien Erlöskonten; andere Sätze netto auf 8200/4200 plus Umsatzsteuer
  auf 1770/3800; übrige Steuerarten auf 8200/4200.
- **Kosten** im Jahr mit Regelbesteuerung: Bruttobetrag mit **BU-Schlüssel 9**
  (19 % Vorsteuer) bzw. **8** (7 %) am Gegenkonto (Aufwands- bzw.
  Anlagekonto), wenn die Belegsteuer dem Satz auf den Cent entspricht; sonst
  Netto auf das Aufwandskonto und die Steuer auf 1570/1400 (je eine Buchung
  Geldkonto H an Sachkonto).
  Bei §19 brutto ohne BU-Schlüssel.

## Bewusste Vereinfachungen (vom Steuerberater zu prüfen)

- **Feste Zuordnung** statt konfigurierbarer Kontentabelle (v1-Entscheidung,
  ADR 0039 Nr. 135): der Steuerberater ordnet beim Import ohnehin um.
- **Anlage-/AfA-Gegenkonto** = Büroeinrichtung (0420/0650) bzw. GWG
  (0480/0670), da Klein.Buch keine anlagenspezifischen Bilanzkonten führt.
- Anlagenverkäufe ohne Umsatzsteuer, auch bei Regelbesteuerung (Hinweis in der
  Ausfüllhilfe); das Geldkonto eines Anlagenverkaufs ist die Bank.

## Quellen

- DATEV-Format, Header + Buchungsstapel:
  <https://developer.datev.de/de/file-format/details/datev-format/format-description/header>
- WHATWG Encoding Standard, Index „windows-1252“:
  <https://encoding.spec.whatwg.org/index-windows-1252.txt>
- DATEV-Kontenrahmen SKR03/SKR04 (Kontenbezeichnungen geprüft am 25.10.2026 an
  den Kontenlisten unter <https://www.steuerschroeder.de/Kontenrahmen-SKR03-Prozessgliederungsprinzip.html>
  und <https://www.steuerschroeder.de/Kontenrahmen-SKR04-Bilanzgliederungsprinzip.html>).
