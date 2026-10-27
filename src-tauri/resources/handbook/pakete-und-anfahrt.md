---
slug: pakete-und-anfahrt
title: Pakete und Anfahrt
category: bedienen
order: 120
keywords: [paket, leistungspaket, anfahrt, kilometer, vorlage, broschüre, katalog, kategorie, version, wiederverwenden, anfahrtspauschale]
---

# Pakete und Anfahrt

Pakete sind wiederverwendbare Leistungen, die Du immer wieder
gleich abrechnest. Anfahrt ist die kilometer-basierte Berechnung
des Wegs zum Kunden. Beide sparen Tipparbeit bei jeder neuen
Rechnung.

## Pakete anlegen und pflegen

Klick im Hauptmenü auf "Pakete". Klein.Buch zeigt Deine Pakete als
Liste je Kategorie (zum Beispiel Wartung, Schulung, Vor-Ort-Service)
mit Paket, Version, Datum der letzten Änderung und Status ("Aktiv"
oder "Archiviert"). Pakete ohne Kategorie stehen unter "Ohne
Kategorie". Kategorien sind optional.

Klick auf "+ Neues Paket". Auf der Seite "Neues Paket" trägst Du ein:

1. **Paket-Name (intern)**, den nur Du siehst,
2. optional eine **Kategorie**,
3. den **Positions-Titel (auf dem Beleg)**,
4. den **Netto-Preis**,
5. die **Einheit** als Auswahl im Klartext (Stunde, Stück, Pauschale
   und weitere; vorbelegt ist "Pauschale", in der E-Rechnung steht
   dafür ein Einheiten-Code),
6. die **Beschreibung**. Die Knöpfe über dem Textfeld ("Überschrift",
   "Zwischentitel", "Fett", "Kursiv", "Aufzählung", "Nummerierung")
   setzen einfache Markdown-Formatierung (Markdown ist eine schlichte
   Schreibweise für Formatierungen, etwa `**fett**`). Daneben zeigt eine
   Vorschau ungefähr, wie es aussieht; der Knopf "PDF-Vorschau" zeigt
   das exakte Ergebnis auf dem Beleg.

Paket-Name, Positions-Titel und Netto-Preis sind Pflicht. Klick auf
"Paket anlegen". Das Paket ist damit als Version 1 aktiv und sofort in
Rechnungen, Angeboten und der Broschüre auswählbar. Einen eigenen
Schritt zum Veröffentlichen gibt es nicht.

Wenn Du ein Paket später änderst, entsteht eine neue Version. Öffne
dazu das Paket mit "Bearbeiten" in der Liste oder über seinen Namen.
Auf der Seite "Paket „…“ bearbeiten" änderst Du die Felder und klickst
auf "Als neue Version speichern"; "Abbrechen" verwirft die Änderungen.
Bereits in Angebote oder Rechnungen eingefügte Paket-Positionen
bleiben dabei unverändert, weil sie eine eingefrorene Kopie der
damaligen Version sind. Das schützt Deine Buchführung vor
nachträglicher Verfälschung.

Ältere Pakete können als Einheit noch einen Freitext wie "Pauschal"
tragen. Dann steht am Feld der Hinweis, dass das keine Einheit der
E-Rechnung ist. Wähl eine Einheit aus der Liste, zum Beispiel
"Pauschale"; sie gilt ab der nächsten Version.

## Versionen und Wiederverwenden

Unter "Versionen" listet die Bearbeiten-Seite jede Version mit
Nummer, Datum, Positions-Titel, Netto-Preis und Einheit; die aktuelle
trägt "Aktiv". Bei jeder älteren Version steht "Wiederverwenden".
Klein.Buch fragt dann "Version N wiederverwenden?" und legt nach
"Als neue Version übernehmen" eine neue Version mit dem Inhalt der
alten an, die zur aktiven wird. Nichts wird überschrieben: Alle
bisherigen Versionen bleiben erhalten, und Positionen in bestehenden
Angeboten und Rechnungen ändern sich nicht. Nicht gespeicherte
Änderungen im Formular darüber gehen dabei verloren.

Gibt es mehr als eine Version, stellst Du unter "Versionen
vergleichen" mit "Links" und "Rechts" zwei Versionen nebeneinander.

## Kategorien

Kategorien pflegst Du unten auf der Seite "Pakete" in der Karte
"Kategorien". Eine neue legst Du an, indem Du unter "Neue Kategorie"
einen Namen einträgst und auf "Kategorie anlegen" klickst.
"Umbenennen" öffnet in der Zeile das Feld "Neuer Name für …"; mit
"Umbenennen" speicherst Du, mit "Abbrechen" bleibt der alte Name.
Die Reihenfolge änderst Du mit "Nach oben" und "Nach unten". Sie gilt
für die Liste der Pakete, für die Auswahl beim Einfügen in Belege und
für die Broschüre.

## Paket archivieren

Ein Paket, das Du nicht mehr anbietest, archivierst Du in der Liste
über das Menü "Mehr" seiner Zeile mit "Archivieren". Das geht ohne
Rückfrage; die Meldung danach bietet "Rückgängig" an. Ein archiviertes
Paket bleibt mit allen Versionen erhalten, ist aber in Belegen und
der Broschüre nicht mehr auswählbar. Mit "Reaktivieren" im selben
Menü holst Du es zurück.

## Paket im Beleg verwenden

In einer Rechnung oder einem Angebot steht unter der Positions-Liste
das Feld "Paket". Wähle dort das Paket aus (nach Kategorien
gegliedert) und klick auf **"+ Paket"**. Klein.Buch fügt die
Paket-Daten als neue Position ein: Titel, Beschreibung, Preis. Die
Position ist eine Kopie der aktuellen Paket-Version, keine
Live-Bindung; ein Vermerk "Aus Paket: …" mit der Versionsnummer zeigt
die Herkunft.

Wenn Du an der eingefügten Position etwas ändern willst, klick auf
**"Paket anpassen"**. Sobald Du editierst, verliert die Position
den Bezug zur Paket-Vorlage und wird zu einer freien
Beleg-Position. Spätere Änderungen an der Paket-Vorlage wirken auf
diese Position nicht mehr. Der Vermerk "Aus Paket" verschwindet von
der Position.

## Anfahrt einrichten

In den Einstellungen unter **"Anfahrtspauschale"** trägst Du unter
"Preis pro Kilometer" Deinen Kilometer-Satz ein (zum Beispiel 0,30;
die steuerlich übliche Kilometerpauschale ist ein guter Startwert).
Mit "Standardmäßig Hin & Rück (×2) vorschlagen" ist das Häkchen
"Hin & Rück" im Beleg schon gesetzt. "Speichern" übernimmt die Werte,
"Abbrechen" führt ohne Änderung zurück.

Im Beleg steht unter der Positions-Liste das Feld "Anfahrt (km)"
mit dem hinterlegten Satz. Trag die einfache Strecke in Kilometern
ein, hake bei Bedarf "Hin & Rück" an und klick auf **"+ Anfahrt"**.
Klein.Buch verdoppelt bei "Hin & Rück" die Strecke, rechnet sofort
Kilometer mal Satz aus und fügt eine normale Position ein. Deren
Beschreibung kannst Du danach anpassen (zum Beispiel "Anfahrt zur
Baustelle Beispielstraße 5"). Ist noch kein Kilometer-Satz
hinterlegt, führt dort ein Link zur Anfahrtspauschale.

Klein.Buch hat keine eingebaute Karten-Funktion und kein Geocoding.
Du gibst die Kilometer von Hand ein. Das ist Absicht: Klein.Buch
schickt keine Kunden-Adressen an einen externen Karten-Dienst.

## Paket-Katalog als Broschüre

Wenn Du Deinen Kunden Deine Pakete zeigen willst, ohne ein konkretes
Angebot zu schreiben, erzeugst Du eine Paket-Katalog-Broschüre.
Klick im Bereich "Pakete" oben auf **"Broschüre"** (auf dem Handy im
Menü "Mehr"). Auf der Seite "Paket-Broschüre" wählst Du die Pakete
aus, nach Kategorien gegliedert oder mit "Alle auswählen". **"PDF
erstellen"** erzeugt die Broschüre als PDF.

Zum Versenden wählst Du unter "Per E-Mail senden" das
Absender-Postfach und unter "Empfänger (Kontakt)" die Empfängerin
oder den Empfänger. Die Auswahl lässt sich durch Tippen durchsuchen
und zeigt nur Kontakte mit hinterlegter E-Mail-Adresse. Betreff und
Nachricht sind optional; ohne Nachricht erzeugt Klein.Buch einen
Standardtext. Klick auf **"Broschüre senden"**.

Wichtig: Eine Katalog-Broschüre ist kein §14-Beleg und kein
Angebot. Sie ist nur eine Übersicht. Klein.Buch protokolliert den
Versand im E-Mail-Protokoll (Art "Paket-Broschüre"), legt aber keinen
Buchhaltungs-Beleg an und vergibt keine Nummer.

## §19 in Paketen und Anfahrt

Paket-Preise und der Kilometer-Satz sind Netto-Beträge. Fügst Du ein
Paket oder eine Anfahrt in einen Beleg ein, bekommt die Position die
Steuerart, die am Datum dieses Belegs gilt (Leistungsdatum der
Rechnung, Datum des Angebots, Termin der Abo-Vorlage): Gilt dort die
Kleinunternehmer-Regelung, setzt Klein.Buch die Steuer-Kategorie
automatisch als steuerfrei nach §19 UStG und druckt den §19-Hinweis
auf das fertige PDF. Eine schon eingefügte Position ändert sich nicht,
wenn das Paket später eine neue Version bekommt.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
