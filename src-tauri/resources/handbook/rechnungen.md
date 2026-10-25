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

1. **Entwurf** — noch nicht ausgestellt. Ein verworfener Entwurf trägt
   zusätzlich den Vermerk "verworfen".
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

## Entwurf prüfen und verwerfen

Ein Entwurf bekommt seine Rechnungsnummer schon beim Anlegen. In der
Detail-Ansicht prüft Klein.Buch die Pflichtangaben; solange Du nicht
"Rechnung ausstellen" klickst, bleibt die Rechnung Entwurf. Einen
gespeicherten Entwurf änderst Du nicht nachträglich: Stimmt etwas nicht,
verwirf ihn und leg einen neuen an.

**Verwerfen** braucht eine Bestätigung mit Deinem Passwort und einen
Grund (mindestens zehn Zeichen). Klein.Buch löscht den Entwurf nicht,
sondern vermerkt Zeitpunkt und Grund daran und im Protokoll. Die Nummer
bleibt vergeben und wird nie ein zweites Mal benutzt. So ist die Lücke
im Nummernkreis erklärt, wie es die GoBD verlangen. Ein verworfener
Entwurf lässt sich danach nicht mehr ändern und nicht ausstellen.
Stammt der Entwurf aus einem Angebot, steht das Angebot danach wieder
auf "Angenommen" und lässt sich erneut in eine Rechnung wandeln.
Eine bereits ausgestellte Rechnung verwirfst Du nicht, sondern
stornierst sie.

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
ändern. Die Rechnung hängt automatisch als ZUGFeRD-PDF an, in dem
die XRechnung-Daten bereits stecken. Klick "Senden".

Klein.Buch protokolliert jeden Versand im E-Mail-Protokoll:
Zeitpunkt, Empfänger, Antwort des Mail-Servers. Das Protokoll
findest Du unter "Einstellungen → E-Mail-Versand", oben rechts über
"E-Mail-Protokoll".

Wenn Du noch kein Postfach eingerichtet hast, lies das Kapitel
"Konten und Mail-Versand".

## Manueller Versand

Wenn Du aus Deinem normalen Mail-Programm versenden möchtest,
klick auf **"PDF herunterladen"**. Dein Browser speichert die
fertige ZUGFeRD-PDF in seinem Download-Ordner, und Du hängst sie in
Deinem Mail-Programm an. Mit **"PDF öffnen"** schaust Du die
Rechnung vorher an; die Vorschau bietet ebenfalls "Herunterladen"
und "In neuem Tab öffnen". Eine separate XRechnung-XML brauchst Du
in der Regel nicht, die XML-Daten stecken bereits im ZUGFeRD-PDF.

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

*Letzte Aktualisierung: 25.10.2026 · Klein.Buch 2026.10*
