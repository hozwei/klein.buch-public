# DATEV-Format — Buchungsstapel

DATEV ist die verbreitete Software der Steuerberater. Klein.Buch exportiert die Buchungen
eines Geschäftsjahres als DATEV-Buchungsstapel (CSV), den die Kanzlei über die
Stapelverarbeitung einliest.

Quelle: https://developer.datev.de/de/file-format/details/datev-format
(Header, Buchungsstapel, Zeichensatz; dort auch das kostenlose Prüfprogramm und Musterdaten)

- Dateiname mit festem Präfix `EXTF_`, z. B. `EXTF_Buchungsstapel_2026_SKR03.csv`.
- Zeichensatz Windows-1252 (ISO-8859-1), ohne BOM, Zeilenende CRLF, Trennzeichen `;`,
  Textfelder in `"…"`, Beträge mit Dezimalkomma ohne Tausendertrenner.
- Zeile 1 Kopf (31 Felder):
  `"EXTF";700;21;"Buchungsstapel";12;<Erzeugt am JJJJMMTTHHMMSSmmm>;;"";"";"";<Berater>;<Mandant>;<WJ-Beginn JJJJ0101>;4;<von>;<bis>;"<Bezeichnung>";"";1;0;0;"EUR";;"";;;"03|04";;;"";""`
  — Berater- (1001–9999999) und Mandantennummer (1–99999) sind Pflicht.
- Zeile 2 Spaltenüberschriften, ab Zeile 3 Buchungen (Umsatz ohne Vorzeichen, Soll/Haben,
  WKZ, Konto, Gegenkonto, BU-Schlüssel, Belegdatum TTMM, Belegfeld 1/2, Buchungstext ≤ 60).

SKR03 oder SKR04 — der Steuerberater wählt den Kontenrahmen; Standard SKR03.
Formatversion 12 bleibt, bis Prüfprogramm oder Import etwas anderes zeigen (ADR 0042 Nr. 15).
