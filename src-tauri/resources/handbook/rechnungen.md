---
slug: rechnungen
title: Rechnungen
category: bedienen
order: 100
keywords: [rechnung, ausgangsrechnung, ausstellen, storno, storno-beleg, xrechnung, zugferd, pdf, versand, leitweg-id, überfällig, teilbezahlt, entwurf verwerfen]
---

# Rechnungen

Im Bereich "Rechnungen" verwaltest Du alle Deine Ausgangsrechnungen.
Du legst sie an, stellst sie aus, verschickst sie, erfasst Zahlungen
und stornierst sie bei Fehlern. Die Erst-Anlage steht im Kapitel
"Erste Rechnung schreiben"; hier geht es um die laufende Bedienung.

## Liste der Rechnungen

Die Hauptansicht zeigt je Rechnung Nummer, Datum, Empfänger,
Brutto-Betrag, bisher bezahlten Betrag, Status und Fälligkeit. Ein
Storno-Beleg trägt neben seiner Nummer das Merkmal "Storno-Beleg".
Die Status-Werte sind:

1. **Entwurf** — noch nicht ausgestellt.
2. **Ausgestellt** — festgeschrieben, PDF und XRechnung-XML im
   Archiv, nichts mehr änderbar (außer Versand und Zahlung).
3. **Versendet** — die Rechnung wurde per Klein.Buch-Mail
   verschickt; der Versand steht im E-Mail-Protokoll.
4. **Teilbezahlt** — eine erste Zahlung ist eingegangen, der
   Restbetrag fehlt.
5. **Überfällig** — die Rechnung ist ausgestellt, ein Betrag ist
   offen und die Fälligkeit liegt vor dem heutigen Tag. Dieser Status
   steht dann anstelle von "Ausgestellt", "Versendet" oder
   "Teilbezahlt", bis die Rechnung bezahlt ist.
6. **Bezahlt** — die Summe ist vollständig eingegangen.
7. **Storniert** — durch einen Storno-Beleg neutralisiert; der
   Status steht durchgestrichen.
8. **Verworfen** — ein verworfener Entwurf (siehe unten), ebenfalls
   durchgestrichen.

Über der Tabelle wählst Du Jahr und Status aus (auch "Überfällig")
und blendest mit der Checkbox "Stornos anzeigen" stornierte Rechnungen
und Storno-Belege ein oder aus; "Filtern" wendet die Auswahl an.
Findet der Filter nichts, bringt Dich "Filter zurücksetzen" zur
vollständigen Liste zurück. Mit "Öffnen" in einer Zeile gelangst Du
zur Detail-Ansicht der Rechnung.

## Entwurf prüfen, bearbeiten und verwerfen

Ein Entwurf bekommt seine Rechnungsnummer schon beim Anlegen.
Solange Du nicht "Rechnung ausstellen" klickst, bleibt die Rechnung
Entwurf.

Unter den Positionen eines Entwurfs steht die Karte "Ausstellen".
Beim Öffnen des Entwurfs prüft Klein.Buch dort die Pflichtangaben.
Ist alles vollständig, steht dort "Alle Pflichtangaben sind vorhanden
— die Rechnung kann ausgestellt werden."; sonst listet die Karte auf,
was noch fehlt. Hinter jedem Punkt steht ein Link zum Ort der
Korrektur: "Firmendaten ergänzen", "Kontakt ergänzen" oder "Entwurf
bearbeiten". Mit "Prüfen" oben auf der Seite wiederholst Du die
Prüfung, zum Beispiel nachdem Du Deine Firmendaten ergänzt hast.

Mit **"Bearbeiten"** oben auf der Seite öffnest Du die Seite "Entwurf
bearbeiten" mit demselben Formular wie bei einer neuen Rechnung und
änderst Empfänger, Daten, Positionen, Notiz und Bezahlt-Hinweis.
"Änderungen speichern" übernimmt sie. Die Rechnungsnummer bleibt,
deshalb bleibt auch das Rechnungsdatum im Geschäftsjahr der Nummer;
für ein anderes Jahr verwirfst Du den Entwurf und legst einen neuen
an. Eine aus einem Paket eingefügte Position, die Du änderst, verliert
ihren Paket-Bezug und gilt danach als eigene Position. Stammt der
Entwurf aus einem Angebot, bleibt der Empfänger der des Angebots.
Nicht bearbeiten lassen sich ausgestellte Rechnungen, verworfene
Entwürfe, Storno-Entwürfe und Entwürfe, die ein Abo-Lauf gerade
festschreibt. Jede Änderung steht im Protokoll.

**"Entwurf verwerfen"** findest Du in der Detail-Ansicht des Entwurfs
im Menü "Mehr". Es öffnet unter dem Kopf der Seite den Bereich
"Entwurf verwerfen": Trag einen Grund ein (mindestens zehn Zeichen,
schreib keine Namen von Personen hinein) und klick "Entwurf
verwerfen". Zur Bestätigung fragt Klein.Buch Daten-Passwort und
App-Code ab. Klein.Buch löscht den Entwurf nicht,
sondern vermerkt Zeitpunkt und Grund daran und im Protokoll. Die Nummer
bleibt vergeben und wird nie ein zweites Mal benutzt. So ist die Lücke
im Nummernkreis erklärt, wie es die GoBD verlangen. Ein verworfener
Entwurf lässt sich danach nicht mehr ändern und nicht ausstellen; in
der Liste steht er als "Verworfen".
Stammt der Entwurf aus einem Angebot, steht das Angebot danach wieder
auf "Angenommen" und lässt sich erneut in eine Rechnung wandeln.
Eine bereits ausgestellte Rechnung verwirfst Du nicht, sondern
stornierst sie.

## Ausstellen (Festschreiben)

Die Leitweg-ID ist die Kennung, unter der eine Behörde E-Rechnungen
entgegennimmt; Klein.Buch schreibt sie als Käuferreferenz in die
E-Rechnung. Du trägst sie im Entwurf ein (Feld "Leitweg-ID
(optional)" in der Karte "Allgemein"), der Entwurf speichert sie. In
der Karte "Ausstellen" steht sie vorbelegt; ein Wert, den Du dort
änderst, ersetzt den gespeicherten. Ein leeres Feld dort lässt die
gespeicherte gelten; entfernen lässt sie sich über "Entwurf
bearbeiten". Ist Dein Auftraggeber keine Behörde, lässt Du das Feld
leer.

Wenn die Rechnung fertig ist, klick auf **"Rechnung ausstellen"**
(oben auf der Seite oder am Ende der Karte "Ausstellen"). Klein.Buch
fragt einmal nach ("Rechnung jetzt ausstellen?"); mit "Ausstellen"
bestätigst Du. Klein.Buch prüft die Pflichtangaben, erzeugt das PDF
und die XRechnung-XML und legt das Original im Archiv ab. Ab diesem
Moment sind Beträge, Kunde, Datum und Positionen gesperrt. Eine
Korrektur geht nur über einen Storno. Diese Sperre kommt aus der GoBD
und ist die "Festschreibung" im rechtlichen Sinn.

Lehnt Klein.Buch das Ausstellen ab, steht in der Karte "Ausstellen"
der Hinweis "Die Rechnung wurde nicht ausgestellt." mit den Gründen
und den Links zur Korrektur.

## Versand per E-Mail

Nach dem Ausstellen ist **"Senden"** der nächste Schritt und steht
als Hauptknopf oben auf der Seite. Er öffnet die Seite "Rechnung
RE-… versenden". Dort wählst Du das Postfach; der Empfänger ist aus
dem Kontakt voreingestellt, Betreff und Nachricht kannst Du ändern.
Die Rechnung hängt automatisch als ZUGFeRD-PDF an, in dem die
XRechnung-Daten bereits stecken. Klick "Senden". Schickst Du an eine
andere Adresse als die beim Kontakt hinterlegte, fragt Klein.Buch zur
Bestätigung Daten-Passwort und App-Code ab.

Nach dem Versand steht neben dem Status "versendet am …", und die
Karte "Versand-Historie" zeigt jeden Versuch. Klein.Buch protokolliert
jeden Versand im E-Mail-Protokoll: Zeitpunkt, Empfänger, Antwort des
Mail-Servers. Das vollständige Protokoll findest Du unter
"Einstellungen → E-Mail-Protokoll".

Wenn Du noch kein Postfach eingerichtet hast, lies das Kapitel
"Konten und Mail-Versand".

## Manueller Versand

Wenn Du aus Deinem normalen Mail-Programm versenden möchtest,
wähl im Menü "Mehr" den Eintrag **"PDF herunterladen"**. Dein Browser
speichert die fertige ZUGFeRD-PDF in seinem Download-Ordner, und Du
hängst sie in Deinem Mail-Programm an. Mit **"PDF öffnen"** schaust
Du die Rechnung vorher an; die Vorschau bietet ebenfalls
"Herunterladen" und "In neuem Tab öffnen". Eine separate
XRechnung-XML brauchst Du in der Regel nicht, die XML-Daten stecken
bereits im ZUGFeRD-PDF. Einen Versand außerhalb von Klein.Buch kennt
die App nicht; "Senden" bleibt deshalb der Hauptknopf.

## Zahlung erfassen

Sobald der Kunde bezahlt hat, klick in der Detail-Ansicht auf
**"+ Zahlung"**. Nach dem Versand ist das der Hauptknopf oben auf der
Seite; vorher steht er neben "Senden" (auf dem Handy im Menü "Mehr").
Gebuchte Zahlungen stehen in der Karte "Zahlungen" mit Zahldatum,
Betrag, Konto und Notiz. Eine falsch erfasste Zahlung stornierst Du dort
über "Mehr" → "Zahlung stornieren …": Klein.Buch bucht eine
Gegenbuchung mit Grund, die Zahlung bleibt als "Storniert" sichtbar.
Mehr dazu im Kapitel "Zahlung erfassen", Abschnitt "Falsch erfasste
Zahlung stornieren".

Einen Bezahlt-Hinweis aus dem Entwurf zeigt die Karte "Daten". Ist
trotzdem noch ein Betrag offen, erinnert der Hinweis "Bezahlt-Hinweis
ohne Buchung." daran, dass er nur Text auf dem Beleg ist. Erst
"+ Zahlung" bucht die Zahlung, und erst dann zählt sie in der EÜR.

## Storno

Bei einer fehlerhaften Rechnung erzeugst Du einen Storno-Beleg.
Wähl in der Detail-Ansicht im Menü "Mehr" den Eintrag
**"Stornieren"**. Klein.Buch öffnet die Seite "Rechnung RE-…
stornieren": Trag Storno-Datum und Grund ein und klick "Storno-Beleg
erstellen". Eine Rückfrage ("Rechnung RE-… stornieren?") nennt die
Folgen; erst "Stornieren" darin legt den Storno-Beleg an. Das lässt
sich nicht rückgängig machen.

Klein.Buch erzeugt einen neuen Beleg mit umgekehrtem Vorzeichen
und der Nummer "ST-JJJJ-NNNN" (zum Beispiel `ST-2026-0001`) und
öffnet ihn; dort ist "Senden" der nächste Schritt. Beide
Belege bleiben sichtbar und verweisen aufeinander. Hat die Rechnung
eine Leitweg-ID, übernimmt der Storno-Beleg sie in die E-Rechnung;
so ordnet die Behörde ihn ihrer Rechnung zu. Du schickst dem
Kunden den Storno-Beleg und gegebenenfalls eine neue korrekte
Rechnung. Mehr dazu im Kapitel "Storno statt Löschung".

## Abo-Rechnungen

Für echte Abo-Rechnungen (jeden Monat, jedes Quartal) klickst Du
in der Rechnungs-Liste oben auf **"Abo-Rechnungen"** (auf dem Handy
im Menü "Mehr") und legst dort eine Abo-Vorlage an. Mehr dazu im
Kapitel "Abo-Rechnungen".

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
