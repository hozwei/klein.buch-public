---
slug: kosten
title: Kosten
category: bedienen
order: 140
keywords: [kosten, ausgabe, eingangsrechnung, beleg, brutto, kategorie, e-rechnung-empfang, storno, abo, bezahlt]
---

# Kosten

Im Bereich "Kosten" verwaltest Du Deine geschäftlichen Ausgaben.
Die Erst-Erfassung ist im Kapitel "Kosten erfassen" beschrieben;
hier steht die laufende Bedienung.

## Liste der Kosten

Die Hauptansicht zeigt je Eintrag Nummer, Beleg-Datum, Lieferant,
Beschreibung mit der Kategorie in einer zweiten Zeile, Brutto-Betrag,
Zahlungs-Datum und Status. Der Status heißt "Bezahlt" oder
"Unbezahlt"; fehlt der Beleg, steht daneben "Kein Beleg". Über der
Liste suchst Du nach Beleg-Nummer, Lieferant oder Beschreibung und
filterst nach Geschäftsjahr und Kategorie. Stornierte Kosten blendet
die Liste aus, bis Du "Stornierte anzeigen" anhakst. "Filter
zurücksetzen" zeigt wieder alles. Darunter stehen Anzahl und Summe der
angezeigten Einträge, ohne Stornos.

Über die Nummer oder den Knopf "Öffnen" kommst Du zur Seite der Kosten
(Titel "Kosten KO-…"). Dort siehst Du alle Angaben, die Zahlung und den
Beleg. Die Angaben selbst lassen sich nicht ändern, weil Klein.Buch
Kosten schon beim Erfassen festschreibt.

## Zahlung nachtragen

Kosten, die Du als "noch nicht bezahlt" erfasst hast, stehen als
"Unbezahlt" in der Liste und zählen noch nicht in der EÜR. Ist das Geld
abgegangen, öffne die Kosten und klick auf "Als bezahlt markieren".
Trag das Datum unter "Bezahlt am" ein, wähle bei Bedarf das
Zahlungs-Konto und klick auf "Zahlung speichern". Liegt das Zahldatum
vor dem Beleg-Datum (etwa bei Vorkasse), weist Klein.Buch darauf hin;
das ist erlaubt, prüf nur, ob beide Daten stimmen.

Hast Du Dich beim Zahldatum vertan, nimm "Zahlung korrigieren". "Als
unbezahlt markieren" setzt die Kosten ohne Rückfrage wieder auf offen;
mit "Rückgängig" in der Einblendung danach holst Du die Zahlung zurück.
Das Zahldatum gehört nicht zu den festgeschriebenen Angaben, jede
Änderung steht aber im Protokoll.

## Korrektur per Storno

Kosten lassen sich nicht bearbeiten, weil sie schon festgeschrieben
sind. Wenn Du Dich vertippt hast, stornierst Du sie: Öffne die Kosten,
wähle im Menü "Mehr" den Eintrag "Stornieren", trag einen Storno-Grund
ein (Pflicht, zum Beispiel "doppelt erfasst") und klick auf "Kosten
stornieren".

Die Kosten bleiben mit dem Status "Storniert" erhalten und zählen nicht
mehr in der EÜR; in der Liste siehst Du sie mit "Stornierte anzeigen".
Auf ihrer Seite steht danach "Korrigiert neu erfassen". Der Knopf öffnet
das Formular für neue Kosten mit allen Werten des Originals, auch mit
Beleg- und Zahldatum. Du korrigierst den Fehler, lädst den Beleg neu hoch
und schreibst die neuen Kosten fest. Das Original bleibt unverändert.

Nicht stornieren lassen sich Kosten, deren Geschäftsjahr (des Belegs
oder der Zahlung) schon abgeschlossen ist, und Kosten, die Du als Anlage
übernommen hast. Wie Du einen rückabgewickelten Kauf einer Anlage
erfasst, steht im Kapitel "Anschaffungen und AfA".

## Ähnliche Kosten als Vorlage

Kommt eine Ausgabe in ähnlicher Form wieder, öffne die früheren Kosten
und klick auf "Als Vorlage für neue Kosten" (auf dem Handy im Menü
"Mehr"). Das Formular übernimmt Lieferant, Kategorie, Beschreibung,
Beträge, Zahlungs-Konto, den §13b-Haken und die Notiz. Beleg- und
Zahldatum stehen auf heute, die Rechnungs-Nr. des Lieferanten bleibt
leer, den Beleg wählst Du neu. Es entstehen neue Kosten; die früheren
bleiben unverändert.

## E-Rechnung importieren

Klick in der Kosten-Liste auf "E-Rechnung importieren" (auf dem Handy im
Menü "Mehr"), wähle eine XRechnung-XML oder ZUGFeRD-PDF aus und klick
auf "Einlesen und prüfen". Klein.Buch lädt die Datei hoch, liest die
XML-Daten, füllt die Felder automatisch und legt die Original-Datei beim
Festschreiben im Archiv ab.

Klein.Buch prüft die Datei zusätzlich mit dem KoSIT-Validator. Wenn
die XML Fehler hat (zum Beispiel fehlende Pflicht-Angabe des
Absenders), zeigt die App die Fehlermeldung an. Du kannst die
Position trotzdem festschreiben, weil Du als Empfänger nicht für die
Korrektheit der Eingangs-XRechnung haftest.

Trage vor dem Festschreiben noch das Zahlungs-Datum und die Kategorie
ein und klick auf "Kosten festschreiben". Mehr dazu im Kapitel
"Rechnungseingang per Upload".

## Kosten-Abos

Klick in der Kosten-Liste auf "Kosten-Abos" (auf dem Handy im
Menü "Mehr") und dann auf "+ Neues Abo". Trag eine Bezeichnung ein,
wähle bei Bedarf Lieferant und Kategorie, dann Frequenz (monatlich,
vierteljährlich, halbjährlich, jährlich), Stichtag (Tag im Monat),
nächste Fälligkeit, den erwarteten Betrag (Brutto) und die
Beschreibungs-Vorlage, also den Text der erzeugten Kosten. Zuletzt
wählst Du, was am Stichtag passieren soll, und klickst auf "Abo
anlegen". Abo-Kosten entstehen immer mit Netto = Brutto und
Umsatzsteuer 0, weil das Abo keine Aufteilung kennt. Es gibt zwei Modi:

**Automatisch.** Klein.Buch bucht am Stichtag selbst eine
Kosten-Position mit dem Stichtag als Beleg-Datum. Die Position entsteht
"noch nicht bezahlt": Das Zahlungs-Datum bestätigst Du, wenn die
Abbuchung auf dem Konto steht (erst dann zählt sie in der EÜR).
Sinnvoll für stehende Lastschriften mit gleichem Betrag.

**Erinnerung.** Klein.Buch bucht nichts selbst. Am Stichtag erscheint
unter "Hinweise" der Eintrag "Abo fällig", und das Abo ist in der Liste
als fällig markiert ("Heute fällig", später "Überfällig"). Klick dort
auf "Buchen": Klein.Buch legt die Kosten-Position mit dem Stichtag als
Beleg-Datum und dem erwarteten Betrag an. Vorher fragt es "Kosten
festschreiben?" und zeigt Abo, Belegdatum und Betrag — nach dem Bestätigen
ist die Position festgeschrieben. Markiere sie als bezahlt, sobald die
Zahlung abgebucht ist. Weicht der Betrag ab, erfasse den Termin stattdessen
über "+ Neue Kosten" und setze im Abo unter "Bearbeiten" die nächste
Fälligkeit weiter; eine schon gebuchte falsche Position stornierst Du und
erfasst sie mit "Korrigiert neu erfassen" neu. Sinnvoll, wenn Du jeden Termin selbst anstoßen
willst.

In der Abo-Liste steht je Abo eine Aktion, die zu seinem Zustand passt:
"Rückstand …", "Buchen", "Bearbeiten" oder bei einem pausierten Abo
"Fortsetzen". Die übrigen Aktionen (etwa "Bearbeiten" und "Pausieren")
liegen im Menü "Mehr" der Zeile. Pausieren geht ohne Rückfrage; mit
"Rückgängig" in der Einblendung setzt Du das Abo gleich wieder fort.
Pausierte Abos zeigt die Liste erst mit "Pausierte anzeigen". Sind
automatische Abos fällig, bucht der Knopf "… fällige Auto-Abos jetzt
buchen" sie sofort; sonst übernimmt das der nächste Lauf im Hintergrund.

### Verpasste Termine

War Klein.Buch längere Zeit gesperrt (etwa nach einem Neustart des
Servers, bis sich jemand anmeldet) oder war ein Abo pausiert, sind
mehrere Termine offen. Bis zu drei offene Termine eines automatischen
Abos bucht Klein.Buch beim nächsten Lauf selbst nach, je Termin mit
dem jeweiligen Stichtag; ein Hinweis nennt die nachgebuchten Termine.

Sind mehr Termine offen, bucht Klein.Buch nichts und meldet unter
"Hinweise": "Abo-Kosten: Entscheidung nötig". Klick in der Abo-Liste
beim Abo auf "Rückstand …" und wähle:

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

Der Dialog nennt, wie viele Termine offen sind, und ihre Daten (die
ersten sechs, danach "und N weitere"), dazu, wie viele Termine "Ab heute
fortsetzen" auslässt und mit welchem Termin das Abo weitermacht. Sind
mehr als 24 Termine offen, bietet er "Nachholen" nicht an: Dann bleibt
"Ab heute fortsetzen", und Du erfasst fehlende Kosten einzeln oder passt
den nächsten Termin im Abo an. Weil sich das Auslassen nicht
zurücknehmen lässt, steht der Fokus dann auf "Abbrechen", und ein Klick
neben den Dialog schließt ihn nicht. Der Dialog zeigt den Stand beim Öffnen;
Klein.Buch prüft bei Deiner Wahl noch einmal.

## Beleg-Anhang anschauen

Öffne die Kosten-Position und klick im Abschnitt "Beleg" auf
"Beleg herunterladen". Dein Browser speichert die Datei in seinem
Download-Ordner, von dort öffnest Du sie. Fehlt der Beleg, zeigt die
Liste beim Eintrag "Kein Beleg". Der Beleg liegt im
Archiv und wird vor dem Herunterladen gegen seinen Prüfwert geprüft.
Wenn jemand die Datei im Archiv heimlich verändert hat, schlägt
Klein.Buch Alarm.

Weitere Dateien, etwa ein nachgereichtes Foto, hängst Du im Abschnitt
"Zusätzliche Anhänge" an: Datei wählen, auf Wunsch eine Bezeichnung
eintragen und auf "Anhängen" klicken. Erlaubt sind PDF, Bilder und XML
bis 20 MB.

## Roh-XML einer Eingangsrechnung ansehen

Bei einer über "E-Rechnung importieren" hochgeladenen
E-Rechnung kannst Du die ursprüngliche XML-Datei einsehen, also die
maschinenlesbaren Felder, die Du nicht im normalen Eingabe-Formular
siehst. Öffne dazu den Eingangsbeleg und klick auf "Roh-XML
anzeigen" (auf dem Handy im Menü "Mehr"). Klein.Buch zeigt die XML
formatiert im Dialog "Roh-XML der E-Rechnung", dazu Format, Prüfwert
und Größe; "XML kopieren" legt sie in die Zwischenablage.

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

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
