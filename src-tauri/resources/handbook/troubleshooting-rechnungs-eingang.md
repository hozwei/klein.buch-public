---
slug: troubleshooting-rechnungs-eingang
title: Probleme beim Rechnungseingang
category: troubleshooting
order: 560
keywords: [rechnungseingang, upload, e-rechnung, fehler, einlesen, xrechnung, zugferd, profil, utf-8, dateityp]
---

# Probleme beim Rechnungseingang

Eingehende E-Rechnungen lädst Du über "Kosten" und "E-Rechnung
importieren" im Browser hoch. Wie das im Normalfall abläuft, steht im
Kapitel "Rechnungseingang per Upload". Dieses Kapitel sammelt die
Meldungen, die dabei auftauchen können, und was Du dagegen tust.
Die Meldungen erscheinen als kurze Einblendung am Bildschirm, meist
mit dem Anfang "E-Rechnung konnte nicht gelesen werden".

## "Dateityp nicht erlaubt"

Klein.Buch nimmt beim Rechnungseingang nur PDF und XML an. Geprüft
wird der Inhalt der Datei, nicht die Endung im Namen. Eine ZIP-Datei,
eine Word-Datei oder eine gespeicherte E-Mail (`.eml`) lehnt
Klein.Buch deshalb ab, auch wenn sie umbenannt wurde. Entpacke die
Datei oder speichere den Anhang der E-Mail einzeln und lade dann die
eigentliche Rechnung hoch.

## "Die XML-Datei ist nicht UTF-8-codiert"

UTF-8 ist die Zeichenkodierung, die für E-Rechnungen vorgeschrieben
ist. Sie sorgt dafür, dass Umlaute wie in "Müller" richtig ankommen.
Hat der Absender die Datei anders kodiert, würde Klein.Buch die Namen
verstümmelt ins Archiv übernehmen. Deshalb lehnt es die Datei ab. Bitte
den Absender, die Rechnung als UTF-8 neu zu erzeugen.

## "Anfrage zu groß" oder "Die Datei ist zu groß"

Eine E-Rechnung darf höchstens 10 MB groß sein. Echte E-Rechnungen
sind meist viel kleiner. Ist die Datei größer, steckt oft etwas
anderes dahinter, zum Beispiel ein eingescannter Stapel. Erfasse die
Ausgabe dann über "+ Neue Kosten" und hänge die Datei dort als Beleg
an. Für Belege liegt die Grenze bei 20 MB.

## "In dieser PDF wurde keine eingebettete E-Rechnung gefunden"

Das PDF ist eine normale PDF-Rechnung ohne XML-Anhang, also keine
ZUGFeRD-Rechnung. Du kannst sie wie einen Papier-Beleg behandeln:
über "+ Neue Kosten" eintragen und das PDF dort als Beleg anhängen.

## "ZUGFeRD/XRechnung-Profil nicht unterstützt"

Die Datei nutzt eines der alten Profile `MINIMUM` oder `BASIC-WL`.
Ein Profil ist eine Variante des ZUGFeRD-Formats. Seit 2025 gelten
nur noch `EN16931`, `EXTENDED` und `XRECHNUNG` als gültige
E-Rechnung. Klein.Buch nennt in der Meldung das erkannte Profil. Bitte
den Absender um eine richtige E-Rechnung. Klappt das nicht, erfasst
Du die Ausgabe selbst über "+ Neue Kosten".

## "Storno-/Gutschrift-Beleg … wird nicht automatisch importiert"

Gutschriften liest Klein.Buch nicht automatisch ein. Sie würden die
EÜR (Einnahmen-Überschuss-Rechnung, die einfache Steuer-Aufstellung)
verfälschen, wenn sie wie eine normale Ausgabe gebucht würden. Erfasse
die Gutschrift selbst als Korrektur.

## "Unbekanntes E-Rechnungs-Format" oder "Pflichtfeld fehlt"

Die Datei ist beschädigt oder keine E-Rechnung, zum Beispiel ein
abgebrochener Download. Öffne sie in einem Text-Editor. Ist sie leer
oder unleserlich, frag beim Absender nach einer neuen Fassung. Fehlt
nur eine Pflichtangabe, nennt die Meldung das Feld. Auch dann muss der
Absender die Rechnung neu ausstellen.

## "Upload nicht gefunden oder abgelaufen"

Eine hochgeladene Datei hält Klein.Buch eine Stunde lang bereit.
Liegt zwischen "Einlesen & prüfen" und "Als Kosten festschreiben"
mehr Zeit, oder wurde der Server zwischendurch neu gestartet, ist sie
weg. Klick auf "Andere Datei" und lade die Rechnung noch einmal hoch.

## "Upload abgelehnt" wegen zu wenig Speicherplatz

Vor jedem Upload prüft Klein.Buch, ob auf dem Server genug Platz frei
bleibt (standardmäßig 2 GB Reserve). Ist der Speicher fast voll, lehnt
es den Upload ab, damit Datenbank und Archiv nicht volllaufen. Das
musst Du als Betreiber auf dem Server lösen. Mehr dazu im Kapitel
"Speicher und Ressourcen".

## "Der Prüf- und Konvertierungsdienst (Java) ist nicht verfügbar"

Klein.Buch braucht für ZUGFeRD-PDFs ein Java-Hilfsprogramm, den
Sidecar. Er holt die XML-Datei aus dem PDF. Antwortet er nicht, kannst
Du ein ZUGFeRD-PDF gerade nicht einlesen. Eine XRechnung-XML geht
trotzdem: Klein.Buch zeigt dann "Die formale Prüfung (KoSIT) war nicht
möglich" und Du kannst die Rechnung ohne Prüfung erfassen. Was hinter
der Meldung steckt, steht im Kapitel "Sidecar-Probleme".

## Die Prüfung meldet formale Mängel

Das ist keine Sperre. Der Fehler liegt beim Absender, Du bist nicht
verpflichtet, die Rechnung abzulehnen. Du kannst sie erfassen,
Klein.Buch speichert den Befund mit. Mit "Prüf-Details anzeigen"
siehst Du die einzelnen Regeln, die Du dem Absender weitergeben
kannst.

## Eine Rechnung taucht doppelt in den Kosten auf

Klein.Buch prüft nicht, ob Du dieselbe Rechnung schon einmal
hochgeladen hast. Jede Übernahme ist ein neuer Eingangsbeleg. Lösche
keinen davon (Belege löschen ist gesetzlich verboten), sondern
storniere den überzähligen Beleg in der Kosten-Position über "Kosten
stornieren". Willst Du eine schon übernommene Rechnung nur noch einmal
ansehen, nimm in der Kosten-Position "Roh-XML anzeigen" oder "Beleg
herunterladen" statt eines neuen Uploads.

## Wenn nichts hilft

Kopiere die Meldung und melde den Fall im Klein.Buch-Repository auf
GitHub. Den Link zum Quellcode findest Du im "Über"-Dialog (Hauptmenü
unten neben "Hilfe"). Hänge die Rechnung selbst nicht an, sie enthält
persönliche Daten.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
