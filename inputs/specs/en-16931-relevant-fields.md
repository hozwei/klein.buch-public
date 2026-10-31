# EN 16931 — relevante Felder für Klein.Buch

Europäische Norm für elektronische Rechnungen (EN 16931-1:2017+A1:2019).
Definiert das semantische Datenmodell (BT = Business Term, BG = Business
Group), Kardinalitäten und Geschäftsregeln (BR-…). Klein.Buch erzeugt
XRechnung 3.0 (deutsche Spezifikation der KoSIT, Syntax UN/CEFACT CII;
geprüft mit dem KoSIT-Validator, Konfiguration laut
`src-tauri/sidecar/versions.json`) und bettet sie im ZUGFeRD-PDF/A-3 ein
(Profil XRECHNUNG). Beim Empfang liest es XRechnung (CII und UBL) sowie
ZUGFeRD/Factur-X in den Profilen EN 16931, EXTENDED und XRECHNUNG;
MINIMUM und BASIC WL sind keine E-Rechnungen im Sinne von § 14 Abs. 1
UStG und werden abgelehnt (ADR 0037).

Quellen: CEN, EN 16931-1 —
https://standards.cen.eu/dyn/www/f?p=204:110:0::::FSP_PROJECT,FSP_ORG_ID:60602,2480 ;
XRechnung (KoSIT) — https://xeinkauf.de/xrechnung/ ; § 14 Abs. 1 und 4 UStG,
§ 33 UStDV, § 34a UStDV.

Die Zuordnung Klein.Buch-Feld → BT steht in `einvoice::generator`. Wichtige
Felder (Pflicht nach EN 16931, nach XRechnung „BR-DE-…“ oder nach § 14 UStG):

- BT-1 Rechnungsnummer, BT-2 Rechnungsdatum, BT-3 Rechnungsart (380,
  Storno-Beleg 384 „Corrected invoice“), BT-5 Währung (EUR).
- BT-10 Käuferreferenz (Leitweg-ID; in XRechnung Pflicht, BR-DE-15 — ohne
  Leitweg-ID „N/A“). Seit Migration 0035 am Entwurf gespeichert (ADR 0042
  Nr. 12).
- BT-22 Rechnungshinweis: bei § 19 UStG die Klausel „Gemäß §19 UStG wird
  keine Umsatzsteuer ausgewiesen.“ (Pflichtangabe nach § 34a UStDV bzw.
  § 14 Abs. 4 Satz 1 Nr. 8 UStG, keine Pflicht der Norm selbst).
- BT-25/BT-26 Bezug auf die stornierte Rechnung (Nummer und Datum).
- BG-6 Verkäuferkontakt (BT-41 bis BT-43; XRechnung BR-DE-2, -5, -6, -7).
- BT-31 USt-IdNr. oder BT-32 Steuernummer des Verkäufers (bei Steuerkategorie
  E: BR-E-02).
- BT-44 Käufername, BG-8 Käuferanschrift (BT-50 bis BT-55; Ort und PLZ in
  XRechnung Pflicht).
- BT-72 Lieferdatum oder BG-14 Rechnungszeitraum (BT-73/BT-74): der
  Leistungszeitpunkt nach § 14 Abs. 4 Satz 1 Nr. 6 UStG (Kalendermonat
  genügt, § 31 Abs. 4 UStDV); mit Zeitraum entfällt BT-72 (ADR 0042 Nr. 10).
  Darf nach dem Rechnungsdatum liegen (Vorausrechnung, ADR 0042 Nr. 11).
- BT-9 Fälligkeitsdatum oder BT-20 Zahlungsbedingungen, wenn ein Betrag
  offen ist (BR-CO-25).
- BT-106 Summe Positionen netto, BT-109 Gesamtbetrag ohne USt, BT-110
  Umsatzsteuer (bei § 19 = 0), BT-112 Gesamtbetrag mit USt, BT-115 fälliger
  Betrag.
- BG-23 Umsatzsteueraufschlüsselung: bei § 19 Steuerkategorie E (BT-118),
  Satz 0, Befreiungsgrund als Code BT-121 oder Text BT-120 Pflicht
  (BR-E-10); Klein.Buch setzt BT-120.
