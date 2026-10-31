---
slug: erste-rechnung-schreiben
title: Erste Rechnung schreiben
category: erste-schritte
order: 50
keywords: [rechnung, erste rechnung, kunde, position, einheit, fälligkeit, entwurf, ausstellen, festschreiben, leitweg-id, xrechnung, zugferd, pdf]
---

# Erste Rechnung schreiben

Du hast die Passphrase eingerichtet, das Verkäuferprofil angelegt
und ein Backup-Ziel gewählt. Jetzt schreibst Du Deine erste
Rechnung. Dauer: etwa fünf Minuten.

Fehlen Deine Firmendaten noch, zeigt das Rechnungs-Formular statt
der Felder den Hinweis "Deine Firmendaten fehlen." mit dem Link
"Jetzt ausfüllen" (siehe Kapitel "Verkäuferprofil anlegen").

## Schritt 1: Kunde anlegen

Klick im Hauptmenü auf "Kontakte" und dann auf "+ Neuer Kontakt".
Trage Name (oder Firmenbezeichnung), Anschrift und eine
E-Mail-Adresse ein, an die Klein.Buch die Rechnung schicken kann.
Klick auf "Kontakt anlegen".

Leg den Kunden vorher an: Im Rechnungs-Formular wählst Du nur aus
den vorhandenen Kontakten aus.

## Schritt 2: Rechnung anlegen

Klick im Hauptmenü auf "Rechnungen" und dann auf "+ Neue
Rechnung". Klein.Buch zeigt das Formular "Neue Rechnung" mit den
Karten "Allgemein", "Positionen", "Notiz" und "Bezahlung".
Pflichtfelder tragen einen Stern.

In "Allgemein" wählst Du den Empfänger aus der Auswahl-Liste. Ab acht
Kontakten steht darüber ein Suchfeld ("Suchen: Name oder Ort"), das
die Liste eingrenzt. Das Rechnungsdatum steht auf heute.

Unter "Leistung" wählst Du zwischen **"Datum"** (ein Leistungsdatum)
und **"Zeitraum (von–bis)"** (ein Leistungszeitraum). Das
Leistungsdatum ist Pflichtangabe nach §14 UStG; Klein.Buch belegt es
mit dem Rechnungsdatum vor und zieht es mit, bis Du es selbst
änderst. Für Leistungen über eine Zeitspanne (zum Beispiel einen
Monat Beratung) wählst Du "Zeitraum (von–bis)" und trägst "Beginn"
und "Ende" ein; das Ende darf nicht vor dem Beginn liegen. Beides
darf nach dem Rechnungsdatum liegen: Klein.Buch zeigt dann unter dem
Feld den Hinweis "Rechnung vor der Leistung" (Vorausrechnung, siehe
Glossar). Das ist erlaubt und kein Fehler.

Auch die Fälligkeit ist Pflicht, weil die E-Rechnung sie verlangt;
Klein.Buch schlägt sie aus dem Zahlungsziel in Deinen Firmendaten
vor. Die Währung steht fest auf "Euro (EUR)". Schreibst Du an eine
Behörde, trägst Du unten in der Karte "Allgemein" die "Leitweg-ID
(optional)" ein (siehe Schritt 5); sonst bleibt das Feld leer.

Die Rechnungsnummer vergibt Klein.Buch automatisch beim Speichern des
Entwurfs in der Form `RE-JJJJ-NNNN` (zum Beispiel `RE-2026-0001`,
fortlaufend pro Geschäftsjahr).

## Schritt 3: Positionen hinzufügen

In der Karte "Positionen" steht schon eine leere Position. Trage
Beschreibung, Menge, Einheit und Einzelpreis ein. Die Menge steht
auf 1, die Einheit auf "Stück"; zur Wahl stehen Einheiten im
Klartext wie "Stunde", "Tag", "Pauschale" oder "Kilometer". Der
Einzelpreis ist anfangs leer. Beträge tippst Du wie gewohnt mit Komma
vor den Cent und auf Wunsch mit Punkt als Tausendertrenner, zum
Beispiel 1.234,56. Ein Minuszeichen nimmt das Feld nicht an. Netto
und Brutto rechnet Klein.Buch automatisch aus.

Gilt am Leistungsdatum der Rechnung die Kleinunternehmer-Regelung,
blendet Klein.Buch die Umsatzsteuer-Felder aus und sagt das in einem
Hinweis über dem Formular. Das schützt Dich vor einer versehentlichen
falschen Umsatzsteuer-Angabe, die nach §14c UStG dazu führen
würde, dass Du die ausgewiesene Umsatzsteuer tatsächlich ans
Finanzamt abführen musst.

Klick auf "+ Position" für jede weitere Position. Mit dem Knopf
"×" am Ende einer Zeile entfernst Du eine Position, solange es mehr
als eine gibt.

Wenn Du bereits Pakete oder einen Kilometersatz für die Anfahrt
angelegt hast, findest Du unter der Positions-Liste die Auswahl
"Paket" mit dem Knopf **"+ Paket"** und das Feld "Anfahrt (km)" mit
dem Knopf **"+ Anfahrt"**. Trägt eine Position aus einem älteren
Paket eine Einheit, die nicht in der Liste steht (etwa "Pauschal"),
meldet Klein.Buch das am Feld und schlägt eine passende Einheit vor,
zum Beispiel "Pauschale". Die E-Rechnung kennt nur die Einheiten der
Liste.

## Schritt 4: Entwurf speichern und Pflichtangaben prüfen

Klick auf **"Als Entwurf speichern"**, oben auf der Seite oder am
Ende des Formulars. Fehlt etwas oder passt etwas nicht, steht die
Meldung direkt am Feld, und der Cursor springt zum ersten
betroffenen Feld. Ein leerer Einzelpreis oder ein Gesamtbetrag von
0 € ist ein Fehler am Preisfeld.

Nach dem Speichern öffnet Klein.Buch den Entwurf. Unter den
Positionen steht die Karte "Ausstellen". Dort prüft Klein.Buch sofort
die wichtigsten Pflichtangaben (Firmendaten samt Steuernummer und
Telefonnummer, Empfänger-Daten, Datum, Fälligkeitsdatum, mindestens
eine Position) und nennt, was noch fehlt, jeweils mit einem Link zur
Korrektur ("Firmendaten ergänzen", "Kontakt ergänzen" oder "Entwurf
bearbeiten"). Nach einer Korrektur prüfst Du mit "Prüfen" oben auf
der Seite erneut. Telefon und Steuernummer (oder USt-IdNr.) brauchst
Du erst zum Ausstellen: Die Firmendaten lassen sich auch ohne sie
speichern, Klein.Buch weist dann am Feld darauf hin; das Feld
"Telefon" trägt den Zusatz "(Pflicht für die E-Rechnung)".

Die §19-Klausel "Gemäß §19 UStG wird keine Umsatzsteuer
ausgewiesen." setzt Klein.Buch automatisch in das PDF und in die
XRechnung-XML. Du musst sie nicht selbst eintippen.

## Schritt 5: Ausstellen (Festschreiben)

Die Leitweg-ID brauchst Du nur, wenn Dein Auftraggeber eine Behörde
ist; sonst lässt Du das Feld leer. Du trägst sie schon im Entwurf ein
(Karte "Allgemein"), der Entwurf speichert sie. Aufgebaut ist sie aus
Grobadressierung, optional Feinadressierung und einer zweistelligen
Prüfziffer, durch Bindestriche getrennt (zum Beispiel
04011000-1234512345-06). In der Karte "Ausstellen" steht sie noch einmal
vorbelegt: Ein geänderter Wert gilt für diese E-Rechnung, ein leeres
Feld lässt die gespeicherte gelten. Entfernen lässt sie sich über
"Entwurf bearbeiten". Beim Ausstellen übernimmt Klein.Buch sie als
Käuferreferenz in die E-Rechnung.

Klick auf **"Rechnung ausstellen"**. Klein.Buch fragt einmal nach
("Rechnung jetzt ausstellen?"). Bestätige mit "Ausstellen". Ab
diesem Moment kannst Du Beträge, Kunde, Datum und Positionen nicht
mehr ändern. Eine Korrektur geht nur noch über einen Storno (siehe
Kapitel "Storno statt Löschung").

Klein.Buch erzeugt jetzt das endgültige PDF inklusive eingebetteter
XRechnung-XML (ZUGFeRD-PDF/A-3). Beides liegt ab sofort im
write-once-Archiv. Außerdem läuft das Auto-Backup an und sichert
den neuen Stand.

## Schritt 6: PDF prüfen und versenden

Nach dem Ausstellen steht oben auf der Seite **"Senden"** als
nächster Schritt, daneben **"PDF öffnen"** (zeigt die fertige
Rechnung in einer Vorschau). **"PDF herunterladen"** (Dein Browser
speichert die Datei in seinem Download-Ordner) findest Du im Menü
"Mehr". Schau, ob alle Angaben stimmen. Auf dem Handy öffnet "PDF
öffnen" die PDF-Ansicht Deines Browsers.

Zum Versenden hast Du zwei Möglichkeiten:

**Per Klein.Buch.** Wenn Du in den Einstellungen ein Postfach
eingerichtet hast (siehe Kapitel "Konten und Mail-Versand"), klick
auf **"Senden"**. Klein.Buch öffnet die Versand-Seite, schlägt
Betreff und Text vor und hängt die ZUGFeRD-PDF automatisch an. Klick
dort noch einmal auf "Senden".

**Per Mail-Programm.** Wähl im Menü "Mehr" den Eintrag "PDF
herunterladen". Dein Browser speichert die fertige ZUGFeRD-PDF. Hänge
die Datei in Deinem normalen Mail-Programm an und verschicke sie.
Eine separate XRechnung-XML brauchst Du nicht — die XML steckt im
PDF.

## Was Klein.Buch im Hintergrund tut

Klein.Buch legt PDF und XRechnung-XML im write-once-Archiv ab. Bei
jedem Lese-Zugriff prüft die App den SHA-256-Prüfwert der Datei
gegen den in der Datenbank. Stimmen die Werte nicht überein,
schlägt Klein.Buch Alarm und benennt die Datei.

Das ist GoBD-Pflicht und schützt Dich bei einer Betriebsprüfung
vor dem Vorwurf, Du hättest Belege nachträglich verändert.

---

*Letzte Aktualisierung: 31.10.2026 · Klein.Buch 2026.11*
