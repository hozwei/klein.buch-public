---
slug: erste-rechnung-schreiben
title: Erste Rechnung schreiben
category: erste-schritte
order: 50
keywords: [rechnung, erste rechnung, kunde, position, ausstellen, festschreiben, xrechnung, zugferd, pdf]
---

# Erste Rechnung schreiben

Du hast die Passphrase eingerichtet, das Verkäuferprofil angelegt
und ein Backup-Ziel gewählt. Jetzt schreibst Du Deine erste
Rechnung. Dauer: etwa fünf Minuten.

## Schritt 1: Kunde anlegen

Klick im Hauptmenü auf "Kontakte" und dann auf "+ Neuer Kontakt".
Trage Name (oder Firmenbezeichnung), Anschrift und mindestens eine
E-Mail-Adresse ein. Speichere.

Du kannst Kontakte auch direkt aus dem Rechnungs-Formular heraus
anlegen, wenn der Kunde dort noch nicht in der Auswahl-Liste steht.

## Schritt 2: Rechnung anlegen

Klick im Hauptmenü auf "Rechnungen" und dann auf "+ Neue
Rechnung". Klein.Buch zeigt das Rechnungs-Formular: Kopfdaten oben,
Positionen in der Mitte, Aktionen unten.

In den Kopfdaten wählst Du den Kunden aus der Auswahl-Liste. Das
Belegdatum steht standardmäßig auf heute. Das Leistungsdatum ist
Pflichtfeld nach §14 UStG — wenn Du es leer lässt, übernimmt
Klein.Buch automatisch das Belegdatum als Fallback. Die
Rechnungsnummer vergibt Klein.Buch automatisch in der Form
`JJJJ-NNNN` (zum Beispiel `2026-0001`, fortlaufend pro
Geschäftsjahr).

## Schritt 3: Positionen hinzufügen

Klick auf "+ Position". Trage Titel, Beschreibung, Menge und
Einzelpreis ein. Die Summe rechnet Klein.Buch automatisch aus.

Wenn Du als Kleinunternehmer arbeitest, bleibt das Feld
"Umsatzsteuer" gesperrt und auf 0. Versuchst Du, es zu öffnen,
warnt Klein.Buch Dich. Das schützt Dich vor einer versehentlichen
falschen Umsatzsteuer-Angabe, die nach §14c UStG dazu führen
würde, dass Du die ausgewiesene Umsatzsteuer tatsächlich ans
Finanzamt abführen musst.

Klick erneut auf "+ Position" für jede weitere Position. Du kannst
Positionen mit Pfeil-Knöpfen umsortieren oder mit dem
Papierkorb-Knopf entfernen, solange die Rechnung Entwurf ist.

Wenn Du bereits Pakete oder eine Anfahrt-Vorlage angelegt hast,
findest Du die Knöpfe **"+ Paket"** und **"+ Anfahrt"** über der
Positions-Liste.

## Schritt 4: Pflichtangaben prüfen

Klein.Buch prüft die wichtigsten Pflichtangaben automatisch, sobald
Du auf "Rechnung ausstellen" klickst (Verkäufer-Daten,
Empfänger-Daten, Datum, mindestens eine Position). Falls etwas
fehlt, benennt die App das fehlende Feld in einem roten
Hinweis-Toast. Korrigiere und versuche erneut.

Die §19-Klausel "Gemäß §19 UStG wird keine Umsatzsteuer
ausgewiesen." setzt Klein.Buch automatisch in das PDF und in die
XRechnung-XML. Du musst sie nicht selbst eintippen.

## Schritt 5: Ausstellen (Festschreiben)

Klick auf **"Rechnung ausstellen"**. Klein.Buch fragt einmal nach,
ob Du sicher bist. Bestätige. Ab diesem Moment kannst Du Beträge,
Kunde, Datum und Positionen nicht mehr ändern. Eine Korrektur geht
nur noch über einen Storno (siehe Kapitel "Storno statt Löschung").

Klein.Buch erzeugt jetzt das endgültige PDF inklusive eingebetteter
XRechnung-XML (ZUGFeRD-PDF/A-3). Beides liegt ab sofort im
write-once-Archiv. Außerdem läuft das Auto-Backup an und sichert
den neuen Stand.

## Schritt 6: PDF prüfen und versenden

Nach dem Ausstellen siehst Du in der PageBar die Knöpfe **"PDF
öffnen"** (zeigt die fertige Rechnung im PDF-Viewer) und **"Im
Ordner zeigen"** (öffnet den Archiv-Ordner). Schau, ob alle
Angaben stimmen.

Zum Versenden hast Du zwei Möglichkeiten:

**Per Klein.Buch.** Wenn Du in den Einstellungen ein Postfach
eingerichtet hast (siehe Kapitel "Konten und Mail-Versand"), klick
auf **"Senden"**. Klein.Buch öffnet einen Versand-Dialog, schlägt
Betreff und Text vor und hängt die ZUGFeRD-PDF automatisch an.

**Per Mail-Programm.** Klick auf "Im Ordner zeigen". Klein.Buch
öffnet den Archiv-Ordner mit der fertigen ZUGFeRD-PDF. Hänge die
Datei in Deinem normalen Mail-Programm an und verschicke sie. Eine
separate XRechnung-XML brauchst Du nicht — die XML steckt im PDF.

## Was Klein.Buch im Hintergrund tut

Klein.Buch legt PDF und XRechnung-XML im write-once-Archiv ab. Bei
jedem Lese-Zugriff prüft die App den SHA-256-Prüfwert der Datei
gegen den in der Datenbank. Stimmen die Werte nicht überein,
schlägt Klein.Buch Alarm und benennt die Datei.

Das ist GoBD-Pflicht und schützt Dich bei einer Betriebsprüfung
vor dem Vorwurf, Du hättest Belege nachträglich verändert.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
