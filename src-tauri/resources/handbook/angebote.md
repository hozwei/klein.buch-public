---
slug: angebote
title: Angebote
category: bedienen
order: 110
keywords: [angebot, kostenvoranschlag, quote, entwurf bearbeiten, fertigstellen, fertiggestellt, annahme, abgesandt, in rechnung wandeln, verwerfen, agb]
---

# Angebote

Ein Angebot ist ein unverbindlicher Kostenvoranschlag. Du sendest
es einem Kunden, bevor Du den Auftrag annimmst. Wenn der Kunde
zustimmt, wandelst Du das Angebot in eine Rechnung.

## Liste der Angebote

Die Hauptansicht zeigt eine Tabelle mit Angebotsnummer, Datum,
Gültigkeit ("Gültig bis"), Empfänger, Brutto-Summe und Status. Die
Status-Werte sind: "Entwurf", "Fertiggestellt", "Angenommen",
"Abgelehnt", "Umgewandelt" sowie durchgestrichen "Verworfen" (ein
zurückgezogener Entwurf) und "Storniert" (ein zurückgezogenes
fertiggestelltes Angebot). Über der Tabelle filterst Du nach Jahr
und Status; die Checkbox "Abgelehnte, verworfene und stornierte
anzeigen" blendet diese Angebote ein oder aus.

## Anlegen

Klick auf "+ Neues Angebot". Wähle den Empfänger, das Angebotsdatum
und das Gültigkeitsdatum ("Gültig bis") und trage Positionen ein. Die
Positions-Logik ist identisch zur Rechnung: Beschreibung, Menge,
Einheit, Einzelpreis. Pakete und Anfahrt kannst Du wie bei
Rechnungen einfügen. Unter Regelbesteuerung wählst Du je Position
"USt-Satz" und "USt-Art" wie in der Rechnung; unter der
Kleinunternehmer-Regelung gibt es diese Felder nicht, jede Position ist
steuerfrei (Steuerart E, 0 %). Klick auf "Als Entwurf speichern".

Das Gültigkeitsdatum schlägt Klein.Buch aus der Einstellung "Angebot
gültig" in Deinen Firmendaten vor. Es steht in der Liste und am
Angebot ("gültig bis …"); abgelaufene Angebote hebt Klein.Buch nicht
gesondert hervor.

## Entwurf bearbeiten

Solange das Angebot Entwurf ist, kannst Du es bearbeiten: Mit
**"Bearbeiten"** am Entwurf änderst Du Empfänger, Angebotsdatum,
"Gültig bis", Vorlage, Positionen und Notiz; "Änderungen speichern"
übernimmt sie. Meldet die Prüfung am Entwurf, dass im Entwurf selbst
etwas fehlt, führt Dich der Link "Entwurf bearbeiten" direkt dorthin.
Lehnt Klein.Buch das Speichern ab, steht der Grund am Feld
(Angebotsdatum, Empfänger) oder über dem Formular.

Die Angebotsnummer bleibt, deshalb bleibt auch das Angebotsdatum im
Geschäftsjahr der Nummer; für ein anderes Jahr
verwirfst Du den Entwurf und legst ein neues Angebot an. Eine aus
einem Paket eingefügte Position, die Du änderst, verliert ihren
Paket-Bezug. Unter der Kleinunternehmer-Regelung lehnt Klein.Buch
schon beim Speichern jede Position mit Umsatzsteuer ab. Jede Änderung
steht im Protokoll.

Nicht bearbeiten lassen sich fertiggestellte Angebote (auch
angenommene, abgelehnte, zurückgezogene und umgewandelte) und
verworfene Entwürfe. Ein Angebot wird erst mit dem Fertigstellen
unveränderlich; vorher ist es weder Handelsbrief noch Buchungsbeleg.

Hat Klein.Buch einen Entwurf nach einer Wiederherstellung vorgemerkt
(die Nummer war vorher schon fertiggestellt, siehe "Backup und
Wiederherstellen"), passt Du ihn hier an das versandte Original an und
bestätigst danach den Abgleich; erst dann lässt er sich fertigstellen.
Speicherst Du einen schon bestätigten Entwurf erneut, ist er wieder
vorgemerkt, und Du bestätigst den Abgleich noch einmal. Darauf weist
der Editor schon vor dem Speichern hin ("Speichern hebt den Abgleich
auf.").

## AGB und Datenschutzhinweis

Sobald Du ein Angebot zum ersten Mal versendest oder das
Druck-PDF mit AGB erzeugst, heftet Klein.Buch automatisch
die zu diesem Zeitpunkt **aktiv markierte** Version Deiner AGB
und Deines Datenschutzhinweises als unveränderliche Kopie an das
Angebot. Du wählst die Version nicht pro Angebot aus — Klein.Buch
nimmt jeweils, was als aktiv hinterlegt ist. Wenn Du später die
AGB änderst, bleibt die einmal angeheftete Version am Angebot
bestehen. Welche Fassungen angeheftet sind, zeigt die Karte
"Beigefügte Dokumente (AGB & Datenschutz)" am Angebot.

Die AGB- und Datenschutz-Versionen pflegst Du in "Einstellungen →
AGB & Datenschutz". Wenn dort nicht je eine aktive Version der AGB
und des Datenschutzhinweises hinterlegt ist, kannst Du das Angebot
weder versenden noch das Druck-PDF erzeugen — leg dann zuerst die
fehlende Version an. Eine bereits an ein Angebot geheftete Version
lässt sich nicht mehr löschen.

## Fertigstellen und Versenden

Beim Öffnen eines Entwurfs prüft Klein.Buch in der Karte "Prüfung" die
Pflichtangaben und nennt, was fehlt, mit den Links "Firmendaten
ergänzen" oder "Kontakt ergänzen"; "Prüfen" wiederholt die Prüfung.

Klick auf **"Fertigstellen"**. Klein.Buch fragt einmal nach
("Angebot jetzt fertigstellen?"); mit "Fertigstellen" bestätigst Du.
Ab dann sind die Felder gesperrt, und das Angebot steht auf
"Fertiggestellt". Das PDF erzeugt Klein.Buch aus diesem festen Stand,
sobald Du es zum ersten Mal öffnest oder versendest, und legt es im
Archiv ab.

Danach ist **"Senden"** der nächste Schritt. Die Versand-Seite hängt
Angebot, AGB und Datenschutzhinweis als drei PDF-Dateien an; Du
schickst sie mit "Angebot senden" ab. Für den Versand aus Deinem
Mail-Programm holst Du das fertige Bundle (Angebot plus AGB plus
Datenschutzhinweis in einer Datei) über **"Druck-PDF (mit AGB)"** im
Menü "Mehr". Mit **"PDF öffnen"** öffnest Du nur das Angebot ohne
Bundle.

Nach dem Versand steht neben dem Status "versendet am …", und die
Karte "Versand-Historie" zeigt Zeitpunkt und Empfänger jedes
Versands. Der Status bleibt "Fertiggestellt": Er sagt, dass das
Angebot festgeschrieben ist, nicht, ob es verschickt wurde.

## Antwort des Kunden vermerken

Wenn der Kunde annimmt, klick auf **"Annehmen"**. Nach dem Versand
ist das der Hauptknopf; vorher steht er neben "Senden" (auf dem Handy
im Menü "Mehr"). Klein.Buch öffnet den Bereich "Angebot annehmen",
in dem Du das Annahmedatum einträgst und optional den
unterschriebenen Vertrag als PDF oder Bild anhängst (Klein.Buch
hängt ihn dauerhaft an das Angebot; er steht danach in der Karte
"Anhänge"). Bestätige mit **"Annahme speichern"**.

Wenn der Kunde ablehnt, klick auf **"Ablehnen"**. Im Bereich
"Angebot ablehnen" kannst Du einen Ablehnungsgrund eintragen; "Als
abgelehnt markieren" speichert. Beide Status sind Notizen ohne
direkte buchhalterische Wirkung.

## Angebot zurückziehen

Ein fertiggestelltes Angebot, das nicht mehr gilt, ziehst Du über das
Menü "Mehr" mit **"Stornieren"** zurück, einen Entwurf, den Du nicht
mehr brauchst, mit **"Entwurf verwerfen"**. Klein.Buch verlangt in
beiden Fällen einen Grund mit mindestens zehn Zeichen und löscht
nichts: Das Angebot behält seine Nummer und steht danach als
"Storniert" bzw. "Verworfen" mit Zeitpunkt und Grund in der Liste und
im Protokoll. Zurückziehen ist endgültig. Ein Angebot, das
schon in eine Rechnung umgewandelt wurde, ziehst Du nicht mehr zurück;
dort korrigierst Du die Rechnung.

## In Rechnung umwandeln

Bei einem angenommenen Angebot ist **"In Rechnung umwandeln"** der
nächste Schritt. Auf der folgenden Seite passt Du Rechnungsdatum,
Leistungsdatum oder Leistungszeitraum ("Leistung", wie bei einer neuen
Rechnung), Fälligkeit und Positionen noch an; für eine Behörde trägst
Du dort auch die "Leitweg-ID (optional)" ein. **"Rechnung erstellen"**
legt eine neue Rechnung mit einem Querverweis auf das
Angebot an. Die Rechnung ist zunächst ein Entwurf; ausgestellt wird
erst die Rechnung selbst.

Das ursprüngliche Angebot bleibt unverändert sichtbar. Klein.Buch
zeigt am Angebot, in welche Rechnung es gewandelt wurde (Status
"Umgewandelt", Knopf "Zur Rechnung").
Verwirfst Du den Rechnungs-Entwurf, der aus dem Angebot entstanden ist,
steht das Angebot wieder auf "Angenommen" und lässt sich erneut
umwandeln. Klein.Buch vermerkt das im Protokoll.

## Pflichtangaben

Angebote brauchen keine §14-Pflichtangaben, weil sie keine
Rechnungen sind. Trotzdem druckt Klein.Buch die wichtigsten Daten
zur Identifikation: Deine Adresse, die Kunden-Adresse, das Datum
und das Verkäuferprofil. Wenn Du Kleinunternehmer bist, druckt
Klein.Buch zur Klarheit auch im Angebot den Hinweis auf §19 UStG.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
