---
slug: kontakte
title: Kontakte
category: bedienen
order: 160
keywords: [kontakte, kunde, lieferant, adressbuch, dsgvo, anonymisierung, archivieren, kontaktdaten]
---

# Kontakte

Im Bereich "Kontakte" verwaltest Du alle Personen und Firmen, mit
denen Du geschäftlich zu tun hast: Kunden, Lieferanten und alle
anderen Geschäftspartner. Die Daten werden in Rechnungen, Angeboten
und Kosten-Positionen referenziert.

## Liste der Kontakte

Die Hauptansicht zeigt Name, Typ (Kunde, Lieferant, Kunde +
Lieferant oder Partner), Ort, USt-IdNr. und E-Mail. Klick auf den
Namen oder auf "Öffnen", um den Kontakt zu öffnen.

Über der Liste findest Du ein Suchfeld (durchsucht Name, E-Mail,
USt-IdNr., Ort und Postleitzahl) mit dem Knopf "Suchen" und das
Häkchen "Archivierte anzeigen", das archivierte Kontakte mit dem
Vermerk "Archiviert" einblendet. Darunter steht, wie viele Kontakte
zur Suche passen, zum Beispiel "3 Kontakte zu „Müller“.". Findet die
Suche nichts, setzt "Suche zurücksetzen" sie zurück.

## Neuen Kontakt anlegen

Klick auf "+ Neuer Kontakt". Wähle den Typ (Kunde, Lieferant, Kunde +
Lieferant oder Partner) und trage Name oder Firmenbezeichnung,
Anschrift, E-Mail-Adresse und optional Telefon ein. Pflicht sind die
Felder mit Stern: Name, Straße + Nr., PLZ, Stadt und Land.

Bei Geschäftskunden im EU-Ausland trägst Du die Umsatzsteuer-
Identifikations-Nummer ein. Bei Endverbrauchern ohne USt-IdNr
lässt Du das Feld leer.

Klick auf "Kontakt anlegen". Klein.Buch legt den Kontakt an, meldet
"Kontakt angelegt." und öffnet ihn. Du kannst ihn ab sofort in
Rechnungen und Angeboten auswählen. "Abbrechen" führt ohne Speichern
zur Liste zurück.

Klein.Buch prüft die Angaben vor dem Speichern und zeigt einen Fehler
direkt am Feld, jeweils mit einem Beispiel; der Cursor springt in das
erste betroffene Feld. Geprüft wird:

1. das Land als Kürzel aus 2 Buchstaben, zum Beispiel DE oder AT,
2. bei Land DE eine PLZ aus 5 Ziffern,
3. eine eingetragene USt-IdNr.: in Deutschland "DE" und 9 Ziffern,
   sonst Ländercode und danach mindestens 8 Zeichen,
4. eine eingetragene E-Mail-Adresse auf vollständige Form, zum
   Beispiel name@example.org,
5. eine eingetragene IBAN auf ihre Länge (15 bis 34 Zeichen, in
   Deutschland 22).

Strenger prüft Klein.Buch nicht: Prüfziffer der IBAN und Form der BIC
bleiben ungeprüft.

Dein Browser füllt die Felder eines Kontakts nicht automatisch mit
Deinen eigenen Daten aus. Das ist Absicht, denn ein Kontakt enthält
die Daten eines anderen.

## Kontakt bearbeiten

Öffne einen Kontakt, ändere seine Kontaktdaten und klick oben auf
"Änderungen speichern". Klein.Buch meldet "Kontakt gespeichert.".
Die Nebenaktion "DSGVO-Auskunft" steht neben der Hauptaktion (auf dem
Handy im Menü "Mehr"); "Archivieren" bzw. "Reaktivieren" und
"Anonymisieren (DSGVO)" stehen immer im Menü "Mehr".

Bereits ausgestellte Rechnungen ändern sich nicht. Klein.Buch
schreibt zum Zeitpunkt der Festschreibung eine Kopie der
Kunden-Adresse in die Rechnung. Spätere Änderungen am Kontakt
wirken nur auf neue Belege. Das ist GoBD-Pflicht und schützt Deine
Buchführung.

## Kontakt archivieren

Einen Kontakt, mit dem Du nicht mehr arbeitest, archivierst Du im
Menü "Mehr" mit "Archivieren". Das geht ohne Rückfrage, weil es sich
umkehren lässt: Die Meldung "Kontakt archiviert." bietet
"Rückgängig" an. Die Seite bleibt stehen und zeigt den Hinweis, dass
der Kontakt archiviert ist. Ein archivierter Kontakt erscheint nicht
mehr in Auswahllisten; seine Rechnungen und Angebote bleiben
erhalten. Mit "Reaktivieren" im Menü "Mehr" holst Du ihn zurück.

## Notizen

Du kannst pro Kontakt freie Notizen eintragen (zum Beispiel
Zahlungs-Gewohnheiten, Sonder-Absprachen, Telefon-Notizen).
Klein.Buch nimmt Notizen derzeit nicht in die DSGVO-Auskunft auf.
Das ist eine Produkt-Entscheidung, keine gesetzliche Ausnahme: Bei
einer förmlichen Auskunfts-Anforderung nach Art. 15 DSGVO musst
Du Notizen mit personenbezogenem Inhalt grundsätzlich offenlegen.
Schreib deshalb nichts hinein, das die betroffene Person nicht
sehen sollte.

## DSGVO-Auskunft

Klick auf der Seite des Kontakts auf "DSGVO-Auskunft". Klein.Buch
fragt "DSGVO-Auskunft erstellen?"; mit "Auskunft erstellen" erstellt
es ein ZIP-Paket mit einer PDF-Zusammenfassung, allen Rechnungen,
Storno-Belegen und Angeboten zu diesem Kontakt und den Daten in
maschinenlesbarem Format (JSON). Vorher fragt Klein.Buch noch einmal
Dein Daten-Passwort und den App-Code ab. Dein Browser speichert das
ZIP in seinem Download-Ordner; von dort sendest Du es der
anfragenden Person.

Klein.Buch protokolliert die Auskunft im Audit-Log: Zeitpunkt,
Kontakt-Kennung und Anzahl der enthaltenen Belege. Der Name selbst
landet aus Datenschutz-Gründen nicht im Protokoll. Mehr dazu im
Kapitel "DSGVO-Auskunft und Anonymisierung".

## Anonymisierung

Wähle im Menü "Mehr" den Eintrag "Anonymisieren (DSGVO)". Klein.Buch
warnt Dich, dass die Aktion unumkehrbar ist. Bestätige mit
"Anonymisieren" und danach mit Daten-Passwort und App-Code. Die App
ersetzt den Namen durch einen anonymen Platzhalter (`Anonymisiert
#2E3F4A5B`, die Hex-Folge ist je Kontakt anders), leert alle weiteren
persönlichen Felder und archiviert den Kontakt.

Bereits ausgestellte Rechnungen bleiben mit der damals
festgeschriebenen Adresse erhalten. Das ist gesetzliche Pflicht
laut GoBD und nicht abschaltbar. Eine Funktion, die alle Daten auf
einmal löscht, gibt es in Klein.Buch nicht.

Klein.Buch lässt eine Anonymisierung nur zu, wenn keine offenen
Entwurfs-Belege zu dem Kontakt existieren. Schreibe diese Entwürfe
vorher fest oder verwirf sie (bei Rechnungs- und Angebots-Entwürfen
jeweils im Menü "Mehr" mit "Entwurf verwerfen"). Verworfene Entwürfe
bleiben mit ihrer Nummer erhalten und stehen der Anonymisierung nicht
im Weg.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
