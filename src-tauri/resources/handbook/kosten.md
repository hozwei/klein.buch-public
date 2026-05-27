---
slug: kosten
title: Kosten
category: bedienen
order: 140
keywords: [kosten, ausgabe, eingangsrechnung, beleg, brutto, kategorie, e-rechnung-empfang]
---

# Kosten

Im Bereich "Kosten" verwaltest Du Deine geschäftlichen Ausgaben.
Die Erst-Erfassung ist im Kapitel "Kosten erfassen" beschrieben;
hier steht die laufende Bedienung.

## Liste der Kosten

Die Hauptansicht zeigt Beleg-Datum, Zahlungs-Datum, Lieferant,
Beschreibung, Brutto-Betrag und Kategorie. Du sortierst über die
Spaltenköpfe und suchst über das Suchfeld oben. Filter: nach
Geschäftsjahr, nach Kategorie, nach Lieferant.

Klick auf eine Zeile, um den Beleg-Anhang zu öffnen oder die
Stamm-Daten einzusehen. Die Felder sind nach dem Speichern
gesperrt, weil Klein.Buch eine Kosten-Position direkt mit dem
Speichern festschreibt.

## Korrektur per Storno

Eine Kosten-Position lässt sich nicht direkt bearbeiten, weil sie
schon festgeschrieben ist. Wenn Du Dich vertippt hast, erzeugst
Du eine Storno-Buchung. Klick auf "Stornieren". Klein.Buch legt
eine Gegen-Buchung mit umgekehrtem Vorzeichen an. Trage danach eine
neue korrekte Position ein.

Beide Positionen (Original und Storno) bleiben sichtbar. Der EÜR-
Effekt ist neutral, sobald beide das gleiche Datum haben.

## E-Rechnung importieren

Klick auf "E-Rechnung importieren" und wähle eine XRechnung-XML
oder ZUGFeRD-PDF aus. Klein.Buch liest die XML-Daten, füllt die
Felder automatisch und legt die Original-Datei im Archiv ab.

Klein.Buch prüft die Datei zusätzlich mit dem KoSIT-Validator. Wenn
die XML Fehler hat (zum Beispiel fehlende Pflicht-Angabe des
Absenders), zeigt die App die Fehlermeldung an. Du kannst die
Position trotzdem speichern, weil Du als Empfänger nicht für die
Korrektheit der Eingangs-XRechnung haftest.

Trage nach dem Import noch das Zahlungs-Datum und die Kategorie
ein. Speichere.

## Wiederkehrende Abos

Klick auf "Wiederkehrende Abos" und dann auf "+ Neues Abo". Trage
Lieferant, Beschreibung, Brutto-Betrag, Kategorie, Rhythmus
(monatlich, vierteljährlich, halbjährlich, jährlich), Stichtag und
Modus ein. Es gibt zwei Modi:

**Automatisch.** Klein.Buch bucht am Stichtag selbst und nimmt den
Stichtag als Zahlungs-Datum. Sinnvoll für stehende Lastschriften,
bei denen der Betrag pünktlich auf den Cent gleich bleibt.

**Erinnerung.** Klein.Buch legt am Stichtag nur einen Eintrag unter
"Hinweise" an. Du öffnest ihn, klickst auf "buchen", prüfst den
Betrag und das tatsächliche Zahlungs-Datum und speicherst. Sinnvoll,
wenn der echte Betrag je Monat etwas abweicht.

## Beleg-Anhang anschauen

Klick auf das Büroklammer-Symbol einer Zeile. Klein.Buch öffnet den
Beleg in einem eigenen Fenster oder im Standard-PDF-Programm
Deines Betriebssystems. Der Beleg liegt im Archiv und wird beim
Öffnen gegen seinen Prüfwert geprüft. Wenn jemand die Datei im
Archiv heimlich verändert hat, schlägt Klein.Buch Alarm.

## Roh-XML einer Eingangsrechnung ansehen

Bei einer importierten oder über den Rechnungs-Eingang übernommenen
E-Rechnung kannst Du die ursprüngliche XML-Datei einsehen, also die
maschinenlesbaren Felder, die Du nicht im normalen Eingabe-Formular
siehst. Öffne dazu den Eingangsbeleg und klick auf "Roh-XML
anzeigen". Klein.Buch zeigt die XML formatiert in einem eigenen
Fenster mit einem Knopf zum Kopieren.

Das ist praktisch, wenn ein Steuerberater oder ein Prüfer in die
Original-Felder schauen will. Die Anzeige verändert nichts am Beleg.
Bei eigenen Eingaben über "Neue Kosten-Position" gibt es keine
XML — der Knopf ist dann ausgegraut.

## EÜR-Wirkung

In der EÜR taucht eine Kosten-Position als Ausgabe im Jahr des
Zahlungs-Datums auf. Wenn Du eine Rechnung im Dezember 2026
bekommst und erst im Januar 2027 bezahlst, gehört die Ausgabe in
das EÜR-Jahr 2027. Das ist die Cash-Basis nach §11 EStG.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
