---
slug: kontakte
title: Kontakte
category: bedienen
order: 160
keywords: [kontakte, kunde, lieferant, adressbuch, dsgvo, anonymisierung]
---

# Kontakte

Im Bereich "Kontakte" verwaltest Du alle Personen und Firmen, mit
denen Du geschäftlich zu tun hast: Kunden, Lieferanten und alle
anderen Geschäftspartner. Die Daten werden in Rechnungen, Angeboten
und Kosten-Positionen referenziert.

## Liste der Kontakte

Die Hauptansicht zeigt Name, Typ (Kunde, Lieferant, Partner), Ort,
USt-IdNr. und E-Mail. Klick auf eine Zeile, um den Kontakt zu
öffnen.

Über dem Listen-Kopf findest Du ein Suchfeld (durchsucht Name,
E-Mail, USt-IdNr., Stadt und Postleitzahl) und einen Schalter, um
archivierte Kontakte einzublenden.

## Neuen Kontakt anlegen

Klick auf "+ Neuer Kontakt". Trage Name oder Firmenbezeichnung,
Anschrift, E-Mail-Adresse und optional Telefon ein. Setze den Typ
(Kunde, Lieferant, Partner oder eine Kombination).

Bei Geschäftskunden im EU-Ausland trägst Du die Umsatzsteuer-
Identifikations-Nummer ein. Bei Endverbrauchern ohne USt-IdNr
lässt Du das Feld leer.

Speichere. Klein.Buch legt den Kontakt an. Du kannst ihn ab sofort
in Rechnungen und Angeboten auswählen.

## Kontakt bearbeiten

Klick auf einen Kontakt und bearbeite die Felder. Speichere.

Bereits ausgestellte Rechnungen ändern sich nicht. Klein.Buch
schreibt zum Zeitpunkt der Festschreibung eine Kopie der
Kunden-Adresse in die Rechnung. Spätere Änderungen am Kontakt
wirken nur auf neue Belege. Das ist GoBD-Pflicht und schützt Deine
Buchführung.

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

Klick auf "DSGVO-Auskunft" im Kontakt-Fenster. Klein.Buch erstellt
ein ZIP-Paket mit einer PDF-Zusammenfassung, allen Rechnungen,
Stornos und Angeboten zu diesem Kontakt und den Daten in
maschinenlesbarem Format (JSON). Speichere das ZIP und sende es der
anfragenden Person.

Klein.Buch protokolliert die Auskunft im Audit-Log: Zeitpunkt,
Kontakt-Kennung und Anzahl der enthaltenen Belege. Der Name selbst
landet aus Datenschutz-Gründen nicht im Protokoll. Mehr dazu im
Kapitel "DSGVO-Auskunft und Anonymisierung".

## Anonymisierung

Klick auf "Anonymisieren (DSGVO)". Klein.Buch warnt Dich, dass die
Aktion unumkehrbar ist. Bestätige. Die App ersetzt den Namen durch
einen anonymen Platzhalter (`Anonymisiert #2E3F4A5B` — die Hex-Folge
ist je Kontakt anders) und leert alle weiteren persönlichen Felder.

Bereits ausgestellte Rechnungen bleiben mit der damals
festgeschriebenen Adresse erhalten. Das ist gesetzliche Pflicht
laut GoBD und nicht abschaltbar. Eine echte Total-Löschung läuft
nur über "Einstellungen → Zurücksetzen" und betrifft die ganze
Klein.Buch-Instanz, nicht einzelne Personen.

Klein.Buch lässt eine Anonymisierung nur zu, wenn keine offenen
Entwurfs-Belege zu dem Kontakt existieren. Schließe oder lösche
diese Entwürfe vorher.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
