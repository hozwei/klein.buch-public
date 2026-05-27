---
slug: rechnungen
title: Rechnungen
category: bedienen
order: 100
keywords: [rechnung, ausgangsrechnung, ausstellen, storno, xrechnung, zugferd, pdf, versand]
---

# Rechnungen

Im Bereich "Rechnungen" verwaltest Du alle Deine Ausgangsrechnungen.
Du legst sie an, stellst sie aus, verschickst sie, erfasst Zahlungen
und stornierst sie bei Fehlern. Die Erst-Anlage steht im Kapitel
"Erste Rechnung schreiben"; hier geht es um die laufende Bedienung.

## Liste der Rechnungen

Die Hauptansicht zeigt Rechnungsnummer, Datum, Kunde, Brutto-Betrag
und Status. Die Status-Werte sind:

1. **Entwurf** — noch nicht ausgestellt, alles editierbar.
2. **Ausgestellt** — festgeschrieben, PDF und XRechnung-XML im
   Archiv, nichts mehr änderbar (außer Versand und Zahlung).
3. **Versendet** — die Rechnung wurde per Klein.Buch-Mail
   verschickt; der Versand steht im Protokoll.
4. **Teilzahlung** — eine erste Zahlung ist eingegangen, der
   Restbetrag fehlt.
5. **Bezahlt** — die Summe ist vollständig eingegangen.
6. **Storniert** — durch eine Storno-Rechnung neutralisiert.

Über der Tabelle filterst Du nach Geschäftsjahr und Status und
blendest mit der Checkbox "Stornos anzeigen" die Storno-Belege ein.

## Entwurf bearbeiten

Solange eine Rechnung den Status "Entwurf" hat, kannst Du alle
Felder ändern: Kunde, Datum, Positionen, Preise. Klick auf die
Zeile in der Liste, ändere im Formular und speichere. Solange Du
nicht "Rechnung ausstellen" klickst, bleibt sie Entwurf.

## Ausstellen (Festschreiben)

Wenn die Rechnung fertig ist, klick auf **"Rechnung ausstellen"**.
Klein.Buch prüft die Pflichtangaben, erzeugt das PDF und die
XRechnung-XML und legt das Original im Archiv ab. Ab diesem Moment
sind Beträge, Kunde, Datum und Positionen gesperrt. Eine Korrektur
geht nur über einen Storno. Diese Sperre kommt aus der GoBD und
ist die "Festschreibung" im rechtlichen Sinn.

## Versand per E-Mail

Bei ausgestellten Rechnungen siehst Du den Knopf **"Senden"** in
der PageBar. Klick öffnet eine eigene Versand-Seite. Empfänger
ist voreingestellt aus dem Kontakt; Betreff und Text kannst Du
ändern. PDF und XRechnung hängen automatisch dran. Klick "Senden".

Klein.Buch protokolliert jeden Versand im Mail-Protokoll: Datum,
Empfänger, Provider-Antwort. Das Protokoll findest Du unter
"Einstellungen → E-Mail-Versand".

Wenn Du noch kein Postfach eingerichtet hast, lies das Kapitel
"Konten und Mail-Versand".

## Manueller Versand

Wenn Du aus Deinem normalen Mail-Programm versenden möchtest,
klick auf **"PDF öffnen"** oder **"Im Ordner zeigen"**. Klein.Buch
zeigt Dir entweder die fertige ZUGFeRD-PDF im PDF-Viewer oder
öffnet den Archiv-Ordner im Datei-Explorer. Eine separate
XRechnung-XML brauchst Du in der Regel nicht — die XML-Daten
stecken bereits im ZUGFeRD-PDF.

## Zahlung erfassen

Sobald der Kunde bezahlt hat, klick in der Detail-Ansicht oben
rechts auf **"+ Zahlung"**. Mehr dazu im Kapitel "Zahlung
erfassen".

## Storno

Bei einer fehlerhaften Rechnung erzeugst Du einen Storno-Beleg.
Klick auf **"Stornieren"** in der Detail-Ansicht. Klein.Buch öffnet
ein Storno-Panel: trage einen Storno-Grund ein und bestätige.

Klein.Buch erzeugt einen neuen Beleg mit umgekehrtem Vorzeichen
und der Nummer "ST-JJJJ-NNNN" (zum Beispiel `ST-2026-0001`). Beide
Belege bleiben sichtbar. Du schickst dem Kunden den Storno und
gegebenenfalls eine neue korrekte Rechnung. Mehr dazu im Kapitel
"Storno statt Löschung".

## Wiederkehrende Rechnungen

Für echte Abo-Rechnungen (jeden Monat, jedes Quartal) klickst Du
in der Rechnungs-Liste oben rechts auf **"Wiederkehrende
Rechnungen"** und legst dort eine Vorlage an. Mehr dazu im Kapitel
"Abo-Rechnungen".

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
