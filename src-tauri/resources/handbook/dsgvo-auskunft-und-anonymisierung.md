---
slug: dsgvo-auskunft-und-anonymisierung
title: DSGVO-Auskunft und Anonymisierung
category: bedienen
order: 260
keywords: [dsgvo, auskunft, anonymisierung, artikel 15, artikel 17, kontakt, datenschutz]
---

# DSGVO-Auskunft und Anonymisierung

Die Datenschutz-Grundverordnung gibt Personen das Recht zu
erfahren, welche Daten Du über sie speicherst, und unter
bestimmten Bedingungen das Recht, die Daten löschen zu lassen.
Klein.Buch hat für beides eine Funktion direkt im Kontakt-Fenster.

## Auskunft erzeugen

Klick auf "Kontakte", öffne den Kontakt, dessen Daten angefragt
sind, und klick auf "DSGVO-Auskunft". Weil das Paket alle Daten der
Person enthält, fragt Klein.Buch vorher noch einmal Dein
Daten-Passwort und den App-Code ab. Dann erzeugt es ein ZIP-Paket
mit:

1. Einer PDF-Zusammenfassung der gespeicherten Stamm-Daten
2. Allen Rechnungen, Stornos und Angeboten als PDF
3. Allen Original-Belegen aus dem Archiv, sofern dem Kontakt
   zuordenbar
4. Den Roh-Daten als JSON (maschinenlesbar)
5. Einer Inhalts-Übersicht

Dein Browser speichert das ZIP in seinem Download-Ordner. Es ist
nicht verschlüsselt, weil es an die anfragende Person geht.
Versende es per E-Mail mit einem ausreichend sicheren Kanal oder
übergib es persönlich.

Klein.Buch protokolliert die Auskunft im Audit-Log: Zeitpunkt,
Kontakt-Kennung und Anzahl der enthaltenen Belege. Aus
Datenschutz-Gründen landet der Name selbst nicht im Protokoll. Du
kannst die Auskunfts-Erteilung trotzdem über die Kontakt-Kennung
einer Person zuordnen.

## Was nicht in der Auskunft steht

Klein.Buch nimmt absichtlich nicht in die Auskunft auf:

1. Interne Notizen am Kontakt. Das sind betriebliche
   Hilfs-Aufzeichnungen, kein für Dritte vorgesehener Inhalt.
2. Privatbewegungen. Sie haben keinen Kontakt-Bezug.
3. Daten anderer Personen, die in derselben Rechnung erscheinen
   könnten (zum Beispiel beim Rechnungs-Empfänger einer
   Abtretung).

Wenn Du dazu Fragen hast (was zählt als "personenbezogenes Datum"
in diesem konkreten Fall), frag einen Anwalt oder den
Landes-Datenschutzbeauftragten Deines Bundeslandes.

## Anonymisierung

Klick im Kontakt-Fenster auf **"Anonymisieren (DSGVO)"**.
Klein.Buch zeigt eine Warnung, dass die Aktion endgültig ist.
Bestätige. Die App ersetzt den Namen durch einen Platzhalter
(`Anonymisiert #2E3F4A5B` — die Hex-Folge ist je Kontakt anders)
und leert alle weiteren persönlichen Felder am Kontakt.

Bereits ausgestellte Rechnungen, die zum Zeitpunkt der
Festschreibung die Kunden-Adresse als Kopie in den Beleg
geschrieben hatten, bleiben mit dieser alten Adresse erhalten. Das
ist gesetzliche Pflicht laut §147 AO und der GoBD. Klein.Buch
kann diese Daten nicht löschen, ohne Deine Buchführung zu zerstören.

Eine Funktion, die alle Daten auf einmal löscht, gibt es in
Klein.Buch nicht. Eine vollständige Löschung beträfe ohnehin die
gesamte Klein.Buch-Instanz, nicht einzelne Personen, und bliebe an
die gesetzlichen Aufbewahrungsfristen gebunden.

## Anonymisierung verweigern

Klein.Buch blockiert die Anonymisierung, wenn zu dem Kontakt offene
Entwurfs-Belege existieren. Schließe die Entwürfe vorher
(festschreiben oder verwerfen) und versuche erneut.

Die App lässt eine bereits anonymisierte Person nicht ein zweites
Mal anonymisieren; die Knöpfe "DSGVO-Auskunft" und "Anonymisieren
(DSGVO)" erscheinen dann nicht mehr.

## Rechtsgrundlagen

Die Auskunft beruht auf DSGVO Art. 15. Die Anonymisierung
entspricht DSGVO Art. 17 ("Recht auf Löschung"), eingeschränkt
durch §147 AO (gesetzliche Aufbewahrungspflichten). Die
Aufbewahrung der bereits ausgestellten Belege ist die Ausnahme,
die das Gesetz selbst ausdrücklich vorsieht.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
