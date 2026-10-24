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

Klick auf "E-Rechnung importieren", wähle eine XRechnung-XML oder
ZUGFeRD-PDF aus und klick auf "Einlesen & prüfen". Klein.Buch lädt
die Datei hoch, liest die XML-Daten, füllt die Felder automatisch und
legt die Original-Datei beim Festschreiben im Archiv ab.

Klein.Buch prüft die Datei zusätzlich mit dem KoSIT-Validator. Wenn
die XML Fehler hat (zum Beispiel fehlende Pflicht-Angabe des
Absenders), zeigt die App die Fehlermeldung an. Du kannst die
Position trotzdem speichern, weil Du als Empfänger nicht für die
Korrektheit der Eingangs-XRechnung haftest.

Trage vor dem Festschreiben noch das Zahlungs-Datum und die Kategorie
ein und klick auf "Als Kosten festschreiben". Mehr dazu im Kapitel
"Rechnungseingang per Upload".

## Wiederkehrende Abos

Klick auf "Wiederkehrende Abos" und dann auf "+ Neues Abo". Trage
Lieferant, Beschreibung, Brutto-Betrag, Kategorie, Rhythmus
(monatlich, vierteljährlich, halbjährlich, jährlich), Stichtag und
Modus ein. Es gibt zwei Modi:

**Automatisch.** Klein.Buch bucht am Stichtag selbst eine
Kosten-Position mit dem Stichtag als Beleg-Datum. Die Position entsteht
"noch nicht bezahlt": Das Zahlungs-Datum bestätigst Du, wenn die
Abbuchung auf dem Konto steht (erst dann zählt sie in der EÜR).
Sinnvoll für stehende Lastschriften mit gleichem Betrag.

**Erinnerung.** Klein.Buch bucht nichts selbst. Am Stichtag erscheint
unter "Hinweise" der Eintrag "Abo fällig", und das Abo steht in der
Liste als "fällig". Klick dort auf "Buchen", prüfe Betrag und Datum
der Kosten-Position und bestätige die Zahlung, sobald sie abgebucht
ist. Sinnvoll, wenn der echte Betrag je Monat etwas abweicht.

### Verpasste Termine

War Klein.Buch längere Zeit gesperrt (etwa nach einem Neustart des
Servers, bis sich jemand anmeldet) oder war ein Abo pausiert, sind
mehrere Termine offen. Bis zu drei offene Termine eines automatischen
Abos bucht Klein.Buch beim nächsten Lauf selbst nach, je Termin mit
dem jeweiligen Stichtag; ein Hinweis nennt die nachgebuchten Termine.

Sind mehr Termine offen, bucht Klein.Buch nichts und meldet unter
"Hinweise": "Entscheidung nötig". Klick in der Abo-Liste beim Abo auf
"Rückstand …" und wähle:

- **Nachholen:** Der nächste Lauf bucht die offenen Termine (höchstens
  24). Termine in einem bereits abgeschlossenen Geschäftsjahr werden
  übersprungen, weil dort nichts mehr gebucht werden kann.
- **Ab heute fortsetzen:** Die verpassten Termine werden ausgelassen;
  das Abo macht mit dem ersten Termin ab heute weiter. Fehlende
  Belege erfasst Du bei Bedarf einzeln.

Dieselbe Frage kommt, wenn Du ein pausiertes automatisches Abo mit
verpassten Terminen fortsetzt. Eine Bestätigung zum Nachholen gilt für die
Termine bis zu dem Tag, an dem Du sie gibst; kommen danach wieder mehr als
drei Termine zusammen, fragt Klein.Buch erneut.

## Beleg-Anhang anschauen

Öffne die Kosten-Position und klick im Abschnitt "Beleg" auf
"Beleg herunterladen". Dein Browser speichert die Datei in seinem
Download-Ordner, von dort öffnest Du sie. Fehlt der Beleg, zeigt die
Liste beim Eintrag den Hinweis "kein Beleg". Der Beleg liegt im
Archiv und wird vor dem Herunterladen gegen seinen Prüfwert geprüft.
Wenn jemand die Datei im Archiv heimlich verändert hat, schlägt
Klein.Buch Alarm.

## Roh-XML einer Eingangsrechnung ansehen

Bei einer über "E-Rechnung importieren" hochgeladenen
E-Rechnung kannst Du die ursprüngliche XML-Datei einsehen, also die
maschinenlesbaren Felder, die Du nicht im normalen Eingabe-Formular
siehst. Öffne dazu den Eingangsbeleg und klick auf "Roh-XML
anzeigen". Klein.Buch zeigt die XML formatiert in einem eigenen
Fenster mit einem Knopf zum Kopieren.

Das ist praktisch, wenn ein Steuerberater oder ein Prüfer in die
Original-Felder schauen will. Die Anzeige verändert nichts am Beleg.
Bei eigenen Eingaben über "+ Neue Kosten" gibt es keine XML, der
Knopf erscheint dann nicht.

## EÜR-Wirkung

In der EÜR taucht eine Kosten-Position als Ausgabe im Jahr des
Zahlungs-Datums auf. Wenn Du eine Rechnung im Dezember 2026
bekommst und erst im Januar 2027 bezahlst, gehört die Ausgabe in
das EÜR-Jahr 2027. Das ist die Cash-Basis nach §11 EStG.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.11*
