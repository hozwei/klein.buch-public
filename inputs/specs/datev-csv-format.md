# DATEV-CSV-Format

DATEV ist die deutsche De-facto-Standard-Software für Steuerberater. Der
**DATEV-Buchungsstapel-Import** (CSV-basiert, "DATEV-Format Version 5.0+")
ist der wichtigste Integrations-Punkt — fast jeder Steuerberater erwartet
diesen Import-Format.

Quelle: https://developer.datev.de/datev/platform/de/dtvf/formate/header

Header-Zeile (erste 17 Spalten):
```
"EXTF";510;21;"Buchungsstapel";13;...;...;<JJJJMMDDHHMMSSXXX>;...
```

Daten-Zeilen (Spalten 1–22):
```
Umsatz (ohne Vorzeichen)
Soll/Haben-Kennzeichen (S/H)
WKZ Umsatz (EUR)
Kurs (leer für EUR)
Basisumsatz (leer)
WKZ Basisumsatz (leer)
Konto (Sachkonto)
Gegenkonto
BU-Schlüssel (Steuerschlüssel)
Belegdatum (TTMM)
Belegfeld 1 (Belegnummer)
Belegfeld 2 (Kostenstelle 1, leer)
Skonto (leer)
Buchungstext (max. 60 Zeichen)
...
```

SKR03 oder SKR04 — Manuels Steuerberater wählt den Kontenrahmen.
Default SKR03 ist verbreiteter; SKR04 ist näher an HGB-Bilanz-Struktur.

Klein.Buch's Block 14 erzeugt diesen CSV mit korrektem Encoding (Win-1252)
und Trennzeichen (`;`).
