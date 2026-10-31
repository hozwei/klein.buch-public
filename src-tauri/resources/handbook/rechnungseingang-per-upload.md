---
slug: rechnungseingang-per-upload
title: Rechnungseingang per Upload
category: bedienen
order: 145
keywords: [rechnungseingang, upload, hochladen, eingangsrechnung, e-rechnung, xrechnung, zugferd, kosit, importieren, einlesen]
---

# Rechnungseingang per Upload

Viele Lieferanten schicken Dir ihre Rechnung als E-Rechnung. Das ist
eine Rechnung, deren Inhalt zusätzlich maschinenlesbar in einer
XML-Datei steckt. Es gibt sie in zwei Formen: als XRechnung (eine
reine XML-Datei) oder als ZUGFeRD-PDF (ein normales PDF, in dem die
XML-Datei als Anhang steckt). Solche Rechnungen musst Du nicht
abtippen. Du lädst die Datei im Browser hoch, Klein.Buch liest die
Angaben aus, Du prüfst sie und schreibst sie als Kosten fest.

Einen überwachten Ordner, in den Du Rechnungen nur hineinlegst, gibt
es in Klein.Buch nicht. Jede Eingangsrechnung kommt über den Upload
im Browser. So ist jede Datei Deiner Anmeldung zugeordnet, in der
Größe begrenzt und vor der Übernahme auf ihren Typ geprüft.

## Schritt für Schritt

1. Speichere die Rechnung aus Deiner E-Mail auf dem Gerät, an dem Du
   gerade sitzt. Auf dem Handy geht das meist über "Teilen" oder
   "In Dateien sichern".
2. Klick im Hauptmenü auf "Kosten" und dort oben auf **"E-Rechnung
   importieren"** (auf dem Handy im Menü "Mehr").
3. Wähle im Feld "Datei (XRechnung-XML oder ZUGFeRD-PDF)" die
   gespeicherte Datei aus.
4. Klick auf **"Einlesen und prüfen"**. Klein.Buch lädt die Datei hoch,
   liest die Angaben aus und prüft die Rechnung. Hast Du noch keine
   Datei gewählt oder kann Klein.Buch sie nicht lesen, steht die
   Meldung direkt am Datei-Feld.
5. Prüfe die vorausgefüllten Felder und ergänze, was fehlt (siehe
   unten).
6. Klick auf **"Kosten festschreiben"**. Klein.Buch fragt nach und
   nennt die Eckwerte (Belegdatum, Lieferant, Brutto, Zahlung). Mit
   "Festschreiben" wird die Position angelegt und geöffnet,
   "Abbrechen" ändert nichts.

Hast Du die falsche Datei erwischt, gehst Du mit "Andere Datei"
zurück zur Auswahl. "Abbrechen" bringt Dich ohne Änderung zurück zur
Kosten-Liste.

## Was die Prüfung bedeutet

Klein.Buch prüft jede eingelesene E-Rechnung mit dem KoSIT-Validator.
KoSIT ist die Koordinierungsstelle für IT-Standards, die offizielle
deutsche Prüfstelle für XRechnung. Ihr Prüfprogramm läuft in
Klein.Buch mit. Das Ergebnis steht oben auf der Prüfseite in einem
farbigen Kasten:

1. "Formal gültige E-Rechnung (keine Beanstandungen)": alles in
   Ordnung.
2. "Gültig, mit … Hinweisen" (mit der Anzahl): die Rechnung ist
   gültig, KoSIT hat aber Kleinigkeiten angemerkt.
3. "Die Rechnung hat formale Mängel (… Fehler)": der Absender hat Fehler in der
   Datei. Du kannst sie trotzdem erfassen. Der Befund wird mit
   gespeichert.
4. "Die formale Prüfung (KoSIT) war nicht möglich": der Prüfdienst
   hat gerade nicht geantwortet. Du kannst die Rechnung trotzdem
   erfassen. Mehr dazu im Kapitel "Sidecar-Probleme".

Die Prüfung ist eine Beratung, keine Sperre. Für die Richtigkeit
einer fremden Rechnung haftet der Absender, nicht Du. Mit "Prüf-Details
anzeigen" klappst Du die einzelnen Meldungen auf. Darunter steht,
welches Format Klein.Buch erkannt hat und an wen die Rechnung laut
Datei adressiert ist.

## Was Du ergänzen musst

Klein.Buch übernimmt Lieferant, Rechnungsnummer des Lieferanten,
Beleg-Datum, Beschreibung, Netto und Umsatzsteuer aus der Datei. Was
die Datei nicht wissen kann, ergänzt Du:

1. **Bezahlt am.** Eine E-Rechnung enthält kein Zahlungsdatum, deshalb
   ist "noch nicht bezahlt" zunächst angehakt. Solange das so bleibt,
   taucht die Ausgabe nicht in der EÜR auf. Die EÜR
   (Einnahmen-Überschuss-Rechnung) ist die einfache
   Steuer-Aufstellung: Einnahmen minus Ausgaben. Sie zählt eine
   Ausgabe erst am Tag der Zahlung. Hast Du schon bezahlt, nimm den
   Haken weg und trag das Datum ein. Sonst holst Du das später in der
   Kosten-Position über "Als bezahlt markieren" nach. Liegt das
   Zahldatum vor dem Beleg-Datum, weist Klein.Buch darauf hin;
   erlaubt ist es.
2. **Kategorie (EÜR).** Klein.Buch setzt zunächst "Sonstiges". Wähle
   die passende Kategorie, zum Beispiel "Software / Lizenzen".
3. **Lieferant (Kontakt).** Stimmt die USt-IdNr. der Rechnung mit
   einem Deiner Lieferanten-Kontakte überein, wählt Klein.Buch ihn
   selbst aus. Sonst bleibt der Name als Freitext im Feld
   "Lieferanten-Name" stehen.
4. **Zahlungs-Konto.** Vorbelegt ist Dein Standard-Konto.

Stimmen Netto oder Umsatzsteuer nicht mit dem Beleg überein,
korrigierst Du sie direkt. Der Brutto-Betrag rechnet sich dann neu.
Als Kleinunternehmer buchst Du immer den Brutto-Betrag als Ausgabe,
weil Du keine Vorsteuer abziehen darfst.

Weist die Rechnung aus, dass Du die Steuer schuldest (§13b, ohne
Umsatzsteuer), setzt Klein.Buch den Haken bei "§13b Reverse-Charge"
schon selbst; der USt-Betrag ist dann auf 0 gesperrt. Was Du bei
§13b sonst tun musst, steht im Kapitel "Kosten erfassen".

Erkennt Klein.Buch in der Rechnung die Steuerkategorie K
(innergemeinschaftliche Lieferung aus dem EU-Ausland, ohne
Umsatzsteuer), erscheint ein Hinweis: Auch mit §19 kann für Dich
Erwerbsteuer anfallen, abhängig von der Erwerbsschwelle und davon, ob
Du Deine USt-IdNr. verwendet hast (§1a Abs. 3 und 4 UStG). Das prüfst
Du selbst; Klein.Buch erfasst nur den Betrag laut Rechnung und ändert
wegen des Hinweises nichts an Beträgen oder §13b-Haken.

## Nach dem Festschreiben

Mit "Kosten festschreiben" wird die Kosten-Position sofort
unveränderlich. Die hochgeladene Original-Datei legt Klein.Buch
unverändert im Archiv ab und bewahrt sie 10 Jahre auf (länger als
die gesetzlichen 8 Jahre für Belege). Danach läuft automatisch ein Backup. Hast Du Dich
vertippt, stornierst Du die Position und erfasst sie neu.

In der Kosten-Position findest Du später den Knopf "Roh-XML
anzeigen" für die maschinenlesbaren Felder und im Abschnitt "Beleg"
den Knopf "Beleg herunterladen" für die Original-Datei.

## Welche Dateien gehen

Klein.Buch nimmt nur PDF und XML (in der Zeichenkodierung UTF-8) an,
höchstens 10 MB pro Datei. Geprüft wird der Inhalt der Datei, nicht
die Endung im Namen. ZIP-Archive, Word-Dateien oder gespeicherte
E-Mails lehnt Klein.Buch ab. Entpacke sie vorher und lade die
enthaltene Rechnung hoch.

Bei ZUGFeRD gibt es verschiedene Profile, also Varianten des Formats.
Klein.Buch akzeptiert die Profile, die seit 2025 als gültige
E-Rechnung gelten: `EN16931`, `EXTENDED` und `XRECHNUNG`. Die alten
Profile `MINIMUM` und `BASIC-WL` lehnt Klein.Buch ab. Sie gelten nur
noch als maschinenlesbare Beilage zu einer Papier- oder PDF-Rechnung.

Gutschriften liest Klein.Buch nicht automatisch ein, damit die EÜR
stimmig bleibt. Erfasse sie selbst als Korrektur.

Ist ein PDF gar keine E-Rechnung, weil keine XML-Datei darin steckt,
meldet Klein.Buch das. Dann erfasst Du die Ausgabe wie einen
Papier-Beleg über "+ Neue Kosten" und hängst das PDF dort als Beleg
an.

## Wenn etwas nicht klappt

Die typischen Fehlermeldungen und was Du dagegen tust, stehen im
Kapitel "Probleme beim Rechnungseingang" unter "Fehlerbehebung".

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
